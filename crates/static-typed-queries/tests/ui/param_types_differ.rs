use static_typed_queries::dialect::postgres::Postgres;
use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE total > {_: i64} AND total < {_: i32}")]
pub struct Between;

fn main() {}
