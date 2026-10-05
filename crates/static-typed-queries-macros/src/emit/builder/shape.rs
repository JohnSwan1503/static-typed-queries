use proc_macro2::TokenStream;
use quote::quote;
use syn::parse_quote;

use super::Builder;
use crate::emit::{docs, krate_path};
use crate::model::item::Role;

impl Builder<'_, '_> {
    pub(super) fn markers(&self) -> TokenStream {
        let vis = &self.item.input.vis;
        let module = &self.module;
        let markers = self.item.groups.iter().map(|group| &group.marker);
        quote! {
            #[doc(hidden)]
            #vis mod #module {
                #(pub struct #markers<__S>(::core::marker::PhantomData<__S>);)*
            }
        }
    }

    pub(super) fn marker_impls(&self) -> TokenStream {
        let b = &self.b;
        let module = &self.module;
        let markers = self.item.groups.iter().map(|group| &group.marker);
        quote! {
            #(
                impl<__S: #b::Ready> #b::Ready for #module::#markers<__S> {
                    type Out = __S::Out;
                }
            )*
        }
    }

    pub(super) fn definition(&self) -> TokenStream {
        let krate = krate_path();
        let item = self.item;
        let name = self.name;
        let vis = &item.input.vis;
        let ident = &item.input.ident;
        let mut params = self.states.clone();
        params.push(quote!(__H = #krate::NoHooks));
        params.push(quote!(__V = ()));
        params.push(quote!(__K = #krate::Root));
        let generics = self.generics(&params);
        let where_clause = &generics.where_clause;
        let group_fields = item.groups.iter().map(|group| {
            let (field, ty) = (&group.field, &group.ty);
            let len = Self::len(group.slots.len());
            quote!(#field: [::core::option::Option<#ty>; #len])
        });
        let child_fields = item.children.iter().map(|child| {
            let (field, state) = (&child.field, &child.state);
            quote!(#field: #state)
        });
        let group_states = item.groups.iter().map(|group| &group.state);
        let args = item.item_args();
        let hidden = matches!(item.role, Role::Wrapper).then(|| quote!(#[doc(hidden)]));
        let doc = docs::attrs(&[
            format!(
                "Builds [`{}`]; start with `{}`.",
                item.params_ident,
                item.constructor()
            ),
            String::new(),
            format!(
                "Each parameter of [`{ident}`] has a setter while it still needs a value, and each item with parameters left has a method that moves to its builder for one setter call. `build` appears once everything is set."
            ),
        ]);
        quote! {
            #(#doc)*
            #hidden
            #vis struct #name #generics #where_clause {
                #(#group_fields,)*
                #(#child_fields,)*
                __values: __V,
                __parent: __K,
                __state: ::core::marker::PhantomData<fn() -> (#(#group_states,)* #(#args,)* __H)>,
            }
        }
    }

    pub(super) fn ready(&self) -> TokenStream {
        let b = &self.b;
        let (hooks, values) = (&self.hooks, &self.values);
        let mut generics = self.generics(&{
            let mut params = self.states.clone();
            params.push(hooks.clone());
            params.push(values.clone());
            params
        });
        let clause = generics.make_where_clause();
        let outs: Vec<TokenStream> = self
            .states
            .iter()
            .map(|state| quote!(<#state as #b::Ready>::Out))
            .collect();
        for state in &self.states {
            clause.predicates.push(parse_quote!(#state: #b::Ready));
        }
        let mut all = outs.last().cloned().expect("a builder has a state");
        for out in outs.iter().rev().skip(1) {
            clause.predicates.push(parse_quote!(#out: #b::And<#all>));
            all = quote!(<#out as #b::And<#all>>::Out);
        }
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let rooted = self.ty(&self.states, hooks, values, &self.root);
        quote! {
            impl #impl_generics #b::Ready for #rooted #where_clause {
                type Out = #all;
            }
        }
    }

    pub(super) fn scope(&self) -> TokenStream {
        let b = &self.b;
        let name = self.name;
        let fields = &self.fields;
        let (hooks, values, parent) = (&self.hooks, &self.values, &self.parent);
        let generics = self.generics(&{
            let mut params = self.states.clone();
            params.push(parent.clone());
            params.push(hooks.clone());
            params.push(values.clone());
            params
        });
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let rooted = self.ty(&self.states, hooks, values, &self.root);
        let scoped = self.ty(&self.states, hooks, values, parent);
        quote! {
            impl #impl_generics #b::Scope<__K> for #rooted #where_clause {
                type Scoped = #scoped;

                fn scope(self, parent: __K) -> #scoped {
                    #name {
                        #(#fields: self.#fields,)*
                        __values: self.__values,
                        __parent: parent,
                        __state: ::core::marker::PhantomData,
                    }
                }
            }
        }
    }

    pub(super) fn build(&self) -> TokenStream {
        let b = &self.b;
        let item = self.item;
        let krate = &self.krate;
        let name = self.name;
        let module = &self.module;
        let root = &self.root;
        let ident = &item.input.ident;
        let (_, ty_generics, _) = item.input.generics.split_for_impl();
        let initial: Vec<TokenStream> = item
            .groups
            .iter()
            .map(|group| {
                let marker = &group.marker;
                let count = (0..group.slots.len())
                    .fold(quote!(#b::Filled), |rest, _| quote!(#b::Missing<#rest>));
                quote!(#module::#marker<#count>)
            })
            .chain(item.children.iter().map(|child| {
                let ty = &child.ty;
                quote!(<<#ty as #b::Build>::Builder as #b::Settled>::Slot)
            }))
            .collect();
        let hooks = if item.runs_as_statement() || item.steps().is_some() {
            quote!(<#ident as #krate::Render>::Hooks)
        } else {
            quote!(#b::NoHooks)
        };
        let initial = self.ty(&initial, &hooks, &quote!(()), root);
        let mut generics = item.input.generics.clone();
        for child in &item.children {
            let ty = &child.ty;
            let clause = generics.make_where_clause();
            clause.predicates.push(parse_quote!(#ty: #b::Build));
            clause
                .predicates
                .push(parse_quote!(<#ty as #b::Build>::Builder: #b::Settled));
        }
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let empty_groups = item.groups.iter().map(|group| {
            let field = &group.field;
            let len = Self::len(group.slots.len());
            quote!(#field: [const { ::core::option::Option::None }; #len])
        });
        let start_children = item.children.iter().map(|child| {
            let (field, ty) = (&child.field, &child.ty);
            quote!(#field: #b::Settled::settled(<#ty as #b::Build>::builder()))
        });
        quote! {
            impl #impl_generics #b::Build for #ident #ty_generics #where_clause {
                type Builder = #initial;

                fn builder() -> Self::Builder {
                    #name {
                        #(#empty_groups,)*
                        #(#start_children,)*
                        __values: (),
                        __parent: #root,
                        __state: ::core::marker::PhantomData,
                    }
                }
            }
        }
    }
}
