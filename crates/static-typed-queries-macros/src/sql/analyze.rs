use std::fmt::Write as _;
use std::ops::{ControlFlow, Range};

use sqlparser::ast::{
    Expr, Ident as SqlIdent, ObjectName, ObjectNamePart, Query, SelectItem, SetExpr, Statement,
    TableAlias, TableFactor, Visit, Visitor,
};
use sqlparser::dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use sqlparser::parser::{Parser, ParserError};
use syn::{Generics, Ident, LitStr, Type};

use crate::sql::excerpt::point;
use crate::sql::template::{Origin, Segment, Template};

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
    let source = &template.source;
    if source.trim_end().ends_with(';') {
        let at = source.trim_end().chars().count() - 1;
        return Err(error(format!(
            "remove the trailing `;`: templates get embedded in other statements{}",
            point(source, at..at + 1)
        )));
    }

    let (checked, refs) = checked(template);
    let statements = Parser::parse_sql(engine.dialect().as_ref(), &checked.text).map_err(|e| {
        error(format!(
            "invalid SQL for {}: {}",
            engine.name(),
            checked.explain(e, source)
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

// What sqlparser reads: the template with a marker for each placeholder. `from` holds the chars
// of the template behind each char of `text`, and `written` how each marker was written there.
struct Checked {
    text: String,
    from: Vec<Range<usize>>,
    written: Vec<(String, String)>,
}

fn checked(template: &Template) -> (Checked, Vec<Position>) {
    let mut checked = Checked {
        text: String::new(),
        from: Vec::new(),
        written: Vec::new(),
    };
    let mut refs = Vec::new();
    for (segment, origin) in template.segments.iter().zip(&template.origins) {
        let before = checked.text.chars().count();
        match segment {
            Segment::Lit(lit) => checked.text.push_str(lit),
            Segment::Param(slot) => write!(checked.text, "{PARAM}{slot}").unwrap(),
            Segment::Ref(item) if item.target => {
                write!(checked.text, "{REF}{}", refs.len()).unwrap()
            }
            Segment::Ref(_) => write!(checked.text, "(SELECT {REF}{})", refs.len()).unwrap(),
        }
        if let Segment::Ref(item) = segment {
            refs.push(Position {
                from: item.target,
                alias: None,
            });
        }
        let added = checked.text.chars().count() - before;
        match origin {
            Origin::Lit(chars) => checked.from.extend(chars.iter().map(|&c| c..c + 1)),
            Origin::Placeholder(at) => {
                let marker = match segment {
                    Segment::Param(slot) => format!("{PARAM}{slot}"),
                    _ => format!("{REF}{}", refs.len() - 1),
                };
                let written = template
                    .source
                    .chars()
                    .skip(at.start)
                    .take(at.len())
                    .collect();
                checked.written.push((marker, written));
                checked.from.extend(std::iter::repeat_n(at.clone(), added));
            }
        }
    }
    (checked, refs)
}

impl Checked {
    // sqlparser's message, with the markers as the template wrote them, and the template's line
    // where it stopped.
    fn explain(&self, error: ParserError, source: &str) -> String {
        let (message, at) = match error {
            ParserError::ParserError(message) | ParserError::TokenizerError(message) => {
                located(&message)
            }
            other => (other.to_string(), None),
        };
        let mut message = self.rename(message);
        let Some((line, column)) = at else {
            return message;
        };
        let index = self
            .text
            .split('\n')
            .take(line - 1)
            .map(|line| line.chars().count() + 1)
            .sum::<usize>()
            + column
            - 1;
        let at = match self.from.get(index) {
            Some(at) if at.len() > 1 => {
                if let Some(found) = message.rfind(", found: ") {
                    message.truncate(found + ", found: ".len());
                    message.extend(source.chars().skip(at.start).take(at.len()));
                }
                at.clone()
            }
            Some(at) => word(source, at.start),
            None => self.from.last().map_or(0..1, |at| at.end..at.end + 1),
        };
        message + &point(source, at)
    }

    fn rename(&self, mut message: String) -> String {
        let mut written: Vec<&(String, String)> = self.written.iter().collect();
        written.sort_by_key(|(marker, _)| std::cmp::Reverse(marker.len()));
        for (marker, placeholder) in written {
            message = message.replace(marker.as_str(), placeholder);
        }
        message
    }
}

// sqlparser ends its messages with ` at Line: 1, Column: 58`, counted in the text it read.
fn located(message: &str) -> (String, Option<(usize, usize)>) {
    let parsed = message.rsplit_once(" at Line: ").and_then(|(message, at)| {
        let (line, column) = at.split_once(", Column: ")?;
        Some((message, line.parse().ok()?, column.parse().ok()?))
    });
    match parsed {
        Some((message, line, column)) if line > 0 && column > 0 => {
            (message.to_owned(), Some((line, column)))
        }
        _ => (message.to_owned(), None),
    }
}

// The word of the template starting at `start`, or the one char there.
fn word(source: &str, start: usize) -> Range<usize> {
    let is_word = |c: &char| c.is_alphanumeric() || *c == '_';
    let len = source.chars().skip(start).take_while(is_word).count();
    start..start + len.max(1)
}
