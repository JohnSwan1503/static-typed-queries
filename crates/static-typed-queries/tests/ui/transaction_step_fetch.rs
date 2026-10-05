use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(Postgres, sql = "DELETE FROM {Orders}")]
pub struct Purge;

#[transaction(Postgres, steps(Purge as one))]
pub struct One;

#[transaction(Postgres, steps(Purge as many))]
pub struct Many;

fn main() {}
