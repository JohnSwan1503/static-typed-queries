use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE status = {_: String}")]
pub struct ByStatus;

fn main() {
    let _ = ByStatus::builder().orders();
}
