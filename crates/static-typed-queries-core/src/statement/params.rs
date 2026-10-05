use sqlx::Database;
use sqlx::error::BoxDynError;

use super::bind::Bind;
use crate::dialect::driver::{self, Arguments, Driver};

// Path steps index a node's distinct referenced items in order of first appearance.
pub trait BindParams<DB: Database> {
    fn bind(&self, path: &[u16], slot: u16, args: &mut DB::Arguments) -> Result<(), BoxDynError>;
}

impl<DB: Database> BindParams<DB> for () {
    fn bind(&self, path: &[u16], slot: u16, _: &mut DB::Arguments) -> Result<(), BoxDynError> {
        Err(unknown(path, slot))
    }
}

pub fn arguments<D, P>(params: &P, binds: &[Bind]) -> Result<Arguments<D>, sqlx::Error>
where
    D: Driver,
    P: BindParams<driver::Database<D>>,
{
    let mut args = Arguments::<D>::default();
    for bind in binds {
        params
            .bind(bind.path().steps(), bind.slot().inner(), &mut args)
            .map_err(sqlx::Error::Encode)?;
    }
    Ok(args)
}

pub fn unknown(path: &[u16], slot: u16) -> BoxDynError {
    format!("no parameter at path {path:?}, slot {slot}").into()
}
