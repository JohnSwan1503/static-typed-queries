pub(crate) mod query;
pub(crate) mod sql;
pub(crate) mod statement;
pub(crate) mod table;
pub(crate) mod transaction;
pub(crate) mod wrapper;

use crate::args::Args;

pub(crate) fn no_steps(args: &Args) -> syn::Result<()> {
    match args.steps.iter().flatten().next() {
        Some(step) => Err(syn::Error::new(
            step.span(),
            "only transactions take `steps`",
        )),
        None => Ok(()),
    }
}

pub(crate) fn no_hooks(args: &Args) -> syn::Result<()> {
    match args.before.iter().chain(&args.after).flatten().next() {
        Some(ty) => Err(syn::Error::new_spanned(
            ty,
            "only tables take `before` and `after`",
        )),
        None => Ok(()),
    }
}
