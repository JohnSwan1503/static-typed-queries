use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(Postgres, sql = "SELECT set_config('app.tenant', {tenant}, true)")]
pub struct SetTenant {
    pub tenant: String,
}

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE status = {status}")]
pub struct ByStatus {
    pub status: String,
}

async fn run(conn: &mut sqlx::PgConnection) -> sqlx::Result<u64> {
    ByStatus {
        status: "open".to_owned(),
    }
    .run(conn)
    .await
}

fn main() {
    let _ = ByStatus {
        status: "open".to_owned(),
    }
    .with(SetTenant {
        tenant: "acme".to_owned(),
    });
}
