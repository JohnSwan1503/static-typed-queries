use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, ItemStruct, LitBool, Type, parse_quote};

use crate::args::{self, Keys};
use crate::emit::bind::bind_impl;
use crate::emit::checks::{embeds, hook_needs, parse_test, step_impl, values};
use crate::emit::docs::{self, item_docs};
use crate::emit::fmt::fmt;
use crate::emit::node::{fingerprint, node};
use crate::emit::run::statement_methods;
use crate::emit::{self, krate};
use crate::model::fields;
use crate::model::role::Role;
use crate::naming::snake_case;

pub(crate) struct StatementArgs {
    target: Type,
    display: Option<Ident>,
    debug: Option<Ident>,
    row: Option<Type>,
    parse_check: Option<LitBool>,
}

const KEYS: Keys = &[&["display"], &["debug"], &["row"], &["parse_check"]];

impl Parse for StatementArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = StatementArgs {
            target: input.parse()?,
            display: None,
            debug: None,
            row: None,
            parse_check: None,
        };
        let target = docs::type_string(&args.target);
        args::parse_keys(input, KEYS, |key, input| {
            match key.to_string().as_str() {
                "display" => args.display = Some(args::value(input)?),
                "debug" => args.debug = Some(args::value(input)?),
                "row" => args.row = Some(args::value(input)?),
                "parse_check" => args.parse_check = Some(args::value(input)?),
                "sql" | "sql_file" | "name" | "cte" | "subquery" | "grammar" => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "statements take their SQL from `{target}`, so they don't take `{key}`"
                        ),
                    ));
                }
                "before" | "after" => return Err(args::only_tables(key)),
                "steps" => return Err(args::only_transactions(key)),
                _ => return Ok(false),
            }
            Ok(true)
        })?;
        Ok(args)
    }
}

pub(crate) fn expand(args: StatementArgs, mut input: ItemStruct) -> syn::Result<TokenStream> {
    let krate = krate();
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "statements can't be generic; name the instantiation instead",
        ));
    }
    let fields = fields::items(&mut input)?;
    let target = &args.target;
    let field = fields::resolve(target, &fields);
    if let Some(other) = fields
        .iter()
        .find(|other| field.is_none_or(|(_, field)| !std::ptr::eq(field, *other)))
    {
        return Err(syn::Error::new(
            other.ident.span(),
            format!(
                "a statement holds only the values of what it runs; name this field in the attribute, as `#[statement({})]`",
                other.ident
            ),
        ));
    }
    let ty: &Type = field.map_or(target, |(_, field)| &field.ty);
    let ident = &input.ident;
    let dialect: Type = parse_quote!(<#ty as #krate::Sql>::Dialect);
    let embeds = match field {
        Some(_) => embeds(&input, &[ty], &[], &dialect),
        None => embeds(&input, &[], std::slice::from_ref(ty), &dialect),
    };
    let node = node(
        &snake_case(&ident.to_string()),
        fingerprint(ident, &input),
        quote!(Scope),
        quote!(#krate::Inject::Subquery),
        Vec::new(),
        &[ty],
        [&[], &[]],
        embeds.node,
    );
    let checked = embeds.checked;
    let hook_needs = hook_needs(
        &input,
        1,
        |_, index| parse_quote!(#ty: #krate::HookNeeds<__V, #index>),
    );
    let values = values(&input, !fields.is_empty());
    let bind = bind_impl(&input, &fields);
    let row = args.row.as_ref();
    let rows = row.map(|row| {
        quote! {
            impl #krate::Rows for #ident {
                type Row = #row;
            }
        }
    });
    let methods = statement_methods(&input, &dialect, row);
    let step = step_impl(&input, row);
    let fmt = fmt(args.display.as_ref(), args.debug.as_ref(), &input, true)?;
    let parse_check = args.parse_check.as_ref().is_none_or(|check| check.value);
    let test = parse_check.then(|| parse_test(ident));
    let mut documented = input.clone();
    documented
        .attrs
        .extend(item_docs(&input, Role::Statement { target }));
    let impls = emit::scoped(quote! {
        impl #krate::Sql for #ident {
            type Dialect = #dialect;
            const NODE: &'static #krate::Node = #node;
        }

        #values
        #checked
        #hook_needs
        #bind

        #krate::impl_statement!(#ident);
        #rows
        #methods
        #step

        impl #ident {
            /// The SQL, rendered at compile time.
            pub const SQL: &'static str = <Self as #krate::Statement>::SQL;
        }

        #fmt
    });
    Ok(quote! {
        #documented
        #test
        #impls
    })
}
