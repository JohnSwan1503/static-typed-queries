use std::fmt::Display;

use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Token, Type};

pub(crate) struct Args {
    pub dialect: Type,
    pub name: Option<LitStr>,
    to_add: ToAdd,
}

struct ToAdd(Vec<Option<&'static str>>, usize);

impl ToAdd {
    fn record_set(&mut self, key: &str) {
        let slots: &[usize] = match key {
            "name" => &[0],
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
        ToAdd(vec![Some("name")], 1)
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
            name: None,
            to_add: ToAdd::default(),
        };
        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            if input.is_empty() {
                break;
            }
            let key: Ident = input.fork().parse()?;
            match key.to_string().as_str() {
                key_str @ "name" => set(
                    &mut args.name,
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
