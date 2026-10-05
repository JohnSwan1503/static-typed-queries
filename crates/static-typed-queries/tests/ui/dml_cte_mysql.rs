use static_typed_queries::prelude::*;

#[table(MySql, name = "events")]
pub struct Events;

#[query(MySql, sql = "DELETE FROM {Events} WHERE created_at < now()")]
pub struct Purged;

#[query(MySql, sql = "SELECT count(*) FROM {purged}")]
pub struct PurgeCount {
    #[cte]
    pub purged: Purged,
}

#[query(MySql, sql = "SELECT count(*) FROM {Purged as cte}")]
pub struct ByType;

#[query(T::Dialect, sql = "SELECT count(*) FROM {of}")]
pub struct CountOf<T: Sql> {
    #[cte]
    pub of: T,
}

#[query(MySql, sql = "SELECT {count} AS n")]
pub struct Counted {
    #[subquery]
    pub count: CountOf<Purged>,
}

fn main() {}
