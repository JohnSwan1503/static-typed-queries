use proc_macro2::{Literal, TokenStream};
use quote::quote;
use syn::{Ident, ItemStruct, Type};

use crate::args::Placement;
use crate::emit::docs::type_string;
use crate::emit::krate;
use crate::model::fields::Field;
use crate::sql::analyze::Analysis;
use crate::sql::template::{Segment, Template};

pub(crate) fn node(
    name: &str,
    fingerprint: TokenStream,
    kind: TokenStream,
    inject: TokenStream,
    parts: Vec<TokenStream>,
    items: &[&Type],
    [before, after]: [&[Type]; 2],
    checks: TokenStream,
) -> TokenStream {
    let krate = krate();
    quote! {
        {
            #checks
            &#krate::Node {
                name: #krate::Name::new(#name),
                fingerprint: #fingerprint,
                kind: #krate::Kind::#kind,
                inject: #inject,
                parts: #krate::Parts(&[#(#parts),*]),
                before: #krate::Hooks(&[#(<#before as #krate::Sql>::NODE),*]),
                after: #krate::Hooks(&[#(<#after as #krate::Sql>::NODE),*]),
                items: #krate::Items(&[#(<#items as #krate::Sql>::NODE),*]),
            }
        }
    }
}

pub(crate) fn fingerprint(ident: &Ident, item: &ItemStruct) -> TokenStream {
    let krate = krate();
    let path = format!("::{ident}");
    let mut fingerprint = quote! {
        #krate::Fingerprint::of(
            ::core::concat!(::core::module_path!(), #path)
        )
    };
    for param in item.generics.type_params() {
        let param = &param.ident;
        fingerprint = quote!(#fingerprint.combine(<#param as #krate::Sql>::NODE.fingerprint));
    }
    fingerprint
}

pub(crate) fn placement(placement: Placement) -> TokenStream {
    let krate = krate();
    match placement {
        Placement::Cte { recursive } => quote!(#krate::Inject::cte(#recursive)),
        Placement::Subquery => quote!(#krate::Inject::Subquery),
    }
}

// An item field is referenced by its index among the item fields; a type holds no values and is
// referenced by its node.
pub(crate) fn target(item: Option<u16>, ty: &Type) -> TokenStream {
    let krate = krate();
    match item {
        Some(index) => {
            let index = Literal::u16_unsuffixed(index);
            quote!(#krate::Target::Item(#index))
        }
        None => quote!(#krate::Target::Node(<#ty as #krate::Sql>::NODE)),
    }
}

pub(crate) fn parts(
    template: &Template,
    analysis: &Analysis,
    fields: &[Field],
) -> Vec<TokenStream> {
    let krate = krate();
    let values: Vec<&Field> = fields.iter().filter(|field| !field.is_item()).collect();
    let mut positions = analysis.refs.iter();
    template
        .segments
        .iter()
        .map(|segment| match segment {
            Segment::Lit(text) => quote!(#krate::Lit::part(#text)),
            Segment::Param(slot) => {
                let field = values[*slot as usize];
                let (name, ty) = (
                    field.ident.to_string().trim_start_matches("r#").to_owned(),
                    type_string(&field.ty),
                );
                let slot = Literal::u16_unsuffixed(*slot);
                quote!(#krate::Param::part(#slot, #name, #ty))
            }
            Segment::Ref(item) => {
                let position = positions.next().expect("a position for every reference");
                let target = target(item.item, &item.ty);
                if !position.from {
                    return quote!(#krate::Expr::part(#target));
                }
                let rule = if position.alias.is_some() {
                    quote!(Given)
                } else {
                    quote!(NodeName)
                };
                let inject = match item.placement {
                    Some(placement) => {
                        let placement = self::placement(placement);
                        quote!(::core::option::Option::Some(#placement))
                    }
                    None => quote!(::core::option::Option::None),
                };
                quote! {
                    #krate::From::part(
                        #target,
                        #krate::AliasRule::#rule,
                        #inject,
                    )
                }
            }
        })
        .collect()
}
