use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

pub struct UserRow;

#[query(Postgres, row = UserRow, sql = "UPDATE {Users} SET seen_at = now() WHERE id = {id: i64}")]
pub struct MarkSeen;

fn main() {}
