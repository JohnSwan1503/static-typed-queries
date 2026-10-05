use quote::ToTokens;
use syn::{Attribute, Ident, Type, parse_quote};

use crate::args::{Fetch, Step};
use crate::model::item::{Item, Role};
use crate::naming::type_key;
use crate::sql::analyze::Origin;

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

impl<'a> Item<'a> {
    pub(crate) fn item_docs(&self) -> Vec<syn::Attribute> {
        let mut lines = Vec::new();
        if self
            .input
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("doc"))
        {
            lines.push(String::new());
        }
        match self.role {
            Role::Query { sql } => {
                lines.push("# Template".to_owned());
                lines.push(String::new());
                lines.push("```sql".to_owned());
                lines.extend(template(&sql.value()));
                lines.push("```".to_owned());
            }
            Role::Statement { target } => {
                lines.push(format!("Runs {} as a statement.", self.link(target)))
            }
            Role::Transaction { steps } => lines.push(format!(
                "Runs {} in one transaction, in this order.",
                self.describe_steps(steps)
            )),
            Role::Table { before, after } => {
                let list = |hooks: &[Type]| {
                    let links: Vec<String> = hooks.iter().map(|ty| self.link(ty)).collect();
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
                    "Every statement that uses this table runs {runs}, each hook once. A hook with parameters takes its values from the statement builder's `with`."
                ));
            }
            Role::Wrapper => {}
        }
        if !self.is_empty() {
            lines.push(String::new());
            lines.push("# Parameters".to_owned());
            lines.push(String::new());
            lines.push(format!(
                "Set them through [`{}`], from `{}`:",
                self.builder,
                self.constructor()
            ));
            lines.push(String::new());
            for group in &self.groups {
                let times = match group.slots.len() {
                    1 => String::new(),
                    len => format!(" ×{len}"),
                };
                lines.push(format!(
                    "- `.{}({})`{times}: {}",
                    group.name,
                    type_string(&group.ty),
                    origins(&group.origins)
                ));
            }
            for child in &self.children {
                let separate = if type_key(&child.ty) == type_key(&child.named) {
                    ""
                } else {
                    ", with its own values for its type arguments"
                };
                lines.push(format!(
                    "- `.{}()`, then a setter: the parameters of {}{separate}",
                    child.name,
                    self.link(&child.named)
                ));
            }
            if !self.children.is_empty() {
                lines.push(String::new());
                lines.push("Items without parameters, such as tables, need no call.".to_owned());
            }
        }
        attrs(&lines)
    }

    pub(crate) fn describe_steps(&self, steps: &[Step]) -> String {
        let steps: Vec<String> = steps
            .iter()
            .map(|step| match step {
                Step::Run(ty, Fetch::Default) => self.link(ty),
                Step::Run(ty, Fetch::One) => format!("{} as one", self.link(ty)),
                Step::Run(ty, Fetch::Optional) => format!("{} as optional", self.link(ty)),
                Step::Run(ty, Fetch::Cte) => format!("{} as cte", self.link(ty)),
                Step::Savepoint(steps) => format!("savepoint({})", self.describe_steps(steps)),
            })
            .collect();
        steps.join(", ")
    }
}
