use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{ItemStruct, Type, parse_quote};

use crate::args::{self, Fetch, Keys, Step};
use crate::emit::bind::bind_impl;
use crate::emit::builder::builder;
use crate::emit::checks::{embeds, hook_needs, values};
use crate::emit::docs::item_docs;
use crate::emit::node::{fingerprint, node, target};
use crate::emit::run::transaction_methods;
use crate::emit::{self, krate};
use crate::model::fields::{self, Field};
use crate::model::role::Role;
use crate::naming::{snake_case, type_key};

pub(crate) struct TransactionArgs {
    dialect: Type,
    steps: Vec<Step>,
}

const KEYS: Keys = &[&["steps"]];

impl Parse for TransactionArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let dialect = input.parse()?;
        let mut steps = Vec::new();
        args::parse_keys(input, KEYS, |key, input| {
            match key.to_string().as_str() {
                "steps" => steps = args::steps(input)?,
                "before" | "after" => return Err(args::only_tables(key)),
                "sql" | "sql_file" | "name" | "cte" | "subquery" | "display" | "debug"
                | "parse_check" | "grammar" | "row" => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("transactions take only `steps(...)`, not `{key}`"),
                    ));
                }
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        if steps.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "transactions need `steps(Type, ...)`",
            ));
        }
        Ok(TransactionArgs { dialect, steps })
    }
}

pub(crate) fn expand(args: TransactionArgs, mut input: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "transactions can't be generic",
        ));
    }
    let fields = fields::items(&mut input)?;
    let steps = &args.steps;
    let ident = &input.ident;
    let dialect = &args.dialect;
    Step::check_ctes(steps)?;
    let mut flat = Vec::new();
    Step::flatten(steps, &mut flat);
    let mut used = vec![false; fields.len()];
    let mut types: Vec<&Type> = Vec::new();
    let mut referenced: Vec<&Type> = Vec::new();
    let mut parts = Vec::new();
    for (step, fetch) in &flat {
        let (item, ty) = match fields::resolve(step, &fields) {
            Some((index, field)) => {
                used[index as usize] = true;
                (Some(index), &field.ty)
            }
            None => {
                if !types.iter().any(|other| type_key(other) == type_key(step)) {
                    types.push(step);
                }
                (None, *step)
            }
        };
        if !referenced
            .iter()
            .any(|other| type_key(other) == type_key(ty))
        {
            referenced.push(ty);
        }
        let step = target(item, ty);
        parts.push(match fetch {
            Fetch::Cte => quote! {
                #krate::From::part(
                    #step,
                    #krate::AliasRule::NodeName,
                    ::core::option::Option::Some(#krate::Inject::cte(false)),
                )
            },
            _ => quote!(#krate::Expr::part(#step)),
        });
    }
    if let Some((field, _)) = fields.iter().zip(&used).find(|(_, used)| !**used) {
        return Err(syn::Error::new(
            field.ident.span(),
            format!(
                "the field `{}` isn't a step; list it in `steps(...)` or remove it",
                field.ident
            ),
        ));
    }
    let items: Vec<&Type> = fields.iter().map(|field| &field.ty).collect();
    let types: Vec<Type> = types.into_iter().cloned().collect();
    let embeds = embeds(&input, &items, &types, dialect);
    let node = node(
        &snake_case(&ident.to_string()),
        fingerprint(ident, &input),
        quote!(Transaction),
        quote!(#krate::Inject::Subquery),
        parts,
        &items,
        [&[], &[]],
        embeds.node,
    );
    let checked = embeds.checked;
    let hook_needs = hook_needs(&input, referenced.len(), |i, index| {
        let ty = referenced[i];
        parse_quote!(#ty: #krate::HookNeeds<__V, #index>)
    });
    let values = values(&input, !fields.is_empty());
    let bind = bind_impl(&input, &fields);
    let (definitions, builder) = builder(&input, &fields);
    let methods = transaction_methods(&input, dialect, &typed(steps, &fields));
    let mut documented = input.clone();
    documented
        .attrs
        .extend(item_docs(&input, Role::Transaction { steps }));
    let impls = emit::scoped(quote! {
        impl #krate::Sql for #ident {
            type Dialect = #dialect;
            const NODE: &'static #krate::Node = #node;
        }

        #values
        #checked
        #hook_needs
        #bind
        #builder

        #krate::impl_transaction!(#ident);
        #methods
    });
    Ok(quote! {
        #documented
        #definitions
        #impls
    })
}

// The steps with each field name replaced by the field's type.
fn typed(steps: &[Step], fields: &[Field]) -> Vec<Step> {
    steps
        .iter()
        .map(|step| match step {
            Step::Run(ty, fetch) => Step::Run(
                fields::resolve(ty, fields)
                    .map_or_else(|| ty.clone(), |(_, field)| field.ty.clone()),
                *fetch,
            ),
            Step::Savepoint(steps) => Step::Savepoint(typed(steps, fields)),
        })
        .collect()
}
