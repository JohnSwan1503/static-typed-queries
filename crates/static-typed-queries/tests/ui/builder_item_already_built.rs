use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, cte, sql = "SELECT id FROM {Users} WHERE org_id = {_: i64}")]
pub struct ActiveUsers;

#[query(Postgres, sql = "SELECT count(*) FROM {ActiveUsers}")]
pub struct Count;

fn main() {
    let _ = Count::builder().active_users().org_id(1).active_users();
}
