use core::marker::PhantomData;

use sqlx::{FromRow, SqlStr};

use super::Rows;
use super::command::Command;
use super::params::{BindHooks, BindParams, arguments};
use crate::dialect::driver::{Connection, Database, Driver, Query, QueryAs, Row};

pub fn query<'q, D, P>(values: &P, statement: &Command) -> Result<Query<'q, D>, sqlx::Error>
where
    D: Driver,
    P: BindParams<Database<D>> + ?Sized,
{
    let args = arguments::<D, P>(values, statement.binds())?;
    Ok(sqlx::query_with(SqlStr::from_static(statement.sql()), args))
}

pub fn query_as<'q, D, O, P>(
    values: &P,
    statement: &Command,
) -> Result<QueryAs<'q, D, O>, sqlx::Error>
where
    D: Driver,
    O: for<'r> FromRow<'r, Row<D>>,
    P: BindParams<Database<D>> + ?Sized,
{
    let args = arguments::<D, P>(values, statement.binds())?;
    Ok(sqlx::query_as_with(
        SqlStr::from_static(statement.sql()),
        args,
    ))
}

pub struct Affected;

pub struct AllRows<T>(PhantomData<T>);

// Each impl binds the values before its future starts, so the future doesn't hold them: a hook's
// values are a `dyn BindParams`, which isn't `Sync`.
pub trait Fetch<D: Driver> {
    type Output;

    fn fetch<P: BindParams<Database<D>> + ?Sized>(
        values: &P,
        statement: &Command,
        conn: &mut Connection<D>,
    ) -> impl Future<Output = Result<Self::Output, sqlx::Error>> + Send;
}

impl<D: Driver> Fetch<D> for Affected {
    type Output = u64;

    fn fetch<P: BindParams<Database<D>> + ?Sized>(
        values: &P,
        statement: &Command,
        conn: &mut Connection<D>,
    ) -> impl Future<Output = Result<u64, sqlx::Error>> + Send {
        let query = query::<D, P>(values, statement);
        async move { Ok(D::rows_affected(&query?.execute(D::executor(conn)).await?)) }
    }
}

impl<D, T> Fetch<D> for AllRows<T>
where
    D: Driver,
    T: for<'r> FromRow<'r, Row<D>> + Send + Unpin,
{
    type Output = Vec<T>;

    fn fetch<P: BindParams<Database<D>> + ?Sized>(
        values: &P,
        statement: &Command,
        conn: &mut Connection<D>,
    ) -> impl Future<Output = Result<Vec<T>, sqlx::Error>> + Send {
        let query = query_as::<D, T, P>(values, statement);
        async move { query?.fetch_all(D::executor(conn)).await }
    }
}

pub struct One<T>(PhantomData<T>);

impl<D, T> Fetch<D> for One<T>
where
    D: Driver,
    T: for<'r> FromRow<'r, Row<D>> + Send + Unpin,
{
    type Output = T;

    fn fetch<P: BindParams<Database<D>> + ?Sized>(
        values: &P,
        statement: &Command,
        conn: &mut Connection<D>,
    ) -> impl Future<Output = Result<T, sqlx::Error>> + Send {
        let query = query_as::<D, T, P>(values, statement);
        async move { query?.fetch_one(D::executor(conn)).await }
    }
}

pub struct Optional<T>(PhantomData<T>);

impl<D, T> Fetch<D> for Optional<T>
where
    D: Driver,
    T: for<'r> FromRow<'r, Row<D>> + Send + Unpin,
{
    type Output = Option<T>;

    fn fetch<P: BindParams<Database<D>> + ?Sized>(
        values: &P,
        statement: &Command,
        conn: &mut Connection<D>,
    ) -> impl Future<Output = Result<Option<T>, sqlx::Error>> + Send {
        let query = query_as::<D, T, P>(values, statement);
        async move { query?.fetch_optional(D::executor(conn)).await }
    }
}

pub struct NoRow;

impl<'r, R: sqlx::Row> FromRow<'r, R> for NoRow {
    fn from_row(_: &'r R) -> Result<Self, sqlx::Error> {
        Err(sqlx::Error::Decode("this statement has no row type".into()))
    }
}

pub trait Step {
    type Fetch;
    type Row;
}

pub const fn rows<S: Rows>() {}

pub type Output<S, D> = <<S as Step>::Fetch as Fetch<D>>::Output;

pub async fn step<D, F, P>(
    values: &P,
    statement: &Command,
    conn: &mut Connection<D>,
) -> Result<F::Output, sqlx::Error>
where
    D: Driver,
    F: Fetch<D>,
    P: BindParams<Database<D>>,
{
    F::fetch(values, statement, conn).await
}

pub async fn hooks<D, V>(
    values: &V,
    hooks: &[Command],
    conn: &mut Connection<D>,
) -> Result<(), sqlx::Error>
where
    D: Driver,
    V: BindHooks<Database<D>>,
{
    for hook in hooks {
        let values = values.values(hook).unwrap_or(&());
        <Affected as Fetch<D>>::fetch(values, hook, &mut *conn).await?;
    }
    Ok(())
}
