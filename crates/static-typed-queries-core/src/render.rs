mod error;
mod renderer;

use crate::dialect::Dialect;
use crate::node::Node;
use crate::node::fingerprint::Fingerprint;
use crate::node::name::Name;
use crate::statement::bind::Bind;
use crate::statement::hook::Hook;
use renderer::Renderer;

pub use renderer::{check_hooks, hooked};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Size {
    pub sql: usize,
    pub binds: usize,
    pub before: usize,
    pub steps: usize,
    pub after: usize,
}

impl Size {
    pub const fn statements(&self) -> usize {
        self.before + self.steps + self.after
    }
}

#[derive(Clone, Copy)]
struct Offsets {
    name: Name,
    fingerprint: Fingerprint,
    sql: usize,
    binds: usize,
}

impl Offsets {
    const EMPTY: Offsets = Offsets {
        name: Name::EMPTY,
        fingerprint: Fingerprint::EMPTY,
        sql: 0,
        binds: 0,
    };
}

pub trait Render {
    const SIZE: Size;
    const OUTPUT: Output;
    type Hooks;
}

pub struct Rendered<const S: usize, const B: usize, const N: usize> {
    sql: [u8; S],
    binds: [Bind; B],
    offsets: [Offsets; N],
    before: usize,
    steps: usize,
}

impl<const S: usize, const B: usize, const N: usize> Rendered<S, B, N> {
    pub const fn statements(&'static self) -> [Hook; N] {
        let mut statements = [Hook::new(Name::EMPTY, Fingerprint::EMPTY, "", &[]); N];
        let mut start = Offsets::EMPTY;
        let mut i = 0;
        while i < N {
            let end = self.offsets[i];
            let sql = self.sql.split_at(end.sql).0.split_at(start.sql).1;
            let sql = match core::str::from_utf8(sql) {
                Ok(sql) => sql,
                Err(_) => panic!("rendered SQL isn't valid UTF-8"),
            };
            let binds = self.binds.split_at(end.binds).0.split_at(start.binds).1;
            statements[i] = Hook::new(end.name, end.fingerprint, sql, binds);
            start = end;
            i += 1;
        }
        statements
    }

    pub const fn before(&self) -> usize {
        self.before
    }

    pub const fn steps(&self) -> usize {
        self.steps
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Output {
    statements: &'static [Hook],
    before: usize,
    steps: usize,
}

impl Output {
    pub const fn new(statements: &'static [Hook], before: usize, steps: usize) -> Output {
        Output {
            statements,
            before,
            steps,
        }
    }

    pub const fn main(&self) -> Hook {
        self.statements[self.before]
    }

    pub const fn before(&self) -> &'static [Hook] {
        self.statements.split_at(self.before).0
    }

    pub const fn steps(&self) -> &'static [Hook] {
        self.statements
            .split_at(self.before + self.steps)
            .0
            .split_at(self.before)
            .1
    }

    pub const fn after(&self) -> &'static [Hook] {
        self.statements.split_at(self.before + self.steps).1
    }
}

pub const fn measure<D: Dialect>(root: &'static Node) -> Size {
    let (mut sql, mut binds, mut offsets) = ([], [], []);
    let mut renderer = Renderer::<D>::new(&mut sql, &mut binds, &mut offsets);
    renderer.statement(root);
    renderer.size()
}

pub const fn render<D: Dialect, const S: usize, const B: usize, const N: usize>(
    root: &'static Node,
) -> Rendered<S, B, N> {
    let mut rendered = Rendered {
        sql: [0; S],
        binds: [Bind::EMPTY; B],
        offsets: [Offsets::EMPTY; N],
        before: 0,
        steps: 0,
    };
    let mut renderer = Renderer::<D>::new(
        &mut rendered.sql,
        &mut rendered.binds,
        &mut rendered.offsets,
    );
    renderer.statement(root);
    let size = renderer.size();
    assert!(
        size.sql == S && size.binds == B && size.statements() == N,
        "rendering changed between passes"
    );
    rendered.before = size.before;
    rendered.steps = size.steps;
    rendered
}

#[doc(hidden)]
#[macro_export]
macro_rules! impl_render {
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
    };
}
