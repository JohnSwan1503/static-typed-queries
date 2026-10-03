use static_typed_queries::dialect::postgres::Postgres;
use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, cte, sql = "SELECT id FROM {Users} WHERE active")]
pub struct ActiveIds;

#[query(
    Postgres,
    sql = "
    SELECT * FROM {ActiveIds as subquery} JOIN {ActiveIds as subquery} a USING (id)
    WHERE id IN {ActiveIds}"
)]
pub struct Placements;

#[test]
fn placement_follows_position_and_overrides() {
    assert_eq!(
        Placements::SQL,
        r#"SELECT * FROM (SELECT id FROM "users" WHERE active) AS "active_ids" JOIN (SELECT id FROM "users" WHERE active) a USING (id) WHERE id IN (SELECT id FROM "users" WHERE active)"#
    );
}
