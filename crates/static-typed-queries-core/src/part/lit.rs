use super::Part;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Lit(&'static str);

impl Lit {
    pub const fn part(literal: &'static str) -> Part {
        Part::Lit(Lit(literal))
    }

    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}
