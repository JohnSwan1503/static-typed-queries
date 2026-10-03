#![cfg(any(
    feature = "mysql",
    feature = "postgres",
    feature = "sqlite",
    feature = "sqlite-unbundled"
))]

#[macro_use]
mod support;

use static_typed_queries_core::node::Node;
use static_typed_queries_core::node::inject::Inject;
use static_typed_queries_core::part::from::From;
use static_typed_queries_core::part::from::rule::AliasRule;
use static_typed_queries_core::part::ident::Ident;
use static_typed_queries_core::part::lit::Lit;
use static_typed_queries_core::part::param::Param;

const RECENT: &Node = node!(
    "recent",
    1,
    Query,
    Inject::Cte { recursive: false },
    [
        Lit::part("SELECT id FROM "),
        Ident::part("events"),
        Lit::part(" WHERE at > "),
        Param::part(0, "since", "i64"),
        Lit::part(" OR at > "),
        Param::part(0, "since", "i64"),
    ]
);

const RECENT_COUNT: &Node = node!(
    "recent_count",
    2,
    Query,
    Inject::Subquery,
    [
        Lit::part("SELECT count(*) FROM "),
        From::part(RECENT, AliasRule::NodeName, None),
        Lit::part(" WHERE id > "),
        Param::part(0, "min_id", "i64"),
    ]
);

#[cfg(feature = "mysql")]
mod mysql {
    use static_typed_queries_core::dialect::mysql::MySql;
    use static_typed_queries_core::statement::Statement;

    root!(RecentCount: MySql = super::RECENT_COUNT);

    #[test]
    fn renders() {
        assert_eq!(
            RecentCount::SQL,
            "WITH `recent` AS (SELECT id FROM `events` WHERE at > ? OR at > ?) SELECT count(*) FROM `recent` WHERE id > ?"
        );
    }
}

#[cfg(feature = "postgres")]
mod postgres {
    use static_typed_queries_core::dialect::postgres::Postgres;
    use static_typed_queries_core::statement::Statement;

    root!(RecentCount: Postgres = super::RECENT_COUNT);

    #[test]
    fn renders() {
        assert_eq!(
            RecentCount::SQL,
            r#"WITH "recent" AS (SELECT id FROM "events" WHERE at > $1 OR at > $1) SELECT count(*) FROM "recent" WHERE id > $2"#
        );
    }
}

#[cfg(any(feature = "sqlite", feature = "sqlite-unbundled"))]
mod sqlite {
    use static_typed_queries_core::dialect::sqlite::Sqlite;
    use static_typed_queries_core::statement::Statement;

    root!(RecentCount: Sqlite = super::RECENT_COUNT);

    #[test]
    fn renders() {
        assert_eq!(
            RecentCount::SQL,
            r#"WITH "recent" AS (SELECT id FROM "events" WHERE at > $1 OR at > $1) SELECT count(*) FROM "recent" WHERE id > $2"#
        );
    }
}
