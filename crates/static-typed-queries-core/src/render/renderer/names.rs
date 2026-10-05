use crate::node::Node;

pub(super) const fn same_node(a: &'static Node, b: &'static Node) -> bool {
    a.fingerprint.0 == b.fingerprint.0 && suffixed_eq(a.name.as_str(), 0, b.name.as_str(), 0)
}

pub(super) const fn suffixed_eq(a: &str, a_suffix: u16, b: &str, b_suffix: u16) -> bool {
    let len = suffixed_len(a, a_suffix);
    if len != suffixed_len(b, b_suffix) {
        return false;
    }
    let mut i = 0;
    while i < len {
        if suffixed_byte(a, a_suffix, i) != suffixed_byte(b, b_suffix, i) {
            return false;
        }
        i += 1;
    }
    true
}

const fn suffixed_len(name: &str, suffix: u16) -> usize {
    if suffix == 0 {
        name.len()
    } else {
        name.len() + 1 + digits(suffix).1
    }
}

const fn suffixed_byte(name: &str, suffix: u16, i: usize) -> u8 {
    let bytes = name.as_bytes();
    if i < bytes.len() {
        return bytes[i];
    }
    if i == bytes.len() {
        return b'_';
    }
    digits(suffix).0[i - bytes.len() - 1]
}

// The decimal digits of `n`, most significant first, and how many there are.
pub(super) const fn digits(n: u16) -> ([u8; 5], usize) {
    let len = match n.checked_ilog10() {
        Some(log) => log as usize + 1,
        None => 1,
    };
    let mut digits = [0; 5];
    let (mut rest, mut i) = (n, len);
    while i > 0 {
        i -= 1;
        digits[i] = b'0' + (rest % 10) as u8;
        rest /= 10;
    }
    (digits, len)
}
