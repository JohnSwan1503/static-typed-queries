use std::marker::PhantomData;

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
