use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemStruct, LitStr, Type, parse_quote};

use crate::emit::checks::embeds;
use crate::emit::docs;
use crate::emit::krate;
use crate::emit::node::{fingerprint, node};
use crate::model::items::{add_item, type_args};
use crate::model::params::Params;
use crate::naming::{camel, field_name, snake_case, type_key};
use crate::sql::analyze::{Analysis, Columns, Kind};
use crate::sql::template::Template;

pub(crate) fn separate(
    types: Option<&[Type]>,
    item: &ItemStruct,
    template: &Template,
) -> syn::Result<Vec<(Type, ItemStruct)>> {
    let Some(types) = types else {
        return Ok(Vec::new());
    };
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "`separate` isn't supported on generic queries",
        ));
    }
    let mut wrappers: Vec<(Type, ItemStruct)> = Vec::new();
    for ty in types {
        let key = type_key(ty);
        if !template.children.iter().any(|child| type_key(child) == key) {
            return Err(syn::Error::new_spanned(
                ty,
                format!(
                    "`{}` isn't referenced in the template",
                    docs::type_string(ty)
                ),
            ));
        }
        if type_args(ty).is_empty() {
            return Err(syn::Error::new_spanned(
                ty,
                format!(
                    "`{}` has no type arguments, so there's nothing to separate",
                    docs::type_string(ty)
                ),
            ));
        }
        if wrappers.iter().any(|(other, _)| type_key(other) == key) {
            return Err(syn::Error::new_spanned(
                ty,
                format!("`{}` is listed twice", docs::type_string(ty)),
            ));
        }
        let ident = format_ident!(
            "__{}{}",
            item.ident,
            camel(&format_ident!("{}", field_name(ty)))
        );
        let vis = &item.vis;
        wrappers.push((ty.clone(), parse_quote!(#[doc(hidden)] #vis struct #ident;)));
    }
    Ok(wrappers)
}

pub(crate) fn wrapper(
    named: &Type,
    item: &ItemStruct,
    dialect: &Type,
    sql: &LitStr,
) -> syn::Result<TokenStream> {
    let krate = krate();
    let mut items = Vec::new();
    add_item(named, &item.generics, &[], &mut items);
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
    let mut params = Params::new(&template, &analysis, &items, item, sql)?;
    params.synthetic = true;
    params.runs = false;
    let ident = &item.ident;
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let node = node(
        &snake_case(&ident.to_string()),
        fingerprint(ident, item),
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
    let builder = params.builder(dialect, None);
    let embed_checks = embeds(item, &fields, dialect);
    Ok(quote! {
        #item

        #params_struct
        #derives

        impl #krate::sql::Sql for #ident {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::node::Node = #node;
        }

        #embed_checks

        #bind_params
        #builder
    })
}
