use std::marker::PhantomData;

use static_typed_queries::__private::render::Render;
use static_typed_queries::prelude::*;
use static_typed_queries::state::{NoHooks, WithHooks};

#[query(Postgres, sql = "SELECT set_config('app.tenant', {tenant}, true)")]
pub struct SetTenant {
    pub tenant: String,
}

#[query(Postgres, sql = "INSERT INTO audit (at) VALUES (now())")]
pub struct Audit;

#[table(Postgres, name = "orders", before(SetTenant), after(Audit))]
pub struct Orders;

#[table(Postgres, name = "events", after(Audit))]
pub struct Events;

#[table(Postgres, name = "refunds", before(SetTenant), after(Audit))]
pub struct Refunds;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE status = {status}")]
pub struct ByStatus {
    pub status: String,
}

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE total > {total}")]
pub struct BigOrders {
    pub total: i64,
}

#[query(Postgres, sql = "SELECT count(*) FROM {big}")]
pub struct BigOrderCount {
    #[cte]
    pub big: BigOrders,
}

#[query(Postgres, sql = "SELECT * FROM {big} JOIN {Orders} o USING (id)")]
pub struct BigOrderRows {
    #[cte]
    pub big: BigOrders,
}

#[query(Postgres, sql = "SELECT id FROM {Orders} JOIN {Refunds} r USING (id)")]
pub struct Refunded;

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
fn hooks_bind_their_own_parameters() {
    let bind = ByStatus::BEFORE[0].binds()[0];
    assert_eq!((bind.path().steps(), bind.field()), (&[][..], "tenant"));
    assert_eq!(
        ByStatus::BEFORE[0].fingerprint(),
        <SetTenant as Sql>::NODE.fingerprint
    );
}

#[test]
fn each_hook_runs_once_per_statement() {
    let names = |hooks: &[static_typed_queries::Command]| -> Vec<&str> {
        hooks.iter().map(|hook| hook.name().as_str()).collect()
    };
    assert_eq!(names(BigOrderRows::BEFORE), ["set_tenant"]);
    assert_eq!(names(BigOrderRows::AFTER), ["audit"]);
    assert_eq!(names(Refunded::BEFORE), ["set_tenant"]);
    assert_eq!(names(Refunded::AFTER), ["audit"]);
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

#[query(Sqlite, sql = "UPDATE counters SET n = n + 1 WHERE name = {name}")]
pub struct Bump {
    pub name: String,
}

#[query(Sqlite, sql = "INSERT INTO audit (note) VALUES ({note})")]
pub struct Note {
    pub note: String,
}

#[table(Sqlite, name = "items", before(Bump), after(Note))]
pub struct Items;

#[table(Sqlite, name = "tags", before(Bump), after(Note))]
pub struct Tags;

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct Item {
    pub id: i64,
    pub price: i64,
}

#[query(
    Sqlite,
    row = Item,
    sql = "SELECT id, price FROM {Items} WHERE price > {price} ORDER BY id"
)]
pub struct Pricey {
    pub price: i64,
}

#[query(
    Sqlite,
    sql = "UPDATE {Items} SET price = price * 2 WHERE price > {price}"
)]
pub struct Double {
    pub price: i64,
}

#[query(
    Sqlite,
    sql = "SELECT id FROM {Items} WHERE id IN (SELECT id FROM {Tags})"
)]
pub struct Tagged;

async fn connect() -> sqlx::Result<sqlx::SqliteConnection> {
    use sqlx::Connection;
    let mut conn = sqlx::SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql(
        "CREATE TABLE items (id INTEGER PRIMARY KEY, price INTEGER);
         CREATE TABLE counters (name TEXT PRIMARY KEY, n INTEGER);
         CREATE TABLE audit (note TEXT UNIQUE);
         CREATE TABLE tags (id INTEGER PRIMARY KEY);
         INSERT INTO items VALUES (1, 5), (2, 20), (3, 30);
         INSERT INTO tags VALUES (2);
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
    let rows = Pricey { price: 10 }
        .with(Bump {
            name: "reads".to_owned(),
        })
        .with(Note {
            note: "pricey".to_owned(),
        })
        .run(&mut conn)
        .await?;
    assert_eq!(rows, [Item { id: 2, price: 20 }, Item { id: 3, price: 30 }]);
    assert_eq!(state(&mut conn).await?, (1, vec!["pricey".to_owned()]));

    let doubled = Double { price: 25 }
        .with(Bump {
            name: "reads".to_owned(),
        })
        .with(Note {
            note: "double".to_owned(),
        })
        .run(&mut conn)
        .await?;
    assert_eq!(doubled, 1);

    let ids: Vec<(i64,)> = Pricey { price: 40 }
        .with(Bump {
            name: "reads".to_owned(),
        })
        .with(Note {
            note: "ids".to_owned(),
        })
        .run_as(&mut conn)
        .await?;
    assert_eq!(ids, [(3,)]);
    Ok(())
}

#[tokio::test]
async fn hooks_shared_by_two_tables_run_once() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let ids: Vec<(i64,)> = Tagged
        .with(Bump {
            name: "reads".to_owned(),
        })
        .with(Note {
            note: "tagged".to_owned(),
        })
        .run_as(&mut conn)
        .await?;
    assert_eq!(ids, [(2,)]);
    assert_eq!(state(&mut conn).await?, (1, vec!["tagged".to_owned()]));
    Ok(())
}

#[tokio::test]
async fn a_failing_hook_rolls_back_the_whole_run() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let run = |note: &str| {
        Double { price: 0 }
            .with(Bump {
                name: "reads".to_owned(),
            })
            .with(Note {
                note: note.to_owned(),
            })
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

#[test]
fn runs_are_send() {
    fn send<T: Send>(_: T) {}
    let bump = || Bump {
        name: "reads".to_owned(),
    };
    let note = || Note {
        note: "send".to_owned(),
    };
    let _ = |conn: &mut sqlx::SqliteConnection| {
        send(Double { price: 0 }.with(bump()).with(note()).run(conn))
    };
    let _ = |conn: &mut sqlx::SqliteConnection| {
        send(
            Tagged
                .with(bump())
                .with(note())
                .run_as::<(i64,), _, _>(conn),
        )
    };
}
