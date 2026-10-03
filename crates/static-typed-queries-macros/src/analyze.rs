use std::fmt::Write as _;
use std::ops::ControlFlow;

use sqlparser::ast::{
    Expr, ObjectName, ObjectNamePart, Query, SelectItem, SetExpr, Statement, TableFactor, Visit,
    Visitor,
};
use sqlparser::dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;
use syn::{LitStr, Type};

use crate::template::{Segment, Template};

const PARAM: &str = "__stq_p";
const REF: &str = "__stq_r";

pub(crate) enum Kind {
    Query,
    Dml,
    Ddl,
}

#[derive(Clone, Copy)]
pub(crate) struct Position {
    pub from: bool,
    pub given_alias: bool,
}

pub(crate) struct Analysis {
    pub kind: Kind,
    pub refs: Vec<Position>,
}

#[derive(Clone, Copy)]
pub(crate) enum Engine {
    Postgres,
    MySql,
    Sqlite,
    Generic,
}

impl Engine {
    pub(crate) fn of(dialect: &Type) -> Engine {
        let Type::Path(path) = dialect else {
            return Engine::Generic;
        };
        match path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string())
            .as_deref()
        {
            Some("Postgres") => Engine::Postgres,
            Some("MySql") => Engine::MySql,
            Some("Sqlite") => Engine::Sqlite,
            _ => Engine::Generic,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Engine::Postgres => "PostgreSQL",
            Engine::MySql => "MySQL",
            Engine::Sqlite => "SQLite",
            Engine::Generic => "generic SQL",
        }
    }

    fn dialect(self) -> Box<dyn Dialect> {
        match self {
            Engine::Postgres => Box::new(PostgreSqlDialect {}),
            Engine::MySql => Box::new(MySqlDialect {}),
            Engine::Sqlite => Box::new(SQLiteDialect {}),
            Engine::Generic => Box::new(GenericDialect {}),
        }
    }
}

pub(crate) fn analyze(template: &Template, engine: Engine, sql: &LitStr) -> syn::Result<Analysis> {
    let error = |message: String| syn::Error::new(sql.span(), message);
    if sql.value().trim_end().ends_with(';') {
        return Err(error(
            "remove the trailing `;`: templates get embedded in other statements".into(),
        ));
    }

    let mut text = String::new();
    let mut refs = Vec::new();
    for segment in &template.segments {
        match segment {
            Segment::Lit(lit) => text.push_str(lit),
            Segment::Param(slot) => write!(text, "{PARAM}{slot}").unwrap(),
            Segment::Ref(item) if item.target => {
                write!(text, "{REF}{}", refs.len()).unwrap();
                refs.push(Position {
                    from: true,
                    given_alias: false,
                });
            }
            Segment::Ref(_) => {
                write!(text, "(SELECT {REF}{})", refs.len()).unwrap();
                refs.push(Position {
                    from: false,
                    given_alias: false,
                });
            }
        }
    }

    let statements = Parser::parse_sql(engine.dialect().as_ref(), &text).map_err(|e| {
        error(format!(
            "invalid SQL for {}: {e}\n  checked: {}",
            engine.name(),
            text.trim()
        ))
    })?;
    let [statement] = statements.as_slice() else {
        return Err(error(
            "a template must contain exactly one statement".into(),
        ));
    };
    let kind = match statement {
        Statement::Query(_) => Kind::Query,
        Statement::Insert(_)
        | Statement::Update(_)
        | Statement::Delete(_)
        | Statement::Merge(_) => Kind::Dml,
        _ => Kind::Ddl,
    };

    let mut analyzer = Analyzer { refs };
    let _ = statement.visit(&mut analyzer);
    Ok(Analysis {
        kind,
        refs: analyzer.refs,
    })
}

struct Analyzer {
    refs: Vec<Position>,
}

impl Analyzer {
    fn place(&mut self, index: Option<usize>, given_alias: bool) {
        if let Some(position) = index.and_then(|index| self.refs.get_mut(index)) {
            *position = Position {
                from: true,
                given_alias,
            };
        }
    }
}

impl Visitor for Analyzer {
    type Break = ();

    fn pre_visit_table_factor(&mut self, factor: &TableFactor) -> ControlFlow<()> {
        match factor {
            TableFactor::Derived {
                subquery, alias, ..
            } => self.place(ref_marker(subquery), alias.is_some()),
            TableFactor::Table { name, alias, .. } => self.place(ref_name(name), alias.is_some()),
            _ => {}
        }
        ControlFlow::Continue(())
    }
}

fn ref_marker(query: &Query) -> Option<usize> {
    let SetExpr::Select(select) = &*query.body else {
        return None;
    };
    match select.projection.as_slice() {
        [SelectItem::UnnamedExpr(Expr::Identifier(ident))] => {
            ident.value.strip_prefix(REF)?.parse().ok()
        }
        _ => None,
    }
}

fn ref_name(name: &ObjectName) -> Option<usize> {
    match name.0.as_slice() {
        [ObjectNamePart::Identifier(ident)] => ident.value.strip_prefix(REF)?.parse().ok(),
        _ => None,
    }
}
