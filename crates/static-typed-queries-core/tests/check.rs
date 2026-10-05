#[macro_use]
mod support;

use static_typed_queries_core::check;
use static_typed_queries_core::dialect::mysql::MySql;
use static_typed_queries_core::dialect::postgres::Postgres;
use static_typed_queries_core::dialect::sqlite::Sqlite;
use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::ident::Ident;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::part::param::Param;
use static_typed_queries_core::part::target::Target;

const MEMBERS: &Node = node!(
    "members",
    1,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT id FROM "),
        Ident::part("users"),
        Lit::part(" WHERE org_id = "),
        Param::part(0, "org_id", "i64"),
    ]
);

root!(MembersPostgres: Postgres = MEMBERS);
root!(MembersMySql: MySql = MEMBERS);
root!(MembersSqlite: Sqlite = MEMBERS);
root!(Broken: Postgres = node!(
    "broken",
    2,
    Query,
    Inject::Subquery,
    [Lit::part("SELEC 1")]
));

const BROKEN_HOOK: &Node = node!(
    "broken_hook",
    3,
    Query,
    Inject::Subquery,
    [Lit::part("SELEC 1")]
);

const HOOKED: &Node = node!(
    "hooked",
    4,
    Table,
    Inject::Ident,
    [Ident::part("hooked")],
    items = [BROKEN_HOOK],
    before = [BROKEN_HOOK]
);

root!(UsesHooked: Postgres = node!(
    "uses_hooked",
    5,
    Query,
    Inject::Subquery,
    [Lit::part("SELECT * FROM "), From::part(Target::Item(0), AliasRule::NodeName, None)],
    items = [HOOKED]
));

#[test]
fn rendered_sql_parses_in_each_dialect() {
    check::parse::<MembersPostgres>();
    check::parse::<MembersMySql>();
    check::parse::<MembersSqlite>();
}

#[test]
#[should_panic(expected = "the SQL of `broken` doesn't parse as PostgreSQL")]
fn unparsable_sql_panics_with_the_item_and_dialect() {
    check::parse::<Broken>();
}

#[test]
#[should_panic(
    expected = "the SQL of `broken_hook`, a hook of `uses_hooked`, doesn't parse as PostgreSQL"
)]
fn hooks_are_checked_too() {
    check::parse::<UsesHooked>();
}
