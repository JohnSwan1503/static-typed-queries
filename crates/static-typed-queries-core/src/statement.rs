pub mod bind;
pub mod hook;
#[cfg(feature = "sqlx")]
pub mod params;

#[cfg(feature = "sqlx")]
use crate::dialect::driver::{Arguments, Database, Driver, Query, QueryAs, Row};
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

    #[cfg(feature = "sqlx")]
    fn arguments(params: &Self::Params) -> Result<Arguments<Self::Dialect>, sqlx::Error>
    where
        Self::Dialect: Driver,
        Self::Params: BindParams<Database<Self::Dialect>>,
    {
        let mut args = Arguments::<Self::Dialect>::default();
        for bind in Self::BINDS {
            params
                .bind(bind.path().steps(), bind.slot().inner(), &mut args)
                .map_err(sqlx::Error::Encode)?;
        }
        Ok(args)
    }

    #[cfg(feature = "sqlx")]
    fn query<'q>(params: &Self::Params) -> Result<Query<'q, Self::Dialect>, sqlx::Error>
    where
        Self::Dialect: Driver,
        Self::Params: BindParams<Database<Self::Dialect>>,
        Arguments<Self::Dialect>: sqlx::IntoArguments<Database<Self::Dialect>>,
    {
        const { unhooked::<Self>() };
        let args = Self::arguments(params)?;
        Ok(sqlx::query_with(sqlx::SqlStr::from_static(Self::SQL), args))
    }

    #[cfg(feature = "sqlx")]
    fn query_as<'q, O>(params: &Self::Params) -> Result<QueryAs<'q, Self::Dialect, O>, sqlx::Error>
    where
        Self::Dialect: Driver,
        Self::Params: BindParams<Database<Self::Dialect>>,
        Arguments<Self::Dialect>: sqlx::IntoArguments<Database<Self::Dialect>>,
        O: for<'r> sqlx::FromRow<'r, Row<Self::Dialect>>,
    {
        const { unhooked::<Self>() };
        let args = Self::arguments(params)?;
        Ok(sqlx::query_as_with(
            sqlx::SqlStr::from_static(Self::SQL),
            args,
        ))
    }
}

#[cfg(feature = "sqlx")]
const fn unhooked<S: Statement + ?Sized>() {
    assert!(
        S::BEFORE.is_empty() && S::AFTER.is_empty(),
        "a statement with `before` or `after` hooks can't run as a single query"
    );
}

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
                $crate::render::Output::new(&STATEMENTS, RENDERED.before())
            };
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
        impl $crate::__private::sqlx::SqlSafeStr for $ty {
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
