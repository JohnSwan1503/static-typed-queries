pub use static_typed_queries_core::builder::{Build, Finish};
pub use static_typed_queries_core::hooks::HookValues;
pub use static_typed_queries_core::sql::Sql;
pub use static_typed_queries_core::statement::command::Command;
pub use static_typed_queries_core::statement::{Rows, Statement};
pub use static_typed_queries_core::transaction::Transaction;
pub use static_typed_queries_core::values::{Valueless, Values};
#[cfg(any(feature = "mysql", feature = "postgres", feature = "sqlite"))]
pub use static_typed_queries_core::with::{Run, Running, With};
pub use static_typed_queries_macros::{query, sql, statement, table, transaction};

#[doc(hidden)]
pub use static_typed_queries_macros::__query_sql;

#[cfg(feature = "parse-check")]
pub use static_typed_queries_core::check;

pub mod state {
    pub use static_typed_queries_core::builder::{Built, Filled, Missing, Open};
    pub use static_typed_queries_core::hooks::{NoHooks, WithHooks};

    // Not for use, but reachable outside `__private`, so rustc prints these short in errors.
    #[doc(hidden)]
    pub use static_typed_queries_core::embed::{Checked, CteIn, DmlInCte, checked, cte, embeds};
    #[doc(hidden)]
    pub use static_typed_queries_core::hooks::{
        Free, HasHooks, Here, HookNeeds, Hooked, Provides, Single, Spent, There, Unhooked,
        Unreached, Used,
    };
    #[doc(hidden)]
    pub use static_typed_queries_core::render::Render;
    #[cfg(any(feature = "mysql", feature = "postgres", feature = "sqlite"))]
    #[doc(hidden)]
    pub use static_typed_queries_core::statement::run::rows;
    #[doc(hidden)]
    pub use static_typed_queries_core::statement::{NotTable, NotTransaction, SingleRef, runnable};
    #[doc(hidden)]
    pub use static_typed_queries_core::values::valueless;
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
    pub use crate::{
        Build, Sql, Statement, Transaction, query, sql, statement, table, transaction,
    };
}

#[doc(hidden)]
pub mod __private {
    pub use static_typed_queries_core::*;
}
