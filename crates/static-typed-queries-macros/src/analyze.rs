use sqlparser::ast::Statement;
use sqlparser::dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;
use syn::{LitStr, Type};

use crate::template::{Segment, Template};

pub(crate) enum Kind {
    Query,
    Dml,
    Ddl,
}

pub(crate) struct Analysis {
    pub kind: Kind,
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
    for segment in &template.segments {
        match segment {
            Segment::Lit(lit) => text.push_str(lit),
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

    Ok(Analysis { kind })
}
