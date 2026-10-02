pub mod ident;
pub mod lit;
pub mod param;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Ident(ident::Ident),
    Lit(lit::Lit),
    Param(param::Param),
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Parts(pub &'static [Part]);
