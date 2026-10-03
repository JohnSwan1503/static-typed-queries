use super::Part;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Param {
    slot: u16,
    field: &'static str,
    ty: &'static str,
}

impl Param {
    pub const fn part(slot: u16, field: &'static str, ty: &'static str) -> Part {
        Part::Param(Param { slot, field, ty })
    }

    pub const fn inner(self) -> u16 {
        self.slot
    }

    pub const fn field(self) -> &'static str {
        self.field
    }

    pub const fn ty(self) -> &'static str {
        self.ty
    }
}
