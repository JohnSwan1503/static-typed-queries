pub mod path;
pub mod slot;

use crate::node::Node;
use crate::node::fingerprint::Fingerprint;
use crate::node::name::Name;
use crate::part::param::Param;

use path::Path;
use slot::Slot;

/// What one parameter of a statement binds: a field of an item, reached from the statement through
/// the fields that hold items. It displays as `item.field: Type`.
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

    #[doc(hidden)]
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

    #[doc(hidden)]
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

    /// The name of the item whose field it binds.
    pub const fn item(&self) -> Name {
        self.item
    }

    #[doc(hidden)]
    pub const fn fingerprint(&self) -> Fingerprint {
        self.fingerprint
    }

    /// The fields that lead from the statement to the item.
    pub const fn path(&self) -> Path {
        self.path
    }

    /// Which of the item's fields that hold values it binds.
    pub const fn slot(&self) -> Slot {
        self.slot
    }

    /// The field's name.
    pub const fn field(&self) -> &'static str {
        self.field
    }

    /// The field's type, as written.
    pub const fn ty(&self) -> &'static str {
        self.ty
    }
}

impl core::fmt::Display for Bind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}: {}", self.item.as_str(), self.field, self.ty)
    }
}
