mod ctes;
mod hooks;
mod names;
mod paths;
mod placement;
mod text;

use core::marker::PhantomData;

use super::error::fail;
use super::{Offsets, Size};
use crate::dialect::Dialect;
use crate::node::Node;
use crate::node::kind::Kind;
use crate::part::Part;
use crate::statement::bind::Bind;
use crate::statement::bind::path::Path;
use ctes::{Attached, Cte};
use hooks::Hook;
use paths::target;
use text::Numbered;

pub(super) use hooks::{check_hooks, hooked};

const MAX_CTES: usize = 64;
const MAX_HOOKS: usize = 64;
const MAX_NUMBERED: usize = 1024;

pub(super) struct Renderer<'a, D> {
    root: Option<&'static Node>,
    sql: &'a mut [u8],
    binds: &'a mut [Bind],
    offsets: &'a mut [Offsets],
    size: Size,
    rendered: usize,
    first_bind: usize,
    hooks: [Option<Hook>; MAX_HOOKS],
    ctes: [Option<Cte>; MAX_CTES],
    cte_count: usize,
    recursive: bool,
    attached: [Option<Attached>; MAX_CTES],
    attached_count: usize,
    numbered: [Option<Numbered>; MAX_NUMBERED],
    numbered_count: usize,
    dialect: PhantomData<D>,
}

impl<'a, D: Dialect> Renderer<'a, D> {
    pub(super) const fn new(
        sql: &'a mut [u8],
        binds: &'a mut [Bind],
        offsets: &'a mut [Offsets],
    ) -> Self {
        Self {
            root: None,
            sql,
            binds,
            offsets,
            size: Size {
                sql: 0,
                binds: 0,
                before: 0,
                steps: 0,
                after: 0,
            },
            rendered: 0,
            first_bind: 0,
            hooks: [None; MAX_HOOKS],
            ctes: [None; MAX_CTES],
            cte_count: 0,
            recursive: false,
            attached: [None; MAX_CTES],
            attached_count: 0,
            numbered: [None; MAX_NUMBERED],
            numbered_count: 0,
            dialect: PhantomData,
        }
    }

    pub(super) const fn size(&self) -> Size {
        self.size
    }

    pub(super) const fn statement(&mut self, root: &'static Node) {
        if let Kind::Transaction = root.kind {
            self.transaction(root);
            return;
        }
        let (main, path) = target(root, Path::ROOT);
        self.find_hooks(main);
        self.render_hooks(false);
        self.single(root, root, main, path);
        self.size.steps = 1;
        self.render_hooks(true);
    }

    const fn transaction(&mut self, root: &'static Node) {
        self.find_hooks(root);
        self.render_hooks(false);
        let parts = root.parts.0;
        let mut i = 0;
        while i < parts.len() {
            self.root = Some(root);
            match parts[i] {
                Part::From(from) => self.attach(root, from.target()),
                Part::Expr(step) => {
                    let (step, path) = self.resolve(root, Path::ROOT, step.target());
                    let (node, path) = target(step, path);
                    self.single(root, step, node, path);
                    self.attached = [None; MAX_CTES];
                    self.attached_count = 0;
                    self.size.steps += 1;
                }
                Part::Lit(_) | Part::Ident(_) | Part::Param(_) => {}
            }
            i += 1;
        }
        if self.attached_count > 0 {
            fail(&["a CTE step needs a step after it to attach to"]);
        }
        self.render_hooks(true);
    }

    const fn single(
        &mut self,
        root: &'static Node,
        named: &'static Node,
        node: &'static Node,
        path: Path,
    ) {
        self.root = Some(root);
        self.ctes = [None; MAX_CTES];
        self.cte_count = 0;
        self.recursive = false;
        self.numbered = [None; MAX_NUMBERED];
        self.numbered_count = 0;
        self.first_bind = self.size.binds;
        self.collect_attached();
        self.collect(node, path);
        self.with_clause();
        self.body(node, path);
        if !self.offsets.is_empty() {
            self.offsets[self.rendered] = Offsets {
                name: named.name,
                fingerprint: named.fingerprint,
                sql: self.size.sql,
                binds: self.size.binds,
            };
        }
        self.rendered += 1;
    }
}
