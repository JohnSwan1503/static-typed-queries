#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Table,
    Query,
    Dml,
    Ddl,
    Scope,
    Transaction,
}
