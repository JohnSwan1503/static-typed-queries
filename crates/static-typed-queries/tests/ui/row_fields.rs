use static_typed_queries::prelude::*;

#[table(Postgres, name = "accounts")]
pub struct Accounts {
    pub id: i64,
}

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id, u.email, 1 AS one FROM {Users} u")]
pub struct Unselected {
    pub id: i64,
    pub name: String,
}

#[query(Postgres, row = (i64,), sql = "SELECT id FROM {Users}")]
pub struct Both {
    pub id: i64,
}

#[query(Postgres, sql = "UPDATE {Users} SET email = {_: String}")]
pub struct NoRows {
    pub id: i64,
}

#[query(T::Dialect, sql = "SELECT id FROM {T}")]
pub struct Generic<T: Sql> {
    pub id: i64,
    marker: std::marker::PhantomData<T>,
}

#[query(Postgres, debug = sql, sql = "SELECT id FROM {Users}")]
pub struct Shown {
    pub id: i64,
}

fn main() {}
