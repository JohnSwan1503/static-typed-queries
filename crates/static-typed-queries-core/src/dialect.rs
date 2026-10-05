#[cfg(feature = "sqlx")]
pub mod driver;
pub mod params;
pub mod quote;

mod impls;

#[doc(inline)]
#[cfg(any(test, feature = "mysql"))]
pub use impls::mysql;
#[doc(inline)]
#[cfg(any(test, feature = "postgres"))]
pub use impls::postgres;
#[doc(inline)]
#[cfg(any(test, feature = "sqlite"))]
pub use impls::sqlite;

pub trait Dialect: 'static {
    const NAME: &'static str;
    const PARAMS: params::ParamStyle;
    const QUOTE: quote::Quote;
    const DML_IN_CTE: bool;

    #[cfg(feature = "parse-check")]
    fn grammar() -> Box<dyn sqlparser::dialect::Dialect> {
        Box::new(sqlparser::dialect::GenericDialect {})
    }
}
