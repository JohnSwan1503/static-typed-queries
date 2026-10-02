#![cfg(test)]

#[macro_use]
mod support;

use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::ident::Ident;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::statement::Statement;

use static_typed_queries_core::dialect::{mysql::MySql, postgres::Postgres};

root!(QuotedPostgres: Postgres = node!(
    "quoted",
    60,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT "), Ident::part("we\"ird"), Lit::part(", "), Ident::part("we`ird")]
));
root!(QuotedMySql: MySql = QuotedPostgres::NODE);

#[test]
fn escapes_quotes_inside_identifiers() {
    assert_eq!(QuotedPostgres::SQL, r#"SELECT "we""ird", "we`ird""#);
    assert_eq!(QuotedMySql::SQL, r#"SELECT `we"ird`, `we``ird`"#);
}
