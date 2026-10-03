#[macro_use]
mod support;

use sqlx::error::BoxDynError;
use sqlx::{Arguments as _, Connection, Database, Encode, Row, SqliteConnection, Type};
use static_typed_queries_core::dialect::mysql::MySql;
use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::dialect::sqlite::Sqlite;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::expr::Expr;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::ident::Ident;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::part::param::Param;
use static_typed_queries_core::statement::Statement;
use static_typed_queries_core::statement::bind::Bind;
use static_typed_queries_core::statement::params::{BindParams, unknown};

const ORG_EVENTS: &Node = node!(
    "org_events",
    1,
    Query,
    Inject::cte(false),
    [
        Lit::part("SELECT id, kind FROM "),
        Ident::part("events"),
        Lit::part(" WHERE org_id = "),
        Param::part(0, "org_id", "i64"),
    ]
);

const KIND_COUNT: &Node = node!(
    "kind_count",
    2,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT count(*) FROM "),
        From::part(ORG_EVENTS, AliasRule::NodeName, None),
        Lit::part(" WHERE kind = "),
        Param::part(0, "kind", "&'static str"),
    ]
);

const REPORT: &Node = node!(
    "report",
    3,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT "),
        Expr::part(KIND_COUNT),
        Lit::part(" AS first, (SELECT max(id) FROM "),
        From::part(ORG_EVENTS, AliasRule::NodeName, None),
        Lit::part(" WHERE id < "),
        Param::part(0, "below", "i64"),
        Lit::part(" AND id <> "),
        Param::part(0, "below", "i64"),
        Lit::part(") AS below"),
    ]
);

pub struct OrgEventsParams {
    pub org_id: i64,
}

pub struct KindCountParams {
    pub kind: &'static str,
    pub org_events: OrgEventsParams,
}

pub struct ReportParams {
    pub below: i64,
    pub kind_count: KindCountParams,
    pub org_events: OrgEventsParams,
}

impl<DB: Database> BindParams<DB> for OrgEventsParams
where
    for<'t> i64: Encode<'t, DB> + Type<DB>,
{
    fn bind(&self, path: &[u16], slot: u16, args: &mut DB::Arguments) -> Result<(), BoxDynError> {
        match (path, slot) {
            ([], 0) => args.add(self.org_id),
            _ => Err(unknown(path, slot)),
        }
    }
}

impl<DB: Database> BindParams<DB> for KindCountParams
where
    for<'t> &'static str: Encode<'t, DB> + Type<DB>,
    OrgEventsParams: BindParams<DB>,
{
    fn bind(&self, path: &[u16], slot: u16, args: &mut DB::Arguments) -> Result<(), BoxDynError> {
        match (path, slot) {
            ([], 0) => args.add(self.kind),
            ([0, rest @ ..], _) => self.org_events.bind(rest, slot, args),
            _ => Err(unknown(path, slot)),
        }
    }
}

impl<DB: Database> BindParams<DB> for ReportParams
where
    for<'t> i64: Encode<'t, DB> + Type<DB>,
    KindCountParams: BindParams<DB>,
    OrgEventsParams: BindParams<DB>,
{
    fn bind(&self, path: &[u16], slot: u16, args: &mut DB::Arguments) -> Result<(), BoxDynError> {
        match (path, slot) {
            ([], 0) => args.add(self.below),
            ([0, rest @ ..], _) => self.kind_count.bind(rest, slot, args),
            ([1, rest @ ..], _) => self.org_events.bind(rest, slot, args),
            _ => Err(unknown(path, slot)),
        }
    }
}

root!(ReportPostgres: Postgres = REPORT, params = ReportParams);
root!(ReportMySql: MySql = REPORT, params = ReportParams);
root!(ReportSqlite: Sqlite = REPORT, params = ReportParams);
root!(EventCount: Sqlite = node!(
    "event_count",
    4,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT count(*) AS n FROM "), Ident::part("events")]
));

const SHARED: &Node = node!(
    "shared",
    5,
    Query,
    Inject::cte(false),
    [Lit::part("SELECT 1 AS id")]
);

const USES_SHARED: &Node = node!(
    "uses_shared",
    6,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT id FROM "),
        From::part(SHARED, AliasRule::NodeName, None),
    ]
);

root!(SharedTwice: Postgres = node!(
    "shared_twice",
    7,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT * FROM "),
        From::part(SHARED, AliasRule::Given, None),
        Lit::part(" s WHERE s.id IN "),
        Expr::part(USES_SHARED),
    ]
));

#[test]
fn items_with_params_get_one_instance_per_path() {
    assert_eq!(
        ReportPostgres::SQL,
        r#"WITH "org_events" AS (SELECT id, kind FROM "events" WHERE org_id = $1), "org_events_2" AS (SELECT id, kind FROM "events" WHERE org_id = $2) SELECT (SELECT count(*) FROM "org_events" WHERE kind = $3) AS first, (SELECT max(id) FROM "org_events_2" WHERE id < $4 AND id <> $4) AS below"#
    );
    assert_eq!(
        ReportPostgres::BINDS,
        [
            Bind::from_native("org_events", 1, &[0, 0], 0, "org_id", "i64"),
            Bind::from_native("org_events", 1, &[1], 0, "org_id", "i64"),
            Bind::from_native("kind_count", 2, &[0], 0, "kind", "&'static str"),
            Bind::from_native("report", 3, &[], 0, "below", "i64"),
        ]
    );
    assert_eq!(
        ReportPostgres::BINDS[2].to_string(),
        "kind_count.kind: &'static str"
    );
}

#[test]
fn positional_binds_repeat_for_every_placeholder() {
    assert_eq!(
        ReportMySql::BINDS,
        [
            Bind::from_native("org_events", 1, &[0, 0], 0, "org_id", "i64"),
            Bind::from_native("org_events", 1, &[1], 0, "org_id", "i64"),
            Bind::from_native("kind_count", 2, &[0], 0, "kind", "&'static str"),
            Bind::from_native("report", 3, &[], 0, "below", "i64"),
            Bind::from_native("report", 3, &[], 0, "below", "i64"),
        ]
    );
}

#[test]
fn ctes_without_params_are_shared_across_paths() {
    assert_eq!(
        SharedTwice::SQL,
        r#"WITH "shared" AS (SELECT 1 AS id) SELECT * FROM "shared" s WHERE s.id IN (SELECT id FROM "shared")"#
    );
}

#[tokio::test]
async fn typed_params_bind_against_sqlite() -> sqlx::Result<()> {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql(
        "CREATE TABLE events (id INTEGER PRIMARY KEY, org_id INTEGER NOT NULL, kind TEXT NOT NULL);
         INSERT INTO events VALUES (1, 1, 'a'), (2, 1, 'b'), (3, 2, 'a'), (4, 1, 'a'), (5, 2, 'b');",
    )
    .execute(&mut conn)
    .await?;

    let params = ReportParams {
        below: 5,
        kind_count: KindCountParams {
            kind: "a",
            org_events: OrgEventsParams { org_id: 1 },
        },
        org_events: OrgEventsParams { org_id: 2 },
    };
    let row = ReportSqlite::query(&params)?.fetch_one(&mut conn).await?;
    assert_eq!(
        (row.get::<i64, _>("first"), row.get::<i64, _>("below")),
        (2, 3)
    );

    let row = EventCount::query(&())?.fetch_one(&mut conn).await?;
    assert_eq!(row.get::<i64, _>("n"), 5);
    let row = sqlx::query(EventCount).fetch_one(&mut conn).await?;
    assert_eq!(row.get::<i64, _>("n"), 5);
    Ok(())
}
