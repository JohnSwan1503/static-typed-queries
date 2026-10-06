//! The same kinds of items where the prelude's names, `core`, `std` and `sqlx` mean something
//! else, which the generated code must not depend on.
#![allow(missing_docs, missing_debug_implementations, unused_macros)]

use stq::prelude::*;

pub struct Option;
pub struct Some;
pub struct None;
pub struct Result;
pub struct Ok;
pub struct Err;
pub struct Vec;
pub struct String;
pub struct Box;
pub struct PhantomData;
pub trait Clone {}
pub trait Copy {}
pub trait Default {}
pub trait Send {}
pub trait Sync {}
pub trait Sized {}
pub trait From {}
pub trait Into {}
pub trait Iterator {}
pub trait IntoIterator {}
pub trait Drop {}
pub trait Fn {}
pub trait FnMut {}
pub trait FnOnce {}
pub trait AsRef {}
pub trait ToOwned {}
pub trait ToString {}
pub trait PartialEq {}
pub trait Eq {}
pub trait Debug {}
pub trait Display {}
pub mod core {}
pub mod std {}
pub mod alloc {}
pub mod sqlx {}

macro_rules! panic {
    ($($t:tt)*) => {
        compile_error!("the prelude's `panic!`")
    };
}
macro_rules! assert {
    ($($t:tt)*) => {
        compile_error!("the prelude's `assert!`")
    };
}
macro_rules! concat {
    ($($t:tt)*) => {
        compile_error!("the prelude's `concat!`")
    };
}
macro_rules! stringify {
    ($($t:tt)*) => {
        compile_error!("the prelude's `stringify!`")
    };
}
macro_rules! format {
    ($($t:tt)*) => {
        compile_error!("the prelude's `format!`")
    };
}
macro_rules! format_args {
    ($($t:tt)*) => {
        compile_error!("the prelude's `format_args!`")
    };
}
macro_rules! write {
    ($($t:tt)*) => {
        compile_error!("the prelude's `write!`")
    };
}
macro_rules! vec {
    ($($t:tt)*) => {
        compile_error!("the prelude's `vec!`")
    };
}
macro_rules! matches {
    ($($t:tt)*) => {
        compile_error!("the prelude's `matches!`")
    };
}
macro_rules! unreachable {
    ($($t:tt)*) => {
        compile_error!("the prelude's `unreachable!`")
    };
}
macro_rules! include_str {
    ($($t:tt)*) => {
        compile_error!("the prelude's `include_str!`")
    };
}

#[query(Sqlite, crate = stq, sql = "INSERT INTO notes (note) VALUES ({note})")]
pub struct Note {
    pub note: ::std::string::String,
}

#[table(Sqlite, crate = stq, name = "items", before(Note), display = name, debug = tree)]
pub struct Items;

#[query(
    Sqlite,
    crate = stq,
    row = super::Item,
    display = sql,
    debug = sql,
    sql = "SELECT id, price FROM {Items} WHERE price >= {price}"
)]
pub struct Pricey {
    pub price: i64,
}

#[sql]
pub const BELOW: &str = "SELECT id FROM {Items} WHERE price < {price}";

#[query(Sqlite, crate = stq, sql = BELOW)]
pub struct Cheap {
    pub price: i64,
}

#[query(Sqlite, crate = stq, sql = "SELECT count(*) FROM {pricey} WHERE id IN {cheap}")]
pub struct Overlap {
    #[cte]
    pub pricey: Pricey,
    #[subquery]
    pub cheap: Cheap,
}

#[query(T::Dialect, crate = stq, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(::core::marker::PhantomData<T>);

#[statement(CountOf<Items>, crate = stq, row = (i64,), display = sql)]
pub struct ItemCount;

#[query(Sqlite, crate = stq, sql = "UPDATE {Items} SET price = price + {by}")]
pub struct Raise {
    pub by: i64,
}

#[transaction(Sqlite, crate = stq, steps(raise, savepoint(ItemCount as one), cheap as cte, pricey))]
pub struct Reprice {
    pub raise: Raise,
    pub cheap: Cheap,
    pub pricey: Pricey,
}
