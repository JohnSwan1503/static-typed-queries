use sqlparser::parser::Parser;

use crate::dialect::Dialect;
use crate::statement::Statement;

pub fn parse<S: Statement>() {
    let failure = match Parser::parse_sql(<S::Dialect as Dialect>::grammar().as_ref(), S::SQL) {
        Ok(statements) if statements.len() == 1 => return,
        Ok(statements) => format!("{} statements", statements.len()),
        Err(error) => error.to_string(),
    };
    panic!(
        "the SQL of `{}` doesn't parse as {}: {failure}\n  sql: {}",
        S::NODE.name.as_str(),
        <S::Dialect as Dialect>::NAME.as_str(),
        S::SQL
    );
}
