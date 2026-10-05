use crate::dialect::Dialect;
use crate::node::Node;
use crate::node::inject::Inject;
use crate::node::kind::Kind;
use crate::part::Part;
use crate::part::target::Target;
use crate::render::error::fail;
use crate::statement::bind::path::Path;

use super::names::{same_node, suffixed_eq};
use super::paths::{embedded, instance_path, resolve, target};
use super::placement::{attachable, placement};
use super::{MAX_CTES, Renderer};

#[derive(Clone, Copy)]
pub(super) struct Cte {
    node: &'static Node,
    path: Path,
    suffix: u16,
}

#[derive(Clone, Copy)]
pub(super) struct Attached {
    node: &'static Node,
    path: Path,
}

impl<'a, D: Dialect> Renderer<'a, D> {
    pub(super) const fn attach(&mut self, root: &'static Node, step: Target) {
        let (node, path) = resolve(root, Path::ROOT, step);
        let (node, path) = target(node, path);
        attachable::<D>(node);
        if self.attached_count == MAX_CTES {
            fail(&["a step can't have more than 64 CTE steps attached"]);
        }
        self.attached[self.attached_count] = Some(Attached { node, path });
        self.attached_count += 1;
    }

    pub(super) const fn collect_attached(&mut self) {
        let mut i = 0;
        while i < self.attached_count {
            if let Some(attached) = self.attached[i] {
                self.collect(attached.node, attached.path);
                self.add_cte(attached.node, attached.path, false);
            }
            i += 1;
        }
    }

    pub(super) const fn collect(&mut self, node: &'static Node, path: Path) {
        let parts = node.parts.0;
        let mut i = 0;
        while i < parts.len() {
            match parts[i] {
                Part::Expr(expr) => {
                    let (child, child_path) = embedded(node, path, expr.target());
                    if !matches!(child.kind, Kind::Query) {
                        fail(&[
                            "`",
                            child.name.as_str(),
                            "` isn't a query, so it can't be used as an expression",
                        ]);
                    }
                    self.collect(child, child_path);
                }
                Part::From(from) => {
                    let (child, child_path) = embedded(node, path, from.target());
                    match placement::<D>(from, child) {
                        Inject::Cte { recursive } => {
                            self.collect(child, child_path);
                            self.add_cte(child, child_path, recursive);
                        }
                        Inject::Ident | Inject::Subquery => self.collect(child, child_path),
                    }
                }
                Part::Lit(_) | Part::Ident(_) | Part::Param(_) => {}
            }
            i += 1;
        }
    }

    const fn add_cte(&mut self, node: &'static Node, path: Path, recursive: bool) {
        self.recursive |= recursive;
        let path = instance_path(node, path);
        if self.cte_suffix(node, path).is_some() {
            return;
        }
        let name = node.name.as_str();
        let mut suffix = 0;
        while self.cte_name_taken(name, suffix) {
            suffix = if suffix == 0 { 2 } else { suffix + 1 };
        }
        if suffix != 0 && recursive {
            if self.has_cte(node) {
                fail(&[
                    "`",
                    name,
                    "` is recursive and has parameters, so it can only be reached through one path",
                ]);
            }
            fail(&[
                "two different CTEs are named `",
                name,
                "`, and the second one is recursive, so it can't be renamed",
            ]);
        }
        if self.cte_count == MAX_CTES {
            fail(&["a statement can't have more than 64 CTEs"]);
        }
        self.ctes[self.cte_count] = Some(Cte { node, path, suffix });
        self.cte_count += 1;
    }

    const fn has_cte(&self, node: &'static Node) -> bool {
        let mut i = 0;
        while i < self.cte_count {
            if let Some(cte) = self.ctes[i]
                && same_node(cte.node, node)
            {
                return true;
            }
            i += 1;
        }
        false
    }

    pub(super) const fn cte_suffix(&self, node: &'static Node, path: Path) -> Option<u16> {
        let mut i = 0;
        while i < self.cte_count {
            if let Some(cte) = self.ctes[i]
                && same_node(cte.node, node)
                && cte.path.same(&path)
            {
                return Some(cte.suffix);
            }
            i += 1;
        }
        None
    }

    const fn cte_name_taken(&self, name: &str, suffix: u16) -> bool {
        let mut i = 0;
        while i < self.cte_count {
            if let Some(cte) = self.ctes[i]
                && suffixed_eq(cte.node.name.as_str(), cte.suffix, name, suffix)
            {
                return true;
            }
            i += 1;
        }
        false
    }

    pub(super) const fn with_clause(&mut self) {
        if self.cte_count == 0 {
            return;
        }
        self.push(if self.recursive {
            "WITH RECURSIVE "
        } else {
            "WITH "
        });
        let mut i = 0;
        while i < self.cte_count {
            if let Some(cte) = self.ctes[i] {
                if i > 0 {
                    self.push(", ");
                }
                self.quoted(cte.node.name.as_str(), cte.suffix);
                self.push(" AS (");
                self.body(cte.node, cte.path);
                self.push(")");
            }
            i += 1;
        }
        self.push(" ");
    }
}
