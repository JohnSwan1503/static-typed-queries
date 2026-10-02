use crate::dialect::Dialect;

pub trait EmbedsIn<D: Dialect>: 'static {}
