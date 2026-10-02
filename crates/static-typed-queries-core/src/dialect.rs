pub mod name;
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
#[cfg(any(test, any(feature = "sqlite", feature = "sqlite-unbundled")))]
pub use impls::sqlite;

pub trait Dialect: 'static {
    const NAME: name::Name;
    const PARAMS: params::ParamStyle;
    const QUOTE: quote::Quote;
    const DML_IN_CTE: bool;
}
