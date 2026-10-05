#[macro_use]
#[path = "../support/mod.rs"]
mod support;

use static_typed_queries_core::dialect::mysql::MySql;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::part::target::Target;

const ARCHIVED: &Node = node!(
    "archived",
    1,
    Dml,
    Inject::Cte { recursive: false },
    [Lit::part("DELETE FROM users RETURNING id")]
);

root!(ArchiveCount: MySql = node!(
    "archive_count",
    2,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT count(*) FROM "), From::part(Target::Item(0), AliasRule::NodeName, None)],
    items = [ARCHIVED]
));

fn main() {}
