use syn::{LitStr, Type};

pub(crate) enum Segment {
    Lit(String),
    Ref(Box<Ref>),
}

pub(crate) struct Ref {
    pub ty: Type,
    pub target: bool,
}

pub(crate) struct Template {
    pub segments: Vec<Segment>,
}

enum Raw {
    Lit(String),
    Ref(Type),
}

pub(crate) fn parse(sql: &LitStr) -> syn::Result<Template> {
    let error = |message: String| syn::Error::new(sql.span(), message);
    let raw = scan(&sql.value()).map_err(error)?;

    let mut segments = Vec::new();
    for (i, item) in raw.iter().enumerate() {
        match item {
            Raw::Lit(text) => segments.push(Segment::Lit(text.clone())),
            Raw::Ref(ty) => {
                let previous = match i.checked_sub(1).map(|j| &raw[j]) {
                    Some(Raw::Lit(text)) => last_words(text),
                    _ => Vec::new(),
                };
                let target = matches!(
                    previous.as_slice(),
                    [.., "INTO" | "UPDATE" | "TABLE" | "TRUNCATE"] | [.., "DELETE", "FROM"]
                );
                segments.push(Segment::Ref(Box::new(Ref {
                    ty: ty.clone(),
                    target,
                })));
            }
        }
    }
    Ok(Template { segments })
}

fn scan(text: &str) -> Result<Vec<Raw>, String> {
    let chars: Vec<char> = text.chars().collect();
    let mut raw = Vec::new();
    let mut lit = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match (c, next) {
            ('\'' | '"', _) => {
                let mut end = i + 1;
                loop {
                    match chars.get(end) {
                        None => return Err(format!("unterminated {c}…{c} in the SQL template")),
                        Some(&q) if q == c && chars.get(end + 1) == Some(&c) => end += 2,
                        Some(&q) if q == c => break,
                        Some(_) => end += 1,
                    }
                }
                lit.extend(&chars[i..=end]);
                i = end + 1;
            }
            ('$', _) if dollar_tag(&chars, i).is_some() => {
                let tag = dollar_tag(&chars, i).expect("a dollar quote tag");
                let body = i + tag.len();
                let end = (body..chars.len())
                    .find(|&j| chars[j..].starts_with(&tag))
                    .ok_or("unterminated $$ string in the SQL template")?;
                lit.extend(&chars[i..end + tag.len()]);
                i = end + tag.len();
            }
            ('-', Some('-')) => {
                i = (i..chars.len())
                    .find(|&j| chars[j] == '\n')
                    .unwrap_or(chars.len());
            }
            (c, _) if c.is_whitespace() => {
                if !(lit.is_empty() && raw.is_empty()) && !lit.ends_with(' ') {
                    lit.push(' ');
                }
                i += 1;
            }
            ('/', Some('*')) => {
                let end = (i + 2..chars.len())
                    .find(|&j| chars[j] == '/' && chars[j - 1] == '*')
                    .ok_or("unterminated /* comment in the SQL template")?;
                lit.extend(&chars[i..=end]);
                i = end + 1;
            }
            ('{', Some('{')) | ('}', Some('}')) => {
                lit.push(c);
                i += 2;
            }
            ('{', _) => {
                let end = (i + 1..chars.len())
                    .find(|&j| chars[j] == '}')
                    .ok_or("unclosed `{` in the SQL template; write `{{` for a literal brace")?;
                let content: String = chars[i + 1..end].iter().collect();
                if !lit.is_empty() {
                    raw.push(Raw::Lit(std::mem::take(&mut lit)));
                }
                raw.push(placeholder(content.trim())?);
                i = end + 1;
            }
            ('}', _) => {
                return Err(
                    "unmatched `}` in the SQL template; write `}}` for a literal brace".into(),
                );
            }
            _ => {
                lit.push(c);
                i += 1;
            }
        }
    }
    lit.truncate(lit.trim_end().len());
    if !lit.is_empty() {
        raw.push(Raw::Lit(lit));
    }
    Ok(raw)
}

fn dollar_tag(chars: &[char], start: usize) -> Option<Vec<char>> {
    let mut end = start + 1;
    while let Some(&c) = chars.get(end) {
        if c == '$' {
            return Some(chars[start..=end].to_vec());
        }
        let valid = c == '_' || c.is_alphabetic() || (end > start + 1 && c.is_ascii_digit());
        if !valid {
            return None;
        }
        end += 1;
    }
    None
}

fn placeholder(content: &str) -> Result<Raw, String> {
    let ty = syn::parse_str::<Type>(content)
        .map_err(|_| format!("can't read placeholder `{{{content}}}`; expected `{{Item}}`"))?;
    Ok(Raw::Ref(ty))
}

fn last_words(text: &str) -> Vec<&'static str> {
    let words: Vec<String> = text
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
        .map(str::to_uppercase)
        .collect();
    let trailing = text
        .trim_end()
        .ends_with(|c: char| c.is_alphanumeric() || c == '_');
    if !trailing {
        return Vec::new();
    }
    words[words.len().saturating_sub(2)..]
        .iter()
        .map(|word| match word.as_str() {
            "INTO" => "INTO",
            "UPDATE" => "UPDATE",
            "TABLE" => "TABLE",
            "TRUNCATE" => "TRUNCATE",
            "DELETE" => "DELETE",
            "FROM" => "FROM",
            _ => "",
        })
        .collect()
}
