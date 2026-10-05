use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE tags @> ARRAY[{tag: String}]")]
pub struct Tagged;

#[query(Sqlite, sql = "SELECT count(*) FROM {Tagged}")]
pub struct TaggedCount;

fn main() {}
