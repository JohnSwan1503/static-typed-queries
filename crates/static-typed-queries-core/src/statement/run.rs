use sqlx::{Acquire, Executor, FromRow, IntoArguments, SqlStr};

use super::Statement;
use super::hook::Hook;
use super::params::{BindParams, arguments};
use crate::dialect::driver::{Arguments, Connection, Database, Driver, Row};

pub async fn execute<'c, S, A>(params: &S::Params, conn: A) -> Result<u64, sqlx::Error>
where
    S: Statement,
    S::Dialect: Driver,
    S::Params: BindParams<Database<S::Dialect>>,
    A: Acquire<'c, Database = Database<S::Dialect>>,
    Arguments<S::Dialect>: IntoArguments<Database<S::Dialect>>,
    for<'e> &'e mut Connection<S::Dialect>: Executor<'e, Database = Database<S::Dialect>>,
{
    let mut tx = conn.begin().await?;
    hooks::<S>(params, S::BEFORE, &mut tx).await?;
    let result = sqlx::query_with(SqlStr::from_static(S::SQL), S::arguments(params)?)
        .execute(&mut *tx)
        .await?;
    hooks::<S>(params, S::AFTER, &mut tx).await?;
    tx.commit().await?;
    Ok(S::Dialect::rows_affected(&result))
}

pub async fn fetch_all<'c, S, O, A>(params: &S::Params, conn: A) -> Result<Vec<O>, sqlx::Error>
where
    S: Statement,
    S::Dialect: Driver,
    S::Params: BindParams<Database<S::Dialect>>,
    O: for<'r> FromRow<'r, Row<S::Dialect>> + Send + Unpin,
    A: Acquire<'c, Database = Database<S::Dialect>>,
    Arguments<S::Dialect>: IntoArguments<Database<S::Dialect>>,
    for<'e> &'e mut Connection<S::Dialect>: Executor<'e, Database = Database<S::Dialect>>,
{
    let mut tx = conn.begin().await?;
    hooks::<S>(params, S::BEFORE, &mut tx).await?;
    let rows = sqlx::query_as_with(SqlStr::from_static(S::SQL), S::arguments(params)?)
        .fetch_all(&mut *tx)
        .await?;
    hooks::<S>(params, S::AFTER, &mut tx).await?;
    tx.commit().await?;
    Ok(rows)
}

async fn hooks<S>(
    params: &S::Params,
    hooks: &[Hook],
    conn: &mut Connection<S::Dialect>,
) -> Result<(), sqlx::Error>
where
    S: Statement,
    S::Dialect: Driver,
    S::Params: BindParams<Database<S::Dialect>>,
    Arguments<S::Dialect>: IntoArguments<Database<S::Dialect>>,
    for<'e> &'e mut Connection<S::Dialect>: Executor<'e, Database = Database<S::Dialect>>,
{
    for hook in hooks {
        let args = arguments::<S::Dialect, _>(params, hook.binds())?;
        sqlx::query_with(SqlStr::from_static(hook.sql()), args)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}
