const MAX_LEN: usize = 256;

pub(super) const fn fail(parts: &[&str]) -> ! {
    let mut buf = [0; MAX_LEN];
    let mut len = 0;
    let mut i = 0;
    while i < parts.len() {
        let bytes = parts[i].as_bytes();
        let mut j = 0;
        while j < bytes.len() && len < MAX_LEN {
            buf[len] = bytes[j];
            len += 1;
            j += 1;
        }
        i += 1;
    }
    match core::str::from_utf8(buf.split_at(len).0) {
        Ok(message) => panic!("{}", message),
        Err(_) => panic!("SQL rendering failed (error message was truncated)"),
    }
}
