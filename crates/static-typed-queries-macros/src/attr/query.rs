use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
    GenericParam, Ident, ItemStruct, LitBool, LitStr, Path, Type, parenthesized, parse_quote,
};

use crate::args::{self, Keys, Placement, Sql};
use crate::attr::wrapper::{self, separate};
use crate::emit::checks::{embeds, hook_needs, step_impl};
use crate::emit::docs::Source;
use crate::emit::fmt::fmt;
use crate::emit::krate;
use crate::emit::node::{fingerprint, node, parts, placement};
use crate::emit::rows::{from_row, row, row_docs};
use crate::model::items::items;
use crate::model::params::Params;
use crate::naming::{snake_case, type_key};
use crate::sql::analyze::{self, Engine, Kind};
use crate::sql::template;

pub(crate) struct QueryArgs {
    pub(crate) dialect: Type,
    pub(crate) sql: Option<Sql>,
    pub(crate) sql_file: Option<LitStr>,
    pub(crate) name: Option<LitStr>,
    pub(crate) placement: Option<Placement>,
    pub(crate) display: Option<Ident>,
    pub(crate) debug: Option<Ident>,
    pub(crate) parse_check: Option<LitBool>,
    pub(crate) separate: Option<Vec<Type>>,
    pub(crate) grammar: Option<Ident>,
    pub(crate) row: Option<Type>,
}

const KEYS: Keys = &[
    &["cte", "subquery"],
    &["sql"],
    &["sql_file"],
    &["name"],
    &["display"],
    &["debug"],
    &["parse_check"],
    &["separate"],
    &["grammar"],
    &["row"],
];

impl Parse for QueryArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = QueryArgs {
            dialect: input.parse()?,
            sql: None,
            sql_file: None,
            name: None,
            placement: None,
            display: None,
            debug: None,
            parse_check: None,
            separate: None,
            grammar: None,
            row: None,
        };
        args::parse_keys(input, KEYS, |key, input| {
            match key.to_string().as_str() {
                "cte" | "subquery" => args.placement = Some(Placement::parse_keyword(input)?),
                "sql" => args.sql = Some(args::value(input)?),
                "sql_file" => args.sql_file = Some(args::value(input)?),
                "name" => args.name = Some(args::value(input)?),
                "display" => args.display = Some(args::value(input)?),
                "debug" => args.debug = Some(args::value(input)?),
                "parse_check" => args.parse_check = Some(args::value(input)?),
                "separate" => args.separate = Some(args::types(input)?),
                "grammar" => args.grammar = Some(args::value(input)?),
                "row" => args.row = Some(args::value(input)?),
                "before" | "after" => return Err(args::only_tables(key)),
                "steps" => return Err(args::only_transactions(key)),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(args)
    }
}

pub(crate) struct NamedQuery {
    sql: LitStr,
    args: QueryArgs,
    item: ItemStruct,
}

impl Parse for NamedQuery {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let sql = input.parse()?;
        let content;
        parenthesized!(content in input);
        Ok(NamedQuery {
            sql,
            args: content.parse()?,
            item: input.parse()?,
        })
    }
}

pub(crate) fn expand(args: QueryArgs, item: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if let Some(param) = item
        .generics
        .params
        .iter()
        .find(|param| !matches!(param, GenericParam::Type(_)))
    {
        return Err(syn::Error::new_spanned(
            param,
            "only type parameters are supported on queries",
        ));
    }
    let file = match (&args.sql, &args.sql_file) {
        (Some(_), Some(file)) => {
            return Err(syn::Error::new(
                file.span(),
                "give the template once, with `sql` or with `sql_file`",
            ));
        }
        (None, Some(file)) => Some(template_file(file)?),
        _ => None,
    };
    let sql = match (&args.sql, &file) {
        (Some(Sql::Inline(sql)), _) => sql,
        (Some(Sql::Named(_)), _) => unreachable!("named templates expand through their macro"),
        (None, Some((sql, _))) => sql,
        (None, None) => {
            return Err(syn::Error::new(
                Span::call_site(),
                "queries need `sql = \"...\"`, `sql = NAME` or `sql_file = \"...\"`",
            ));
        }
    };
    let track = file.as_ref().map(|(_, track)| track);
    let template = template::parse(sql)?;
    let engine = Engine::of(&args.dialect, args.grammar.as_ref(), &item.generics)?;
    let analysis = analyze::analyze(&template, engine, sql)?;
    if let Some(row) = &args.row {
        if !item.generics.params.is_empty() {
            return Err(syn::Error::new_spanned(
                row,
                "generic queries can't take `row`; they aren't statements on their own",
            ));
        }
        if !analysis.returns_rows {
            return Err(syn::Error::new_spanned(
                row,
                "this statement returns no rows; only queries and statements with `RETURNING` take `row`",
            ));
        }
    }
    let row = row(
        args.row.as_ref(),
        &item,
        analysis.returns_rows,
        Some(&analysis.columns),
    )?;
    let kind = match analysis.kind {
        Kind::Query => quote!(Query),
        Kind::Dml => quote!(Dml),
        Kind::Ddl => quote!(Ddl),
    };

    let wrappers = separate(args.separate.as_deref(), &item, &template)?;
    let separate: Vec<(Type, Type)> = wrappers
        .iter()
        .map(|(named, wrapper)| {
            let wrapper = &wrapper.ident;
            (named.clone(), parse_quote!(#wrapper))
        })
        .collect();
    let items = items(&template, &item.generics, &separate);
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let embed_checks = embeds(&item, &fields, &args.dialect);
    let mut referenced: Vec<&Type> = Vec::new();
    for ty in &template.children {
        if !referenced
            .iter()
            .any(|other| type_key(other) == type_key(ty))
        {
            referenced.push(ty);
        }
    }
    let needs: Vec<TokenStream> = referenced.iter().map(|ty| quote!(#ty)).collect();
    let hook_needs = hook_needs(&item, &needs, quote!(#krate::builder::HookNeeds));
    let ident = &item.ident;
    let node_name = args
        .name
        .as_ref()
        .map_or_else(|| snake_case(&ident.to_string()), LitStr::value);
    let inject = placement(args.placement.unwrap_or(Placement::Subquery));
    let params = Params::new(&template, &analysis, &items, &item, sql)?;
    let node = node(
        &node_name,
        fingerprint(ident, &item),
        kind,
        inject,
        parts(&template, &analysis, &params),
        &fields,
        [&[], &[]],
    );
    let wrappers = wrappers
        .iter()
        .map(|(named, wrapper)| wrapper::wrapper(named, wrapper, &args.dialect, sql))
        .collect::<syn::Result<Vec<_>>>()?;

    let dialect = &args.dialect;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    let params_ty = params.ty();
    let params_struct = params.definition();
    let derives = params.derives();
    let bind_params = params.bind_impl();
    let builder = params.builder(dialect, row.as_ref());
    let parse_check = args.parse_check.as_ref().is_none_or(|check| check.value);
    let statement = item.generics.params.is_empty().then(|| {
        let test = format_ident!("{}_sql_parses", snake_case(&ident.to_string()));
        let test = parse_check.then(|| {
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
        let rows = row.as_ref().map(|row| {
            quote! {
                impl #krate::statement::Rows for #ident {
                    type Row = #row;
                }
            }
        });
        let from_row = from_row(&item, dialect);
        quote! {
            #krate::impl_statement!(#ident);
            #rows
            #from_row

            impl #ident {
                /// The SQL, rendered at compile time.
                pub const SQL: &'static str = <Self as #krate::statement::Statement>::SQL;
            }

            #test
        }
    });
    let fmt = fmt(args.display.as_ref(), args.debug.as_ref(), &item, true)?;
    let step = step_impl(&item, row.as_ref());
    let mut documented = item.clone();
    documented
        .attrs
        .extend(params.item_docs(Source::Template(sql)));
    documented.attrs.extend(row_docs(&item));

    Ok(quote! {
        #documented

        #params_struct
        #derives

        impl #impl_generics #krate::sql::Sql for #ident #ty_generics #where_clause {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::node::Node = #node;
        }

        #embed_checks
        #hook_needs
        #bind_params
        #builder
        #statement
        #step
        #fmt
        #(#wrappers)*
        #track
    })
}

fn template_file(file: &LitStr) -> syn::Result<(LitStr, TokenStream)> {
    let root = std::env::var_os("CARGO_MANIFEST_DIR").ok_or_else(|| {
        syn::Error::new(
            file.span(),
            "`sql_file` is read from the crate's directory, but `CARGO_MANIFEST_DIR` isn't set",
        )
    })?;
    let path = std::path::Path::new(&root).join(file.value());
    let sql = std::fs::read_to_string(&path).map_err(|error| {
        syn::Error::new(
            file.span(),
            format!(
                "can't read `{}` from the crate's directory: {error}",
                file.value()
            ),
        )
    })?;
    let full = path
        .to_str()
        .ok_or_else(|| syn::Error::new(file.span(), "the path to `sql_file` isn't valid UTF-8"))?;
    Ok((
        LitStr::new(&sql, file.span()),
        quote!(
            const _: &str = ::core::include_str!(#full);
        ),
    ))
}

pub(crate) fn named(name: &Path, args: TokenStream, item: &ItemStruct) -> TokenStream {
    quote!(#name! { (#args) #item })
}

pub(crate) fn named_query(named: NamedQuery) -> syn::Result<TokenStream> {
    let NamedQuery {
        sql,
        mut args,
        item,
    } = named;
    let used = match args.sql.replace(Sql::Inline(sql)) {
        Some(Sql::Named(name)) => Some(quote!(const _: &str = #name;)),
        _ => None,
    };
    let query = expand(args, item)?;
    Ok(quote!(#used #query))
}
