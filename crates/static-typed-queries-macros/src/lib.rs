mod analyze;
mod args;
mod expand;
mod naming;
mod template;

use proc_macro::TokenStream;
use syn::{ItemStruct, parse_macro_input};

/// Declares a database table.
#[proc_macro_attribute]
pub fn table(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as args::Args);
    let item = parse_macro_input!(item as ItemStruct);
    expand::table(args, item)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Turns a struct into a SQL statement that is checked and rendered at
/// compile time.
#[proc_macro_attribute]
pub fn query(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as args::Args);
    let item = parse_macro_input!(item as ItemStruct);
    expand::query(args, item)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}
