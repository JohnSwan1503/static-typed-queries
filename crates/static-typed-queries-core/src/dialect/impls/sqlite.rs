use crate::dialect::Dialect;
use crate::dialect::name::Name;
use crate::dialect::params::ParamStyle;
use crate::dialect::quote::Quote;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sqlite;

impl Dialect for Sqlite {
    const NAME: Name = Name::SQLITE;
    const PARAMS: ParamStyle = ParamStyle::dollar_sign(true);
    const DML_IN_CTE: bool = false;
    const QUOTE: Quote = Quote::new(b'"', b'"');
}

#[cfg(any(feature = "sqlite", feature = "sqlite-unbundled"))]
impl crate::dialect::driver::Driver for Sqlite {
    type Database = sqlx::Sqlite;
}
