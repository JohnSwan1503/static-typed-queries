use syn::ext::IdentExt;
use syn::{Attribute, Fields, Ident, ItemStruct, Meta, Type, Visibility};

use crate::args::Placement;

pub(crate) enum Kind {
    Value,
    Item(Option<Placement>),
}

pub(crate) struct Field {
    pub(crate) ident: Ident,
    pub(crate) ty: Type,
    pub(crate) vis: Visibility,
    pub(crate) docs: Vec<Attribute>,
    pub(crate) kind: Kind,
}

impl Field {
    pub(crate) fn is_item(&self) -> bool {
        matches!(self.kind, Kind::Item(_))
    }
}

// A query's fields are its bind values, except those marked `#[cte]`, `#[cte(recursive)]` or
// `#[subquery]`, which embed an item. The markers are taken off the struct.
pub(crate) fn marked(input: &mut ItemStruct) -> syn::Result<Vec<Field>> {
    let mut fields = Vec::new();
    for field in named(input)? {
        let mut placement = None;
        let mut attrs = Vec::new();
        for attr in std::mem::take(&mut field.attrs) {
            match marker(&attr)? {
                Some(_) if placement.is_some() => {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "a field is embedded one way; give it one of `#[cte]`, `#[cte(recursive)]` or `#[subquery]`",
                    ));
                }
                Some(marked) => placement = Some(marked),
                None => attrs.push(attr),
            }
        }
        field.attrs = attrs;
        fields.push(Field {
            ident: field.ident.clone().expect("a named field"),
            ty: field.ty.clone(),
            vis: field.vis.clone(),
            docs: docs(&field.attrs),
            kind: match placement {
                Some(placement) => Kind::Item(Some(placement)),
                None => Kind::Value,
            },
        });
    }
    Ok(fields)
}

// A statement's or transaction's fields are all items, named by the attribute's arguments.
pub(crate) fn items(input: &mut ItemStruct) -> syn::Result<Vec<Field>> {
    let mut fields = Vec::new();
    for field in named(input)? {
        for attr in &field.attrs {
            if marker(attr)?.is_some() {
                return Err(syn::Error::new_spanned(
                    attr,
                    "only a query's template embeds items; a step runs on its own",
                ));
            }
        }
        fields.push(Field {
            ident: field.ident.clone().expect("a named field"),
            ty: field.ty.clone(),
            vis: field.vis.clone(),
            docs: docs(&field.attrs),
            kind: Kind::Item(None),
        });
    }
    Ok(fields)
}

fn named(input: &mut ItemStruct) -> syn::Result<Vec<&mut syn::Field>> {
    match &mut input.fields {
        Fields::Named(fields) => Ok(fields
            .named
            .iter_mut()
            .filter(|field| !is_phantom(&field.ty))
            .collect()),
        Fields::Unnamed(fields) => match fields.unnamed.iter().find(|field| !is_phantom(&field.ty))
        {
            Some(field) => Err(syn::Error::new_spanned(
                field,
                "the fields hold the values, so they need names",
            )),
            None => Ok(Vec::new()),
        },
        Fields::Unit => Ok(Vec::new()),
    }
}

fn docs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .cloned()
        .collect()
}

// The names of the fields that hold no value, which a built struct fills with `PhantomData`.
pub(crate) fn phantoms(input: &ItemStruct) -> Vec<Ident> {
    match &input.fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .filter(|field| is_phantom(&field.ty))
            .filter_map(|field| field.ident.clone())
            .collect(),
        _ => Vec::new(),
    }
}

fn marker(attr: &Attribute) -> syn::Result<Option<Placement>> {
    if attr.path().is_ident("subquery") {
        return match &attr.meta {
            Meta::Path(_) => Ok(Some(Placement::Subquery)),
            _ => Err(syn::Error::new_spanned(attr, "expected `#[subquery]`")),
        };
    }
    if !attr.path().is_ident("cte") {
        return Ok(None);
    }
    match &attr.meta {
        Meta::Path(_) => Ok(Some(Placement::Cte { recursive: false })),
        Meta::List(list) if list.tokens.to_string() == "recursive" => {
            Ok(Some(Placement::Cte { recursive: true }))
        }
        _ => Err(syn::Error::new_spanned(
            attr,
            "expected `#[cte]` or `#[cte(recursive)]`",
        )),
    }
}

fn is_phantom(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Path(path) if path.path.segments.last().is_some_and(|last| last.ident == "PhantomData")
    )
}

// A statement's target or a transaction's step names a field, or a type that holds no values.
pub(crate) fn resolve<'a>(named: &Type, fields: &'a [Field]) -> Option<(u16, &'a Field)> {
    let Type::Path(path) = named else {
        return None;
    };
    let name = path.path.get_ident()?;
    fields
        .iter()
        .enumerate()
        .find(|(_, field)| field.ident.unraw() == name.unraw())
        .map(|(index, field)| (index as u16, field))
}
