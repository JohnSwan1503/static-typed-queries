pub(crate) mod bind;
pub(crate) mod builder;
pub(crate) mod checks;
pub(crate) mod docs;
pub(crate) mod fmt;
pub(crate) mod node;
pub(crate) mod run;
pub(crate) mod statement;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::Path;

// Generated impls sit in `const _: () = { use <codegen> as __stq; … }` so they can name core
// items briefly. Definitions outside that block, which must stay nameable, use the full path.
pub(crate) fn krate() -> TokenStream {
    quote!(__stq)
}

// The facade crate: `::static_typed_queries`, or the path an item gives with `crate = path`.
pub(crate) fn facade(path: Option<&Path>) -> TokenStream {
    path.map_or_else(|| quote!(::static_typed_queries), ToTokens::to_token_stream)
}

pub(crate) fn krate_path(facade: &TokenStream) -> TokenStream {
    quote!(#facade::__private::codegen)
}

pub(crate) fn scoped(facade: &TokenStream, impls: TokenStream) -> TokenStream {
    let path = krate_path(facade);
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
