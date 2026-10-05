use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Fields, ItemStruct, LitStr, Type};

use crate::args::{self, Fetch, Keys, Step};
use crate::emit::checks::{embeds, hook_needs};
use crate::emit::docs::Source;
use crate::emit::krate;
use crate::emit::node::{fingerprint, node};
use crate::model::items::add_item;
use crate::model::params::Params;
use crate::naming::{snake_case, type_key};
use crate::sql::analyze::{Analysis, Columns, Kind};
use crate::sql::template::Template;

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

pub(crate) fn expand(args: TransactionArgs, item: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "transactions can't be generic",
        ));
    }
    if !matches!(item.fields, Fields::Unit) {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "transactions can't have fields; each step returns its own rows",
        ));
    }
    let steps = &args.steps;
    let ident = &item.ident;
    let dialect = &args.dialect;
    Step::check_ctes(steps)?;
    let mut flat = Vec::new();
    Step::flatten(steps, &mut flat);
    let mut items = Vec::new();
    for (step, _) in &flat {
        add_item(step, &item.generics, &[], &mut items);
    }
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let template = Template {
        segments: Vec::new(),
        params: Vec::new(),
        children: Vec::new(),
    };
    let analysis = Analysis {
        kind: Kind::Query,
        returns_rows: false,
        columns: Columns::default(),
        refs: Vec::new(),
        names: Vec::new(),
    };
    let source = LitStr::new(&ident.to_string(), Span::call_site());
    let mut params = Params::new(&template, &analysis, &items, &item, &source)?;
    params.runs = false;
    params.steps = Some(steps);
    let parts = flat
        .iter()
        .map(|(step, fetch)| {
            let node = quote!(<#step as #krate::sql::Sql>::NODE);
            match fetch {
                Fetch::Cte => quote! {
                    #krate::part::from::From::part(
                        #node,
                        #krate::part::from::rule::AliasRule::NodeName,
                        ::core::option::Option::Some(#krate::node::inject::Inject::cte(false)),
                    )
                },
                _ => quote!(#krate::part::expr::Expr::part(#node)),
            }
        })
        .collect();
    let node = node(
        &snake_case(&ident.to_string()),
        fingerprint(ident, &item),
        quote!(Transaction),
        quote!(#krate::node::inject::Inject::Subquery),
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
    let hook_needs = hook_needs(&item, &needs, quote!(#krate::builder::HookNeeds));
    let embed_checks = embeds(&item, &fields, dialect);
    let params_ty = params.ty();
    let params_struct = params.definition();
    let derives = params.derives();
    let bind_params = params.bind_impl();
    let builder = params.builder(dialect, None);
    let mut documented = item.clone();
    documented
        .attrs
        .extend(params.item_docs(Source::Transaction(steps)));
    Ok(quote! {
        #documented

        #params_struct
        #derives

        impl #krate::sql::Sql for #ident {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::node::Node = #node;
        }

        #embed_checks
        #hook_needs
        #bind_params
        #builder

        #krate::impl_transaction!(#ident);
    })
}
