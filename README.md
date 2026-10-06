# static-typed-queries

Composable SQL for Rust, checked and rendered at compile time.

A query is a struct with a SQL template. The struct's fields are the
template's parameters, and other tables and queries are embedded by name,
as subqueries or CTEs. The macros check each template against its
database's SQL grammar and render the composed statement into a constant,
so at run time a query only binds its values. Statements run through
[sqlx](https://github.com/launchbadge/sqlx).

## Quick start

```toml
[dependencies]
static-typed-queries = { version = "0.1", features = ["sqlite"] }
sqlx = { version = "0.9", features = ["runtime-tokio", "sqlite"] }
tokio = { version = "1", features = ["macros", "rt"] }
```

```rust
use sqlx::{Connection, SqliteConnection};
use static_typed_queries::prelude::*;

#[table(Sqlite, name = "users")]
pub struct Users;

#[query(Sqlite, sql = "SELECT id, email FROM {Users} WHERE org_id = {org_id}")]
pub struct OrgUsers {
    pub org_id: i64,
}

#[query(Sqlite, sql = "SELECT count(*) FROM {users} WHERE email LIKE {pattern}")]
pub struct CountMatching {
    pub pattern: String,
    #[cte]
    pub users: OrgUsers,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), sqlx::Error> {
    assert_eq!(
        CountMatching::SQL,
        r#"WITH "org_users" AS (SELECT id, email FROM "users" WHERE org_id = $1) SELECT count(*) FROM "org_users" WHERE email LIKE $2"#
    );

    let mut conn = SqliteConnection::connect("sqlite::memory:").await?;
    sqlx::raw_sql(
        "CREATE TABLE users (id INTEGER PRIMARY KEY, org_id INTEGER, email TEXT);
         INSERT INTO users (org_id, email)
         VALUES (7, 'ada@example.com'), (7, 'bob@example.org'), (8, 'cy@example.com');",
    )
    .execute(&mut conn)
    .await?;

    let (count,): (i64,) = CountMatching::builder()
        .pattern("%@example.com".to_owned())
        .users()
        .org_id(7)
        .query_as()?
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(count, 1);
    Ok(())
}
```

## What it checks

At compile time:

- each template parses with its dialect's grammar, and every placeholder
  names a field of the struct or a type;
- every field is used, and the builder sets each one exactly once before
  it builds;
- embedded items are written for the same dialect, and a CTE that modifies
  data only goes where the dialect runs one;
- a statement whose tables have `before` or `after` hooks only runs with
  them, in one transaction, with values for each hook it reaches and none
  for a hook it doesn't.

## Features

| Feature | Enables |
| --- | --- |
| `postgres`, `mysql`, `sqlite` | the dialect, and running its statements through sqlx |
| `all` | all three dialects |
| `parse-check` | a `#[test]` per statement that parses its rendered SQL with the dialect's grammar; enable it under `[dev-dependencies]` |

## Compatibility

- Rust 1.94 or newer, the oldest release sqlx 0.9 supports. Crates on
  edition 2018 or later can use the attributes.
- Statements run through sqlx 0.9, so depend on the same sqlx version. The
  runtime, TLS and type features of sqlx are yours to choose.
- A renamed dependency, or one reached through another crate's re-export,
  works when each attribute is given its path, as in `crate = stq`.

## Versioning

The three crates are released together at the same version, and only
`static-typed-queries` is meant to be depended on. Until 1.0, a release that
breaks the API raises the minor version. Items hidden from the docs, such as
`__private`, exist for the generated code and can change in any release.
[CHANGELOG.md](CHANGELOG.md) lists what changed.

## Documentation

[docs.rs](https://docs.rs/static-typed-queries) has the crate's docs: a
tour of tables, queries, statements, transactions, hooks and dialects, and
a reference for each attribute. `cargo doc --open -p static-typed-queries`
builds them locally.

## License

MIT
