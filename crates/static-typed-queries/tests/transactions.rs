use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(
    Postgres,
    cte,
    sql = "SELECT id FROM {Orders} WHERE customer_id = {customer_id: i64}"
)]
pub struct CustomerOrders;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[query(
    Postgres,
    sql = "DELETE FROM {Orders} WHERE created_at < {before: i64}"
)]
pub struct Archive;

#[statement(CountOf<Orders>)]
pub struct OrderCount;

#[transaction(
    Postgres,
    steps(Archive, CountOf<CustomerOrders>, OrderCount, Archive)
)]
pub struct Nightly;

#[test]
fn steps_render_in_order() {
    let steps: Vec<(&str, &str)> = Nightly::STEPS
        .iter()
        .map(|step| (step.name().as_str(), step.sql()))
        .collect();
    assert_eq!(
        steps,
        [
            ("archive", Archive::SQL),
            (
                "count_of",
                r#"WITH "customer_orders" AS (SELECT id FROM "orders" WHERE customer_id = $1) SELECT count(*) FROM "customer_orders""#
            ),
            ("order_count", OrderCount::SQL),
            ("archive", Archive::SQL),
        ]
    );
}

#[test]
fn steps_take_their_values_from_the_transaction() {
    let params = Nightly::builder()
        .archive()
        .before(10)
        .customer_orders()
        .customer_id(7)
        .build();
    assert_eq!(params.archive.before, 10);
    assert_eq!(params.customer_orders.customer_id, 7);
}
