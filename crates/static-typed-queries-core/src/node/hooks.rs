use super::Node;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hooks(pub &'static [&'static Node]);
