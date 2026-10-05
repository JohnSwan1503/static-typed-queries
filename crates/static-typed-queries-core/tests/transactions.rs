#[macro_use]
mod support;

use core::marker::PhantomData;

use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::impl_transaction;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::fingerprint::Fingerprint;
use static_typed_queries_core::node::hooks::Hooks;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::node::items::Items;
use static_typed_queries_core::node::kind::Kind;
use static_typed_queries_core::node::name::Name;
use static_typed_queries_core::part::Parts;
use static_typed_queries_core::part::expr::Expr;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::ident::Ident;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::part::param::Param;
use static_typed_queries_core::part::target::Target;
use static_typed_queries_core::sql::Sql;
use static_typed_queries_core::statement::Statement;
use static_typed_queries_core::statement::hook::Hook;
use static_typed_queries_core::transaction::Transaction;

const SET_TENANT: &Node = node!(
    "set_tenant",
    1,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT set_config('app.tenant', "),
        Param::part(0, "tenant", "String"),
        Lit::part(", true)"),
    ]
);

const ORDERS: &Node = node!(
    "orders",
    2,
    Table,
    Inject::Ident,
    [Ident::part("orders")],
    before = [SET_TENANT]
);

const ARCHIVE: &Node = node!(
    "archive",
    3,
    Dml,
    Inject::Subquery,
    [
        Lit::part("DELETE FROM "),
        From::part(Target::Item(0), AliasRule::NodeName, None),
        Lit::part(" WHERE created_at < "),
        Param::part(0, "before", "i64"),
    ],
    items = [ORDERS]
);

const ORDER_COUNT: &Node = node!(
    "order_count",
    4,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT count(*) FROM "),
        From::part(Target::Item(0), AliasRule::NodeName, None),
    ],
    items = [ORDERS]
);

root!(Archive: Postgres = ARCHIVE);

struct Active;

impl Sql for Active {
    type Dialect = Postgres;
    type Params = ();
    const NODE: &'static Node = node!(
        "active",
        5,
        Query,
        Inject::cte(false),
        [
            Lit::part("SELECT id FROM users WHERE org_id = "),
            Param::part(0, "org_id", "i64"),
        ]
    );
}

struct CountOf<T>(PhantomData<T>);

impl<T: Sql> Sql for CountOf<T> {
    type Dialect = T::Dialect;
    type Params = ();
    const NODE: &'static Node = &Node {
        name: Name::new("count_of"),
        fingerprint: Fingerprint(6).combine(T::NODE.fingerprint),
        kind: Kind::Query,
        inject: Inject::Subquery,
        parts: Parts(&[
            Lit::part("SELECT count(*) FROM "),
            From::part(Target::Node(T::NODE), AliasRule::NodeName, None),
        ]),
        before: Hooks(&[]),
        after: Hooks(&[]),
        items: Items(&[]),
    };
}

struct Nightly;

impl Sql for Nightly {
    type Dialect = Postgres;
    type Params = ();
    const NODE: &'static Node = node!(
        "nightly",
        7,
        Transaction,
        Inject::Subquery,
        [
            Expr::part(Target::Item(0)),
            Expr::part(Target::Item(1)),
            Expr::part(Target::Item(3)),
            Expr::part(Target::Item(0)),
        ],
        items = [
            ARCHIVE,
            <CountOf<Active> as Sql>::NODE,
            <Active as Sql>::NODE,
            ORDER_COUNT
        ]
    );
}

impl_transaction!(Nightly);

const ORDER_TOTAL: &Node = node!(
    "order_total",
    8,
    Scope,
    Inject::Subquery,
    [],
    items = [ORDER_COUNT]
);

struct Report;

impl Sql for Report {
    type Dialect = Postgres;
    type Params = ();
    const NODE: &'static Node = node!(
        "report",
        9,
        Transaction,
        Inject::Subquery,
        [Expr::part(Target::Item(0))],
        items = [ORDER_TOTAL]
    );
}

impl_transaction!(Report);

fn sql(statements: &[Hook]) -> Vec<(&str, &str)> {
    statements
        .iter()
        .map(|statement| (statement.name().as_str(), statement.sql()))
        .collect()
}

fn paths(statement: &Hook) -> Vec<Vec<u16>> {
    statement
        .binds()
        .iter()
        .map(|bind| bind.path().steps().to_vec())
        .collect()
}

#[test]
fn each_step_renders_as_its_own_statement() {
    assert_eq!(
        sql(Nightly::STEPS),
        [
            ("archive", r#"DELETE FROM "orders" WHERE created_at < $1"#),
            (
                "count_of",
                r#"WITH "active" AS (SELECT id FROM users WHERE org_id = $1) SELECT count(*) FROM "active""#
            ),
            ("order_count", r#"SELECT count(*) FROM "orders""#),
            ("archive", r#"DELETE FROM "orders" WHERE created_at < $1"#),
        ]
    );
    assert_eq!(Nightly::STEPS[0].sql(), Archive::SQL);
}

#[test]
fn step_binds_are_relative_to_the_transaction() {
    let steps: Vec<_> = Nightly::STEPS.iter().map(paths).collect();
    assert_eq!(steps, [vec![vec![0]], vec![vec![2]], vec![], vec![vec![0]]]);
}

#[test]
fn hooks_run_once_around_all_the_steps() {
    assert_eq!(
        sql(Nightly::BEFORE),
        [("set_tenant", "SELECT set_config('app.tenant', $1, true)")]
    );
    assert_eq!(paths(&Nightly::BEFORE[0]), [Vec::<u16>::new()]);
    assert!(Nightly::AFTER.is_empty());
}

#[test]
fn statement_steps_render_their_target() {
    assert_eq!(
        sql(Report::STEPS),
        [("order_total", r#"SELECT count(*) FROM "orders""#)]
    );
    assert_eq!(paths(&Report::STEPS[0]), Vec::<Vec<u16>>::new());
    assert_eq!(sql(Report::BEFORE), sql(Nightly::BEFORE));
}
