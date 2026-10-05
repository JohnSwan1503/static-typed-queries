use static_typed_queries::prelude::*;

#[query(Postgres, sql_file = "tests/templates/missing.sql")]
pub struct Missing;

#[query(Postgres, sql = "SELECT 1", sql_file = "tests/templates/one.sql")]
pub struct Both;

fn main() {}
