// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT

//! Field-specific constants for Montgomery reduction.

/// Trait providing precomputed constants for efficient modular reduction.
///
/// When reducing a wide integer (more than 4 limbs = 256 bits) modulo a prime p,
/// we need to handle "overflow limbs" that represent values >= 2^256.
pub trait FieldReductionConstants {
    /// The 4-limb prime modulus p (little-endian, 256 bits)
    const MODULUS: [u64; 4];

    /// R² mod p where R = 2^256 (equivalently, 2^512 mod p).
    ///
    /// Used in `montgomery_reduce_9` to fold a 9-limb accumulator to 8 limbs:
    /// the 9th limb h represents h·2^512, which we replace with h·R512_MOD ≡ h·2^512 (mod p).
    const R512_MOD: [u64; 4];

    /// Montgomery inverse: -p[0]^(-1) mod 2^64
    ///
    /// Only a 64-bit inverse (not 256-bit) because we use limb-by-limb Montgomery
    /// reduction. Each iteration computes `m = t_low * MONT_INV mod 2^64` to zero
    /// out the lowest limb—this only depends on `p mod 2^64` (i.e., `p[0]`).
    /// See [`compute_mont_inv`](crate::big_num::macros::compute_mont_inv) for details.
    const MONT_INV: u64;

    /// R mod p = 2^256 mod p (Montgomery representation of 1)
    /// Used for carry correction after folding: if carry c=1, add R_MOD.
    const R_MOD: [u64; 4];

    /// Maximum correction subtractions needed in Montgomery REDC: ⌊R/p⌋.
    ///
    /// Standard REDC produces results in [0, R) where R = 2^256. Since R > p
    /// for 256-bit primes, we need up to ⌊R/p⌋ final subtractions to reduce
    /// the result into the canonical range [0, p).
    const MAX_REDC_SUB_CORRECTIONS: usize;
}

// =============================================================================
// Test helpers (exported for use by provider test modules)
// =============================================================================

#[cfg(test)]
use super::montgomery::MontgomeryLimbs;
#[cfg(test)]
use ff::PrimeField;
#[cfg(test)]
use num_bigint::BigUint;

#[cfg(test)]
fn limbs_to_biguint(limbs: &[u64; 4]) -> BigUint {
    let mut bytes = [0u8; 32];
    for (i, limb) in limbs.iter().enumerate() {
        bytes[i * 8..(i + 1) * 8].copy_from_slice(&limb.to_le_bytes());
    }
    BigUint::from_bytes_le(&bytes)
}

#[cfg(test)]
pub(crate) fn test_modulus_impl<F: FieldReductionConstants + PrimeField>() {
    let neg_one = -F::ONE;
    let neg_one_limbs = neg_one.to_repr();
    let mut modulus_minus_one = <F as FieldReductionConstants>::MODULUS;
    let (new_val, borrow) = modulus_minus_one[0].overflowing_sub(1);
    modulus_minus_one[0] = new_val;
    assert!(!borrow, "MODULUS should be > 1");
    let mut modulus_bytes = [0u8; 32];
    for (i, limb) in modulus_minus_one.iter().enumerate() {
        modulus_bytes[i * 8..(i + 1) * 8].copy_from_slice(&limb.to_le_bytes());
    }
    assert_eq!(neg_one_limbs.as_ref(), &modulus_bytes[..]);
}

#[cfg(test)]
pub(crate) fn test_modulus_bit_length_impl<F: FieldReductionConstants>() {
    let p = limbs_to_biguint(&F::MODULUS);
    assert!(p.bits() >= 254 && p.bits() <= 256);
}

#[cfg(test)]
pub(crate) fn test_mont_inv_impl<F: FieldReductionConstants>() {
    let product = F::MODULUS[0].wrapping_mul(F::MONT_INV);
    assert_eq!(product, u64::MAX);
}

#[cfg(test)]
pub(crate) fn test_r_mod_impl<F: FieldReductionConstants + MontgomeryLimbs + PrimeField>() {
    let one = F::ONE;
    let one_limbs = one.to_limbs();
    assert_eq!(<F as FieldReductionConstants>::R_MOD, *one_limbs);
}

#[cfg(test)]
pub(crate) fn test_r512_mod_direct_impl<F: FieldReductionConstants>() {
    let p = limbs_to_biguint(&F::MODULUS);
    let two_pow_512 = BigUint::from(1u64) << 512;
    let expected = &two_pow_512 % &p;
    let actual = limbs_to_biguint(&F::R512_MOD);
    assert_eq!(actual, expected);
}

#[cfg(test)]
pub(crate) fn test_max_redc_sub_corrections_impl<F: FieldReductionConstants>() {
    let p = limbs_to_biguint(&F::MODULUS);
    let r = BigUint::from(1u64) << 256;
    let expected = &r / &p;
    assert_eq!(BigUint::from(F::MAX_REDC_SUB_CORRECTIONS as u64), expected);
}

/// Generate tests for `FieldReductionConstants` implementation.
#[cfg(test)]
#[macro_export]
macro_rules! test_field_reduction_constants {
    ($mod_name:ident, $field:ty) => {
        mod $mod_name {
            #[test]
            fn modulus() {
                $crate::big_num::field_reduction_constants::test_modulus_impl::<$field>();
            }
            #[test]
            fn modulus_bit_length() {
                $crate::big_num::field_reduction_constants::test_modulus_bit_length_impl::<$field>(
                );
            }
            #[test]
            fn mont_inv() {
                $crate::big_num::field_reduction_constants::test_mont_inv_impl::<$field>();
            }
            #[test]
            fn r_mod() {
                $crate::big_num::field_reduction_constants::test_r_mod_impl::<$field>();
            }
            #[test]
            fn r512_mod_direct() {
                $crate::big_num::field_reduction_constants::test_r512_mod_direct_impl::<$field>();
            }
            #[test]
            fn max_redc_sub_corrections() {
                $crate::big_num::field_reduction_constants::test_max_redc_sub_corrections_impl::<
                    $field,
                >();
            }
        }
    };
}
