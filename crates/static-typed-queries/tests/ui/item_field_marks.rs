use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE org_id = {org_id}")]
pub struct ActiveUsers {
    pub org_id: i64,
}

#[query(Postgres, sql = "SELECT count(*) FROM {active as cte}")]
pub struct Overridden {
    #[cte]
    pub active: ActiveUsers,
}

#[query(Postgres, sql = "SELECT count(*) FROM {active}")]
pub struct Twice {
    #[cte]
    #[subquery]
    pub active: ActiveUsers,
}

#[query(Postgres, sql = "SELECT count(*) FROM {active}")]
pub struct Misspelt {
    #[cte(recursve)]
    pub active: ActiveUsers,
}

fn main() {}
