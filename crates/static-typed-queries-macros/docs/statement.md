Makes an instantiation of a generic query a statement, with its own SQL,
`BINDS` and builder.

A generic query holds no values for its type arguments, so it can't run on
its own. A statement lists the instantiation and every type in its type
arguments as its items, so their parameters are set on its builder.

```
use std::marker::PhantomData;

use static_typed_queries::prelude::*;

#[table(Postgres, name = "users")]
pub struct Users;

#[query(Postgres, cte, sql = "SELECT id FROM {Users} WHERE org_id = {_: i64}")]
pub struct OrgUsers;

#[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
pub struct CountOf<T: Sql>(PhantomData<T>);

#[statement(CountOf<OrgUsers>)]
pub struct OrgSize;

assert_eq!(
    OrgSize::SQL,
    r#"WITH "org_users" AS (SELECT id FROM "users" WHERE org_id = $1) SELECT count(*) FROM "org_users""#
);
let params = OrgSize::builder().org_users().org_id(7).build();
assert_eq!(params.org_users.org_id, 7);
```

It takes `display`, `debug`, `row` and `parse_check` like [`query`], and
a struct with named fields is its row type, as for [`query`].
