use crate::dialect;
use crate::node;

pub trait Sql: 'static {
    type Dialect: dialect::Dialect;
    const NODE: &'static node::Node;
}
