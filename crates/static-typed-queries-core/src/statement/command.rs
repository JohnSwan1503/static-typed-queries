use crate::node::fingerprint::Fingerprint;
use crate::node::name::Name;

use super::bind::Bind;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Command {
    name: Name,
    fingerprint: Fingerprint,
    sql: &'static str,
    binds: &'static [Bind],
}

impl Command {
    pub(crate) const fn new(
        name: Name,
        fingerprint: Fingerprint,
        sql: &'static str,
        binds: &'static [Bind],
    ) -> Command {
        Command {
            name,
            fingerprint,
            sql,
            binds,
        }
    }

    pub const fn name(&self) -> Name {
        self.name
    }

    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    pub const fn sql(&self) -> &'static str {
        self.sql
    }

    pub const fn binds(&self) -> &'static [Bind] {
        self.binds
    }
}
