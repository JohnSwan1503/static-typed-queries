macro_rules! dialect {
    (@dml_in_cte true $ty:ident) => {
        impl crate::embed::DmlInCte for $ty {}
    };
    (@dml_in_cte false $ty:ident) => {};
    ($module:ident::$ty:ident {
        feature: $feature:literal,
        name: $name:literal,
        grammar: $grammar:ident,
        prefix: $prefix:literal,
        numbered: $numbered:literal,
        quote: ($open:literal, $close:literal),
        dml_in_cte: $dml_in_cte:tt,
    }) => {
        #[cfg(any(test, feature = $feature))]
        pub mod $module {
            use crate::dialect::Dialect;
            use crate::dialect::params::ParamStyle;
            use crate::dialect::quote::Quote;

            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            pub struct $ty;

            impl Dialect for $ty {
                const NAME: &'static str = $name;
                const PARAMS: ParamStyle = ParamStyle {
                    prefix: $prefix,
                    numbered: $numbered,
                };
                const QUOTE: Quote = Quote::new($open, $close);
                const DML_IN_CTE: bool = $dml_in_cte;

                #[cfg(feature = "parse-check")]
                fn grammar() -> Box<dyn sqlparser::dialect::Dialect> {
                    Box::new(sqlparser::dialect::$grammar {})
                }
            }

            dialect!(@dml_in_cte $dml_in_cte $ty);

            // sqlx names its databases like the dialects.
            #[cfg(feature = $feature)]
            impl crate::dialect::driver::Driver for $ty {
                type Database = sqlx::$ty;

                fn executor(
                    conn: &mut <sqlx::$ty as sqlx::Database>::Connection,
                ) -> impl sqlx::Executor<'_, Database = sqlx::$ty> {
                    conn
                }

                fn rows_affected(result: &<sqlx::$ty as sqlx::Database>::QueryResult) -> u64 {
                    result.rows_affected()
                }
            }
        }
    };
}

dialect!(mysql::MySql {
    feature: "mysql",
    name: "MySQL",
    grammar: MySqlDialect,
    prefix: b'?',
    numbered: false,
    quote: (b'`', b'`'),
    dml_in_cte: false,
});

dialect!(postgres::Postgres {
    feature: "postgres",
    name: "PostgreSQL",
    grammar: PostgreSqlDialect,
    prefix: b'$',
    numbered: true,
    quote: (b'"', b'"'),
    dml_in_cte: true,
});

dialect!(sqlite::Sqlite {
    feature: "sqlite",
    name: "SQLite",
    grammar: SQLiteDialect,
    prefix: b'$',
    numbered: true,
    quote: (b'"', b'"'),
    dml_in_cte: false,
});
