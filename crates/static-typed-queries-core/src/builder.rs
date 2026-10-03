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
    message = "`{Self}` can't build `{P}`",
    label = "not every parameter is set"
)]
pub trait Finish<P> {
    fn finish(self) -> P;
}

pub struct Root;

pub trait Fill<B> {
    type Output;

    fn fill(self, builder: B) -> Self::Output;
}

impl<B> Fill<B> for Root {
    type Output = B;

    fn fill(self, builder: B) -> B {
        builder
    }
}

#[diagnostic::on_unimplemented(
    message = "there's nothing to set on this item",
    label = "it has no parameters, so it needs no call"
)]
pub trait Scope<K> {
    type Scoped;

    fn scope(self, parent: K) -> Self::Scoped;
}

pub struct NoParams;

impl Finish<()> for NoParams {
    fn finish(self) {}
}
