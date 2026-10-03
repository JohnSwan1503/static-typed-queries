#[macro_use]
#[path = "../support/mod.rs"]
mod support;

use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::expr::Expr;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::part::param::Param;

const TREE: &Node = node!(
    "tree",
    1,
    Query,
    Inject::cte(true),
    [
        Lit::part("SELECT id FROM nodes WHERE id = "),
        Param::part(0, "root", "i64"),
        Lit::part(" UNION ALL SELECT n.id FROM nodes n JOIN tree t ON n.parent_id = t.id"),
    ]
);

const SUBTREE_SIZE: &Node = node!(
    "subtree_size",
    2,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT count(*) FROM "),
        From::part(TREE, AliasRule::NodeName, None),
    ],
    items = [TREE]
);

root!(Both: Postgres = node!(
    "both",
    3,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT "),
        Expr::part(SUBTREE_SIZE),
        Lit::part(", id FROM "),
        From::part(TREE, AliasRule::NodeName, None),
    ],
    items = [SUBTREE_SIZE, TREE]
));

fn main() {}
