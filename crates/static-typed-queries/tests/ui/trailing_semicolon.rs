use static_typed_queries::dialect::postgres::Postgres;
use static_typed_queries::prelude::*;

#[query(Postgres, sql = "SELECT 1;")]
pub struct One;

fn main() {}
