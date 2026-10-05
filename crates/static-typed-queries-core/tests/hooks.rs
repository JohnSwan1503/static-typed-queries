#[macro_use]
mod support;

use static_typed_queries_core::dialect::mysql::MySql;
use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::ident::Ident;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::part::param::Param;
use static_typed_queries_core::statement::Statement;

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

const STALE: &Node = node!(
    "open_orders",
    2,
    Query,
    Inject::cte(false),
    [Lit::part(
        "SELECT id FROM carts WHERE seen_at < now() - interval '1 day'"
    )]
);

const PURGE: &Node = node!(
    "purge",
    3,
    Dml,
    Inject::Subquery,
    [
        Lit::part("DELETE FROM carts USING "),
        From::part(STALE, AliasRule::NodeName, None),
        Lit::part(" WHERE carts.id = open_orders.id"),
    ],
    items = [STALE]
);

const ORDERS: &Node = node!(
    "orders",
    4,
    Table,
    Inject::Ident,
    [Ident::part("orders")],
    before = [SET_TENANT],
    after = [PURGE]
);

const OPEN_ORDERS: &Node = node!(
    "open_orders",
    5,
    Query,
    Inject::cte(false),
    [
        Lit::part("SELECT id FROM "),
        From::part(ORDERS, AliasRule::NodeName, None),
        Lit::part(" WHERE status = "),
        Param::part(0, "status", "String"),
    ],
    items = [ORDERS]
);

const ORDER_BY_ID: &Node = node!(
    "order_by_id",
    6,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT * FROM "),
        From::part(ORDERS, AliasRule::NodeName, None),
        Lit::part(" WHERE id = "),
        Param::part(0, "id", "i64"),
        Lit::part(" OR parent_id IN (SELECT id FROM "),
        From::part(ORDERS, AliasRule::NodeName, None),
        Lit::part(")"),
    ],
    items = [ORDERS]
);

root!(OrderById: Postgres = ORDER_BY_ID);
root!(OrderByIdMySql: MySql = ORDER_BY_ID);

root!(BigOpenOrders: Postgres = node!(
    "big_open_orders",
    7,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT * FROM "),
        From::part(OPEN_ORDERS, AliasRule::NodeName, None),
        Lit::part(" JOIN "),
        From::part(ORDERS, AliasRule::Given, None),
        Lit::part(" o USING (id) WHERE o.total > "),
        Param::part(0, "total", "i64"),
    ],
    items = [OPEN_ORDERS, ORDERS]
));

fn sql(hooks: &[static_typed_queries_core::statement::hook::Hook]) -> Vec<(&str, &str)> {
    hooks
        .iter()
        .map(|hook| (hook.name().as_str(), hook.sql()))
        .collect()
}

#[test]
fn hooks_render_as_their_own_statements_around_the_main_one() {
    assert_eq!(
        OrderById::SQL,
        r#"SELECT * FROM "orders" WHERE id = $1 OR parent_id IN (SELECT id FROM "orders")"#
    );
    assert_eq!(
        sql(OrderById::BEFORE),
        [("set_tenant", "SELECT set_config('app.tenant', $1, true)")]
    );
    assert_eq!(
        sql(OrderById::AFTER),
        [(
            "purge",
            r#"WITH "open_orders" AS (SELECT id FROM carts WHERE seen_at < now() - interval '1 day') DELETE FROM carts USING "open_orders" WHERE carts.id = open_orders.id"#
        )]
    );
    assert_eq!(
        sql(OrderByIdMySql::BEFORE),
        [("set_tenant", "SELECT set_config('app.tenant', ?, true)")]
    );
}

#[test]
fn hooks_bind_from_their_own_parameters() {
    let paths = |binds: &[static_typed_queries_core::statement::bind::Bind]| {
        binds
            .iter()
            .map(|bind| (bind.path().steps().to_vec(), bind.field()))
            .collect::<Vec<_>>()
    };
    assert_eq!(paths(OrderById::BINDS), [(vec![], "id")]);
    assert_eq!(paths(OrderById::BEFORE[0].binds()), [(vec![], "tenant")]);
    assert_eq!(paths(OrderById::AFTER[0].binds()), []);
    assert_eq!(OrderById::BEFORE[0].fingerprint(), SET_TENANT.fingerprint);
    assert_eq!(OrderById::AFTER[0].fingerprint(), PURGE.fingerprint);
}

#[test]
fn hooks_run_once_per_statement() {
    assert_eq!(
        BigOpenOrders::SQL,
        r#"WITH "open_orders" AS (SELECT id FROM "orders" WHERE status = $1) SELECT * FROM "open_orders" JOIN "orders" o USING (id) WHERE o.total > $2"#
    );
    assert_eq!(sql(BigOpenOrders::BEFORE), sql(OrderById::BEFORE));
    assert_eq!(sql(BigOpenOrders::AFTER), sql(OrderById::AFTER));
}
