pub mod bind;
pub mod hook;
#[cfg(feature = "sqlx")]
pub mod params;
#[cfg(feature = "sqlx")]
pub mod run;

#[cfg(feature = "sqlx")]
use crate::dialect::driver::{Arguments, Database, Driver, Query, QueryAs, Row};
use crate::hooks::Single;
use crate::sql::Sql;
use bind::Bind;
use hook::Hook;
#[cfg(feature = "sqlx")]
use params::BindParams;

pub trait Statement: Sql {
    const SQL: &'static str;
    const BINDS: &'static [Bind];
    const BEFORE: &'static [Hook];
    const AFTER: &'static [Hook];
}

#[doc(hidden)]
pub trait SingleRef {}

impl<S: Single> SingleRef for &S {}

#[cfg(feature = "sqlx")]
#[doc(hidden)]
pub fn query<'q, S, I>(statement: &S) -> Result<Query<'q, S::Dialect>, sqlx::Error>
where
    S: Statement + Single<I> + BindParams<Database<S::Dialect>>,
    S::Dialect: Driver,
    Arguments<S::Dialect>: sqlx::IntoArguments<Database<S::Dialect>>,
{
    let args = params::arguments::<S::Dialect, S>(statement, S::BINDS)?;
    Ok(sqlx::query_with(sqlx::SqlStr::from_static(S::SQL), args))
}

#[cfg(feature = "sqlx")]
#[doc(hidden)]
pub fn query_as<'q, S, O, I>(statement: &S) -> Result<QueryAs<'q, S::Dialect, O>, sqlx::Error>
where
    S: Statement + Single<I> + BindParams<Database<S::Dialect>>,
    S::Dialect: Driver,
    Arguments<S::Dialect>: sqlx::IntoArguments<Database<S::Dialect>>,
    O: for<'r> sqlx::FromRow<'r, Row<S::Dialect>>,
{
    let args = params::arguments::<S::Dialect, S>(statement, S::BINDS)?;
    Ok(sqlx::query_as_with(sqlx::SqlStr::from_static(S::SQL), args))
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` has no row type",
    note = "give it `row = Type` or named fields to read one row with `as one` or `as optional`"
)]
pub trait Rows: Statement {
    type Row;
}

#[macro_export]
macro_rules! impl_statement {
    ($($ty:ty),+ $(,)?) => {$(
        impl $crate::render::Render for $ty {
            const SIZE: $crate::render::Size = $crate::render::measure::<
                <$ty as $crate::sql::Sql>::Dialect,
            >(<$ty as $crate::sql::Sql>::NODE);
            const OUTPUT: $crate::render::Output = {
                const RENDERED: $crate::render::Rendered<
                    { <$ty as $crate::render::Render>::SIZE.sql },
                    { <$ty as $crate::render::Render>::SIZE.binds },
                    { <$ty as $crate::render::Render>::SIZE.statements() },
                > = $crate::render::render::<<$ty as $crate::sql::Sql>::Dialect, _, _, _>(
                    <$ty as $crate::sql::Sql>::NODE,
                );
                const STATEMENTS: [$crate::statement::hook::Hook;
                    <$ty as $crate::render::Render>::SIZE.statements()] = RENDERED.statements();
                $crate::render::Output::new(&STATEMENTS, RENDERED.before(), RENDERED.steps())
            };
            type Hooks = <$crate::hooks::HookFlag<
                { $crate::render::hooked(<$ty as $crate::sql::Sql>::NODE) },
            > as $crate::hooks::HookState>::Out;
        }

        impl $crate::statement::Statement for $ty {
            const SQL: &'static str = <$ty as $crate::render::Render>::OUTPUT.main().sql();
            const BINDS: &'static [$crate::statement::bind::Bind] =
                <$ty as $crate::render::Render>::OUTPUT.main().binds();
            const BEFORE: &'static [$crate::statement::hook::Hook] =
                <$ty as $crate::render::Render>::OUTPUT.before();
            const AFTER: &'static [$crate::statement::hook::Hook] =
                <$ty as $crate::render::Render>::OUTPUT.after();
        }

        $crate::__impl_sqlx!($ty);

        const _: &str = <$ty as $crate::statement::Statement>::SQL;
    )+};
}

#[cfg(feature = "sqlx")]
#[doc(hidden)]
#[macro_export]
macro_rules! __impl_sqlx {
    ($ty:ty) => {
        impl<'a> $crate::__private::sqlx::SqlSafeStr for $ty
        where
            &'a $ty: $crate::statement::SingleRef,
        {
            fn into_sql_str(self) -> $crate::__private::sqlx::SqlStr {
                $crate::__private::sqlx::SqlStr::from_static(
                    <$ty as $crate::statement::Statement>::SQL,
                )
            }
        }
    };
}

#[cfg(not(feature = "sqlx"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __impl_sqlx {
    ($ty:ty) => {};
}

#[macro_export]
macro_rules! impl_display {
    ($ty:ty => sql) => {
        impl ::core::fmt::Display for $ty {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(<$ty as $crate::statement::Statement>::SQL)
            }
        }
    };
    ($ty:ty => name) => {
        impl ::core::fmt::Display for $ty {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.write_str(<$ty as $crate::sql::Sql>::NODE.name.as_str())
            }
        }
    };
}

#[macro_export]
macro_rules! impl_debug {
    ($ty:ty => sql) => {
        impl ::core::fmt::Debug for $ty {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Debug::fmt(<$ty as $crate::statement::Statement>::SQL, f)
            }
        }
    };
    ($ty:ty => tree) => {
        impl ::core::fmt::Debug for $ty {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Debug::fmt(<$ty as $crate::sql::Sql>::NODE, f)
            }
        }
    };
}
