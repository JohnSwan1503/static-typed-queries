use static_typed_queries::prelude::*;

#[query(Postgres, sql = "SELECT 1")]
pub struct Ping;

#[query(Postgres, before(Ping), sql = "SELECT 2")]
pub struct Pong;

fn main() {}
