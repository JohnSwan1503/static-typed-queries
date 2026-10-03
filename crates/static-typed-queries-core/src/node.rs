pub mod before;
pub mod fingerprint;
pub mod inject;
pub mod items;
pub mod kind;
pub mod name;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Node {
    pub name: name::Name,
    pub fingerprint: fingerprint::Fingerprint,
    pub kind: kind::Kind,
    pub inject: inject::Inject,
    pub parts: crate::part::Parts,
    pub before: before::Before,
    pub items: items::Items,
}
