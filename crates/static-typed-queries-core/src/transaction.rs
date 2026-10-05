use crate::sql::Sql;
use crate::statement::command::Command;

/// Statements that run in order and commit together, declared with `#[transaction]`. Everything
/// here is rendered at compile time.
pub trait Transaction: Sql {
    /// Each step's SQL and binds, in the order the steps run. A step attached to the next one as
    /// a CTE is part of that step's SQL.
    const STEPS: &'static [Command];
    /// The hooks that run before the first step, in order.
    const BEFORE: &'static [Command];
    /// The hooks that run after the last step, in order.
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
