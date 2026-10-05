use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(Postgres, sql = "DELETE FROM {Orders} WHERE created_at < {before}")]
pub struct Archive {
    pub before: i64,
}

#[query(Postgres, sql = "SELECT count(*) FROM {Orders}")]
pub struct OrderCount;

#[transaction(Postgres, steps(OrderCount))]
pub struct Unlisted {
    pub archive: Archive,
}

#[transaction(Postgres, steps(archive, OrderCount))]
pub struct Marked {
    #[cte]
    pub archive: Archive,
}

fn main() {}
