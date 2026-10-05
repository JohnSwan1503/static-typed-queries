use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[table(Postgres, name = "orders", before(Users))]
pub struct Orders;

fn main() {}
