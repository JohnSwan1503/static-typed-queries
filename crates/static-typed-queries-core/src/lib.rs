#[cfg(feature = "parse-check")]
pub mod check;
#[doc(hidden)]
pub mod codegen;
pub mod dialect;
pub mod embed;
pub mod hooks;
pub mod node;
pub mod part;
pub mod render;
pub mod sql;
pub mod statement;
pub mod transaction;
pub mod values;
#[cfg(feature = "sqlx")]
pub mod with;

#[doc(hidden)]
pub mod __private {
    #[cfg(feature = "sqlx")]
    pub use sqlx;
}

#[cfg(feature = "sqlx")]
#[doc(hidden)]
#[macro_export]
macro_rules! __if_sqlx {
    ($($tokens:tt)*) => {
        $($tokens)*
    };
}

#[cfg(not(feature = "sqlx"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __if_sqlx {
    ($($tokens:tt)*) => {};
}

#[cfg(feature = "parse-check")]
#[doc(hidden)]
#[macro_export]
macro_rules! __if_parse_check {
    ($($tokens:tt)*) => {
        $($tokens)*
    };
}

#[cfg(not(feature = "parse-check"))]
#[doc(hidden)]
#[macro_export]
macro_rules! __if_parse_check {
    ($($tokens:tt)*) => {};
}
