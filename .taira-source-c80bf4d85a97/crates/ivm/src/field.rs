//! Minimal Goldilocks field arithmetic used by the mock cryptographic gadgets.
//!
//! The mock circuits operate on the 64-bit "Goldilocks" prime field with
//! modulus `p = 2^64 - 2^32 + 1`. These helpers keep the operations lightweight
//! and deterministic so tests model the intended arithmetic exactly.

/// Goldilocks prime `2^64 - 2^32 + 1`.
const MODULUS: u128 = 0xffff_ffff_0000_0001;
/// Goldilocks prime as `u64`.
const MODULUS_U64: u64 = MODULUS as u64;

#[inline]
fn reduce_u128(mut value: u128) -> u64 {
    if value >= MODULUS {
        value -= MODULUS;
    }
    value as u64
}

/// Add two field elements modulo the Goldilocks prime.
#[inline]
pub fn add(a: u64, b: u64) -> u64 {
    let sum = (a as u128) + (b as u128);
    reduce_u128(sum)
}

/// Subtract two field elements modulo the Goldilocks prime.
#[inline]
pub fn sub(a: u64, b: u64) -> u64 {
    if a >= b {
        a - b
    } else {
        let diff = (a as u128) + MODULUS - (b as u128);
        diff as u64
    }
}

/// Multiply two field elements modulo the Goldilocks prime.
#[inline]
pub fn mul(a: u64, b: u64) -> u64 {
    let prod = (a as u128) * (b as u128);
    (prod % MODULUS) as u64
}

/// Compute the multiplicative inverse if the element is non-zero.
pub fn inv(a: u64) -> Option<u64> {
    if a == 0 {
        return None;
    }
    let mut base = a as u128;
    let mut exp = MODULUS - 2;
    let mut acc = 1u128;
    while exp > 0 {
        if exp & 1 == 1 {
            acc = (acc * base) % MODULUS;
        }
        base = (base * base) % MODULUS;
        exp >>= 1;
    }
    Some(acc as u64)
}

/// Return `true` if the value is a canonical field representative.
#[inline]
pub fn is_canonical(value: u64) -> bool {
    value < MODULUS_U64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_wraps_modulus() {
        let a = MODULUS_U64 - 1;
        let b = 2;
        assert_eq!(add(a, b), 1);
    }

    #[test]
    fn sub_wraps_modulus() {
        let a = 1;
        let b = MODULUS_U64 - 1;
        assert_eq!(sub(a, b), 2);
    }

    #[test]
    fn mul_matches_reference() {
        let a = 123_456_789_101_112_131u64;
        let b = 111_213_141_516_171_819u64;
        let expected = ((a as u128) * (b as u128) % MODULUS) as u64;
        assert_eq!(mul(a, b), expected);
    }

    #[test]
    fn inv_roundtrip() {
        for &value in &[1u64, 5, 12345, MODULUS_U64 - 2] {
            let inv_value = inv(value).expect("invertible");
            assert_eq!(mul(value, inv_value), 1);
        }
        assert!(inv(0).is_none());
    }

    #[test]
    fn canonical_range() {
        assert!(is_canonical(0));
        assert!(is_canonical(MODULUS_U64 - 1));
        assert!(!is_canonical(MODULUS_U64));
    }
}
