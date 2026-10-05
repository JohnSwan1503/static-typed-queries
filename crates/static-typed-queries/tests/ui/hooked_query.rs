use static_typed_queries::prelude::*;

#[query(Postgres, sql = "INSERT INTO audit (at) VALUES (now())")]
pub struct Audit;

#[table(Postgres, name = "orders", after(Audit))]
pub struct Orders;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE id = {id}")]
pub struct OrderById {
    pub id: i64,
}

fn main() {
    let _ = OrderById { id: 1 }.query();
}
