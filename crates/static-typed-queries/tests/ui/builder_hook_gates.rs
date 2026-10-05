use static_typed_queries::prelude::*;

#[query(Postgres, sql = "SELECT set_config('app.tenant', {tenant}, true)")]
pub struct SetTenant {
    pub tenant: String,
}

#[table(Postgres, name = "orders", before(SetTenant))]
pub struct Orders;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE status = {status}")]
pub struct ByStatus {
    pub status: String,
}

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE org_id = {org_id}")]
pub struct ByOrg {
    pub org_id: i64,
}

fn main() {
    let _ = ByStatus::builder().status("open".to_owned()).query();
    let _ = ByOrg::builder().org_id(1).with(SetTenant {
        tenant: "acme".to_owned(),
    });
}
