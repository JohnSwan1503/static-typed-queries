use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[table(Sqlite, name = "events")]
pub struct Events;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T} JOIN {Users} USING (id)")]
pub struct JoinUsers<T: Sql>(PhantomData<T>);

#[query(Sqlite, sql = "SELECT * FROM {JoinUsers<Events>}")]
pub struct Joined;

fn main() {}
