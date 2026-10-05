use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, parse_quote};

use super::Builder;
use crate::emit::doc;

impl Builder<'_> {
    // An item's method moves into that item's builder, scoped by a hole that holds this builder
    // with the item's slot empty; the item's next setter fills the hole and returns here.
    pub(super) fn holes(&self) -> TokenStream {
        let vis = &self.input.vis;
        let holes = self
            .fields
            .iter()
            .enumerate()
            .filter(|(_, field)| field.is_item())
            .map(|(slot, _)| self.hole(slot));
        quote!(#(#[doc(hidden)] #vis struct #holes<P>(P);)*)
    }

    fn hole(&self, slot: usize) -> Ident {
        format_ident!("__{}{}", self.name, self.slots[slot].camel)
    }

    pub(super) fn item_methods(&self) -> TokenStream {
        let krate = &self.krate;
        let mut out = TokenStream::new();
        for (slot, field) in self.fields.iter().enumerate() {
            if !field.is_item() {
                continue;
            }
            let (method, own) = (&self.slots[slot].method, &self.slots[slot].field);
            let (vis, docs) = (&field.vis, &field.docs);
            let hole = self.hole(slot);
            let vacant = self.ty(&self.states_with(slot, quote!(())), &quote!(__K));
            let current = self.ty(
                &self.states_with(slot, quote!(#krate::Open<__B>)),
                &quote!(__K),
            );
            let generics = self.generics_except(slot, &[quote!(__B), quote!(__K)]);
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let summary = doc(&format!(
                "Moves to the builder of `{}` for one setter call, which returns to the outermost builder.",
                field.ident
            ));
            let separator = (!docs.is_empty()).then(|| quote!(#[doc = ""]));
            let others = |parent: TokenStream| {
                move |other: usize| {
                    let field = &self.slots[other].field;
                    quote!(#parent.#field)
                }
            };
            let vacated = self.literal(
                |other| {
                    if other == slot {
                        quote!(())
                    } else {
                        others(quote!(self))(other)
                    }
                },
                quote!(self.__parent),
            );
            out.extend(quote! {
                impl #impl_generics #current #where_clause {
                    #summary
                    #separator
                    #(#docs)*
                    #vis fn #method<__Scoped>(self) -> __Scoped
                    where
                        __B: #krate::Scope<#hole<#vacant>, Scoped = __Scoped>,
                    {
                        #krate::Scope::scope(self.#own.0, #hole(#vacated))
                    }
                }
            });

            let filled = self.ty(
                &self.states_with(slot, quote!(<__Item as #krate::Settled>::Slot)),
                &quote!(#krate::Root),
            );
            let mut generics = self.generics_except(slot, &[quote!(__K), quote!(__Item)]);
            let clause = generics.make_where_clause();
            clause
                .predicates
                .push(parse_quote!(__Item: #krate::Settled));
            clause
                .predicates
                .push(parse_quote!(__K: #krate::Fill<#filled>));
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let refilled = self.literal(
                |other| {
                    if other == slot {
                        quote!(#krate::Settled::settled(item))
                    } else {
                        others(quote!(parent))(other)
                    }
                },
                quote!(#krate::Root),
            );
            out.extend(quote! {
                impl #impl_generics #krate::Fill<__Item> for #hole<#vacant> #where_clause {
                    type Output = <__K as #krate::Fill<#filled>>::Output;

                    fn fill(self, item: __Item) -> Self::Output {
                        let parent = self.0;
                        #krate::Fill::fill(parent.__parent, #refilled)
                    }
                }
            });
        }
        out
    }
}
