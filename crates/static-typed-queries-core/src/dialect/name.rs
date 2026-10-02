#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Name(&'static str);

impl Name {
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}
