pub mod bind;
#[cfg(feature = "sqlx")]
pub mod params;

#[cfg(feature = "sqlx")]
use crate::dialect::driver::{Arguments, Database, Driver, Query, QueryAs, Row};
use crate::sql::Sql;
use bind::Bind;
#[cfg(feature = "sqlx")]
use params::BindParams;

pub trait Statement: Sql {
    const SQL: &'static str;
    const BINDS: &'static [Bind];

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
        let args = Self::arguments(params)?;
        Ok(sqlx::query_as_with(
            sqlx::SqlStr::from_static(Self::SQL),
            args,
        ))
    }
}

pub trait Rows: Statement {
    type Row;
}

#[macro_export]
macro_rules! impl_statement {
    ($($ty:ty),+ $(,)?) => {$(
        impl $crate::render::Measure for $ty {
            const SIZE: $crate::render::Size = $crate::render::measure::<
                <$ty as $crate::sql::Sql>::Dialect,
            >(<$ty as $crate::sql::Sql>::NODE);
        }

        impl $crate::statement::Statement for $ty {
            const SQL: &'static str = {
                const BYTES: [u8; <$ty as $crate::render::Measure>::SIZE.sql] =
                    $crate::render::sql::<
                        <$ty as $crate::sql::Sql>::Dialect,
                        { <$ty as $crate::render::Measure>::SIZE.sql },
                    >(<$ty as $crate::sql::Sql>::NODE);
                $crate::render::as_str(&BYTES)
            };
            const BINDS: &'static [$crate::statement::bind::Bind] = {
                const BINDS: [$crate::statement::bind::Bind;
                    <$ty as $crate::render::Measure>::SIZE.binds] = $crate::render::binds::<
                    <$ty as $crate::sql::Sql>::Dialect,
                    { <$ty as $crate::render::Measure>::SIZE.binds },
                >(<$ty as $crate::sql::Sql>::NODE);
                &BINDS
            };
        }

        $crate::__impl_sqlx!($ty);

        const _: $crate::render::Size = <$ty as $crate::render::Measure>::SIZE;
        const _: &str = <$ty as $crate::statement::Statement>::SQL;
        const _: &[$crate::statement::bind::Bind] = <$ty as $crate::statement::Statement>::BINDS;
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
