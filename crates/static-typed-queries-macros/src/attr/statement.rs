use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Ident, ItemStruct, LitBool, LitStr, Type, parse_quote};

use crate::args::{self, Keys};
use crate::emit::checks::{embeds, hook_needs, step_impl};
use crate::emit::docs::{self, Source};
use crate::emit::fmt::fmt;
use crate::emit::krate;
use crate::emit::node::{fingerprint, node};
use crate::emit::rows::{from_row, row, row_docs};
use crate::model::items::add_item;
use crate::model::params::Params;
use crate::naming::snake_case;
use crate::sql::analyze::{Analysis, Columns, Kind};
use crate::sql::template::Template;

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

pub(crate) fn expand(args: StatementArgs, item: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    let target = &args.target;
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "statements can't be generic; name the instantiation instead",
        ));
    }
    let ident = &item.ident;
    let dialect: Type = parse_quote!(<#target as #krate::sql::Sql>::Dialect);
    let mut items = Vec::new();
    add_item(target, &item.generics, &[], &mut items);
    let source = LitStr::new(&docs::type_string(target), Span::call_site());
    let template = Template {
        segments: Vec::new(),
        params: Vec::new(),
        children: Vec::new(),
    };
    let analysis = Analysis {
        kind: Kind::Query,
        returns_rows: true,
        columns: Columns::default(),
        refs: Vec::new(),
        names: Vec::new(),
    };
    let params = Params::new(&template, &analysis, &items, &item, &source)?;
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let node = node(
        &snake_case(&ident.to_string()),
        fingerprint(ident, &item),
        quote!(Scope),
        quote!(#krate::node::inject::Inject::Subquery),
        Vec::new(),
        &fields,
        [&[], &[]],
    );
    let params_ty = params.ty();
    let params_struct = params.definition();
    let derives = params.derives();
    let bind_params = params.bind_impl();
    let row = row(args.row.as_ref(), &item, true, None)?;
    let builder = params.builder(&dialect, row.as_ref());
    let embed_checks = embeds(&item, &fields, &dialect);
    let hook_needs = hook_needs(
        &item,
        &[quote!(#target)],
        quote!(#krate::builder::HookNeeds),
    );
    let fmt = fmt(args.display.as_ref(), args.debug.as_ref(), &item, true)?;
    let rows = row.as_ref().map(|row| {
        quote! {
            impl #krate::statement::Rows for #ident {
                type Row = #row;
            }
        }
    });
    let from_row = from_row(&item, &dialect);
    let step = step_impl(&item, row.as_ref());
    let parse_check = args.parse_check.as_ref().is_none_or(|check| check.value);
    let test = parse_check.then(|| {
        let test = format_ident!("{}_sql_parses", snake_case(&ident.to_string()));
        quote! {
            #krate::__if_parse_check! {
                #[cfg(test)]
                #[test]
                fn #test() {
                    #krate::check::parse::<#ident>();
                }
            }
        }
    });
    let mut documented = item.clone();
    documented
        .attrs
        .extend(params.item_docs(Source::Statement(target)));
    documented.attrs.extend(row_docs(&item));
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

        #krate::impl_statement!(#ident);
        #rows
        #from_row
        #step

        impl #ident {
            /// The SQL, rendered at compile time.
            pub const SQL: &'static str = <Self as #krate::statement::Statement>::SQL;
        }

        #test
        #fmt
    })
}
