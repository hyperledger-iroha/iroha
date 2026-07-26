//! Canonical arithmetic for Jindo's 255-bit coefficient field.
//!
//! The modulus is
//! `p = 60272^16 + 1 =
//! 0x430d45996b62afc2d65643d9e6fb65558e9630dc8c3732810000000000000001`.
//! Values use one canonical 32-byte little-endian wire encoding and a
//! four-limb Montgomery representation internally.

use core::ops::{Add, Mul, Neg, Sub};

/// Canonical field element in Montgomery form.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct JindoFieldElementV1([u64; 4]);

impl JindoFieldElementV1 {
    /// Field modulus in little-endian 64-bit limbs.
    pub(crate) const MODULUS: [u64; 4] = [
        0x0000_0000_0000_0001,
        0x8e96_30dc_8c37_3281,
        0xd656_43d9_e6fb_6555,
        0x430d_4599_6b62_afc2,
    ];

    /// `R mod p` for `R = 2^256`.
    const MONTGOMERY_R: [u64; 4] = [
        0xffff_ffff_ffff_fffd,
        0x543d_6d6a_5b5a_687c,
        0x7cfd_3472_4b0d_cfff,
        0x36d8_2f33_bdd7_f0b7,
    ];

    /// `R^2 mod p`, used when importing an ordinary residue.
    const MONTGOMERY_R2: [u64; 4] = [
        0xf116_9a2d_c3ac_a563,
        0x3e4e_0dc8_607a_08cd,
        0x3e18_db5d_7300_0020,
        0x34a8_0d03_8d4a_9211,
    ];

    /// `-p^{-1} mod 2^64`.
    const MONTGOMERY_NEG_INV: u64 = 0xffff_ffff_ffff_ffff;

    /// Exponent `p - 2`, in little-endian limbs.
    const INVERSE_EXPONENT: [u64; 4] = [
        0xffff_ffff_ffff_ffff,
        0x8e96_30dc_8c37_3280,
        0xd656_43d9_e6fb_6555,
        0x430d_4599_6b62_afc2,
    ];

    /// Additive identity.
    pub(crate) const ZERO: Self = Self([0; 4]);

    /// Multiplicative identity.
    pub(crate) const ONE: Self = Self(Self::MONTGOMERY_R);

    /// Construct from a small unsigned integer.
    pub(crate) fn from_u64(value: u64) -> Self {
        Self(Self::montgomery_mul_limbs(
            [value, 0, 0, 0],
            Self::MONTGOMERY_R2,
        ))
    }

    /// Decode one canonical 32-byte little-endian residue.
    pub(crate) fn from_canonical_bytes(bytes: [u8; 32]) -> Option<Self> {
        let mut limbs = [0_u64; 4];
        for (limb, chunk) in limbs.iter_mut().zip(bytes.chunks_exact(8)) {
            let mut word = [0_u8; 8];
            word.copy_from_slice(chunk);
            *limb = u64::from_le_bytes(word);
        }
        if !Self::less_than(limbs, Self::MODULUS) {
            return None;
        }
        Some(Self(Self::montgomery_mul_limbs(
            limbs,
            Self::MONTGOMERY_R2,
        )))
    }

    /// Encode as the unique 32-byte little-endian residue in `[0, p)`.
    pub(crate) fn to_canonical_bytes(self) -> [u8; 32] {
        let limbs = Self::montgomery_mul_limbs(self.0, [1, 0, 0, 0]);
        let mut bytes = [0_u8; 32];
        for (chunk, limb) in bytes.chunks_exact_mut(8).zip(limbs) {
            chunk.copy_from_slice(&limb.to_le_bytes());
        }
        bytes
    }

    /// Return true exactly for the additive identity.
    pub(crate) fn is_zero(self) -> bool {
        self == Self::ZERO
    }

    /// Multiplicative inverse, or `None` for zero.
    pub(crate) fn invert(self) -> Option<Self> {
        if self.is_zero() {
            return None;
        }
        let mut accumulator = Self::ONE;
        for limb_index in (0..4).rev() {
            let limb = Self::INVERSE_EXPONENT[limb_index];
            for bit_index in (0..64).rev() {
                accumulator = accumulator * accumulator;
                if ((limb >> bit_index) & 1) == 1 {
                    accumulator = accumulator * self;
                }
            }
        }
        Some(accumulator)
    }

    fn less_than(left: [u64; 4], right: [u64; 4]) -> bool {
        for index in (0..4).rev() {
            if left[index] != right[index] {
                return left[index] < right[index];
            }
        }
        false
    }

    fn add_limbs(left: [u64; 4], right: [u64; 4]) -> ([u64; 4], u64) {
        let mut out = [0_u64; 4];
        let mut carry = 0_u64;
        for index in 0..4 {
            let sum = u128::from(left[index])
                + u128::from(right[index])
                + u128::from(carry);
            out[index] = sum as u64;
            carry = (sum >> 64) as u64;
        }
        (out, carry)
    }

    fn sub_limbs(left: [u64; 4], right: [u64; 4]) -> ([u64; 4], u64) {
        let mut out = [0_u64; 4];
        let mut borrow = 0_u64;
        for index in 0..4 {
            let (partial, borrow_left) = left[index].overflowing_sub(right[index]);
            let (value, borrow_carry) = partial.overflowing_sub(borrow);
            out[index] = value;
            borrow = u64::from(borrow_left || borrow_carry);
        }
        (out, borrow)
    }

    fn reduce_once(value: [u64; 4], high: u64) -> [u64; 4] {
        if high != 0 || !Self::less_than(value, Self::MODULUS) {
            let (reduced, borrow) = Self::sub_limbs(value, Self::MODULUS);
            debug_assert_eq!(borrow, u64::from(high == 0));
            reduced
        } else {
            value
        }
    }

    fn add_with_propagation(words: &mut [u64; 9], index: usize, value: u64) {
        let mut cursor = index;
        let mut carry = value;
        while carry != 0 {
            debug_assert!(cursor < words.len());
            let (sum, overflow) = words[cursor].overflowing_add(carry);
            words[cursor] = sum;
            carry = u64::from(overflow);
            cursor += 1;
        }
    }

    fn montgomery_mul_limbs(left: [u64; 4], right: [u64; 4]) -> [u64; 4] {
        let mut product = [0_u64; 9];

        for left_index in 0..4 {
            let mut carry = 0_u64;
            for right_index in 0..4 {
                let index = left_index + right_index;
                let accumulation = u128::from(left[left_index])
                    * u128::from(right[right_index])
                    + u128::from(product[index])
                    + u128::from(carry);
                product[index] = accumulation as u64;
                carry = (accumulation >> 64) as u64;
            }
            Self::add_with_propagation(&mut product, left_index + 4, carry);
        }

        for offset in 0..4 {
            let multiplier = product[offset].wrapping_mul(Self::MONTGOMERY_NEG_INV);
            let mut carry = 0_u64;
            for modulus_index in 0..4 {
                let index = offset + modulus_index;
                let accumulation = u128::from(multiplier)
                    * u128::from(Self::MODULUS[modulus_index])
                    + u128::from(product[index])
                    + u128::from(carry);
                product[index] = accumulation as u64;
                carry = (accumulation >> 64) as u64;
            }
            Self::add_with_propagation(&mut product, offset + 4, carry);
            debug_assert_eq!(product[offset], 0);
        }

        Self::reduce_once(
            [product[4], product[5], product[6], product[7]],
            product[8],
        )
    }
}

impl Add for JindoFieldElementV1 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let (sum, carry) = Self::add_limbs(self.0, rhs.0);
        Self(Self::reduce_once(sum, carry))
    }
}

impl Sub for JindoFieldElementV1 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        let (difference, borrow) = Self::sub_limbs(self.0, rhs.0);
        if borrow == 0 {
            Self(difference)
        } else {
            let (wrapped, carry) = Self::add_limbs(difference, Self::MODULUS);
            debug_assert_eq!(carry, 1);
            Self(wrapped)
        }
    }
}

impl Mul for JindoFieldElementV1 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(Self::montgomery_mul_limbs(self.0, rhs.0))
    }
}

impl Neg for JindoFieldElementV1 {
    type Output = Self;

    fn neg(self) -> Self::Output {
        if self.is_zero() {
            Self::ZERO
        } else {
            Self(Self::sub_limbs(Self::MODULUS, self.0).0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_from_limbs(limbs: [u64; 4]) -> [u8; 32] {
        let mut bytes = [0_u8; 32];
        for (chunk, limb) in bytes.chunks_exact_mut(8).zip(limbs) {
            chunk.copy_from_slice(&limb.to_le_bytes());
        }
        bytes
    }

    fn decode(limbs: [u64; 4]) -> JindoFieldElementV1 {
        JindoFieldElementV1::from_canonical_bytes(canonical_from_limbs(limbs))
            .expect("canonical test field element")
    }

    #[test]
    fn modulus_matches_the_jindo_friendly_base_relation() {
        let mut value = JindoFieldElementV1::ONE;
        let base = JindoFieldElementV1::from_u64(60_272);
        for _ in 0..16 {
            value = value * base;
        }
        assert_eq!(value + JindoFieldElementV1::ONE, JindoFieldElementV1::ZERO);
    }

    #[test]
    fn canonical_decoder_rejects_modulus_and_larger_values() {
        assert!(
            JindoFieldElementV1::from_canonical_bytes(canonical_from_limbs(
                JindoFieldElementV1::MODULUS
            ))
            .is_none()
        );
        let mut larger = JindoFieldElementV1::MODULUS;
        larger[0] = larger[0].wrapping_add(1);
        assert!(
            JindoFieldElementV1::from_canonical_bytes(canonical_from_limbs(larger)).is_none()
        );
        assert!(
            JindoFieldElementV1::from_canonical_bytes(canonical_from_limbs([0; 4])).is_some()
        );
    }

    #[test]
    fn canonical_roundtrip_covers_limb_boundaries_and_modulus_minus_one() {
        let values = [
            [0, 0, 0, 0],
            [1, 0, 0, 0],
            [u64::MAX, 0, 0, 0],
            [0, 1, 0, 0],
            [0, 0, 1, 0],
            [0, 0, 0, 1],
            [
                0,
                0x8e96_30dc_8c37_3281,
                0xd656_43d9_e6fb_6555,
                0x430d_4599_6b62_afc2,
            ],
        ];
        for limbs in values {
            let encoded = canonical_from_limbs(limbs);
            assert_eq!(decode(limbs).to_canonical_bytes(), encoded);
        }
    }

    #[test]
    fn addition_subtraction_and_negation_cross_the_modulus_boundary() {
        let modulus_minus_one = decode([
            0,
            0x8e96_30dc_8c37_3281,
            0xd656_43d9_e6fb_6555,
            0x430d_4599_6b62_afc2,
        ]);
        assert_eq!(
            modulus_minus_one + JindoFieldElementV1::ONE,
            JindoFieldElementV1::ZERO
        );
        assert_eq!(
            JindoFieldElementV1::ZERO - JindoFieldElementV1::ONE,
            modulus_minus_one
        );
        assert_eq!(-JindoFieldElementV1::ONE, modulus_minus_one);

        for value in [
            JindoFieldElementV1::ZERO,
            JindoFieldElementV1::ONE,
            JindoFieldElementV1::from_u64(u64::MAX),
            modulus_minus_one,
        ] {
            assert_eq!(value + (-value), JindoFieldElementV1::ZERO);
            assert_eq!(value - value, JindoFieldElementV1::ZERO);
        }
    }

    #[test]
    fn multiplication_matches_independent_small_integer_vectors() {
        let vectors = [
            (0_u64, 0_u64, 0_u64),
            (0, u64::MAX, 0),
            (1, u64::MAX, u64::MAX),
            (2, 3, 6),
            (u32::MAX.into(), u32::MAX.into(), 0xffff_fffe_0000_0001),
        ];
        for (left, right, expected) in vectors {
            let product =
                JindoFieldElementV1::from_u64(left) * JindoFieldElementV1::from_u64(right);
            assert_eq!(
                product,
                JindoFieldElementV1::from_u64(expected),
                "{left} * {right}"
            );
        }
    }

    #[test]
    fn inversion_is_total_only_for_nonzero_elements() {
        assert!(JindoFieldElementV1::ZERO.invert().is_none());
        let values = [
            JindoFieldElementV1::ONE,
            JindoFieldElementV1::from_u64(2),
            JindoFieldElementV1::from_u64(60_272),
            decode([
                0,
                0x8e96_30dc_8c37_3281,
                0xd656_43d9_e6fb_6555,
                0x430d_4599_6b62_afc2,
            ]),
        ];
        for value in values {
            let inverse = value.invert().expect("nonzero inverse");
            assert_eq!(value * inverse, JindoFieldElementV1::ONE);
            assert_eq!(inverse * value, JindoFieldElementV1::ONE);
        }
    }

    #[test]
    fn distributivity_vectors_exercise_full_width_carries() {
        let a = decode([
            u64::MAX,
            0x1111_2222_3333_4444,
            0x5555_6666_7777_8888,
            0x1234_5678_9abc_def0,
        ]);
        let b = decode([
            0xdead_beef_cafe_babe,
            0x0102_0304_0506_0708,
            0x1111_1111_1111_1111,
            0x2222_2222_2222_2222,
        ]);
        let c = decode([
            0xffff_ffff_ffff_fffe,
            0x8e96_30dc_8c37_3280,
            0xd656_43d9_e6fb_6555,
            0x430d_4599_6b62_afc2,
        ]);
        assert_eq!(a * (b + c), a * b + a * c);
        assert_eq!((a - b) + b, a);
    }
}
