use std::marker::PhantomData;

use sqlx::{Connection, SqliteConnection};
use static_typed_queries::prelude::*;

#[table(Sqlite, name = "events")]
pub struct Events;

#[query(
    Sqlite,
    sql = "SELECT e.kind, at FROM {Events} e WHERE at >= {_: i64} ORDER BY at"
)]
#[derive(Debug, PartialEq)]
pub struct Event {
    pub kind: String,
    pub at: i64,
}

#[query(
    Sqlite,
    sql = r#"INSERT INTO {Events} (kind, at) VALUES ({_: String}, {_: i64}) RETURNING kind AS "type", at"#
)]
#[derive(Debug, PartialEq)]
pub struct Added {
    pub r#type: String,
    pub at: i64,
}

#[query(Sqlite, sql = "SELECT * FROM {Events} WHERE kind = {_: String}")]
#[derive(Debug, PartialEq)]
pub struct Kinds {
    pub kind: String,
}

#[query(T::Dialect, sql = "SELECT count(*) AS n FROM {T}")]
pub struct Tally<T: Sql>(PhantomData<T>);

#[statement(Tally<Events>)]
#[derive(Debug, PartialEq)]
pub struct EventTally {
    pub n: i64,
}

#[query(Sqlite, sql = "INSERT INTO reads (at) VALUES (0)")]
pub struct Read;

#[table(Sqlite, name = "events", after(Read))]
pub struct AuditedEvents;

#[query(Sqlite, sql = "SELECT kind FROM {AuditedEvents} ORDER BY at")]
#[derive(Debug, PartialEq)]
pub struct AuditedKinds {
    pub kind: String,
}

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT u.Email FROM {Users} u")]
pub struct Emails {
    pub email: String,
}

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
async fn a_struct_with_fields_is_its_own_row() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let added = Added::builder()
        .kind("c".to_owned())
        .at(9)
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
    let events = Event::builder().at(5).query()?.fetch_all(&mut conn).await?;
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
    let kinds = Kinds::builder()
        .kind("a".to_owned())
        .query()?
        .fetch_all(&mut conn)
        .await?;
    assert_eq!(
        kinds,
        [Kinds {
            kind: "a".to_owned()
        }]
    );
    let tally = EventTally::builder().query()?.fetch_one(&mut conn).await?;
    assert_eq!(tally, EventTally { n: 3 });
    Ok(())
}

#[tokio::test]
async fn statements_with_hooks_run_into_their_rows() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let kinds = AuditedKinds::builder().run(&mut conn).await?;
    assert_eq!(
        kinds,
        [
            AuditedKinds {
                kind: "a".to_owned()
            },
            AuditedKinds {
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

#[test]
fn postgres_folds_unquoted_column_names() {
    assert_eq!(Emails::SQL, r#"SELECT u.Email FROM "users" u"#);
}
