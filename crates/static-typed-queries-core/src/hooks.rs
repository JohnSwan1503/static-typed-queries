use core::marker::PhantomData;

use crate::render::Render;
use crate::sql::Sql;
use crate::values::{Valueless, Values};

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

#[diagnostic::on_unimplemented(
    message = "this statement runs `before` or `after` hooks, so it can't run as a single query",
    label = "has hooks",
    note = "run it with `run`, which wraps the hooks and the statement in a transaction"
)]
pub trait Unhooked {}

impl Unhooked for NoHooks {}

#[diagnostic::on_unimplemented(
    message = "this statement has no hooks, so it runs as a single query",
    label = "has no hooks",
    note = "bind it with `query` instead of `run`"
)]
pub trait HasHooks {}

impl HasHooks for WithHooks {}

// `I` is inferred at each call, so a method on a concrete statement can carry the bound to its
// call site instead of failing where the method is defined.
pub trait Single<I = ()> {}

impl<S: Render> Single for S where S::Hooks: Unhooked {}

pub trait Hooked<I = ()> {}

impl<S: Render> Hooked for S where S::Hooks: HasHooks {}

pub struct HookValues<H>(pub H);

pub struct Here;

pub struct There<I>(PhantomData<I>);

pub struct Free;

#[diagnostic::on_unimplemented(
    message = "no values for the hook `{H}`",
    label = "`{H}` has values",
    note = "give them with `.with({H} {{ .. }})`"
)]
pub trait Provides<H, I> {}

#[diagnostic::do_not_recommend]
impl<H: Sql, Rest> Provides<H, Here> for (HookValues<H>, Rest) {}

#[diagnostic::do_not_recommend]
impl<H, X, Rest: Provides<H, I>, I> Provides<H, There<I>> for (X, Rest) {}

#[diagnostic::do_not_recommend]
impl<H: Valueless> Provides<H, Free> for () {}

pub trait HookNeeds<V, I> {}

pub fn with<H: Values, V>(values: H, rest: V) -> (HookValues<H>, V) {
    (HookValues(values), rest)
}
