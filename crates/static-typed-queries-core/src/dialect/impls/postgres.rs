use crate::dialect::Dialect;
use crate::dialect::name::Name;
use crate::dialect::params::ParamStyle;
use crate::dialect::quote::Quote;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Postgres;

impl Dialect for Postgres {
    const NAME: Name = Name::POSTGRES;
    const PARAMS: ParamStyle = ParamStyle::dollar_sign(true);
    const QUOTE: Quote = Quote::new(b'"', b'"');
}
