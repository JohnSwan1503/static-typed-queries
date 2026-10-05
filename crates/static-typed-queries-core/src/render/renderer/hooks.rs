use crate::dialect::Dialect;
use crate::node::Node;
use crate::node::kind::Kind;
use crate::render::error::fail;
use crate::statement::bind::path::Path;

use super::Renderer;
use super::names::same_node;
use super::paths::{Probe, has, resolve_node, target, unwrap_scope};

#[derive(Clone, Copy)]
pub(super) struct Hook {
    root: &'static Node,
    node: &'static Node,
    path: Path,
    after: bool,
}

impl<'a, D: Dialect> Renderer<'a, D> {
    #[track_caller]
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

    #[track_caller]
    const fn add_hooks(&mut self, owner: &'static Node, after: bool) {
        let hooks = if after { owner.after.0 } else { owner.before.0 };
        let mut i = 0;
        while i < hooks.len() {
            let root = hooks[i];
            let (node, path) = hook(owner, root, Path::ROOT);
            if !self.has_hook(root, after) {
                self.hooks.push(
                    Hook {
                        root,
                        node,
                        path,
                        after,
                    },
                    "a statement can't have more than 64 hooks",
                );
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
        while let Some(hook) = self.hooks.get(i) {
            if hook.after == after && same_node(hook.root, root) {
                return true;
            }
            i += 1;
        }
        false
    }

    #[track_caller]
    pub(super) const fn render_hooks(&mut self, after: bool) {
        let mut i = 0;
        while let Some(hook) = self.hooks.get(i) {
            if hook.after == after {
                self.single(hook.root, hook.node, hook.path);
            }
            i += 1;
        }
    }
}

#[track_caller]
pub const fn check_hooks(owner: &'static Node) {
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

#[track_caller]
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

pub const fn hooked(root: &'static Node) -> bool {
    has(root, Probe::Hooks)
}
