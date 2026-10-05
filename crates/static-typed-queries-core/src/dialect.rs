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

/// A SQL dialect: how identifiers are quoted and parameters written, and what its statements
/// can do.
pub trait Dialect: 'static {
    /// The dialect's name, as messages give it, such as `"PostgreSQL"`.
    const NAME: &'static str;
    #[doc(hidden)]
    const PARAMS: params::ParamStyle;
    #[doc(hidden)]
    const QUOTE: quote::Quote;
    #[doc(hidden)]
    const DML_IN_CTE: bool;

    #[doc(hidden)]
    #[cfg(feature = "parse-check")]
    fn grammar() -> Box<dyn sqlparser::dialect::Dialect> {
        Box::new(sqlparser::dialect::GenericDialect {})
    }
}
