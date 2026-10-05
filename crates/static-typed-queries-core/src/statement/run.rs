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

async fn hooks<D, V>(
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
