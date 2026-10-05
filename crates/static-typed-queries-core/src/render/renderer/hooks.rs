use crate::dialect::Dialect;
use crate::node::Node;
use crate::node::kind::Kind;
use crate::render::error::fail;
use crate::statement::bind::path::Path;

use super::names::same_node;
use super::paths::{Probe, has, resolve_node, target, unwrap_scope};
use super::{MAX_HOOKS, Renderer};

#[derive(Clone, Copy)]
pub(super) struct Hook {
    root: &'static Node,
    node: &'static Node,
    path: Path,
    after: bool,
}

impl<'a, D: Dialect> Renderer<'a, D> {
    pub(super) const fn find_hooks(&mut self, node: &'static Node) {
        let node = unwrap_scope(node);
        self.add_hooks(node, false);
        self.add_hooks(node, true);
        let parts = node.parts.0;
        let mut i = 0;
        while i < parts.len() {
            if let Some(target) = parts[i].target() {
                self.find_hooks(resolve_node(node, target));
            }
            i += 1;
        }
    }

    const fn add_hooks(&mut self, owner: &'static Node, after: bool) {
        let hooks = if after { owner.after.0 } else { owner.before.0 };
        let mut i = 0;
        while i < hooks.len() {
            let root = hooks[i];
            let (node, path) = hook(owner, root, Path::ROOT);
            if !self.has_hook(root, after) {
                let count = self.size.before + self.size.after;
                if count == MAX_HOOKS {
                    fail(&["a statement can't have more than 64 hooks"]);
                }
                self.hooks[count] = Some(Hook {
                    root,
                    node,
                    path,
                    after,
                });
                if after {
                    self.size.after += 1;
                } else {
                    self.size.before += 1;
                }
            }
            i += 1;
        }
    }

    const fn has_hook(&self, root: &'static Node, after: bool) -> bool {
        let mut i = 0;
        while i < self.size.before + self.size.after {
            if let Some(hook) = self.hooks[i]
                && hook.after == after
                && same_node(hook.root, root)
            {
                return true;
            }
            i += 1;
        }
        false
    }

    pub(super) const fn render_hooks(&mut self, after: bool) {
        let mut i = 0;
        while i < self.size.before + self.size.after {
            if let Some(hook) = self.hooks[i]
                && hook.after == after
            {
                self.single(hook.root, hook.node, hook.path);
            }
            i += 1;
        }
    }
}

pub(crate) const fn check_hooks(owner: &'static Node) {
    let (before, after) = (owner.before.0, owner.after.0);
    let mut i = 0;
    while i < before.len() + after.len() {
        let node = if i < before.len() {
            before[i]
        } else {
            after[i - before.len()]
        };
        hook(owner, node, Path::ROOT);
        i += 1;
    }
}

const fn hook(owner: &'static Node, hook: &'static Node, path: Path) -> (&'static Node, Path) {
    if let Kind::Table = hook.kind {
        fail(&[
            "`",
            hook.name.as_str(),
            "` is a table, so it can't be a hook of `",
            owner.name.as_str(),
            "`",
        ]);
    }
    let (node, path) = target(hook, path);
    if has(node, Probe::Hooks) {
        fail(&[
            "`",
            hook.name.as_str(),
            "` is a hook of `",
            owner.name.as_str(),
            "`, so it can't use items with hooks of its own",
        ]);
    }
    (node, path)
}

pub(crate) const fn hooked(root: &'static Node) -> bool {
    has(root, Probe::Hooks)
}
