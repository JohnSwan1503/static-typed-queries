use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, ItemStruct};

use crate::emit::krate;

pub(crate) fn fmt(
    display: Option<&Ident>,
    debug: Option<&Ident>,
    item: &ItemStruct,
    statement: bool,
) -> syn::Result<TokenStream> {
    let krate = krate();
    let ident = &item.ident;
    let mut out = TokenStream::new();
    for (option, allowed, macro_name) in [
        (display, ["sql", "name"], quote!(impl_display)),
        (debug, ["sql", "tree"], quote!(impl_debug)),
    ] {
        let Some(value) = option else { continue };
        if !allowed.iter().any(|allowed| value == allowed) {
            return Err(syn::Error::new(
                value.span(),
                format!("expected `{}` or `{}`", allowed[0], allowed[1]),
            ));
        }
        if !item.generics.params.is_empty() {
            return Err(syn::Error::new(
                value.span(),
                "generic items can't take `display`/`debug`; give them to a `#[statement]` for an instantiation",
            ));
        }
        if value == "sql" && !statement {
            return Err(syn::Error::new(
                value.span(),
                "tables have no SQL of their own",
            ));
        }
        out.extend(quote!(#krate::#macro_name!(#ident => #value);));
    }
    Ok(out)
}
