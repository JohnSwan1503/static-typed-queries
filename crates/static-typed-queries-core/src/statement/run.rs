use core::marker::PhantomData;

use sqlx::{Acquire, Executor, FromRow, IntoArguments, SqlStr};

use super::Statement;
use super::hook::Hook;
use super::params::{BindHooks, BindParams, arguments};
use crate::dialect::driver::{Arguments, Connection, Database, Driver, Row};

pub async fn execute<'c, S, V, A>(
    params: &S::Params,
    values: &V,
    conn: A,
) -> Result<u64, sqlx::Error>
where
    S: Statement,
    S::Dialect: Driver,
    S::Params: BindParams<Database<S::Dialect>>,
    V: BindHooks<Database<S::Dialect>>,
    A: Acquire<'c, Database = Database<S::Dialect>>,
    Arguments<S::Dialect>: IntoArguments<Database<S::Dialect>>,
    for<'e> &'e mut Connection<S::Dialect>: Executor<'e, Database = Database<S::Dialect>>,
{
    let mut tx = conn.begin().await?;
    hooks::<S::Dialect, V>(values, S::BEFORE, &mut tx).await?;
    let result = sqlx::query_with(SqlStr::from_static(S::SQL), S::arguments(params)?)
        .execute(&mut *tx)
        .await?;
    hooks::<S::Dialect, V>(values, S::AFTER, &mut tx).await?;
    tx.commit().await?;
    Ok(S::Dialect::rows_affected(&result))
}

pub async fn fetch_all<'c, S, O, V, A>(
    params: &S::Params,
    values: &V,
    conn: A,
) -> Result<Vec<O>, sqlx::Error>
where
    S: Statement,
    S::Dialect: Driver,
    S::Params: BindParams<Database<S::Dialect>>,
    O: for<'r> FromRow<'r, Row<S::Dialect>> + Send + Unpin,
    V: BindHooks<Database<S::Dialect>>,
    A: Acquire<'c, Database = Database<S::Dialect>>,
    Arguments<S::Dialect>: IntoArguments<Database<S::Dialect>>,
    for<'e> &'e mut Connection<S::Dialect>: Executor<'e, Database = Database<S::Dialect>>,
{
    let mut tx = conn.begin().await?;
    hooks::<S::Dialect, V>(values, S::BEFORE, &mut tx).await?;
    let rows = sqlx::query_as_with(SqlStr::from_static(S::SQL), S::arguments(params)?)
        .fetch_all(&mut *tx)
        .await?;
    hooks::<S::Dialect, V>(values, S::AFTER, &mut tx).await?;
    tx.commit().await?;
    Ok(rows)
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

pub trait Step {
    type Fetch;
}

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
