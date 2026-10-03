use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[query(Postgres, separate(CountOf<Users>), sql = "SELECT id FROM {Users}")]
pub struct Ids;

fn main() {}
