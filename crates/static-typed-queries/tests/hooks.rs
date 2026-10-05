use std::marker::PhantomData;

use static_typed_queries::__private::render::Render;
use static_typed_queries::prelude::*;
use static_typed_queries::state::{NoHooks, WithHooks};

#[query(
    Postgres,
    sql = "SELECT set_config('app.tenant', {tenant: String}, true)"
)]
pub struct SetTenant;

#[query(Postgres, sql = "INSERT INTO audit (at) VALUES (now())")]
pub struct Audit;

#[table(Postgres, name = "orders", before(SetTenant), after(Audit))]
pub struct Orders;

#[table(Postgres, name = "events", after(Audit))]
pub struct Events;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE status = {_: String}")]
pub struct ByStatus;

#[query(Postgres, cte, sql = "SELECT id FROM {Orders} WHERE total > {_: i64}")]
pub struct BigOrders;

#[query(Postgres, sql = "SELECT count(*) FROM {BigOrders}")]
pub struct BigOrderCount;

#[query(Postgres, sql = "SELECT count(*) FROM {Events}")]
pub struct EventCount;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[statement(CountOf<Orders>)]
pub struct OrderCount;

#[statement(CountOf<Users>)]
pub struct UserCount;

fn hooked<T: Render<Hooks = WithHooks>>() {}

fn unhooked<T: Render<Hooks = NoHooks>>() {}

#[test]
fn statements_that_use_a_hooked_table_run_its_hooks() {
    assert_eq!(
        ByStatus::SQL,
        r#"SELECT id FROM "orders" WHERE status = $1"#
    );
    let before: Vec<&str> = ByStatus::BEFORE.iter().map(|hook| hook.sql()).collect();
    assert_eq!(before, ["SELECT set_config('app.tenant', $1, true)"]);
    let after: Vec<&str> = ByStatus::AFTER.iter().map(|hook| hook.sql()).collect();
    assert_eq!(after, ["INSERT INTO audit (at) VALUES (now())"]);
    assert!(UserCount::BEFORE.is_empty() && UserCount::AFTER.is_empty());
}

#[test]
fn hook_parameters_are_set_through_the_table() {
    let params = ByStatus::builder()
        .status("open".to_owned())
        .orders()
        .set_tenant()
        .tenant("acme".to_owned())
        .build();
    assert_eq!(params.status, "open");
    assert_eq!(params.orders.set_tenant.tenant, "acme");
    assert_eq!(ByStatus::BEFORE[0].binds()[0].path().steps(), [0, 0]);

    let params = EventCount::builder().build();
    assert_eq!(params.events.audit, ());
}

#[test]
fn hooks_mark_every_statement_that_reaches_the_table() {
    hooked::<ByStatus>();
    hooked::<BigOrderCount>();
    hooked::<EventCount>();
    hooked::<OrderCount>();
    unhooked::<SetTenant>();
    unhooked::<UserCount>();
}

#[query(
    Sqlite,
    sql = "UPDATE counters SET n = n + 1 WHERE name = {name: String}"
)]
pub struct Bump;

#[query(Sqlite, sql = "INSERT INTO audit (note) VALUES ({note: String})")]
pub struct Note;

#[table(Sqlite, name = "items", before(Bump), after(Note))]
pub struct Items;

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct Item {
    pub id: i64,
    pub price: i64,
}

#[query(
    Sqlite,
    row = Item,
    sql = "SELECT id, price FROM {Items} WHERE price > {_: i64} ORDER BY id"
)]
pub struct Pricey;

#[query(
    Sqlite,
    sql = "UPDATE {Items} SET price = price * 2 WHERE price > {_: i64}"
)]
pub struct Double;

async fn connect() -> sqlx::Result<sqlx::SqliteConnection> {
    use sqlx::Connection;
    let mut conn = sqlx::SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, price INTEGER);
         CREATE TABLE counters (name TEXT PRIMARY KEY, n INTEGER);
         CREATE TABLE audit (note TEXT UNIQUE);
         INSERT INTO items VALUES (1, 5), (2, 20), (3, 30);
         INSERT INTO counters VALUES ('reads', 0);",
    )
    .execute(&mut conn)
    .await?;
    Ok(conn)
}

async fn state(conn: &mut sqlx::SqliteConnection) -> sqlx::Result<(i64, Vec<String>)> {
    let (n,): (i64,) = sqlx::query_as("SELECT n FROM counters")
        .fetch_one(&mut *conn)
        .await?;
    let notes: Vec<(String,)> = sqlx::query_as("SELECT note FROM audit ORDER BY note")
        .fetch_all(&mut *conn)
        .await?;
    Ok((n, notes.into_iter().map(|(note,)| note).collect()))
}

#[tokio::test]
async fn run_wraps_the_statement_in_its_hooks() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let rows = Pricey::builder()
        .price(10)
        .items()
        .bump()
        .name("reads".to_owned())
        .items()
        .note()
        .note("pricey".to_owned())
        .run(&mut conn)
        .await?;
    assert_eq!(rows, [Item { id: 2, price: 20 }, Item { id: 3, price: 30 }]);
    assert_eq!(state(&mut conn).await?, (1, vec!["pricey".to_owned()]));

    let doubled = Double::builder()
        .price(25)
        .items()
        .bump()
        .name("reads".to_owned())
        .items()
        .note()
        .note("double".to_owned())
        .run(&mut conn)
        .await?;
    assert_eq!(doubled, 1);

    let ids: Vec<(i64,)> = Pricey::builder()
        .price(40)
        .items()
        .bump()
        .name("reads".to_owned())
        .items()
        .note()
        .note("ids".to_owned())
        .run_as(&mut conn)
        .await?;
    assert_eq!(ids, [(3,)]);
    Ok(())
}

#[tokio::test]
async fn a_failing_hook_rolls_back_the_whole_run() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let run = |note: &str| {
        Double::builder()
            .price(0)
            .items()
            .bump()
            .name("reads".to_owned())
            .items()
            .note()
            .note(note.to_owned())
    };
    run("once").run(&mut conn).await?;
    let error = run("once").run(&mut conn).await.unwrap_err();
    assert!(error.to_string().contains("UNIQUE"), "{error}");
    assert_eq!(state(&mut conn).await?, (1, vec!["once".to_owned()]));
    let (price,): (i64,) = sqlx::query_as("SELECT price FROM items WHERE id = 1")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(price, 10);
    Ok(())
}
