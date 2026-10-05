use crate::dialect::Dialect;

#[diagnostic::on_unimplemented(
    message = "{Self} SQL can't be embedded in a {D} statement",
    label = "this item is written for {Self}"
)]
pub trait EmbedsIn<D: Dialect>: Dialect {}

impl<D: Dialect> EmbedsIn<D> for D {}

pub trait Checked {}

pub const fn embeds<D: Dialect, E: EmbedsIn<D>>() {}

pub const fn checked<T: Checked>() {}
