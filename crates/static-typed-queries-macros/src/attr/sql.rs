use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;
use syn::{Expr, ExprLit, ItemConst, Lit, Type, Visibility};

pub(crate) fn expand(args: TokenStream, item: ItemConst) -> syn::Result<TokenStream> {
    if !args.is_empty() {
        return Err(syn::Error::new_spanned(args, "`#[sql]` takes no arguments"));
    }
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "`#[sql]` constants can't be generic",
        ));
    }
    let is_str = matches!(
        &*item.ty,
        Type::Reference(reference) if reference.mutability.is_none()
            && matches!(&*reference.elem, Type::Path(path) if path.qself.is_none() && path.path.is_ident("str"))
    );
    if !is_str {
        return Err(syn::Error::new_spanned(
            &item.ty,
            "`#[sql]` declares a `&str` constant",
        ));
    }
    let Expr::Lit(ExprLit {
        lit: Lit::Str(sql), ..
    }) = &*item.expr
    else {
        return Err(syn::Error::new_spanned(
            &item.expr,
            "`#[sql]` needs the template as a string literal, since `#[query]` reads it before constants have values",
        ));
    };
    let ident = &item.ident;
    let hidden = format_ident!("__stq_sql_{}", ident.unraw());
    let vis = match &item.vis {
        Visibility::Inherited => None,
        Visibility::Public(_) => Some(quote!(pub(crate))),
        Visibility::Restricted(restricted) => Some(quote!(#restricted)),
    };
    Ok(quote! {
        #item

        #[doc(hidden)]
        #[allow(unused_macros)]
        macro_rules! #hidden {
            ($($tokens:tt)*) => {
                ::static_typed_queries::__query_sql! { #sql $($tokens)* }
            };
        }

        #[doc(hidden)]
        #[allow(unused_imports)]
        #vis use #hidden as #ident;
    })
}
