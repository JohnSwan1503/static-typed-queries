use syn::{LitStr, Type};

use crate::args::Step;

pub(crate) enum Role<'a> {
    Table {
        before: &'a [Type],
        after: &'a [Type],
    },
    Query {
        sql: &'a LitStr,
    },
    Statement {
        target: &'a Type,
    },
    Transaction {
        steps: &'a [Step],
    },
}
