use super::Node;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Items(pub &'static [&'static Node]);
