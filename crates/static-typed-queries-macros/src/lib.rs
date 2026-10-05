mod args;
mod attr;
mod emit;
mod model;
mod naming;
mod sql;

use proc_macro::TokenStream;
use syn::{ItemConst, ItemStruct, parse_macro_input};

#[doc = include_str!("../docs/table.md")]
#[proc_macro_attribute]
pub fn table(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as attr::table::TableArgs);
    let item = parse_macro_input!(item as ItemStruct);
    attr::table::expand(args, item)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

#[doc = include_str!("../docs/query.md")]
#[proc_macro_attribute]
pub fn query(args: TokenStream, item: TokenStream) -> TokenStream {
    let tokens = proc_macro2::TokenStream::from(args.clone());
    let args = parse_macro_input!(args as attr::query::QueryArgs);
    let item = parse_macro_input!(item as ItemStruct);
    if let Some(args::Sql::Named(name)) = &args.sql {
        return attr::query::named(name, tokens, &item).into();
    }
    attr::query::expand(args, item)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

#[doc(hidden)]
#[proc_macro]
pub fn __query_sql(input: TokenStream) -> TokenStream {
    let named = parse_macro_input!(input as attr::query::NamedQuery);
    attr::query::named_query(named)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

#[doc = include_str!("../docs/statement.md")]
#[proc_macro_attribute]
pub fn statement(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as attr::statement::StatementArgs);
    let item = parse_macro_input!(item as ItemStruct);
    attr::statement::expand(args, item)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

#[doc = include_str!("../docs/sql.md")]
#[proc_macro_attribute]
pub fn sql(args: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemConst);
    attr::sql::expand(args.into(), item)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

#[doc = include_str!("../docs/transaction.md")]
#[proc_macro_attribute]
pub fn transaction(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as attr::transaction::TransactionArgs);
    let item = parse_macro_input!(item as ItemStruct);
    attr::transaction::expand(args, item)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}
