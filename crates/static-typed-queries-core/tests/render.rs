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
use static_typed_queries_core::part::target::Target;
use static_typed_queries_core::sql::Sql;
use static_typed_queries_core::statement::Statement;

use static_typed_queries_core::dialect::{mysql::MySql, postgres::Postgres, sqlite::Sqlite};

struct Users;

impl Sql for Users {
    type Dialect = Postgres;
    const NODE: &'static Node = node!("users", 1, Table, Inject::Ident, [Ident::part("users")]);
}

struct ActiveUsers;

impl Sql for ActiveUsers {
    type Dialect = Postgres;
    const NODE: &'static Node = node!(
        "active_users",
        2,
        Query,
        Inject::Cte { recursive: false },
        [
            Lit::part("SELECT id, email FROM "),
            From::part(Target::Item(0), AliasRule::NodeName, None),
            Lit::part(" WHERE deleted_at IS NULL AND org_id = "),
            Param::part(0, "org_id", "i64"),
        ],
        items = [Users::NODE]
    );
}

// A generic item lists its argument as an item of its own, so the argument's values nest under it.
struct CountOf<T>(PhantomData<T>);

impl<T: Sql> Sql for CountOf<T> {
    type Dialect = T::Dialect;
    const NODE: &'static Node = node!(
        "count_of",
        3,
        Query,
        Inject::Subquery,
        [
            Lit::part("SELECT count(*) FROM "),
            From::part(Target::Item(0), AliasRule::NodeName, None),
        ],
        items = [T::NODE]
    );
}

struct UserReport;

impl Sql for UserReport {
    type Dialect = Postgres;
    const NODE: &'static Node = node!(
        "user_report",
        4,
        Query,
        Inject::Subquery,
        [
            Lit::part("SELECT u.email, "),
            Expr::part(Target::Item(0)),
            Lit::part(" AS total FROM "),
            From::part(Target::Item(1), AliasRule::Given, None),
            Lit::part(" u WHERE u.id = "),
            Param::part(0, "id", "i64"),
        ],
        items = [CountOf::<ActiveUsers>::NODE, ActiveUsers::NODE]
    );
}

impl_statement!(UserReport, CountOf<Users>);

root!(UserReportMySql: MySql = UserReport::NODE);
root!(UserReportSqlite: Sqlite = UserReport::NODE);

#[test]
fn each_item_is_its_own_instance() {
    assert_eq!(
        UserReport::SQL,
        r#"WITH "active_users" AS (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $1), "active_users_2" AS (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $2) SELECT u.email, (SELECT count(*) FROM "active_users") AS total FROM "active_users_2" u WHERE u.id = $3"#
    );
    let paths: Vec<Vec<u16>> = UserReport::BINDS
        .iter()
        .map(|bind| bind.path().steps().to_vec())
        .collect();
    assert_eq!(paths, [vec![0, 0], vec![1], vec![]]);
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
    assert_eq!(CountOf::<Users>::SQL, r#"SELECT count(*) FROM "users""#);
}

const BETWEEN: &Node = node!(
    "between",
    10,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT id FROM t WHERE a > "),
        Param::part(0, "min", "i64"),
        Lit::part(" AND b > "),
        Param::part(0, "min", "i64")
    ]
);

const TWICE: &Node = node!(
    "twice",
    11,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT * FROM t WHERE id IN "),
        Expr::part(Target::Item(0)),
        Lit::part(" OR id IN "),
        Expr::part(Target::Item(0)),
        Lit::part(" LIMIT "),
        Param::part(0, "limit", "i64"),
    ],
    items = [BETWEEN]
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
        From::part(Target::Item(0), AliasRule::NodeName, Some(Inject::Subquery)),
        Lit::part(" JOIN "),
        From::part(Target::Item(0), AliasRule::Given, Some(Inject::Subquery)),
        Lit::part(" a USING (id)"),
    ],
    items = [ActiveUsers::NODE]
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
        From::part(Target::Item(0), AliasRule::NodeName, None),
        Lit::part(" WHERE vip")
    ],
    items = [ActiveUsers::NODE]
);

root!(VipCount: Postgres = node!(
    "vip_count",
    31,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT count(*) FROM "),
        From::part(Target::Item(0), AliasRule::NodeName, None),
        Lit::part(" JOIN "),
        From::part(Target::Item(1), AliasRule::Given, None),
        Lit::part(" a USING (id)"),
    ],
    items = [VIPS, ActiveUsers::NODE]
));

#[test]
fn hoists_cte_dependencies_first() {
    assert_eq!(
        VipCount::SQL,
        r#"WITH "active_users" AS (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $1), "vips" AS (SELECT id FROM "active_users" WHERE vip), "active_users_2" AS (SELECT id, email FROM "users" WHERE deleted_at IS NULL AND org_id = $2) SELECT count(*) FROM "vips" JOIN "active_users_2" a USING (id)"#
    );
}

const TREE: &Node = node!(
    "tree",
    40,
    Query,
    Inject::Cte { recursive: true },
    [
        Lit::part("SELECT id, parent_id FROM nodes WHERE id = "),
        Param::part(0, "root", "i64"),
        Lit::part(
            " UNION ALL SELECT n.id, n.parent_id FROM nodes n JOIN tree t ON n.parent_id = t.id"
        ),
    ]
);

root!(Subtree: Postgres = node!(
    "subtree",
    41,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT id FROM "), From::part(Target::Item(0), AliasRule::NodeName, None)],
    items = [TREE]
));

#[test]
fn recursive_ctes_make_the_with_clause_recursive() {
    assert_eq!(
        Subtree::SQL,
        r#"WITH RECURSIVE "tree" AS (SELECT id, parent_id FROM nodes WHERE id = $1 UNION ALL SELECT n.id, n.parent_id FROM nodes n JOIN tree t ON n.parent_id = t.id) SELECT id FROM "tree""#
    );
}

const ARCHIVED: &Node = node!(
    "archived",
    50,
    Dml,
    Inject::Cte { recursive: false },
    [
        Lit::part("DELETE FROM "),
        From::part(Target::Item(0), AliasRule::NodeName, None),
        Lit::part(" WHERE deleted_at < now() RETURNING id"),
    ],
    items = [Users::NODE]
);

root!(ArchiveCount: Postgres = node!(
    "archive_count",
    51,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT count(*) FROM "), From::part(Target::Item(0), AliasRule::NodeName, None)],
    items = [ARCHIVED]
));

#[test]
fn data_modifying_ctes_where_the_dialect_allows_them() {
    assert_eq!(
        ArchiveCount::SQL,
        r#"WITH "archived" AS (DELETE FROM "users" WHERE deleted_at < now() RETURNING id) SELECT count(*) FROM "archived""#
    );
}

root!(TableAsSubquery: Postgres = node!(
    "table_as_subquery",
    55,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT * FROM "),
        From::part(Target::Item(0), AliasRule::NodeName, Some(Inject::Subquery)),
    ],
    items = [Users::NODE]
));

#[test]
fn tables_are_always_written_by_name() {
    assert_eq!(TableAsSubquery::SQL, r#"SELECT * FROM "users""#);
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
