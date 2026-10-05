use sqlx::{Executor, IntoArguments};

use super::Dialect;

pub trait Driver: Dialect {
    type Database: sqlx::Database<Arguments: IntoArguments<Self::Database>>;

    // sqlx implements `Executor` for each connection type separately; generic code gets it here.
    fn executor(conn: &mut Connection<Self>) -> impl Executor<'_, Database = Self::Database>;

    fn rows_affected(result: &<Self::Database as sqlx::Database>::QueryResult) -> u64;
}

pub type Database<D> = <D as Driver>::Database;
pub type Arguments<D> = <<D as Driver>::Database as sqlx::Database>::Arguments;
pub type Connection<D> = <<D as Driver>::Database as sqlx::Database>::Connection;
pub type Query<'q, D> = sqlx::query::Query<'q, Database<D>, Arguments<D>>;
pub type QueryAs<'q, D, O> = sqlx::query::QueryAs<'q, Database<D>, O, Arguments<D>>;
pub type Row<D> = <<D as Driver>::Database as sqlx::Database>::Row;
