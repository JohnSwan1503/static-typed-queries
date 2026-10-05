use crate::sql::Sql;

#[diagnostic::on_unimplemented(
    message = "`{Self}` holds no values",
    label = "has no values",
    note = "only a hook with values takes them through `with`"
)]
pub trait Values: Sql {}

#[diagnostic::on_unimplemented(
    message = "`{Self}` holds values, so it has to be a field of the query that uses it",
    label = "holds values",
    note = "add a field such as `#[subquery] name: {Self}` and write `{{name}}` in the template"
)]
pub trait Valueless: Sql {}

pub const fn valueless<T: Valueless>() {}
