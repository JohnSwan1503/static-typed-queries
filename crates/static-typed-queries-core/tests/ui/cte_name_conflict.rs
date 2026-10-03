#[macro_use]
#[path = "../support/mod.rs"]
mod support;

use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::lit::Lit;

const FIRST: &Node = node!(
    "recent",
    1,
    Query,
    Inject::cte(false),
    [Lit::part("SELECT 1")]
);
const SECOND: &Node = node!(
    "recent",
    2,
    Query,
    Inject::cte(true),
    [Lit::part("SELECT 2")]
);

root!(Both: Postgres = node!(
    "both",
    3,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT * FROM "),
        From::part(FIRST, AliasRule::NodeName, None),
        Lit::part(", "),
        From::part(SECOND, AliasRule::NodeName, None),
    ],
    items = [FIRST, SECOND]
));

fn main() {}
