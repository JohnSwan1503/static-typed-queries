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

use super::paths::instance_path;
use super::placement::placement;
use super::{MAX_NUMBERED, Renderer};

#[derive(Clone, Copy)]
pub(super) struct Numbered {
    path: Path,
    slot: Slot,
}

impl<'a, D: Dialect> Renderer<'a, D> {
    pub(super) const fn body(&mut self, node: &'static Node, path: Path) {
        let parts = node.parts.0;
        let mut i = 0;
        while i < parts.len() {
            match parts[i] {
                Part::Lit(lit) => self.push(lit.as_str()),
                Part::Ident(ident) => self.quoted(ident.0, 0),
                Part::Param(param) => self.param(node, path, param),
                Part::Expr(expr) => {
                    self.push("(");
                    self.body(expr.as_ref(), self.child_path(path, expr.as_ref()));
                    self.push(")");
                }
                Part::From(from) => self.from(path, from),
            }
            i += 1;
        }
    }

    const fn from(&mut self, path: Path, from: From) {
        let child = from.node();
        let child_path = self.child_path(path, child);
        match placement::<D>(from) {
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

    const fn param(&mut self, node: &'static Node, path: Path, param: Param) {
        self.byte(D::PARAMS.prefix());
        if !D::PARAMS.numbered() {
            self.bind(node, path, param);
            return;
        }
        let slot = Slot::from_param(param);
        let number = match self.number_of(path, slot) {
            Some(number) => number,
            None => {
                if self.numbered_count == MAX_NUMBERED {
                    fail(&["a statement can't have more than 1024 distinct parameters"]);
                }
                self.numbered[self.numbered_count] = Some(Numbered { path, slot });
                self.numbered_count += 1;
                self.bind(node, path, param);
                self.numbered_count as u16
            }
        };
        self.number(number);
    }

    const fn number_of(&self, path: Path, slot: Slot) -> Option<u16> {
        let mut i = 0;
        while i < self.numbered_count {
            if let Some(numbered) = self.numbered[i]
                && numbered.slot.inner() == slot.inner()
                && numbered.path.same(&path)
            {
                return Some(i as u16 + 1);
            }
            i += 1;
        }
        None
    }

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
        let mut digits = [0; 5];
        let mut count = 0;
        let mut rest = n;
        loop {
            digits[count] = b'0' + (rest % 10) as u8;
            count += 1;
            rest /= 10;
            if rest == 0 {
                break;
            }
        }
        while count > 0 {
            count -= 1;
            self.byte(digits[count]);
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
