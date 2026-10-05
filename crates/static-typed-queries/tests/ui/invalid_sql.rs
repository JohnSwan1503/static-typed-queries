use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE org_id = {org_id} ORDER")]
pub struct Broken {
    pub org_id: i64,
}

fn main() {}
