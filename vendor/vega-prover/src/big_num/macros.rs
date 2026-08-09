// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT

//! Macros and const fn helpers for implementing field traits.
//!
//! This module provides:
//! - `impl_field_reduction_constants!` - implements `FieldReductionConstants` for a field type
//! - `impl_montgomery_limbs!` - implements `MontgomeryLimbs` for a field type
//!
//! All constants are computed at compile time from the field's `PrimeField::MODULUS`
//! and `Field::ONE` values.

use super::limbs::{gte_5_4, mul_4_by_4, reduce_8_mod_4, sub_5_4};

// =============================================================================
// Implementation macros
// =============================================================================

/// Implement `FieldReductionConstants` for a field type.
///
/// This macro automatically computes all constants at compile time from the
/// field's `PrimeField::MODULUS` hex string and `Field::ONE` value.
///
/// # Example
/// ```ignore
/// crate::impl_field_reduction_constants!(Fp);
/// ```
#[macro_export]
macro_rules! impl_field_reduction_constants {
    ($field:ty) => {
        impl $crate::big_num::FieldReductionConstants for $field {
            const MODULUS: [u64; 4] =
                $crate::big_num::macros::parse_hex_to_limbs(<$field as ff::PrimeField>::MODULUS);
            const R_MOD: [u64; 4] = <$field as ff::Field>::ONE.0;
            const MONT_INV: u64 = $crate::big_num::macros::compute_mont_inv(Self::MODULUS[0]);
            const R512_MOD: [u64; 4] =
                $crate::big_num::macros::compute_r512_mod(Self::MODULUS, Self::R_MOD);
            const MAX_REDC_SUB_CORRECTIONS: usize =
                $crate::big_num::macros::compute_max_redc_sub_corrections(Self::MODULUS);
        }

        // Enforce small MAX_REDC_SUB_CORRECTIONS for performance.
        // The final reduction loop in montgomery_reduce_8 iterates this many times.
        // For 254-256 bit primes, this should be 1-4. Values > 8 indicate a problem.
        const _: () = assert!(
            <$field as $crate::big_num::FieldReductionConstants>::MAX_REDC_SUB_CORRECTIONS <= 8,
            "MAX_REDC_SUB_CORRECTIONS too large for efficient reduction"
        );
    };
}

/// Implement `MontgomeryLimbs` for a field type with `.0` containing `[u64; 4]`.
///
/// # Example
/// ```ignore
/// crate::impl_montgomery_limbs!(Fp);
/// ```
#[macro_export]
macro_rules! impl_montgomery_limbs {
    ($field:ty) => {
        impl $crate::big_num::montgomery::MontgomeryLimbs for $field {
            #[inline]
            fn from_limbs(limbs: [u64; 4]) -> Self {
                Self(limbs)
            }

            #[inline]
            fn to_limbs(&self) -> &[u64; 4] {
                &self.0
            }
        }
    };
}

// =============================================================================
// Const fn helpers for compile-time computation of field reduction constants
// =============================================================================

/// Parse a single hex character to its value (0-15).
const fn hex_char_to_nibble(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => panic!("invalid hex character"),
    }
}

/// Parse a 64-char or 66-char (with "0x" prefix) hex string to 4 little-endian u64 limbs.
///
/// The input string should be exactly 64 hex characters (or 66 with "0x" prefix)
/// representing a 256-bit number in big-endian order.
/// The output is 4 u64 limbs in little-endian order.
pub const fn parse_hex_to_limbs(s: &str) -> [u64; 4] {
    let bytes = s.as_bytes();

    // Handle optional "0x" prefix
    let (hex_start, hex_len) =
        if bytes.len() >= 2 && bytes[0] == b'0' && (bytes[1] == b'x' || bytes[1] == b'X') {
            (2, bytes.len() - 2)
        } else {
            (0, bytes.len())
        };

    assert!(
        hex_len == 64,
        "hex string must be 64 characters (excluding optional 0x prefix)"
    );

    let mut limbs = [0u64; 4];
    let mut i = 0;
    while i < 4 {
        // Each limb is 16 hex chars, big-endian in string
        // limbs[3] is most significant, comes first in string
        let limb_idx = 3 - i;
        let str_offset = hex_start + i * 16;

        let mut limb = 0u64;
        let mut j = 0;
        while j < 16 {
            let nibble = hex_char_to_nibble(bytes[str_offset + j]);
            limb = (limb << 4) | (nibble as u64);
            j += 1;
        }
        limbs[limb_idx] = limb;
        i += 1;
    }
    limbs
}

/// Compute -p[0]^(-1) mod 2^64 using a bit-doubling iteration.
///
/// **Why a 64-bit inverse, not 256-bit?**
/// We use limb-by-limb Montgomery reduction, zeroing one 64-bit limb per iteration.
/// Each step computes `m = t_low * mont_inv mod 2^64`, which only depends on
/// `p mod 2^64` (i.e., `p[0]`). This approach maps efficiently to 64-bit CPU
/// registers and avoids full 256×256-bit multiplications that a single-step
/// reduction with `-p^(-1) mod 2^256` would require.
///
/// **The iteration:** `inv' = inv * (2 - p0 * inv)` doubles correct bits.
///
/// Why? If `p0 * inv = 1 + e·2^k` (correct mod 2^k), then:
///   p0 * inv' = (1 + e·2^k)(1 - e·2^k) = 1 - e²·2^(2k) ≡ 1 (mod 2^(2k))
///
/// **Convergence:**
/// ```text
/// Start: p0 is odd, so p0·1 ≡ 1 (mod 2) →  1 bit
/// ×2 each iteration: 1 → 2 → 4 → 8 → 16 → 32 → 64 bits (6 iterations)
/// ```
///
/// Returns `-inv` for use in REDC.
pub const fn compute_mont_inv(p0: u64) -> u64 {
    // Bit-doubling iteration: inv = inv * (2 - p0 * inv)
    // Each iteration doubles correct bits: 1 → 2 → 4 → 8 → 16 → 32 → 64
    let mut inv = 1u64;
    inv = inv.wrapping_mul(2u64.wrapping_sub(p0.wrapping_mul(inv)));
    inv = inv.wrapping_mul(2u64.wrapping_sub(p0.wrapping_mul(inv)));
    inv = inv.wrapping_mul(2u64.wrapping_sub(p0.wrapping_mul(inv)));
    inv = inv.wrapping_mul(2u64.wrapping_sub(p0.wrapping_mul(inv)));
    inv = inv.wrapping_mul(2u64.wrapping_sub(p0.wrapping_mul(inv)));
    inv = inv.wrapping_mul(2u64.wrapping_sub(p0.wrapping_mul(inv)));
    inv.wrapping_neg()
}

/// Compute ⌊R/p⌋ where R = 2^256, by counting subtractions.
///
/// This is the maximum number of final subtractions needed in Montgomery REDC
/// to reduce a value from [0, R) into the canonical range [0, p).
pub const fn compute_max_redc_sub_corrections(p: [u64; 4]) -> usize {
    // Start with a = 2^256 represented as a 5-limb number
    // 2^256 = [0, 0, 0, 0, 1] in little-endian 5-limb representation
    let mut a = [0u64, 0, 0, 0, 1u64];
    let mut count = 0usize;

    // Count how many times we can subtract p from a
    while gte_5_4(&a, &p) {
        a = sub_5_4(&a, &p);
        count += 1;
    }

    count
}

/// Compute 2^512 mod p = R_MOD² mod p.
///
/// This computes R512_MOD = 2^512 mod p by squaring R_MOD (which is 2^256 mod p)
/// and then reducing the result modulo p.
pub const fn compute_r512_mod(p: [u64; 4], r_mod: [u64; 4]) -> [u64; 4] {
    // R512_MOD = R_MOD * R_MOD mod p = (2^256 mod p)² mod p = 2^512 mod p
    let r_squared = mul_4_by_4(&r_mod, &r_mod);
    reduce_8_mod_4(&r_squared, &p)
}
