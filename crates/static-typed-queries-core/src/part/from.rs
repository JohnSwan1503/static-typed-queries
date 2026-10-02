pub mod rule;

use crate::node::{Node, inject::Inject};
use crate::part::Part;

use rule::AliasRule;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct From {
    node: &'static Node,
    rule: AliasRule,
    inject: Option<Inject>,
}

impl From {
    pub const fn part(node: &'static Node, rule: AliasRule, inject: Option<Inject>) -> Part {
        Part::From(From { node, rule, inject })
    }

    pub const fn node(&self) -> &'static Node {
        self.node
    }

    pub const fn rule(&self) -> AliasRule {
        self.rule
    }

    pub const fn inject(&self) -> Inject {
        match self.inject {
            Some(inject) => inject,
            _ => self.node.inject,
        }
    }
}
