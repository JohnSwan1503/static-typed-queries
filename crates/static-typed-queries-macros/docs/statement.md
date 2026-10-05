Makes an instantiation of a generic query a statement, with its own SQL
and `BINDS`.

A generic query isn't a statement on its own. A statement names an
instantiation: a field of the struct, when the instantiation holds values,
or a type, when it holds none.

```
use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, sql = "SELECT id FROM {Users} WHERE org_id = {org_id}")]
pub struct OrgUsers {
    pub org_id: i64,
}

#[query(T::Dialect, sql = "SELECT count(*) FROM {of}")]
pub struct CountOf<T: Sql> {
    #[cte]
    pub of: T,
}

#[statement(count)]
pub struct OrgSize {
    pub count: CountOf<OrgUsers>,
}

assert_eq!(
    OrgSize::SQL,
    r#"WITH "org_users" AS (SELECT id FROM "users" WHERE org_id = $1) SELECT count(*) FROM "org_users""#
);
let size = OrgSize::builder().count().of().org_id(7).build();
assert_eq!(size.count.of.org_id, 7);
```

It takes `display`, `debug`, `row` and `parse_check` like [`query`]. The
struct's only field, if any, is the one the attribute names. Its builder
reaches the item's fields through the field's method, as a query's does.
