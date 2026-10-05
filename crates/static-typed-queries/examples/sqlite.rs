use sqlx::{Connection, FromRow, SqliteConnection};
use static_typed_queries::prelude::*;

#[table(Sqlite, name = "users")]
pub struct Users;

#[table(Sqlite, name = "orders")]
pub struct Orders;

#[query(
    Sqlite,
    sql = "
    SELECT id, email FROM {Users}
    WHERE org_id = {org_id} AND deleted_at IS NULL"
)]
pub struct ActiveUsers {
    pub org_id: i64,
}

#[query(Sqlite, display = sql, debug = sql, sql = "
    SELECT u.email,
           (SELECT count(*) FROM {active}) AS org_size,
           (SELECT count(*) FROM {Orders} o
             WHERE o.user_id = u.id AND o.total >= {big_total}) AS big_orders
    FROM {active} u
    WHERE u.email LIKE {pattern}
    ORDER BY u.email", row = ReportRow)]
pub struct UserReport {
    pub big_total: i64,
    pub pattern: String,
    #[cte]
    pub active: ActiveUsers,
}

#[derive(FromRow)]
pub struct ReportRow {
    pub email: String,
    pub org_size: i64,
    pub big_orders: i64,
}

const SCHEMA: &str = "
    CREATE TABLE users (id INTEGER PRIMARY KEY, org_id INTEGER, email TEXT, deleted_at TEXT);
    CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER, total INTEGER);
    INSERT INTO users VALUES
        (1, 1, 'ada@example.com', NULL),
        (2, 1, 'brian@example.com', NULL),
        (3, 1, 'carol@example.org', NULL),
        (4, 1, 'dave@example.com', '2026-01-01'),
        (5, 2, 'erin@example.com', NULL);
    INSERT INTO orders VALUES (1, 1, 250), (2, 1, 40), (3, 2, 120), (4, 3, 500), (5, 5, 900);
";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), sqlx::Error> {
    println!(
        "-- UserReport::SQL, built at compile time:\n{}\n",
        UserReport::SQL
    );
    println!("-- binds:");
    for (i, bind) in UserReport::BINDS.iter().enumerate() {
        println!("   ${} {bind}", i + 1);
    }
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql(SCHEMA).execute(&mut conn).await?;

    let report = UserReport {
        big_total: 100,
        pattern: "%@example.com".to_owned(),
        active: ActiveUsers { org_id: 1 },
    };
    let rows = report.query()?.fetch_all(&mut conn).await?;

    println!("\n-- results of {report:?}:");
    for row in rows {
        println!(
            "   {:<20} org_size={} big_orders={}",
            row.email, row.org_size, row.big_orders
        );
    }
    Ok(())
}
