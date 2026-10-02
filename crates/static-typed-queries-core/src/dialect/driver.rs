use super::Dialect;

pub trait Driver: Dialect {
    type Database: sqlx::Database;
}

pub type Database<D> = <D as Driver>::Database;
pub type Arguments<D> = <<D as Driver>::Database as sqlx::Database>::Arguments;
pub type Query<'q, D> = sqlx::query::Query<'q, Database<D>, Arguments<D>>;
