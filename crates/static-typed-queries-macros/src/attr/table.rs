use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Fields, Ident, ItemStruct, LitStr, Type, parse_quote};

use crate::args::{self, Keys};
use crate::emit::builder::builder;
use crate::emit::checks::{CteRule, cte_in, embeds, hook_needs, step_impl, values};
use crate::emit::docs::{self, item_docs};
use crate::emit::fmt::fmt;
use crate::emit::node::node;
use crate::emit::{self, krate};
use crate::model::role::Role;
use crate::naming::push_unique;

pub(crate) struct TableArgs {
    dialect: Type,
    name: LitStr,
    before: Vec<Type>,
    after: Vec<Type>,
    display: Option<Ident>,
    debug: Option<Ident>,
}

const KEYS: Keys = &[&["name"], &["before"], &["after"], &["display"], &["debug"]];

impl Parse for TableArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let dialect = input.parse()?;
        let (mut name, mut display, mut debug) = (None, None, None);
        let (mut before, mut after) = (Vec::new(), Vec::new());
        args::parse_keys(input, KEYS, |key, input| {
            let error = |message: &str| syn::Error::new(key.span(), message);
            match key.to_string().as_str() {
                "name" => name = Some(args::value(input)?),
                "before" => before = args::list(input)?,
                "after" => after = args::list(input)?,
                "display" => display = Some(args::value(input)?),
                "debug" => debug = Some(args::value(input)?),
                "sql" | "sql_file" => {
                    return Err(error(&format!("tables take `name`, not `{key}`")));
                }
                "cte" | "subquery" => return Err(error("tables are always referenced by name")),
                "parse_check" | "grammar" => return Err(error("tables have no SQL to check")),
                "row" => {
                    return Err(error(
                        "tables aren't statements; give `row` to a query that selects from it",
                    ));
                }
                "steps" => return Err(args::only_transactions(key)),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        let name =
            name.ok_or_else(|| syn::Error::new(Span::call_site(), "tables need `name = \"...\"`"))?;
        Ok(TableArgs {
            dialect,
            name,
            before,
            after,
            display,
            debug,
        })
    }
}

pub(crate) fn expand(args: TableArgs, input: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "tables can't be generic",
        ));
    }
    if !matches!(input.fields, Fields::Unit) {
        return Err(syn::Error::new_spanned(
            &input.fields,
            "tables hold no values, so they can't have fields",
        ));
    }
    let name = &args.name;
    let table = name.value();
    let mut parts = Vec::new();
    for (i, segment) in table.split('.').enumerate() {
        if i > 0 {
            parts.push(quote!(#krate::Lit::part(".")));
        }
        parts.push(quote!(#krate::Ident::part(#segment)));
    }
    let before = hooks(&args.before)?;
    let after = hooks(&args.after)?;
    let node_name = table.rsplit('.').next().unwrap_or(&table);
    let ident = &input.ident;
    let dialect = &args.dialect;
    let hook_types: Vec<&Type> = before.iter().chain(&after).collect();
    let embeds = embeds(&input, &hook_types, &[], &[], dialect);
    let cte_in = cte_in(&input, CteRule::Any);
    let node = node(
        &input,
        node_name,
        "Table",
        parts,
        &[],
        [&before, &after],
        embeds.node,
    );
    let checked = embeds.checked;
    let check = (!before.is_empty() || !after.is_empty()).then(|| {
        quote! {
            const _: () = #krate::check_hooks(<#ident as #krate::Sql>::NODE);
        }
    });
    let hook_needs = hook_needs(&input, hook_types.len(), |i, index| {
        let hook = hook_types[i];
        parse_quote!(__V: #krate::Provides<#hook, #index>)
    });
    let values = values(&input, false);
    let (_, built) = builder(&input, &[], TokenStream::new());
    let step = step_impl(&input, None);
    let fmt = fmt(args.display.as_ref(), args.debug.as_ref(), &input, false)?;
    let mut documented = input.clone();
    documented.attrs.extend(item_docs(
        &input,
        Role::Table {
            before: &before,
            after: &after,
        },
    ));

    let impls = emit::scoped(quote! {
        impl #krate::Sql for #ident {
            type Dialect = #dialect;
            const NODE: &'static #krate::Node = #node;
        }

        #check
        #values
        #built
        #checked
        #cte_in
        #hook_needs
        #step
        #fmt
    });
    Ok(quote! {
        #documented
        #impls
    })
}

fn hooks(types: &[Type]) -> syn::Result<Vec<Type>> {
    let mut hooks = Vec::new();
    for ty in types {
        if !push_unique(&mut hooks, ty) {
            return Err(syn::Error::new_spanned(
                ty,
                format!("`{}` is listed twice", docs::type_string(ty)),
            ));
        }
    }
    Ok(hooks)
}
