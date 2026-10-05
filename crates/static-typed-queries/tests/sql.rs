use static_typed_queries::prelude::*;

mod queries {
    use static_typed_queries::prelude::*;

    #[sql]
    pub const BY_ORG: &str = "SELECT id FROM {Users} WHERE org_id = {org_id: i64}";
}

mod pg {
    use static_typed_queries::prelude::*;

    #[table(Postgres, name = "users")]
    pub struct Users;

    #[query(Postgres, sql = crate::queries::BY_ORG)]
    pub struct ByOrg;
}

mod my {
    use static_typed_queries::prelude::*;

    #[table(MySql, name = "users")]
    pub struct Users;

    #[query(MySql, sql = crate::queries::BY_ORG)]
    pub struct ByOrg;
}

mod file {
    use static_typed_queries::prelude::*;

    #[table(Postgres, name = "users")]
    pub struct Users;

    #[query(Postgres, sql_file = "tests/templates/by_org.sql")]
    pub struct ByOrg;
}

#[table(Postgres, name = "events")]
pub struct Events;

#[sql]
const RECENT: &str = "SELECT id FROM {Events} ORDER BY at DESC LIMIT {_: i64}";

#[query(Postgres, sql = RECENT)]
pub struct Recent;

mod nested {
    use static_typed_queries::prelude::*;

    #[sql]
    const RECENT: &str = "SELECT id FROM {super::Events} LIMIT 1";

    #[query(Postgres, sql = RECENT)]
    pub struct Latest;
}

#[test]
fn queries_share_a_template_across_dialects() {
    assert_eq!(
        pg::ByOrg::SQL,
        r#"SELECT id FROM "users" WHERE org_id = $1"#
    );
    assert_eq!(my::ByOrg::SQL, "SELECT id FROM `users` WHERE org_id = ?");
    assert_eq!(my::ByOrg::builder().org_id(7).build().org_id, 7);
    assert_eq!(
        queries::BY_ORG,
        "SELECT id FROM {Users} WHERE org_id = {org_id: i64}"
    );
}

#[test]
fn each_module_resolves_its_own_template() {
    assert_eq!(
        Recent::SQL,
        r#"SELECT id FROM "events" ORDER BY at DESC LIMIT $1"#
    );
    assert_eq!(nested::Latest::SQL, r#"SELECT id FROM "events" LIMIT 1"#);
}

#[test]
fn templates_can_come_from_files() {
    assert_eq!(file::ByOrg::SQL, pg::ByOrg::SQL);
    assert_eq!(file::ByOrg::builder().org_id(7).build().org_id, 7);
}

#[test]
fn templates_can_be_declared_inside_functions() {
    #[sql]
    const ONE: &str = "SELECT 1";

    #[query(Postgres, parse_check = false, sql = ONE)]
    struct One;

    assert_eq!(One::SQL, "SELECT 1");
}
