use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{
    GenericParam, Ident, ItemStruct, LitBool, LitStr, Path, Type, parenthesized, parse_quote,
};

use crate::args::{self, Keys, Sql};
use crate::emit::bind::bind_impl;
use crate::emit::builder::builder;
use crate::emit::checks::{embeds, hook_needs, step_impl, values};
use crate::emit::docs::item_docs;
use crate::emit::fmt::fmt;
use crate::emit::node::{fingerprint, node, parts};
use crate::emit::statement::statement;
use crate::emit::{self, krate};
use crate::model::fields;
use crate::model::role::Role;
use crate::naming::{push_unique, snake_case};
use crate::sql::analyze::{self, Engine, Kind};
use crate::sql::template;

pub(crate) struct QueryArgs {
    pub(crate) dialect: Type,
    pub(crate) sql: Option<Sql>,
    pub(crate) sql_file: Option<LitStr>,
    pub(crate) name: Option<LitStr>,
    pub(crate) display: Option<Ident>,
    pub(crate) debug: Option<Ident>,
    pub(crate) parse_check: Option<LitBool>,
    pub(crate) grammar: Option<Ident>,
    pub(crate) row: Option<Type>,
}

const KEYS: Keys = &[
    &["sql"],
    &["sql_file"],
    &["name"],
    &["display"],
    &["debug"],
    &["parse_check"],
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
            display: None,
            debug: None,
            parse_check: None,
            grammar: None,
            row: None,
        };
        args::parse_keys(input, KEYS, |key, input| {
            match key.to_string().as_str() {
                "sql" => args.sql = Some(args::value(input)?),
                "sql_file" => args.sql_file = Some(args::value(input)?),
                "name" => args.name = Some(args::value(input)?),
                "display" => args.display = Some(args::value(input)?),
                "debug" => args.debug = Some(args::value(input)?),
                "parse_check" => args.parse_check = Some(args::value(input)?),
                "grammar" => args.grammar = Some(args::value(input)?),
                "row" => args.row = Some(args::value(input)?),
                "cte" | "subquery" => {
                    return Err(syn::Error::new(
                        key.span(),
                        "a query is embedded the way each use asks: mark the field `#[cte]` or `#[subquery]`, or write `{Type as cte}`",
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

pub(crate) struct NamedQuery {
    sql: LitStr,
    args: QueryArgs,
    input: ItemStruct,
}

impl Parse for NamedQuery {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let sql = input.parse()?;
        let content;
        parenthesized!(content in input);
        Ok(NamedQuery {
            sql,
            args: content.parse()?,
            input: input.parse()?,
        })
    }
}

pub(crate) fn expand(args: QueryArgs, mut input: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if let Some(param) = input
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
    let fields = fields::marked(&mut input)?;
    let template = template::parse(sql, &fields)?;
    let engine = Engine::of(&args.dialect, args.grammar.as_ref(), &input.generics)?;
    let analysis = analyze::analyze(&template, engine, sql)?;
    if let Some(row) = &args.row {
        if !input.generics.params.is_empty() {
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
    let row = args.row.as_ref();
    let kind = match analysis.kind {
        Kind::Query => quote!(Query),
        Kind::Dml => quote!(Dml),
        Kind::Ddl => quote!(Ddl),
    };

    let items: Vec<&Type> = fields
        .iter()
        .filter(|field| field.is_item())
        .map(|field| &field.ty)
        .collect();
    let embeds = embeds(&input, &items, &template.types, &args.dialect);
    let mut referenced = Vec::new();
    for ty in items.iter().copied().chain(&template.types) {
        push_unique(&mut referenced, ty);
    }
    let hook_needs = hook_needs(&input, referenced.len(), |i, index| {
        let ty = &referenced[i];
        parse_quote!(#ty: #krate::HookNeeds<__V, #index>)
    });
    let ident = &input.ident;
    let node_name = args
        .name
        .as_ref()
        .map_or_else(|| snake_case(&ident.to_string()), LitStr::value);
    let node = node(
        &node_name,
        fingerprint(ident, &input),
        kind,
        quote!(#krate::Inject::Subquery),
        parts(&template, &analysis, &fields),
        &items,
        [&[], &[]],
        embeds.node,
    );
    let checked = embeds.checked;

    let dialect = &args.dialect;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let values = values(&input, !fields.is_empty());
    let bind = bind_impl(&input, &fields);
    let (statement, methods, test) = if input.generics.params.is_empty() {
        statement(&input, dialect, row, args.parse_check.as_ref())
    } else {
        Default::default()
    };
    let (definitions, builder) = builder(&input, &fields, methods);
    let fmt = fmt(args.display.as_ref(), args.debug.as_ref(), &input, true)?;
    let step = step_impl(&input, row);
    let mut documented = input.clone();
    documented
        .attrs
        .extend(item_docs(&input, Role::Query { sql }));

    let impls = emit::scoped(quote! {
        impl #impl_generics #krate::Sql for #ident #ty_generics #where_clause {
            type Dialect = #dialect;
            const NODE: &'static #krate::Node = #node;
        }

        #values
        #checked
        #hook_needs
        #bind
        #builder
        #statement
        #step
        #fmt
        #track
    });
    Ok(quote! {
        #documented
        #definitions
        #test
        #impls
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

pub(crate) fn named(name: &Path, args: TokenStream, input: &ItemStruct) -> TokenStream {
    quote!(#name! { (#args) #input })
}

pub(crate) fn named_query(named: NamedQuery) -> syn::Result<TokenStream> {
    let NamedQuery {
        sql,
        mut args,
        input,
    } = named;
    let used = match args.sql.replace(Sql::Inline(sql)) {
        Some(Sql::Named(name)) => Some(quote!(const _: &str = #name;)),
        _ => None,
    };
    let query = expand(args, input)?;
    Ok(quote!(#used #query))
}
