use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, ItemStruct, LitBool, Type, parse_quote};

use crate::args::{self, Keys};
use crate::emit::checks::{embeds, hook_needs, parse_test, step_impl};
use crate::emit::docs;
use crate::emit::fmt::fmt;
use crate::emit::node::{fingerprint, node};
use crate::emit::rows::{from_row, row, row_docs};
use crate::emit::{self, krate};
use crate::model::item::{Item, Role};
use crate::model::items::add_item;
use crate::model::params;
use crate::naming::snake_case;

pub(crate) struct StatementArgs {
    target: Type,
    display: Option<Ident>,
    debug: Option<Ident>,
    row: Option<Type>,
    parse_check: Option<LitBool>,
}

const KEYS: Keys = &[&["display"], &["debug"], &["row"], &["parse_check"]];

impl Parse for StatementArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = StatementArgs {
            target: input.parse()?,
            display: None,
            debug: None,
            row: None,
            parse_check: None,
        };
        let target = docs::type_string(&args.target);
        args::parse_keys(input, KEYS, |key, input| {
            match key.to_string().as_str() {
                "display" => args.display = Some(args::value(input)?),
                "debug" => args.debug = Some(args::value(input)?),
                "row" => args.row = Some(args::value(input)?),
                "parse_check" => args.parse_check = Some(args::value(input)?),
                "sql" | "sql_file" | "name" | "cte" | "subquery" | "separate" | "grammar" => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "statements take their SQL from `{target}`, so they don't take `{key}`"
                        ),
                    ));
                }
                "before" | "after" => return Err(args::only_tables(key)),
                "steps" => return Err(args::only_transactions(key)),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(args)
    }
}

pub(crate) fn expand(args: StatementArgs, input: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    let target = &args.target;
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "statements can't be generic; name the instantiation instead",
        ));
    }
    let ident = &input.ident;
    let dialect: Type = parse_quote!(<#target as #krate::Sql>::Dialect);
    let mut items = Vec::new();
    add_item(target, &input.generics, &[], &mut items);
    let item = Item::new(
        &input,
        Role::Statement { target },
        Vec::new(),
        params::children(&items, &[], &[]),
    );
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let node = node(
        &snake_case(&ident.to_string()),
        fingerprint(ident, &input),
        quote!(Scope),
        quote!(#krate::Inject::Subquery),
        Vec::new(),
        &fields,
        [&[], &[]],
    );
    let params_ty = item.ty();
    let params_struct = item.definition();
    let derives = item.derives();
    let bind_params = item.bind_impl();
    let row = row(args.row.as_ref(), &input, true, None)?;
    let (definitions, builder) = item.builder(&dialect, row.as_ref());
    let embed_checks = embeds(&input, &fields, &dialect);
    let hook_needs = hook_needs(&input, &[quote!(#target)], quote!(#krate::HookNeeds));
    let fmt = fmt(args.display.as_ref(), args.debug.as_ref(), &input, true)?;
    let rows = row.as_ref().map(|row| {
        quote! {
            impl #krate::Rows for #ident {
                type Row = #row;
            }
        }
    });
    let from_row = from_row(&input, &dialect);
    let step = step_impl(&input, row.as_ref());
    let parse_check = args.parse_check.as_ref().is_none_or(|check| check.value);
    let test = parse_check.then(|| parse_test(ident));
    let mut documented = input.clone();
    documented.attrs.extend(item.item_docs());
    documented.attrs.extend(row_docs(&input));
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

        #krate::impl_statement!(#ident);
        #rows
        #from_row
        #step

        impl #ident {
            /// The SQL, rendered at compile time.
            pub const SQL: &'static str = <Self as #krate::Statement>::SQL;
        }

        #fmt
    });
    Ok(quote! {
        #documented
        #params_struct
        #definitions
        #test
        #impls
    })
}
