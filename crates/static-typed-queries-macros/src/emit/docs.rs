use quote::ToTokens;
use syn::{Attribute, Ident, ItemStruct, Type, parse_quote};

use crate::args::{Fetch, Step};
use crate::model::role::Role;

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

// A doc link to a type, unless it's a type parameter or a field named in place of a type.
pub(crate) fn link(ty: &Type, generics: &[&Ident]) -> String {
    let text = type_string(ty);
    let Type::Path(path) = ty else {
        return format!("`{text}`");
    };
    let first = path.path.segments.first();
    let generic = first.is_some_and(|segment| generics.contains(&&segment.ident));
    let field = path.path.get_ident().is_some_and(|ident| {
        ident
            .to_string()
            .trim_start_matches("r#")
            .starts_with(|c: char| c.is_lowercase())
    });
    if path.qself.is_some() || generic || field {
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

pub(crate) fn item_docs(input: &ItemStruct, role: Role) -> Vec<Attribute> {
    let generics: Vec<&Ident> = input
        .generics
        .type_params()
        .map(|param| &param.ident)
        .collect();
    let link = |ty: &Type| link(ty, &generics);
    let mut lines = Vec::new();
    if input.attrs.iter().any(|attr| attr.path().is_ident("doc")) {
        lines.push(String::new());
    }
    match role {
        Role::Query { sql } => {
            lines.push("# Template".to_owned());
            lines.push(String::new());
            lines.push("```sql".to_owned());
            lines.extend(template(&sql.value()));
            lines.push("```".to_owned());
        }
        Role::Statement { target } => lines.push(format!("Runs {} as a statement.", link(target))),
        Role::Transaction { steps } => lines.push(format!(
            "Runs {} in one transaction, in this order.",
            describe_steps(steps, &link)
        )),
        Role::Table { before, after } => {
            let list = |hooks: &[Type]| {
                let links: Vec<String> = hooks.iter().map(&link).collect();
                links.join(", ")
            };
            let runs = match (before.is_empty(), after.is_empty()) {
                (true, true) => return Vec::new(),
                (false, true) => format!("{} before it", list(before)),
                (true, false) => format!("{} after it", list(after)),
                (false, false) => {
                    format!("{} before it and {} after it", list(before), list(after))
                }
            };
            lines.push("# Hooks".to_owned());
            lines.push(String::new());
            lines.push(format!(
                "Every statement that uses this table runs {runs}, each hook once. A hook with values takes them from the statement's `with`."
            ));
        }
    }
    attrs(&lines)
}

fn describe_steps(steps: &[Step], link: &dyn Fn(&Type) -> String) -> String {
    let steps: Vec<String> = steps
        .iter()
        .map(|step| match step {
            Step::Run(ty, Fetch::Default) => link(ty),
            Step::Run(ty, Fetch::One) => format!("{} as one", link(ty)),
            Step::Run(ty, Fetch::Optional) => format!("{} as optional", link(ty)),
            Step::Run(ty, Fetch::Cte) => format!("{} as cte", link(ty)),
            Step::Savepoint(steps) => format!("savepoint({})", describe_steps(steps, link)),
        })
        .collect();
    steps.join(", ")
}
