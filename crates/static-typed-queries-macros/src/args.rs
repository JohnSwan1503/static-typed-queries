use std::fmt::Display;

use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Token, Type, parenthesized};

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

pub(crate) struct Args {
    pub dialect: Type,
    pub sql: Option<LitStr>,
    pub name: Option<LitStr>,
    pub placement: Option<Placement>,
    pub display: Option<Ident>,
    pub debug: Option<Ident>,
    to_add: ToAdd,
}

struct ToAdd(Vec<Option<&'static str>>, usize);

impl ToAdd {
    fn record_set(&mut self, key: &str) {
        let slots: &[usize] = match key {
            "cte" | "subquery" => &[0, 1],
            "sql" => &[2],
            "name" => &[3],
            "display" => &[4],
            "debug" => &[5],
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
                Some("name"),
                Some("display"),
                Some("debug"),
            ],
            6,
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

impl Args {}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Args {
            dialect: input.parse()?,
            sql: None,
            name: None,
            placement: None,
            display: None,
            debug: None,
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
                _ => {
                    return Err(syn::Error::new(key.span(), args.to_add));
                }
            }
        }
        Ok(args)
    }
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
