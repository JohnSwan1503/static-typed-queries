use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[query(Sqlite, sql = "UPDATE counters SET n = n + 1 WHERE name = {name: String}")]
pub struct Bump;

#[query(Sqlite, sql = "INSERT INTO audit (note) VALUES ({note: String})")]
pub struct Note;

#[table(Sqlite, name = "items", before(Bump), after(Note))]
pub struct Items;

#[query(Sqlite, sql = "SELECT id FROM {Items} WHERE price > {_: i64}")]
pub struct Pricey;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[statement(CountOf<Items>)]
pub struct ItemCount;

async fn pricey(conn: &mut sqlx::SqliteConnection) -> sqlx::Result<u64> {
    Pricey::builder()
        .price(10)
        .with(Bump::builder().name("reads".to_owned()))
        .run(conn)
        .await
}

async fn count(conn: &mut sqlx::SqliteConnection) -> sqlx::Result<Vec<(i64,)>> {
    ItemCount::builder().run_as(conn).await
}

fn main() {}
