Declares a database table that [`query`] templates can reference as `{Type}`.

```
use static_typed_queries::prelude::*;

#[table(Postgres, name = "audit.events")]
pub struct Events;

#[query(Postgres, sql = "SELECT id FROM {Events}")]
pub struct EventIds;

assert_eq!(EventIds::SQL, r#"SELECT id FROM "audit"."events""#);
```

## Arguments

The first argument is the dialect: `MySql`, `Postgres` or `Sqlite`, each
behind the feature of the same name. It's followed by any of these in any
order:

- `name = "..."` (required): the table's name in the database. Write a
  schema-qualified name with dots; each segment is quoted on its own using
  the dialect's identifier quotes.
- `before(Type, ...)` and `after(Type, ...)`: hooks, statements that run
  before and after every statement that uses the table.
- `display = name`: implements `Display`, writing the unqualified table name.
- `debug = tree`: implements `Debug`, writing the table's node tree.

## Hooks

A statement that uses a table with hooks, directly or through the items it
embeds, renders them as statements of their own in its `BEFORE` and `AFTER`
lists. Each hook runs once per statement, however many tables or paths
reach it. The statement has no `query()` or `query_as()`, since one query
would skip the hooks. It has `run(conn)` instead, which takes anything that
implements `sqlx::Acquire` and runs the hooks and the statement in one
transaction, so a failing hook rolls back the statement too. `run` returns
a `Running` future of the statement's rows when it has `row = Type`, and
otherwise of the number of rows it affected. The future is `Send`, so the
values it binds must be `Send` and `Sync`. `run_as(conn)` reads the rows as
any other `sqlx::FromRow` type, named by annotation or as
`run_as::<T, _, _>`.

A hook with values takes them from the run: pass the hook's complete
builder, or a value of the hook, to `with`, which returns a `With` that has
the same `with`, `run` and `run_as`. `run` doesn't compile until every hook
the statement reaches has its values, and the error names the hook that's
missing. Values for a hook the statement never runs don't compile either,
and neither do a hook's values given twice.

```
# use static_typed_queries::prelude::*;
# use static_typed_queries::__private::__private::sqlx;
#[query(Postgres, sql = "SELECT set_config('app.tenant', {tenant}, true)")]
pub struct SetTenant {
    pub tenant: String,
}

#[table(Postgres, name = "orders", before(SetTenant))]
pub struct Orders;

#[query(Postgres, sql = "SELECT id FROM {Orders} WHERE status = {status}")]
pub struct ByStatus {
    pub status: String,
}

assert_eq!(
    ByStatus::BEFORE[0].sql(),
    "SELECT set_config('app.tenant', $1, true)"
);

async fn open_orders(conn: &mut sqlx::PgConnection) -> sqlx::Result<u64> {
    ByStatus::builder()
        .status("open".to_owned())
        .with(SetTenant::builder().tenant("acme".to_owned()))
        .run(conn)
        .await
}
```

In a [`transaction`], each hook runs once around all of its steps. Hooks
can't be tables, and they can't use tables with hooks of their own.

## Generated items

The struct implements `Sql`. A table holds no values, even with hooks, so
queries reference it by type, as `{Table}`, and never hold it in a field.

A table has no SQL of its own: it doesn't implement `Statement`, and it is
always rendered as its name, whatever placement a reference or a generic
field asks for. The struct can't be generic or have fields.
