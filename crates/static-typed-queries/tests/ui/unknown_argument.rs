use static_typed_queries::prelude::*;

#[query(Postgres, sqll = "SELECT 1")]
pub struct One;

fn main() {}
