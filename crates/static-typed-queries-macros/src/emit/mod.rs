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

// Generated impls sit in `const _: () = { use <codegen> as __stq; … }` so they can name core
// items briefly. Definitions outside that block, which must stay nameable, use the full path.
pub(crate) fn krate() -> TokenStream {
    quote!(__stq)
}

pub(crate) fn krate_path() -> TokenStream {
    quote!(::static_typed_queries::__private::codegen)
}

pub(crate) fn scoped(impls: TokenStream) -> TokenStream {
    let path = krate_path();
    quote! {
        const _: () = {
            use #path as __stq;

            #impls
        };
    }
}

pub(crate) fn doc(text: &str) -> TokenStream {
    let text = format!(" {text}");
    quote!(#[doc = #text])
}
