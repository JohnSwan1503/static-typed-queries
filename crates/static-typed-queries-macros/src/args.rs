use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Ident, LitStr, Path, Token, Type, parenthesized};

#[derive(Clone, Copy)]
pub(crate) enum Placement {
    Cte { recursive: bool },
    Subquery,
}

impl Placement {
    pub(crate) fn parse_keyword(input: ParseStream) -> syn::Result<Placement> {
        let keyword: Ident = input.parse()?;
        if keyword == "subquery" {
            return Ok(Placement::Subquery);
        }
        if keyword != "cte" {
            return Err(syn::Error::new(
                keyword.span(),
                "expected `cte`, `cte(recursive)` or `subquery`",
            ));
        }
        if !input.peek(syn::token::Paren) {
            return Ok(Placement::Cte { recursive: false });
        }
        let content;
        parenthesized!(content in input);
        let flag: Ident = content.parse()?;
        if flag != "recursive" {
            return Err(syn::Error::new(flag.span(), "expected `recursive`"));
        }
        Ok(Placement::Cte { recursive: true })
    }
}

pub(crate) enum Sql {
    Inline(LitStr),
    Named(Path),
}

impl Parse for Sql {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(LitStr) {
            Ok(Sql::Inline(input.parse()?))
        } else {
            Ok(Sql::Named(input.parse()?))
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Fetch {
    Default,
    One,
    Optional,
    Cte,
}

pub(crate) enum Step {
    Run(Type, Fetch),
    Savepoint(Vec<Step>),
}

impl Step {
    pub(crate) fn flatten<'a>(steps: &'a [Step], out: &mut Vec<(&'a Type, Fetch)>) {
        for step in steps {
            match step {
                Step::Run(ty, fetch) => out.push((ty, *fetch)),
                Step::Savepoint(steps) => Step::flatten(steps, out),
            }
        }
    }

    pub(crate) fn check_ctes(steps: &[Step]) -> syn::Result<()> {
        for (i, step) in steps.iter().enumerate() {
            match (step, steps.get(i + 1)) {
                (Step::Run(ty, Fetch::Cte), None) => {
                    return Err(syn::Error::new(
                        ty.span(),
                        "`as cte` attaches this step to the next one, so another step must follow it",
                    ));
                }
                (Step::Run(ty, Fetch::Cte), Some(Step::Savepoint(_))) => {
                    return Err(syn::Error::new(
                        ty.span(),
                        "`as cte` attaches this step to the next one, which can't be a savepoint",
                    ));
                }
                (Step::Savepoint(steps), _) => Step::check_ctes(steps)?,
                _ => {}
            }
        }
        Ok(())
    }
}

impl Parse for Step {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Ident) && input.peek2(syn::token::Paren) {
            let keyword: Ident = input.fork().parse()?;
            if keyword == "savepoint" {
                let steps: Vec<Step> = list(input)?;
                if steps.is_empty() {
                    return Err(syn::Error::new(
                        keyword.span(),
                        "a savepoint needs at least one step",
                    ));
                }
                return Ok(Step::Savepoint(steps));
            }
        }
        let ty = input.parse()?;
        if input.parse::<Option<Token![as]>>()?.is_none() {
            return Ok(Step::Run(ty, Fetch::Default));
        }
        let fetch: Ident = input.parse()?;
        let fetch = match fetch.to_string().as_str() {
            "one" => Fetch::One,
            "optional" => Fetch::Optional,
            "cte" => Fetch::Cte,
            _ => {
                return Err(syn::Error::new(
                    fetch.span(),
                    "expected `one`, `optional` or `cte`",
                ));
            }
        };
        Ok(Step::Run(ty, fetch))
    }
}

pub(crate) type Keys = &'static [&'static [&'static str]];

// `take` parses one `key = value` and returns false for keys the attribute doesn't know.
pub(crate) fn parse_keys(
    input: ParseStream,
    keys: Keys,
    mut take: impl FnMut(&Ident, ParseStream) -> syn::Result<bool>,
) -> syn::Result<()> {
    let mut given = vec![false; keys.len()];
    while !input.is_empty() {
        input.parse::<Token![,]>()?;
        if input.is_empty() {
            break;
        }
        let key: Ident = input.fork().parse()?;
        let group = keys
            .iter()
            .position(|group| group.iter().any(|name| key == name));
        if let Some(group) = group
            && given[group]
        {
            return Err(syn::Error::new(
                key.span(),
                format!("`{key}` is given more than once"),
            ));
        }
        if !take(&key, input)? {
            return Err(syn::Error::new(key.span(), expected(keys, &given)));
        }
        if let Some(group) = group {
            given[group] = true;
        }
    }
    Ok(())
}

fn expected(keys: Keys, given: &[bool]) -> String {
    let left: Vec<String> = keys
        .iter()
        .zip(given)
        .filter(|(_, given)| !**given)
        .flat_map(|(group, _)| group.iter().map(|key| format!("`{key}`")))
        .collect();
    match left.as_slice() {
        [] => "expected no more arguments".to_owned(),
        [key] => format!("expected {key}"),
        [first, second] => format!("expected either {first} or {second}"),
        [rest @ .., last] => format!("expected one of {} or {last}", rest.join(", ")),
    }
}

pub(crate) fn only_tables(key: &Ident) -> syn::Error {
    syn::Error::new(key.span(), "only tables take `before` and `after`")
}

pub(crate) fn only_transactions(key: &Ident) -> syn::Error {
    syn::Error::new(key.span(), "only transactions take `steps`")
}

// Parses `name(a, b, …)`.
pub(crate) fn list<T: Parse>(input: ParseStream) -> syn::Result<Vec<T>> {
    input.parse::<Ident>()?;
    let content;
    parenthesized!(content in input);
    Ok(content
        .parse_terminated(T::parse, Token![,])?
        .into_iter()
        .collect())
}

pub(crate) fn value<T: Parse>(input: ParseStream) -> syn::Result<T> {
    input.parse::<Ident>()?;
    input.parse::<Token![=]>()?;
    input.parse()
}
