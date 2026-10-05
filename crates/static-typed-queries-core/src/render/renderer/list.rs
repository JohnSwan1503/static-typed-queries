use crate::render::error::fail;

pub(super) struct List<T: Copy, const N: usize> {
    items: [Option<T>; N],
    len: usize,
}

impl<T: Copy, const N: usize> List<T, N> {
    pub(super) const fn new() -> Self {
        List {
            items: [None; N],
            len: 0,
        }
    }

    pub(super) const fn len(&self) -> usize {
        self.len
    }

    pub(super) const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub(super) const fn get(&self, i: usize) -> Option<T> {
        if i < self.len { self.items[i] } else { None }
    }

    pub(super) const fn push(&mut self, item: T, too_many: &'static str) {
        if self.len == N {
            fail(&[too_many]);
        }
        self.items[self.len] = Some(item);
        self.len += 1;
    }

    pub(super) const fn clear(&mut self) {
        self.len = 0;
    }
}
