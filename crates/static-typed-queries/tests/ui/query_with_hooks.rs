use static_typed_queries::prelude::*;

#[query(Postgres, sql = "INSERT INTO audit (at) VALUES (now())")]
pub struct Audit;

#[table(Postgres, name = "orders", after(Audit))]
pub struct Orders;

#[query(Postgres, sql = "SELECT count(*) FROM {Orders}")]
pub struct OrderCount;

fn main() {
    let _ = sqlx::query(OrderCount);
}
