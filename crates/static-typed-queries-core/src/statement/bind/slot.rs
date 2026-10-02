use crate::part::param::Param;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot(u16);

impl Slot {
    pub(crate) const EMPTY: Slot = Slot(0);

    pub const fn new(slot: u16) -> Slot {
        Slot(slot)
    }

    pub const fn from_param(param: Param) -> Slot {
        Slot(param.inner())
    }

    pub const fn inner(self) -> u16 {
        self.0
    }

    pub const fn as_index(self) -> usize {
        self.0 as usize
    }
}
