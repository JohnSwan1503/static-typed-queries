use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[table(Postgres, name = "audit.events")]
pub struct Events;

#[query(
    Postgres,
    sql = "
    DELETE FROM {Events}
    WHERE created_at < now() - interval '30 days'
    RETURNING id, org_id"
)]
pub struct Purged;

#[query(Postgres, display = sql, sql = "
    SELECT org_id, count(*) AS purged FROM {Purged as cte} GROUP BY org_id")]
pub struct PurgeSummary;

#[table(Postgres, name = "categories")]
pub struct Categories;

#[query(
    Postgres,
    name = "tree",
    sql = "
    SELECT id, parent_id FROM {Categories} WHERE id = {root}
    UNION ALL
    SELECT c.id, c.parent_id FROM {Categories} c JOIN tree t ON c.parent_id = t.id"
)]
pub struct Subtree {
    pub root: i32,
}

#[query(Postgres, display = sql, sql = "SELECT count(*) FROM {tree}")]
pub struct SubtreeSize {
    #[cte(recursive)]
    pub tree: Subtree,
}

#[table(Postgres, name = "users")]
pub struct Users;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(
    T::Dialect,
    sql = "
    SELECT id FROM {T} WHERE created_at > now() - interval '1 day'"
)]
pub struct Recent<T: Sql>(PhantomData<T>);

#[query(Postgres, display = sql, sql = "
    SELECT * FROM {Recent<Users> as cte} u JOIN {Recent<Orders> as cte} o USING (id)")]
pub struct RecentActivity;

fn main() {
    println!("-- a DELETE ... RETURNING embedded as a CTE:\n{PurgeSummary}\n");
    let size = SubtreeSize::builder().tree().root(1).build();
    println!("-- a recursive CTE with a parameter:\n{size}\n");
    println!("-- one generic CTE, instantiated twice and renamed:\n{RecentActivity}");
}
