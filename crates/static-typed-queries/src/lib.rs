pub use static_typed_queries_core::builder::Build;
pub use static_typed_queries_core::sql::Sql;
pub use static_typed_queries_core::statement::{Rows, Statement};
pub use static_typed_queries_macros::{query, statement, table};

#[cfg(feature = "parse-check")]
pub use static_typed_queries_core::check;

pub mod state {
    pub use static_typed_queries_core::builder::{Built, Filled, Missing, Open};
}

pub mod bind {
    pub use static_typed_queries_core::statement::bind::Bind;
    pub use static_typed_queries_core::statement::bind::path::Path;
    pub use static_typed_queries_core::statement::bind::slot::Slot;
}

pub mod dialect {
    pub use static_typed_queries_core::dialect::Dialect;
    #[cfg(feature = "mysql")]
    pub use static_typed_queries_core::dialect::mysql::MySql;
    #[cfg(feature = "postgres")]
    pub use static_typed_queries_core::dialect::postgres::Postgres;
    #[cfg(feature = "sqlite")]
    pub use static_typed_queries_core::dialect::sqlite::Sqlite;
    pub use static_typed_queries_core::embed::EmbedsIn;
}

pub mod prelude {
    #[cfg(feature = "mysql")]
    pub use crate::dialect::MySql;
    #[cfg(feature = "postgres")]
    pub use crate::dialect::Postgres;
    #[cfg(feature = "sqlite")]
    pub use crate::dialect::Sqlite;
    pub use crate::{Build, Sql, Statement, query, statement, table};
}

#[doc(hidden)]
pub mod __private {
    pub use static_typed_queries_core::*;
}
