use static_typed_queries::prelude::*;

const ONE: &str = "SELECT 1";

#[query(Postgres, sql = ONE)]
pub struct One;

fn main() {}
