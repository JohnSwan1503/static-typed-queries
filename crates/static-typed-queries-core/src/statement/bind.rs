pub mod path;
pub mod slot;

use crate::node::Node;
use crate::node::fingerprint::Fingerprint;
use crate::node::name::Name;

use path::Path;
use slot::Slot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bind {
    name: Name,
    fingerprint: Fingerprint,
    path: Path,
    slot: Slot,
}

impl Bind {
    pub const fn new(name: Name, fingerprint: Fingerprint, path: Path, slot: Slot) -> Bind {
        Bind {
            name,
            fingerprint,
            path,
            slot,
        }
    }

    pub const fn from_node(node: &'static Node, path: Path, slot: Slot) -> Bind {
        Bind {
            name: node.name,
            fingerprint: node.fingerprint,
            path,
            slot,
        }
    }

    pub const fn from_native(
        name: &'static str,
        fingerprint: u64,
        path: &[u16],
        slot: u16,
    ) -> Bind {
        Bind {
            name: Name::new(name),
            fingerprint: Fingerprint(fingerprint),
            path: Path::new(path),
            slot: Slot::new(slot),
        }
    }

    pub const fn name(&self) -> Name {
        self.name
    }

    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    pub const fn path(&self) -> Path {
        self.path
    }

    pub const fn slot(&self) -> Slot {
        self.slot
    }
}
