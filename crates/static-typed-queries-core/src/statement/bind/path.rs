const MAX_DEPTH: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Path {
    steps: [u16; MAX_DEPTH],
    len: u8,
}

impl Path {
    pub const ROOT: Path = Path {
        steps: [0; MAX_DEPTH],
        len: 0,
    };

    pub const fn new(steps: &[u16]) -> Path {
        assert!(
            steps.len() <= MAX_DEPTH,
            "paths can't be more than 16 steps deep"
        );
        let mut path = Path::ROOT;
        while (path.len as usize) < steps.len() {
            path.steps[path.len as usize] = steps[path.len as usize];
            path.len += 1;
        }
        path
    }

    pub const fn steps(&self) -> &[u16] {
        self.steps.split_at(self.len as usize).0
    }

    pub(crate) const fn child(self, step: u16) -> Option<Path> {
        if self.len as usize == MAX_DEPTH {
            return None;
        }
        let mut path = self;
        path.steps[path.len as usize] = step;
        path.len += 1;
        Some(path)
    }

    pub(crate) const fn same(&self, other: &Path) -> bool {
        if self.len != other.len {
            return false;
        }
        let mut i = 0;
        while i < self.len as usize {
            if self.steps[i] != other.steps[i] {
                return false;
            }
            i += 1;
        }
        true
    }
}

impl core::fmt::Debug for Path {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_list().entries(self.steps()).finish()
    }
}
