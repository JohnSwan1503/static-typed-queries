use std::marker::PhantomData;

use sqlx::{Connection, Row, SqliteConnection};
use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(
    Postgres,
    sql = "
    SELECT id FROM {Orders}
    WHERE created_at BETWEEN {since} AND {until}
      AND status = {status}
      AND customer_id IN ({customer}, {other_customer})
      AND lower(note) LIKE lower({note})
    ORDER BY id
    LIMIT {limit} OFFSET {offset}"
)]
pub struct Search {
    pub since: i64,
    pub until: i64,
    pub status: String,
    pub customer: i64,
    pub other_customer: i64,
    pub note: String,
    pub limit: i64,
    pub offset: i64,
}

#[query(
    Postgres,
    sql = "SELECT id FROM {Orders} WHERE status = {status} OR previous_status = {status}"
)]
pub struct EitherStatus {
    pub status: String,
}

#[query(
    Postgres,
    sql = r#"SELECT id FROM {Orders} WHERE "type" = {type} AND build = {build}"#
)]
pub struct ByType {
    pub r#type: String,
    pub build: i64,
}

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE status = {status}")]
pub struct Matching {
    pub status: String,
}

#[query(
    Postgres,
    sql = "SELECT o.id FROM {Orders} o, {matching} m WHERE o.id = m.id"
)]
pub struct CommaJoin {
    #[cte]
    pub matching: Matching,
}

#[query(
    Postgres,
    sql = "SELECT $${not a placeholder}$$ AS a, $tag$ {neither} $tag$ AS b"
)]
pub struct Dollars;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(
    Postgres,
    sql = "SELECT id, email FROM {Users} WHERE org_id = {org_id}"
)]
pub struct ActiveUsers {
    pub org_id: i64,
}

#[query(
    Postgres,
    sql = "
    SELECT u.email, (SELECT count(*) FROM {active}) AS org_size
    FROM {active} u
    WHERE u.email LIKE {pattern}"
)]
pub struct Report {
    pub pattern: String,
    #[cte]
    pub active: ActiveUsers,
}

#[query(
    Postgres,
    sql = "SELECT a.id FROM {first} a JOIN {second} b ON a.id = b.id"
)]
pub struct Pair {
    #[cte]
    pub first: ActiveUsers,
    #[cte]
    pub second: ActiveUsers,
}

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[query(
    T::Dialect,
    sql = "SELECT count(*) FROM {of} JOIN {Orders} o USING (id) WHERE o.total > {total}"
)]
pub struct WithOrders<T: Sql> {
    pub total: i64,
    #[subquery]
    pub of: T,
}

#[query(
    Postgres,
    sql = "SELECT {CountOf<Users>} AS users, {n} AS big_spenders"
)]
pub struct Totals {
    #[subquery]
    pub n: WithOrders<ActiveUsers>,
}

#[test]
fn fields_are_the_parameters() {
    assert_eq!(
        Search::SQL,
        r#"SELECT id FROM "orders" WHERE created_at BETWEEN $1 AND $2 AND status = $3 AND customer_id IN ($4, $5) AND lower(note) LIKE lower($6) ORDER BY id LIMIT $7 OFFSET $8"#
    );
    let binds: Vec<String> = Search::BINDS.iter().map(ToString::to_string).collect();
    assert_eq!(
        binds,
        [
            "search.since: i64",
            "search.until: i64",
            "search.status: String",
            "search.customer: i64",
            "search.other_customer: i64",
            "search.note: String",
            "search.limit: i64",
            "search.offset: i64",
        ]
    );
    assert_eq!(
        ByType::SQL,
        r#"SELECT id FROM "orders" WHERE "type" = $1 AND build = $2"#
    );
}

#[test]
fn a_field_used_twice_binds_once_on_numbered_dialects() {
    assert_eq!(
        EitherStatus::SQL,
        r#"SELECT id FROM "orders" WHERE status = $1 OR previous_status = $1"#
    );
    assert_eq!(EitherStatus::BINDS.len(), 1);
}

#[test]
fn an_item_field_is_one_instance_however_often_it_is_used() {
    assert_eq!(
        Report::SQL,
        r#"WITH "active_users" AS (SELECT id, email FROM "users" WHERE org_id = $1) SELECT u.email, (SELECT count(*) FROM "active_users") AS org_size FROM "active_users" u WHERE u.email LIKE $2"#
    );
    let binds: Vec<String> = Report::BINDS.iter().map(ToString::to_string).collect();
    assert_eq!(
        binds,
        ["active_users.org_id: i64", "report.pattern: String"]
    );
    assert_eq!(
        CommaJoin::SQL,
        r#"WITH "matching" AS (SELECT id FROM "orders" WHERE status = $1) SELECT o.id FROM "orders" o, "matching" m WHERE o.id = m.id"#
    );
}

#[test]
fn two_fields_of_one_type_are_two_instances() {
    assert_eq!(
        Pair::SQL,
        r#"WITH "active_users" AS (SELECT id, email FROM "users" WHERE org_id = $1), "active_users_2" AS (SELECT id, email FROM "users" WHERE org_id = $2) SELECT a.id FROM "active_users" a JOIN "active_users_2" b ON a.id = b.id"#
    );
    let paths: Vec<Vec<u16>> = Pair::BINDS
        .iter()
        .map(|bind| bind.path().steps().to_vec())
        .collect();
    assert_eq!(paths, [vec![0], vec![1]]);
}

#[test]
fn generic_items_hold_their_arguments() {
    assert_eq!(
        Totals::SQL,
        r#"SELECT (SELECT count(*) FROM "users") AS users, (SELECT count(*) FROM (SELECT id, email FROM "users" WHERE org_id = $1) AS "active_users" JOIN "orders" o USING (id) WHERE o.total > $2) AS big_spenders"#
    );
    let paths: Vec<Vec<u16>> = Totals::BINDS
        .iter()
        .map(|bind| bind.path().steps().to_vec())
        .collect();
    assert_eq!(paths, [vec![0, 0], vec![0]]);
}

#[test]
fn dollar_strings_are_left_alone() {
    assert_eq!(
        Dollars::SQL,
        "SELECT $${not a placeholder}$$ AS a, $tag$ {neither} $tag$ AS b"
    );
}

#[table(Sqlite, name = "events")]
pub struct Events;

#[query(
    Sqlite,
    sql = "
    SELECT count(*) AS n FROM {Events}
    WHERE kind = {kind} AND at BETWEEN {since} AND {until}"
)]
pub struct CountEvents {
    pub kind: String,
    pub since: i64,
    pub until: i64,
}

#[tokio::test]
async fn queries_bind_their_fields() -> sqlx::Result<()> {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql(
        "CREATE TABLE events (kind TEXT, at INTEGER);
         INSERT INTO events VALUES ('a', 1), ('a', 5), ('b', 5), ('a', 9);",
    )
    .execute(&mut conn)
    .await?;
    let count = CountEvents {
        kind: "a".to_owned(),
        since: 2,
        until: 9,
    };
    let row = count.query()?.fetch_one(&mut conn).await?;
    assert_eq!(row.get::<i64, _>("n"), 2);
    let row = count.query()?.fetch_one(&mut conn).await?;
    assert_eq!(row.get::<i64, _>("n"), 2);
    Ok(())
}

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct EventRow {
    pub kind: String,
    pub at: i64,
}

#[query(
    Sqlite,
    row = EventRow,
    sql = "SELECT kind, at FROM {Events} WHERE at >= {since} ORDER BY at"
)]
pub struct EventsSince {
    pub since: i64,
}

#[query(
    Sqlite,
    row = EventRow,
    sql = "INSERT INTO {Events} (kind, at) VALUES ({kind}, {at}) RETURNING kind, at"
)]
pub struct AddEvent {
    pub kind: String,
    pub at: i64,
}

#[tokio::test]
async fn rows_are_typed_by_row_or_query_as() -> sqlx::Result<()> {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql("CREATE TABLE events (kind TEXT, at INTEGER); INSERT INTO events VALUES ('a', 1), ('b', 5);")
        .execute(&mut conn)
        .await?;

    let added = AddEvent {
        kind: "c".to_owned(),
        at: 9,
    }
    .query()?
    .fetch_one(&mut conn)
    .await?;
    assert_eq!(
        added,
        EventRow {
            kind: "c".to_owned(),
            at: 9
        }
    );

    let rows = EventsSince { since: 5 }
        .query()?
        .fetch_all(&mut conn)
        .await?;
    assert_eq!(
        rows,
        [
            EventRow {
                kind: "b".to_owned(),
                at: 5
            },
            EventRow {
                kind: "c".to_owned(),
                at: 9
            },
        ]
    );

    let (n,): (i64,) = CountEvents {
        kind: "a".to_owned(),
        since: 0,
        until: 10,
    }
    .query_as()?
    .fetch_one(&mut conn)
    .await?;
    assert_eq!(n, 1);
    Ok(())
}

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct Count {
    pub n: i64,
}

#[query(Sqlite, sql = "SELECT kind FROM {Events} WHERE at > {since}")]
pub struct RecentEvents {
    pub since: i64,
}

#[query(T::Dialect, sql = "SELECT count(*) AS n FROM {of}")]
pub struct Tally<T: Sql> {
    #[cte]
    pub of: T,
}

#[statement(tally, row = Count)]
pub struct RecentTally {
    pub tally: Tally<RecentEvents>,
}

#[statement(CountOf<Events>)]
pub struct EventTally;

#[tokio::test]
async fn statements_run_generic_instantiations() -> sqlx::Result<()> {
    assert_eq!(
        RecentTally::SQL,
        r#"WITH "recent_events" AS (SELECT kind FROM "events" WHERE at > $1) SELECT count(*) AS n FROM "recent_events""#
    );
    assert_eq!(EventTally::SQL, r#"SELECT count(*) FROM "events""#);
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql("CREATE TABLE events (kind TEXT, at INTEGER); INSERT INTO events VALUES ('a', 1), ('b', 5), ('c', 9);")
        .execute(&mut conn)
        .await?;
    let recent = RecentTally {
        tally: Tally {
            of: RecentEvents { since: 4 },
        },
    };
    assert_eq!(recent.query()?.fetch_one(&mut conn).await?, Count { n: 2 });
    let (total,): (i64,) = EventTally.query_as()?.fetch_one(&mut conn).await?;
    assert_eq!(total, 3);
    Ok(())
}
