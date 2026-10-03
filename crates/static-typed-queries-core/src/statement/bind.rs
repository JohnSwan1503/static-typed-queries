pub mod path;
pub mod slot;

use crate::node::Node;
use crate::node::fingerprint::Fingerprint;
use crate::node::name::Name;
use crate::part::param::Param;

use path::Path;
use slot::Slot;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Bind {
    item: Name,
    fingerprint: Fingerprint,
    path: Path,
    slot: Slot,
    field: &'static str,
    ty: &'static str,
}

impl Bind {
    pub(crate) const EMPTY: Bind = Bind {
        item: Name::EMPTY,
        fingerprint: Fingerprint::EMPTY,
        path: Path::ROOT,
        slot: Slot::EMPTY,
        field: "",
        ty: "",
    };

    pub const fn from_param(node: &'static Node, path: Path, param: Param) -> Bind {
        Bind {
            item: node.name,
            fingerprint: node.fingerprint,
            path,
            slot: Slot::from_param(param),
            field: param.field(),
            ty: param.ty(),
        }
    }

    pub const fn from_native(
        item: &'static str,
        fingerprint: u64,
        path: &[u16],
        slot: u16,
        field: &'static str,
        ty: &'static str,
    ) -> Bind {
        Bind {
            item: Name::new(item),
            fingerprint: Fingerprint(fingerprint),
            path: Path::new(path),
            slot: Slot::new(slot),
            field,
            ty,
        }
    }

    pub const fn item(&self) -> Name {
        self.item
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

    pub const fn field(&self) -> &'static str {
        self.field
    }

    pub const fn ty(&self) -> &'static str {
        self.ty
    }
}

impl core::fmt::Display for Bind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}: {}", self.item.as_str(), self.field, self.ty)
    }
}
