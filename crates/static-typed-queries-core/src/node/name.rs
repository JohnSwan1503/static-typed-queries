/// An item's name: a table's name, or the name a query goes by as a CTE or subquery.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Name(&'static str);

impl Name {
    pub(crate) const EMPTY: Name = Name("");

    #[doc(hidden)]
    pub const fn new(name: &'static str) -> Name {
        Name(name)
    }

    /// The name.
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}
