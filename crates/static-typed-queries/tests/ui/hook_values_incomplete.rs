use static_typed_queries::prelude::*;

#[query(Postgres, sql = "SELECT set_config('app.tenant', {tenant}, true)")]
pub struct SetTenant {
    pub tenant: String,
}

#[table(Postgres, name = "orders", before(SetTenant))]
pub struct Orders;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE id = {id}")]
pub struct OrderById {
    pub id: i64,
}

fn main() {
    let _ = OrderById::builder().id(1).with(SetTenant::builder());
}
