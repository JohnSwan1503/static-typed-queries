use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, ItemStruct, LitBool, Type, parse_quote};

use crate::args::{self, Keys};
use crate::emit::bind::bind_impl;
use crate::emit::builder::builder;
use crate::emit::checks::{CteRule, cte_in, embeds, hook_needs, step_impl, values};
use crate::emit::docs::{self, item_docs};
use crate::emit::fmt::fmt;
use crate::emit::node::node;
use crate::emit::statement::statement;
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
        Some(_) => embeds(&input, &[ty], &[], &[], &dialect),
        None => embeds(&input, &[], std::slice::from_ref(ty), &[], &dialect),
    };
    let cte_in = cte_in(&input, CteRule::Like(ty));
    let node = node(
        &input,
        &snake_case(&ident.to_string()),
        "Scope",
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
    let (statement, methods, test) = statement(&input, &dialect, row, args.parse_check.as_ref());
    let (definitions, builder) = builder(&input, &fields, methods);
    let step = step_impl(&input, row);
    let fmt = fmt(args.display.as_ref(), args.debug.as_ref(), &input, true)?;
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
        #cte_in
        #hook_needs
        #bind
        #builder
        #statement
        #step
        #fmt
    });
    Ok(quote! {
        #documented
        #definitions
        #test
        #impls
    })
}
