use static_typed_queries::dialect::postgres::Postgres;
use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE created_at BETWEEN {_: i64} AND {_: i64} AND status = {_: String}")]
pub struct Search;

fn main() {
    let _ = Search::builder().status("paid".to_owned()).status("again".to_owned());
}
