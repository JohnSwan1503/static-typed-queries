use static_typed_queries::prelude::*;

#[table(MySql, name = "events")]
pub struct Events;

#[query(MySql, cte, sql = "DELETE FROM {Events} WHERE created_at < now()")]
pub struct Purged;

#[query(MySql, sql = "SELECT count(*) FROM {Purged}")]
pub struct PurgeCount;

fn main() {}
