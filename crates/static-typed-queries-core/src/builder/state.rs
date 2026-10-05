pub struct Missing;

pub struct Filled;

pub struct Open<B>(pub B);

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

pub trait Finish {
    type Output;

    fn finish(self) -> Self::Output;
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
