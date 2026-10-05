use proc_macro2::Span;
use quote::{ToTokens, format_ident};
use syn::{GenericArgument, Ident, PathArguments, Type};

pub(crate) fn snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            let prev_lower =
                i > 0 && (chars[i - 1].is_lowercase() || chars[i - 1].is_ascii_digit());
            let acronym_end = i > 0
                && chars[i - 1].is_uppercase()
                && chars.get(i + 1).is_some_and(|next| next.is_lowercase());
            if prev_lower || acronym_end {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

pub(crate) fn short_name(ty: &Type) -> String {
    match ty {
        Type::Path(path) => path.path.segments.last().map_or_else(
            || "item".to_owned(),
            |last| snake_case(&last.ident.to_string()),
        ),
        _ => "item".to_owned(),
    }
}

pub(crate) fn field_name(ty: &Type) -> String {
    let mut name = short_name(ty);
    let Type::Path(path) = ty else {
        return name;
    };
    let Some(last) = path.path.segments.last() else {
        return name;
    };
    if let PathArguments::AngleBracketed(args) = &last.arguments {
        for arg in &args.args {
            if let GenericArgument::Type(inner) = arg {
                name.push('_');
                name.push_str(&field_name(inner));
            }
        }
    }
    name
}

pub(crate) fn unique(name: String, taken: &[String]) -> String {
    if !taken.contains(&name) {
        return name;
    }
    (2..)
        .map(|n| format!("{name}_{n}"))
        .find(|candidate| !taken.contains(candidate))
        .expect("an unused name")
}

pub(crate) fn to_ident(name: &str) -> Ident {
    let mut name: String = snake_case(name)
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.is_empty() {
        name.push_str("param");
    }
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        name.insert(0, '_');
    }
    if syn::parse_str::<Ident>(&name).is_ok() {
        return format_ident!("{name}");
    }
    if matches!(name.as_str(), "self" | "super" | "crate" | "Self" | "_") {
        return format_ident!("{name}_");
    }
    Ident::new_raw(&name, Span::call_site())
}

pub(crate) fn camel(ident: &Ident) -> String {
    let name = ident.to_string();
    name.trim_start_matches("r#")
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect()
}

pub(crate) fn type_key(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}
