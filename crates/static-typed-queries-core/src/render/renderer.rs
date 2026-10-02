use core::marker::PhantomData;

use super::Size;
use super::error::fail;
use crate::dialect::Dialect;
use crate::node::Node;
use crate::node::kind::Kind;
use crate::part::Part;
use crate::statement::bind::Bind;
use crate::statement::bind::path::Path;
use crate::statement::bind::slot::Slot;

const MAX_NUMBERED: usize = 1024;

#[derive(Clone, Copy)]
struct Numbered {
    path: Path,
    slot: Slot,
}

pub(super) struct Renderer<'a, D> {
    sql: &'a mut [u8],
    binds: &'a mut [Bind],
    size: Size,
    numbered: [Option<Numbered>; MAX_NUMBERED],
    numbered_count: usize,
    dialect: PhantomData<D>,
}

impl<'a, D: Dialect> Renderer<'a, D> {
    pub(super) const fn new(sql: &'a mut [u8], binds: &'a mut [Bind]) -> Self {
        Self {
            sql,
            binds,
            size: Size { sql: 0, binds: 0 },
            numbered: [None; MAX_NUMBERED],
            numbered_count: 0,
            dialect: PhantomData,
        }
    }

    pub(super) const fn size(&self) -> Size {
        self.size
    }

    pub(super) const fn statement(&mut self, root: &'static Node) {
        if let Kind::Table = root.kind {
            fail(&["`", root.name.as_str(), "` is a table, not a statement"]);
        }
        self.body(root, Path::ROOT);
    }

    const fn body(&mut self, node: &'static Node, path: Path) {
        let parts = node.parts.0;
        let mut i = 0;
        while i < parts.len() {
            match parts[i] {
                Part::Lit(lit) => self.push(lit.as_str()),
                Part::Ident(ident) => self.quoted(ident.0),
                Part::Param(param) => self.param(node, path, Slot::from_param(param)),
            }
            i += 1;
        }
    }

    const fn param(&mut self, node: &'static Node, path: Path, slot: Slot) {
        self.byte(D::PARAMS.prefix());
        if !D::PARAMS.numbered() {
            self.bind(node, path, slot);
            return;
        }
        let number = match self.number_of(path, slot) {
            Some(number) => number,
            None => {
                if self.numbered_count == MAX_NUMBERED {
                    fail(&["a statement can't have more than 1024 distinct parameters"]);
                }
                self.numbered[self.numbered_count] = Some(Numbered { path, slot });
                self.numbered_count += 1;
                self.bind(node, path, slot);
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

    const fn bind(&mut self, node: &'static Node, path: Path, slot: Slot) {
        if self.size.binds == u16::MAX as usize {
            fail(&["a statement can't have more than 65535 parameters"]);
        }
        if !self.binds.is_empty() {
            self.binds[self.size.binds] = Bind::from_node(node, path, slot);
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

    const fn quoted(&mut self, ident: &str) {
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
        self.byte(close);
    }

    const fn push(&mut self, s: &str) {
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
