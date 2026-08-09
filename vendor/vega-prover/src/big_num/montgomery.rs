// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT

//! Montgomery form operations: limb access and REDC reduction.
//!
//! Implementations for concrete field types are in the provider modules
//! (e.g., `provider::bn254`, `provider::pasta`, `provider::pt256`).

use super::{
    field_reduction_constants::FieldReductionConstants,
    limbs::{add, gte, sub, sub_5_4},
};

/// Trait for field types that expose their internal Montgomery-form limbs.
///
/// Field elements are stored as `value * R mod p` where R = 2^256.
pub trait MontgomeryLimbs: FieldReductionConstants {
    /// Construct a field element from its raw Montgomery-form limbs.
    fn from_limbs(limbs: [u64; 4]) -> Self;
    /// Access the raw Montgomery-form limbs of this field element.
    fn to_limbs(&self) -> &[u64; 4];
}

/// Montgomery REDC for 9-limb input (optimized single-fold algorithm).
///
/// Reduces a 2R-scaled value (sum of field×field products) to 1R-scaled.
/// Input: T representing x*R² (up to 9 limbs)
/// Output: x*R mod p (4 limbs, canonical Montgomery form in [0, p))
///
/// # Algorithm
///
/// 1. **Fold**: Compute `low8 += h * R512_MOD` where h = c[8].
///    Track the carry c ∈ {0,1} from this fold.
///
/// 2. **REDC**: Call `montgomery_reduce_8` which returns canonical [0, p).
///
/// 3. **Carry correction**: If c=1, add R_MOD and do one conditional subtract.
#[inline]
pub(crate) fn montgomery_reduce_9<F: FieldReductionConstants>(c: &[u64; 9]) -> [u64; 4] {
    // STEP 1: Fold 9 limbs to 8 limbs + carry bit
    //
    // Input: C = c[0..8] + c[8]·2^512  (9 limbs representing a value up to ~2^576)
    // Goal:  Compute C mod p, but first reduce to 8 limbs for montgomery_reduce_8.
    //
    // Key insight: 2^512 ≡ R512_MOD (mod p), where R512_MOD = 2^512 mod p < p < 2^256.
    // So: C ≡ c[0..8] + c[8]·R512_MOD (mod p)
    //
    // Since R512_MOD has only 4 limbs, h·R512_MOD has at most 5 limbs.
    // We add this 5-limb value to c[0..8]:
    //   - First 4 limbs: low8[0..4] += h * R512_MOD[0..4] (with carry chain)
    //   - 5th limb (carry from above): propagate through low8[4..8]
    //   - Final carry: fold_carry ∈ {0,1} (bounded because h < 2^64, R512_MOD < 2^256)

    let mut low8 = [c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]];
    let h = c[8];
    let mut fold_carry = 0u64;

    if h != 0 {
        // Fused: low8[0..4] += h * R512_MOD[0..4] with carry chain
        let mut carry = 0u128;
        for (limb, &r512_limb) in low8.iter_mut().zip(F::R512_MOD.iter()) {
            let prod = (h as u128) * (r512_limb as u128) + (*limb as u128) + carry;
            *limb = prod as u64;
            carry = prod >> 64;
        }

        // The 5th limb of h*R512_MOD is `carry`; propagate it through low8[4..8]
        for limb in &mut low8[4..] {
            let sum = (*limb as u128) + carry;
            *limb = sum as u64;
            carry = sum >> 64;
        }

        fold_carry = carry as u64;
    }

    // Proof of bound: h < 2^64, R512_MOD < p < R = 2^256
    // So h * R512_MOD < 2^64 * 2^256 = 2^320
    // And low8 + h * R512_MOD < 2^512 + 2^320 < 2 * 2^512 = 2R²
    // Therefore fold_carry = ⌊result / R²⌋ ∈ {0, 1}
    debug_assert!(
        fold_carry <= 1,
        "fold carry must be 0 or 1, got {}",
        fold_carry
    );

    // STEP 2: Montgomery REDC on 8 limbs → canonical result in [0, p)
    let mut out = montgomery_reduce_8::<F>(&low8);

    // STEP 3: Carry correction
    // If fold_carry == 1, we have an extra R² term that REDC turns into R.
    // In Montgomery form, R mod p = R_MOD. So: out += R_MOD, then canonicalize.
    if fold_carry == 1 {
        let (sum, carry) = add::<4>(&out, &F::R_MOD);
        out = sum;

        // out is now in [0, 2p) (since out was in [0,p) and R_MOD < p).
        // At most one subtract needed.
        if carry == 1 || gte::<4>(&out, &F::MODULUS) {
            out = sub::<4>(&out, &F::MODULUS);
        }
    }

    out
}

/// Montgomery REDC for 8-limb input.
///
/// Input: T[8] limbs representing an integer in [0, R²)
/// Output: canonical 4-limb result in [0, p)
///
/// # Algorithm
///
/// 1. 4 Montgomery elimination rounds with fused multiply+add
/// 2. Extract 5-limb result x5 = [r[4]..r[8]] in [0, R+p)
/// 3. If x5[4] == 1: subtract p once → value now in [0, R)
/// 4. Canonicalize from [0, R) → [0, p) via Q = ⌊R/p⌋ conditional subtracts
///
/// **Key insight**: Standard REDC produces a value in [0, R), NOT [0, p).
/// Since R > p for 256-bit primes, we need Q subtractions to canonicalize.
#[inline]
fn montgomery_reduce_8<F: FieldReductionConstants>(t: &[u64; 8]) -> [u64; 4] {
    let mut r = [t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], 0u64];

    // Montgomery reduction: eliminate low 4 limbs (exactly 4 iterations)
    for i in 0..4 {
        // Compute multiplier q that zeros out r[i]: q ≡ r[i] * (-p⁻¹) mod 2^64
        let q = r[i].wrapping_mul(F::MONT_INV);

        // Fused: r[i..i+5] += q * MODULUS[0..4] with carry chain
        let mut carry = 0u128;
        for j in 0..4 {
            let prod = (q as u128) * (F::MODULUS[j] as u128) + (r[i + j] as u128) + carry;
            r[i + j] = prod as u64;
            carry = prod >> 64;
        }

        // Add carry into r[i+4], then propagate through remaining limbs
        let sum = (r[i + 4] as u128) + carry;
        r[i + 4] = sum as u64;
        carry = sum >> 64;

        // Propagate remaining carry (bounded, rarely needed)
        let mut k = i + 5;
        while carry != 0 && k < 9 {
            let sum = (r[k] as u128) + carry;
            r[k] = sum as u64;
            carry = sum >> 64;
            k += 1;
        }
    }

    // Result is in r[4..9] (5 limbs), in range [0, R+p)
    let mut x5 = [r[4], r[5], r[6], r[7], r[8]];

    // If 5th limb is 1, subtract p once to bring below R
    if x5[4] == 1 {
        x5 = sub_5_4(&x5, &F::MODULUS);
        debug_assert!(x5[4] == 0, "after 5th-limb subtract, x5[4] should be 0");
    }

    // Now x5[0..4] < R, reduce to [0, p) with final correction subtractions
    let mut out = [x5[0], x5[1], x5[2], x5[3]];
    for _ in 0..F::MAX_REDC_SUB_CORRECTIONS {
        if gte::<4>(&out, &F::MODULUS) {
            out = sub::<4>(&out, &F::MODULUS);
        }
    }

    debug_assert!(
        !gte::<4>(&out, &F::MODULUS),
        "REDC final reduction failed after {} subtractions",
        F::MAX_REDC_SUB_CORRECTIONS
    );

    out
}

// =============================================================================
// Test helpers (exported for use by provider test modules)
// =============================================================================

#[cfg(test)]
use super::limbs::mul_4_by_4;
#[cfg(test)]
use ff::PrimeField;

#[cfg(test)]
pub(crate) fn test_r512_mod_impl<F: FieldReductionConstants + MontgomeryLimbs + PrimeField>() {
    let mut wide_512 = [0u64; 9];
    wide_512[8] = 1;
    let reduced = montgomery_reduce_9::<F>(&wide_512);
    let reduced_field = F::from_limbs(reduced);
    assert!(reduced_field != F::ZERO);
}

#[cfg(test)]
pub(crate) fn test_r512_folding_identity_impl<
    F: FieldReductionConstants + MontgomeryLimbs + PrimeField,
>() {
    let base_input: [u64; 9] = [0, 0, 0, 0, 0, 0, 0, 0, 1];
    let base_reduced = F::from_limbs(montgomery_reduce_9::<F>(&base_input));
    for h in [1u64, 2, 0xFF, 0xFFFF_FFFF, 0xFFFF_FFFF_FFFF_FFFF] {
        let mut wide = [0u64; 9];
        wide[8] = h;
        let reduced = F::from_limbs(montgomery_reduce_9::<F>(&wide));
        let expected = F::from(h) * base_reduced;
        assert_eq!(reduced, expected);
    }
}

#[cfg(test)]
pub(crate) fn test_montgomery_round_trip_impl<
    F: FieldReductionConstants + MontgomeryLimbs + PrimeField + Copy,
>() {
    use rand::{SeedableRng, rngs::StdRng};
    let mut rng = StdRng::seed_from_u64(12345);
    for _ in 0..100 {
        let a = F::random(&mut rng);
        let b = F::random(&mut rng);
        let expected = a * b;
        let product_wide = mul_4_by_4(a.to_limbs(), b.to_limbs());
        let mut wide_9 = [0u64; 9];
        wide_9[..8].copy_from_slice(&product_wide);
        let reduced_limbs = montgomery_reduce_9::<F>(&wide_9);
        let result = F::from_limbs(reduced_limbs);
        assert_eq!(result, expected);
    }
}

/// Generate tests for Montgomery reduction.
#[cfg(test)]
#[macro_export]
macro_rules! test_montgomery {
    ($mod_name:ident, $field:ty) => {
        mod $mod_name {
            #[test]
            fn r512_mod() {
                $crate::big_num::montgomery::test_r512_mod_impl::<$field>();
            }
            #[test]
            fn r512_folding_identity() {
                $crate::big_num::montgomery::test_r512_folding_identity_impl::<$field>();
            }
            #[test]
            fn montgomery_round_trip() {
                $crate::big_num::montgomery::test_montgomery_round_trip_impl::<$field>();
            }
        }
    };
}
