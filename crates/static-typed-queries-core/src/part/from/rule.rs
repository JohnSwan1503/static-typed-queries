#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AliasRule {
    Given,
    NodeName,
}

impl AliasRule {
    pub const fn new<Rule: AliasRuleType>() -> AliasRule {
        Rule::RULE
    }
}

pub trait AliasRuleType: 'static {
    const RULE: AliasRule;
}

pub enum Given {}
pub enum NodeName {}

impl AliasRuleType for Given {
    const RULE: AliasRule = AliasRule::Given;
}
impl AliasRuleType for NodeName {
    const RULE: AliasRule = AliasRule::NodeName;
}
