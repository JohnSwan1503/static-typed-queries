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
    message = "`{Self}` still has parameters to set",
    label = "call every setter (as many times as its parameter appears) before building"
)]
pub trait Finish<P> {
    fn finish(self) -> P;
}

pub struct NoParams;

impl Finish<()> for NoParams {
    fn finish(self) {}
}
