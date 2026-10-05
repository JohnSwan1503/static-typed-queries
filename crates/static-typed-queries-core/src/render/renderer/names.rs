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
        name.len() + 1 + digit_count(suffix)
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
    let mut rest = suffix;
    let mut skip = digit_count(suffix) - (i - bytes.len());
    while skip > 0 {
        rest /= 10;
        skip -= 1;
    }
    b'0' + (rest % 10) as u8
}

const fn digit_count(n: u16) -> usize {
    let mut count = 1;
    let mut rest = n / 10;
    while rest > 0 {
        count += 1;
        rest /= 10;
    }
    count
}
