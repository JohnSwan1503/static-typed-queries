mod ctes;
mod hooks;
mod list;
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
use list::List;
use paths::{resolve, target};
use text::Numbered;

pub use hooks::{check_hooks, hooked};

const MAX_CTES: usize = 64;
const MAX_HOOKS: usize = 64;
const MAX_NUMBERED: usize = 1024;

pub(super) struct Renderer<'a, D> {
    sql: &'a mut [u8],
    binds: &'a mut [Bind],
    offsets: &'a mut [Offsets],
    size: Size,
    rendered: usize,
    first_bind: usize,
    hooks: List<Hook, MAX_HOOKS>,
    ctes: List<Cte, MAX_CTES>,
    recursive: bool,
    attached: List<Attached, MAX_CTES>,
    numbered: List<Numbered, MAX_NUMBERED>,
    dialect: PhantomData<D>,
}

impl<'a, D: Dialect> Renderer<'a, D> {
    pub(super) const fn new(
        sql: &'a mut [u8],
        binds: &'a mut [Bind],
        offsets: &'a mut [Offsets],
    ) -> Self {
        Self {
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
            hooks: List::new(),
            ctes: List::new(),
            recursive: false,
            attached: List::new(),
            numbered: List::new(),
            dialect: PhantomData,
        }
    }

    pub(super) const fn size(&self) -> Size {
        self.size
    }

    #[track_caller]
    pub(super) const fn statement(&mut self, root: &'static Node) {
        if let Kind::Transaction = root.kind {
            self.transaction(root);
            return;
        }
        let (main, path) = target(root, Path::ROOT);
        self.find_hooks(main);
        self.render_hooks(false);
        self.single(root, main, path);
        self.size.steps = 1;
        self.render_hooks(true);
    }

    #[track_caller]
    const fn transaction(&mut self, root: &'static Node) {
        self.find_hooks(root);
        self.render_hooks(false);
        let parts = root.parts.0;
        let mut i = 0;
        while i < parts.len() {
            match parts[i] {
                Part::From(from) => self.attach(root, from.target()),
                Part::Expr(step) => {
                    let (step, path) = resolve(root, Path::ROOT, step.target());
                    let (node, path) = target(step, path);
                    self.single(step, node, path);
                    self.attached.clear();
                    self.size.steps += 1;
                }
                Part::Lit(_) | Part::Ident(_) | Part::Param(_) => {}
            }
            i += 1;
        }
        if !self.attached.is_empty() {
            fail(&["a CTE step needs a step after it to attach to"]);
        }
        self.render_hooks(true);
    }

    #[track_caller]
    const fn single(&mut self, named: &'static Node, node: &'static Node, path: Path) {
        self.ctes.clear();
        self.recursive = false;
        self.numbered.clear();
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
