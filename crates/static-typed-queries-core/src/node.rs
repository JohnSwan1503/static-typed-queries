pub mod fingerprint;
pub mod inject;
pub mod kind;
pub mod name;
pub mod nodes;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Node {
    pub name: name::Name,
    pub fingerprint: fingerprint::Fingerprint,
    pub kind: kind::Kind,
    pub inject: inject::Inject,
    pub parts: crate::part::Parts,
    pub before: nodes::Nodes,
    pub after: nodes::Nodes,
    pub items: nodes::Nodes,
}
