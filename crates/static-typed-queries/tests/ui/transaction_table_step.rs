use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[transaction(Postgres, steps(Orders))]
pub struct Nightly;

fn main() {}
