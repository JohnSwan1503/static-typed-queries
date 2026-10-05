mod scope;
mod state;

pub use scope::*;
pub use state::*;

use crate::sql::Sql;

/// Items with a builder, which sets their fields one at a time.
pub trait Build: Sql {
    /// The builder. It has a setter for each field while that field is unset, and `build()` once
    /// every field is set; an item without fields is built from the start.
    type Builder;

    /// A builder with every field unset.
    fn builder() -> Self::Builder;
}
