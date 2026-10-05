use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE org_id = {org_id}")]
pub struct ActiveUsers {
    pub org_id: i64,
}

#[query(Postgres, sql = "SELECT count(*) FROM {ActiveUsers}")]
pub struct ByType;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[query(Postgres, sql = "SELECT {CountOf<ActiveUsers>} AS n")]
pub struct Counted;

#[statement(ActiveUsers)]
pub struct Run;

#[transaction(Postgres, steps(ActiveUsers))]
pub struct Steps;

fn main() {}
