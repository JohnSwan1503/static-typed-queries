use std::fmt::Display;

use proc_macro2::Span;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Ident, ItemStruct, LitBool, LitStr, Path, Token, Type, parenthesized};

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

impl Sql {
    pub(crate) fn span(&self) -> Span {
        match self {
            Sql::Inline(sql) => sql.span(),
            Sql::Named(path) => path.span(),
        }
    }
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
}

pub(crate) enum Step {
    Run(Type, Fetch),
    Savepoint(Ident, Vec<Step>),
}

impl Step {
    pub(crate) fn span(&self) -> Span {
        match self {
            Step::Run(ty, _) => ty.span(),
            Step::Savepoint(keyword, _) => keyword.span(),
        }
    }

    pub(crate) fn flatten<'a>(steps: &'a [Step], out: &mut Vec<&'a Type>) {
        for step in steps {
            match step {
                Step::Run(ty, _) => out.push(ty),
                Step::Savepoint(_, steps) => Step::flatten(steps, out),
            }
        }
    }
}

impl Parse for Step {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Ident) && input.peek2(syn::token::Paren) {
            let keyword: Ident = input.fork().parse()?;
            if keyword == "savepoint" {
                input.parse::<Ident>()?;
                let content;
                parenthesized!(content in input);
                let steps: Vec<Step> = content
                    .parse_terminated(Step::parse, Token![,])?
                    .into_iter()
                    .collect();
                if steps.is_empty() {
                    return Err(syn::Error::new(
                        keyword.span(),
                        "a savepoint needs at least one step",
                    ));
                }
                return Ok(Step::Savepoint(keyword, steps));
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
            _ => {
                return Err(syn::Error::new(
                    fetch.span(),
                    "expected `one` or `optional`",
                ));
            }
        };
        Ok(Step::Run(ty, fetch))
    }
}

pub(crate) struct Args {
    pub dialect: Type,
    pub sql: Option<Sql>,
    pub sql_file: Option<LitStr>,
    pub name: Option<LitStr>,
    pub placement: Option<Placement>,
    pub display: Option<Ident>,
    pub debug: Option<Ident>,
    pub parse_check: Option<LitBool>,
    pub separate: Option<Vec<Type>>,
    pub grammar: Option<Ident>,
    pub row: Option<Type>,
    pub before: Option<Vec<Type>>,
    pub after: Option<Vec<Type>>,
    pub steps: Option<Vec<Step>>,
    to_add: ToAdd,
}

struct ToAdd(Vec<Option<&'static str>>, usize);

impl ToAdd {
    fn record_set(&mut self, key: &str) {
        let slots: &[usize] = match key {
            "cte" | "subquery" => &[0, 1],
            "sql" | "sql_file" => &[2, 3],
            "name" => &[4],
            "display" => &[5],
            "debug" => &[6],
            "parse_check" => &[7],
            "separate" => &[8],
            "grammar" => &[9],
            "row" => &[10],
            "before" => &[11],
            "after" => &[12],
            "steps" => &[13],
            _ => return,
        };
        for &idx in slots {
            if self.0[idx].take().is_some() {
                self.1 -= 1;
            }
        }
    }
}

impl Default for ToAdd {
    fn default() -> Self {
        ToAdd(
            vec![
                Some("cte"),
                Some("subquery"),
                Some("sql"),
                Some("sql_file"),
                Some("name"),
                Some("display"),
                Some("debug"),
                Some("parse_check"),
                Some("separate"),
                Some("grammar"),
                Some("row"),
                Some("before"),
                Some("after"),
                Some("steps"),
            ],
            14,
        )
    }
}

impl Display for ToAdd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn sep(remain: usize) -> &'static str {
            match remain {
                0 => "",
                1 => " or ",
                _ => ", ",
            }
        }
        f.write_str("expected ")?;
        if self.1 == 2 {
            f.write_str("either ")?;
        } else if self.1 > 2 {
            f.write_str("one of ")?;
        }
        let mut remain = self.1;
        self.0.iter().try_for_each(|name| {
            if let Some(name) = name {
                remain -= 1;
                write!(f, "`{}`{}", name, sep(remain))?;
            }
            Ok::<(), std::fmt::Error>(())
        })?;

        Ok(())
    }
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Args {
            dialect: input.parse()?,
            sql: None,
            sql_file: None,
            name: None,
            placement: None,
            display: None,
            debug: None,
            parse_check: None,
            separate: None,
            grammar: None,
            row: None,
            before: None,
            after: None,
            steps: None,
            to_add: ToAdd::default(),
        };
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let key: Ident = input.fork().parse()?;
            match key.to_string().as_str() {
                key_str @ "cte" | key_str @ "subquery" => set(
                    &mut args.placement,
                    Placement::parse_keyword(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "sql" => set(
                    &mut args.sql,
                    value(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "sql_file" => set(
                    &mut args.sql_file,
                    value(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "name" => set(
                    &mut args.name,
                    value(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "display" => set(
                    &mut args.display,
                    value(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "debug" => set(
                    &mut args.debug,
                    value(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "parse_check" => set(
                    &mut args.parse_check,
                    value(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "separate" => set(
                    &mut args.separate,
                    types(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "grammar" => set(
                    &mut args.grammar,
                    value(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "row" => set(
                    &mut args.row,
                    value(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "before" => set(
                    &mut args.before,
                    types(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "after" => set(
                    &mut args.after,
                    types(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                key_str @ "steps" => set(
                    &mut args.steps,
                    steps(input)?,
                    &key,
                    &mut args.to_add,
                    key_str,
                )?,
                _ => {
                    return Err(syn::Error::new(key.span(), args.to_add));
                }
            }
        }
        Ok(args)
    }
}

pub(crate) struct NamedQuery {
    pub sql: LitStr,
    pub args: Args,
    pub item: ItemStruct,
}

impl Parse for NamedQuery {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let sql = input.parse()?;
        let content;
        parenthesized!(content in input);
        Ok(NamedQuery {
            sql,
            args: content.parse()?,
            item: input.parse()?,
        })
    }
}

fn steps(input: ParseStream) -> syn::Result<Vec<Step>> {
    input.parse::<Ident>()?;
    let content;
    parenthesized!(content in input);
    Ok(content
        .parse_terminated(Step::parse, Token![,])?
        .into_iter()
        .collect())
}

fn types(input: ParseStream) -> syn::Result<Vec<Type>> {
    input.parse::<Ident>()?;
    let content;
    parenthesized!(content in input);
    Ok(content
        .parse_terminated(Type::parse, Token![,])?
        .into_iter()
        .collect())
}

fn value<T: Parse>(input: ParseStream) -> syn::Result<T> {
    input.parse::<Ident>()?;
    input.parse::<Token![=]>()?;
    input.parse()
}

fn set<T>(
    slot: &mut Option<T>,
    value: T,
    key: &Ident,
    to_add: &mut ToAdd,
    key_str: &str,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new(
            key.span(),
            format!("`{key}` is given more than once"),
        ));
    }
    to_add.record_set(key_str);
    *slot = Some(value);
    Ok(())
}
