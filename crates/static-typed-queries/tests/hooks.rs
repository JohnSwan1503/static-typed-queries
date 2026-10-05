use std::marker::PhantomData;

use static_typed_queries::__private::builder::{Hooked, NoHooks, WithHooks};
use static_typed_queries::prelude::*;

#[query(
    Postgres,
    sql = "SELECT set_config('app.tenant', {tenant: String}, true)"
)]
pub struct SetTenant;

#[query(Postgres, sql = "INSERT INTO audit (at) VALUES (now())")]
pub struct Audit;

#[table(Postgres, name = "orders", before(SetTenant), after(Audit))]
pub struct Orders;

#[table(Postgres, name = "events", after(Audit))]
pub struct Events;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE status = {_: String}")]
pub struct ByStatus;

#[query(Postgres, cte, sql = "SELECT id FROM {Orders} WHERE total > {_: i64}")]
pub struct BigOrders;

#[query(Postgres, sql = "SELECT count(*) FROM {BigOrders}")]
pub struct BigOrderCount;

#[query(Postgres, sql = "SELECT count(*) FROM {Events}")]
pub struct EventCount;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[statement(CountOf<Orders>)]
pub struct OrderCount;

#[statement(CountOf<Users>)]
pub struct UserCount;

fn hooked<T: Hooked<Out = WithHooks>>() {}

fn unhooked<T: Hooked<Out = NoHooks>>() {}

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
fn hook_parameters_are_set_through_the_table() {
    let params = ByStatus::builder()
        .status("open".to_owned())
        .orders()
        .set_tenant()
        .tenant("acme".to_owned())
        .build();
    assert_eq!(params.status, "open");
    assert_eq!(params.orders.set_tenant.tenant, "acme");
    assert_eq!(ByStatus::BEFORE[0].binds()[0].path().steps(), [0, 0]);

    let params = EventCount::builder().build();
    assert_eq!(params.events.audit, ());
}

#[test]
fn hooks_mark_every_statement_that_reaches_the_table() {
    hooked::<Orders>();
    hooked::<Events>();
    hooked::<ByStatus>();
    hooked::<BigOrderCount>();
    hooked::<EventCount>();
    hooked::<OrderCount>();
    unhooked::<Users>();
    unhooked::<SetTenant>();
    unhooked::<CountOf<Orders>>();
    unhooked::<UserCount>();
}
