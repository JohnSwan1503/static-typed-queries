#[macro_use]
#[path = "../support/mod.rs"]
mod support;

use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::before::Before;
use static_typed_queries_core::node::fingerprint::Fingerprint;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::node::kind::Kind;
use static_typed_queries_core::node::name::Name;
use static_typed_queries_core::part::Parts;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::ident::Ident;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::part::param::Param;

const SET_TENANT: &Node = node!(
    "set_tenant",
    1,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT set_config('app.tenant', "),
        Param::part(0, "tenant", "String"),
        Lit::part(", true)")
    ]
);

const ORDERS: &Node = &Node {
    name: Name::new("orders"),
    fingerprint: Fingerprint(2),
    kind: Kind::Table,
    inject: Inject::Ident,
    parts: Parts(&[Ident::part("orders")]),
    before: Before(&[SET_TENANT]),
};

root!(TenantOrders: Postgres = node!(
    "tenant_orders",
    3,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT * FROM "), From::part(ORDERS, AliasRule::NodeName, None)]
));

fn main() {}
