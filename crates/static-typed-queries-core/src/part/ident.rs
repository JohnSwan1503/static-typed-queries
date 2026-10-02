use super::Part;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ident(pub &'static str);

impl Ident {
    pub const fn part(ident: &'static str) -> Part {
        Part::Ident(Ident(ident))
    }

    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}
