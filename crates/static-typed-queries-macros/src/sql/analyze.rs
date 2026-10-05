use std::fmt::Write as _;
use std::ops::ControlFlow;

use sqlparser::ast::{
    Expr, Ident as SqlIdent, ObjectName, ObjectNamePart, Query, SelectItem, SetExpr, Statement,
    TableAlias, TableFactor, Visit, Visitor,
};
use sqlparser::dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::Parser;
use syn::{Generics, Ident, LitStr, Type};

use crate::sql::template::{Segment, Template};

const PARAM: &str = "__stq_p";
const REF: &str = "__stq_r";

pub(crate) enum Kind {
    Query,
    Dml,
    Ddl,
}

#[derive(Clone)]
pub(crate) struct Position {
    pub from: bool,
    pub alias: Option<String>,
}

pub(crate) struct Analysis {
    pub kind: Kind,
    pub returns_rows: bool,
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
    pub(crate) fn of(
        dialect: &Type,
        grammar: Option<&Ident>,
        generics: &Generics,
    ) -> syn::Result<Engine> {
        if let Some(grammar) = grammar {
            return match grammar.to_string().as_str() {
                "postgres" => Ok(Engine::Postgres),
                "mysql" => Ok(Engine::MySql),
                "sqlite" => Ok(Engine::Sqlite),
                "generic" => Ok(Engine::Generic),
                _ => Err(syn::Error::new(
                    grammar.span(),
                    "expected `postgres`, `mysql`, `sqlite` or `generic`",
                )),
            };
        }
        if let Type::Path(path) = dialect
            && path.qself.is_none()
        {
            let segments = &path.path.segments;
            match segments
                .last()
                .map(|segment| segment.ident.to_string())
                .as_deref()
            {
                Some("Postgres") => return Ok(Engine::Postgres),
                Some("MySql") => return Ok(Engine::MySql),
                Some("Sqlite") => return Ok(Engine::Sqlite),
                Some("Dialect")
                    if segments.len() == 2
                        && generics
                            .type_params()
                            .any(|param| param.ident == segments[0].ident) =>
                {
                    return Ok(Engine::Generic);
                }
                _ => {}
            }
        }
        Err(syn::Error::new_spanned(
            dialect,
            format!(
                "can't tell which SQL grammar `{}` uses; add `grammar = postgres`, `mysql`, `sqlite` or `generic`",
                crate::emit::docs::type_string(dialect)
            ),
        ))
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
                    alias: None,
                });
            }
            Segment::Ref(_) => {
                write!(text, "(SELECT {REF}{})", refs.len()).unwrap();
                refs.push(Position {
                    from: false,
                    alias: None,
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
    let returns_rows = match statement {
        Statement::Query(_) => true,
        Statement::Insert(insert) => insert.returning.is_some(),
        Statement::Update(update) => update.returning.is_some(),
        Statement::Delete(delete) => delete.returning.is_some(),
        _ => false,
    };

    let mut analyzer = Analyzer { refs };
    let _ = statement.visit(&mut analyzer);
    Ok(Analysis {
        kind,
        returns_rows,
        refs: analyzer.refs,
    })
}

struct Analyzer {
    refs: Vec<Position>,
}

impl Analyzer {
    fn place(&mut self, index: Option<usize>, alias: Option<&TableAlias>) {
        if let Some(position) = index.and_then(|index| self.refs.get_mut(index)) {
            *position = Position {
                from: true,
                alias: alias.map(|alias| alias.name.value.clone()),
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
            } => self.place(ref_marker(subquery), alias.as_ref()),
            TableFactor::Table { name, alias, .. } => self.place(ref_name(name), alias.as_ref()),
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        if let SetExpr::Select(select) = &*query.body {
            for item in &select.projection {
                if let SelectItem::ExprWithAlias {
                    expr: Expr::Subquery(subquery),
                    alias,
                } = item
                    && let Some(position) =
                        ref_marker(subquery).and_then(|index| self.refs.get_mut(index))
                {
                    position.alias = Some(alias.value.clone());
                }
            }
        }
        ControlFlow::Continue(())
    }
}

fn ref_marker(query: &Query) -> Option<usize> {
    let SetExpr::Select(select) = &*query.body else {
        return None;
    };
    match select.projection.as_slice() {
        [SelectItem::UnnamedExpr(Expr::Identifier(ident))] => marker(ident),
        _ => None,
    }
}

fn ref_name(name: &ObjectName) -> Option<usize> {
    match name.0.as_slice() {
        [ObjectNamePart::Identifier(ident)] => marker(ident),
        _ => None,
    }
}

fn marker(ident: &SqlIdent) -> Option<usize> {
    ident.value.strip_prefix(REF)?.parse().ok()
}
