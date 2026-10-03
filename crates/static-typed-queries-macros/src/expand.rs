use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{GenericParam, Ident, ItemStruct, LitStr, Type};

use crate::analyze::{self, Analysis, Engine, Kind};
use crate::args::{Args, Placement};
use crate::naming::{field_name, snake_case, unique};
use crate::template::{self, Segment, Template};

fn krate() -> TokenStream {
    quote!(::static_typed_queries)
}

pub(crate) fn table(args: Args, item: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if let Some(sql) = &args.sql {
        return Err(syn::Error::new(sql.span(), "tables take `name`, not `sql`"));
    }
    if args.placement.is_some() {
        return Err(syn::Error::new(
            Span::call_site(),
            "tables are always referenced by name",
        ));
    }
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &item.generics,
            "tables can't be generic",
        ));
    }
    let name = args
        .name
        .as_ref()
        .ok_or_else(|| syn::Error::new(Span::call_site(), "tables need `name = \"...\"`"))?;
    let table = name.value();
    let mut parts = Vec::new();
    for (i, segment) in table.split('.').enumerate() {
        if i > 0 {
            parts.push(quote!(#krate::part::lit::Lit::part(".")));
        }
        parts.push(quote!(#krate::part::ident::Ident::part(#segment)));
    }
    let node_name = table.rsplit('.').next().unwrap_or(&table);
    let ident = &item.ident;
    let dialect = &args.dialect;
    let node = node(
        node_name,
        fingerprint(ident, &item),
        quote!(Table),
        quote!(#krate::node::inject::Inject::Ident),
        parts,
    );

    Ok(quote! {
        #item

        impl #krate::sql::Sql for #ident {
            type Dialect = #dialect;
            type Params = ();
            const NODE: &'static #krate::node::Node = #node;
        }
    })
}

pub(crate) fn query(args: Args, item: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if let Some(param) = item
        .generics
        .params
        .iter()
        .find(|param| !matches!(param, GenericParam::Type(_)))
    {
        return Err(syn::Error::new_spanned(
            param,
            "only type parameters are supported on queries",
        ));
    }
    let sql = args
        .sql
        .as_ref()
        .ok_or_else(|| syn::Error::new(Span::call_site(), "queries need `sql = \"...\"`"))?;
    let template = template::parse(sql)?;
    let analysis = analyze::analyze(&template, Engine::of(&args.dialect), sql)?;
    let kind = match analysis.kind {
        Kind::Query => quote!(Query),
        Kind::Dml => quote!(Dml),
        Kind::Ddl => quote!(Ddl),
    };

    let ident = &item.ident;
    let node_name = args
        .name
        .as_ref()
        .map_or_else(|| snake_case(&ident.to_string()), LitStr::value);
    let inject = placement(args.placement.unwrap_or(Placement::Subquery));
    let node = node(
        &node_name,
        fingerprint(ident, &item),
        kind,
        inject,
        parts(&template, &analysis),
    );

    let dialect = &args.dialect;
    let (impl_generics, ty_generics, where_clause) = item.generics.split_for_impl();
    let params = Params::new(&template, &item);
    let params_ty = params.ty();
    let params_struct = params.definition();
    let statement = item.generics.params.is_empty().then(|| {
        quote! {
            #krate::impl_statement!(#ident);

            impl #ident {
                pub const SQL: &'static str = <Self as #krate::statement::Statement>::SQL;
            }
        }
    });

    Ok(quote! {
        #item

        #params_struct

        impl #impl_generics #krate::sql::Sql for #ident #ty_generics #where_clause {
            type Dialect = #dialect;
            type Params = #params_ty;
            const NODE: &'static #krate::node::Node = #node;
        }

        #statement
    })
}

fn node(
    name: &str,
    fingerprint: TokenStream,
    kind: TokenStream,
    inject: TokenStream,
    parts: Vec<TokenStream>,
) -> TokenStream {
    let krate = krate();
    quote! {
        &#krate::node::Node {
            name: #krate::node::name::Name::new(#name),
            fingerprint: #fingerprint,
            kind: #krate::node::kind::Kind::#kind,
            inject: #inject,
            parts: #krate::part::Parts(&[#(#parts),*]),
            before: #krate::node::before::Before(&[]),
        }
    }
}

fn fingerprint(ident: &Ident, item: &ItemStruct) -> TokenStream {
    let krate = krate();
    let path = format!("::{ident}");
    let mut fingerprint = quote! {
        #krate::node::fingerprint::Fingerprint::of(
            ::core::concat!(::core::module_path!(), #path)
        )
    };
    for param in item.generics.type_params() {
        let param = &param.ident;
        fingerprint = quote!(#fingerprint.combine(<#param as #krate::sql::Sql>::NODE.fingerprint));
    }
    fingerprint
}

fn placement(placement: Placement) -> TokenStream {
    let krate = krate();
    match placement {
        Placement::Cte { recursive } => quote!(#krate::node::inject::Inject::cte(#recursive)),
        Placement::Subquery => quote!(#krate::node::inject::Inject::Subquery),
    }
}

fn parts(template: &Template, analysis: &Analysis) -> Vec<TokenStream> {
    let krate = krate();
    let mut positions = analysis.refs.iter();
    template
        .segments
        .iter()
        .map(|segment| match segment {
            Segment::Lit(text) => quote!(#krate::part::lit::Lit::part(#text)),
            Segment::Param(slot) => {
                let slot = Literal::u16_unsuffixed(*slot);
                quote!(#krate::part::param::Param::part(#slot))
            }
            Segment::Ref(item) => {
                let position = positions.next().expect("a position for every reference");
                let ty = &item.ty;
                let node = quote!(<#ty as #krate::sql::Sql>::NODE);
                if !position.from {
                    return quote!(#krate::part::expr::Expr::part(#node));
                }
                let rule = if position.given_alias {
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
                    #krate::part::from::From::part(
                        #node,
                        #krate::part::from::rule::AliasRule::#rule,
                        #inject,
                    )
                }
            }
        })
        .collect()
}

struct Field {
    name: Ident,
    ty: Type,
}

struct Child {
    name: Ident,
    ty: Type,
}

struct Params<'a> {
    item: &'a ItemStruct,
    ident: Ident,
    fields: Vec<Field>,
    children: Vec<Child>,
}

impl<'a> Params<'a> {
    fn new(template: &Template, item: &'a ItemStruct) -> Params<'a> {
        let fields: Vec<Field> = template
            .params
            .iter()
            .enumerate()
            .map(|(slot, param)| Field {
                name: param
                    .name
                    .clone()
                    .unwrap_or_else(|| format_ident!("bind{}", slot + 1)),
                ty: param.ty.clone(),
            })
            .collect();

        let mut taken: Vec<String> = fields.iter().map(|field| field.name.to_string()).collect();
        let children = template
            .children
            .iter()
            .map(|ty| {
                let name = unique(field_name(ty), &taken);
                taken.push(name.clone());
                Child {
                    name: format_ident!("{name}"),
                    ty: ty.clone(),
                }
            })
            .collect();
        Params {
            item,
            ident: format_ident!("{}Params", item.ident),
            fields,
            children,
        }
    }

    fn is_empty(&self) -> bool {
        self.fields.is_empty() && self.children.is_empty()
    }

    fn item_args(&self) -> Vec<&Ident> {
        self.item
            .generics
            .type_params()
            .map(|param| &param.ident)
            .collect()
    }

    fn ty(&self) -> TokenStream {
        if self.is_empty() {
            return quote!(());
        }
        let ident = &self.ident;
        let args = self.item_args();
        if args.is_empty() {
            quote!(#ident)
        } else {
            quote!(#ident<#(#args),*>)
        }
    }

    fn definition(&self) -> Option<TokenStream> {
        if self.is_empty() {
            return None;
        }
        let krate = krate();
        let vis = &self.item.vis;
        let ident = &self.ident;
        let generics = &self.item.generics;
        let where_clause = &generics.where_clause;
        let fields = self.fields.iter().map(|field| {
            let (name, ty) = (&field.name, &field.ty);
            quote!(pub #name: #ty)
        });
        let children = self.children.iter().map(|child| {
            let (name, ty) = (&child.name, &child.ty);
            quote!(pub #name: <#ty as #krate::sql::Sql>::Params)
        });
        Some(quote! {
            #vis struct #ident #generics #where_clause {
                #(#fields,)*
                #(#children,)*
            }
        })
    }
}
