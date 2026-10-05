use crate::node::fingerprint::Fingerprint;
use crate::node::name::Name;

use super::bind::Bind;

/// One rendered statement, the main one, a hook or a transaction step, with what its parameters
/// bind.
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

    /// The name of the item it renders.
    pub const fn name(&self) -> Name {
        self.name
    }

    #[doc(hidden)]
    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// The SQL.
    pub const fn sql(&self) -> &'static str {
        self.sql
    }

    /// What each parameter of the SQL binds, in order.
    pub const fn binds(&self) -> &'static [Bind] {
        self.binds
    }
}
