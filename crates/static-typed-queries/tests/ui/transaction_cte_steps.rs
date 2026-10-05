use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(Postgres, sql = "DELETE FROM {Orders}")]
pub struct Purge;

#[transaction(Postgres, steps(Purge as cte))]
pub struct Trailing;

#[transaction(Postgres, steps(Purge as cte, savepoint(Purge)))]
pub struct BeforeSavepoint;

fn main() {}
