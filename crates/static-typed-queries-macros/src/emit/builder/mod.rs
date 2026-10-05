mod finish;
mod items;
mod run;
mod setters;
mod shape;

use proc_macro2::{Literal, TokenStream};
use quote::quote;
use syn::{Generics, Ident, Type};

use crate::emit::krate;
use crate::model::item::Item;

impl<'a> Item<'a> {
    pub(crate) fn builder(&self, dialect: &Type, row: Option<&Type>) -> TokenStream {
        if self.is_empty() {
            return self.no_builder();
        }
        let builder = Builder::new(self);
        let mut out = TokenStream::new();
        out.extend(builder.markers());
        out.extend(builder.definition());
        out.extend(builder.ready());
        out.extend(builder.scope());
        out.extend(builder.setters());
        out.extend(builder.item_methods());
        out.extend(builder.finish());
        if self.runs_as_statement() {
            out.extend(builder.query(dialect, row));
            out.extend(builder.run(dialect, row));
        }
        if self.runs_as_statement() || self.steps().is_some() {
            out.extend(builder.with());
        }
        if let Some(steps) = self.steps() {
            out.extend(builder.transaction_run(steps, dialect));
        }
        out.extend(builder.build());
        out
    }

    fn no_builder(&self) -> TokenStream {
        let krate = krate();
        let ident = &self.input.ident;
        let (impl_generics, ty_generics, where_clause) = self.input.generics.split_for_impl();
        quote! {
            impl #impl_generics #krate::builder::Build for #ident #ty_generics #where_clause {
                type Builder = #krate::builder::NoParams;

                fn builder() -> Self::Builder {
                    #krate::builder::NoParams
                }
            }
        }
    }
}

// The generated builder is `NameBuilder<states…, __H, __V, __K>`: one state per parameter group
// and child item, whether the statement has hooks, the hook values given so far, and the parent
// a scoped builder returns to.
struct Builder<'i, 'a> {
    item: &'i Item<'a>,
    krate: TokenStream,
    b: TokenStream,
    name: &'i Ident,
    module: Ident,
    states: Vec<TokenStream>,
    fields: Vec<&'i Ident>,
    hooks: TokenStream,
    values: TokenStream,
    parent: TokenStream,
    root: TokenStream,
}

impl<'i, 'a> Builder<'i, 'a> {
    fn new(item: &'i Item<'a>) -> Self {
        let krate = krate();
        let b = quote!(#krate::builder);
        Builder {
            root: quote!(#b::Root),
            b,
            krate,
            name: &item.builder,
            module: item.module(),
            states: item.states(),
            fields: item.fields(),
            hooks: quote!(__H),
            values: quote!(__V),
            parent: quote!(__K),
            item,
        }
    }

    fn ty(
        &self,
        states: &[TokenStream],
        hooks: &TokenStream,
        values: &TokenStream,
        parent: &TokenStream,
    ) -> TokenStream {
        let name = self.name;
        let args = self.item.item_args();
        quote!(#name<#(#args,)* #(#states,)* #hooks, #values, #parent>)
    }

    fn generics(&self, extra: &[TokenStream]) -> Generics {
        let mut generics = self.item.input.generics.clone();
        for param in extra {
            generics
                .params
                .push(syn::parse2(param.clone()).expect("a generic parameter"));
        }
        generics
    }

    fn generics_except(&self, slot: usize, extra: &[TokenStream]) -> Generics {
        let mut params: Vec<TokenStream> = self
            .states
            .iter()
            .enumerate()
            .filter(|(other, _)| *other != slot)
            .map(|(_, state)| state.clone())
            .collect();
        params.extend(extra.iter().cloned());
        params.push(self.hooks.clone());
        params.push(self.values.clone());
        self.generics(&params)
    }

    fn states_with(&self, slot: usize, state: TokenStream) -> Vec<TokenStream> {
        let mut states = self.states.clone();
        states[slot] = state;
        states
    }

    fn others(&self, field: &Ident) -> Vec<&'i Ident> {
        self.fields
            .iter()
            .copied()
            .filter(|other| *other != field)
            .collect()
    }

    fn len(slots: usize) -> Literal {
        Literal::usize_unsuffixed(slots)
    }
}
