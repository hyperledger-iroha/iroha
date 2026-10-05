//! Accelerated kernels, their dispatch and the scalar references they must equal.
//!
//! Exact modular arithmetic has one right answer, so an accelerated kernel is a
//! different schedule for the same words, not a different result. Each
//! dispatching function below returns exactly what its `_scalar` reference
//! returns for every input: ineligible inputs (wrong length, unreduced
//! operands, unsupported modulus, no CPU support) take the scalar path.
//!
//! The `simd` feature compiles the NEON kernel on `AArch64` and the AVX2 kernel
//! on x86-64 (selected at run time by CPU detection). Without the feature, or
//! on any other target, only the scalar path exists. No environment variable
//! or configuration value selects a backend.
//!
//! Test builds also compile the AVX2 kernel on hosts that cannot execute it,
//! against a lane model of its intrinsics, so its schedule is compared with
//! the scalar reference everywhere; x86-64 hosts compare the model with the
//! CPU. The NEON kernel runs natively on every `AArch64` host.
//!
//! Accelerated products use Montgomery reduction with `R = 2^32` and therefore
//! require an odd modulus below `2^32`; accelerated additions require a
//! modulus below `2^62`. Kernels do not copy operands into heap scratch, so
//! they add no clearing obligation beyond the caller's buffers; the transform
//! twiddle table holds public powers of the root only.
use crate::modular::{add_mod_u64, mul_mod_u64, sub_mod_u64};

#[cfg(any(test, all(feature = "simd", target_arch = "x86_64")))]
mod avx2;
#[cfg(test)]
mod avx2_model;
#[cfg(all(feature = "simd", target_arch = "aarch64"))]
mod neon;
#[cfg(all(feature = "simd", target_arch = "x86_64"))]
use avx2 as simd;
#[cfg(all(feature = "simd", target_arch = "aarch64"))]
use neon as simd;

/// Kernel family selected for eligible inputs on this build and host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    /// Portable scalar reference.
    Scalar,
    /// `AArch64` NEON, two 64-bit lanes.
    Neon,
    /// x86-64 AVX2, four 64-bit lanes.
    Avx2,
}

/// Shortest slice an accelerated kernel accepts.
pub const MIN_ACCELERATED_LEN: usize = 8;
/// Accelerated products and transforms require an odd modulus below this bound.
pub const ACCELERATED_MUL_MODULUS_BOUND: u64 = 1 << 32;
/// Accelerated additions and subtractions require a modulus below this bound.
pub const ACCELERATED_ADD_MODULUS_BOUND: u64 = 1 << 62;

/// The backend the dispatchers use for eligible inputs on this host.
#[must_use]
pub fn active_backend() -> Backend {
    #[cfg(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        if !simd::is_available() {
            return Backend::Scalar;
        }
        if cfg!(target_arch = "aarch64") {
            Backend::Neon
        } else {
            Backend::Avx2
        }
    }
    #[cfg(not(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64"))))]
    {
        Backend::Scalar
    }
}

/// Montgomery constants for an odd modulus below `2^32` with `R = 2^32`.
#[cfg(any(
    test,
    all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64"))
))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Montgomery32 {
    /// The modulus.
    pub(crate) modulus: u64,
    /// `-modulus^-1 mod 2^32`.
    pub(crate) negative_inverse: u32,
    /// `2^64 mod modulus`: multiplying by it undoes one Montgomery reduction.
    pub(crate) r_squared: u64,
}

#[cfg(any(
    test,
    all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64"))
))]
impl Montgomery32 {
    /// Constants for an odd modulus in `[3, 2^32)`; `None` otherwise.
    pub(crate) fn new(modulus: u64) -> Option<Self> {
        if modulus < 3 || modulus.is_multiple_of(2) || modulus >= ACCELERATED_MUL_MODULUS_BOUND {
            return None;
        }
        let modulus32 = u32::try_from(modulus).ok()?;
        // Newton iteration doubles the correct low bits: 5 -> 10 -> 20 -> 40.
        let mut inverse = modulus32.wrapping_mul(3) ^ 2;
        for _ in 0..3 {
            inverse = inverse.wrapping_mul(2_u32.wrapping_sub(modulus32.wrapping_mul(inverse)));
        }
        Some(Self {
            modulus,
            negative_inverse: inverse.wrapping_neg(),
            r_squared: u64::try_from((1_u128 << 64) % u128::from(modulus)).ok()?,
        })
    }

    /// `value * 2^32 mod modulus` for `value < modulus`.
    pub(crate) fn to_montgomery(self, value: u64) -> u64 {
        (value << 32) % self.modulus
    }

    /// Scalar model of the vector reduction: `lhs * rhs * 2^-32 mod modulus`.
    #[cfg(test)]
    pub(crate) fn multiply_reduce(self, lhs: u64, rhs: u64) -> u64 {
        let product = lhs * rhs;
        let low = u32::try_from(product & 0xFFFF_FFFF).expect("masked");
        let correction = u64::from(low.wrapping_mul(self.negative_inverse)) * self.modulus;
        let reduced = (product >> 32) + (correction >> 32) + u64::from(low != 0);
        if reduced >= self.modulus {
            reduced - self.modulus
        } else {
            reduced
        }
    }
}

#[cfg(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64")))]
fn all_canonical(values: &[u64], modulus: u64) -> bool {
    values.iter().all(|&value| value < modulus)
}

/// Constants when a product kernel may run on these operands.
#[cfg(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64")))]
fn product_kernel_constants(len: usize, modulus: u64) -> Option<Montgomery32> {
    if len < MIN_ACCELERATED_LEN {
        return None;
    }
    Montgomery32::new(modulus)
}

#[cfg(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64")))]
fn sum_kernel_eligible(lhs: &[u64], rhs: &[u64], modulus: u64) -> bool {
    lhs.len() == rhs.len()
        && lhs.len() >= MIN_ACCELERATED_LEN
        && modulus != 0
        && modulus < ACCELERATED_ADD_MODULUS_BOUND
        && all_canonical(lhs, modulus)
        && all_canonical(rhs, modulus)
}

/// Run the accelerated forward cyclic transform when the input is eligible.
///
/// The caller guarantees a power-of-two length. Returns `false` without
/// touching `values` when the scalar path must run.
pub(crate) fn try_cyclic_ntt(values: &mut [u64], root: u64, modulus: u64) -> bool {
    #[cfg(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        let Some(constants) = product_kernel_constants(values.len(), modulus) else {
            return false;
        };
        if !values.len().is_power_of_two() || !all_canonical(values, modulus) {
            return false;
        }
        simd::try_cyclic_ntt(values, root % modulus, constants)
    }
    #[cfg(not(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64"))))]
    {
        let _ = (values, root, modulus);
        false
    }
}

/// Run the accelerated pointwise product when the operands are eligible.
fn try_mul_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64) -> bool {
    #[cfg(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        let Some(constants) = product_kernel_constants(lhs.len(), modulus) else {
            return false;
        };
        if lhs.len() != rhs.len() || !all_canonical(lhs, modulus) || !all_canonical(rhs, modulus) {
            return false;
        }
        simd::try_mul_mod_assign(lhs, rhs, constants)
    }
    #[cfg(not(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64"))))]
    {
        let _ = (lhs, rhs, modulus);
        false
    }
}

/// Run the accelerated scalar product when the operands are eligible.
fn try_mul_scalar_mod(values: &mut [u64], scalar: u64, modulus: u64) -> bool {
    #[cfg(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        let Some(constants) = product_kernel_constants(values.len(), modulus) else {
            return false;
        };
        if !all_canonical(values, modulus) {
            return false;
        }
        simd::try_mul_scalar_mod(values, scalar % modulus, constants)
    }
    #[cfg(not(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64"))))]
    {
        let _ = (values, scalar, modulus);
        false
    }
}

fn try_add_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64) -> bool {
    #[cfg(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        if !sum_kernel_eligible(lhs, rhs, modulus) {
            return false;
        }
        simd::try_add_mod_assign(lhs, rhs, modulus)
    }
    #[cfg(not(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64"))))]
    {
        let _ = (lhs, rhs, modulus);
        false
    }
}

fn try_sub_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64) -> bool {
    #[cfg(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        if !sum_kernel_eligible(lhs, rhs, modulus) {
            return false;
        }
        simd::try_sub_mod_assign(lhs, rhs, modulus)
    }
    #[cfg(not(all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64"))))]
    {
        let _ = (lhs, rhs, modulus);
        false
    }
}

/// Scalar reference: `lhs[i] = (lhs[i] + rhs[i]) mod modulus` over the common prefix.
pub fn add_mod_assign_scalar(lhs: &mut [u64], rhs: &[u64], modulus: u64) {
    for (left, &right) in lhs.iter_mut().zip(rhs) {
        *left = add_mod_u64(*left, right, modulus);
    }
}

/// `lhs[i] = (lhs[i] + rhs[i]) mod modulus` over the common prefix.
pub fn add_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64) {
    if !try_add_mod_assign(lhs, rhs, modulus) {
        add_mod_assign_scalar(lhs, rhs, modulus);
    }
}

/// Scalar reference: `lhs[i] = (lhs[i] - rhs[i]) mod modulus` over the common prefix.
pub fn sub_mod_assign_scalar(lhs: &mut [u64], rhs: &[u64], modulus: u64) {
    for (left, &right) in lhs.iter_mut().zip(rhs) {
        *left = sub_mod_u64(*left, right, modulus);
    }
}

/// `lhs[i] = (lhs[i] - rhs[i]) mod modulus` over the common prefix.
pub fn sub_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64) {
    if !try_sub_mod_assign(lhs, rhs, modulus) {
        sub_mod_assign_scalar(lhs, rhs, modulus);
    }
}

/// Scalar reference: `lhs[i] = (lhs[i] * rhs[i]) mod modulus` over the common prefix.
pub fn mul_mod_assign_scalar(lhs: &mut [u64], rhs: &[u64], modulus: u64) {
    for (left, &right) in lhs.iter_mut().zip(rhs) {
        *left = mul_mod_u64(*left, right, modulus);
    }
}

/// `lhs[i] = (lhs[i] * rhs[i]) mod modulus` over the common prefix.
pub fn mul_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64) {
    if !try_mul_mod_assign(lhs, rhs, modulus) {
        mul_mod_assign_scalar(lhs, rhs, modulus);
    }
}

/// Scalar reference: `values[i] = (values[i] * scalar) mod modulus`.
pub fn mul_scalar_mod_scalar(values: &mut [u64], scalar: u64, modulus: u64) {
    for value in values {
        *value = mul_mod_u64(*value, scalar, modulus);
    }
}

/// `values[i] = (values[i] * scalar) mod modulus`.
pub fn mul_scalar_mod(values: &mut [u64], scalar: u64, modulus: u64) {
    if !try_mul_scalar_mod(values, scalar, modulus) {
        mul_scalar_mod_scalar(values, scalar, modulus);
    }
}

/// Canonical butterfly on one pair with twiddle one: `(a, b) -> (a + b, a - b)`.
#[cfg(any(
    test,
    all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64"))
))]
fn first_stage(values: &mut [u64], modulus: u64) {
    use crate::constant_time::{add_mod_canonical_u64, sub_mod_canonical_u64};
    for pair in values.chunks_exact_mut(2) {
        let (left, right) = (pair[0], pair[1]);
        pair[0] = add_mod_canonical_u64(left, right, modulus);
        pair[1] = sub_mod_canonical_u64(left, right, modulus);
    }
}

/// Powers `root^0 .. root^(len/2 - 1)` in Montgomery form for a transform of length `len`.
///
/// `root` must be canonical: the running power is multiplied in one word.
#[cfg(any(
    test,
    all(feature = "simd", any(target_arch = "aarch64", target_arch = "x86_64"))
))]
fn montgomery_twiddles(len: usize, root: u64, constants: Montgomery32) -> Vec<u64> {
    let mut twiddles = Vec::with_capacity(len / 2);
    let mut power = 1_u64;
    for _ in 0..len / 2 {
        twiddles.push(constants.to_montgomery(power));
        power = power * root % constants.modulus;
    }
    twiddles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn splitmix64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = *state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    /// The last three moduli are at and above the accelerated-addition bound: a sum of two
    /// residues of the last two wraps a word, so a sum kernel that accepted them would be wrong.
    const MODULI: [u64; 11] = [
        1,
        2,
        3,
        30_593,
        35_969,
        2_013_265_921,
        4_293_918_721,
        70_368_744_067_073,
        (1 << 62) + 135,
        (1 << 63) + 29,
        u64::MAX,
    ];

    fn operand_sets(modulus: u64, len: usize, state: &mut u64) -> Vec<(Vec<u64>, Vec<u64>)> {
        let canonical = |state: &mut u64| -> Vec<u64> {
            (0..len).map(|_| splitmix64(state) % modulus).collect()
        };
        let mut sets = vec![(canonical(state), canonical(state))];
        // Boundary residues in every lane position.
        let boundary = [0, modulus - 1, modulus / 2, (modulus - 1) / 2, 1 % modulus];
        let lhs: Vec<u64> = (0..len)
            .map(|index| boundary[index % boundary.len()])
            .collect();
        let rhs: Vec<u64> = (0..len)
            .map(|index| boundary[(index / 2 + 1) % boundary.len()])
            .collect();
        sets.push((lhs, rhs));
        sets.push((vec![modulus - 1; len], vec![modulus - 1; len]));
        // Unreduced operands must take the scalar path and still be exact.
        sets.push((
            (0..len).map(|_| splitmix64(state)).collect(),
            canonical(state),
        ));
        sets
    }

    #[test]
    fn backend_report_matches_the_build() {
        let backend = active_backend();
        if cfg!(not(feature = "simd")) {
            assert_eq!(backend, Backend::Scalar);
        } else if cfg!(target_arch = "aarch64") {
            assert_eq!(
                backend,
                Backend::Neon,
                "NEON is baseline on supported AArch64 targets"
            );
        } else if cfg!(not(target_arch = "x86_64")) {
            assert_eq!(backend, Backend::Scalar);
        }
    }

    #[test]
    fn montgomery_constants_satisfy_their_identities_and_reject_unsupported_moduli() {
        for modulus in [3_u64, 30_593, 2_013_265_921, 4_293_918_721, (1 << 32) - 5] {
            let constants = Montgomery32::new(modulus).expect("odd modulus below 2^32");
            let modulus32 = u32::try_from(modulus).unwrap();
            assert_eq!(modulus32.wrapping_mul(constants.negative_inverse), u32::MAX);
            assert_eq!(
                u128::from(constants.r_squared),
                (1_u128 << 64) % u128::from(modulus)
            );
            let mut state = modulus;
            for _ in 0..256 {
                let lhs = splitmix64(&mut state) % modulus;
                let rhs = splitmix64(&mut state) % modulus;
                let reduced = constants.multiply_reduce(lhs, constants.to_montgomery(rhs));
                assert_eq!(reduced, lhs * rhs % modulus);
                let twice = constants
                    .multiply_reduce(constants.multiply_reduce(lhs, rhs), constants.r_squared);
                assert_eq!(twice, lhs * rhs % modulus);
            }
            for (lhs, rhs) in [(0, 0), (modulus - 1, modulus - 1), (modulus - 1, 1), (1, 1)] {
                assert_eq!(
                    constants.multiply_reduce(lhs, constants.to_montgomery(rhs)),
                    lhs * rhs % modulus
                );
            }
        }
        for rejected in [0_u64, 1, 2, 4, 1 << 32, (1 << 32) + 15] {
            assert_eq!(Montgomery32::new(rejected), None);
        }
    }

    #[test]
    fn dispatching_slice_kernels_equal_their_scalar_references() {
        let mut state = 0xACCE_u64;
        for modulus in MODULI {
            for len in [0_usize, 1, 7, 8, 9, 15, 16, 17, 64, 67] {
                for (lhs, rhs) in operand_sets(modulus, len, &mut state) {
                    type Kernel = fn(&mut [u64], &[u64], u64);
                    let pairs: [(Kernel, Kernel); 3] = [
                        (add_mod_assign, add_mod_assign_scalar),
                        (sub_mod_assign, sub_mod_assign_scalar),
                        (mul_mod_assign, mul_mod_assign_scalar),
                    ];
                    for (dispatching, scalar) in pairs {
                        let mut actual = lhs.clone();
                        let mut expected = lhs.clone();
                        dispatching(&mut actual, &rhs, modulus);
                        scalar(&mut expected, &rhs, modulus);
                        assert_eq!(actual, expected, "modulus {modulus} len {len}");
                    }
                    for scalar_operand in [0, 1, modulus - 1, splitmix64(&mut state)] {
                        let mut actual = lhs.clone();
                        let mut expected = lhs.clone();
                        mul_scalar_mod(&mut actual, scalar_operand, modulus);
                        mul_scalar_mod_scalar(&mut expected, scalar_operand, modulus);
                        assert_eq!(actual, expected, "modulus {modulus} len {len}");
                    }
                }
            }
        }
    }

    /// A sum of two canonical residues of a modulus at or above `2^63` wraps a word. The
    /// dispatcher must keep such a modulus on the scalar path at every accelerated length.
    #[test]
    fn sums_at_and_above_the_accelerated_bound_stay_exact() {
        for modulus in [
            ACCELERATED_ADD_MODULUS_BOUND - 1,
            ACCELERATED_ADD_MODULUS_BOUND,
            (1 << 63) - 1,
            1 << 63,
            (1 << 63) + 29,
            u64::MAX,
        ] {
            for len in [MIN_ACCELERATED_LEN, 9, 16, 19] {
                // Largest residues: the sum is 2 * modulus - 2, the difference of 0 and m - 1 is 1.
                let top = vec![modulus - 1; len];
                let mut sum = top.clone();
                add_mod_assign(&mut sum, &top, modulus);
                assert_eq!(sum, vec![modulus - 2; len], "modulus {modulus} len {len}");
                let mut difference = vec![0_u64; len];
                sub_mod_assign(&mut difference, &top, modulus);
                assert_eq!(difference, vec![1; len], "modulus {modulus} len {len}");
                // Around the half point, where a 64-bit sum first reaches the modulus.
                let half = vec![modulus / 2 + 1; len];
                let mut sum = half.clone();
                add_mod_assign(&mut sum, &half, modulus);
                let expected =
                    u64::try_from((u128::from(modulus / 2 + 1) * 2) % u128::from(modulus))
                        .expect("residue");
                assert_eq!(sum, vec![expected; len], "modulus {modulus} len {len}");
            }
        }
    }

    /// Canonical operand pairs for a direct kernel call: random, boundary residues in every lane
    /// position, and the largest residue everywhere.
    fn canonical_sets(modulus: u64, len: usize, state: &mut u64) -> Vec<(Vec<u64>, Vec<u64>)> {
        let mut sets = operand_sets(modulus, len, state);
        // The last set of `operand_sets` holds unreduced operands, which only the dispatcher sees.
        sets.pop();
        sets
    }

    /// The AVX2 kernel, called directly, returns the words of the scalar reference.
    ///
    /// On x86-64 this runs the CPU's instructions (and does nothing when the CPU lacks AVX2);
    /// on every other host the same kernel source runs over the lane model of its intrinsics.
    #[test]
    fn avx2_kernel_sums_and_products_equal_the_scalar_references() {
        if !avx2::is_available() {
            eprintln!("AVX2 is not available on this CPU; the kernel did not run");
            return;
        }
        let mut state = 0xA2_u64;
        let lengths = [8_usize, 9, 10, 11, 12, 13, 15, 16, 17, 64, 67];
        // Sums: every modulus below the accelerated bound, including the largest.
        for modulus in [
            1_u64,
            2,
            3,
            30_593,
            4_293_918_721,
            70_368_744_067_073,
            ACCELERATED_ADD_MODULUS_BOUND - 57,
            ACCELERATED_ADD_MODULUS_BOUND - 1,
        ] {
            for len in lengths {
                for (lhs, rhs) in canonical_sets(modulus, len, &mut state) {
                    let mut actual = lhs.clone();
                    let mut expected = lhs.clone();
                    assert!(avx2::try_add_mod_assign(&mut actual, &rhs, modulus));
                    add_mod_assign_scalar(&mut expected, &rhs, modulus);
                    assert_eq!(actual, expected, "sum, modulus {modulus} len {len}");
                    let mut actual = lhs.clone();
                    let mut expected = lhs.clone();
                    assert!(avx2::try_sub_mod_assign(&mut actual, &rhs, modulus));
                    sub_mod_assign_scalar(&mut expected, &rhs, modulus);
                    assert_eq!(actual, expected, "difference, modulus {modulus} len {len}");
                }
            }
        }
        // Products: every odd modulus class below 2^32, including the smallest and the largest.
        for modulus in [
            3_u64,
            30_593,
            35_969,
            2_013_265_921,
            4_293_918_721,
            (1 << 32) - 5,
            (1 << 32) - 1,
        ] {
            let constants = Montgomery32::new(modulus).expect("odd modulus below 2^32");
            for len in lengths {
                for (lhs, rhs) in canonical_sets(modulus, len, &mut state) {
                    let mut actual = lhs.clone();
                    let mut expected = lhs.clone();
                    assert!(avx2::try_mul_mod_assign(&mut actual, &rhs, constants));
                    mul_mod_assign_scalar(&mut expected, &rhs, modulus);
                    assert_eq!(actual, expected, "product, modulus {modulus} len {len}");
                    for scalar in [
                        0,
                        1,
                        modulus - 1,
                        modulus / 2,
                        splitmix64(&mut state) % modulus,
                    ] {
                        let mut actual = lhs.clone();
                        let mut expected = lhs.clone();
                        assert!(avx2::try_mul_scalar_mod(&mut actual, scalar, constants));
                        mul_scalar_mod_scalar(&mut expected, scalar, modulus);
                        assert_eq!(actual, expected, "scaling, modulus {modulus} len {len}");
                    }
                }
            }
        }
    }

    /// The AVX2 transform, called directly, equals the scalar transform at every length:
    /// lengths one to four exercise the scalar stages, eight and above the four-lane stages.
    #[test]
    fn avx2_kernel_transform_equals_the_scalar_reference() {
        use crate::{
            modular::primitive_root_of_order_with_candidate_limit, ntt::cyclic_ntt_in_place_scalar,
        };
        if !avx2::is_available() {
            eprintln!("AVX2 is not available on this CPU; the kernel did not run");
            return;
        }
        let mut state = 0xA3_u64;
        for (modulus, max_log) in [
            (30_593_u64, 7_u32),
            (2_013_265_921, 10),
            (4_293_918_721, 10),
        ] {
            let constants = Montgomery32::new(modulus).expect("odd modulus below 2^32");
            for log_len in 0..=max_log {
                let len = 1_usize << log_len;
                let root = if len == 1 {
                    1
                } else {
                    primitive_root_of_order_with_candidate_limit(modulus, len as u64, 4_096)
                        .expect("root")
                };
                let inputs = [
                    (0..len).map(|_| splitmix64(&mut state) % modulus).collect(),
                    vec![modulus - 1; len],
                    (0..len)
                        .map(|index| [0, modulus - 1, 1, modulus / 2][index % 4])
                        .collect::<Vec<u64>>(),
                ];
                for input in inputs {
                    let mut actual = input.clone();
                    let mut expected = input;
                    assert!(avx2::try_cyclic_ntt(&mut actual, root, constants));
                    cyclic_ntt_in_place_scalar(&mut expected, root, modulus, false)
                        .expect("scalar transform");
                    assert_eq!(actual, expected, "modulus {modulus} len {len}");
                }
            }
        }
    }

    #[test]
    fn mismatched_lengths_use_the_common_prefix_like_the_scalar_reference() {
        let modulus = 30_593_u64;
        let rhs: Vec<u64> = (0..12).map(|value| value * 2_000 % modulus).collect();
        let mut actual: Vec<u64> = (0..20).map(|value| value * 1_500 % modulus).collect();
        let mut expected = actual.clone();
        add_mod_assign(&mut actual, &rhs, modulus);
        add_mod_assign_scalar(&mut expected, &rhs, modulus);
        assert_eq!(actual, expected);
        mul_mod_assign(&mut actual, &rhs, modulus);
        mul_mod_assign_scalar(&mut expected, &rhs, modulus);
        assert_eq!(actual, expected);
        sub_mod_assign(&mut actual, &rhs, modulus);
        sub_mod_assign_scalar(&mut expected, &rhs, modulus);
        assert_eq!(actual, expected);
    }

    #[test]
    fn scalar_references_use_exact_reduction_and_zero_for_a_zero_modulus() {
        let mut values = [5_u64, u64::MAX, 0];
        add_mod_assign_scalar(&mut values, &[7, 1, 0], 11);
        // 2^10 = 1 mod 11, so u64::MAX + 1 = 2^64 = 2^4 = 5 mod 11.
        assert_eq!(values, [1, 5, 0]);
        let mut values = [5_u64, 3];
        sub_mod_assign_scalar(&mut values, &[7, 3], 11);
        assert_eq!(values, [9, 0]);
        let mut values = [5_u64, 3];
        mul_mod_assign_scalar(&mut values, &[7, 4], 11);
        assert_eq!(values, [2, 1]);
        let mut values = [5_u64, 3];
        mul_scalar_mod_scalar(&mut values, 9, 11);
        assert_eq!(values, [1, 5]);
        let mut values = [5_u64, 3];
        mul_scalar_mod(&mut values, 9, 0);
        assert_eq!(values, [0, 0]);
    }
}
