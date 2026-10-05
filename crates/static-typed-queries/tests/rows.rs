use std::marker::PhantomData;

use sqlx::{Connection, FromRow, SqliteConnection};
use static_typed_queries::prelude::*;

#[table(Sqlite, name = "events")]
pub struct Events;

#[derive(FromRow, Debug, PartialEq)]
pub struct Event {
    pub kind: String,
    pub at: i64,
}

#[query(
    Sqlite,
    row = Event,
    sql = "SELECT e.kind, at FROM {Events} e WHERE at >= {since} ORDER BY at"
)]
pub struct EventsSince {
    pub since: i64,
}

#[derive(FromRow, Debug, PartialEq)]
pub struct Added {
    pub r#type: String,
    pub at: i64,
}

#[query(
    Sqlite,
    row = Added,
    sql = r#"INSERT INTO {Events} (kind, at) VALUES ({kind}, {at}) RETURNING kind AS "type", at"#
)]
pub struct AddEvent {
    pub kind: String,
    pub at: i64,
}

#[derive(FromRow, Debug, PartialEq)]
pub struct Count {
    pub n: i64,
}

#[query(T::Dialect, sql = "SELECT count(*) AS n FROM {T}")]
pub struct Tally<T: Sql>(PhantomData<T>);

#[statement(Tally<Events>, row = Count)]
pub struct EventTally;

#[query(Sqlite, sql = "INSERT INTO reads (at) VALUES (0)")]
pub struct Read;

#[table(Sqlite, name = "events", after(Read))]
pub struct AuditedEvents;

#[derive(FromRow, Debug, PartialEq)]
pub struct Kind {
    pub kind: String,
}

#[query(Sqlite, row = Kind, sql = "SELECT kind FROM {AuditedEvents} ORDER BY at")]
pub struct AuditedKinds;

async fn connect() -> sqlx::Result<SqliteConnection> {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql(
        "CREATE TABLE events (kind TEXT, at INTEGER);
         CREATE TABLE reads (at INTEGER);
         INSERT INTO events VALUES ('a', 1), ('b', 5);",
    )
    .execute(&mut conn)
    .await?;
    Ok(conn)
}

#[tokio::test]
async fn rows_are_read_into_the_row_type() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let added = AddEvent {
        kind: "c".to_owned(),
        at: 9,
    }
    .query()?
    .fetch_one(&mut conn)
    .await?;
    assert_eq!(
        added,
        Added {
            r#type: "c".to_owned(),
            at: 9
        }
    );
    let events = EventsSince { since: 5 }
        .query()?
        .fetch_all(&mut conn)
        .await?;
    assert_eq!(
        events,
        [
            Event {
                kind: "b".to_owned(),
                at: 5
            },
            Event {
                kind: "c".to_owned(),
                at: 9
            },
        ]
    );
    let tally = EventTally.query()?.fetch_one(&mut conn).await?;
    assert_eq!(tally, Count { n: 3 });
    Ok(())
}

#[tokio::test]
async fn statements_with_hooks_run_into_their_rows() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let kinds = AuditedKinds.run(&mut conn).await?;
    assert_eq!(
        kinds,
        [
            Kind {
                kind: "a".to_owned()
            },
            Kind {
                kind: "b".to_owned()
            },
        ]
    );
    let (reads,): (i64,) = sqlx::query_as("SELECT count(*) FROM reads")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(reads, 1);
    Ok(())
}
