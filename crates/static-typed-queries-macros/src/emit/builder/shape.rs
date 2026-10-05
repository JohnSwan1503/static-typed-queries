use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::parse_quote;

use super::Builder;
use crate::emit::{docs, krate_path};

impl Builder<'_> {
    pub(super) fn markers(&self) -> TokenStream {
        let markers: Vec<_> = self
            .slots
            .iter()
            .filter_map(|slot| slot.marker.as_ref())
            .collect();
        if markers.is_empty() {
            return TokenStream::new();
        }
        let vis = &self.input.vis;
        let module = &self.module;
        quote! {
            #[doc(hidden)]
            #vis mod #module {
                #(pub struct #markers<__S>(::core::marker::PhantomData<__S>);)*
            }
        }
    }

    pub(super) fn marker_impls(&self) -> TokenStream {
        let krate = &self.krate;
        let module = &self.module;
        let markers = self.slots.iter().filter_map(|slot| slot.marker.as_ref());
        quote! {
            #(
                impl<__S: #krate::Ready> #krate::Ready for #module::#markers<__S> {
                    type Out = <__S as #krate::Ready>::Out;
                }
            )*
        }
    }

    pub(super) fn definition(&self) -> TokenStream {
        let path = krate_path();
        let ident = &self.input.ident;
        let name = &self.name;
        let vis = &self.input.vis;
        let mut params = self.states();
        params.push(quote!(__K = #path::Root));
        let generics = self.generics(params);
        let where_clause = &generics.where_clause;
        let fields = self.fields.iter().zip(&self.slots).map(|(field, slot)| {
            let name = &slot.field;
            if field.is_item() {
                let state = &slot.state;
                quote!(#name: #state)
            } else {
                let ty = &field.ty;
                quote!(#name: ::core::option::Option<#ty>)
            }
        });
        let args = self.args();
        let values = self
            .slots
            .iter()
            .filter(|slot| slot.marker.is_some())
            .map(|slot| &slot.state);
        let doc = docs::attrs(&[
            format!(
                "Builds a [`{ident}`] one field at a time, from `{}`.",
                self.constructor()
            ),
            String::new(),
            "Each field has a setter while it's unset, and each field that holds an item has a method that moves to the item's builder for one setter call. `build` appears once every field is set.".to_owned(),
        ]);
        quote! {
            #(#doc)*
            #vis struct #name #generics #where_clause {
                #(#fields,)*
                __parent: __K,
                __state: ::core::marker::PhantomData<fn() -> (#(#args,)* #(#values,)*)>,
            }
        }
    }

    pub(super) fn ready(&self) -> TokenStream {
        let krate = &self.krate;
        let states = self.states();
        let mut generics = self.generics(states.clone());
        let clause = generics.make_where_clause();
        for state in &states {
            clause.predicates.push(parse_quote!(#state: #krate::Ready));
        }
        let outs: Vec<TokenStream> = states
            .iter()
            .map(|state| quote!(<#state as #krate::Ready>::Out))
            .collect();
        let mut all = outs.last().cloned().expect("a builder has a field");
        for out in outs.iter().rev().skip(1) {
            clause
                .predicates
                .push(parse_quote!(#out: #krate::And<#all>));
            all = quote!(<#out as #krate::And<#all>>::Out);
        }
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let rooted = self.ty(&states, &quote!(#krate::Root));
        quote! {
            impl #impl_generics #krate::Ready for #rooted #where_clause {
                type Out = #all;
            }
        }
    }

    pub(super) fn scope(&self) -> TokenStream {
        let krate = &self.krate;
        let states = self.states();
        let mut params = states.clone();
        params.push(quote!(__K));
        let generics = self.generics(params);
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let rooted = self.ty(&states, &quote!(#krate::Root));
        let scoped = self.ty(&states, &quote!(__K));
        let literal = self.literal(
            |slot| {
                let field = &self.slots[slot].field;
                quote!(self.#field)
            },
            quote!(parent),
        );
        quote! {
            impl #impl_generics #krate::Scope<__K> for #rooted #where_clause {
                type Scoped = #scoped;

                fn scope(self, parent: __K) -> #scoped {
                    #literal
                }
            }
        }
    }

    // Each item field starts settled: built when its item needs no values, open otherwise. An
    // item whose type names a type parameter needs that bound on the impl.
    pub(super) fn build(&self) -> TokenStream {
        let krate = &self.krate;
        let ident = &self.input.ident;
        let params: Vec<String> = self.args().iter().map(ToString::to_string).collect();
        let mut generics = self.input.generics.clone();
        let initial: Vec<TokenStream> = self
            .fields
            .iter()
            .enumerate()
            .map(|(slot, field)| {
                if !field.is_item() {
                    return self.marker(slot, quote!(#krate::Missing));
                }
                let ty = &field.ty;
                if mentions(ty.to_token_stream(), &params) {
                    let clause = generics.make_where_clause();
                    clause.predicates.push(parse_quote!(#ty: #krate::Build));
                    clause
                        .predicates
                        .push(parse_quote!(<#ty as #krate::Build>::Builder: #krate::Settled));
                }
                quote!(<<#ty as #krate::Build>::Builder as #krate::Settled>::Slot)
            })
            .collect();
        let builder = self.ty(&initial, &quote!(#krate::Root));
        let literal = self.literal(
            |slot| {
                let field = &self.fields[slot];
                if field.is_item() {
                    let ty = &field.ty;
                    quote!(#krate::Settled::settled(<#ty as #krate::Build>::builder()))
                } else {
                    quote!(::core::option::Option::None)
                }
            },
            quote!(#krate::Root),
        );
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
        quote! {
            impl #impl_generics #krate::Build for #ident #ty_generics #where_clause {
                type Builder = #builder;

                fn builder() -> Self::Builder {
                    #literal
                }
            }
        }
    }
}

fn mentions(tokens: TokenStream, params: &[String]) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Group(group) => mentions(group.stream(), params),
        TokenTree::Ident(ident) => params.contains(&ident.to_string()),
        _ => false,
    })
}
