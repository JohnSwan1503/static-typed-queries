use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[statement(CountOf<Users>, sql = "SELECT 1")]
pub struct UserCount;

fn main() {}
