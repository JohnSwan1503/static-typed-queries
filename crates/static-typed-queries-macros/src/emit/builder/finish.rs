use proc_macro2::TokenStream;
use quote::quote;

use super::Builder;
use crate::emit::doc;
use crate::model::fields;

impl Builder<'_> {
    fn complete(&self) -> TokenStream {
        let krate = &self.krate;
        let states: Vec<TokenStream> = self
            .fields
            .iter()
            .enumerate()
            .map(|(slot, field)| {
                if field.is_item() {
                    let ty = &field.ty;
                    quote!(#krate::Built<#ty>)
                } else {
                    self.marker(slot, quote!(#krate::Filled))
                }
            })
            .collect();
        self.ty(&states, &quote!(#krate::Root))
    }

    pub(super) fn finish(&self) -> TokenStream {
        let krate = &self.krate;
        let ident = &self.input.ident;
        let complete = self.complete();
        let (impl_generics, ty_generics, where_clause) = self.input.generics.split_for_impl();
        let values = self.fields.iter().zip(&self.slots).map(|(field, slot)| {
            let (name, own) = (&field.ident, &slot.field);
            if field.is_item() {
                quote!(#name: self.#own.0)
            } else {
                quote!(#name: self.#own.expect("the builder's type guarantees every field is set"))
            }
        });
        let phantoms = fields::phantoms(self.input);
        let doc = doc(&format!("Returns the [`{ident}`] with every field set."));
        quote! {
            impl #impl_generics #krate::Finish for #complete #where_clause {
                type Output = #ident #ty_generics;

                fn finish(self) -> #ident #ty_generics {
                    #ident {
                        #(#values,)*
                        #(#phantoms: ::core::marker::PhantomData,)*
                    }
                }
            }

            impl #impl_generics #complete #where_clause {
                #doc
                pub fn build(self) -> #ident #ty_generics {
                    #krate::Finish::finish(self)
                }
            }
        }
    }
}
