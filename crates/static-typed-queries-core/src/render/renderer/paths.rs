use crate::dialect::Dialect;
use crate::node::Node;
use crate::node::kind::Kind;
use crate::part::Part;
use crate::part::target::Target;
use crate::render::error::fail;
use crate::statement::bind::path::Path;

use super::Renderer;
use super::names::same_node;

impl<'a, D: Dialect> Renderer<'a, D> {
    // The node a part refers to from `node`, and the path to its values.
    pub(super) const fn resolve(
        &self,
        node: &'static Node,
        path: Path,
        target: Target,
    ) -> (&'static Node, Path) {
        match target {
            Target::Item(index) => match path.child(index) {
                Some(path) => (node.items.0[index as usize], path),
                None => fail(&["items can't be nested more than 16 deep"]),
            },
            Target::Node(child) => (child, self.child_path(path, child)),
        }
    }

    // An embedded item, looking through a statement wrapper to the statement it names.
    pub(super) const fn embedded(
        &self,
        node: &'static Node,
        path: Path,
        target: Target,
    ) -> (&'static Node, Path) {
        let (child, path) = self.resolve(node, path, target);
        unwrap(child, path)
    }

    const fn child_path(&self, path: Path, child: &'static Node) -> Path {
        let root = match self.root {
            Some(root) => root,
            None => panic!("rendering started without a root"),
        };
        let mut depth = path.steps().len();
        loop {
            if let Some(path) = item_path(frame(root, path, depth), path.prefix(depth), child) {
                return path;
            }
            if depth == 0 {
                break;
            }
            depth -= 1;
        }
        if has_params(child) {
            fail(&[
                "`",
                child.name.as_str(),
                "` has parameters, but no query it's used in lists it among its items",
            ]);
        }
        path
    }
}

// The node a part refers to from `node`, with statement wrappers unwrapped.
pub(super) const fn resolve_node(node: &'static Node, target: Target) -> &'static Node {
    match target {
        Target::Item(index) => unwrap_scope(node.items.0[index as usize]),
        Target::Node(child) => child,
    }
}

const fn frame(root: &'static Node, path: Path, depth: usize) -> &'static Node {
    let steps = path.steps();
    let mut node = root;
    let mut i = 0;
    while i < depth {
        node = node.items.0[steps[i] as usize];
        i += 1;
    }
    node
}

const fn item_path(frame: &'static Node, prefix: Path, child: &'static Node) -> Option<Path> {
    let items = frame.items.0;
    let mut i = 0;
    while i < items.len() {
        let item = items[i];
        let path = if same_node(item, child) {
            prefix.child(i as u16)
        } else if let Kind::Scope = item.kind
            && same_node(item.items.0[0], child)
        {
            match prefix.child(i as u16) {
                Some(path) => path.child(0),
                None => None,
            }
        } else {
            i += 1;
            continue;
        };
        return match path {
            Some(path) => Some(path),
            None => fail(&["items can't be nested more than 16 deep"]),
        };
    }
    None
}

pub(super) const fn instance_path(node: &'static Node, path: Path) -> Path {
    if has_params(node) { path } else { Path::ROOT }
}

const fn unwrap(node: &'static Node, path: Path) -> (&'static Node, Path) {
    match (node.kind, path.child(0)) {
        (Kind::Scope, Some(path)) => (node.items.0[0], path),
        (Kind::Scope, None) => fail(&["items can't be nested more than 16 deep"]),
        _ => (node, path),
    }
}

pub(super) const fn target(node: &'static Node, path: Path) -> (&'static Node, Path) {
    let (node, path) = unwrap(node, path);
    match node.kind {
        Kind::Table => fail(&["`", node.name.as_str(), "` is a table, not a statement"]),
        Kind::Transaction => fail(&[
            "`",
            node.name.as_str(),
            "` is a transaction, so it can't be a step or a hook",
        ]),
        _ => {}
    }
    (node, path)
}

pub(super) const fn unwrap_scope(node: &'static Node) -> &'static Node {
    match node.kind {
        Kind::Scope => node.items.0[0],
        _ => node,
    }
}

const fn has_params(node: &'static Node) -> bool {
    let parts = node.parts.0;
    let mut i = 0;
    while i < parts.len() {
        let found = match parts[i] {
            Part::Param(_) => true,
            Part::Expr(expr) => has_params(resolve_node(node, expr.target())),
            Part::From(from) => has_params(resolve_node(node, from.target())),
            Part::Lit(_) | Part::Ident(_) => false,
        };
        if found {
            return true;
        }
        i += 1;
    }
    false
}
