use static_typed_queries::__private::node::kind::Kind;
use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(
    Postgres,
    sql = "
    SELECT id FROM {Users} -- line comments are dropped
    WHERE org_id = {org_id} OR parent_org_id = {org_id}
      AND note <> '{not a placeholder}'"
)]
pub struct Members {
    pub org_id: i64,
}

#[table(MySql, name = "users")]
pub struct MyUsers;

#[query(
    MySql,
    sql = "SELECT id FROM {MyUsers} WHERE org_id = {org_id} OR parent_org_id = {org_id}"
)]
pub struct MyMembers {
    pub org_id: i64,
}

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE active")]
pub struct ActiveIds;

#[query(
    Postgres,
    sql = "
    SELECT * FROM {ActiveIds} JOIN {ActiveIds as cte} a USING (id)
    WHERE id IN {ActiveIds}"
)]
pub struct Placements;

#[query(Postgres, sql = "UPDATE {Users} SET seen_at = now() WHERE id = {id}")]
pub struct MarkSeen {
    pub id: i64,
}

#[query(
    Postgres,
    sql = "INSERT INTO {Users} (email) VALUES ({email}) RETURNING id"
)]
pub struct AddUser {
    pub email: String,
}

#[test]
fn fields_are_reused_by_name() {
    assert_eq!(
        Members::SQL,
        r#"SELECT id FROM "users" WHERE org_id = $1 OR parent_org_id = $1 AND note <> '{not a placeholder}'"#
    );
    assert_eq!(Members::BINDS.len(), 1);
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
        r#"WITH "active_ids" AS (SELECT id FROM "users" WHERE active) SELECT * FROM (SELECT id FROM "users" WHERE active) AS "active_ids" JOIN "active_ids" a USING (id) WHERE id IN (SELECT id FROM "users" WHERE active)"#
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

#[test]
fn queries_inside_functions_can_skip_the_parse_check() {
    #[query(
        Postgres,
        parse_check = false,
        sql = "SELECT id FROM {Users} WHERE id = {id}"
    )]
    pub struct Local {
        pub id: i64,
    }

    assert_eq!(Local::SQL, r#"SELECT id FROM "users" WHERE id = $1"#);
}

pub type Db = Postgres;

#[query(
    Db,
    grammar = postgres,
    sql = "SELECT id FROM {Users} WHERE id = ANY({ids})"
)]
pub struct AnyOf {
    pub ids: Vec<i64>,
}

#[test]
fn dialect_aliases_name_their_grammar() {
    assert_eq!(AnyOf::SQL, r#"SELECT id FROM "users" WHERE id = ANY($1)"#);
}
