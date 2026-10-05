use crate::part::param::Param;

/// A field's index among the fields of its item that hold values.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot(u16);

impl Slot {
    pub(crate) const EMPTY: Slot = Slot(0);

    #[doc(hidden)]
    pub const fn new(slot: u16) -> Slot {
        Slot(slot)
    }

    #[doc(hidden)]
    pub const fn from_param(param: Param) -> Slot {
        Slot(param.inner())
    }

    /// The index.
    pub const fn inner(self) -> u16 {
        self.0
    }
}
