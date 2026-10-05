use crate::dialect::Dialect;
use crate::node::Node;
use crate::node::inject::Inject;
use crate::part::Part;
use crate::part::from::From;
use crate::part::from::rule::AliasRule;
use crate::part::param::Param;
use crate::render::error::fail;
use crate::statement::bind::Bind;
use crate::statement::bind::path::Path;
use crate::statement::bind::slot::Slot;

use super::Renderer;
use super::names::digits;
use super::paths::{embedded, instance_path};
use super::placement::placement;

#[derive(Clone, Copy)]
pub(super) struct Numbered {
    path: Path,
    slot: Slot,
}

impl<'a, D: Dialect> Renderer<'a, D> {
    #[track_caller]
    pub(super) const fn body(&mut self, node: &'static Node, path: Path) {
        let parts = node.parts.0;
        let mut i = 0;
        while i < parts.len() {
            match parts[i] {
                Part::Lit(lit) => self.push(lit.as_str()),
                Part::Ident(ident) => self.quoted(ident.0, 0),
                Part::Param(param) => self.param(node, path, param),
                Part::Expr(expr) => {
                    let (child, child_path) = embedded(node, path, expr.target());
                    self.push("(");
                    self.body(child, child_path);
                    self.push(")");
                }
                Part::From(from) => self.from(node, path, from),
            }
            i += 1;
        }
    }

    #[track_caller]
    const fn from(&mut self, node: &'static Node, path: Path, from: From) {
        let (child, child_path) = embedded(node, path, from.target());
        match placement::<D>(from, child) {
            Inject::Ident => self.body(child, child_path),
            Inject::Cte { .. } => match self.cte_suffix(child, instance_path(child, child_path)) {
                Some(suffix) => self.quoted(child.name.as_str(), suffix),
                None => panic!("CTE wasn't collected before rendering"),
            },
            Inject::Subquery => {
                self.push("(");
                self.body(child, child_path);
                self.push(")");
                if let AliasRule::NodeName = from.rule() {
                    self.push(" AS ");
                    self.quoted(child.name.as_str(), 0);
                }
            }
        }
    }

    #[track_caller]
    const fn param(&mut self, node: &'static Node, path: Path, param: Param) {
        self.byte(D::PARAMS.prefix);
        if !D::PARAMS.numbered {
            self.bind(node, path, param);
            return;
        }
        let slot = Slot::from_param(param);
        let number = match self.number_of(path, slot) {
            Some(number) => number,
            None => {
                self.numbered.push(
                    Numbered { path, slot },
                    "a statement can't have more than 1024 distinct parameters",
                );
                self.bind(node, path, param);
                self.numbered.len() as u16
            }
        };
        self.number(number);
    }

    const fn number_of(&self, path: Path, slot: Slot) -> Option<u16> {
        let mut i = 0;
        while let Some(numbered) = self.numbered.get(i) {
            if numbered.slot.inner() == slot.inner() && numbered.path.same(&path) {
                return Some(i as u16 + 1);
            }
            i += 1;
        }
        None
    }

    #[track_caller]
    const fn bind(&mut self, node: &'static Node, path: Path, param: Param) {
        if self.size.binds - self.first_bind == u16::MAX as usize {
            fail(&["a statement can't have more than 65535 parameters"]);
        }
        if !self.binds.is_empty() {
            self.binds[self.size.binds] = Bind::from_param(node, path, param);
        }
        self.size.binds += 1;
    }

    const fn number(&mut self, n: u16) {
        let (digits, len) = digits(n);
        let mut i = 0;
        while i < len {
            self.byte(digits[i]);
            i += 1;
        }
    }

    pub(super) const fn quoted(&mut self, ident: &str, suffix: u16) {
        let [open, close] = D::QUOTE.as_array();
        self.byte(open);
        let bytes = ident.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == close {
                self.byte(close);
            }
            self.byte(bytes[i]);
            i += 1;
        }
        if suffix != 0 {
            self.byte(b'_');
            self.number(suffix);
        }
        self.byte(close);
    }

    pub(super) const fn push(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            self.byte(bytes[i]);
            i += 1;
        }
    }

    const fn byte(&mut self, byte: u8) {
        if !self.sql.is_empty() {
            self.sql[self.size.sql] = byte;
        }
        self.size.sql += 1;
    }
}
