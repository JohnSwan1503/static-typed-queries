pub mod name;
pub mod params;
pub mod quote;

pub trait Dialect: 'static {
    const NAME: name::Name;
    const PARAMS: params::ParamStyle;
    const QUOTE: quote::Quote;
}
