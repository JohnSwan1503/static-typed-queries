use core::pin::Pin;

use sqlx::{Acquire, FromRow};

use crate::builder::Finish;
use crate::dialect::driver::{Connection, Database, Driver, Row};
use crate::hooks::{HookNeeds, HookValues, Provides};
use crate::render::Render;
use crate::sql::Sql;
use crate::statement::Statement;
use crate::statement::params::{BindHooks, BindParams};
use crate::statement::run::{self, AllRows, Fetch, Step, hooks, step};
use crate::values::Values;

// `run` returns this rather than an `impl Future`, which carries the method's bounds and has a
// missing hook value reported three times: at the call, the method and the `.await`.
pub type Running<'c, T> = Pin<Box<dyn Future<Output = Result<T, sqlx::Error>> + Send + 'c>>;

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
    pub fn with<B, H, I>(self, values: B) -> With<S, (HookValues<H>, V)>
    where
        B: Finish<Output = H>,
        H: Values,
        (HookValues<H>, V): Provides<H, I>,
    {
        With {
            statement: self.statement,
            values: (HookValues(values.finish()), self.values),
        }
    }

    pub fn run<'c, I, A>(self, conn: A) -> Running<'c, S::Output>
    where
        S: Run + HookNeeds<V, I> + Send + Sync + 'c,
        S::Dialect: Driver,
        S::Output: Send,
        V: BindHooks<Database<S::Dialect>> + Send + Sync + 'c,
        A: Acquire<'c, Database = Database<S::Dialect>> + Send + 'c,
    {
        Box::pin(self.transaction(conn, async |statement, conn| statement.run(conn).await))
    }

    pub fn run_as<'c, O, I, A>(self, conn: A) -> Running<'c, Vec<O>>
    where
        S: Statement
            + Render
            + HookNeeds<V, I>
            + BindParams<Database<S::Dialect>>
            + Send
            + Sync
            + 'c,
        S::Dialect: Driver,
        O: for<'r> FromRow<'r, Row<S::Dialect>> + Send + Unpin + 'c,
        V: BindHooks<Database<S::Dialect>> + Send + Sync + 'c,
        A: Acquire<'c, Database = Database<S::Dialect>> + Send + 'c,
    {
        Box::pin(self.transaction(conn, async |statement, conn| {
            step::<S::Dialect, AllRows<O>, _>(&statement, &S::OUTPUT.main(), conn).await
        }))
    }

    // Everything runs in one transaction: the hooks before, the statement or steps, the hooks
    // after.
    async fn transaction<'c, A, T>(
        self,
        conn: A,
        body: impl AsyncFnOnce(S, &mut Connection<S::Dialect>) -> Result<T, sqlx::Error>,
    ) -> Result<T, sqlx::Error>
    where
        S: Sql + Render,
        S::Dialect: Driver,
        V: BindHooks<Database<S::Dialect>>,
        A: Acquire<'c, Database = Database<S::Dialect>>,
    {
        let mut tx = conn.begin().await?;
        hooks::<S::Dialect, V>(&self.values, S::OUTPUT.before(), &mut tx).await?;
        let output = body(self.statement, &mut tx).await?;
        hooks::<S::Dialect, V>(&self.values, S::OUTPUT.after(), &mut tx).await?;
        tx.commit().await?;
        Ok(output)
    }
}

pub trait Run: Sql + Render
where
    Self::Dialect: Driver,
{
    type Output;

    fn run(
        self,
        conn: &mut Connection<Self::Dialect>,
    ) -> impl Future<Output = Result<Self::Output, sqlx::Error>> + Send;
}

impl<S> Run for S
where
    S: Statement + Render + Step + BindParams<Database<S::Dialect>> + Send + Sync,
    S::Dialect: Driver,
    S::Fetch: Fetch<S::Dialect>,
{
    type Output = run::Output<S, S::Dialect>;

    async fn run(self, conn: &mut Connection<S::Dialect>) -> Result<Self::Output, sqlx::Error> {
        step::<S::Dialect, S::Fetch, S>(&self, &S::OUTPUT.main(), conn).await
    }
}
