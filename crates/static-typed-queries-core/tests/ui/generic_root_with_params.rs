#[macro_use]
#[path = "../support/mod.rs"]
mod support;

use core::marker::PhantomData;

use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::part::param::Param;
use static_typed_queries_core::sql::Sql;

pub struct ActiveUsers;

impl Sql for ActiveUsers {
    type Dialect = Postgres;
    type Params = ();
    const NODE: &'static Node = node!(
        "active_users",
        1,
        Query,
        Inject::Subquery,
        [Lit::part("SELECT id FROM users WHERE org_id = "), Param::part(0, "org_id", "i64")]
    );
}

pub struct CountOf<T>(PhantomData<T>);

impl<T: Sql> Sql for CountOf<T> {
    type Dialect = T::Dialect;
    type Params = ();
    const NODE: &'static Node = node!(
        "count_of",
        2,
        Query,
        Inject::Subquery,
        [Lit::part("SELECT count(*) FROM "), From::part(T::NODE, AliasRule::NodeName, None)]
    );
}

root!(CountActive: Postgres = CountOf::<ActiveUsers>::NODE);

fn main() {}
