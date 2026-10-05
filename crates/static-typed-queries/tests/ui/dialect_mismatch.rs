use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE tags @> ARRAY[{tag}]")]
pub struct Tagged {
    pub tag: String,
}

#[query(Sqlite, sql = "SELECT count(*) FROM {tagged}")]
pub struct TaggedCount {
    #[subquery]
    pub tagged: Tagged,
}

fn main() {}
