use static_typed_queries::dialect::mysql::MySql;
use static_typed_queries::dialect::postgres::Postgres;
use static_typed_queries::node::kind::Kind;
use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(
    Postgres,
    sql = "
    SELECT id FROM {Users} -- line comments are dropped
    WHERE org_id = {org_id: i64} OR parent_org_id = {org_id}
      AND note <> '{not a placeholder}'"
)]
pub struct Members;

#[table(MySql, name = "users")]
pub struct MyUsers;

#[query(
    MySql,
    sql = "SELECT id FROM {MyUsers} WHERE org_id = {org_id: i64} OR parent_org_id = {org_id}"
)]
pub struct MyMembers;

#[query(Postgres, cte, sql = "SELECT id FROM {Users} WHERE active")]
pub struct ActiveIds;

#[query(
    Postgres,
    sql = "
    SELECT * FROM {ActiveIds as subquery} JOIN {ActiveIds as subquery} a USING (id)
    WHERE id IN {ActiveIds}"
)]
pub struct Placements;

#[query(
    Postgres,
    sql = "UPDATE {Users} SET seen_at = now() WHERE id = {id: i64}"
)]
pub struct MarkSeen;

#[query(
    Postgres,
    sql = "INSERT INTO {Users} (email) VALUES ({email: String}) RETURNING id"
)]
pub struct AddUser;

#[test]
fn params_are_declared_once_and_reused_by_name() {
    assert_eq!(
        Members::SQL,
        r#"SELECT id FROM "users" WHERE org_id = $1 OR parent_org_id = $1 AND note <> '{not a placeholder}'"#
    );
    let _ = MembersParams {
        org_id: 7,
        users: (),
    };
}

#[test]
fn positional_dialects_bind_every_use() {
    assert_eq!(
        MyMembers::SQL,
        "SELECT id FROM `users` WHERE org_id = ? OR parent_org_id = ?"
    );
    assert_eq!(MyMembers::BINDS.len(), 2);
}

#[test]
fn placement_follows_position_and_overrides() {
    assert_eq!(
        Placements::SQL,
        r#"SELECT * FROM (SELECT id FROM "users" WHERE active) AS "active_ids" JOIN (SELECT id FROM "users" WHERE active) a USING (id) WHERE id IN (SELECT id FROM "users" WHERE active)"#
    );
}

#[test]
fn statement_kind_comes_from_the_parsed_sql() {
    assert_eq!(Members::NODE.kind, Kind::Query);
    assert_eq!(MarkSeen::NODE.kind, Kind::Dml);
    assert_eq!(
        MarkSeen::SQL,
        r#"UPDATE "users" SET seen_at = now() WHERE id = $1"#
    );
    assert_eq!(
        AddUser::SQL,
        r#"INSERT INTO "users" (email) VALUES ($1) RETURNING id"#
    );
}
