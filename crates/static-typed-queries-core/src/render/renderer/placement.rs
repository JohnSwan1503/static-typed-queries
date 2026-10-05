use crate::dialect::Dialect;
use crate::node::Node;
use crate::node::inject::Inject;
use crate::node::kind::Kind;
use crate::part::from::From;
use crate::render::error::fail;

// A table is always written as its name, whatever placement the reference asks for, so a generic
// field marked for queries can hold one too.
pub(super) const fn placement<D: Dialect>(from: From, node: &'static Node) -> Inject {
    let inject = from.inject(node);
    let name = node.name.as_str();
    match (node.kind, inject) {
        (Kind::Table, _) => return Inject::Ident,
        (Kind::Query, Inject::Cte { .. } | Inject::Subquery) => {}
        (_, Inject::Ident) => fail(&[
            "`",
            name,
            "` isn't a table, so it can't be referenced by name",
        ]),
        (Kind::Dml, Inject::Cte { .. }) if D::DML_IN_CTE => {}
        (Kind::Dml, Inject::Cte { .. }) => fail(&[
            "`",
            name,
            "` modifies data, which ",
            D::NAME.as_str(),
            " doesn't allow in a CTE",
        ]),
        (Kind::Dml, _) => fail(&[
            "`",
            name,
            "` modifies data, so it can only be embedded as a CTE",
        ]),
        (Kind::Ddl, _) => fail(&["`", name, "` is DDL, so it can't be embedded"]),
        (Kind::Scope, _) => fail(&["`", name, "` only holds values, so it can't be embedded"]),
        (Kind::Transaction, _) => fail(&["`", name, "` is a transaction, so it can't be embedded"]),
    }
    inject
}

pub(super) const fn attachable<D: Dialect>(node: &'static Node) {
    let name = node.name.as_str();
    match node.kind {
        Kind::Query => {}
        Kind::Dml if D::DML_IN_CTE => {}
        Kind::Dml => fail(&[
            "`",
            name,
            "` modifies data, which ",
            D::NAME.as_str(),
            " doesn't allow in a CTE",
        ]),
        _ => fail(&["`", name, "` can't run as a CTE"]),
    }
}
