//! Exact fixed-profile qPCS wire encoding for two canonical RNS coefficients.
//!
//! Every current native modulus is strictly below 2^60. The canonical pair is
//! the 120-bit integer `(c0 << 60) | c1`, encoded in exactly fifteen big-endian
//! bytes. This is a wire representation; the field and FRI algebra are unchanged.

use super::{rns_native_profile::ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1, rns_native_qpcs_prefix::Fq2V1};

/// Exact coefficient width of every modulus in the fixed native RNS profile.
pub(super) const RNS_NATIVE_QPCS_COEFFICIENT_BITS_V1: u32 = 60;
/// Sole encoded width of a pair of canonical field coefficients.
pub(super) const RNS_NATIVE_QPCS_FQ2_BYTES_V1: usize = 15;
const COEFFICIENT_LIMIT_V1: u64 = 1_u64 << RNS_NATIVE_QPCS_COEFFICIENT_BITS_V1;
const COEFFICIENT_MASK_V1: u128 = (COEFFICIENT_LIMIT_V1 - 1) as u128;

/// Invalid exact qPCS pair, modulus coordinate, or field value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RnsNativeQpcsFieldWireErrorV1 {
    /// A coordinate is outside the current forty-prime profile.
    InvalidLimb,
    /// The native profile no longer fits its sole sixty-bit wire representation.
    InvalidProfile,
    /// A pair has a length other than exactly fifteen bytes.
    InvalidLength,
    /// Either coefficient is at least the coordinate's own modulus.
    NonCanonicalResidue,
}

fn modulus_v1(limb: usize) -> Result<u64, RnsNativeQpcsFieldWireErrorV1> {
    let modulus = *ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1
        .get(limb)
        .ok_or(RnsNativeQpcsFieldWireErrorV1::InvalidLimb)?;
    if modulus < 2 || modulus >= COEFFICIENT_LIMIT_V1 {
        return Err(RnsNativeQpcsFieldWireErrorV1::InvalidProfile);
    }
    Ok(modulus)
}

/// Encode one pair without coefficient reduction, padding, or a second layout.
pub(super) fn encode_fq2_v1(
    limb: usize,
    value: Fq2V1,
) -> Result<[u8; RNS_NATIVE_QPCS_FQ2_BYTES_V1], RnsNativeQpcsFieldWireErrorV1> {
    let modulus = modulus_v1(limb)?;
    if value.c0 >= modulus || value.c1 >= modulus {
        return Err(RnsNativeQpcsFieldWireErrorV1::NonCanonicalResidue);
    }
    let packed =
        (u128::from(value.c0) << RNS_NATIVE_QPCS_COEFFICIENT_BITS_V1) | u128::from(value.c1);
    let full = packed.to_be_bytes();
    let mut encoded = [0_u8; RNS_NATIVE_QPCS_FQ2_BYTES_V1];
    // Both validated coefficients have exactly sixty available bits. The
    // omitted high byte is mathematically zero, not truncated proof material.
    encoded.copy_from_slice(&full[1..]);
    Ok(encoded)
}

/// Decode exactly fifteen bytes and check both coefficients against their limb.
pub(super) fn decode_fq2_v1(
    limb: usize,
    bytes: &[u8],
) -> Result<Fq2V1, RnsNativeQpcsFieldWireErrorV1> {
    if bytes.len() != RNS_NATIVE_QPCS_FQ2_BYTES_V1 {
        return Err(RnsNativeQpcsFieldWireErrorV1::InvalidLength);
    }
    let modulus = modulus_v1(limb)?;
    let mut full = [0_u8; 16];
    full[1..].copy_from_slice(bytes);
    let packed = u128::from_be_bytes(full);
    let value = Fq2V1 {
        c0: (packed >> RNS_NATIVE_QPCS_COEFFICIENT_BITS_V1) as u64,
        c1: (packed & COEFFICIENT_MASK_V1) as u64,
    };
    if value.c0 >= modulus || value.c1 >= modulus {
        return Err(RnsNativeQpcsFieldWireErrorV1::NonCanonicalResidue);
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn unchecked_pair(c0: u64, c1: u64) -> [u8; RNS_NATIVE_QPCS_FQ2_BYTES_V1] {
        let full = ((u128::from(c0) << 60) | u128::from(c1)).to_be_bytes();
        full[1..].try_into().expect("one exact 120-bit test value")
    }

    #[test]
    fn all_native_moduli_have_one_injective_pair_encoding() {
        for (limb, modulus) in ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.into_iter().enumerate() {
            assert!(modulus < (1_u64 << 60));
            let mut seen = BTreeSet::new();
            let values = [0, 1, 15, 16, (1_u64 << 59) - 1, 1_u64 << 59, modulus - 1];
            for c0 in values {
                for c1 in values {
                    let value = Fq2V1 { c0, c1 };
                    let encoded = encode_fq2_v1(limb, value).expect("canonical coefficients");
                    assert!(seen.insert(encoded), "limb {limb} pair collision");
                    assert_eq!(decode_fq2_v1(limb, &encoded), Ok(value));
                    assert_eq!(encoded, unchecked_pair(c0, c1));
                }
            }
            for invalid in [
                Fq2V1 { c0: modulus, c1: 0 },
                Fq2V1 { c0: 0, c1: modulus },
                Fq2V1 {
                    c0: modulus,
                    c1: modulus,
                },
                Fq2V1 {
                    c0: u64::MAX,
                    c1: 0,
                },
                Fq2V1 {
                    c0: 0,
                    c1: u64::MAX,
                },
            ] {
                assert_eq!(
                    encode_fq2_v1(limb, invalid),
                    Err(RnsNativeQpcsFieldWireErrorV1::NonCanonicalResidue)
                );
            }
            for (c0, c1) in [(modulus, 0), (0, modulus), (modulus, modulus)] {
                assert_eq!(
                    decode_fq2_v1(limb, &unchecked_pair(c0, c1)),
                    Err(RnsNativeQpcsFieldWireErrorV1::NonCanonicalResidue)
                );
            }
        }
    }

    #[test]
    fn exact_length_rejects_retired_word_pairs_and_every_prefix_or_suffix() {
        for limb in 0..ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.len() {
            for len in 0..=32 {
                if len != RNS_NATIVE_QPCS_FQ2_BYTES_V1 {
                    assert_eq!(
                        decode_fq2_v1(limb, &vec![0; len]),
                        Err(RnsNativeQpcsFieldWireErrorV1::InvalidLength)
                    );
                }
            }
            let value = Fq2V1 { c0: 1, c1: 2 };
            let mut retired = [0_u8; 16];
            retired[..8].copy_from_slice(&value.c0.to_be_bytes());
            retired[8..].copy_from_slice(&value.c1.to_be_bytes());
            assert_eq!(
                decode_fq2_v1(limb, &retired),
                Err(RnsNativeQpcsFieldWireErrorV1::InvalidLength)
            );
            assert_eq!(
                decode_fq2_v1(limb, &encode_fq2_v1(limb, value).unwrap()),
                Ok(value)
            );
        }
        let invalid_limb = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.len();
        assert_eq!(
            encode_fq2_v1(invalid_limb, Fq2V1::ZERO),
            Err(RnsNativeQpcsFieldWireErrorV1::InvalidLimb)
        );
        assert_eq!(
            decode_fq2_v1(invalid_limb, &[0; 15]),
            Err(RnsNativeQpcsFieldWireErrorV1::InvalidLimb)
        );
    }

    #[test]
    fn pair_order_and_sixty_bit_boundary_have_exact_bytes() {
        assert_eq!(
            encode_fq2_v1(0, Fq2V1 { c0: 1, c1: 0 }).unwrap(),
            [0, 0, 0, 0, 0, 0, 0, 0x10, 0, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            encode_fq2_v1(0, Fq2V1 { c0: 0, c1: 1 }).unwrap(),
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]
        );
        assert_ne!(
            encode_fq2_v1(0, Fq2V1 { c0: 1, c1: 2 }),
            encode_fq2_v1(0, Fq2V1 { c0: 2, c1: 1 })
        );
    }

    #[test]
    fn six_lane_initial_multiproof_fits_unchanged_cap_for_actual_maximum_frontier() {
        use super::super::{
            rns_native_profile::{
                ZK_AMS_MKHE_RNS_NATIVE_INITIAL_MULTIPROOF_MAX_BYTES_V1,
                ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1, ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1,
            },
            rns_native_qpcs_prefix::{descriptor_for_indices_v1, query_pair_indices_v1},
        };
        let domain = 1_usize << ZK_AMS_MKHE_RNS_NATIVE_LDE_DOMAIN_LOG2_V1;
        let queries: [u32; ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 as usize] =
            core::array::from_fn(|index| {
                u32::try_from(
                    index * (domain / 2) / usize::from(ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1),
                )
                .expect("fixed native query coordinate")
            });
        let indices = query_pair_indices_v1(&queries, domain).unwrap();
        let descriptor = descriptor_for_indices_v1(indices, domain).unwrap();
        assert_eq!((descriptor.opened, descriptor.authentication), (320, 3392));
        let coordinates = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1.len() * 10;
        let new_leaf_bytes = coordinates * RNS_NATIVE_QPCS_FQ2_BYTES_V1;
        let six_lane_bytes = fastpq_isi::GOLDILOCKS_DIGEST384_BYTES_V1;
        let initial_pair_bytes =
            2 * (descriptor.opened * new_leaf_bytes + descriptor.authentication * six_lane_bytes);
        assert_eq!(initial_pair_bytes, 4_165_632);
        assert!(
            initial_pair_bytes as u64 <= ZK_AMS_MKHE_RNS_NATIVE_INITIAL_MULTIPROOF_MAX_BYTES_V1
        );
        let retired_pair_bytes =
            2 * (descriptor.opened * coordinates * 16 + descriptor.authentication * six_lane_bytes);
        assert!(retired_pair_bytes as u64 > ZK_AMS_MKHE_RNS_NATIVE_INITIAL_MULTIPROOF_MAX_BYTES_V1);
    }
}
