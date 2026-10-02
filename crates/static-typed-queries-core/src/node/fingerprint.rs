#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fingerprint(pub u64);

impl Fingerprint {
    pub(crate) const EMPTY: Fingerprint = Fingerprint(0);

    pub const fn of(text: &str) -> Fingerprint {
        let bytes = text.as_bytes();
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        let mut i = 0;
        while i < bytes.len() {
            hash ^= bytes[i] as u64;
            hash = hash.wrapping_mul(0x0100_0000_01b3);
            i += 1;
        }
        Fingerprint(hash)
    }

    pub const fn combine(self, other: Fingerprint) -> Fingerprint {
        let mut hash = self.0.rotate_left(5) ^ other.0;
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        hash ^= hash >> 33;
        Fingerprint(hash)
    }
}
