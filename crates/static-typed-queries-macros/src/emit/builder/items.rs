use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, parse_quote};

use super::Builder;
use crate::emit::doc;
use crate::model::params::Child;
use crate::naming::camel;

impl Builder<'_, '_> {
    // An item's method moves into that item's builder, scoped by a hole that holds this builder
    // with the item's slot empty; the item's next setter fills the hole and returns here.
    pub(super) fn holes(&self) -> TokenStream {
        let vis = &self.item.input.vis;
        let holes = self.item.children.iter().map(|child| self.hole(child));
        quote!(#(#[doc(hidden)] #vis struct #holes<P>(P);)*)
    }

    fn hole(&self, child: &Child) -> Ident {
        format_ident!("__{}{}", self.name, camel(&child.name))
    }

    pub(super) fn item_methods(&self) -> TokenStream {
        let b = &self.b;
        let item = self.item;
        let name = self.name;
        let (hooks, values, parent, root) = (&self.hooks, &self.values, &self.parent, &self.root);
        let mut out = TokenStream::new();
        for (index, child) in item.children.iter().enumerate() {
            let slot = item.groups.len() + index;
            let (method, field) = (&child.name, &child.field);
            let hole = self.hole(child);
            let others = self.others(field);
            let vacant = self.ty(&self.states_with(slot, quote!(())), hooks, values, parent);
            let current = self.ty(
                &self.states_with(slot, quote!(#b::Open<__B>)),
                hooks,
                values,
                parent,
            );
            let generics = self.generics_except(slot, &[quote!(__B), parent.clone()]);
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let doc = doc(&format!(
                "Moves to the builder of {}. Its next setter sets one of its parameters and returns to the outermost builder.",
                item.link(&child.named)
            ));
            out.extend(quote! {
                impl #impl_generics #current #where_clause {
                    #doc
                    pub fn #method<__Scoped>(self) -> __Scoped
                    where
                        __B: #b::Scope<#hole<#vacant>, Scoped = __Scoped>,
                    {
                        #b::Scope::scope(self.#field.0, #hole(#name {
                            #field: (),
                            #(#others: self.#others,)*
                            __values: self.__values,
                            __parent: self.__parent,
                            __state: ::core::marker::PhantomData,
                        }))
                    }
                }
            });

            let filled = self.states_with(slot, quote!(<__Item as #b::Settled>::Slot));
            let filled = self.ty(&filled, hooks, values, root);
            let mut generics = self.generics_except(slot, &[parent.clone(), quote!(__Item)]);
            let clause = generics.make_where_clause();
            clause.predicates.push(parse_quote!(__Item: #b::Settled));
            clause.predicates.push(parse_quote!(__K: #b::Fill<#filled>));
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            out.extend(quote! {
                impl #impl_generics #b::Fill<__Item> for #hole<#vacant> #where_clause {
                    type Output = <__K as #b::Fill<#filled>>::Output;

                    fn fill(self, item: __Item) -> Self::Output {
                        let parent = self.0;
                        #b::Fill::fill(parent.__parent, #name {
                            #field: #b::Settled::settled(item),
                            #(#others: parent.#others,)*
                            __values: parent.__values,
                            __parent: #root,
                            __state: ::core::marker::PhantomData,
                        })
                    }
                }
            });
        }
        out
    }
}
