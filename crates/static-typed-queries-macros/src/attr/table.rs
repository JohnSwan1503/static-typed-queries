use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{ItemStruct, Type};

use crate::args::Args;
use crate::attr::no_steps;
use crate::emit::checks::{embeds, hook_needs, step_impl};
use crate::emit::docs::{self, Source};
use crate::emit::fmt::fmt;
use crate::emit::krate;
use crate::emit::node::{fingerprint, node};
use crate::emit::rows::named_fields;
use crate::model::params::Params;
use crate::naming::type_key;
use crate::sql::analyze::{Analysis, Columns, Kind};
use crate::sql::template::Template;

pub(crate) fn table(args: Args, item: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if let Some(sql) = &args.sql {
        return Err(syn::Error::new(sql.span(), "tables take `name`, not `sql`"));
    }
    if let Some(file) = &args.sql_file {
        return Err(syn::Error::new(
            file.span(),
            "tables take `name`, not `sql_file`",
        ));
    }
    if args.placement.is_some() {
        return Err(syn::Error::new(
            Span::call_site(),
            "tables are always referenced by name",
        ));
    }
    if let Some(parse_check) = &args.parse_check {
        return Err(syn::Error::new(
            parse_check.span(),
            "tables have no SQL to check",
        ));
    }
    if let Some(grammar) = &args.grammar {
        return Err(syn::Error::new(
            grammar.span(),
            "tables have no SQL to check",
        ));
    }
    if let Some(row) = &args.row {
        return Err(syn::Error::new_spanned(
            row,
            "tables aren't statements; give `row` to a query that selects from it",
        ));
    }
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "tables can't be generic",
        ));
    }
    if !named_fields(&item).is_empty() {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "tables can't have fields; give them to a query that selects from the table to read its rows",
        ));
    }
    let name = args
        .name
        .as_ref()
        .ok_or_else(|| syn::Error::new(Span::call_site(), "tables need `name = \"...\"`"))?;
    let table = name.value();
    let mut parts = Vec::new();
    for (i, segment) in table.split('.').enumerate() {
        if i > 0 {
            parts.push(quote!(#krate::part::lit::Lit::part(".")));
        }
        parts.push(quote!(#krate::part::ident::Ident::part(#segment)));
    }
    no_steps(&args)?;
    let before = hooks(args.before.as_deref())?;
    let after = hooks(args.after.as_deref())?;
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
    let params = Params::new(&template, &analysis, &[], &item, name)?;
    let node_name = table.rsplit('.').next().unwrap_or(&table);
    let ident = &item.ident;
    let dialect = &args.dialect;
    let node = node(
        node_name,
        fingerprint(ident, &item),
        quote!(Table),
        quote!(#krate::node::inject::Inject::Ident),
        parts,
        &[],
        [&before, &after],
    );
    let check = (!before.is_empty() || !after.is_empty()).then(|| {
        quote! {
            const _: () = #krate::render::check_hooks(<#ident as #krate::sql::Sql>::NODE);
        }
    });
    let builder = params.builder(dialect, None);
    let hook_types: Vec<Type> = before.iter().chain(&after).cloned().collect();
    let embed_checks = embeds(&item, &hook_types, dialect);
    let needs: Vec<TokenStream> = hook_types
        .iter()
        .map(|ty| quote!(<#ty as #krate::sql::Sql>::Params))
        .collect();
    let hook_needs = hook_needs(&item, &needs, quote!(#krate::builder::Demand));
    let step = step_impl(&item, None);
    let fmt = fmt(&args, &item, false)?;
    let mut documented = item.clone();
    documented
        .attrs
        .extend(params.item_docs(Source::Table(&before, &after)));

    Ok(quote! {
        #documented

        impl #krate::sql::Sql for #ident {
            type Dialect = #dialect;
            type Params = ();
            const NODE: &'static #krate::node::Node = #node;
        }

        #check
        #embed_checks
        #hook_needs
        #builder
        #step
        #fmt
    })
}

pub(crate) fn hooks(types: Option<&[Type]>) -> syn::Result<Vec<Type>> {
    let mut hooks: Vec<Type> = Vec::new();
    for ty in types.unwrap_or_default() {
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
