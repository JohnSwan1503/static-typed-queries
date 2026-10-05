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
