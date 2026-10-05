use crate::node::name::Name;

use super::bind::Bind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hook {
    name: Name,
    sql: &'static str,
    binds: &'static [Bind],
}

impl Hook {
    pub(crate) const fn new(name: Name, sql: &'static str, binds: &'static [Bind]) -> Hook {
        Hook { name, sql, binds }
    }

    pub const fn name(&self) -> Name {
        self.name
    }

    pub const fn sql(&self) -> &'static str {
        self.sql
    }

    pub const fn binds(&self) -> &'static [Bind] {
        self.binds
    }
}
