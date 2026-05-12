//! Short Authentication String generation for first-time pairing.
//!
//! Both sides display the same 6-digit number, derived deterministically
//! from the two long-term public keys plus a per-pairing salt that comes
//! from the TLS exporter. Users compare aloud / by chat. This is the
//! standard SAS construction used by ZRTP, Signal Safety Numbers, etc.,
//! reduced to LAN scale.

use blake3::Hasher;

/// 6-decimal-digit short authentication string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShortAuthString(pub u32);

impl ShortAuthString {
    /// Always renders zero-padded to 6 digits.
    pub fn to_string_padded(&self) -> String {
        format!("{:06}", self.0)
    }
}

impl std::fmt::Display for ShortAuthString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string_padded())
    }
}

/// Derive 6 decimal digits of SAS from two pubkeys plus a binding salt.
///
/// The pubkeys are sorted lexicographically before hashing so peer A
/// and peer B compute the same digest regardless of who initiated.
pub fn sas_digits(pk_a: &[u8; 32], pk_b: &[u8; 32], salt: &[u8]) -> ShortAuthString {
    let (lo, hi) = if pk_a <= pk_b { (pk_a, pk_b) } else { (pk_b, pk_a) };
    let mut h = Hasher::new();
    h.update(b"borderless/sas/v0");
    h.update(lo);
    h.update(hi);
    h.update(salt);
    let digest = h.finalize();
    let mut int = 0u32;
    for b in &digest.as_bytes()[..4] {
        int = int.wrapping_mul(256).wrapping_add(*b as u32);
    }
    ShortAuthString(int % 1_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_in_pubkey_order() {
        let pk_a = [1u8; 32];
        let pk_b = [2u8; 32];
        let salt = b"abc";
        assert_eq!(sas_digits(&pk_a, &pk_b, salt), sas_digits(&pk_b, &pk_a, salt));
    }

    #[test]
    fn salt_changes_digits() {
        let pk_a = [1u8; 32];
        let pk_b = [2u8; 32];
        assert_ne!(sas_digits(&pk_a, &pk_b, b"a"), sas_digits(&pk_a, &pk_b, b"b"));
    }

    #[test]
    fn six_digit_pad() {
        let s = ShortAuthString(42);
        assert_eq!(s.to_string_padded(), "000042");
    }
}
