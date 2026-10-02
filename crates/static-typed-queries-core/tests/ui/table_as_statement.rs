#[macro_use]
#[path = "../support/mod.rs"]
mod support;

use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::ident::Ident;

const USERS: &Node = node!("users", 1, Table, Inject::Ident, [Ident::part("users")]);

root!(UsersStatement: Postgres = USERS);

fn main() {}
