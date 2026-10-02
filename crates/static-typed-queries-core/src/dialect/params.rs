pub mod dollar_sign;
pub mod question_mark;

use dollar_sign::DollarSign;
use question_mark::QuestionMark;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParamStyle {
    DollarSign(DollarSign) = b'$',
    QuestionMark(QuestionMark) = b'?',
}

impl ParamStyle {
    pub const fn dollar_sign(numbered: bool) -> ParamStyle {
        ParamStyle::DollarSign(DollarSign { numbered })
    }

    pub const fn question_mark(numbered: bool) -> ParamStyle {
        ParamStyle::QuestionMark(QuestionMark { numbered })
    }

    pub const fn prefix(self) -> u8 {
        match self {
            Self::DollarSign(..) => b'$',
            Self::QuestionMark(..) => b'?',
        }
    }

    pub const fn numbered(self) -> bool {
        match self {
            Self::DollarSign(DollarSign { numbered })
            | Self::QuestionMark(QuestionMark { numbered }) => numbered,
        }
    }
}
