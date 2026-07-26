//! CELPC coefficient encoding used by the fixed Jindo profile.
//!
//! Sixteen coefficient-field values are interleaved across the 256
//! application-ring coefficients.  For slot `i`, coefficients at
//! `i + 16*j` are the 16 base-60272 digits.  Evaluation at
//! `X^16 = 60272` recovers the slot modulo `p = 60272^16 + 1`.

use super::{
    JINDO_ENCODING_BASE_V1, JINDO_ENCODING_EXPONENT_V1, JINDO_ENCODING_SLOTS_V1,
    JINDO_RING_DEGREE_V1,
    field::JindoFieldElementV1,
    ring::{JINDO_INNER_MODULI_V1, JindoRnsPolynomialV1},
};

/// Deterministically encode at most sixteen coefficient-field values.
pub(crate) fn encode_coefficient_slots_v1(
    values: &[JindoFieldElementV1],
) -> Option<JindoRnsPolynomialV1> {
    if values.len() > JINDO_ENCODING_SLOTS_V1 {
        return None;
    }

    let mut coefficients = [0_i128; JINDO_RING_DEGREE_V1];
    for (slot, value) in values.iter().copied().enumerate() {
        let mut limbs = value.to_canonical_limbs();
        for digit in 0..(JINDO_ENCODING_EXPONENT_V1 - 1) {
            let remainder = div_rem_small(&mut limbs, JINDO_ENCODING_BASE_V1);
            coefficients[digit * JINDO_ENCODING_SLOTS_V1 + slot] = i128::from(remainder);
        }
        debug_assert_eq!(limbs[1..], [0; 3]);
        debug_assert!(limbs[0] <= JINDO_ENCODING_BASE_V1);
        coefficients[(JINDO_ENCODING_EXPONENT_V1 - 1) * JINDO_ENCODING_SLOTS_V1 + slot] =
            i128::from(limbs[0]);
    }
    Some(JindoRnsPolynomialV1::from_balanced_coefficients(
        coefficients,
        JINDO_INNER_MODULI_V1,
    ))
}

/// Decode all sixteen slots through the Jindo ring homomorphism.
pub(crate) fn decode_coefficient_slots_v1(
    polynomial: &JindoRnsPolynomialV1,
) -> [JindoFieldElementV1; JINDO_ENCODING_SLOTS_V1] {
    let mut values = [JindoFieldElementV1::ZERO; JINDO_ENCODING_SLOTS_V1];
    let base = JindoFieldElementV1::from_u64(JINDO_ENCODING_BASE_V1);
    for slot in 0..JINDO_ENCODING_SLOTS_V1 {
        let mut value = JindoFieldElementV1::ZERO;
        for digit in (0..JINDO_ENCODING_EXPONENT_V1).rev() {
            let coefficient = polynomial.balanced_coefficient(
                digit * JINDO_ENCODING_SLOTS_V1 + slot,
                JINDO_INNER_MODULI_V1,
            );
            value = value * base + JindoFieldElementV1::from_i128(coefficient);
        }
        values[slot] = value;
    }
    values
}

fn div_rem_small(limbs: &mut [u64; 4], divisor: u64) -> u64 {
    let mut remainder = 0_u128;
    for limb in limbs.iter_mut().rev() {
        let numerator = (remainder << 64) | u128::from(*limb);
        *limb = (numerator / u128::from(divisor)) as u64;
        remainder = numerator % u128::from(divisor);
    }
    remainder as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(bytes: [u8; 32]) -> JindoFieldElementV1 {
        JindoFieldElementV1::from_canonical_bytes(bytes).expect("canonical field value")
    }

    #[test]
    fn base_division_reconstructs_full_width_values() {
        let original = [
            0xffff_ffff_ffff_ffff,
            0x8e96_30dc_8c37_3280,
            0xd656_43d9_e6fb_6555,
            0x430d_4599_6b62_afc2,
        ];
        let mut quotient = original;
        let mut digits = [0_u64; JINDO_ENCODING_EXPONENT_V1];
        for digit in &mut digits[..JINDO_ENCODING_EXPONENT_V1 - 1] {
            *digit = div_rem_small(&mut quotient, JINDO_ENCODING_BASE_V1);
            assert!(*digit < JINDO_ENCODING_BASE_V1);
        }
        assert_eq!(quotient[1..], [0; 3]);
        digits[JINDO_ENCODING_EXPONENT_V1 - 1] = quotient[0];
        assert!(digits[JINDO_ENCODING_EXPONENT_V1 - 1] <= JINDO_ENCODING_BASE_V1);

        let mut reconstructed = JindoFieldElementV1::ZERO;
        let base = JindoFieldElementV1::from_u64(JINDO_ENCODING_BASE_V1);
        for digit in digits.into_iter().rev() {
            reconstructed = reconstructed * base + JindoFieldElementV1::from_u64(digit);
        }
        assert_eq!(
            reconstructed,
            JindoFieldElementV1::from_canonical_bytes({
                let mut bytes = [0_u8; 32];
                for (chunk, limb) in bytes.chunks_exact_mut(8).zip(original) {
                    chunk.copy_from_slice(&limb.to_le_bytes());
                }
                bytes
            })
            .expect("modulus minus two")
        );
    }

    #[test]
    fn deterministic_encoding_roundtrips_every_slot_and_boundary_value() {
        let values = [
            JindoFieldElementV1::ZERO,
            JindoFieldElementV1::ONE,
            JindoFieldElementV1::from_u64(JINDO_ENCODING_BASE_V1 - 1),
            JindoFieldElementV1::from_u64(JINDO_ENCODING_BASE_V1),
            JindoFieldElementV1::from_u64(u64::MAX),
            field([
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x80, 0x32, 0x37, 0x8c, 0xdc, 0x30,
                0x96, 0x8e, 0x55, 0x65, 0xfb, 0xe6, 0xd9, 0x43, 0x56, 0xd6, 0xc2, 0xaf, 0x62, 0x6b,
                0x99, 0x45, 0x0d, 0x43,
            ]),
            field([
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x81, 0x32, 0x37, 0x8c, 0xdc, 0x30,
                0x96, 0x8e, 0x55, 0x65, 0xfb, 0xe6, 0xd9, 0x43, 0x56, 0xd6, 0xc2, 0xaf, 0x62, 0x6b,
                0x99, 0x45, 0x0d, 0x43,
            ]),
            field([
                0xbe, 0xba, 0xfe, 0xca, 0xef, 0xbe, 0xad, 0xde, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03,
                0x02, 0x01, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x22, 0x22, 0x22, 0x22,
                0x22, 0x22, 0x22, 0x22,
            ]),
            JindoFieldElementV1::from_u64(7),
            JindoFieldElementV1::from_u64(8),
            JindoFieldElementV1::from_u64(9),
            JindoFieldElementV1::from_u64(10),
            JindoFieldElementV1::from_u64(11),
            JindoFieldElementV1::from_u64(12),
            JindoFieldElementV1::from_u64(13),
            JindoFieldElementV1::from_u64(14),
        ];
        let encoded = encode_coefficient_slots_v1(&values).expect("sixteen values");
        assert_eq!(decode_coefficient_slots_v1(&encoded), values);
    }

    #[test]
    fn encoder_rejects_more_than_sixteen_slots() {
        assert!(
            encode_coefficient_slots_v1(&[JindoFieldElementV1::ZERO; JINDO_ENCODING_SLOTS_V1 + 1])
                .is_none()
        );
    }

    #[test]
    fn decoding_is_additively_homomorphic() {
        let left_values: [JindoFieldElementV1; JINDO_ENCODING_SLOTS_V1] =
            core::array::from_fn(|index| JindoFieldElementV1::from_u64(index as u64 * 17 + 1));
        let right_values: [JindoFieldElementV1; JINDO_ENCODING_SLOTS_V1] =
            core::array::from_fn(|index| JindoFieldElementV1::from_u64(index as u64 * 31 + 2));
        let mut sum =
            encode_coefficient_slots_v1(&left_values).expect("left deterministic encoding");
        sum.add_assign(
            &encode_coefficient_slots_v1(&right_values).expect("right deterministic encoding"),
            JINDO_INNER_MODULI_V1,
        );
        let expected: [JindoFieldElementV1; JINDO_ENCODING_SLOTS_V1] =
            core::array::from_fn(|index| left_values[index] + right_values[index]);
        assert_eq!(decode_coefficient_slots_v1(&sum), expected);
    }
}
