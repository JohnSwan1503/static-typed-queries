use super::Node;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Before(pub &'static [&'static Node]);

impl Before {
    pub const fn new(before: &'static [&'static Node]) -> Before {
        Before(before)
    }

    pub const fn into_iter(self) -> BeforeIter {
        BeforeIter(self, 0, self.0.len())
    }
}

pub struct BeforeIter(Before, usize, usize);

impl BeforeIter {
    pub const fn next(&mut self) -> Option<&'static Node> {
        if self.1 < self.2 {
            self.1 += 1;
            Some(self.0.0[self.1 - 1])
        } else {
            None
        }
    }
}
