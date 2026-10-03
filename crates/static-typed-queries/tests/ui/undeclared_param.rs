use static_typed_queries::dialect::postgres::Postgres;
use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE org_id = {org_id}")]
pub struct Members;

fn main() {}
