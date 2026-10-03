use std::marker::PhantomData;

use sqlx::{Connection, Row, SqliteConnection};
use static_typed_queries::dialect::postgres::Postgres;
use static_typed_queries::dialect::sqlite::Sqlite;
use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(
    Postgres,
    sql = "
    SELECT id FROM {Orders}
    WHERE created_at BETWEEN {_: i64} AND {_: i64}
      AND status = {_: String}
      AND customer_id IN ({_: i64}, {_: i64})
      AND lower(note) LIKE lower({_: String})
    ORDER BY id
    LIMIT {_: i64} OFFSET {_: i64}"
)]
pub struct Search;

#[query(
    Postgres,
    sql = "
    INSERT INTO {Orders} (customer_id, total) VALUES ({_: i64}, {_: i64}) RETURNING id"
)]
pub struct PlaceOrder;

#[query(
    Postgres,
    sql = "UPDATE {Orders} SET status = {_: String} WHERE id = {_: i64}"
)]
pub struct SetStatus;

#[query(
    Postgres,
    sql = "
    SELECT {_: i64} AS answer, coalesce({_: i64}, 0) AS fallback, {total: i64} + {_: i64} AS sum"
)]
pub struct Misc;

#[query(
    Postgres,
    sql = "
    SELECT id FROM {Orders} WHERE status = {status: String} OR status = {_: String}"
)]
pub struct EitherStatus;

#[query(
    Postgres,
    sql = r#"SELECT id FROM {Orders} WHERE "type" = {_: String}"#
)]
pub struct ByType;

#[query(
    Postgres,
    cte,
    sql = "SELECT id FROM {Orders} WHERE status = {_: String}"
)]
pub struct Matching;

#[query(
    Postgres,
    sql = "SELECT o.id FROM {Orders} o, {Matching} m WHERE o.id = m.id"
)]
pub struct CommaJoin;

#[query(
    Postgres,
    sql = "SELECT $${not a placeholder}$$ AS a, $tag$ {neither} $tag$ AS b"
)]
pub struct Dollars;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(
    Postgres,
    cte,
    sql = "SELECT id, email FROM {Users} WHERE org_id = {_: i64}"
)]
pub struct ActiveUsers;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[query(
    Postgres,
    sql = "
    SELECT u.email, {CountOf<ActiveUsers>} AS org_size
    FROM {ActiveUsers} u
    WHERE u.email LIKE {_: String}"
)]
pub struct Report;

#[query(
    Postgres,
    sql = "
    SELECT {CountOf<Users>} AS users_total, {CountOf<Orders>}, {CountOf<Matching>}
    FROM {ActiveUsers} au"
)]
pub struct ItemNames;

#[test]
fn names_come_from_the_sql_and_collisions_need_repeated_calls() {
    let params = Search::builder()
        .limit(50)
        .created_at(1)
        .status("paid".to_owned())
        .customer_id(10)
        .created_at(2)
        .note("%gift%".to_owned())
        .customer_id(11)
        .offset(100)
        .build();
    assert_eq!(params.created_at, [1, 2]);
    assert_eq!(params.customer_id, [10, 11]);
    assert_eq!((params.limit, params.offset), (50, 100));
    assert_eq!(
        Search::SQL,
        r#"SELECT id FROM "orders" WHERE created_at BETWEEN $1 AND $2 AND status = $3 AND customer_id IN ($4, $5) AND lower(note) LIKE lower($6) ORDER BY id LIMIT $7 OFFSET $8"#
    );
}

#[test]
fn binds_name_the_field_and_type_they_come_from() {
    let binds: Vec<String> = Search::BINDS.iter().map(ToString::to_string).collect();
    assert_eq!(
        binds,
        [
            "search.created_at[0]: i64",
            "search.created_at[1]: i64",
            "search.status: String",
            "search.customer_id[0]: i64",
            "search.customer_id[1]: i64",
            "search.note: String",
            "search.limit: i64",
            "search.offset: i64",
        ]
    );
    let binds: Vec<String> = Report::BINDS.iter().map(ToString::to_string).collect();
    assert_eq!(
        binds,
        [
            "active_users.org_id: i64",
            "active_users.org_id: i64",
            "report.email: String",
        ]
    );
}

#[test]
fn insert_columns_update_targets_and_aliases_name_params() {
    let insert = PlaceOrder::builder().customer_id(1).total(500).build();
    assert_eq!((insert.customer_id, insert.total), (1, 500));

    let update = SetStatus::builder()
        .id(7)
        .status("shipped".to_owned())
        .build();
    assert_eq!((update.id, update.status.as_str()), (7, "shipped"));

    let misc = Misc::builder()
        .answer(42)
        .bind2(1)
        .total(3)
        .bind4(4)
        .build();
    assert_eq!(
        (misc.answer, misc.bind2, misc.total, misc.bind4),
        (42, 1, 3, 4)
    );

    let either = EitherStatus::builder()
        .status("paid".to_owned())
        .status("refunded".to_owned())
        .build();
    assert_eq!(either.status, ["paid".to_owned(), "refunded".to_owned()]);

    let by_type = ByType::builder().r#type("gift".to_owned()).build();
    assert_eq!(by_type.r#type, "gift");
}

#[test]
fn nested_items_take_closures_and_tables_need_no_call() {
    let params = Report::builder()
        .email("%@example.com".to_owned())
        .org_size()
        .t()
        .org_id(1)
        .active_users()
        .org_id(2)
        .build();
    assert_eq!(params.org_size.t.org_id, 1);
    assert_eq!(params.active_users.org_id, 2);

    let join = CommaJoin::builder()
        .matching()
        .status("paid".to_owned())
        .build();
    assert_eq!(join.matching.status, "paid");
}

#[test]
fn items_are_named_by_alias_then_type_then_full_type() {
    let params = ItemNames::builder()
        .count_of_matching()
        .t()
        .status("paid".to_owned())
        .au()
        .org_id(3)
        .build();
    let ((), ()) = (params.users_total.t, params.count_of_orders.t);
    assert_eq!(params.count_of_matching.t.status, "paid");
    assert_eq!(params.au.org_id, 3);
}

#[test]
fn comma_joins_and_dollar_strings() {
    assert_eq!(
        CommaJoin::SQL,
        r#"WITH "matching" AS (SELECT id FROM "orders" WHERE status = $1) SELECT o.id FROM "orders" o, "matching" m WHERE o.id = m.id"#
    );
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
    WHERE kind = {_: String} AND at BETWEEN {_: i64} AND {_: i64}"
)]
pub struct CountEvents;

#[tokio::test]
async fn builder_queries_run_against_sqlite() -> sqlx::Result<()> {
    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql(
        "CREATE TABLE events (kind TEXT, at INTEGER);
         INSERT INTO events VALUES ('a', 1), ('a', 5), ('b', 5), ('a', 9);",
    )
    .execute(&mut conn)
    .await?;
    let row = CountEvents::builder()
        .kind("a".to_owned())
        .at(2)
        .at(9)
        .query()?
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(row.get::<i64, _>("n"), 2);
    Ok(())
}
