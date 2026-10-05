#[macro_use]
mod support;

use core::marker::PhantomData;

use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::fingerprint::Fingerprint;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::node::kind::Kind;
use static_typed_queries_core::node::name::Name;
use static_typed_queries_core::node::nodes::Nodes;
use static_typed_queries_core::part::Parts;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::ident::Ident;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::part::target::Target;
use static_typed_queries_core::sql::Sql;
use static_typed_queries_core::statement::Statement;

struct Users;

impl Sql for Users {
    type Dialect = Postgres;
    const NODE: &'static Node = node!("users", 1, Table, Inject::Ident, [Ident::part("users")]);
}

struct Orders;

impl Sql for Orders {
    type Dialect = Postgres;
    const NODE: &'static Node = node!("orders", 2, Table, Inject::Ident, [Ident::part("orders")]);
}

struct Recent<T>(PhantomData<T>);

impl<T: Sql> Sql for Recent<T> {
    type Dialect = T::Dialect;
    const NODE: &'static Node = &Node {
        name: Name::new("recent"),
        fingerprint: Fingerprint(3).combine(T::NODE.fingerprint),
        kind: Kind::Query,
        inject: Inject::cte(false),
        parts: Parts(&[
            Lit::part("SELECT id FROM "),
            From::part(Target::Node(T::NODE), AliasRule::NodeName, None),
            Lit::part(" WHERE created_at > now() - interval '1 day'"),
        ]),
        before: Nodes(&[]),
        after: Nodes(&[]),
        items: Nodes(&[]),
    };
}

const RECENT_2: &Node = node!(
    "recent_2",
    4,
    Query,
    Inject::cte(false),
    [Lit::part("SELECT 2 AS id")]
);

root!(Activity: Postgres = node!(
    "activity",
    5,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT * FROM "),
        From::part(Target::Item(0), AliasRule::Given, None),
        Lit::part(" u JOIN "),
        From::part(Target::Item(1), AliasRule::Given, None),
        Lit::part(" o USING (id) JOIN "),
        From::part(Target::Item(2), AliasRule::Given, None),
        Lit::part(" r USING (id) WHERE u.id IN (SELECT id FROM "),
        From::part(Target::Item(0), AliasRule::NodeName, None),
        Lit::part(")"),
    ],
    items = [Recent::<Users>::NODE, Recent::<Orders>::NODE, RECENT_2]
));

#[test]
fn generic_fingerprints_depend_on_type_parameters() {
    assert_ne!(
        Recent::<Users>::NODE.fingerprint,
        Recent::<Orders>::NODE.fingerprint
    );
    assert_ne!(
        Fingerprint(1).combine(Fingerprint(2)),
        Fingerprint(2).combine(Fingerprint(1))
    );
}

#[test]
fn renames_distinct_ctes_that_share_a_name() {
    assert_eq!(
        Activity::SQL,
        r#"WITH "recent" AS (SELECT id FROM "users" WHERE created_at > now() - interval '1 day'), "recent_2" AS (SELECT id FROM "orders" WHERE created_at > now() - interval '1 day'), "recent_2_2" AS (SELECT 2 AS id) SELECT * FROM "recent" u JOIN "recent_2" o USING (id) JOIN "recent_2_2" r USING (id) WHERE u.id IN (SELECT id FROM "recent")"#
    );
}
