use std::marker::PhantomData;

use static_typed_queries::dialect::postgres::Postgres;
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
        .count_of_active_users(|b| b.t(|b| b.org_id(1)))
        .active_users(|b| b.org_id(2))
        .build();
    assert_eq!(params.count_of_active_users.t.org_id, 1);
    assert_eq!(params.active_users.org_id, 2);

    let join = CommaJoin::builder()
        .matching(|b| b.status("paid".to_owned()))
        .build();
    assert_eq!(join.matching.status, "paid");
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
