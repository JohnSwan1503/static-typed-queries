use crate::node::Node;
use crate::node::kind::Kind;
use crate::part::Part;
use crate::part::target::Target;
use crate::render::error::fail;
use crate::statement::bind::path::Path;

// The node a part refers to from `node`, and the path to its values. A node referenced by type
// holds no values, so it keeps the path it is reached through.
#[track_caller]
pub(super) const fn resolve(
    node: &'static Node,
    path: Path,
    target: Target,
) -> (&'static Node, Path) {
    match target {
        Target::Item(index) => item(node, path, index),
        Target::Node(child) => {
            if has(child, Probe::Params) {
                fail(&[
                    "`",
                    child.name.as_str(),
                    "` holds values, so it has to be a field of the query that uses it",
                ]);
            }
            (child, path)
        }
    }
}

// An embedded item, looking through a statement wrapper to the statement it names.
#[track_caller]
pub(super) const fn embedded(
    node: &'static Node,
    path: Path,
    target: Target,
) -> (&'static Node, Path) {
    let (child, path) = resolve(node, path, target);
    unwrap(child, path)
}

// The node a part refers to from `node`, with statement wrappers unwrapped.
pub(super) const fn resolve_node(node: &'static Node, target: Target) -> &'static Node {
    match target {
        Target::Item(index) => unwrap_scope(node.items.0[index as usize]),
        Target::Node(child) => child,
    }
}

pub(super) const fn instance_path(node: &'static Node, path: Path) -> Path {
    if has(node, Probe::Params) {
        path
    } else {
        Path::ROOT
    }
}

#[track_caller]
const fn unwrap(node: &'static Node, path: Path) -> (&'static Node, Path) {
    match node.kind {
        Kind::Scope => item(node, path, 0),
        _ => (node, path),
    }
}

#[track_caller]
const fn item(node: &'static Node, path: Path, index: u16) -> (&'static Node, Path) {
    match path.child(index) {
        Some(path) => (node.items.0[index as usize], path),
        None => fail(&["items can't be nested more than 16 deep"]),
    }
}

#[track_caller]
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

#[derive(Clone, Copy)]
pub(super) enum Probe {
    Hooks,
    Params,
}

// Whether `node`, or an item it embeds at any depth, has hooks or binds a value.
pub(super) const fn has(node: &'static Node, probe: Probe) -> bool {
    let node = unwrap_scope(node);
    if let Probe::Hooks = probe
        && !(node.before.0.is_empty() && node.after.0.is_empty())
    {
        return true;
    }
    let parts = node.parts.0;
    let mut i = 0;
    while i < parts.len() {
        let found = match (parts[i], probe) {
            (Part::Param(_), Probe::Params) => true,
            (part, _) => match part.target() {
                Some(target) => has(resolve_node(node, target), probe),
                None => false,
            },
        };
        if found {
            return true;
        }
        i += 1;
    }
    false
}
