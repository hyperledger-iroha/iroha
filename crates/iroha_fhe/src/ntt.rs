//! Radix-2 number-theoretic transforms and negacyclic ring products.
//!
//! One transform skeleton serves every caller. [`cyclic_ntt_with`] is generic
//! over [`ModularArithmetic`], so the variable-time word arithmetic of
//! [`WordModulus`] and the branch-free Montgomery arithmetic of
//! [`crate::constant_time::FixedModulus`] run the same butterfly schedule.
//!
//! Layout and order are fixed: input is bit-reversed in place, then stages of
//! length `2, 4, ..., n` run decimation-in-time butterflies
//! `(a, b) -> (a + w*b, a - w*b)` with `w` stepping through powers of the stage
//! root. Output is in natural order. A negacyclic transform multiplies
//! coefficient `i` by `psi^i` first, where `psi` is a primitive `2n`-th root of
//! unity and `psi^2` is the cyclic root.
//!
//! [`cyclic_ntt_in_place`] may run an accelerated kernel (see [`crate::accel`]);
//! [`cyclic_ntt_in_place_scalar`] is the semantic reference it must equal.
use crate::{
    accel,
    modular::{ModularArithmetic, WordModulus, mod_inv_prime_u64, mod_pow_u64, mul_mod_u64},
    rns,
};
use zeroize::Zeroizing;

/// Permute a power-of-two slice into bit-reversed index order.
///
/// Slices whose length is zero or one are left unchanged.
pub fn bit_reverse_permute(values: &mut [u64]) {
    if values.len() < 2 {
        return;
    }
    let bits = values.len().ilog2();
    for index in 0..values.len() {
        let reversed = index.reverse_bits() >> (usize::BITS - bits);
        if reversed > index {
            values.swap(index, reversed);
        }
    }
}

/// Forward cyclic transform with the given root over any modular arithmetic.
///
/// `root` must be a primitive root of order `values.len()`; no inverse scaling
/// is applied. Operands must satisfy the operand contract of the arithmetic
/// (canonical residues for [`crate::constant_time::FixedModulus`]). The index
/// schedule depends only on the length, so the transform is constant-time
/// whenever the arithmetic is.
///
/// # Returns
/// `None`, leaving `values` untouched, when the length is zero or not a power
/// of two. The check is the same in every build profile.
pub fn cyclic_ntt_with<A: ModularArithmetic>(
    values: &mut [u64],
    arithmetic: &A,
    root: u64,
) -> Option<()> {
    let len = values.len();
    if len == 0 || !len.is_power_of_two() {
        return None;
    }
    bit_reverse_permute(values);
    let mut stage_len = 2_usize;
    while stage_len <= len {
        // usize always fits u64 on supported targets.
        let step = arithmetic.pow(root, (len / stage_len) as u64);
        for chunk in values.chunks_exact_mut(stage_len) {
            let (lo, hi) = chunk.split_at_mut(stage_len / 2);
            let mut twiddle = 1_u64;
            for (left, right) in lo.iter_mut().zip(hi.iter_mut()) {
                let product = arithmetic.mul(*right, twiddle);
                let left_value = *left;
                *left = arithmetic.add(left_value, product);
                *right = arithmetic.sub(left_value, product);
                twiddle = arithmetic.mul(twiddle, step);
            }
        }
        let Some(next) = stage_len.checked_mul(2) else {
            break;
        };
        stage_len = next;
    }
    Some(())
}

/// Reduce the only element of a length-one transform.
///
/// A transform of length one has no butterfly, so nothing else would reduce an
/// unreduced input. `modulus` is non-zero.
fn reduce_single_element(values: &mut [u64], modulus: u64) {
    if let [value] = values {
        *value %= modulus;
    }
}

/// Resolve the transform root and inverse scale shared by both transform entry points.
fn transform_parameters(
    len: usize,
    root: u64,
    modulus: u64,
    invert: bool,
) -> Option<(u64, Option<u64>)> {
    if len == 0 || !len.is_power_of_two() || modulus <= 2 {
        return None;
    }
    if !invert {
        return Some((root, None));
    }
    let inverse_root = mod_inv_prime_u64(root, modulus)?;
    let inverse_len = mod_inv_prime_u64(u64::try_from(len).ok()?, modulus)?;
    Some((inverse_root, Some(inverse_len)))
}

/// Scalar reference for [`cyclic_ntt_in_place`]; never dispatches to an accelerator.
///
/// # Returns
/// `None`, leaving `values` untouched, under the same conditions as
/// [`cyclic_ntt_in_place`].
pub fn cyclic_ntt_in_place_scalar(
    values: &mut [u64],
    root: u64,
    modulus: u64,
    invert: bool,
) -> Option<()> {
    let (root, inverse_len) = transform_parameters(values.len(), root, modulus, invert)?;
    reduce_single_element(values, modulus);
    cyclic_ntt_with(values, &WordModulus(modulus), root)?;
    if let Some(inverse_len) = inverse_len {
        for value in values {
            *value = mul_mod_u64(*value, inverse_len, modulus);
        }
    }
    Some(())
}

/// In-place cyclic transform modulo a prime, forward or inverse.
///
/// The inverse transform uses `root^-1` and multiplies every output by
/// `len^-1`. Unreduced inputs and an unreduced root are reduced exactly;
/// outputs are canonical at every length, including one.
///
/// # Returns
/// `None`, leaving `values` untouched, when the length is zero or not a power
/// of two, `modulus <= 2`, or (inverse only) the root or the length is not
/// invertible modulo `modulus`.
pub fn cyclic_ntt_in_place(
    values: &mut [u64],
    root: u64,
    modulus: u64,
    invert: bool,
) -> Option<()> {
    let (root, inverse_len) = transform_parameters(values.len(), root, modulus, invert)?;
    // TODO: dispatch to Metal and CUDA here once a word-modulus NTT kernel
    // exists. The repository's GPU transforms (crates/fastpq_prover Metal and
    // CUDA stages) are specialised to the Goldilocks field and sit above this
    // crate, so there is no kernel to reuse yet; the scalar and SIMD paths
    // below are the only ones.
    reduce_single_element(values, modulus);
    if !accel::try_cyclic_ntt(values, root, modulus) {
        cyclic_ntt_with(values, &WordModulus(modulus), root)?;
    }
    if let Some(inverse_len) = inverse_len {
        accel::mul_scalar_mod(values, inverse_len, modulus);
    }
    Some(())
}

/// Multiply coefficient `i` by `psi^i` in place.
pub fn twist_in_place_with<A: ModularArithmetic>(values: &mut [u64], arithmetic: &A, psi: u64) {
    let mut power = 1_u64;
    for value in values {
        *value = arithmetic.mul(*value, power);
        power = arithmetic.mul(power, psi);
    }
}

/// Forward negacyclic transform: twist by `psi`, then the cyclic transform with root `psi^2`.
///
/// # Returns
/// `None`, leaving `values` untouched, when the length is zero or not a power
/// of two.
pub fn forward_negacyclic_ntt_with<A: ModularArithmetic>(
    values: &mut [u64],
    arithmetic: &A,
    psi: u64,
) -> Option<()> {
    if values.is_empty() || !values.len().is_power_of_two() {
        return None;
    }
    twist_in_place_with(values, arithmetic, psi);
    let cyclic_root = arithmetic.mul(psi, psi);
    cyclic_ntt_with(values, arithmetic, cyclic_root)
}

/// Inverse negacyclic transform from pinned inverses.
///
/// Runs the cyclic transform with root `inverse_psi^2`, then multiplies
/// coefficient `i` by `inverse_degree * inverse_psi^i`.
///
/// # Returns
/// `None`, leaving `values` untouched, when the length is zero or not a power
/// of two.
pub fn inverse_negacyclic_ntt_with<A: ModularArithmetic>(
    values: &mut [u64],
    arithmetic: &A,
    inverse_psi: u64,
    inverse_degree: u64,
) -> Option<()> {
    let inverse_cyclic_root = arithmetic.mul(inverse_psi, inverse_psi);
    cyclic_ntt_with(values, arithmetic, inverse_cyclic_root)?;
    let mut inverse_twist = 1_u64;
    for value in values.iter_mut() {
        *value = arithmetic.mul(arithmetic.mul(*value, inverse_degree), inverse_twist);
        inverse_twist = arithmetic.mul(inverse_twist, inverse_psi);
    }
    Some(())
}

/// Negacyclic product in `Z_m[X] / (X^n + 1)`, leaving the product in `lhs`.
///
/// `psi` must be a primitive `2n`-th root of unity modulo the prime `modulus`.
/// `rhs` is overwritten with its transform.
///
/// # Returns
/// `None` when the operands are empty, differ in length, are not a power of
/// two long, `modulus <= 2`, or `psi`, `psi^2` or `n` is not invertible. The
/// operands then hold unspecified intermediate values.
pub fn negacyclic_multiply_ntt_in_place(
    lhs: &mut [u64],
    rhs: &mut [u64],
    psi: u64,
    modulus: u64,
) -> Option<()> {
    let degree = lhs.len();
    if degree == 0 || rhs.len() != degree || !degree.is_power_of_two() {
        return None;
    }
    let arithmetic = WordModulus(modulus);
    let omega = mul_mod_u64(psi, psi, modulus);
    let inverse_psi = mod_inv_prime_u64(psi, modulus)?;
    twist_in_place_with(lhs, &arithmetic, psi);
    twist_in_place_with(rhs, &arithmetic, psi);
    cyclic_ntt_in_place(lhs, omega, modulus, false)?;
    cyclic_ntt_in_place(rhs, omega, modulus, false)?;
    accel::mul_mod_assign(lhs, rhs, modulus);
    cyclic_ntt_in_place(lhs, omega, modulus, true)?;
    twist_in_place_with(lhs, &arithmetic, inverse_psi);
    Some(())
}

/// Negacyclic product in `Z_m[X] / (X^n + 1)` by the transform path.
///
/// Both operand copies live in clearing buffers: they are cleared when the
/// product is rejected, and on success only the product leaves. See
/// [`negacyclic_multiply_ntt_in_place`] for the `None` conditions.
#[must_use]
pub fn negacyclic_multiply_ntt(
    lhs: &[u64],
    rhs: &[u64],
    psi: u64,
    modulus: u64,
) -> Option<Vec<u64>> {
    let mut product = Zeroizing::new(lhs.to_vec());
    let mut rhs_transform = Zeroizing::new(rhs.to_vec());
    negacyclic_multiply_ntt_in_place(&mut product, &mut rhs_transform, psi, modulus)?;
    Some(std::mem::take(&mut *product))
}

/// One helper prime of the exact CRT convolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NttPrime {
    /// The prime modulus.
    pub modulus: u64,
    /// A generator of the multiplicative group modulo [`Self::modulus`].
    pub primitive_root: u64,
    /// Largest `k` such that `2^k` divides `modulus - 1` and transforms of length `2^k` are used.
    pub max_power_of_two: u32,
}

/// The four helper primes of the exact CRT convolution.
///
/// Their product is about `2^128 - 2^108`, so every coefficient of a linear
/// product of polynomials with word coefficients and length up to `2^15` is
/// recovered exactly whenever it fits `u128`.
pub const CRT_NTT_PRIMES: [NttPrime; 4] = [
    NttPrime {
        modulus: 4_293_918_721,
        primitive_root: 19,
        max_power_of_two: 20,
    },
    NttPrime {
        modulus: 4_292_804_609,
        primitive_root: 3,
        max_power_of_two: 16,
    },
    NttPrime {
        modulus: 4_292_149_249,
        primitive_root: 14,
        max_power_of_two: 16,
    },
    NttPrime {
        modulus: 4_292_018_177,
        primitive_root: 5,
        max_power_of_two: 16,
    },
];

/// Whether the exact CRT convolution supports a linear product of this length.
#[must_use]
pub fn crt_ntt_supports_convolution_len(convolution_len: usize) -> bool {
    if convolution_len == 0 || !convolution_len.is_power_of_two() {
        return false;
    }
    let required_log = convolution_len.ilog2();
    CRT_NTT_PRIMES
        .iter()
        .all(|prime| prime.max_power_of_two >= required_log)
}

/// Primitive `len`-th root of unity modulo a helper prime.
///
/// Returns `None` when `len` is zero, not a power of two, or longer than the prime supports.
#[must_use]
pub fn root_for_length(prime: NttPrime, len: usize) -> Option<u64> {
    if len == 0 || prime.modulus <= 1 {
        return None;
    }
    let log_len = len.ilog2();
    if !len.is_power_of_two() || log_len > prime.max_power_of_two {
        return None;
    }
    let len = u64::try_from(len).ok()?;
    Some(mod_pow_u64(
        prime.primitive_root,
        (prime.modulus - 1) / len,
        prime.modulus,
    ))
}

/// Linear convolution of two coefficient vectors modulo one helper prime.
///
/// Operands are reduced modulo the prime and zero-padded to `len`. Scratch
/// buffers clear on drop.
#[must_use]
pub fn convolve_linear_mod_prime(
    lhs: &[u64],
    rhs: &[u64],
    len: usize,
    prime: NttPrime,
) -> Option<Vec<u64>> {
    if len == 0 || !len.is_power_of_two() || len.ilog2() > prime.max_power_of_two {
        return None;
    }
    let modulus = prime.modulus;
    let root = root_for_length(prime, len)?;
    let mut lhs_ntt = Zeroizing::new(vec![0_u64; len]);
    let mut rhs_ntt = Zeroizing::new(vec![0_u64; len]);
    for (slot, &coefficient) in lhs_ntt.iter_mut().zip(lhs) {
        *slot = coefficient % modulus;
    }
    for (slot, &coefficient) in rhs_ntt.iter_mut().zip(rhs) {
        *slot = coefficient % modulus;
    }
    cyclic_ntt_in_place(&mut lhs_ntt, root, modulus, false)?;
    cyclic_ntt_in_place(&mut rhs_ntt, root, modulus, false)?;
    accel::mul_mod_assign(&mut lhs_ntt, &rhs_ntt, modulus);
    cyclic_ntt_in_place(&mut lhs_ntt, root, modulus, true)?;
    Some(std::mem::take(&mut *lhs_ntt))
}

/// Mixed-radix (Garner) recombination over helper primes.
///
/// Returns `None` when the lengths differ or the value does not fit `u128`.
#[must_use]
pub fn garner_reconstruct_u128(residues: &[u64], primes: &[NttPrime]) -> Option<u128> {
    if residues.len() != primes.len() || primes.len() > rns::MAX_CRT_LIMBS {
        return None;
    }
    let mut moduli = [0_u64; rns::MAX_CRT_LIMBS];
    for (modulus, prime) in moduli.iter_mut().zip(primes) {
        *modulus = prime.modulus;
    }
    rns::reconstruct_coefficient(residues, &moduli[..primes.len()]).ok()
}

/// Exact linear product of two equal-length word polynomials over the integers.
///
/// The product is computed modulo each helper prime and recombined, so every
/// output coefficient is the true integer coefficient when it fits `u128`.
/// The result has `2 * lhs.len()` coefficients.
///
/// # Returns
/// `None` when the doubled length is zero, not a power of two or longer than
/// the helper primes support, or a coefficient does not fit `u128`.
#[must_use]
pub fn convolve_linear_crt_ntt(lhs: &[u64], rhs: &[u64]) -> Option<Vec<u128>> {
    let len = lhs.len().checked_mul(2)?;
    if !crt_ntt_supports_convolution_len(len) {
        return None;
    }
    let mut residues = Zeroizing::new(Vec::with_capacity(CRT_NTT_PRIMES.len()));
    for prime in CRT_NTT_PRIMES {
        residues.push(convolve_linear_mod_prime(lhs, rhs, len, prime)?);
    }
    let mut output = Zeroizing::new(Vec::with_capacity(len));
    for index in 0..len {
        let coeffs = Zeroizing::new([
            residues[0][index],
            residues[1][index],
            residues[2][index],
            residues[3][index],
        ]);
        output.push(garner_reconstruct_u128(&*coeffs, &CRT_NTT_PRIMES)?);
    }
    Some(std::mem::take(&mut *output))
}

/// Exact signed negacyclic product of two degree-`n` word polynomials.
///
/// Coefficient `i` is `linear[i] - linear[i + n]` of the exact linear product:
/// the fold of `X^n = -1`. Nothing is reduced.
///
/// # Returns
/// `None` when the operands are empty or differ in length, the exact linear
/// product is unavailable, or a linear coefficient does not fit `i128`.
#[must_use]
pub fn negacyclic_product_raw_crt_ntt(lhs: &[u64], rhs: &[u64]) -> Option<Vec<i128>> {
    let n = lhs.len();
    if n == 0 || rhs.len() != n {
        return None;
    }
    let linear = Zeroizing::new(convolve_linear_crt_ntt(lhs, rhs)?);
    let mut folded = Zeroizing::new(vec![0_i128; n]);
    for (index, slot) in folded.iter_mut().enumerate() {
        let low = i128::try_from(*linear.get(index)?).ok()?;
        let high = i128::try_from(*linear.get(index + n)?).ok()?;
        *slot = low - high;
    }
    Some(std::mem::take(&mut *folded))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        constant_time::FixedModulus,
        modular::{add_mod_u64, primitive_root_of_order_with_candidate_limit, sub_mod_u64},
    };

    fn splitmix64(state: &mut u64) -> u64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = *state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }
    /// The dispatching and the scalar checked transform share this signature.
    type Transform = fn(&mut [u64], u64, u64, bool) -> Option<()>;
    fn random_poly(state: &mut u64, len: usize, modulus: u64) -> Vec<u64> {
        (0..len).map(|_| splitmix64(state) % modulus).collect()
    }
    /// Quadratic evaluation of the transform definition: `out[k] = sum_i in[i] * root^(i*k)`.
    fn naive_dft(values: &[u64], root: u64, modulus: u64) -> Vec<u64> {
        (0..values.len())
            .map(|k| {
                let step = mod_pow_u64(root, k as u64, modulus);
                let mut power = 1_u64;
                let mut sum = 0_u64;
                for &value in values {
                    sum = add_mod_u64(sum, mul_mod_u64(value, power, modulus), modulus);
                    power = mul_mod_u64(power, step, modulus);
                }
                sum
            })
            .collect()
    }
    fn naive_negacyclic(lhs: &[u64], rhs: &[u64], modulus: u64) -> Vec<u64> {
        let n = lhs.len();
        let mut output = vec![0_u64; n];
        for (i, &left) in lhs.iter().enumerate() {
            for (j, &right) in rhs.iter().enumerate() {
                let term = mul_mod_u64(left, right, modulus);
                if i + j < n {
                    output[i + j] = add_mod_u64(output[i + j], term, modulus);
                } else {
                    output[i + j - n] = sub_mod_u64(output[i + j - n], term, modulus);
                }
            }
        }
        output
    }

    #[test]
    fn bit_reversal_is_an_involution_and_matches_known_orders() {
        let mut values: Vec<u64> = (0..8).collect();
        bit_reverse_permute(&mut values);
        assert_eq!(values, [0, 4, 2, 6, 1, 5, 3, 7]);
        bit_reverse_permute(&mut values);
        assert_eq!(values, [0, 1, 2, 3, 4, 5, 6, 7]);
        let mut empty: [u64; 0] = [];
        bit_reverse_permute(&mut empty);
        let mut single = [9_u64];
        bit_reverse_permute(&mut single);
        assert_eq!(single, [9]);
    }

    #[test]
    fn cyclic_transform_matches_the_definition_and_round_trips() {
        let mut state = 0xA1_u64;
        for (modulus, max_log) in [
            (30_593_u64, 7_u32),
            (4_293_918_721, 9),
            (70_368_744_067_073, 8),
        ] {
            for log_len in 0..=max_log {
                let len = 1_usize << log_len;
                let order = len as u64;
                let root = if len == 1 {
                    1
                } else {
                    primitive_root_of_order_with_candidate_limit(modulus, order, 4_096)
                        .expect("root")
                };
                let input = random_poly(&mut state, len, modulus);
                let mut transformed = input.clone();
                cyclic_ntt_in_place(&mut transformed, root, modulus, false).expect("forward");
                assert_eq!(
                    transformed,
                    naive_dft(&input, root, modulus),
                    "modulus {modulus} len {len}"
                );
                let mut reference = input.clone();
                cyclic_ntt_in_place_scalar(&mut reference, root, modulus, false).expect("scalar");
                assert_eq!(transformed, reference);
                cyclic_ntt_in_place(&mut transformed, root, modulus, true).expect("inverse");
                assert_eq!(transformed, input);
            }
        }
    }

    #[test]
    fn cyclic_transform_reduces_unreduced_inputs_exactly() {
        let modulus = 30_593_u64;
        let root = primitive_root_of_order_with_candidate_limit(modulus, 16, 4_096).unwrap();
        let mut state = 0xA2_u64;
        let unreduced: Vec<u64> = (0..16).map(|_| splitmix64(&mut state)).collect();
        let reduced: Vec<u64> = unreduced.iter().map(|value| value % modulus).collect();
        let mut from_unreduced = unreduced;
        let mut from_reduced = reduced;
        cyclic_ntt_in_place(&mut from_unreduced, root, modulus, false).unwrap();
        cyclic_ntt_in_place(&mut from_reduced, root, modulus, false).unwrap();
        assert_eq!(from_unreduced, from_reduced);
        // A transform of length one has no butterfly; its element is still reduced.
        let transforms: [Transform; 2] = [cyclic_ntt_in_place, cyclic_ntt_in_place_scalar];
        for transform in transforms {
            for invert in [false, true] {
                for (value, modulus, expected) in [
                    (100_u64, 17_u64, 15_u64),
                    (u64::MAX, 30_593, 4_440),
                    (16, 17, 16),
                ] {
                    let mut single = [value];
                    transform(&mut single, 1, modulus, invert).expect("length one");
                    assert_eq!(single, [expected], "{value} mod {modulus} invert {invert}");
                    assert_eq!(expected, value % modulus);
                }
            }
        }
    }

    /// The root may be unreduced: the accelerated twiddle table and the scalar stage roots are
    /// built from its residue, and the inverse transform inverts its residue.
    #[test]
    fn cyclic_transform_reduces_an_unreduced_root() {
        let mut state = 0xA6_u64;
        for (modulus, len) in [
            (30_593_u64, 64_usize),
            (4_293_918_721, 256),
            (2_013_265_921, 8),
            (70_368_744_067_073, 16),
        ] {
            let root = primitive_root_of_order_with_candidate_limit(modulus, len as u64, 4_096)
                .expect("root");
            let input = random_poly(&mut state, len, modulus);
            let mut expected = input.clone();
            cyclic_ntt_in_place_scalar(&mut expected, root, modulus, false).expect("scalar");
            assert_eq!(expected, naive_dft(&input, root, modulus));
            // Largest multiple of the modulus that keeps the root in a word, and one modulus.
            for unreduced in [
                root + modulus,
                root + modulus * ((u64::MAX - root) / modulus),
            ] {
                assert_eq!(unreduced % modulus, root);
                assert!(unreduced >= modulus);
                let transforms: [Transform; 2] = [cyclic_ntt_in_place, cyclic_ntt_in_place_scalar];
                for transform in transforms {
                    let mut actual = input.clone();
                    transform(&mut actual, unreduced, modulus, false).expect("forward");
                    assert_eq!(actual, expected, "modulus {modulus} root {unreduced}");
                    transform(&mut actual, unreduced, modulus, true).expect("inverse");
                    assert_eq!(actual, input, "modulus {modulus} root {unreduced}");
                }
            }
        }
    }

    #[test]
    fn cyclic_transform_rejects_invalid_shapes_without_touching_the_input() {
        let original = [5_u64, 6, 7];
        for (len, root, modulus, invert) in [
            (0_usize, 3_u64, 17_u64, false),
            (3, 3, 17, false),
            (2, 3, 2, false),
            (2, 3, 1, true),
            (2, 17, 17, true),
            (2, 0, 17, true),
        ] {
            let mut values = original[..len].to_vec();
            assert_eq!(
                cyclic_ntt_in_place(&mut values, root, modulus, invert),
                None
            );
            assert_eq!(values, original[..len]);
            assert_eq!(
                cyclic_ntt_in_place_scalar(&mut values, root, modulus, invert),
                None
            );
            assert_eq!(values, original[..len]);
        }
        // A length that is a multiple of the modulus has no inverse.
        let mut values = vec![1_u64; 4];
        assert_eq!(cyclic_ntt_in_place(&mut values, 3, 4, true), None);
        assert_eq!(values, [1; 4]);
    }

    /// The generic transforms reject a length that is zero or not a power of two in every build
    /// profile, before they touch the input.
    #[test]
    fn generic_transforms_reject_invalid_lengths_without_touching_the_input() {
        let word = WordModulus(17);
        let fixed = FixedModulus::derive(17).unwrap();
        for len in [0_usize, 3, 5, 6, 7, 12] {
            let original: Vec<u64> = (1..=len as u64).collect();
            let mut values = original.clone();
            assert_eq!(cyclic_ntt_with(&mut values, &word, 4), None, "len {len}");
            assert_eq!(values, original);
            assert_eq!(cyclic_ntt_with(&mut values, &fixed, 4), None, "len {len}");
            assert_eq!(values, original);
            assert_eq!(
                forward_negacyclic_ntt_with(&mut values, &word, 2),
                None,
                "len {len}"
            );
            assert_eq!(values, original);
            assert_eq!(
                forward_negacyclic_ntt_with(&mut values, &fixed, 2),
                None,
                "len {len}"
            );
            assert_eq!(values, original);
            assert_eq!(
                inverse_negacyclic_ntt_with(&mut values, &word, 9, 13),
                None,
                "len {len}"
            );
            assert_eq!(values, original);
        }
        // Powers of two are accepted, including length one (the identity).
        let mut single = [5_u64];
        assert_eq!(cyclic_ntt_with(&mut single, &word, 1), Some(()));
        assert_eq!(single, [5]);
        let mut pair = [5_u64, 7];
        assert_eq!(cyclic_ntt_with(&mut pair, &word, 16), Some(()));
        assert_eq!(pair, [12, 15]);
    }

    #[test]
    fn generic_transform_agrees_between_word_and_montgomery_arithmetic() {
        let modulus = 1_125_899_906_840_833_u64;
        let psi = 900_675_728_376_939_u64;
        let fixed = FixedModulus::derive(modulus).unwrap();
        let word = WordModulus(modulus);
        let mut state = 0xA3_u64;
        let input = random_poly(&mut state, 64, modulus);
        let mut by_word = input.clone();
        let mut by_fixed = input.clone();
        forward_negacyclic_ntt_with(&mut by_word, &word, psi).expect("power-of-two length");
        forward_negacyclic_ntt_with(&mut by_fixed, &fixed, psi).expect("power-of-two length");
        assert_eq!(by_word, by_fixed);
        let inverse_psi = mod_inv_prime_u64(psi, modulus).unwrap();
        let inverse_degree = mod_inv_prime_u64(64, modulus).unwrap();
        assert_eq!(
            inverse_psi, 477_721_967_291_069,
            "pinned Bootle-Lantern inverse root"
        );
        assert_eq!(
            inverse_degree, 1_108_307_720_796_445,
            "pinned Bootle-Lantern inverse degree"
        );
        inverse_negacyclic_ntt_with(&mut by_word, &word, inverse_psi, inverse_degree)
            .expect("power-of-two length");
        inverse_negacyclic_ntt_with(&mut by_fixed, &fixed, inverse_psi, inverse_degree)
            .expect("power-of-two length");
        assert_eq!(by_word, input);
        assert_eq!(by_fixed, input);
    }

    #[test]
    fn twist_multiplies_by_successive_powers() {
        let modulus = 17_u64;
        let mut values = [1_u64, 1, 1, 1, 2];
        twist_in_place_with(&mut values, &WordModulus(modulus), 3);
        assert_eq!(values, [1, 3, 9, 10, 2 * 13 % 17]);
    }

    #[test]
    fn negacyclic_product_matches_schoolbook_and_wraps_with_a_negative_sign() {
        let mut state = 0xA4_u64;
        for (modulus, degree) in [
            (30_593_u64, 64_usize),
            (35_969, 64),
            (2_013_265_921, 8),
            (70_368_744_067_073, 1024),
        ] {
            let psi =
                primitive_root_of_order_with_candidate_limit(modulus, 2 * degree as u64, 4_096)
                    .expect("psi");
            let lhs = random_poly(&mut state, degree, modulus);
            let rhs = random_poly(&mut state, degree, modulus);
            let product = negacyclic_multiply_ntt(&lhs, &rhs, psi, modulus).expect("product");
            if degree <= 64 {
                assert_eq!(product, naive_negacyclic(&lhs, &rhs, modulus));
            }
            // X^(n-1) * X = X^n = -1.
            let mut x_top = vec![0_u64; degree];
            x_top[degree - 1] = 1;
            let mut x = vec![0_u64; degree];
            x[1] = 1;
            let wrapped = negacyclic_multiply_ntt(&x_top, &x, psi, modulus).expect("wrap");
            let mut expected = vec![0_u64; degree];
            expected[0] = modulus - 1;
            assert_eq!(wrapped, expected);
            // Multiplying by one is the identity.
            let mut one = vec![0_u64; degree];
            one[0] = 1;
            assert_eq!(
                negacyclic_multiply_ntt(&lhs, &one, psi, modulus).unwrap(),
                lhs
            );
        }
    }

    #[test]
    fn negacyclic_product_rejects_invalid_operands() {
        assert_eq!(negacyclic_multiply_ntt(&[], &[], 3, 17), None);
        assert_eq!(negacyclic_multiply_ntt(&[1, 2], &[1], 3, 17), None);
        assert_eq!(negacyclic_multiply_ntt(&[1, 2, 3], &[1, 2, 3], 3, 17), None);
        assert_eq!(negacyclic_multiply_ntt(&[1, 2], &[1, 2], 3, 2), None);
        assert_eq!(negacyclic_multiply_ntt(&[1, 2], &[1, 2], 0, 17), None);
        let (mut lhs, mut rhs) = ([1_u64, 2], [3_u64]);
        assert_eq!(
            negacyclic_multiply_ntt_in_place(&mut lhs, &mut rhs, 4, 17),
            None
        );
    }

    #[test]
    fn helper_primes_are_prime_with_generators_of_the_stated_two_adic_order() {
        for prime in CRT_NTT_PRIMES {
            assert!(crate::modular::is_prime_u64(prime.modulus));
            assert!((prime.modulus - 1).is_multiple_of(1 << prime.max_power_of_two));
            let len = 1_usize << prime.max_power_of_two;
            let root = root_for_length(prime, len).expect("root");
            assert!(crate::modular::is_primitive_root_of_order(
                prime.modulus,
                root,
                len as u64
            ));
        }
        let product = CRT_NTT_PRIMES.iter().try_fold(1_u128, |product, prime| {
            product.checked_mul(u128::from(prime.modulus))
        });
        assert!(product.is_some(), "helper-prime product fits u128");
    }

    /// Assertions carried over from the BFV `crt_ntt_helpers_reject_invalid_lengths_without_panic`
    /// and `crt_reconstruction_overflow_returns_none` unit tests.
    #[test]
    fn crt_helpers_reject_invalid_lengths_and_overflow_without_panic() {
        assert_eq!(convolve_linear_crt_ntt(&[], &[]), None);
        assert_eq!(convolve_linear_crt_ntt(&[1, 2, 3], &[4, 5, 6]), None);
        assert_eq!(root_for_length(CRT_NTT_PRIMES[0], 0), None);
        assert_eq!(root_for_length(CRT_NTT_PRIMES[0], 3), None);
        assert_eq!(root_for_length(CRT_NTT_PRIMES[1], 1_usize << 17), None);
        let wide_prime = NttPrime {
            modulus: u64::MAX,
            primitive_root: 2,
            max_power_of_two: 1,
        };
        assert_eq!(
            garner_reconstruct_u128(
                &[0, 0, 0, 0],
                &[wide_prime, wide_prime, wide_prime, wide_prime]
            ),
            None
        );
        assert_eq!(garner_reconstruct_u128(&[0, 0], &CRT_NTT_PRIMES), None);
        assert_eq!(
            convolve_linear_mod_prime(&[1], &[1], 3, CRT_NTT_PRIMES[0]),
            None
        );
        assert_eq!(
            convolve_linear_mod_prime(&[1], &[1], 0, CRT_NTT_PRIMES[0]),
            None
        );
        assert_eq!(
            convolve_linear_mod_prime(&[1], &[1], 1 << 17, CRT_NTT_PRIMES[1]),
            None
        );
        assert_eq!(negacyclic_product_raw_crt_ntt(&[], &[]), None);
        assert_eq!(negacyclic_product_raw_crt_ntt(&[1, 2], &[1]), None);
        assert!(!crt_ntt_supports_convolution_len(0));
        assert!(!crt_ntt_supports_convolution_len(6));
        assert!(crt_ntt_supports_convolution_len(1 << 16));
        assert!(!crt_ntt_supports_convolution_len(1 << 17));
    }

    #[test]
    fn exact_convolution_matches_the_integer_schoolbook_product() {
        let mut state = 0xA5_u64;
        for (degree, bound) in [
            (4_usize, u64::MAX),
            (64, 1 << 40),
            (64, 269_484_032),
            (256, 1 << 50),
        ] {
            let lhs: Vec<u64> = (0..degree)
                .map(|_| splitmix64(&mut state) % bound)
                .collect();
            let rhs: Vec<u64> = (0..degree)
                .map(|_| splitmix64(&mut state) % bound)
                .collect();
            let mut linear = vec![0_u128; 2 * degree];
            let mut overflow = false;
            for (i, &left) in lhs.iter().enumerate() {
                for (j, &right) in rhs.iter().enumerate() {
                    match linear[i + j].checked_add(u128::from(left) * u128::from(right)) {
                        Some(sum) => linear[i + j] = sum,
                        None => overflow = true,
                    }
                }
            }
            if overflow {
                continue;
            }
            // The helper-prime product bounds what the recombination can represent.
            let helper_product = CRT_NTT_PRIMES
                .iter()
                .fold(1_u128, |product, prime| product * u128::from(prime.modulus));
            if linear
                .iter()
                .any(|&coefficient| coefficient >= helper_product)
            {
                continue;
            }
            assert_eq!(convolve_linear_crt_ntt(&lhs, &rhs).expect("exact"), linear);
            let folded: Vec<i128> = (0..degree)
                .map(|index| {
                    i128::try_from(linear[index]).unwrap()
                        - i128::try_from(linear[index + degree]).unwrap()
                })
                .collect();
            assert_eq!(
                negacyclic_product_raw_crt_ntt(&lhs, &rhs).expect("folded"),
                folded
            );
        }
    }
}
