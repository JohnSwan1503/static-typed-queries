# Changelog

`static-typed-queries`, `static-typed-queries-core` and
`static-typed-queries-macros` are released together, so one entry covers
all three.

## 0.1.0 - Unreleased

The first release.

- Tables, queries, statements and transactions declared as structs with
  `#[table]`, `#[query]`, `#[statement]` and `#[transaction]`, and shared
  templates with `#[sql]`.
- Templates checked against the PostgreSQL, MySQL or SQLite grammar and
  rendered into a constant at compile time, with queries embedded as CTEs
  or subqueries.
- Builders whose setters and `build` only exist while they're valid.
- Hooks that run around every statement using a table, transactions with
  savepoints, and running through sqlx 0.9.
- `crate = path` for a renamed or re-exported dependency.
