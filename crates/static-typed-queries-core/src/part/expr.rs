use crate::node::{Node, kind::Kind, name::Name};

use super::Part;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Expr(&'static Node);

impl Expr {
    pub const fn part(expr: &'static Node) -> Part {
        Part::Expr(Expr(expr))
    }

    pub const fn name(&self) -> Name {
        self.0.name
    }

    pub const fn kind(&self) -> Kind {
        self.0.kind
    }

    pub const fn as_ref(&self) -> &'static Node {
        self.0
    }
}
