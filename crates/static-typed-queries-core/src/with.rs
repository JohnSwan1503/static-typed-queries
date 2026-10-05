use sqlx::{Acquire, Executor, FromRow, IntoArguments, SqlStr};

use crate::dialect::driver::{Arguments, Connection, Database, Driver, Row};
use crate::hooks::{HookNeeds, HookValues, Provides, with};
use crate::sql::Sql;
use crate::statement::Statement;
use crate::statement::hook::Hook;
use crate::statement::params::{BindHooks, BindParams, arguments};
use crate::statement::run::hooks;
use crate::values::Values;

// A statement or transaction together with the values of its hooks, which `with` adds one hook
// at a time; `run` needs every reachable hook with values to be present.
pub struct With<S, V> {
    statement: S,
    values: V,
}

impl<S> With<S, ()> {
    pub fn new(statement: S) -> Self {
        With {
            statement,
            values: (),
        }
    }
}

impl<S, V> With<S, V> {
    pub fn with<H, I>(self, values: H) -> With<S, (HookValues<H>, V)>
    where
        H: Values,
        (HookValues<H>, V): Provides<H, I>,
    {
        With {
            statement: self.statement,
            values: with(values, self.values),
        }
    }

    // Everything runs in one transaction: the hooks before, the statement or steps, the hooks
    // after.
    pub async fn run<'c, I, A>(self, conn: A) -> Result<S::Output, sqlx::Error>
    where
        S: Run + HookNeeds<V, I>,
        S::Dialect: Driver,
        V: BindHooks<Database<S::Dialect>>,
        A: Acquire<'c, Database = Database<S::Dialect>>,
        Arguments<S::Dialect>: IntoArguments<Database<S::Dialect>>,
        for<'e> &'e mut Connection<S::Dialect>: Executor<'e, Database = Database<S::Dialect>>,
    {
        let mut tx = conn.begin().await?;
        hooks::<S::Dialect, V>(&self.values, S::BEFORE, &mut tx).await?;
        let output = self.statement.run(&mut tx).await?;
        hooks::<S::Dialect, V>(&self.values, S::AFTER, &mut tx).await?;
        tx.commit().await?;
        Ok(output)
    }

    pub async fn run_as<'c, O, I, A>(self, conn: A) -> Result<Vec<O>, sqlx::Error>
    where
        S: Statement + HookNeeds<V, I> + BindParams<Database<S::Dialect>>,
        S::Dialect: Driver,
        O: for<'r> FromRow<'r, Row<S::Dialect>> + Send + Unpin,
        V: BindHooks<Database<S::Dialect>>,
        A: Acquire<'c, Database = Database<S::Dialect>>,
        Arguments<S::Dialect>: IntoArguments<Database<S::Dialect>>,
        for<'e> &'e mut Connection<S::Dialect>: Executor<'e, Database = Database<S::Dialect>>,
    {
        let mut tx = conn.begin().await?;
        hooks::<S::Dialect, V>(&self.values, S::BEFORE, &mut tx).await?;
        let args = arguments::<S::Dialect, S>(&self.statement, S::BINDS)?;
        let rows = sqlx::query_as_with(SqlStr::from_static(S::SQL), args)
            .fetch_all(&mut *tx)
            .await?;
        hooks::<S::Dialect, V>(&self.values, S::AFTER, &mut tx).await?;
        tx.commit().await?;
        Ok(rows)
    }
}

pub trait Run: Sql
where
    Self::Dialect: Driver,
{
    type Output;
    const BEFORE: &'static [Hook];
    const AFTER: &'static [Hook];

    fn run(
        self,
        conn: &mut Connection<Self::Dialect>,
    ) -> impl Future<Output = Result<Self::Output, sqlx::Error>>;
}
