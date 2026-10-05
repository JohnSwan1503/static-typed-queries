use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemStruct, Type, parse_quote};

use crate::emit::checks::embeds;
use crate::emit::docs;
use crate::emit::krate;
use crate::emit::node::{fingerprint, node};
use crate::model::item::{Item, Role};
use crate::model::items::{add_item, type_args};
use crate::model::params;
use crate::naming::{camel, field_name, snake_case, type_key};
use crate::sql::template::Template;

pub(crate) fn separate(
    types: Option<&[Type]>,
    input: &ItemStruct,
    template: &Template,
) -> syn::Result<Vec<(Type, ItemStruct)>> {
    let Some(types) = types else {
        return Ok(Vec::new());
    };
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
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
            input.ident,
            camel(&format_ident!("{}", field_name(ty)))
        );
        let vis = &input.vis;
        wrappers.push((ty.clone(), parse_quote!(#[doc(hidden)] #vis struct #ident;)));
    }
    Ok(wrappers)
}

pub(crate) fn expand(named: &Type, input: &ItemStruct, dialect: &Type) -> TokenStream {
    let krate = krate();
    let mut items = Vec::new();
    add_item(named, &input.generics, &[], &mut items);
    let item = Item::new(
        input,
        Role::Wrapper,
        Vec::new(),
        params::children(&items, &[], &[]),
    );
    let ident = &input.ident;
    let fields: Vec<Type> = items.iter().map(|(ty, _)| ty.clone()).collect();
    let node = node(
        &snake_case(&ident.to_string()),
        fingerprint(ident, input),
        quote!(Scope),
        quote!(#krate::node::inject::Inject::Subquery),
        Vec::new(),
        &fields,
        [&[], &[]],
    );
    let params_ty = item.ty();
    let params_struct = item.definition();
    let derives = item.derives();
    let bind_params = item.bind_impl();
    let builder = item.builder(dialect, None);
    let embed_checks = embeds(input, &fields, dialect);
    quote! {
        #input

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
    }
}
