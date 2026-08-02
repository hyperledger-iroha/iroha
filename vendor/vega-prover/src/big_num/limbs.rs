// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT

//! Wide integers and limb operations for delayed modular reduction.
//!
//! # Why We Built WideLimbs Instead of Using Existing Libraries
//!
//! ## The Problem
//!
//! When summing many field×field products `Σ (a_i × b_i)`, the standard approach
//! reduces each product immediately. Montgomery REDC is expensive (~100+ cycles),
//! so N products = N REDCs. Delayed reduction accumulates unreduced 512-bit products
//! in a 576-bit accumulator, reducing only once at the end: N products = 1 REDC.
//!
//! ## Why Not num-bigint?
//!
//! `num-bigint` uses heap-allocated `Vec<u64>` for limbs. In tight cryptographic
//! loops (sumcheck has millions of iterations), heap allocation adds ~30-45%
//! overhead vs stack allocation. We need stack-allocated fixed-size arrays.
//!
//! ## Why Not ff/PrimeField?
//!
//! The `ff` crate's `PrimeField` trait intentionally hides internal representation.
//! Every operation returns a **reduced** 4-limb result:
//!
//! ```text
//! trait Field {
//!     fn mul(self, rhs: Self) -> Self;  // Always returns reduced result
//!     // No: fn mul_wide(self, rhs: Self) -> [u64; 8];
//! }
//! ```
//!
//! There's no "unreduced product" type because it breaks the mathematical abstraction.
//! We need access to raw 8-limb products before reduction.
//!
//! ## Why Not crypto-bigint?
//!
//! `crypto-bigint` provides stack-allocated `Uint<N>`, but:
//!
//! 1. **Type system mismatch**: halo2curves fields are already in Montgomery form
//!    with their own constants. crypto-bigint's `MontyForm` manages its own
//!    Montgomery representation - we can't mix them without conversion overhead.
//!
//! 2. **No custom REDC**: We need `montgomery_reduce_9` that uses halo2curves-specific
//!    constants (MODULUS, MONT_INV, R512_MOD). crypto-bigint's REDC uses its own params.
//!
//! 3. **Overhead for simplicity**: Our `WideLimbs<N>` is ~30 lines. We'd add a full
//!    dependency, type conversions, and still need to write our own REDC.
//!
//! ## What WideLimbs Provides
//!
//! A minimal stack-allocated accumulator with public limbs for direct manipulation:
//!
//! ```text
//! pub struct WideLimbs<const N: usize>(pub [u64; N]);
//!                                      ^^^
//!                                      Direct limb access for carry chains
//! ```
//!
//! Used as `WideLimbs<9>` (576 bits) to accumulate up to 2^64 field×field products
//! before a single Montgomery reduction back to a 4-limb field element.

use std::ops::AddAssign;

/// Stack-allocated wide integer with N 64-bit limbs.
///
/// Limbs are stored in little-endian order: `limbs[0]` is the least significant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WideLimbs<const N: usize>(pub [u64; N]);

impl<const N: usize> Default for WideLimbs<N> {
    fn default() -> Self {
        Self([0u64; N])
    }
}

impl<const N: usize> AddAssign for WideLimbs<N> {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        let mut carry = 0u64;
        for i in 0..N {
            let (sum, c1) = self.0[i].overflowing_add(other.0[i]);
            let (sum, c2) = sum.overflowing_add(carry);
            self.0[i] = sum;
            carry = (c1 as u64) + (c2 as u64);
        }
        debug_assert!(carry == 0, "WideLimbs<{N}> overflow: carry {carry} dropped");
    }
}

// =============================================================================
// Generic N-limb operations
// =============================================================================

/// Check if N-limb value a >= N-limb value b.
#[inline(always)]
pub(super) const fn gte<const N: usize>(a: &[u64; N], b: &[u64; N]) -> bool {
    let mut i = N;
    while i > 0 {
        i -= 1;
        if a[i] > b[i] {
            return true;
        }
        if a[i] < b[i] {
            return false;
        }
    }
    true // equal
}

/// Add two N-limb values, returning N limbs + carry (0 or 1).
#[inline(always)]
pub(super) const fn add<const N: usize>(a: &[u64; N], b: &[u64; N]) -> ([u64; N], u64) {
    let mut result = [0u64; N];
    let mut carry = 0u128;
    let mut i = 0;
    while i < N {
        let sum = (a[i] as u128) + (b[i] as u128) + carry;
        result[i] = sum as u64;
        carry = sum >> 64;
        i += 1;
    }
    (result, carry as u64)
}

/// Subtract N-limb b from N-limb a: a - b (assumes a >= b).
#[inline(always)]
pub(super) const fn sub<const N: usize>(a: &[u64; N], b: &[u64; N]) -> [u64; N] {
    let mut result = [0u64; N];
    let mut borrow = 0u64;
    let mut i = 0;
    while i < N {
        let (diff, b1) = a[i].overflowing_sub(b[i]);
        let (diff2, b2) = diff.overflowing_sub(borrow);
        result[i] = diff2;
        borrow = (b1 as u64) + (b2 as u64);
        i += 1;
    }
    result
}

/// Shift an N-limb value left by one bit.
#[inline(always)]
pub(super) const fn shl<const N: usize>(a: &[u64; N]) -> [u64; N] {
    let mut result = [0u64; N];
    let mut carry = 0u64;
    let mut i = 0;
    while i < N {
        let new_carry = a[i] >> 63;
        result[i] = (a[i] << 1) | carry;
        carry = new_carry;
        i += 1;
    }
    result
}

/// Count leading zeros in N-limb value.
#[inline(always)]
pub(super) const fn clz<const N: usize>(a: &[u64; N]) -> u32 {
    let mut i = N;
    let mut count = 0u32;
    while i > 0 {
        i -= 1;
        if a[i] != 0 {
            return count + a[i].leading_zeros();
        }
        count += 64;
    }
    count
}

// =============================================================================
// 4-limb (256-bit) operations
// =============================================================================

/// Multiply two 4-limb values, producing an 8-limb result.
#[inline(always)]
pub const fn mul_4_by_4(a: &[u64; 4], b: &[u64; 4]) -> [u64; 8] {
    let mut result = [0u64; 8];
    let mut i = 0;
    while i < 4 {
        let mut carry = 0u128;
        let mut j = 0;
        while j < 4 {
            let prod = (a[i] as u128) * (b[j] as u128) + (result[i + j] as u128) + carry;
            result[i + j] = prod as u64;
            carry = prod >> 64;
            j += 1;
        }
        result[i + 4] = carry as u64;
        i += 1;
    }
    result
}

/// Fused multiply-accumulate: acc[0..9] += a[0..4] * b[0..4]
///
/// On x86_64 with BMI2+ADX, uses an optimized inline-asm path with mulx/adcx/adox.
/// Falls back to portable Rust on other architectures or when BMI2+ADX are unavailable.
#[cfg(target_arch = "x86_64")]
#[inline(always)]
#[allow(unsafe_code)]
pub fn mul_acc_4_by_4(acc: &mut [u64; 9], a: &[u64; 4], b: &[u64; 4]) {
    if std::is_x86_feature_detected!("bmi2") && std::is_x86_feature_detected!("adx") {
        // Safety: we have verified BMI2+ADX are available at runtime.
        unsafe { mul_acc_4_by_4_asm(acc, a, b) }
    } else {
        mul_acc_4_by_4_portable(acc, a, b)
    }
}

/// x86_64 BMI2+ADX implementation.
///
/// Two-phase approach using x86_64 BMI2 (mulx) and ADX (adcx/adox):
/// Phase 1: Schoolbook 4×4 multiply into r8..r15 using adcx/adox dual carry chains
///          for rows 1-3 (OF carries high-word additions, CF carries low-word additions)
/// Phase 2: Single adc chain to add the 8-limb product into the 9-limb accumulator
///
/// ~25% faster than the Rust reference (9.2ns vs 12.4ns per call on AMD Zen3).
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "bmi2", enable = "adx")]
#[allow(unsafe_code)]
unsafe fn mul_acc_4_by_4_asm(acc: &mut [u64; 9], a: &[u64; 4], b: &[u64; 4]) {
    unsafe {
        core::arch::asm!(
          // Phase 1: Schoolbook multiply a*b into r8..r15

          // Row 0: r8..r12 = a[0] * b[0..4]
          "mov rdx, [{a_ptr}]",
          "mulx r9,  r8,  [{b_ptr}]",
          "mulx r10, rax, [{b_ptr} + 8]",
          "add  r9, rax",
          "mulx r11, rax, [{b_ptr} + 16]",
          "adc  r10, rax",
          "mulx r12, rax, [{b_ptr} + 24]",
          "adc  r11, rax",
          "adc  r12, 0",
          "xor  r13d, r13d",

          // Row 1: += a[1] * b[0..4] << 64 (adcx=lo/CF, adox=hi/OF)
          "mov rdx, [{a_ptr} + 8]",
          "xor  r14d, r14d",
          "mulx rcx, rax, [{b_ptr}]",
          "adcx r9,  rax",
          "adox r10, rcx",
          "mulx rcx, rax, [{b_ptr} + 8]",
          "adcx r10, rax",
          "adox r11, rcx",
          "mulx rcx, rax, [{b_ptr} + 16]",
          "adcx r11, rax",
          "adox r12, rcx",
          "mulx rcx, rax, [{b_ptr} + 24]",
          "adcx r12, rax",
          "adox r13, rcx",
          "adcx r13, r14",

          // Row 2: += a[2] * b[0..4] << 128
          "mov rdx, [{a_ptr} + 16]",
          "xor  r14d, r14d",
          "mulx rcx, rax, [{b_ptr}]",
          "adcx r10, rax",
          "adox r11, rcx",
          "mulx rcx, rax, [{b_ptr} + 8]",
          "adcx r11, rax",
          "adox r12, rcx",
          "mulx rcx, rax, [{b_ptr} + 16]",
          "adcx r12, rax",
          "adox r13, rcx",
          "mulx rcx, rax, [{b_ptr} + 24]",
          "adcx r13, rax",
          "mov  r15d, 0",
          "adox r14, rcx",
          "adcx r14, r15",

          // Row 3: += a[3] * b[0..4] << 192
          "mov rdx, [{a_ptr} + 24]",
          "xor  r15d, r15d",
          "mulx rcx, rax, [{b_ptr}]",
          "adcx r11, rax",
          "adox r12, rcx",
          "mulx rcx, rax, [{b_ptr} + 8]",
          "adcx r12, rax",
          "adox r13, rcx",
          "mulx rcx, rax, [{b_ptr} + 16]",
          "adcx r13, rax",
          "adox r14, rcx",
          "mulx rcx, rax, [{b_ptr} + 24]",
          "adcx r14, rax",
          "mov  rax, 0",
          "adox r15, rcx",
          "adcx r15, rax",

          // Phase 2: Add product r8..r15 to accumulator in one adc chain
          "add  r8,  [{acc_ptr}]",
          "adc  r9,  [{acc_ptr} + 8]",
          "adc  r10, [{acc_ptr} + 16]",
          "adc  r11, [{acc_ptr} + 24]",
          "adc  r12, [{acc_ptr} + 32]",
          "adc  r13, [{acc_ptr} + 40]",
          "adc  r14, [{acc_ptr} + 48]",
          "adc  r15, [{acc_ptr} + 56]",
          "mov [{acc_ptr}], r8",
          "mov [{acc_ptr} + 8], r9",
          "mov [{acc_ptr} + 16], r10",
          "mov [{acc_ptr} + 24], r11",
          "mov [{acc_ptr} + 32], r12",
          "mov [{acc_ptr} + 40], r13",
          "mov [{acc_ptr} + 48], r14",
          "mov [{acc_ptr} + 56], r15",
          "mov rax, 0",
          "adc rax, [{acc_ptr} + 64]",
          "mov [{acc_ptr} + 64], rax",

          acc_ptr = in(reg) acc.as_mut_ptr(),
          a_ptr = in(reg) a.as_ptr(),
          b_ptr = in(reg) b.as_ptr(),
          out("rax") _,
          out("rcx") _,
          out("rdx") _,
          out("r8") _,
          out("r9") _,
          out("r10") _,
          out("r11") _,
          out("r12") _,
          out("r13") _,
          out("r14") _,
          out("r15") _,
          options(nostack)
        );
    }
}

/// Portable fallback implementation.
#[inline(always)]
fn mul_acc_4_by_4_portable(acc: &mut [u64; 9], a: &[u64; 4], b: &[u64; 4]) {
    let product = mul_4_by_4(a, b);
    let mut carry = 0u128;
    for i in 0..8 {
        let sum = (acc[i] as u128) + (product[i] as u128) + carry;
        acc[i] = sum as u64;
        carry = sum >> 64;
    }
    let new_val = (acc[8] as u128) + carry;
    debug_assert!(
        new_val <= u64::MAX as u128,
        "mul_acc_4_by_4: acc[8] overflow ({new_val} > u64::MAX)"
    );
    acc[8] = new_val as u64;
}

/// Fused multiply-accumulate for non-x86_64 targets.
#[cfg(not(target_arch = "x86_64"))]
#[inline(always)]
pub fn mul_acc_4_by_4(acc: &mut [u64; 9], a: &[u64; 4], b: &[u64; 4]) {
    mul_acc_4_by_4_portable(acc, a, b)
}

// =============================================================================
// 5-limb (320-bit) operations
// =============================================================================

/// Check if 5-limb value a >= 4-limb value b.
#[inline(always)]
pub(super) const fn gte_5_4(a: &[u64; 5], b: &[u64; 4]) -> bool {
    // If a[4] > 0, then a >= b (since b fits in 256 bits)
    if a[4] > 0 {
        return true;
    }
    // Compare the lower 4 limbs from most significant to least
    let mut i = 4;
    while i > 0 {
        i -= 1;
        if a[i] > b[i] {
            return true;
        }
        if a[i] < b[i] {
            return false;
        }
    }
    true // equal
}

/// Subtract 4-limb from 5-limb: a - b (returns 5 limbs, assumes a >= b).
#[inline(always)]
pub(super) const fn sub_5_4(a: &[u64; 5], b: &[u64; 4]) -> [u64; 5] {
    let mut result = [0u64; 5];
    let mut borrow = 0u64;
    let mut i = 0;
    while i < 4 {
        let (diff, b1) = a[i].overflowing_sub(b[i]);
        let (diff2, b2) = diff.overflowing_sub(borrow);
        result[i] = diff2;
        borrow = (b1 as u64) + (b2 as u64);
        i += 1;
    }
    let (diff, _) = a[4].overflowing_sub(borrow);
    result[4] = diff;
    result
}

// =============================================================================
// 8-limb (512-bit) operations
// =============================================================================

/// Reduce an 8-limb value modulo a 4-limb prime using old-school binary long division.
///
/// Aligns p with x's MSB, then iteratively trial-subtracts at each bit position.
/// Slow (O(n²) in bits) but works in `const fn` context where Montgomery reduction
/// isn't available. Result fits in 4 limbs because `x mod p < p`.
#[inline(always)]
pub(crate) const fn reduce_8_mod_4(x: &[u64; 8], p: &[u64; 4]) -> [u64; 4] {
    // Extend p to 8 limbs for comparison
    let p8: [u64; 8] = [p[0], p[1], p[2], p[3], 0, 0, 0, 0];

    // If x < p, we're done
    if !gte::<8>(x, &p8) {
        return [x[0], x[1], x[2], x[3]];
    }

    // Binary long division: shift p left until it's just <= x, then subtract
    let x_clz = clz::<8>(x);
    let p_clz = clz::<8>(&p8);

    // p needs to shift left by (p_clz - x_clz) bits to align with x
    if p_clz <= x_clz {
        // p is already larger than x in leading position, but we know x >= p
        // so just do final subtraction
        let result = sub::<8>(x, &p8);
        return [result[0], result[1], result[2], result[3]];
    }

    let shift_bits = p_clz - x_clz;
    let mut remainder = *x;

    // Shift p left by shift_bits
    let mut shifted_p = p8;
    let mut bits_to_shift = shift_bits;
    while bits_to_shift > 0 {
        shifted_p = shl::<8>(&shifted_p);
        bits_to_shift -= 1;
    }

    // Do shift_bits + 1 iterations of trial subtraction
    let mut iterations = shift_bits + 1;
    while iterations > 0 {
        if gte::<8>(&remainder, &shifted_p) {
            remainder = sub::<8>(&remainder, &shifted_p);
        }
        // Shift p right by 1 (for next iteration)
        if iterations > 1 {
            let mut i = 0;
            while i < 7 {
                shifted_p[i] = (shifted_p[i] >> 1) | (shifted_p[i + 1] << 63);
                i += 1;
            }
            shifted_p[7] >>= 1;
        }
        iterations -= 1;
    }

    [remainder[0], remainder[1], remainder[2], remainder[3]]
}
