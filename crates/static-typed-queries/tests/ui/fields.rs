use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[table(Postgres, name = "accounts")]
pub struct Accounts {
    pub id: i64,
}

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE org_id = {org_id}")]
pub struct Unused {
    pub org_id: i64,
    pub name: String,
}

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE org_id = {org_id}")]
pub struct Unnamed(i64);

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[statement(CountOf<Users>)]
pub struct UserCount {
    pub org_id: i64,
}

fn main() {}
