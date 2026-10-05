use super::Part;
use super::target::Target;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Expr(Target);

impl Expr {
    pub const fn part(target: Target) -> Part {
        Part::Expr(Expr(target))
    }

    pub const fn target(&self) -> Target {
        self.0
    }
}
