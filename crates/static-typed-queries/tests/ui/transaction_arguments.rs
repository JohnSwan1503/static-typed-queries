use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(Postgres, steps(Orders), sql = "SELECT 1")]
pub struct One;

#[transaction(Postgres)]
pub struct Empty;

#[transaction(Postgres, sql = "SELECT 1", steps(One))]
pub struct WithSql;

#[transaction(Postgres, steps(One, savepoint()))]
pub struct EmptySavepoint;

fn main() {}
