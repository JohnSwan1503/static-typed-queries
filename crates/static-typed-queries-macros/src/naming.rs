use syn::{GenericArgument, PathArguments, Type};

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

pub(crate) fn field_name(ty: &Type) -> String {
    let Type::Path(path) = ty else {
        return "item".to_owned();
    };
    let Some(last) = path.path.segments.last() else {
        return "item".to_owned();
    };
    let mut name = snake_case(&last.ident.to_string());
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
