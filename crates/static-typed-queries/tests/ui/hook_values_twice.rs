use static_typed_queries::prelude::*;

#[query(Postgres, sql = "SELECT set_config('app.tenant', {tenant: String}, true)")]
pub struct SetTenant;

#[table(Postgres, name = "orders", before(SetTenant))]
pub struct Orders;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE id = {_: i64}")]
pub struct OrderById;

fn main() {
    let _ = OrderById::builder()
        .id(1)
        .with(SetTenant::builder().tenant("acme".to_owned()))
        .with(SetTenant::builder().tenant("other".to_owned()));
}
