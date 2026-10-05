use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{ItemStruct, LitStr, Type, parse_quote};

use crate::args::Args;
use crate::attr::{no_hooks, no_steps};
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

pub(crate) fn statement(args: Args, item: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    let target = &args.dialect;
    for (present, name) in [
        (args.sql.is_some(), "sql"),
        (args.sql_file.is_some(), "sql_file"),
        (args.name.is_some(), "name"),
        (args.placement.is_some(), "cte` or `subquery"),
        (args.separate.is_some(), "separate"),
        (args.grammar.is_some(), "grammar"),
    ] {
        if present {
            return Err(syn::Error::new(
                Span::call_site(),
                format!(
                    "statements take their SQL from `{}`, so they don't take `{name}`",
                    docs::type_string(target)
                ),
            ));
        }
    }
    no_hooks(&args)?;
    no_steps(&args)?;
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
    let row = row(&args, &item, true, None)?;
    let builder = params.builder(&dialect, row.as_ref());
    let embed_checks = embeds(&item, &fields, &dialect);
    let hook_needs = hook_needs(
        &item,
        &[quote!(#target)],
        quote!(#krate::builder::HookNeeds),
    );
    let fmt = fmt(&args, &item, true)?;
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
