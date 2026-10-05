//! Everything `#[query]` generates is documented.
#![deny(missing_docs)]

use std::marker::PhantomData;

use static_typed_queries::prelude::*;

/// Users.
#[table(Postgres, name = "users")]
pub struct Users;

/// The users of one organisation.
#[query(Postgres, sql = "SELECT id FROM {Users} WHERE org_id = {org_id}")]
pub struct ActiveUsers {
    /// The organisation.
    pub org_id: i64,
}

/// Counts any item.
#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

/// A report.
#[query(
    Postgres,
    sql = "
    SELECT {CountOf<Users>} AS total FROM {active}
    WHERE id IN ({first}, {second}) AND kind = {kind}
    LIMIT {limit}"
)]
pub struct Report {
    /// One id.
    pub first: i64,
    /// Another id.
    pub second: i64,
    /// The kind.
    pub kind: String,
    /// The limit.
    pub limit: i64,
    /// The users to report on.
    #[cte]
    pub active: ActiveUsers,
}

/// Just one.
#[query(Postgres, sql = "SELECT 1")]
pub struct One;

/// The report as a statement.
#[statement(report)]
pub struct Printed {
    /// The report.
    pub report: Report,
}

/// Both in one transaction.
#[transaction(Postgres, steps(report, One))]
pub struct Both {
    /// The report.
    pub report: Report,
}

fn main() {}
