Turns a struct into a SQL statement that is checked, composed and rendered
at compile time from a template.

```
use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(
    Postgres,
    sql = "
    SELECT id, email FROM {Users}
    WHERE org_id = {org_id: i64} AND email LIKE {_: String}"
)]
pub struct UsersByOrg;

assert_eq!(
    UsersByOrg::SQL,
    r#"SELECT id, email FROM "users" WHERE org_id = $1 AND email LIKE $2"#
);

let params = UsersByOrg::builder()
    .org_id(7)
    .email("%@example.com".to_owned())
    .build();
assert_eq!(params.org_id, 7);
```

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
- `cte`, `cte(recursive)` or `subquery`: how this query is embedded when
  another query references it in a `FROM` or `JOIN`. Defaults to
  `subquery`.
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
  `sqlx::FromRow`. The complete builder's `query()` then returns a
  `sqlx` `QueryAs` for it, and `run()` a `Vec` of it. Only queries and
  statements with `RETURNING` take it. Every complete builder also has
  `query_as::<T>()`, or `run_as(conn)`, for reading rows as some other
  type. A struct with named fields is its own row type instead; see
  [Rows](#rows).
- `separate(Type, ...)`: gives each listed generic item its own values for
  the items in its type arguments, instead of sharing the query's. With
  `separate(CountOf<ActiveUsers>)`, `.active_users()` and
  `.count_of().active_users()` set two different `ActiveUsers`.

The template is parsed with the grammar of the dialect, which is picked by
the last segment of the dialect path: `Postgres`, `MySql` or `Sqlite`. Any
other path, such as `T::Dialect`, is checked against a generic grammar.

# Template

The template is one SQL statement without a trailing `;`. Placeholders in
braces are replaced when the statement is rendered:

- `{Type}` embeds another [`table`] or `query`. `{Type as cte}`,
  `{Type as cte(recursive)}` and `{Type as subquery}` override how that
  item is embedded at this one place.
- `{name: Type}` declares a bind parameter and uses it. It can be declared
  only once; write `{name}` to use it again.
- `{_: Type}` is a bind parameter named after its surroundings: the column
  it is compared with (`=`, `<`, `BETWEEN`, `IN (...)`, `LIKE`, `ANY`, and
  so on), the column it is inserted into or assigned to, its `AS` alias in
  a select list, or `limit` and `offset`. When nothing names it, it is
  called `bindN`, where `N` is its position among the parameters.

Write `{{` and `}}` for literal braces. Braces inside quoted strings,
quoted identifiers, dollar-quoted strings and comments are left alone.
Whitespace is collapsed and `--` comments are dropped.

## Embedding other items

A reference in a `FROM` or `JOIN`, or as the target of an `INSERT`,
`UPDATE` or `DELETE`, is embedded the way the item asks for. A table is
written as its quoted name. A CTE is hoisted into the statement's `WITH`
clause and referenced by name. A subquery is inlined in parentheses, with
the item's name as its alias unless the template gives one. A reference
anywhere else is always inlined as a parenthesized subquery.

```
# use static_typed_queries::prelude::*;
# #[table(Postgres, name = "users")]
# pub struct Users;
#[query(
    Postgres,
    cte,
    sql = "SELECT id FROM {Users} WHERE org_id = {org_id: i64}"
)]
pub struct OrgUsers;

#[query(Postgres, sql = "SELECT count(*) FROM {OrgUsers}")]
pub struct OrgSize;

assert_eq!(
    OrgSize::SQL,
    r#"WITH "org_users" AS (SELECT id FROM "users" WHERE org_id = $1) SELECT count(*) FROM "org_users""#
);

let params = OrgSize::builder().org_users().org_id(7).build();
assert_eq!(params.org_users.org_id, 7);
```

Some combinations are only rejected once the whole statement is rendered,
such as a data-modifying CTE on a dialect that doesn't support one. Those
show up as compile errors on the outermost query.

# Rows

A struct with named fields is the statement's row type. It gets a
`sqlx::FromRow` impl that reads each field from the column of the same
name, and `query()` returns a `sqlx` `QueryAs` for it. The macro checks the
field names against the columns the statement returns, unless those
include `*` or an expression without an alias. Unquoted names are
lowercased for PostgreSQL, as the database does.

```
# use static_typed_queries::prelude::*;
# #[table(Postgres, name = "users")]
# pub struct Users;
#[query(Postgres, sql = "SELECT id, email FROM {Users} WHERE org_id = {_: i64}")]
pub struct Member {
    pub id: i64,
    pub email: String,
}

let params = Member::builder().org_id(7).build();
assert_eq!(params.org_id, 7);
```

Only non-generic queries and statements with `RETURNING` can have fields,
and a struct with fields doesn't take `row`, `display` or `debug`.

# Generated items

For `struct Name`, the macro implements `Sql` and generates:

- `NameParams`, with a public field for every parameter and, for every
  item the query uses, a field holding that item's own params. Each item
  is listed once, so all its references share one set of values, and a
  CTE is rendered once. The types inside generic arguments are items too:
  `{CountOf<Users>}` lists `CountOf<Users>` and `Users`. An item field takes the item's alias in the template if it has
  one of at least two characters (`{CountOf<Users>} AS total` becomes
  `total`), and otherwise the type's name in snake case (`count_of`).
  Items whose names would clash use the full type in snake case, generic
  arguments included: `count_of_users`.
- `NameBuilder`, returned by `Name::builder()`. Its methods follow its
  state: a parameter's setter exists while the parameter still needs a
  value, and an item's method exists while the item has parameters left.
  The item method moves to that item's builder for one setter call, which
  then returns to the outermost builder:
  `Name::builder().active_users().org_id(7)`. Items without parameters,
  such as tables, have no method. Once everything is set, the builder has
  only `build()`, which returns `NameParams`, and, with a database feature
  enabled, `query()`, which returns a `sqlx` query with everything bound.
  A query that uses a table with hooks has `run()` instead, and `with()`
  for the values of its hooks; see [`table`].
- A `Statement` impl and an inherent `Name::SQL` constant holding the
  rendered SQL.
- With the `parse-check` feature, a `#[cfg(test)]` test named
  `name_sql_parses` that parses the rendered SQL, everything embedded,
  with the dialect's grammar. Enable it under `[dev-dependencies]`.

When several `{_: Type}` parameters end up with the same name, they share
one field of type `[Type; N]` and the setter is called `N` times, in the
order they appear in the template. Their types must match. A parameter
whose name would clash with a builder method (`build`, `builder`, `finish`,
`query`, `query_as`, `run`, `run_as` or `with`) gets a trailing underscore.

A query with neither parameters nor references uses `()` as its params
and gets no params or builder struct.

Every generated item is documented. The struct's own docs get the template
and a list of its builder's methods appended.

# Generic queries

The struct can take type parameters, which the template can reference
like any other item. A generic query isn't a `Statement` on its own; it
is rendered as part of the queries that embed it. It also can't take
`display` or `debug`.

A generic query holds no values for its type parameters. In
`{CountOf<ActiveUsers>}`, `ActiveUsers` is an item of the query that wrote
it, so its parameters are set there, once, and `CountOf` has nothing to set.

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
