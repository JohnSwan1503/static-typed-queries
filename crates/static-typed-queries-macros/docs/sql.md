Declares a SQL template as a `&str` constant that [`query`] takes as
`sql = NAME`, so several queries can share one template, such as one per
dialect.

```
use static_typed_queries::prelude::*;

#[sql]
pub const BY_ORG: &str = "SELECT id FROM {Users} WHERE org_id = {org_id}";

mod pg {
    use static_typed_queries::prelude::*;

    #[table(Postgres, name = "users")]
    pub struct Users;

    #[query(Postgres, sql = super::BY_ORG)]
    pub struct ByOrg {
        pub org_id: i64,
    }
}

mod my {
    use static_typed_queries::prelude::*;

    #[table(MySql, name = "users")]
    pub struct Users;

    #[query(MySql, sql = super::BY_ORG)]
    pub struct ByOrg {
        pub org_id: i64,
    }
}

# fn main() {
assert_eq!(pg::ByOrg::SQL, r#"SELECT id FROM "users" WHERE org_id = $1"#);
assert_eq!(my::ByOrg::SQL, "SELECT id FROM `users` WHERE org_id = ?");
# }
```

Names in the template, such as `{Users}` above, resolve where each query
is declared. The constant stays a plain `&str`. `sql = NAME` works through
any path to it within the crate that declares it, such as
`crate::queries::BY_ORG`, but not from other crates. A plain `const` can't
be a query's template, because `#[query]` reads the template before
constants have values.
