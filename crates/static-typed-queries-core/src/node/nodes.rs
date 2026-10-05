use super::Node;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Nodes(pub &'static [&'static Node]);
