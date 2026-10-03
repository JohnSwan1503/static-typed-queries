//! Everything `#[query]` generates is documented.
#![deny(missing_docs)]

use std::marker::PhantomData;

use static_typed_queries::dialect::postgres::Postgres;
use static_typed_queries::prelude::*;

/// Users.
#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, cte, sql = "SELECT id FROM {Users} WHERE org_id = {_: i64}")]
pub struct ActiveUsers;

/// Counts any item.
#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

/// A report.
#[query(
    Postgres,
    sql = "
    SELECT {CountOf<ActiveUsers>} AS total FROM {ActiveUsers}
    WHERE id IN ({_: i64}, {_: i64}) AND kind = {kind: String}
    LIMIT {_: i64}"
)]
pub struct Report;

#[query(Postgres, sql = "SELECT 1")]
pub struct One;

fn main() {}
