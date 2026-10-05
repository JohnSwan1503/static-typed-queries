use static_typed_queries::prelude::*;

#[query(Postgres, sql = "INSERT INTO audit (at) VALUES (now())")]
pub struct Audit;

#[table(Postgres, name = "orders", after(Audit))]
pub struct Orders;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE id = {_: i64}")]
pub struct OrderById;

fn main() {
    let _ = OrderById::builder().id(1).with(Audit::builder());
}
