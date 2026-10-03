use std::marker::PhantomData;

use static_typed_queries::dialect::postgres::Postgres;
use static_typed_queries::prelude::*;

#[table(Postgres, name = "audit.events")]
pub struct Events;

#[query(
    Postgres,
    cte,
    sql = "
    DELETE FROM {Events}
    WHERE created_at < now() - interval '30 days'
    RETURNING id, org_id"
)]
pub struct Purged;

#[query(Postgres, display = sql, sql = "
    SELECT org_id, count(*) AS purged FROM {Purged} GROUP BY org_id")]
pub struct PurgeSummary;

#[table(Postgres, name = "categories")]
pub struct Categories;

#[query(
    Postgres,
    cte(recursive),
    name = "tree",
    sql = "
    SELECT id, parent_id FROM {Categories} WHERE id = {root: i32}
    UNION ALL
    SELECT c.id, c.parent_id FROM {Categories} c JOIN tree t ON c.parent_id = t.id"
)]
pub struct Subtree;

#[query(Postgres, display = sql, sql = "SELECT count(*) FROM {Subtree}")]
pub struct SubtreeSize;

#[table(Postgres, name = "users")]
pub struct Users;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(
    T::Dialect,
    cte,
    sql = "
    SELECT id FROM {T} WHERE created_at > now() - interval '1 day'"
)]
pub struct Recent<T: Sql>(PhantomData<T>);

#[query(Postgres, display = sql, sql = "
    SELECT * FROM {Recent<Users>} u JOIN {Recent<Orders>} o USING (id)")]
pub struct RecentActivity;

fn main() {
    println!("-- a DELETE ... RETURNING embedded as a CTE:\n{PurgeSummary}\n");
    println!("-- a recursive CTE with a parameter:\n{SubtreeSize}\n");
    println!("-- one generic CTE, instantiated twice and renamed:\n{RecentActivity}");
}
