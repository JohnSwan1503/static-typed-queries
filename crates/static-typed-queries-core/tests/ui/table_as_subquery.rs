#[macro_use]
#[path = "../support/mod.rs"]
mod support;

use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::ident::Ident;
use static_typed_queries_core::part::lit::Lit;

const USERS: &Node = node!("users", 1, Table, Inject::Ident, [Ident::part("users")]);

root!(Derived: Postgres = node!(
    "derived",
    2,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT * FROM "), From::part(USERS, AliasRule::NodeName, Some(Inject::Subquery))]
));

fn main() {}
