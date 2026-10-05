use crate::dialect;
use crate::node;

/// An item written in SQL: a table, query, statement or transaction declared with one of the
/// crate's attributes.
pub trait Sql: 'static {
    /// The dialect the item's SQL is written for.
    type Dialect: dialect::Dialect;

    #[doc(hidden)]
    const NODE: &'static node::Node;
}
