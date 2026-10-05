use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, ItemStruct, LitStr, Type};

use crate::args::{self, Keys};
use crate::emit::checks::{embeds, hook_needs, step_impl};
use crate::emit::docs;
use crate::emit::fmt::fmt;
use crate::emit::node::{fingerprint, node};
use crate::emit::rows::named_fields;
use crate::emit::{self, krate};
use crate::model::item::{Item, Role};
use crate::naming::type_key;

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
                "before" => before = args::types(input)?,
                "after" => after = args::types(input)?,
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
    if !named_fields(&input).is_empty() {
        return Err(syn::Error::new_spanned(
            &input.fields,
            "tables can't have fields; give them to a query that selects from the table to read its rows",
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
    let node = node(
        node_name,
        fingerprint(ident, &input),
        quote!(Table),
        quote!(#krate::Inject::Ident),
        parts,
        &[],
        [&before, &after],
    );
    let check = (!before.is_empty() || !after.is_empty()).then(|| {
        quote! {
            const _: () = #krate::check_hooks(<#ident as #krate::Sql>::NODE);
        }
    });
    let item = Item::new(
        &input,
        Role::Table {
            before: &before,
            after: &after,
        },
        Vec::new(),
        Vec::new(),
    );
    let (definitions, builder) = item.builder(dialect, None);
    let hook_types: Vec<Type> = before.iter().chain(&after).cloned().collect();
    let embed_checks = embeds(&input, &hook_types, dialect);
    let needs: Vec<TokenStream> = hook_types
        .iter()
        .map(|ty| quote!(<#ty as #krate::Sql>::Params))
        .collect();
    let hook_needs = hook_needs(&input, &needs, quote!(#krate::Demand));
    let step = step_impl(&input, None);
    let fmt = fmt(args.display.as_ref(), args.debug.as_ref(), &input, false)?;
    let mut documented = input.clone();
    documented.attrs.extend(item.item_docs());

    let impls = emit::scoped(quote! {
        impl #krate::Sql for #ident {
            type Dialect = #dialect;
            type Params = ();
            const NODE: &'static #krate::Node = #node;
        }

        #check
        #embed_checks
        #hook_needs
        #builder
        #step
        #fmt
    });
    Ok(quote! {
        #documented
        #definitions
        #impls
    })
}

fn hooks(types: &[Type]) -> syn::Result<Vec<Type>> {
    let mut hooks: Vec<Type> = Vec::new();
    for ty in types {
        if hooks.iter().any(|other| type_key(other) == type_key(ty)) {
            return Err(syn::Error::new_spanned(
                ty,
                format!("`{}` is listed twice", docs::type_string(ty)),
            ));
        }
        hooks.push(ty.clone());
    }
    Ok(hooks)
}
