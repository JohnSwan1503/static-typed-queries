use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;

use super::Builder;
use crate::emit::doc;

impl Builder<'_> {
    pub(super) fn setters(&self) -> TokenStream {
        let krate = &self.krate;
        let mut out = TokenStream::new();
        for (slot, field) in self.fields.iter().enumerate() {
            if field.is_item() {
                continue;
            }
            let method = &self.slots[slot].method;
            let (ty, vis, docs) = (&field.ty, &field.vis, &field.docs);
            let current = self.ty(
                &self.states_with(slot, self.marker(slot, quote!(#krate::Missing))),
                &quote!(__K),
            );
            let next = self.ty(
                &self.states_with(slot, self.marker(slot, quote!(#krate::Filled))),
                &quote!(#krate::Root),
            );
            let mut generics = self.generics_except(slot, &[quote!(__K)]);
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(__K: #krate::Fill<#next>));
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let summary = doc(&format!("Sets `{}`.", field.ident));
            let separator = (!docs.is_empty()).then(|| quote!(#[doc = ""]));
            let literal = self.literal(
                |other| {
                    let field = &self.slots[other].field;
                    if other == slot {
                        quote!(::core::option::Option::Some(value))
                    } else {
                        quote!(self.#field)
                    }
                },
                quote!(#krate::Root),
            );
            out.extend(quote! {
                impl #impl_generics #current #where_clause {
                    #summary
                    #separator
                    #(#docs)*
                    #vis fn #method(self, value: #ty) -> <__K as #krate::Fill<#next>>::Output {
                        #krate::Fill::fill(self.__parent, #literal)
                    }
                }
            });
        }
        out
    }
}
