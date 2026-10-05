mod error;
mod renderer;

use crate::dialect::Dialect;
use crate::node::Node;
use crate::node::fingerprint::Fingerprint;
use crate::node::name::Name;
use crate::statement::bind::Bind;
use crate::statement::hook::Hook;
use renderer::Renderer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Size {
    pub sql: usize,
    pub binds: usize,
    pub before: usize,
    pub after: usize,
}

impl Size {
    pub const fn statements(&self) -> usize {
        self.before + 1 + self.after
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

#[doc(hidden)]
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
}

#[derive(Clone, Copy, Debug)]
pub struct Output {
    statements: &'static [Hook],
    before: usize,
}

impl Output {
    pub const fn new(statements: &'static [Hook], before: usize) -> Output {
        Output { statements, before }
    }

    pub const fn main(&self) -> Hook {
        self.statements[self.before]
    }

    pub const fn before(&self) -> &'static [Hook] {
        self.statements.split_at(self.before).0
    }

    pub const fn after(&self) -> &'static [Hook] {
        self.statements.split_at(self.before + 1).1
    }
}

pub const fn check_hooks(owner: &'static Node) {
    renderer::check_hooks(owner);
}

pub const fn hooked(root: &'static Node) -> bool {
    renderer::hooked(root)
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
    rendered
}
