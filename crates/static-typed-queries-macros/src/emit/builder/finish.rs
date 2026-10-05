use proc_macro2::TokenStream;
use quote::quote;

use super::Builder;

impl Builder<'_, '_> {
    pub(super) fn complete(&self, hooks: &TokenStream, values: &TokenStream) -> TokenStream {
        let (b, krate, module) = (&self.b, &self.krate, &self.module);
        let states: Vec<TokenStream> = self
            .item
            .groups
            .iter()
            .map(|group| {
                let marker = &group.marker;
                quote!(#module::#marker<#b::Filled>)
            })
            .chain(self.item.children.iter().map(|child| {
                let ty = &child.ty;
                quote!(#b::Built<<#ty as #krate::sql::Sql>::Params>)
            }))
            .collect();
        self.ty(&states, hooks, values, &self.root)
    }

    pub(super) fn params(&self) -> TokenStream {
        let item = self.item;
        let params_ident = &item.params_ident;
        let groups = item.groups.iter().map(|group| {
            let (name, field) = (&group.name, &group.field);
            let expect = quote!(.expect("the builder's type guarantees every parameter is set"));
            if group.slots.len() == 1 {
                quote!(#name: { let [value] = self.#field; value #expect })
            } else {
                quote!(#name: self.#field.map(|value| value #expect))
            }
        });
        let children = item.children.iter().map(|child| {
            let (name, field) = (&child.name, &child.field);
            quote!(#name: self.#field.0)
        });
        let marker_value = item.marker_value();
        quote! {
            #params_ident {
                #(#groups,)*
                #(#children,)*
                #marker_value
            }
        }
    }

    pub(super) fn finish(&self) -> TokenStream {
        let b = &self.b;
        let params_ty = self.item.ty();
        let complete = self.complete(&self.hooks, &self.values);
        let generics = self.generics(&[self.hooks.clone(), self.values.clone()]);
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let params = self.params();
        quote! {
            impl #impl_generics #b::Finish for #complete #where_clause {
                type Params = #params_ty;

                fn finish(self) -> #params_ty {
                    #params
                }
            }

            impl #impl_generics #complete #where_clause {
                /// Returns the parameters.
                pub fn build(self) -> #params_ty {
                    #b::Finish::finish(self)
                }
            }
        }
    }
}
