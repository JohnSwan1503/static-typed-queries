pub mod rule;

use crate::node::{Node, inject::Inject};
use crate::part::Part;
use crate::part::target::Target;

use rule::AliasRule;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct From {
    target: Target,
    rule: AliasRule,
    inject: Option<Inject>,
}

impl From {
    pub const fn part(target: Target, rule: AliasRule, inject: Option<Inject>) -> Part {
        Part::From(From {
            target,
            rule,
            inject,
        })
    }

    pub const fn target(&self) -> Target {
        self.target
    }

    pub const fn rule(&self) -> AliasRule {
        self.rule
    }

    pub const fn inject(&self, node: &'static Node) -> Inject {
        match self.inject {
            Some(inject) => inject,
            _ => node.inject,
        }
    }
}
