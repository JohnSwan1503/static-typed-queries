Turns a struct into a transaction: statements, its steps, that run in
order and commit together.

```
use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[table(Postgres, name = "orders")]
pub struct Orders;

#[query(Postgres, sql = "DELETE FROM {Orders} WHERE created_at < {before: i64}")]
pub struct Archive;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[transaction(Postgres, steps(Archive, CountOf<Orders>))]
pub struct Nightly;

assert_eq!(
    Nightly::STEPS[0].sql(),
    r#"DELETE FROM "orders" WHERE created_at < $1"#
);
assert_eq!(Nightly::STEPS[1].sql(), r#"SELECT count(*) FROM "orders""#);
let params = Nightly::builder().archive().before(1).build();
assert_eq!(params.archive.before, 1);
```

The first argument is the dialect, followed by `steps(Type, ...)`: the
statements to run, in order. A step is a [`query`], a [`statement`], or an
instantiation of a generic query such as `CountOf<Orders>`.

## Running

The complete builder's `run(conn)` takes anything that implements
`sqlx::Acquire`, runs the steps in order in one transaction and commits
them together. If a step outside a savepoint fails, every step rolls back.
It returns a tuple with one element per step: a `Vec` of the step's row
type when it has one, and otherwise the number of rows the step affected.
`Type as one` reads exactly one row instead, failing with
`sqlx::Error::RowNotFound` when there's none, and `Type as optional` reads
an `Option` of one. Both need a step with a row type.

`Type as cte` attaches a step to the next one: it runs in that step's
`WITH` clause, under its name, as part of the same statement, and has no
element of its own in the output. A step that modifies data can only be
attached in dialects that allow that in a CTE, such as PostgreSQL.

`savepoint(A, B)` among the steps runs a group in a nested transaction. If
one of its steps fails, only the group rolls back, and the steps after it
still run and commit. The group's element in the output is a `Result`
holding a tuple of its steps' outputs, or the error. Savepoints can nest.

The hooks of the tables the steps use run once for the whole transaction,
`before` hooks ahead of the first step and `after` hooks after the last,
with values from `with` as for a single statement.

## Generated items

The struct implements `Sql`, `Build` and `Transaction`, whose `STEPS`
holds each step's SQL and binds. Like a query, the transaction lists its
step types once as its items, along with the types in their type
arguments. A step listed twice shares its values, and the type arguments
of a generic step get theirs from the transaction. `NameParams` and
`NameBuilder` work as for [`query`]. The struct can't be generic or have
fields.
