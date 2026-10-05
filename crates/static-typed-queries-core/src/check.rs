use sqlparser::parser::Parser;

use crate::dialect::Dialect;
use crate::statement::Statement;

pub fn parse<S: Statement>() {
    let name = S::NODE.name.as_str();
    statement::<S::Dialect>(&format!("`{name}`"), S::SQL);
    for hook in S::BEFORE.iter().chain(S::AFTER) {
        let subject = format!("`{}`, a hook of `{name}`,", hook.name().as_str());
        statement::<S::Dialect>(&subject, hook.sql());
    }
}

fn statement<D: Dialect>(subject: &str, sql: &str) {
    let failure = match Parser::parse_sql(D::grammar().as_ref(), sql) {
        Ok(statements) if statements.len() == 1 => return,
        Ok(statements) => format!("{} statements", statements.len()),
        Err(error) => error.to_string(),
    };
    panic!(
        "the SQL of {subject} doesn't parse as {}: {failure}\n  sql: {sql}",
        D::NAME,
    );
}
