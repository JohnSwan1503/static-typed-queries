use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[table(Sqlite, name = "events")]
pub struct Events;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[query(Postgres, sql = "SELECT {CountOf<Events>} AS events FROM {Users}")]
pub struct Mixed;

fn main() {}
