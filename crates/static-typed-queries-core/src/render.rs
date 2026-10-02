mod error;
mod renderer;

use crate::dialect::Dialect;
use crate::node::Node;
use crate::statement::bind::Bind;
use renderer::Renderer;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Size {
    pub sql: usize,
    pub binds: usize,
}

#[doc(hidden)]
pub trait Measure {
    const SIZE: Size;
}

pub const fn measure<D: Dialect>(root: &'static Node) -> Size {
    let (mut sql, mut binds) = ([], []);
    let mut renderer = Renderer::<D>::new(&mut sql, &mut binds);
    renderer.statement(root);
    renderer.size()
}

pub const fn sql<D: Dialect, const N: usize>(root: &'static Node) -> [u8; N] {
    let (mut sql, mut binds) = ([0; N], []);
    let mut renderer = Renderer::<D>::new(&mut sql, &mut binds);
    renderer.statement(root);
    assert!(
        renderer.size().sql == N,
        "rendered length changed between passes"
    );
    sql
}

pub const fn binds<D: Dialect, const N: usize>(root: &'static Node) -> [Bind; N] {
    let (mut sql, mut binds) = ([], [Bind::EMPTY; N]);
    let mut renderer = Renderer::<D>::new(&mut sql, &mut binds);
    renderer.statement(root);
    assert!(
        renderer.size().binds == N,
        "bind count changed between passes"
    );
    binds
}

pub const fn as_str(bytes: &'static [u8]) -> &'static str {
    match core::str::from_utf8(bytes) {
        Ok(sql) => sql,
        Err(_) => panic!("rendered SQL isn't valid UTF-8"),
    }
}
