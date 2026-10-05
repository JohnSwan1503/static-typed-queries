use std::ops::Range;

use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Token, Type};

use crate::args::Placement;
use crate::model::fields::{self, Field, Kind};
use crate::naming::push_unique;
use crate::sql::excerpt::point;

pub(crate) enum Segment {
    Lit(String),
    Param(u16),
    Ref(Box<Ref>),
}

// An embedded item: one of the item fields, by index, or a type that holds no values.
pub(crate) struct Ref {
    pub ty: Type,
    pub item: Option<u16>,
    pub placement: Option<Placement>,
    pub target: bool,
}

// `origins` follow `segments`: where in `source` each one was written, by char.
pub(crate) struct Template {
    pub segments: Vec<Segment>,
    pub types: Vec<Type>,
    pub source: String,
    pub origins: Vec<Origin>,
}

pub(crate) enum Origin {
    Lit(Vec<usize>),
    Placeholder(Range<usize>),
}

enum Raw {
    Lit(String, Vec<usize>),
    Decl(Option<Ident>, Type),
    Name(Ident),
    Ref(RefSyntax),
}

impl Raw {
    fn origin(&self, at: &Range<usize>) -> Origin {
        match self {
            Raw::Lit(_, chars) => Origin::Lit(chars.clone()),
            _ => Origin::Placeholder(at.clone()),
        }
    }
}

struct AnyIdent(Ident);

impl Parse for AnyIdent {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(AnyIdent(input.call(Ident::parse_any)?))
    }
}

struct ParamDeclaration {
    name: Option<Ident>,
    ty: Type,
}

impl Parse for ParamDeclaration {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = if input.parse::<Option<Token![_]>>()?.is_some() {
            None
        } else {
            Some(input.parse::<Ident>()?)
        };
        if input.peek(Token![::]) {
            return Err(input.error("not a parameter declaration"));
        }
        input.parse::<Token![:]>()?;
        Ok(ParamDeclaration {
            name,
            ty: input.parse()?,
        })
    }
}

struct RefSyntax {
    ty: Type,
    placement: Option<Placement>,
}

impl Parse for RefSyntax {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ty = input.parse()?;
        let placement = if input.parse::<Option<Token![as]>>()?.is_some() {
            Some(Placement::parse_keyword(input)?)
        } else {
            None
        };
        Ok(RefSyntax { ty, placement })
    }
}

// `{name}` is the field `name`: a value to bind, or an item to embed where the field is marked.
// Every field must be used. Anything else names a type that holds no values.
pub(crate) fn parse(sql: &LitStr, fields: &[Field]) -> syn::Result<Template> {
    let source = sql.value();
    let error = |message: String, at: &Range<usize>| {
        syn::Error::new(
            sql.span(),
            format!("{message}{}", point(&source, at.clone())),
        )
    };
    let pieces = scan(&source).map_err(|(message, at)| error(message, &at))?;
    let raw: Vec<&Raw> = pieces.iter().map(|(raw, _)| raw).collect();

    let mut segments = Vec::new();
    let mut origins = Vec::new();
    let mut used = vec![false; fields.len()];
    let mut types: Vec<Type> = Vec::new();
    for (i, (item, at)) in pieces.iter().enumerate() {
        origins.push(item.origin(at));
        match item {
            Raw::Lit(text, _) => segments.push(Segment::Lit(text.clone())),
            Raw::Decl(name, ty) => {
                let (name, ty) = (
                    name.as_ref().map_or("name".to_owned(), ToString::to_string),
                    crate::emit::docs::type_string(ty),
                );
                return Err(error(
                    format!(
                        "parameters are the struct's fields; add `pub {name}: {ty}` and write `{{{name}}}`"
                    ),
                    at,
                ));
            }
            Raw::Name(name) if let Some(index) = fields::find(fields, name) => {
                used[index] = true;
                let field = &fields[index];
                let slot = fields::slot(fields, index);
                segments.push(match field.kind {
                    Kind::Value => Segment::Param(slot),
                    Kind::Item(placement) => {
                        reference(&raw, i, field.ty.clone(), Some(slot), placement)
                    }
                });
            }
            Raw::Name(name) if name.to_string().starts_with(|c: char| c.is_lowercase()) => {
                return Err(error(
                    format!("there is no field named `{name}`; parameters are the struct's fields"),
                    at,
                ));
            }
            Raw::Name(name) => {
                let ty = syn::parse_quote!(#name);
                push_unique(&mut types, &ty);
                segments.push(reference(&raw, i, ty, None, None));
            }
            Raw::Ref(syntax) => {
                if let Type::Path(path) = &syntax.ty
                    && let Some(name) = path.path.get_ident()
                    && fields::find(fields, name).is_some()
                {
                    return Err(error(
                        format!(
                            "`{name}` is a field, so it is embedded the way the field is marked; write `{{{name}}}`"
                        ),
                        at,
                    ));
                }
                push_unique(&mut types, &syntax.ty);
                segments.push(reference(
                    &raw,
                    i,
                    syntax.ty.clone(),
                    None,
                    syntax.placement,
                ));
            }
        }
    }
    if let Some((field, _)) = fields.iter().zip(&used).find(|(_, used)| !**used) {
        return Err(syn::Error::new(
            field.ident.span(),
            format!(
                "the field `{}` isn't used in the template; write `{{{}}}` where it goes, or remove it",
                field.ident, field.ident
            ),
        ));
    }
    Ok(Template {
        segments,
        types,
        source,
        origins,
    })
}

fn reference(
    raw: &[&Raw],
    i: usize,
    ty: Type,
    item: Option<u16>,
    placement: Option<Placement>,
) -> Segment {
    let previous = match i.checked_sub(1).map(|j| raw[j]) {
        Some(Raw::Lit(text, _)) => last_words(text),
        _ => Vec::new(),
    };
    let target = matches!(
        previous.as_slice(),
        [.., "INTO" | "UPDATE" | "TABLE" | "TRUNCATE"] | [.., "DELETE", "FROM"]
    );
    Segment::Ref(Box::new(Ref {
        ty,
        item,
        placement,
        target,
    }))
}

// Each piece with the chars it was read from. A literal also keeps where each of its own chars
// came from, since whitespace is collapsed and `--` comments are dropped.
fn scan(text: &str) -> Result<Vec<(Raw, Range<usize>)>, (String, Range<usize>)> {
    let chars: Vec<char> = text.chars().collect();
    let mut raw = Vec::new();
    let mut lit = String::new();
    let mut from: Vec<usize> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match (c, next) {
            ('\'' | '"', _) => {
                let mut end = i + 1;
                loop {
                    match chars.get(end) {
                        None => {
                            return Err((
                                format!("unterminated {c}…{c} in the SQL template"),
                                i..i + 1,
                            ));
                        }
                        Some(&q) if q == c && chars.get(end + 1) == Some(&c) => end += 2,
                        Some(&q) if q == c => break,
                        Some(_) => end += 1,
                    }
                }
                lit.extend(&chars[i..=end]);
                from.extend(i..=end);
                i = end + 1;
            }
            ('$', _) if dollar_tag(&chars, i).is_some() => {
                let tag = dollar_tag(&chars, i).expect("a dollar quote tag");
                let body = i + tag.len();
                let end = (body..chars.len())
                    .find(|&j| chars[j..].starts_with(&tag))
                    .ok_or(("unterminated $$ string in the SQL template".into(), i..body))?;
                lit.extend(&chars[i..end + tag.len()]);
                from.extend(i..end + tag.len());
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
                    from.push(i);
                }
                i += 1;
            }
            ('/', Some('*')) => {
                let end = (i + 2..chars.len())
                    .find(|&j| chars[j] == '/' && chars[j - 1] == '*')
                    .ok_or((
                        "unterminated /* comment in the SQL template".into(),
                        i..i + 2,
                    ))?;
                lit.extend(&chars[i..=end]);
                from.extend(i..=end);
                i = end + 1;
            }
            ('{', Some('{')) | ('}', Some('}')) => {
                lit.push(c);
                from.push(i);
                i += 2;
            }
            ('{', _) => {
                let end = (i + 1..chars.len()).find(|&j| chars[j] == '}').ok_or((
                    "unclosed `{` in the SQL template; write `{{` for a literal brace".into(),
                    i..i + 1,
                ))?;
                let content: String = chars[i + 1..end].iter().collect();
                if !lit.is_empty() {
                    let start = from[0];
                    let text = std::mem::take(&mut lit);
                    raw.push((Raw::Lit(text, std::mem::take(&mut from)), start..i));
                }
                let at = i..end + 1;
                let placeholder =
                    placeholder(content.trim()).map_err(|message| (message, at.clone()))?;
                raw.push((placeholder, at));
                i = end + 1;
            }
            ('}', _) => {
                return Err((
                    "unmatched `}` in the SQL template; write `}}` for a literal brace".into(),
                    i..i + 1,
                ));
            }
            _ => {
                lit.push(c);
                from.push(i);
                i += 1;
            }
        }
    }
    let len = lit.trim_end().len();
    lit.truncate(len);
    from.truncate(lit.chars().count());
    if !lit.is_empty() {
        let at = from[0]..from[from.len() - 1] + 1;
        raw.push((Raw::Lit(lit, from), at));
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
    if let Ok(decl) = syn::parse_str::<ParamDeclaration>(content) {
        return Ok(Raw::Decl(decl.name, decl.ty));
    }
    if let Ok(AnyIdent(name)) = syn::parse_str::<AnyIdent>(content) {
        return Ok(Raw::Name(name));
    }
    let syntax = syn::parse_str::<RefSyntax>(content).map_err(|_| {
        format!("can't read placeholder `{{{content}}}`; expected `{{field}}` or `{{Type}}`")
    })?;
    Ok(Raw::Ref(syntax))
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
