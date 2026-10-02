use crate::dialect::Dialect;
use crate::dialect::name::Name;
use crate::dialect::params::ParamStyle;
use crate::dialect::quote::Quote;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MySql;

impl Dialect for MySql {
    const NAME: Name = Name::MYSQL;
    const PARAMS: ParamStyle = ParamStyle::question_mark(false);
    const DML_IN_CTE: bool = false;
    const QUOTE: Quote = Quote::new(b'`', b'`');
}
