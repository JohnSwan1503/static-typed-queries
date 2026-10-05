use proc_macro2::TokenStream;
use quote::quote;
use syn::ItemStruct;

use crate::args::Args;
use crate::emit::krate;
use crate::emit::rows::named_fields;

pub(crate) fn fmt(args: &Args, item: &ItemStruct, statement: bool) -> syn::Result<TokenStream> {
    let krate = krate();
    let ident = &item.ident;
    let mut out = TokenStream::new();
    for (option, allowed, macro_name) in [
        (&args.display, ["sql", "name"], quote!(impl_display)),
        (&args.debug, ["sql", "tree"], quote!(impl_debug)),
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
        if !named_fields(item).is_empty() {
            return Err(syn::Error::new(
                value.span(),
                "a struct with fields is its own row type, so `display`/`debug` would describe each row; implement them yourself",
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
