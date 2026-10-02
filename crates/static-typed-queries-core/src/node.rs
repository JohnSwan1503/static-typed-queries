pub mod before;
pub mod fingerprint;
pub mod inject;
pub mod kind;
pub mod name;

#[doc(inline)]
pub use crate::part as parts;

pub fn foo() -> parts::Parts {
    let foo: &'static [parts::Part] = &[];
    parts::Parts(foo)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Node {
    pub name: name::Name,
    pub fingerprint: fingerprint::Fingerprint,
    pub kind: kind::Kind,
    pub inject: inject::Inject,
    pub parts: parts::Parts,
    pub before: before::Before,
}

impl Node {}
