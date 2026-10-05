use proc_macro2::{Literal, TokenStream};
use quote::quote;
use syn::{Ident, parse_quote};

use crate::emit::docs;
use crate::emit::{doc, krate};
use crate::model::item::{Item, Role};

impl<'a> Item<'a> {
    pub(crate) fn marker(&self) -> Option<TokenStream> {
        let args = self.item_args();
        (!args.is_empty()).then(|| quote!(::core::marker::PhantomData<fn() -> (#(#args,)*)>))
    }

    pub(crate) fn marker_value(&self) -> Option<TokenStream> {
        self.marker()
            .map(|_| quote!(__marker: ::core::marker::PhantomData,))
    }

    pub(crate) fn ty(&self) -> TokenStream {
        if self.is_empty() {
            return quote!(());
        }
        let ident = &self.params_ident;
        let args = self.item_args();
        if args.is_empty() {
            quote!(#ident)
        } else {
            quote!(#ident<#(#args),*>)
        }
    }

    pub(crate) fn definition(&self) -> Option<TokenStream> {
        if self.is_empty() {
            return None;
        }
        let krate = krate();
        let vis = &self.input.vis;
        let ident = &self.params_ident;
        let generics = &self.input.generics;
        let where_clause = &generics.where_clause;
        let groups = self.groups.iter().map(|group| {
            let (name, ty) = (&group.name, &group.ty);
            let origins = docs::origins(&group.origins);
            match group.slots.len() {
                1 => {
                    let doc = doc(&format!("{}.", docs::capitalize(&origins)));
                    quote!(#doc pub #name: #ty)
                }
                len => {
                    let doc = doc(&format!("{len} values in template order, {origins}."));
                    let len = Literal::usize_unsuffixed(len);
                    quote!(#doc pub #name: [#ty; #len])
                }
            }
        });
        let children = self.children.iter().map(|child| {
            let (name, ty) = (&child.name, &child.ty);
            let doc = doc(&format!("The parameters of {}.", self.link(&child.named)));
            quote!(#doc pub #name: <#ty as #krate::sql::Sql>::Params)
        });
        let doc = doc(&format!(
            "The parameters of [`{}`], built by [`{}`].",
            self.input.ident, self.builder
        ));
        let hidden = matches!(self.role, Role::Wrapper).then(|| quote!(#[doc(hidden)]));
        let marker = self.marker().map(|marker| {
            quote! {
                #[doc(hidden)]
                pub __marker: #marker,
            }
        });
        Some(quote! {
            #doc
            #hidden
            #vis struct #ident #generics #where_clause {
                #(#groups,)*
                #(#children,)*
                #marker
            }
        })
    }

    pub(crate) fn derives(&self) -> Option<TokenStream> {
        if self.is_empty() {
            return None;
        }
        let ident = &self.params_ident;
        let name = ident.to_string();
        let krate = krate();
        let fields: Vec<(&Ident, TokenStream)> = self
            .groups
            .iter()
            .map(|group| {
                let ty = &group.ty;
                let ty = match group.slots.len() {
                    1 => quote!(#ty),
                    len => {
                        let len = Literal::usize_unsuffixed(len);
                        quote!([#ty; #len])
                    }
                };
                (&group.name, ty)
            })
            .chain(self.children.iter().map(|child| {
                let ty = &child.ty;
                (&child.name, quote!(<#ty as #krate::sql::Sql>::Params))
            }))
            .collect();
        let names: Vec<&Ident> = fields.iter().map(|(name, _)| *name).collect();
        let marker_value = self.marker_value();
        let labels: Vec<String> = names
            .iter()
            .map(|name| name.to_string().trim_start_matches("r#").to_owned())
            .collect();
        let bounded = |bound: TokenStream| {
            let mut generics = self.input.generics.clone();
            let where_clause = generics.make_where_clause();
            for (_, ty) in &fields {
                where_clause
                    .predicates
                    .push(parse_quote!(for<'__a> #ty: #bound));
            }
            generics
        };
        let (_, ty_generics, _) = self.input.generics.split_for_impl();
        let generics = bounded(quote!(::core::clone::Clone));
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        let mut out = quote! {
            impl #impl_generics ::core::clone::Clone for #ident #ty_generics #where_clause {
                fn clone(&self) -> Self {
                    Self {
                        #(#names: ::core::clone::Clone::clone(&self.#names),)*
                        #marker_value
                    }
                }
            }
        };
        let generics = bounded(quote!(::core::fmt::Debug));
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        out.extend(quote! {
            impl #impl_generics ::core::fmt::Debug for #ident #ty_generics #where_clause {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct(#name)
                        #(.field(#labels, &self.#names))*
                        .finish()
                }
            }
        });
        let generics = bounded(quote!(::core::cmp::PartialEq));
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        out.extend(quote! {
            impl #impl_generics ::core::cmp::PartialEq for #ident #ty_generics #where_clause {
                fn eq(&self, other: &Self) -> bool {
                    true #(&& self.#names == other.#names)*
                }
            }
        });
        let generics = bounded(quote!(::core::cmp::Eq));
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        out.extend(quote! {
            impl #impl_generics ::core::cmp::Eq for #ident #ty_generics #where_clause {}
        });
        let (impl_generics, _, where_clause) = self.input.generics.split_for_impl();
        let item = &self.input.ident;
        out.extend(quote! {
            impl #impl_generics #krate::builder::ParamsOf for #ident #ty_generics #where_clause {
                type Item = #item #ty_generics;
            }
        });
        Some(out)
    }
}
