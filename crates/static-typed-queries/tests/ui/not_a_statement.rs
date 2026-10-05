use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(Postgres, sql = "DELETE FROM {Orders}")]
pub struct Purge;

#[statement(Orders)]
pub struct AllOrders;

#[transaction(Postgres, steps(Purge))]
pub struct Nightly;

#[transaction(Postgres, steps(Nightly, Purge))]
pub struct Twice;

fn main() {}
