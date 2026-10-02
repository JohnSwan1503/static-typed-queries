#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Inject {
    Ident,
    Cte { recursive: bool },
    Subquery,
}

impl Inject {
    pub const fn cte(recursive: bool) -> Inject {
        Inject::Cte { recursive }
    }
}
