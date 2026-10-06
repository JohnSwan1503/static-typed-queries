Composable SQL statements, checked and rendered at compile time.

Tables, queries, statements and transactions are structs with an attribute.
A query's SQL is a template on its struct: the fields are the template's
parameters, and other items are embedded by name. The macros check each
template with its dialect's grammar and render the composed statement into
a constant, so at run time a statement only binds its values. Statements run
through [sqlx](https://docs.rs/sqlx).

# Tables and queries

[`table`] declares a table and [`query`] a statement. In a template,
`{field}` binds the struct's field of that name and `{Type}` embeds a table
or a query that holds no values:

```
use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id, email FROM {Users} WHERE org_id = {org_id}")]
pub struct OrgUsers {
    pub org_id: i64,
}

assert_eq!(OrgUsers::SQL, r#"SELECT id, email FROM "users" WHERE org_id = $1"#);
```

A value of the struct holds the values to bind. Write it as a struct
literal, or set one field at a time with the builder from [`Build`], which
only has `build()` once every field is set:

```
# use static_typed_queries::prelude::*;
# #[table(Postgres, name = "users")]
# pub struct Users;
# #[query(Postgres, sql = "SELECT id, email FROM {Users} WHERE org_id = {org_id}")]
# pub struct OrgUsers {
#     pub org_id: i64,
# }
let literal = OrgUsers { org_id: 7 };
let built = OrgUsers::builder().org_id(7).build();
assert_eq!(literal.org_id, built.org_id);
```

# Embedding queries

A field marked `#[cte]`, `#[cte(recursive)]` or `#[subquery]` holds another
query, values and all, and `{field}` embeds it that way. The builder moves
into the field's own builder for one setter call:

```
# use static_typed_queries::prelude::*;
# #[table(Postgres, name = "users")]
# pub struct Users;
# #[query(Postgres, sql = "SELECT id, email FROM {Users} WHERE org_id = {org_id}")]
# pub struct OrgUsers {
#     pub org_id: i64,
# }
#[query(Postgres, sql = "SELECT count(*) FROM {users}")]
pub struct OrgSize {
    #[cte]
    pub users: OrgUsers,
}

assert_eq!(
    OrgSize::SQL,
    r#"WITH "org_users" AS (SELECT id, email FROM "users" WHERE org_id = $1) SELECT count(*) FROM "org_users""#
);
let size = OrgSize::builder().users().org_id(7).build();
assert_eq!(size.users.org_id, 7);
```

A query can be generic over what it embeds, as `CountOf<T>`. It renders as
part of the statements that use it, and [`statement`] makes one of its
instantiations a statement of its own. [`query`] describes templates,
placements and generics in full.

# Running

With a dialect's feature enabled, `query()` binds a statement's values and
returns a `sqlx` query, and `query_as::<T>()` one that reads rows as `T`. A
query declared with `row = T` reads its rows as `T` from `query()`:

```no_run
# use static_typed_queries::prelude::*;
# #[table(Postgres, name = "users")]
# pub struct Users;
#[derive(sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub email: String,
}

#[query(Postgres, row = User, sql = "SELECT id, email FROM {Users} WHERE org_id = {org_id}")]
pub struct OrgUsers {
    pub org_id: i64,
}

async fn org_users(conn: &mut sqlx::PgConnection) -> sqlx::Result<Vec<User>> {
    OrgUsers { org_id: 7 }.query()?.fetch_all(conn).await
}
```

# Hooks

A table can name statements to run `before` and `after` every statement
that uses it, such as one that sets the tenant for row-level security. A
statement that reaches hooks runs with `run(conn)`, which runs the hooks
and the statement in one transaction, and has no `query()`, since a lone
query would skip them. Each hook runs once per statement. A hook with
fields takes its values through `with`:

```no_run
# use static_typed_queries::prelude::*;
#[query(Postgres, sql = "SELECT set_config('app.tenant', {tenant}, true)")]
pub struct SetTenant {
    pub tenant: String,
}

#[table(Postgres, name = "orders", before(SetTenant))]
pub struct Orders;

#[query(Postgres, sql = "UPDATE {Orders} SET status = 'closed' WHERE status = {status}")]
pub struct CloseAll {
    pub status: String,
}

async fn close_stale(conn: &mut sqlx::PgConnection) -> sqlx::Result<u64> {
    CloseAll { status: "stale".to_owned() }
        .with(SetTenant { tenant: "acme".to_owned() })
        .run(conn)
        .await
}
```

`run` only compiles once every hook the statement reaches has its values,
and the error names a hook that's missing one. Values for a hook the
statement never runs, or for one hook twice, don't compile either. `run`
returns a [`Running`] future, which is `Send`, so the values must be `Send`
and `Sync`. The "Hooks" section of [`table`] has the details.

# Transactions

[`transaction`] runs statements, its steps, in order in one transaction
and returns each step's output. A `savepoint(...)` group can fail on its
own without rolling back the rest, `as one` and `as optional` read a single
row, and `as cte` attaches a step to the next one's `WITH` clause. The hooks
of every step run once, around the whole transaction.

# Dialects

The first argument of each attribute is the dialect: [`Postgres`],
[`MySql`] or [`Sqlite`], each behind the feature of the same name. It sets
how identifiers are quoted and parameters numbered, and which grammar the
template is checked with. A generic query uses `T::Dialect` and is checked
with a generic grammar.

An item only embeds into a statement of its own dialect, which
[`EmbedsIn`] checks, and a query that modifies data is only placed as a CTE
in a dialect that runs one, such as PostgreSQL.

# Sharing templates

[`sql`] declares a template as a constant, which several queries take as
`sql = NAME`, for instance one query per dialect. `sql_file = "path"` reads
a template from a file instead.

# Renaming the dependency

The generated code names this crate `::static_typed_queries`. When the
dependency is renamed in `Cargo.toml`, or reached through another crate's
re-export, each attribute takes the path to it as `crate = path`:

```
mod db {
    pub use static_typed_queries as stq;
}

use db::stq::prelude::*;

#[query(Postgres, crate = db::stq, sql = "SELECT 1")]
pub struct One;

assert_eq!(One::SQL, "SELECT 1");
```

# Feature flags

- `postgres`, `mysql`, `sqlite`: the dialect, and running its statements
  through `sqlx`.
- `all`: all three dialects.
- `parse-check`: a `#[test]` per statement that parses its rendered SQL
  with the dialect's grammar. Enable it under `[dev-dependencies]`.

[`Postgres`]: dialect::Postgres
[`MySql`]: dialect::MySql
[`Sqlite`]: dialect::Sqlite
[`EmbedsIn`]: dialect::EmbedsIn
