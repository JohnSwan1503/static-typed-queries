Turns a struct into a SQL statement that is checked, composed and rendered
at compile time from a template. The struct's fields are the template's
parameters, and a value of the struct holds their values.

```
use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(
    Postgres,
    sql = "
    SELECT id, email FROM {Users}
    WHERE org_id = {org_id} AND email LIKE {pattern}"
)]
pub struct UsersByOrg {
    pub org_id: i64,
    pub pattern: String,
}

assert_eq!(
    UsersByOrg::SQL,
    r#"SELECT id, email FROM "users" WHERE org_id = $1 AND email LIKE $2"#
);

let query = UsersByOrg {
    org_id: 7,
    pattern: "%@example.com".to_owned(),
};
assert_eq!(query.org_id, 7);
```

With a database feature enabled, `query.query()?` binds the values to the
SQL as a `sqlx` query.

# Arguments

The dialect type comes first, followed by any of these in any order:

- `sql = "..."` or `sql = NAME` (required): the template, described below,
  written inline or taken from a constant declared with [`sql`].
- `sql_file = "path"`: reads the template from a file instead, relative to
  the directory of the crate's `Cargo.toml`. The query is rebuilt when the
  file changes.
- `name = "..."`: the name this query goes by when another query embeds
  it, as a CTE name or a subquery alias. Defaults to the struct name in
  snake case.
- `display = sql` or `display = name`: implements `Display`, writing the
  rendered SQL or the name.
- `debug = sql` or `debug = tree`: implements `Debug`, writing the rendered
  SQL as a quoted string or the query's node tree.
- `parse_check = false`: leaves the query out of the `parse-check`
  feature's tests. Use it on queries defined inside functions, where rustc
  can't run the generated test.
- `grammar = postgres | mysql | sqlite | generic`: the SQL grammar the
  template is checked against. It's needed when the dialect isn't written
  as `Postgres`, `MySql`, `Sqlite` or `T::Dialect`, such as through a type
  alias, since the macro only sees the name.
- `row = Type`: the type each returned row is read as, through
  `sqlx::FromRow`. `query()` then returns a `sqlx` `QueryAs` for it, and
  `run()` a `Vec` of it. Only queries and statements with `RETURNING` take
  it. Every statement also has `query_as::<T>()`, or `run_as(conn)`, for
  reading rows as some other type.

The template is parsed with the grammar of the dialect, which is picked by
the last segment of the dialect path: `Postgres`, `MySql` or `Sqlite`. Any
other path, such as `T::Dialect`, is checked against a generic grammar.

# Template

The template is one SQL statement without a trailing `;`. Placeholders in
braces are replaced when the statement is rendered:

- `{field}` is the struct's field of that name. A plain field is a bind
  parameter: its value is bound with the field's type, however many times
  the template uses it. A field marked `#[cte]`, `#[cte(recursive)]` or
  `#[subquery]` holds another query, which is embedded that way; see
  below. Every field must be used, except fields of type `PhantomData`.
- `{Type}` embeds a [`table`] or a query that holds no values. `{Type as
  cte}`, `{Type as cte(recursive)}` and `{Type as subquery}` choose how a
  query is embedded at this one place; the default is `subquery`.

Write `{{` and `}}` for literal braces. Braces inside quoted strings,
quoted identifiers, dollar-quoted strings and comments are left alone.
Whitespace is collapsed and `--` comments are dropped.

## Embedding other items

A reference in a `FROM` or `JOIN`, or as the target of an `INSERT`,
`UPDATE` or `DELETE`, is embedded the way the field or the reference asks.
A table is written as its quoted name, whatever is asked. A CTE is hoisted
into the statement's `WITH` clause and referenced by name. A subquery is
inlined in parentheses, with the item's name as its alias unless the
template gives one. A reference anywhere else is always inlined as a
parenthesized subquery.

A query that holds values is embedded through a field, which holds those
values. Each field is one instance: using it twice refers to the same CTE
or the same bound values, and two fields of one type are two instances,
rendered as two CTEs with the second one renamed (`x`, `x_2`).

```
# use static_typed_queries::prelude::*;
# #[table(Postgres, name = "users")]
# pub struct Users;
#[query(Postgres, sql = "SELECT id FROM {Users} WHERE org_id = {org_id}")]
pub struct OrgUsers {
    pub org_id: i64,
}

#[query(Postgres, sql = "SELECT count(*) FROM {users}")]
pub struct OrgSize {
    #[cte]
    pub users: OrgUsers,
}

assert_eq!(
    OrgSize::SQL,
    r#"WITH "org_users" AS (SELECT id FROM "users" WHERE org_id = $1) SELECT count(*) FROM "org_users""#
);

let size = OrgSize {
    users: OrgUsers { org_id: 7 },
};
assert_eq!(size.users.org_id, 7);
```

Some combinations are only rejected once the whole statement is rendered,
such as a data-modifying CTE on a dialect that doesn't support one. Those
show up as compile errors on the outermost query.

# Generated items

For `struct Name`, the macro implements `Sql` and generates:

- A `Statement` impl and an inherent `Name::SQL` constant holding the
  rendered SQL, for non-generic queries.
- With a database feature enabled, `query()` and `query_as::<T>()` on the
  struct, which bind its values and return a `sqlx` query. A query that
  uses a table with hooks has `with()`, `run()` and `run_as()` instead; see
  [`table`].
- With the `parse-check` feature, a `#[cfg(test)]` test named
  `name_sql_parses` that parses the rendered SQL, everything embedded,
  with the dialect's grammar. Enable it under `[dev-dependencies]`.

The struct's own docs get the template appended. The fields and their
docs are left as written; the `#[cte]` and `#[subquery]` markers are
taken off.

# Generic queries

The struct can take type parameters. A type parameter can be the type of
a field, which then holds the values of whatever it is instantiated with,
or be embedded by type with `{T}` when it holds no values. A generic query
isn't a `Statement` on its own; it is rendered as part of the queries that
embed it, or named as a [`statement`]. It also can't take `display`,
`debug` or `row`.

```
# use static_typed_queries::prelude::*;
# #[table(Postgres, name = "users")]
# pub struct Users;
use std::marker::PhantomData;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[query(Postgres, sql = "SELECT {CountOf<Users>} AS users")]
pub struct Totals;

assert_eq!(
    Totals::SQL,
    r#"SELECT (SELECT count(*) FROM "users") AS users"#
);
```
