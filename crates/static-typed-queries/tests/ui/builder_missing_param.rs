use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE created_at BETWEEN {since} AND {until}")]
pub struct Search {
    pub since: i64,
    pub until: i64,
}

fn main() {
    let _ = Search::builder().since(1).build();
}
