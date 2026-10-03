pub mod builder;
pub mod dialect;
pub mod embed;
pub mod node;
pub mod part;
pub mod render;
pub mod sql;
pub mod statement;

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
