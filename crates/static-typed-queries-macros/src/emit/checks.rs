use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, ItemStruct, Type, parse_quote};

use crate::emit::krate;

pub(crate) fn embeds(item: &ItemStruct, items: &[Type], dialect: &Type) -> TokenStream {
    let krate = krate();
    let ident = &item.ident;
    if item.generics.params.is_empty() {
        let checks = items.iter().map(|ty| {
            quote! {
                const _: () = {
                    #krate::embed::embeds::<#dialect, <#ty as #krate::sql::Sql>::Dialect>();
                    #krate::embed::checked::<#ty>();
                };
            }
        });
        return quote! {
            impl #krate::embed::Checked for #ident {}

            #(#checks)*
        };
    }
    let mut generics = item.generics.clone();
    let clause = generics.make_where_clause();
    for ty in items {
        clause.predicates.push(parse_quote!(
            <#ty as #krate::sql::Sql>::Dialect: #krate::embed::EmbedsIn<#dialect>
        ));
        clause
            .predicates
            .push(parse_quote!(#ty: #krate::embed::Checked));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    quote! {
        impl #impl_generics #krate::embed::Checked for #ident #ty_generics #where_clause {}
    }
}

pub(crate) fn step_impl(item: &ItemStruct, row: Option<&Type>) -> TokenStream {
    let krate = krate();
    let run = quote!(#krate::statement::run);
    let (fetch, row) = match row {
        Some(row) => (quote!(#run::AllRows<#row>), quote!(#row)),
        None => (quote!(#run::Affected), quote!(#run::NoRow)),
    };
    let ident = &item.ident;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    quote! {
        #krate::__if_sqlx! {
            impl #impl_generics #run::Step for #ident #ty_generics #where_clause {
                type Fetch = #fetch;
                type Row = #row;
            }
        }
    }
}

pub(crate) fn hook_needs(
    item: &ItemStruct,
    needs: &[TokenStream],
    bound: TokenStream,
) -> TokenStream {
    let krate = krate();
    let ident = &item.ident;
    let indices: Vec<Ident> = (0..needs.len()).map(|i| format_ident!("__I{i}")).collect();
    let mut generics = item.generics.clone();
    generics.params.push(parse_quote!(__V));
    for index in &indices {
        generics.params.push(parse_quote!(#index));
    }
    let clause = generics.make_where_clause();
    for (need, index) in needs.iter().zip(&indices) {
        clause
            .predicates
            .push(parse_quote!(#need: #bound<__V, #index>));
    }
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let (_, ty_generics, _) = item.generics.split_for_impl();
    quote! {
        impl #impl_generics #krate::builder::HookNeeds<__V, (#(#indices,)*)> for #ident #ty_generics #where_clause {}
    }
}
