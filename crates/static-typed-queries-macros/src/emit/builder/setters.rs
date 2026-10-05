use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;

use super::Builder;
use crate::emit::{doc, docs};

impl Builder<'_, '_> {
    pub(super) fn setters(&self) -> TokenStream {
        let b = &self.b;
        let name = self.name;
        let module = &self.module;
        let (hooks, values, parent, root) = (&self.hooks, &self.values, &self.parent, &self.root);
        let mut out = TokenStream::new();
        for (slot, group) in self.item.groups.iter().enumerate() {
            let (setter, field, ty, marker) = (&group.name, &group.field, &group.ty, &group.marker);
            let len = Self::len(group.slots.len());
            let current = self.states_with(slot, quote!(#module::#marker<#b::Missing<__Rest>>));
            let next = self.states_with(slot, quote!(#module::#marker<__Rest>));
            let next = self.ty(&next, hooks, values, root);
            let mut generics =
                self.generics_except(slot, &[quote!(__Rest: #b::Remaining), parent.clone()]);
            generics
                .make_where_clause()
                .predicates
                .push(parse_quote!(__K: #b::Fill<#next>));
            let (impl_generics, _, where_clause) = generics.split_for_impl();
            let current = self.ty(&current, hooks, values, parent);
            let others = self.others(field);
            let origins = docs::origins(&group.origins);
            let doc = doc(&match group.slots.len() {
                1 => format!("Sets `{setter}`, {origins}."),
                len => format!(
                    "Sets the next of {len} `{setter}` values, {origins}. Call it once per value, in template order."
                ),
            });
            out.extend(quote! {
                impl #impl_generics #current #where_clause {
                    #doc
                    pub fn #setter(self, value: #ty) -> <__K as #b::Fill<#next>>::Output {
                        let mut #field = self.#field;
                        #field[#len - 1 - <__Rest as #b::Remaining>::N] =
                            ::core::option::Option::Some(value);
                        #b::Fill::fill(self.__parent, #name {
                            #field,
                            #(#others: self.#others,)*
                            __values: self.__values,
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
