use sqlx::Database;
use sqlx::error::BoxDynError;

use super::bind::Bind;
use super::command::Command;
use crate::dialect::driver::{self, Arguments, Driver};
use crate::hooks::HookValues;
use crate::sql::Sql;
use crate::values::Valueless;

// Path steps index a node's items, its item fields in order; the slot is a field of the node the
// path leads to.
pub trait BindParams<DB: Database> {
    fn bind(&self, path: &[u16], slot: u16, args: &mut DB::Arguments) -> Result<(), BoxDynError>;
}

impl<DB: Database> BindParams<DB> for () {
    fn bind(&self, path: &[u16], slot: u16, _: &mut DB::Arguments) -> Result<(), BoxDynError> {
        Err(unknown(path, slot))
    }
}

impl<DB: Database, T: Valueless> BindParams<DB> for T {
    fn bind(&self, path: &[u16], slot: u16, _: &mut DB::Arguments) -> Result<(), BoxDynError> {
        Err(unknown(path, slot))
    }
}

pub trait BindHooks<DB: Database> {
    fn values(&self, hook: &Command) -> Option<&dyn BindParams<DB>>;
}

impl<DB: Database> BindHooks<DB> for () {
    fn values(&self, _: &Command) -> Option<&dyn BindParams<DB>> {
        None
    }
}

impl<DB: Database, H: Sql + BindParams<DB>, Rest: BindHooks<DB>> BindHooks<DB>
    for (HookValues<H>, Rest)
{
    fn values(&self, hook: &Command) -> Option<&dyn BindParams<DB>> {
        if hook.fingerprint() == H::NODE.fingerprint && hook.name() == H::NODE.name {
            Some(&self.0.0)
        } else {
            self.1.values(hook)
        }
    }
}

pub fn arguments<D, P>(params: &P, binds: &[Bind]) -> Result<Arguments<D>, sqlx::Error>
where
    D: Driver,
    P: BindParams<driver::Database<D>> + ?Sized,
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
