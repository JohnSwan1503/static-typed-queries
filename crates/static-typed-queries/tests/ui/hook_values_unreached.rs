use static_typed_queries::prelude::*;

#[query(Postgres, sql = "SELECT set_config('app.tenant', {tenant}, true)")]
pub struct SetTenant {
    pub tenant: String,
}

#[query(Postgres, sql = "SELECT set_config('app.user', {user}, true)")]
pub struct SetUser {
    pub user: String,
}

#[table(Postgres, name = "orders", before(SetTenant))]
pub struct Orders;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE id = {id}")]
pub struct OrderById {
    pub id: i64,
}

async fn run(conn: &mut sqlx::PgConnection) -> sqlx::Result<u64> {
    OrderById { id: 1 }
        .with(SetTenant {
            tenant: "acme".to_owned(),
        })
        .with(SetUser {
            user: "ann".to_owned(),
        })
        .run(conn)
        .await
}

fn main() {}
