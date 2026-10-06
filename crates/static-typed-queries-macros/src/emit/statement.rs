use proc_macro2::TokenStream;
use quote::quote;
use syn::{ItemStruct, LitBool, Type};

use crate::emit::checks::parse_test;
use crate::emit::krate;
use crate::emit::run::statement_methods;

// Returns the impls that make a concrete item a statement, the methods of its complete builder,
// and the test that parses its SQL.
pub(crate) fn statement(
    input: &ItemStruct,
    dialect: &Type,
    row: Option<&Type>,
    parse_check: Option<&LitBool>,
    facade: &TokenStream,
) -> (TokenStream, TokenStream, Option<TokenStream>) {
    let krate = krate();
    let ident = &input.ident;
    let (methods, delegates) = statement_methods(input, dialect, row);
    let rows = row.map(|row| {
        quote! {
            impl #krate::Rows for #ident {
                type Row = #row;
            }
        }
    });
    let impls = quote! {
        #krate::impl_statement!(#ident);
        #rows
        #methods

        impl #ident {
            /// The SQL, rendered at compile time.
            pub const SQL: &'static str = <Self as #krate::Statement>::SQL;
        }
    };
    (impls, delegates, parse_test(ident, parse_check, facade))
}
