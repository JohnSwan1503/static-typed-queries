mod args;
mod expand;

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
