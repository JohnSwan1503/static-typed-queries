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
