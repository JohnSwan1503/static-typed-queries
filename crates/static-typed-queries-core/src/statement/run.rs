use core::marker::PhantomData;

use sqlx::{Executor, FromRow, IntoArguments, SqlStr};

use super::hook::Hook;
use super::params::{BindHooks, BindParams, arguments};
use super::{Rows, Statement};
use crate::dialect::driver::{Arguments, Connection, Database, Driver, Row};

// Runs a statement's own SQL on a connection, fetching the way `F` says.
pub async fn statement<D, F, S>(
    statement: &S,
    conn: &mut Connection<D>,
) -> Result<F::Output, sqlx::Error>
where
    D: Driver,
    F: Fetch<D>,
    S: Statement + BindParams<Database<D>>,
{
    let args = arguments::<D, S>(statement, S::BINDS)?;
    F::fetch(S::SQL, args, conn).await
}

pub struct Affected;

pub struct AllRows<T>(PhantomData<T>);

pub trait Fetch<D: Driver> {
    type Output;

    fn fetch(
        sql: &'static str,
        args: Arguments<D>,
        conn: &mut Connection<D>,
    ) -> impl Future<Output = Result<Self::Output, sqlx::Error>>;
}

impl<D> Fetch<D> for Affected
where
    D: Driver,
    Arguments<D>: IntoArguments<Database<D>>,
    for<'e> &'e mut Connection<D>: Executor<'e, Database = Database<D>>,
{
    type Output = u64;

    async fn fetch(
        sql: &'static str,
        args: Arguments<D>,
        conn: &mut Connection<D>,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query_with(SqlStr::from_static(sql), args)
            .execute(conn)
            .await?;
        Ok(D::rows_affected(&result))
    }
}

impl<D, T> Fetch<D> for AllRows<T>
where
    D: Driver,
    T: for<'r> FromRow<'r, Row<D>> + Send + Unpin,
    Arguments<D>: IntoArguments<Database<D>>,
    for<'e> &'e mut Connection<D>: Executor<'e, Database = Database<D>>,
{
    type Output = Vec<T>;

    async fn fetch(
        sql: &'static str,
        args: Arguments<D>,
        conn: &mut Connection<D>,
    ) -> Result<Vec<T>, sqlx::Error> {
        sqlx::query_as_with(SqlStr::from_static(sql), args)
            .fetch_all(conn)
            .await
    }
}

pub struct One<T>(PhantomData<T>);

impl<D, T> Fetch<D> for One<T>
where
    D: Driver,
    T: for<'r> FromRow<'r, Row<D>> + Send + Unpin,
    Arguments<D>: IntoArguments<Database<D>>,
    for<'e> &'e mut Connection<D>: Executor<'e, Database = Database<D>>,
{
    type Output = T;

    async fn fetch(
        sql: &'static str,
        args: Arguments<D>,
        conn: &mut Connection<D>,
    ) -> Result<T, sqlx::Error> {
        sqlx::query_as_with(SqlStr::from_static(sql), args)
            .fetch_one(conn)
            .await
    }
}

pub struct Optional<T>(PhantomData<T>);

impl<D, T> Fetch<D> for Optional<T>
where
    D: Driver,
    T: for<'r> FromRow<'r, Row<D>> + Send + Unpin,
    Arguments<D>: IntoArguments<Database<D>>,
    for<'e> &'e mut Connection<D>: Executor<'e, Database = Database<D>>,
{
    type Output = Option<T>;

    async fn fetch(
        sql: &'static str,
        args: Arguments<D>,
        conn: &mut Connection<D>,
    ) -> Result<Option<T>, sqlx::Error> {
        sqlx::query_as_with(SqlStr::from_static(sql), args)
            .fetch_optional(conn)
            .await
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
    params: &P,
    statement: &Hook,
    conn: &mut Connection<D>,
) -> Result<F::Output, sqlx::Error>
where
    D: Driver,
    F: Fetch<D>,
    P: BindParams<Database<D>>,
{
    let args = arguments::<D, P>(params, statement.binds())?;
    F::fetch(statement.sql(), args, conn).await
}

pub async fn hooks<D, V>(
    values: &V,
    hooks: &[Hook],
    conn: &mut Connection<D>,
) -> Result<(), sqlx::Error>
where
    D: Driver,
    V: BindHooks<Database<D>>,
    Arguments<D>: IntoArguments<Database<D>>,
    for<'e> &'e mut Connection<D>: Executor<'e, Database = Database<D>>,
{
    for hook in hooks {
        let args = match values.values(hook) {
            Some(params) => arguments::<D, _>(params, hook.binds())?,
            None => arguments::<D, _>(&(), hook.binds())?,
        };
        sqlx::query_with(SqlStr::from_static(hook.sql()), args)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}
