use core::marker::PhantomData;

use crate::sql::Sql;

pub struct Filled;

pub struct Missing<Rest>(PhantomData<Rest>);

pub trait Remaining {
    const N: usize;
}

impl Remaining for Filled {
    const N: usize = 0;
}

impl<Rest: Remaining> Remaining for Missing<Rest> {
    const N: usize = Rest::N + 1;
}

pub struct Open<B>(pub B);

pub struct Built<P>(pub P);

pub struct True;

pub struct False;

pub trait Ready {
    type Out;
}

impl Ready for Filled {
    type Out = True;
}

impl<Rest> Ready for Missing<Rest> {
    type Out = False;
}

impl<B> Ready for Open<B> {
    type Out = False;
}

impl<P> Ready for Built<P> {
    type Out = True;
}

pub trait And<B> {
    type Out;
}

impl And<True> for True {
    type Out = True;
}

impl And<False> for True {
    type Out = False;
}

impl<B> And<B> for False {
    type Out = False;
}

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

pub trait Build: Sql {
    type Builder;

    fn builder() -> Self::Builder;
}

pub trait Finish {
    type Params;

    fn finish(self) -> Self::Params;
}

pub trait Settle<B> {
    type Out;

    fn settle(builder: B) -> Self::Out;
}

impl<B: Finish> Settle<B> for True {
    type Out = Built<B::Params>;

    fn settle(builder: B) -> Self::Out {
        Built(builder.finish())
    }
}

impl<B> Settle<B> for False {
    type Out = Open<B>;

    fn settle(builder: B) -> Self::Out {
        Open(builder)
    }
}

pub trait Settled {
    type Slot;

    fn settled(self) -> Self::Slot;
}

impl<B: Ready> Settled for B
where
    B::Out: Settle<B>,
{
    type Slot = <B::Out as Settle<B>>::Out;

    fn settled(self) -> Self::Slot {
        <B::Out as Settle<B>>::settle(self)
    }
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

pub trait Scope<K> {
    type Scoped;

    fn scope(self, parent: K) -> Self::Scoped;
}

pub struct NoParams;

impl Ready for NoParams {
    type Out = True;
}

impl Finish for NoParams {
    type Params = ();

    fn finish(self) {}
}
