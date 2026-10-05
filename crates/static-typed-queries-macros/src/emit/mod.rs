pub(crate) mod bind;
pub(crate) mod builder;
pub(crate) mod checks;
pub(crate) mod docs;
pub(crate) mod fmt;
pub(crate) mod node;
pub(crate) mod params;
pub(crate) mod rows;

use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn krate() -> TokenStream {
    quote!(::static_typed_queries::__private)
}

pub(crate) fn doc(text: &str) -> TokenStream {
    let text = format!(" {text}");
    quote!(#[doc = #text])
}
