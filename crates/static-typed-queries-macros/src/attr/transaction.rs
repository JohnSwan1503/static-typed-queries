use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Fields, ItemStruct, Type};

use crate::args::{self, Fetch, Keys, Step};
use crate::emit::checks::{embeds, hook_needs};
use crate::emit::node::{fingerprint, node};
use crate::emit::{self, krate};
use crate::model::item::{Item, Role};
use crate::model::items::add_item;
use crate::model::params;
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
                | "parse_check" | "separate" | "grammar" | "row" => {
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

pub(crate) fn expand(args: TransactionArgs, input: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "transactions can't be generic",
        ));
    }
    if !matches!(input.fields, Fields::Unit) {
        return Err(syn::Error::new_spanned(
            &input.fields,
            "transactions can't have fields; each step returns its own rows",
        ));
    }
    let steps = &args.steps;
    let ident = &input.ident;
    let dialect = &args.dialect;
    Step::check_ctes(steps)?;
    let mut flat = Vec::new();
    Step::flatten(steps, &mut flat);
    let mut items = Vec::new();
    for (step, _) in &flat {
        add_item(step, &input.generics, &[], &mut items);
    }
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let item = Item::new(
        &input,
        Role::Transaction { steps },
        Vec::new(),
        params::children(&items, &[], &[]),
    );
    let parts = flat
        .iter()
        .map(|(step, fetch)| {
            let node = quote!(<#step as #krate::Sql>::NODE);
            match fetch {
                Fetch::Cte => quote! {
                    #krate::From::part(
                        #node,
                        #krate::AliasRule::NodeName,
                        ::core::option::Option::Some(#krate::Inject::cte(false)),
                    )
                },
                _ => quote!(#krate::Expr::part(#node)),
            }
        })
        .collect();
    let node = node(
        &snake_case(&ident.to_string()),
        fingerprint(ident, &input),
        quote!(Transaction),
        quote!(#krate::Inject::Subquery),
        parts,
        &fields,
        [&[], &[]],
    );
    let mut referenced: Vec<&Type> = Vec::new();
    for (step, _) in flat {
        if !referenced
            .iter()
            .any(|other| type_key(other) == type_key(step))
        {
            referenced.push(step);
        }
    }
    let needs: Vec<TokenStream> = referenced.iter().map(|ty| quote!(#ty)).collect();
    let hook_needs = hook_needs(&input, &needs, quote!(#krate::HookNeeds));
    let embed_checks = embeds(&input, &fields, dialect);
    let params_ty = item.ty();
    let params_struct = item.definition();
    let derives = item.derives();
    let bind_params = item.bind_impl();
    let (definitions, builder) = item.builder(dialect, None);
    let mut documented = input.clone();
    documented.attrs.extend(item.item_docs());
    let impls = emit::scoped(quote! {
        impl #krate::Sql for #ident {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::Node = #node;
        }

        #derives
        #embed_checks
        #hook_needs
        #bind_params
        #builder

        #krate::impl_transaction!(#ident);
    });
    Ok(quote! {
        #documented
        #params_struct
        #definitions
        #impls
    })
}
