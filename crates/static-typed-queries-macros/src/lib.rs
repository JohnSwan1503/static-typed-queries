mod analyze;
mod args;
mod docs;
mod expand;
mod naming;
mod template;

use proc_macro::TokenStream;
use syn::{ItemStruct, parse_macro_input};

#[cfg(doc)]
use static_typed_queries::dialect::postgres::Postgres;

/// Declares a database table that [`query`] templates can reference as `{Type}`.
///
/// ```
/// use static_typed_queries::dialect::postgres::Postgres;
/// use static_typed_queries::prelude::*;
///
/// #[table(Postgres, name = "audit.events")]
/// pub struct Events;
///
/// #[query(Postgres, sql = "SELECT id FROM {Events}")]
/// pub struct EventIds;
///
/// assert_eq!(EventIds::SQL, r#"SELECT id FROM "audit"."events""#);
/// ```
///
/// ## Arguments
///
/// 1. ***Dialect***: The first argument is always a type that implements `Dialect`,
///   This crate provides implementations for the following, available via their
///   respective feature flags:
///     * MySQL: [`MySql`]()
///     * PostgreSQL: [`Postgres`](::static_typed_queries::dialect::postgres::Postgres)
///     * SQLite: [`Sqlite`](::static_typed_queries::dialect::sqlite::Sqlite)
/// 2.
///
/// The dialect type comes first, followed by any of these in any order:
///
/// - `name = "..."` (required): the table's name in the database. Write a
///   schema-qualified name with dots; each segment is quoted on its own using
///   the dialect's identifier quotes.
/// - `display = name`: implements `Display`, writing the unqualified table name.
/// - `debug = tree`: implements `Debug`, writing the table's node tree.
///
/// ## Generated items
///
/// The struct implements `Sql` with no parameters, and `Build` with a builder
/// that has nothing to set, so queries that reference it never need a builder
/// call for it.
///
/// A table has no SQL of its own: it doesn't implement `Statement`, and it is
/// always rendered as its name, so `{Table as cte}` and `{Table as subquery}`
/// fail to compile. The struct can't be generic.
#[proc_macro_attribute]
pub fn table(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as args::Args);
    let item = parse_macro_input!(item as ItemStruct);
    expand::table(args, item)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}

/// Turns a struct into a SQL statement that is checked, composed and rendered
/// at compile time from a template.
///
/// ```
/// use static_typed_queries::dialect::postgres::Postgres;
/// use static_typed_queries::prelude::*;
///
/// #[table(Postgres, name = "users")]
/// pub struct Users;
///
/// #[query(
///     Postgres,
///     sql = "
///     SELECT id, email FROM {Users}
///     WHERE org_id = {org_id: i64} AND email LIKE {_: String}"
/// )]
/// pub struct UsersByOrg;
///
/// assert_eq!(
///     UsersByOrg::SQL,
///     r#"SELECT id, email FROM "users" WHERE org_id = $1 AND email LIKE $2"#
/// );
///
/// let params = UsersByOrg::builder()
///     .org_id(7)
///     .email("%@example.com".to_owned())
///     .build();
/// assert_eq!(params.org_id, 7);
/// ```
///
/// # Arguments
///
/// The dialect type comes first, followed by any of these in any order:
///
/// - `sql = "..."` (required): the template, described below.
/// - `name = "..."`: the name this query goes by when another query embeds
///   it, as a CTE name or a subquery alias. Defaults to the struct name in
///   snake case.
/// - `cte`, `cte(recursive)` or `subquery`: how this query is embedded when
///   another query references it in a `FROM` or `JOIN`. Defaults to
///   `subquery`.
/// - `display = sql` or `display = name`: implements `Display`, writing the
///   rendered SQL or the name.
/// - `debug = sql` or `debug = tree`: implements `Debug`, writing the rendered
///   SQL as a quoted string or the query's node tree.
/// - `parse_check = false`: leaves the query out of the `parse-check`
///   feature's tests. Use it on queries defined inside functions, where rustc
///   can't run the generated test.
/// - `separate(Type, ...)`: gives each listed generic item its own values for
///   the items in its type arguments, instead of sharing the query's. With
///   `separate(CountOf<ActiveUsers>)`, `.active_users()` and
///   `.count_of().active_users()` set two different `ActiveUsers`.
///
/// The template is parsed with the grammar of the dialect, which is picked by
/// the last segment of the dialect path: `Postgres`, `MySql` or `Sqlite`. Any
/// other path, such as `T::Dialect`, is checked against a generic grammar.
///
/// # Template
///
/// The template is one SQL statement without a trailing `;`. Placeholders in
/// braces are replaced when the statement is rendered:
///
/// - `{Type}` embeds another [`table`] or `query`. `{Type as cte}`,
///   `{Type as cte(recursive)}` and `{Type as subquery}` override how that
///   item is embedded at this one place.
/// - `{name: Type}` declares a bind parameter and uses it. It can be declared
///   only once; write `{name}` to use it again.
/// - `{_: Type}` is a bind parameter named after its surroundings: the column
///   it is compared with (`=`, `<`, `BETWEEN`, `IN (...)`, `LIKE`, `ANY`, and
///   so on), the column it is inserted into or assigned to, its `AS` alias in
///   a select list, or `limit` and `offset`. When nothing names it, it is
///   called `bindN`, where `N` is its position among the parameters.
///
/// Write `{{` and `}}` for literal braces. Braces inside quoted strings,
/// quoted identifiers, dollar-quoted strings and comments are left alone.
/// Whitespace is collapsed and `--` comments are dropped.
///
/// ## Embedding other items
///
/// A reference in a `FROM` or `JOIN`, or as the target of an `INSERT`,
/// `UPDATE` or `DELETE`, is embedded the way the item asks for. A table is
/// written as its quoted name. A CTE is hoisted into the statement's `WITH`
/// clause and referenced by name. A subquery is inlined in parentheses, with
/// the item's name as its alias unless the template gives one. A reference
/// anywhere else is always inlined as a parenthesized subquery.
///
/// ```
/// # use static_typed_queries::dialect::postgres::Postgres;
/// # use static_typed_queries::prelude::*;
/// # #[table(Postgres, name = "users")]
/// # pub struct Users;
/// #[query(
///     Postgres,
///     cte,
///     sql = "SELECT id FROM {Users} WHERE org_id = {org_id: i64}"
/// )]
/// pub struct OrgUsers;
///
/// #[query(Postgres, sql = "SELECT count(*) FROM {OrgUsers}")]
/// pub struct OrgSize;
///
/// assert_eq!(
///     OrgSize::SQL,
///     r#"WITH "org_users" AS (SELECT id FROM "users" WHERE org_id = $1) SELECT count(*) FROM "org_users""#
/// );
///
/// let params = OrgSize::builder().org_users().org_id(7).build();
/// assert_eq!(params.org_users.org_id, 7);
/// ```
///
/// Some combinations are only rejected once the whole statement is rendered,
/// such as a data-modifying CTE on a dialect that doesn't support one. Those
/// show up as compile errors on the outermost query.
///
/// # Generated items
///
/// For `struct Name`, the macro implements `Sql` and generates:
///
/// - `NameParams`, with a public field for every parameter and, for every
///   item the query uses, a field holding that item's own params. Each item
///   is listed once, so all its references share one set of values, and a
///   CTE is rendered once. The types inside generic arguments are items too:
///   `{CountOf<Users>}` lists `CountOf<Users>` and `Users`. An item field takes the item's alias in the template if it has
///   one of at least two characters (`{CountOf<Users>} AS total` becomes
///   `total`), and otherwise the type's name in snake case (`count_of`).
///   Items whose names would clash use the full type in snake case, generic
///   arguments included: `count_of_users`.
/// - `NameBuilder`, returned by `Name::builder()`. It has a setter for every
///   parameter and, for every referenced item, a method that moves to that
///   item's builder for one setter call, which then returns to the outermost
///   builder: `Name::builder().active_users().org_id(7)`. Items without
///   parameters, such as tables, need no call. `build()` returns
///   `NameParams`, and with a database feature enabled `query()` returns a
///   `sqlx` query with everything bound. Both only compile once every
///   parameter is set.
/// - A `Statement` impl and an inherent `Name::SQL` constant holding the
///   rendered SQL.
/// - With the `parse-check` feature, a `#[cfg(test)]` test named
///   `name_sql_parses` that parses the rendered SQL, everything embedded,
///   with the dialect's grammar. Enable it under `[dev-dependencies]`.
///
/// When several `{_: Type}` parameters end up with the same name, they share
/// one field of type `[Type; N]` and the setter is called `N` times, in the
/// order they appear in the template. Their types must match. A parameter
/// whose name would clash with a builder method (`build`, `builder`, `finish`
/// or `query`) gets a trailing underscore.
///
/// A query with neither parameters nor references uses `()` as its params
/// and gets no params or builder struct.
///
/// Every generated item is documented. The struct's own docs get the template
/// and a list of its builder's methods appended.
///
/// # Generic queries
///
/// The struct can take type parameters, which the template can reference
/// like any other item. A generic query isn't a `Statement` on its own; it
/// is rendered as part of the queries that embed it. It also can't take
/// `display` or `debug`.
///
/// A generic query holds no values for its type parameters. In
/// `{CountOf<ActiveUsers>}`, `ActiveUsers` is an item of the query that wrote
/// it, so its parameters are set there, once, and `CountOf` has nothing to set.
///
/// ```
/// # use static_typed_queries::dialect::postgres::Postgres;
/// # use static_typed_queries::prelude::*;
/// # #[table(Postgres, name = "users")]
/// # pub struct Users;
/// use std::marker::PhantomData;
///
/// #[query(T::Dialect, sql = "SELECT count(*) FROM {T}")]
/// pub struct CountOf<T: Sql>(PhantomData<T>);
///
/// #[query(Postgres, sql = "SELECT {CountOf<Users>} AS users")]
/// pub struct Totals;
///
/// assert_eq!(
///     Totals::SQL,
///     r#"SELECT (SELECT count(*) FROM "users") AS users"#
/// );
/// ```
#[proc_macro_attribute]
pub fn query(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as args::Args);
    let item = parse_macro_input!(item as ItemStruct);
    expand::query(args, item)
        .unwrap_or_else(|error| error.to_compile_error())
        .into()
}
