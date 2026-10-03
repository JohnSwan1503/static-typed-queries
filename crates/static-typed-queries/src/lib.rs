pub use static_typed_queries_core::{builder, dialect, embed, node, part, render, sql, statement};
pub use static_typed_queries_core::{impl_debug, impl_display, impl_statement};
pub use static_typed_queries_macros::{query, table};

#[doc(hidden)]
pub use static_typed_queries_core::{__if_sqlx, __private};

pub mod prelude {
    pub use crate::builder::Build;
    pub use crate::sql::Sql;
    pub use crate::statement::Statement;
    pub use crate::{query, table};
}
