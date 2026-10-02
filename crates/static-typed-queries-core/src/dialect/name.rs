#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Name(&'static str);

impl Name {
    #[cfg(any(test, feature = "postgres"))]
    pub const POSTGRES: Name = Name("PostgreSQL");
    #[cfg(any(test, feature = "sqlite", feature = "sqlite-unbundled"))]
    pub const SQLITE: Name = Name("Sqlite");
    #[cfg(any(test, feature = "mysql"))]
    pub const MYSQL: Name = Name("MySql");

    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}
