use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE org_id = {org_id}")]
pub struct ActiveUsers {
    pub org_id: i64,
}

#[query(Postgres, sql = "SELECT count(*) FROM {active}")]
pub struct Count {
    #[cte]
    pub active: ActiveUsers,
}

fn main() {
    let _ = Count::builder().build();
}
