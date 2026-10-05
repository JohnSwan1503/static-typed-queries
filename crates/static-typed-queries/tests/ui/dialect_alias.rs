use static_typed_queries::prelude::*;

pub type Db = Postgres;

#[table(Db, name = "users")]
pub struct Users;

#[query(Db, sql = "SELECT id FROM {Users} WHERE tags @> ARRAY[{tag: String}]")]
pub struct Tagged;

fn main() {}
