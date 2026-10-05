use core::marker::PhantomData;

use super::Finish;
use crate::sql::Sql;

pub struct NoHooks;

pub struct WithHooks;

pub struct HookFlag<const HOOKED: bool>;

pub trait HookState {
    type Out;
}

impl HookState for HookFlag<false> {
    type Out = NoHooks;
}

impl HookState for HookFlag<true> {
    type Out = WithHooks;
}

pub struct HookValues<H: Sql>(pub H::Params);

#[diagnostic::on_unimplemented(
    message = "only hooks with parameters take values",
    label = "builds no parameters",
    note = "a hook without parameters runs without a `with`"
)]
pub trait ParamsOf {
    type Item: Sql<Params = Self>;
}

pub type With<B, V> = (HookValues<<<B as Finish>::Params as ParamsOf>::Item>, V);

pub fn with<B, V>(builder: B, values: V) -> With<B, V>
where
    B: Finish,
    B::Params: ParamsOf,
{
    (HookValues(builder.finish()), values)
}

pub struct Here;

pub struct There<I>(PhantomData<I>);

pub struct Free;

#[diagnostic::on_unimplemented(
    message = "no values for the hook `{H}`",
    label = "`{H}` has parameters",
    note = "give them with `.with({H}::builder()…)`"
)]
pub trait Provides<H, I> {}

#[diagnostic::do_not_recommend]
impl<H: Sql, Rest> Provides<H, Here> for (HookValues<H>, Rest) {}

#[diagnostic::do_not_recommend]
impl<H, X, Rest: Provides<H, I>, I> Provides<H, There<I>> for (X, Rest) {}

pub trait Demand<V, I> {}

impl<V> Demand<V, Free> for () {}

impl<V, P: ParamsOf, I> Demand<V, I> for P where V: Provides<P::Item, I> {}

pub trait HookNeeds<V, I> {}
