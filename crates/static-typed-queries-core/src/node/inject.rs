#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Inject {
    Ident,
    Subquery,
}
