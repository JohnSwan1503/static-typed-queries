use crate::sql::Sql;
use crate::statement::command::Command;

pub trait Transaction: Sql {
    const STEPS: &'static [Command];
    const BEFORE: &'static [Command];
    const AFTER: &'static [Command];
}

#[macro_export]
macro_rules! impl_transaction {
    ($ty:ty) => {
        $crate::impl_render!($ty);

        impl $crate::transaction::Transaction for $ty {
            const STEPS: &'static [$crate::statement::command::Command] =
                <$ty as $crate::render::Render>::OUTPUT.steps();
            const BEFORE: &'static [$crate::statement::command::Command] =
                <$ty as $crate::render::Render>::OUTPUT.before();
            const AFTER: &'static [$crate::statement::command::Command] =
                <$ty as $crate::render::Render>::OUTPUT.after();
        }

        const _: &[$crate::statement::command::Command] =
            <$ty as $crate::transaction::Transaction>::STEPS;
    };
}
