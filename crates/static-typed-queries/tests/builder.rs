use std::marker::PhantomData;

use sqlx::{Connection, Row, SqliteConnection};
use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(
    Postgres,
    sql = "
    SELECT id FROM {Orders}
    WHERE created_at BETWEEN {since} AND {until} AND status = {status}"
)]
pub struct Search {
    pub since: i64,
    pub until: i64,
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

#[query(
    Postgres,
    sql = "SELECT id FROM {Orders} WHERE a = {_1} AND b = {self_} AND c = {org_id} AND d = {orgId}"
)]
#[allow(non_snake_case)]
pub struct OddNames {
    pub _1: i64,
    pub self_: i64,
    pub org_id: i64,
    pub orgId: i64,
}

#[query(
    Postgres,
    sql = "SELECT id, email FROM {Users} WHERE org_id = {org_id}"
)]
pub struct ActiveUsers {
    pub org_id: i64,
}

#[query(
    Postgres,
    sql = "SELECT u.email FROM {active} u WHERE u.email LIKE {pattern}"
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

#[query(
    T::Dialect,
    sql = "SELECT count(*) FROM {of} JOIN {Orders} o USING (id) WHERE o.total > {total}"
)]
pub struct WithOrders<T: Sql> {
    pub total: i64,
    #[subquery]
    pub of: T,
}

#[query(Postgres, sql = "SELECT {n} AS big_spenders")]
pub struct Totals {
    #[subquery]
    pub n: WithOrders<ActiveUsers>,
}

#[query(T::Dialect, sql = "SELECT count(*) FROM {of}")]
pub struct Tally<T: Sql> {
    #[cte]
    pub of: T,
}

#[query(Postgres, sql = "SELECT {users} AS users")]
pub struct UserTotal {
    #[subquery]
    pub users: Tally<Users>,
}

#[query(T::Dialect, sql = "SELECT count(*) FROM {T} WHERE id > {min}")]
pub struct AtLeast<T: Sql> {
    pub min: i64,
    _of: PhantomData<T>,
}

#[statement(count)]
pub struct ActiveCount {
    pub count: Tally<ActiveUsers>,
}

#[statement(count)]
pub struct UserCount {
    pub count: Tally<Users>,
}

#[query(Postgres, sql = "SELECT 1")]
pub struct One;

#[transaction(Postgres, steps(search, count, One))]
pub struct Nightly {
    pub search: Search,
    pub count: ActiveCount,
}

#[test]
fn setters_take_any_order_and_build_the_struct() {
    let search = Search::builder()
        .status("paid".to_owned())
        .until(2)
        .since(1)
        .build();
    assert_eq!((search.since, search.until), (1, 2));
    assert_eq!(search.status, "paid");

    let by_type = ByType::builder().r#type("gift".to_owned()).build(3).build();
    assert_eq!((by_type.r#type.as_str(), by_type.build), ("gift", 3));

    let odd = OddNames::builder()
        ._1(1)
        .self_(2)
        .org_id(3)
        .orgId(4)
        .build();
    assert_eq!((odd._1, odd.self_, odd.org_id, odd.orgId), (1, 2, 3, 4));
}

#[test]
fn an_item_field_moves_to_its_builder_for_one_setter() {
    let report = Report::builder()
        .active()
        .org_id(7)
        .pattern("%@example.com".to_owned())
        .build();
    assert_eq!(report.active.org_id, 7);
    assert_eq!(report.pattern, "%@example.com");

    let pair = Pair::builder().second().org_id(2).first().org_id(1).build();
    assert_eq!((pair.first.org_id, pair.second.org_id), (1, 2));
}

#[test]
fn scoped_setters_return_to_the_outermost_builder() {
    let totals = Totals::builder().n().of().org_id(3).n().total(100).build();
    assert_eq!(totals.n.total, 100);
    assert_eq!(totals.n.of.org_id, 3);

    let tally = Tally::<ActiveUsers>::builder().of().org_id(4).build();
    assert_eq!(tally.of.org_id, 4);
}

#[test]
fn items_without_values_start_built() {
    let total = UserTotal::builder().build();
    let Tally { of: Users } = total.users;
}

#[test]
fn phantom_fields_need_no_setter() {
    let at_least = AtLeast::<Users>::builder().min(5).build();
    assert_eq!(at_least.min, 5);
}

#[test]
fn statements_and_transactions_build_their_fields() {
    let active = ActiveCount::builder().count().of().org_id(7).build();
    assert_eq!(active.count.of.org_id, 7);
    let Tally { of: Users } = UserCount::builder().build().count;

    let nightly = Nightly::builder()
        .count()
        .count()
        .of()
        .org_id(1)
        .search()
        .since(1)
        .search()
        .until(2)
        .search()
        .status("paid".to_owned())
        .build();
    assert_eq!(nightly.count.count.of.org_id, 1);
    assert_eq!((nightly.search.since, nightly.search.until), (1, 2));
    assert_eq!(nightly.search.status, "paid");
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

#[query(Sqlite, sql = "UPDATE counters SET n = n + 1 WHERE name = {name}")]
pub struct Bump {
    pub name: String,
}

#[query(Sqlite, sql = "INSERT INTO audit (note) VALUES ({note})")]
pub struct Note {
    pub note: String,
}

#[query(Sqlite, sql = "INSERT INTO audit (note) VALUES ('read')")]
pub struct Audit;

#[table(Sqlite, name = "events", before(Bump), after(Note))]
pub struct CountedEvents;

#[table(Sqlite, name = "events", after(Audit))]
pub struct AuditedEvents;

#[derive(sqlx::FromRow, Debug, PartialEq)]
pub struct Event {
    pub kind: String,
    pub at: i64,
}

#[query(
    Sqlite,
    row = Event,
    sql = "SELECT kind, at FROM {CountedEvents} WHERE at >= {since} ORDER BY at"
)]
pub struct EventsSince {
    pub since: i64,
}

#[query(
    Sqlite,
    sql = "SELECT count(*) FROM {AuditedEvents} WHERE kind = {kind}"
)]
pub struct AuditedCount {
    pub kind: String,
}

#[query(
    Sqlite,
    sql = "INSERT INTO {CountedEvents} (kind, at) VALUES ({kind}, {at})"
)]
pub struct AddEvent {
    pub kind: String,
    pub at: i64,
}

#[transaction(Sqlite, steps(add, list))]
pub struct AddAndList {
    pub add: AddEvent,
    pub list: EventsSince,
}

async fn connect() -> sqlx::Result<SqliteConnection> {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql(
        "CREATE TABLE events (kind TEXT, at INTEGER);
         CREATE TABLE counters (name TEXT PRIMARY KEY, n INTEGER);
         CREATE TABLE audit (note TEXT);
         INSERT INTO events VALUES ('a', 1), ('a', 5), ('b', 5), ('a', 9);
         INSERT INTO counters VALUES ('reads', 0);",
    )
    .execute(&mut conn)
    .await?;
    Ok(conn)
}

async fn reads(conn: &mut SqliteConnection) -> sqlx::Result<(i64, i64)> {
    let (n,): (i64,) = sqlx::query_as("SELECT n FROM counters")
        .fetch_one(&mut *conn)
        .await?;
    let (audits,): (i64,) = sqlx::query_as("SELECT count(*) FROM audit")
        .fetch_one(&mut *conn)
        .await?;
    Ok((n, audits))
}

#[tokio::test]
async fn complete_builders_bind_their_values() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let row = CountEvents::builder()
        .kind("a".to_owned())
        .since(2)
        .until(9)
        .query()?
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(row.get::<i64, _>("n"), 2);

    let (n,): (i64,) = CountEvents::builder()
        .until(10)
        .since(0)
        .kind("b".to_owned())
        .query_as()?
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(n, 1);
    Ok(())
}

#[tokio::test]
async fn complete_builders_run_with_their_hooks() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let events = EventsSince::builder()
        .since(6)
        .with(Bump {
            name: "reads".to_owned(),
        })
        .with(Note {
            note: "since".to_owned(),
        })
        .run(&mut conn)
        .await?;
    assert_eq!(
        events,
        [Event {
            kind: "a".to_owned(),
            at: 9
        }]
    );
    assert_eq!(reads(&mut conn).await?, (1, 1));

    let counts: Vec<(i64,)> = AuditedCount::builder()
        .kind("a".to_owned())
        .run_as(&mut conn)
        .await?;
    assert_eq!(counts, [(3,)]);
    assert_eq!(reads(&mut conn).await?, (1, 2));

    let (added, listed) = AddAndList::builder()
        .list()
        .since(9)
        .add()
        .kind("c".to_owned())
        .add()
        .at(10)
        .with(Bump {
            name: "reads".to_owned(),
        })
        .with(Note {
            note: "add".to_owned(),
        })
        .run(&mut conn)
        .await?;
    assert_eq!(added, 1);
    assert_eq!(listed.len(), 2);
    assert_eq!(reads(&mut conn).await?, (2, 3));
    Ok(())
}

#[tokio::test]
async fn with_takes_a_value_or_a_complete_builder() -> sqlx::Result<()> {
    let mut conn = connect().await?;
    let events = EventsSince::builder()
        .since(9)
        .with(Bump::builder().name("reads".to_owned()))
        .with(Note::builder().note("builders".to_owned()))
        .run(&mut conn)
        .await?;
    assert_eq!(events.len(), 1);

    let events = EventsSince { since: 9 }
        .with(Bump::builder().name("reads".to_owned()))
        .with(Note {
            note: "mixed".to_owned(),
        })
        .run(&mut conn)
        .await?;
    assert_eq!(events.len(), 1);
    assert_eq!(reads(&mut conn).await?, (2, 2));
    Ok(())
}
