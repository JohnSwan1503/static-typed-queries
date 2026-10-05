use crate::dialect::Dialect;

#[diagnostic::on_unimplemented(
    message = "{Self} SQL can't be embedded in a {D} statement",
    label = "this item is written for {Self}"
)]
pub trait EmbedsIn<D: Dialect>: Dialect {}

impl<D: Dialect> EmbedsIn<D> for D {}

pub trait Checked {}

// A query that modifies data implements this only for dialects that run such a query as a CTE.
#[diagnostic::on_unimplemented(
    message = "`{Self}` modifies data, which {D} doesn't allow in a CTE",
    label = "modifies data",
    note = "run it as a step of its own in a `#[transaction]`"
)]
pub trait CteIn<D: Dialect> {}

pub trait DmlInCte: Dialect {}

pub const fn embeds<D: Dialect, E: EmbedsIn<D>>() {}

pub const fn checked<T: Checked>() {}

pub const fn cte<D: Dialect, T: CteIn<D>>() {}
