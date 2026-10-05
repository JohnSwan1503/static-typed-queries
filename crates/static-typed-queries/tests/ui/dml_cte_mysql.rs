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

fn main() {}
