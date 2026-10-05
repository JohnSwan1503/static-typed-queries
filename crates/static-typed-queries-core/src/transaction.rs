use crate::sql::Sql;
use crate::statement::hook::Hook;

pub trait Transaction: Sql {
    const STEPS: &'static [Hook];
    const BEFORE: &'static [Hook];
    const AFTER: &'static [Hook];
}

#[macro_export]
macro_rules! impl_transaction {
    ($ty:ty) => {
        $crate::impl_render!($ty);

        impl $crate::transaction::Transaction for $ty {
            const STEPS: &'static [$crate::statement::hook::Hook] =
                <$ty as $crate::render::Render>::OUTPUT.steps();
            const BEFORE: &'static [$crate::statement::hook::Hook] =
                <$ty as $crate::render::Render>::OUTPUT.before();
            const AFTER: &'static [$crate::statement::hook::Hook] =
                <$ty as $crate::render::Render>::OUTPUT.after();
        }

        const _: &[$crate::statement::hook::Hook] =
            <$ty as $crate::transaction::Transaction>::STEPS;
    };
}
