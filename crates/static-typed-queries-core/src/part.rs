pub mod expr;
pub mod from;
pub mod ident;
pub mod lit;
pub mod param;
pub mod target;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Expr(expr::Expr),
    From(from::From),
    Ident(ident::Ident),
    Lit(lit::Lit),
    Param(param::Param),
}

impl Part {
    pub const fn target(&self) -> Option<target::Target> {
        match self {
            Part::Expr(expr) => Some(expr.target()),
            Part::From(from) => Some(from.target()),
            Part::Ident(_) | Part::Lit(_) | Part::Param(_) => None,
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Parts(pub &'static [Part]);
