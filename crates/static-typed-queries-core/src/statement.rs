pub mod bind;

use crate::sql::Sql;
use bind::Bind;

pub trait Statement: Sql {
    const SQL: &'static str;
    const BINDS: &'static [Bind];
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

        const _: $crate::render::Size = <$ty as $crate::render::Measure>::SIZE;
        const _: &str = <$ty as $crate::statement::Statement>::SQL;
        const _: &[$crate::statement::bind::Bind] = <$ty as $crate::statement::Statement>::BINDS;
    )+};
}
