use quote::ToTokens;
use syn::{Attribute, Ident, Type, parse_quote};

use crate::analyze::Origin;

pub(crate) fn attrs(lines: &[String]) -> Vec<Attribute> {
    lines
        .iter()
        .map(|line| {
            let line = if line.is_empty() {
                String::new()
            } else {
                format!(" {line}")
            };
            parse_quote!(#[doc = #line])
        })
        .collect()
}

pub(crate) fn template(sql: &str) -> Vec<String> {
    let lines: Vec<&str> = sql.lines().collect();
    let blank = |line: &&str| line.trim().is_empty();
    let (Some(first), Some(last)) = (
        lines.iter().position(|line| !blank(line)),
        lines.iter().rposition(|line| !blank(line)),
    ) else {
        return Vec::new();
    };
    // A first line that follows the opening quote doesn't count towards the indent.
    let indent = lines[first..=last]
        .iter()
        .enumerate()
        .filter(|(i, line)| !blank(line) && !(*i == 0 && first == 0))
        .map(|(_, line)| line.len() - line.trim_start().len())
        .min()
        .unwrap_or(0);
    lines[first..=last]
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if i == 0 && first == 0 {
                line.trim().to_owned()
            } else {
                line.get(indent..).unwrap_or("").trim_end().to_owned()
            }
        })
        .collect()
}

pub(crate) fn origin(origin: Origin, name: &str) -> String {
    match origin {
        Origin::Compared => format!("compared with `{name}`"),
        Origin::Inserted => format!("inserted into `{name}`"),
        Origin::Assigned => format!("assigned to `{name}`"),
        Origin::Selected => format!("selected as `{name}`"),
        Origin::Limit => "used as the `LIMIT`".to_owned(),
        Origin::Offset => "used as the `OFFSET`".to_owned(),
    }
}

pub(crate) fn origins(origins: &[String]) -> String {
    match origins {
        [one] => one.clone(),
        [first, rest @ ..] if rest.iter().all(|other| other == first) => format!("each {first}"),
        _ => origins.join(", then "),
    }
}

pub(crate) fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

pub(crate) fn link(ty: &Type, generics: &[&Ident]) -> String {
    let text = type_string(ty);
    let Type::Path(path) = ty else {
        return format!("`{text}`");
    };
    let generic = path
        .path
        .segments
        .first()
        .is_some_and(|segment| generics.contains(&&segment.ident));
    if path.qself.is_some() || generic {
        return format!("`{text}`");
    }
    let mut target = String::new();
    if path.path.leading_colon.is_some() {
        target.push_str("::");
    }
    let segments: Vec<String> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    target.push_str(&segments.join("::"));
    if target == text {
        format!("[`{text}`]")
    } else {
        format!("[`{text}`]({target})")
    }
}

pub(crate) fn type_string(ty: &Type) -> String {
    let text = ty.to_token_stream().to_string();
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' {
            let prev = out.chars().last();
            let next = chars.get(i + 1).copied();
            if matches!(prev, Some('<' | '(' | '[' | '&' | ':'))
                || matches!(next, Some('<' | '>' | '(' | ')' | ']' | ',' | ';' | ':'))
            {
                continue;
            }
        }
        out.push(c);
    }
    out
}
