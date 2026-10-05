use proc_macro2::{Literal, TokenStream};
use quote::quote;
use syn::{GenericParam, ItemStruct, parse_quote};

use crate::emit::krate;
use crate::model::fields::{self, Field};

// Binds a value field by its slot, and hands the rest of a path to the item field it starts with.
pub(crate) fn bind_impl(input: &ItemStruct, fields: &[Field]) -> Option<TokenStream> {
    if fields.is_empty() {
        return None;
    }
    let krate = krate();
    let sqlx = quote!(#krate::sqlx);
    let bind = quote!(#krate::BindParams);

    let mut generics = input.generics.clone();
    generics
        .params
        .insert(0, GenericParam::Type(parse_quote!(__DB: #sqlx::Database)));
    let where_clause = generics.make_where_clause();
    for field in fields {
        let ty = &field.ty;
        where_clause.predicates.push(if field.is_item() {
            parse_quote!(#ty: #bind<__DB>)
        } else {
            parse_quote!(for<'__t> #ty: #sqlx::Encode<'__t, __DB> + #sqlx::Type<__DB>)
        });
    }
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    let values = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| !field.is_item())
        .map(|(index, field)| {
            let name = &field.ident;
            let slot = Literal::u16_unsuffixed(fields::slot(fields, index));
            quote!(([], #slot) => args.add(&self.#name),)
        });
    let items = fields
        .iter()
        .enumerate()
        .filter(|(_, field)| field.is_item())
        .map(|(index, field)| {
            let name = &field.ident;
            let step = Literal::u16_unsuffixed(fields::slot(fields, index));
            quote!(([#step, rest @ ..], _) => #bind::<__DB>::bind(&self.#name, rest, slot, args),)
        });
    let ident = &input.ident;
    let (_, ty_generics, _) = input.generics.split_for_impl();

    Some(quote! {
        #krate::__if_sqlx! {
            impl #impl_generics #bind<__DB> for #ident #ty_generics #where_clause {
                fn bind(
                    &self,
                    path: &[u16],
                    slot: u16,
                    args: &mut <__DB as #sqlx::Database>::Arguments,
                ) -> ::core::result::Result<(), #sqlx::error::BoxDynError> {
                    use #sqlx::Arguments as _;
                    match (path, slot) {
                        #(#values)*
                        #(#items)*
                        _ => ::core::result::Result::Err(
                            #krate::unknown(path, slot),
                        ),
                    }
                }
            }
        }
    })
}
