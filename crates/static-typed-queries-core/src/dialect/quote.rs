#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Quote([u8; 2]);

impl Quote {
    pub const fn new(l: u8, r: u8) -> Quote {
        Quote([l, r])
    }

    pub const fn as_array(self) -> [u8; 2] {
        self.0
    }

    pub const fn open(&self) -> u8 {
        self.0[0]
    }

    pub const fn close(&self) -> u8 {
        self.0[1]
    }
}
