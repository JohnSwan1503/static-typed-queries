use syn::{GenericArgument, Generics, PathArguments, Type};

use crate::naming::type_key;
use crate::sql::template::Template;

pub(crate) fn items(
    template: &Template,
    generics: &Generics,
    separate: &[(Type, Type)],
) -> Vec<(Type, Type)> {
    let mut items = Vec::new();
    for ty in &template.children {
        add_item(ty, generics, separate, &mut items);
    }
    items
}

pub(crate) fn add_item(
    ty: &Type,
    generics: &Generics,
    separate: &[(Type, Type)],
    items: &mut Vec<(Type, Type)>,
) {
    let open = generics.type_params().any(
        |param| matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.is_ident(&param.ident)),
    );
    if open
        || items
            .iter()
            .any(|(_, named)| type_key(named) == type_key(ty))
    {
        return;
    }
    if let Some((_, wrapper)) = separate
        .iter()
        .find(|(named, _)| type_key(named) == type_key(ty))
    {
        items.push((wrapper.clone(), ty.clone()));
        return;
    }
    items.push((ty.clone(), ty.clone()));
    for arg in type_args(ty) {
        add_item(arg, generics, separate, items);
    }
}

pub(crate) fn type_args(ty: &Type) -> Vec<&Type> {
    let Type::Path(path) = ty else {
        return Vec::new();
    };
    let Some(PathArguments::AngleBracketed(args)) =
        path.path.segments.last().map(|last| &last.arguments)
    else {
        return Vec::new();
    };
    args.args
        .iter()
        .filter_map(|arg| match arg {
            GenericArgument::Type(arg) => Some(arg),
            _ => None,
        })
        .collect()
}
