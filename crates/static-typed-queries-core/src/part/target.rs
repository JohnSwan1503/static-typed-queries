use crate::node::Node;

// What a part refers to: one of the enclosing node's items, by index, or a node that the
// enclosing items don't list, such as a bare type parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Item(u16),
    Node(&'static Node),
}
