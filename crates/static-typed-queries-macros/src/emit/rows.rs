use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Field, Fields, ItemStruct, Type, parse_quote};

use crate::emit::docs;
use crate::emit::krate;
use crate::sql::analyze::Columns;

pub(crate) fn named_fields(item: &ItemStruct) -> Vec<&Field> {
    match &item.fields {
        Fields::Named(fields) => fields.named.iter().collect(),
        Fields::Unnamed(_) | Fields::Unit => Vec::new(),
    }
}

pub(crate) fn column_name(field: &Field) -> String {
    let name = field.ident.as_ref().expect("a named field");
    name.to_string().trim_start_matches("r#").to_owned()
}

pub(crate) fn row(
    declared: Option<&Type>,
    item: &ItemStruct,
    returns_rows: bool,
    columns: Option<&Columns>,
) -> syn::Result<Option<Type>> {
    let fields = named_fields(item);
    if fields.is_empty() {
        return Ok(declared.cloned());
    }
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "generic queries aren't statements, so their fields can't be a row type; declare them as `struct Name<T>(PhantomData<T>)`",
        ));
    }
    if let Some(row) = declared {
        return Err(syn::Error::new_spanned(
            row,
            "this struct's fields are its row type, so it doesn't take `row`",
        ));
    }
    if !returns_rows {
        return Err(syn::Error::new_spanned(
            &item.fields,
            "this statement returns no rows, so its struct can't have fields; only queries and statements with `RETURNING` have a row type",
        ));
    }
    if let Some(columns) = columns.filter(|columns| columns.complete) {
        for field in &fields {
            let name = column_name(field);
            if !columns.names.contains(&name) {
                let names: Vec<String> = columns
                    .names
                    .iter()
                    .map(|name| format!("`{name}`"))
                    .collect();
                return Err(syn::Error::new_spanned(
                    &field.ident,
                    format!(
                        "the statement returns no column named `{name}`; its columns are {}",
                        names.join(", ")
                    ),
                ));
            }
        }
    }
    let ident = &item.ident;
    Ok(Some(parse_quote!(#ident)))
}

pub(crate) fn from_row(item: &ItemStruct, dialect: &Type) -> Option<TokenStream> {
    let fields = named_fields(item);
    if fields.is_empty() {
        return None;
    }
    let krate = krate();
    let sqlx = quote!(#krate::sqlx);
    let driver = quote!(#krate::driver);
    let ident = &item.ident;
    let reads = fields.iter().map(|field| {
        let (name, ty) = (&field.ident, &field.ty);
        let column = column_name(field);
        quote_spanned!(ty.span()=> #name: #sqlx::Row::try_get::<#ty, _>(row, #column)?)
    });
    Some(quote! {
        #krate::__if_sqlx! {
            impl<'__r> #sqlx::FromRow<'__r, #driver::Row<#dialect>> for #ident {
                fn from_row(
                    row: &'__r #driver::Row<#dialect>,
                ) -> ::core::result::Result<Self, #sqlx::Error> {
                    ::core::result::Result::Ok(Self {
                        #(#reads,)*
                    })
                }
            }
        }
    })
}

pub(crate) fn row_docs(item: &ItemStruct) -> Vec<syn::Attribute> {
    if named_fields(item).is_empty() {
        return Vec::new();
    }
    docs::attrs(&[
        String::new(),
        "# Row".to_owned(),
        String::new(),
        "Each row the statement returns is read into this struct, matching fields to columns by name."
            .to_owned(),
    ])
}
