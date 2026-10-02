#[macro_use]
mod support;

use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::sql::Sql;
use static_typed_queries_core::{impl_debug, impl_display};

root!(Report: Postgres = node!(
    "report",
    1,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT 1")]
));
root!(Named: Postgres = node!(
    "named",
    2,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT 2")]
));

impl_display!(Report => sql);
impl_debug!(Report => tree);
impl_display!(Named => name);
impl_debug!(Named => sql);

#[test]
fn display_writes_sql_or_name() {
    assert_eq!(Report.to_string(), "SELECT 1");
    assert_eq!(Named.to_string(), "named");
}

#[test]
fn debug_writes_quoted_sql_or_node_tree() {
    assert_eq!(format!("{Named:?}"), r#""SELECT 2""#);
    assert_eq!(format!("{Report:?}"), format!("{:?}", Report::NODE));
    assert!(format!("{Report:?}").contains("kind: Query"));
}
