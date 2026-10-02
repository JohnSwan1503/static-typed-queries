use sqlx::Database;
use sqlx::error::BoxDynError;

// Path steps index a node's distinct referenced items in order of first appearance.
pub trait BindParams<DB: Database> {
    fn bind(&self, path: &[u16], slot: u16, args: &mut DB::Arguments) -> Result<(), BoxDynError>;
}

impl<DB: Database> BindParams<DB> for () {
    fn bind(&self, path: &[u16], slot: u16, _: &mut DB::Arguments) -> Result<(), BoxDynError> {
        Err(unknown(path, slot))
    }
}

pub fn unknown(path: &[u16], slot: u16) -> BoxDynError {
    format!("no parameter at path {path:?}, slot {slot}").into()
}
