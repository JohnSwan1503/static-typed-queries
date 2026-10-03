use core::marker::PhantomData;

use crate::sql::Sql;

pub struct Set;

pub struct Unset<Rest>(PhantomData<Rest>);

pub trait Remaining {
    const N: usize;
}

impl Remaining for Set {
    const N: usize = 0;
}

impl<Rest: Remaining> Remaining for Unset<Rest> {
    const N: usize = Rest::N + 1;
}

pub trait Build: Sql {
    type Builder;

    fn builder() -> Self::Builder;
}

#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't a builder for `{P}`",
    label = "an item's closure must return the builder it was given"
)]
pub trait Finish<P> {
    fn finish(self) -> P;
}

pub struct NoParams;

impl Finish<()> for NoParams {
    fn finish(self) {}
}
