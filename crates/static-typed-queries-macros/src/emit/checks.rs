use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, ItemStruct, LitBool, Type, WherePredicate, parse_quote};

use crate::emit::run::{Rows, rows};
use crate::emit::{krate, krate_path};
use crate::naming::snake_case;

// Every embedded item must speak the statement's dialect and pass its own checks; one that is
// named by type rather than held in a field must hold no values, and one placed as a CTE must be
// allowed there. For a concrete item the checks run inside its node, so a failing one stops the
// node from rendering too.
pub(crate) struct Embeds {
    pub(crate) checked: TokenStream,
    pub(crate) node: TokenStream,
}

pub(crate) fn embeds(
    item: &ItemStruct,
    fields: &[&Type],
    types: &[Type],
    ctes: &[&Type],
    dialect: &Type,
) -> Embeds {
    let krate = krate();
    let ident = &item.ident;
    if item.generics.params.is_empty() {
        let check = |ty: &Type, valueless: bool| {
            let valueless = valueless.then(|| quote!(#krate::valueless::<#ty>();));
            quote! {
                #krate::embeds::<#dialect, <#ty as #krate::Sql>::Dialect>();
                #krate::checked::<#ty>();
                #valueless
            }
        };
        let checks = fields
            .iter()
            .map(|ty| check(ty, false))
            .chain(types.iter().map(|ty| check(ty, true)))
            .chain(
                ctes.iter()
                    .map(|ty| quote!(#krate::cte::<#dialect, #ty>();)),
            );
        return Embeds {
            checked: quote!(impl #krate::Checked for #ident {}),
            node: quote!(#(#checks)*),
        };
    }
    let mut generics = item.generics.clone();
    let clause = generics.make_where_clause();
    for ty in fields.iter().copied().chain(types) {
        clause.predicates.push(parse_quote!(
            <#ty as #krate::Sql>::Dialect: #krate::EmbedsIn<#dialect>
        ));
        clause.predicates.push(parse_quote!(#ty: #krate::Checked));
    }
    for ty in types {
        clause.predicates.push(parse_quote!(#ty: #krate::Valueless));
    }
    for ty in ctes {
        clause
            .predicates
            .push(parse_quote!(#ty: #krate::CteIn<#dialect>));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    Embeds {
        checked: quote! {
            impl #impl_generics #krate::Checked for #ident #ty_generics #where_clause {}
        },
        node: TokenStream::new(),
    }
}

// What an item is, for the marker traits checked where it is named: the dialects it can be a CTE
// in (any, those that run data-modifying CTEs, or those its target can be one in), and whether it
// runs as a statement.
pub(crate) enum Item<'a> {
    Table,
    Query { modifies: bool },
    Statement { target: &'a Type },
    Transaction,
}

pub(crate) fn markers(item: &ItemStruct, kind: Item) -> TokenStream {
    let krate = krate();
    let ident = &item.ident;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    let mut generics = item.generics.clone();
    let dialect = match kind {
        Item::Query { modifies: true } => quote!(#krate::DmlInCte),
        _ => quote!(#krate::Dialect),
    };
    generics.params.push(parse_quote!(__D: #dialect));
    if let Item::Statement { target } = kind {
        generics
            .make_where_clause()
            .predicates
            .push(parse_quote!(#target: #krate::CteIn<__D>));
    }
    let (cte_generics, _, cte_where) = generics.split_for_impl();
    let runs: &[&str] = match kind {
        Item::Table => &["NotTransaction"],
        Item::Transaction => &["NotTable"],
        Item::Query { .. } | Item::Statement { .. } => &["NotTable", "NotTransaction"],
    };
    let runs = runs.iter().map(|marker| format_ident!("{marker}"));
    quote! {
        #[diagnostic::do_not_recommend]
        impl #cte_generics #krate::CteIn<__D> for #ident #ty_generics #cte_where {}
        #(
            #[diagnostic::do_not_recommend]
            impl #impl_generics #krate::#runs for #ident #ty_generics #where_clause {}
        )*
    }
}

// Checks, for the item's node, that each of `types` runs as a statement.
pub(crate) fn runnable(types: &[&Type]) -> TokenStream {
    let krate = krate();
    quote!(#(#krate::runnable::<#types>();)*)
}

pub(crate) fn values(item: &ItemStruct, holds_values: bool) -> TokenStream {
    let krate = krate();
    let ident = &item.ident;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    let marker = if holds_values {
        quote!(Values)
    } else {
        quote!(Valueless)
    };
    quote! {
        #[diagnostic::do_not_recommend]
        impl #impl_generics #krate::#marker for #ident #ty_generics #where_clause {}
    }
}

pub(crate) fn step_impl(item: &ItemStruct, row: Option<&Type>) -> TokenStream {
    let krate = krate();
    let Rows { fetch, row, .. } = rows(row);
    let ident = &item.ident;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    quote! {
        #krate::__if_sqlx! {
            impl #impl_generics #krate::run::Step for #ident #ty_generics #where_clause {
                type Fetch = #fetch;
                type Row = #row;
            }
        }
    }
}

// `need` turns an index type into the predicate that says the values `__V` cover one item.
pub(crate) fn hook_needs(
    item: &ItemStruct,
    count: usize,
    need: impl Fn(usize, &Ident) -> WherePredicate,
) -> TokenStream {
    let krate = krate();
    let ident = &item.ident;
    let indices: Vec<Ident> = (0..count).map(|i| format_ident!("__I{i}")).collect();
    let mut generics = item.generics.clone();
    generics.params.push(parse_quote!(__V));
    for index in &indices {
        generics.params.push(parse_quote!(#index));
    }
    let clause = generics.make_where_clause();
    for (i, index) in indices.iter().enumerate() {
        clause.predicates.push(need(i, index));
    }
    let (impl_generics, _, where_clause) = generics.split_for_impl();
    let (_, ty_generics, _) = item.generics.split_for_impl();
    quote! {
        impl #impl_generics #krate::HookNeeds<__V, (#(#indices,)*)> for #ident #ty_generics #where_clause {}
    }
}

pub(crate) fn parse_test(ident: &Ident, parse_check: Option<&LitBool>) -> Option<TokenStream> {
    let krate = krate_path();
    let test = format_ident!("{}_sql_parses", snake_case(&ident.to_string()));
    parse_check.is_none_or(|check| check.value).then(|| {
        quote! {
            #krate::__if_parse_check! {
                #[cfg(test)]
                #[test]
                fn #test() {
                    #krate::check::parse::<#ident>();
                }
            }
        }
    })
}
