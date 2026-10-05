use core::marker::PhantomData;

use crate::render::Render;
use crate::sql::Sql;
use crate::values::Valueless;

/// A statement without hooks, which binds as one query through `query()`.
pub struct NoHooks;

/// A statement with hooks, which runs through `run()` with them in one transaction.
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

#[diagnostic::do_not_recommend]
impl Unhooked for NoHooks {}

#[diagnostic::on_unimplemented(
    message = "this statement has no hooks, so it runs as a single query",
    label = "has no hooks",
    note = "bind it with `query` instead of `run`"
)]
pub trait HasHooks {}

#[diagnostic::do_not_recommend]
impl HasHooks for WithHooks {}

// `I` is inferred at each call, so a method on a concrete statement can carry the bound to its
// call site instead of failing where the method is defined.
pub trait Single<I = ()> {}

impl<S: Render> Single for S where S::Hooks: Unhooked {}

pub trait Hooked<I = ()> {}

impl<S: Render> Hooked for S where S::Hooks: HasHooks {}

/// The values of the hook `H`, given to `with`.
pub struct HookValues<H>(pub H);

pub struct Here;

pub struct There<I>(PhantomData<I>);

pub struct Free;

pub struct Used<H>(PhantomData<H>);

// `Out` is the list with `H`'s values marked used, so `run` can refuse values nothing uses.
#[diagnostic::on_unimplemented(
    message = "no values for the hook `{H}`",
    label = "`{H}` has values",
    note = "give them with `.with({H} {{ .. }})`"
)]
pub trait Provides<H, I> {
    type Out;
}

#[diagnostic::do_not_recommend]
impl<H: Sql, Rest> Provides<H, Here> for (HookValues<H>, Rest) {
    type Out = (Used<H>, Rest);
}

#[diagnostic::do_not_recommend]
impl<H: Sql, Rest> Provides<H, Here> for (Used<H>, Rest) {
    type Out = (Used<H>, Rest);
}

#[diagnostic::do_not_recommend]
impl<H, X, Rest: Provides<H, I>, I> Provides<H, There<I>> for (X, Rest) {
    type Out = (X, Rest::Out);
}

#[diagnostic::do_not_recommend]
impl<H: Valueless> Provides<H, Free> for () {
    type Out = ();
}

// `Out` is `V` with the values of every hook the item reaches marked used.
pub trait HookNeeds<V, I> {
    type Out;
}

pub trait Spent {}

#[diagnostic::do_not_recommend]
impl Spent for () {}

#[diagnostic::do_not_recommend]
impl<H, Rest: Spent> Spent for (Used<H>, Rest) {}

impl<H: Unreached, Rest: Spent> Spent for (HookValues<H>, Rest) {}

// Never implemented: values left unused name their hook through this.
#[diagnostic::on_unimplemented(
    message = "this statement never runs the hook `{Self}`",
    label = "`{Self}` never runs",
    note = "remove `.with({Self} {{ .. }})`"
)]
pub trait Unreached {}
