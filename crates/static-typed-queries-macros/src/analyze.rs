use std::fmt::Write as _;
use std::ops::ControlFlow;

use sqlparser::ast::{
    AssignmentTarget, BinaryOperator, Expr, Function, FunctionArg, FunctionArgExpr,
    FunctionArguments, LimitClause, ObjectName, ObjectNamePart, Query, SelectItem, SetExpr,
    Statement, TableFactor, Visit, Visitor,
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

#[derive(Clone, Copy)]
pub(crate) enum Origin {
    Compared,
    Inserted,
    Assigned,
    Selected,
    Limit,
    Offset,
}

pub(crate) struct Analysis {
    pub kind: Kind,
    pub refs: Vec<Position>,
    pub names: Vec<Option<(String, Origin)>>,
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

    let mut analyzer = Analyzer {
        refs,
        names: vec![None; template.params.len()],
    };
    let _ = statement.visit(&mut analyzer);
    Ok(Analysis {
        kind,
        refs: analyzer.refs,
        names: analyzer.names,
    })
}

struct Analyzer {
    refs: Vec<Position>,
    names: Vec<Option<(String, Origin)>>,
}

impl Analyzer {
    fn name(&mut self, expr: &Expr, origin: Origin, name: Option<String>) {
        if let (Some(slot), Some(name)) = (param_slot(expr), name)
            && let Some(entry @ None) = self.names.get_mut(slot)
        {
            *entry = Some((name, origin));
        }
    }

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

    fn pre_visit_expr(&mut self, expr: &Expr) -> ControlFlow<()> {
        match expr {
            Expr::BinaryOp { left, op, right } if is_comparison(op) => {
                self.name(right, Origin::Compared, column(left));
                self.name(left, Origin::Compared, column(right));
            }
            Expr::Between {
                expr, low, high, ..
            } => {
                self.name(low, Origin::Compared, column(expr));
                self.name(high, Origin::Compared, column(expr));
            }
            Expr::InList { expr, list, .. } => {
                for item in list {
                    self.name(item, Origin::Compared, column(expr));
                }
            }
            Expr::Like { expr, pattern, .. } | Expr::ILike { expr, pattern, .. } => {
                self.name(pattern, Origin::Compared, column(expr));
            }
            Expr::AnyOp { left, right, .. } | Expr::AllOp { left, right, .. } => {
                self.name(right, Origin::Compared, column(left));
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_query(&mut self, query: &Query) -> ControlFlow<()> {
        if let Some(LimitClause::LimitOffset { limit, offset, .. }) = &query.limit_clause {
            if let Some(limit) = limit {
                self.name(limit, Origin::Limit, Some("limit".into()));
            }
            if let Some(offset) = offset {
                self.name(&offset.value, Origin::Offset, Some("offset".into()));
            }
        }
        if let SetExpr::Select(select) = &*query.body {
            for item in &select.projection {
                if let SelectItem::ExprWithAlias { expr, alias } = item {
                    self.name(expr, Origin::Selected, Some(alias.value.clone()));
                }
            }
        }
        ControlFlow::Continue(())
    }

    fn pre_visit_statement(&mut self, statement: &Statement) -> ControlFlow<()> {
        match statement {
            Statement::Insert(insert) => {
                if let Some(source) = &insert.source
                    && let SetExpr::Values(values) = &*source.body
                {
                    for row in &values.rows {
                        for (expr, column) in row.content.iter().zip(&insert.columns) {
                            self.name(expr, Origin::Inserted, object_name(column));
                        }
                    }
                }
            }
            Statement::Update(update) => {
                for assignment in &update.assignments {
                    if let AssignmentTarget::ColumnName(column) = &assignment.target {
                        self.name(&assignment.value, Origin::Assigned, object_name(column));
                    }
                }
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}

fn is_comparison(op: &BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::Eq
            | BinaryOperator::NotEq
            | BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
    )
}

fn unwrap(expr: &Expr) -> &Expr {
    match expr {
        Expr::Nested(inner) | Expr::Cast { expr: inner, .. } => unwrap(inner),
        Expr::Function(function) => single_arg(function).map_or(expr, unwrap),
        _ => expr,
    }
}

fn single_arg(function: &Function) -> Option<&Expr> {
    let FunctionArguments::List(list) = &function.args else {
        return None;
    };
    match list.args.as_slice() {
        [FunctionArg::Unnamed(FunctionArgExpr::Expr(expr))] => Some(expr),
        _ => None,
    }
}

fn param_slot(expr: &Expr) -> Option<usize> {
    match unwrap(expr) {
        Expr::Identifier(ident) => ident.value.strip_prefix(PARAM)?.parse().ok(),
        _ => None,
    }
}

fn column(expr: &Expr) -> Option<String> {
    match unwrap(expr) {
        Expr::Identifier(ident) if !ident.value.starts_with("__stq_") => Some(ident.value.clone()),
        Expr::CompoundIdentifier(parts) => parts.last().map(|ident| ident.value.clone()),
        _ => None,
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

fn object_name(name: &ObjectName) -> Option<String> {
    match name.0.last()? {
        ObjectNamePart::Identifier(ident) => Some(ident.value.clone()),
        _ => None,
    }
}
