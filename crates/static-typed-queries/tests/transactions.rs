use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(
    Postgres,
    cte,
    sql = "SELECT id FROM {Orders} WHERE customer_id = {customer_id: i64}"
)]
pub struct CustomerOrders;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[query(
    Postgres,
    sql = "DELETE FROM {Orders} WHERE created_at < {before: i64}"
)]
pub struct Archive;

#[statement(CountOf<Orders>)]
pub struct OrderCount;

#[transaction(
    Postgres,
    steps(Archive, CountOf<CustomerOrders>, OrderCount, Archive)
)]
pub struct Nightly;

#[test]
fn steps_render_in_order() {
    let steps: Vec<(&str, &str)> = Nightly::STEPS
        .iter()
        .map(|step| (step.name().as_str(), step.sql()))
        .collect();
    assert_eq!(
        steps,
        [
            ("archive", Archive::SQL),
            (
                "count_of",
                r#"WITH "customer_orders" AS (SELECT id FROM "orders" WHERE customer_id = $1) SELECT count(*) FROM "customer_orders""#
            ),
            ("order_count", OrderCount::SQL),
            ("archive", Archive::SQL),
        ]
    );
}

#[test]
fn steps_take_their_values_from_the_transaction() {
    let params = Nightly::builder()
        .archive()
        .before(10)
        .customer_orders()
        .customer_id(7)
        .build();
    assert_eq!(params.archive.before, 10);
    assert_eq!(params.customer_orders.customer_id, 7);
}

mod lite {
    use static_typed_queries::prelude::*;

    #[query(
        Sqlite,
        sql = "UPDATE counters SET n = n + 1 WHERE name = {name: String}"
    )]
    pub struct Bump;

    #[query(Sqlite, sql = "INSERT INTO audit (note) VALUES ({note: String})")]
    pub struct Note;

    #[table(Sqlite, name = "items", before(Bump), after(Note))]
    pub struct Items;

    #[query(
        Sqlite,
        sql = "UPDATE {Items} SET price = price * 2 WHERE price > {_: i64}"
    )]
    pub struct Double;

    #[query(
        Sqlite,
        sql = "INSERT INTO {Items} (id, price) VALUES ({id: i64}, {price: i64})"
    )]
    pub struct AddItem;

    #[query(Sqlite, sql = "SELECT id, price FROM {Items} ORDER BY id")]
    #[derive(Debug, PartialEq)]
    pub struct Stock {
        pub id: i64,
        pub price: i64,
    }

    #[transaction(Sqlite, steps(Double, AddItem, Stock))]
    pub struct Restock;
}

use lite::{Bump, Note, Restock, Stock};
use sqlx::{Connection, SqliteConnection};

async fn connect() -> sqlx::Result<SqliteConnection> {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, price INTEGER);
         CREATE TABLE counters (name TEXT PRIMARY KEY, n INTEGER);
         CREATE TABLE audit (note TEXT);
         INSERT INTO items VALUES (1, 5), (2, 20);
         INSERT INTO counters VALUES ('runs', 0);",
    )
    .execute(&mut conn)
    .await?;
    Ok(conn)
}

async fn hooks(conn: &mut SqliteConnection) -> sqlx::Result<(i64, Vec<String>)> {
    let (n,): (i64,) = sqlx::query_as("SELECT n FROM counters")
        .fetch_one(&mut *conn)
        .await?;
    let notes: Vec<(String,)> = sqlx::query_as("SELECT note FROM audit")
        .fetch_all(&mut *conn)
        .await?;
    Ok((n, notes.into_iter().map(|(note,)| note).collect()))
}

async fn restock(
    conn: &mut SqliteConnection,
    id: i64,
    note: &str,
) -> sqlx::Result<(u64, u64, Vec<Stock>)> {
    Restock::builder()
        .double()
        .price(10)
        .add_item()
        .id(id)
        .add_item()
        .price(30)
        .with(Bump::builder().name("runs".to_owned()))
        .with(Note::builder().note(note.to_owned()))
        .run(conn)
        .await
}

#[tokio::test]
async fn run_returns_the_output_of_each_step() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let (doubled, added, stock) = restock(&mut conn, 3, "restock").await?;
    assert_eq!((doubled, added), (1, 1));
    assert_eq!(
        stock,
        [
            Stock { id: 1, price: 5 },
            Stock { id: 2, price: 40 },
            Stock { id: 3, price: 30 },
        ]
    );
    assert_eq!(hooks(&mut conn).await?, (1, vec!["restock".to_owned()]));
    Ok(())
}

#[tokio::test]
async fn a_failing_step_rolls_back_every_step() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let error = restock(&mut conn, 1, "again").await.unwrap_err();
    assert!(error.to_string().contains("UNIQUE"), "{error}");
    let prices: Vec<(i64, i64)> = sqlx::query_as("SELECT id, price FROM items ORDER BY id")
        .fetch_all(&mut conn)
        .await?;
    assert_eq!(prices, [(1, 5), (2, 20)]);
    assert_eq!(hooks(&mut conn).await?, (0, vec![]));
    Ok(())
}

#[test]
fn runs_are_send() {
    fn send<T: Send>(_: T) {}
    let _ = |conn: &mut SqliteConnection| send(restock(conn, 3, "send"));
}
