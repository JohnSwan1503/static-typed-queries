use super::Part;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Param(u16);

impl Param {
    pub const fn part(param: u16) -> Part {
        Part::Param(Param(param))
    }

    pub const fn inner(self) -> u16 {
        self.0
    }
}
