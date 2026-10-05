use static_typed_queries::prelude::*;

#[sql]
pub const JOINED: &str = concat!("SELECT ", "1");

#[sql]
pub const COUNT: i64 = 1;

fn main() {}
