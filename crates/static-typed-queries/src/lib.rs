pub use static_typed_queries_core::{dialect, embed, node, part, render, sql, statement};
pub use static_typed_queries_core::{impl_debug, impl_display, impl_statement};

pub mod prelude {
    pub use crate::sql::Sql;
    pub use crate::statement::Statement;
}
