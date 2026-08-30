//! Goldilocks field arithmetic used by the IVM field opcodes.
//!
//! Operations use the prime `p = 2^64 - 2^32 + 1` and reduce every register
//! input canonically so execution remains deterministic across hosts.
/// Goldilocks prime `2^64 - 2^32 + 1`.
const MODULUS: u128 = 0xffff_ffff_0000_0001;
/// Goldilocks prime as `u64`.
const MODULUS_U64: u64 = MODULUS as u64;
/// Reduce any register-sized value to its canonical Goldilocks representative.
///
/// Every `u64` is smaller than twice the Goldilocks modulus, so at most one
/// subtraction is required.
#[inline]
fn canonicalize(value: u64) -> u64 {
    if value >= MODULUS_U64 {
        value - MODULUS_U64
    } else {
        value
    }
}
#[inline]
fn reduce_u128(mut value: u128) -> u64 {
    debug_assert!(value < MODULUS * 2);
    if value >= MODULUS {
        value -= MODULUS;
    }
    value as u64
}
/// Add two field elements modulo the Goldilocks prime.
#[inline]
pub fn add(a: u64, b: u64) -> u64 {
    let sum = u128::from(canonicalize(a)) + u128::from(canonicalize(b));
    reduce_u128(sum)
}
/// Subtract two field elements modulo the Goldilocks prime.
#[inline]
pub fn sub(a: u64, b: u64) -> u64 {
    let a = canonicalize(a);
    let b = canonicalize(b);
    if a >= b {
        a - b
    } else {
        let diff = u128::from(a) + MODULUS - u128::from(b);
        diff as u64
    }
}
/// Multiply two field elements modulo the Goldilocks prime.
#[inline]
pub fn mul(a: u64, b: u64) -> u64 {
    let prod = u128::from(canonicalize(a)) * u128::from(canonicalize(b));
    (prod % MODULUS) as u64
}
/// Compute the multiplicative inverse if the element is non-zero.
pub fn inv(a: u64) -> Option<u64> {
    let a = canonicalize(a);
    if a == 0 {
        return None;
    }
    let mut base = u128::from(a);
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
    fn canonicalize_covers_the_complete_register_range() {
        assert_eq!(canonicalize(0), 0);
        assert_eq!(canonicalize(MODULUS_U64 - 1), MODULUS_U64 - 1);
        assert_eq!(canonicalize(MODULUS_U64), 0);
        assert_eq!(canonicalize(MODULUS_U64 + 1), 1);
        assert_eq!(canonicalize(u64::MAX), u64::from(u32::MAX) - 1);
    }
    #[test]
    fn add_reduces_noncanonical_register_operands() {
        assert_eq!(add(u64::MAX, u64::MAX), 2 * (u64::from(u32::MAX) - 1));
        assert_eq!(add(MODULUS_U64, MODULUS_U64), 0);
    }
    #[test]
    fn sub_wraps_modulus() {
        let a = 1;
        let b = MODULUS_U64 - 1;
        assert_eq!(sub(a, b), 2);
    }
    #[test]
    fn sub_reduces_noncanonical_register_operands() {
        let max_reduced = u64::from(u32::MAX) - 1;
        assert_eq!(sub(u64::MAX, 0), max_reduced);
        assert_eq!(
            sub(0, u64::MAX),
            u64::try_from(MODULUS - u128::from(max_reduced)).expect("field result fits u64")
        );
        assert_eq!(sub(MODULUS_U64, 0), 0);
    }
    #[test]
    fn mul_matches_reference() {
        let a = 123_456_789_101_112_131u64;
        let b = 111_213_141_516_171_819u64;
        let expected = ((a as u128) * (b as u128) % MODULUS) as u64;
        assert_eq!(mul(a, b), expected);
    }
    #[test]
    fn mul_reduces_noncanonical_register_operands() {
        let expected = (u128::from(u64::MAX) * u128::from(u64::MAX) % MODULUS) as u64;
        assert_eq!(mul(u64::MAX, u64::MAX), expected);
        assert_eq!(mul(MODULUS_U64, u64::MAX), 0);
    }
    #[test]
    fn inv_roundtrip() {
        for &value in &[1u64, 5, 12345, MODULUS_U64 - 2] {
            let inv_value = inv(value).expect("invertible");
            assert_eq!(mul(value, inv_value), 1);
        }
        assert!(inv(0).is_none());
        assert!(inv(MODULUS_U64).is_none());
        let max_inverse = inv(u64::MAX).expect("nonzero reduced value is invertible");
        assert_eq!(mul(u64::MAX, max_inverse), 1);
    }
    #[test]
    fn canonical_range() {
        assert!(is_canonical(0));
        assert!(is_canonical(MODULUS_U64 - 1));
        assert!(!is_canonical(MODULUS_U64));
    }
}
