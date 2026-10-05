use static_typed_queries::prelude::*;

#[table(MySql, name = "orders")]
pub struct Orders;

#[query(MySql, sql = "DELETE FROM {Orders}")]
pub struct Purge;

#[query(MySql, sql = "SELECT count(*) FROM {Orders}")]
pub struct OrderCount;

#[transaction(MySql, steps(Purge as cte, OrderCount))]
pub struct Sweep;

fn main() {}
