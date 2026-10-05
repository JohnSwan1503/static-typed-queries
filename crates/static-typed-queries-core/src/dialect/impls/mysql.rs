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

    #[cfg(feature = "parse-check")]
    fn grammar() -> Box<dyn sqlparser::dialect::Dialect> {
        Box::new(sqlparser::dialect::MySqlDialect {})
    }
}

#[cfg(feature = "mysql")]
impl crate::dialect::driver::Driver for MySql {
    type Database = sqlx::MySql;

    fn rows_affected(result: &<sqlx::MySql as sqlx::Database>::QueryResult) -> u64 {
        result.rows_affected()
    }
}
