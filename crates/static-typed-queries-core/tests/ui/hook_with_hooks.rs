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

const SET_TENANT: &Node = node!(
    "set_tenant",
    1,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT set_config('app.tenant', 'acme', true)")]
);

const ORDERS: &Node = node!(
    "orders",
    2,
    Table,
    Inject::Ident,
    [Ident::part("orders")],
    items = [SET_TENANT],
    before = [SET_TENANT]
);

const AUDIT: &Node = node!(
    "audit",
    3,
    Dml,
    Inject::Subquery,
    [
        Lit::part("INSERT INTO audit (order_id) SELECT id FROM "),
        From::part(ORDERS, AliasRule::NodeName, None),
    ],
    items = [ORDERS]
);

const PAYMENTS: &Node = node!(
    "payments",
    4,
    Table,
    Inject::Ident,
    [Ident::part("payments")],
    items = [AUDIT],
    after = [AUDIT]
);

root!(Paid: Postgres = node!(
    "paid",
    5,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT * FROM "), From::part(PAYMENTS, AliasRule::NodeName, None)],
    items = [PAYMENTS]
));

fn main() {}
