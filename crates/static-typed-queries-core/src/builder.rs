mod hooks;
mod scope;
mod state;

pub use hooks::*;
pub use scope::*;
pub use state::*;

use crate::sql::Sql;

pub trait Build: Sql {
    type Builder;

    fn builder() -> Self::Builder;
}
