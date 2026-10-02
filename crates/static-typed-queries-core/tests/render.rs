#![cfg(test)]

#[macro_use]
mod support;

use core::marker::PhantomData;

use static_typed_queries_core::impl_statement;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::expr::Expr;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::ident::Ident;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::part::param::Param;
use static_typed_queries_core::sql::Sql;
use static_typed_queries_core::statement::Statement;

use static_typed_queries_core::dialect::{mysql::MySql, postgres::Postgres, sqlite::Sqlite};

struct Users;

impl Sql for Users {
    type Dialect = Postgres;
    type Params = ();
    const NODE: &'static Node = node!("users", 1, Table, Inject::Ident, [Ident::part("users")]);
}

struct ActiveUsers;

impl Sql for ActiveUsers {
    type Dialect = Postgres;
    type Params = ();
    const NODE: &'static Node = node!(
        "active_users",
        2,
        Query,
        Inject::Cte { recursive: false },
        [
            Lit::part("SELECT id, email FROM "),
            From::part(Users::NODE, AliasRule::NodeName, None),
            Lit::part(" WHERE deleted_at IS NULL AND org_id = "),
            Param::part(0),
        ]
    );
}

struct CountOf<T>(PhantomData<T>);

impl<T: Sql> Sql for CountOf<T> {
    type Dialect = T::Dialect;
    type Params = ();
    const NODE: &'static Node = node!(
        "count_of",
        3,
        Query,
        Inject::Subquery,
        [
            Lit::part("SELECT count(*) FROM "),
            From::part(T::NODE, AliasRule::NodeName, None),
        ]
    );
}

struct UserReport;

impl Sql for UserReport {
    type Dialect = Postgres;
    type Params = ();
    const NODE: &'static Node = node!(
        "user_report",
        4,
        Query,
        Inject::Subquery,
        [
            Lit::part("SELECT u.email, "),
            Expr::part(CountOf::<ActiveUsers>::NODE),
            Lit::part(" AS total FROM "),
            From::part(ActiveUsers::NODE, AliasRule::Given, None),
            Lit::part(" u WHERE u.id = "),
            Param::part(0),
        ]
    );
}

impl_statement!(UserReport, CountOf<ActiveUsers>);

root!(UserReportMySql: MySql = UserReport::NODE);
root!(UserReportSqlite: Sqlite = UserReport::NODE);

#[test]
fn composes_ctes_and_subqueries() {
    assert_eq!(
        UserReport::SQL,
        r#"WITH "active_users" AS (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $1), "active_users_2" AS (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $2) SELECT u.email, (SELECT count(*) FROM "active_users") AS total FROM "active_users_2" u WHERE u.id = $3"#
    );
}

#[test]
fn renders_in_the_root_dialect() {
    assert_eq!(
        UserReportMySql::SQL,
        "WITH `active_users` AS (SELECT id, email FROM `users` WHERE deleted_at IS NULL AND org_id = ?), `active_users_2` AS (SELECT id, email FROM `users` WHERE deleted_at IS NULL AND org_id = ?) SELECT u.email, (SELECT count(*) FROM `active_users`) AS total FROM `active_users_2` u WHERE u.id = ?"
    );
    assert_eq!(
        UserReportSqlite::SQL,
        r#"WITH "active_users" AS (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $1), "active_users_2" AS (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $2) SELECT u.email, (SELECT count(*) FROM "active_users") AS total FROM "active_users_2" u WHERE u.id = $3"#
    );
}

#[test]
fn generic_items_render_standalone() {
    assert_eq!(
        CountOf::<ActiveUsers>::SQL,
        r#"WITH "active_users" AS (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $1) SELECT count(*) FROM "active_users""#
    );
}

const BETWEEN: &Node = node!(
    "between",
    10,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT id FROM t WHERE a > "),
        Param::part(0),
        Lit::part(" AND b > "),
        Param::part(0)
    ]
);

const TWICE: &Node = node!(
    "twice",
    11,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT * FROM t WHERE id IN "),
        Expr::part(BETWEEN),
        Lit::part(" OR id IN "),
        Expr::part(BETWEEN),
        Lit::part(" LIMIT "),
        Param::part(0),
    ]
);

root!(TwicePostgres: Postgres = TWICE);
root!(TwiceMySql: MySql = TWICE);
root!(TwiceSqlite: Sqlite = TWICE);

#[test]
fn references_within_one_template_share_params() {
    assert_eq!(
        TwicePostgres::SQL,
        "SELECT * FROM t WHERE id IN (SELECT id FROM t WHERE a > $1 AND b > $1) OR id IN (SELECT id FROM t WHERE a > $1 AND b > $1) LIMIT $2"
    );
    assert_eq!(
        TwiceSqlite::SQL,
        "SELECT * FROM t WHERE id IN (SELECT id FROM t WHERE a > $1 AND b > $1) OR id IN (SELECT id FROM t WHERE a > $1 AND b > $1) LIMIT $2"
    );
    assert_eq!(
        TwiceMySql::SQL,
        "SELECT * FROM t WHERE id IN (SELECT id FROM t WHERE a > ? AND b > ?) OR id IN (SELECT id FROM t WHERE a > ? AND b > ?) LIMIT ?"
    );
}

root!(Inlined: Postgres = node!(
    "inlined",
    20,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT * FROM "),
        From::part(ActiveUsers::NODE, AliasRule::NodeName, Some(Inject::Subquery)),
        Lit::part(" JOIN "),
        From::part(ActiveUsers::NODE, AliasRule::Given, Some(Inject::Subquery)),
        Lit::part(" a USING (id)"),
    ]
));

#[test]
fn subquery_placement_and_alias_rules() {
    assert_eq!(
        Inlined::SQL,
        r#"SELECT * FROM (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $1) AS "active_users" JOIN (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $1) a USING (id)"#
    );
}

const VIPS: &Node = node!(
    "vips",
    30,
    Query,
    Inject::Cte { recursive: false },
    [
        Lit::part("SELECT id FROM "),
        From::part(ActiveUsers::NODE, AliasRule::NodeName, None),
        Lit::part(" WHERE vip")
    ]
);

root!(VipCount: Postgres = node!(
    "vip_count",
    31,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT count(*) FROM "),
        From::part(VIPS, AliasRule::NodeName, None),
        Lit::part(" JOIN "),
        From::part(ActiveUsers::NODE, AliasRule::Given, None),
        Lit::part(" a USING (id)"),
    ]
));

#[test]
fn hoists_cte_dependencies_first() {
    assert_eq!(
        VipCount::SQL,
        r#"WITH "active_users" AS (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $1), "vips" AS (SELECT id FROM "active_users" WHERE vip), "active_users_2" AS (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $2) SELECT count(*) FROM "vips" JOIN "active_users_2" a USING (id)"#
    );
}

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
