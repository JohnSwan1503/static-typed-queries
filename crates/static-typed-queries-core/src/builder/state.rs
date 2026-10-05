use crate::sql::Sql;

/// A builder's field that isn't set yet.
pub struct Missing;

/// A builder's field that is set.
pub struct Filled;

/// A builder's item field whose own builder still has fields to set.
pub struct Open<B>(pub B);

/// A builder's item field that is complete, or the builder of an item without fields.
pub struct Built<T>(pub T);

pub struct True;

pub struct False;

pub trait Ready {
    type Out;
}

impl Ready for Missing {
    type Out = False;
}

impl Ready for Filled {
    type Out = True;
}

impl<B> Ready for Open<B> {
    type Out = False;
}

impl<T> Ready for Built<T> {
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

#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't a value or a complete builder",
    label = "expected a value or a complete builder",
    note = "a builder is complete once every field is set"
)]
/// A value of an item, or its builder with every field set. `with` takes either.
pub trait Finish {
    /// The item.
    type Output;

    /// The value, built first if this is a builder.
    fn finish(self) -> Self::Output;
}

// A value is finished already. Without `do_not_recommend`, an incomplete builder would be
// reported as not implementing `Sql`.
#[diagnostic::do_not_recommend]
impl<T: Sql> Finish for T {
    type Output = T;

    fn finish(self) -> T {
        self
    }
}

// An item that holds no values is built from the start.
impl<T> Finish for Built<T> {
    type Output = T;

    fn finish(self) -> T {
        self.0
    }
}

pub trait Settle<B> {
    type Out;

    fn settle(builder: B) -> Self::Out;
}

impl<B: Finish> Settle<B> for True {
    type Out = Built<B::Output>;

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
