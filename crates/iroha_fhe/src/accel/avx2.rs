//! x86-64 AVX2 kernels: four 64-bit lanes, Montgomery products with `R = 2^32`.
//!
//! AVX2 is not part of the x86-64 baseline, so every entry point checks CPU
//! support at run time and reports `false` when the scalar path must run. The
//! only `unsafe` code is the call into a `#[target_feature]` function after
//! that check; lanes are assembled and read back with safe intrinsics.
//!
//! On every other architecture this file is compiled for tests only, against
//! the lane model in [`super::avx2_model`]. The schedule below (lane assembly,
//! the Montgomery carry, the signed compares, the stage of length four, the
//! twiddle gather and the scalar tails) is therefore executed and compared
//! with the scalar reference on every host. x86-64 hosts run it with the
//! CPU's own instructions and check the model against them.
// TODO: record a run of these kernels on a physical x86-64 CPU. The CI foundation lane builds
// and tests this crate on x86-64 runners; on the AArch64 development host the kernels have run
// over the lane model and under QEMU's x86-64 emulation only.
// The lane model keeps the names of the intrinsics it stands in for.
#![cfg_attr(not(target_arch = "x86_64"), allow(clippy::used_underscore_items))]
#[cfg(not(target_arch = "x86_64"))]
use super::avx2_model::{
    _mm256_add_epi64, _mm256_and_si256, _mm256_andnot_si256, _mm256_cmpeq_epi64,
    _mm256_cmpgt_epi64, _mm256_extract_epi64, _mm256_mul_epu32, _mm256_set_epi64x,
    _mm256_set1_epi64x, _mm256_setzero_si256, _mm256_srli_epi64, _mm256_sub_epi64, Vector,
};
use super::{Montgomery32, first_stage, montgomery_twiddles};
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::{
    __m256i as Vector, _mm256_add_epi64, _mm256_and_si256, _mm256_andnot_si256, _mm256_cmpeq_epi64,
    _mm256_cmpgt_epi64, _mm256_extract_epi64, _mm256_mul_epu32, _mm256_set_epi64x,
    _mm256_set1_epi64x, _mm256_setzero_si256, _mm256_srli_epi64, _mm256_sub_epi64,
};

const LANES: usize = 4;

/// Whether the kernels can run here: the CPU supports AVX2, or the lane model stands in for it.
pub(super) fn is_available() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        std::arch::is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        true
    }
}

#[derive(Clone, Copy)]
struct Lanes {
    modulus: Vector,
    negative_inverse: Vector,
    low_mask: Vector,
    one: Vector,
}

impl Lanes {
    #[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
    fn new(constants: Montgomery32) -> Self {
        Self {
            modulus: _mm256_set1_epi64x(constants.modulus.cast_signed()),
            negative_inverse: _mm256_set1_epi64x(i64::from(constants.negative_inverse)),
            low_mask: _mm256_set1_epi64x(0xFFFF_FFFF),
            one: _mm256_set1_epi64x(1),
        }
    }

    /// `lhs * rhs * 2^-32 mod modulus` for lanes below `2^32` whose product is below `modulus * 2^32`.
    #[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
    fn multiply_reduce(self, lhs: Vector, rhs: Vector) -> Vector {
        // `_mm256_mul_epu32` multiplies the low 32 bits of each 64-bit lane.
        let product = _mm256_mul_epu32(lhs, rhs);
        let correction = _mm256_mul_epu32(product, self.negative_inverse);
        let multiple = _mm256_mul_epu32(correction, self.modulus);
        // The low words of `product` and `multiple` sum to 0 or 2^32; the carry is one exactly
        // when the low word of `product` is non-zero. A zero low word compares to all ones (-1).
        let low_is_zero = _mm256_cmpeq_epi64(
            _mm256_and_si256(product, self.low_mask),
            _mm256_setzero_si256(),
        );
        let carry = _mm256_add_epi64(self.one, low_is_zero);
        let reduced = _mm256_add_epi64(
            _mm256_add_epi64(
                _mm256_srli_epi64::<32>(product),
                _mm256_srli_epi64::<32>(multiple),
            ),
            carry,
        );
        self.reduce_once(reduced)
    }

    /// Lanes are below `2^63`, so the signed comparison orders them as unsigned values.
    #[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
    fn reduce_once(self, value: Vector) -> Vector {
        let below = _mm256_cmpgt_epi64(self.modulus, value);
        _mm256_sub_epi64(value, _mm256_andnot_si256(below, self.modulus))
    }

    #[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
    fn add(self, lhs: Vector, rhs: Vector) -> Vector {
        self.reduce_once(_mm256_add_epi64(lhs, rhs))
    }

    #[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
    fn sub(self, lhs: Vector, rhs: Vector) -> Vector {
        let borrow = _mm256_cmpgt_epi64(rhs, lhs);
        _mm256_add_epi64(
            _mm256_sub_epi64(lhs, rhs),
            _mm256_and_si256(borrow, self.modulus),
        )
    }
}

#[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
fn load(values: &[u64], index: usize) -> Vector {
    _mm256_set_epi64x(
        values[index + 3].cast_signed(),
        values[index + 2].cast_signed(),
        values[index + 1].cast_signed(),
        values[index].cast_signed(),
    )
}

#[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
fn store(values: &mut [u64], index: usize, lanes: Vector) {
    values[index] = _mm256_extract_epi64::<0>(lanes).cast_unsigned();
    values[index + 1] = _mm256_extract_epi64::<1>(lanes).cast_unsigned();
    values[index + 2] = _mm256_extract_epi64::<2>(lanes).cast_unsigned();
    values[index + 3] = _mm256_extract_epi64::<3>(lanes).cast_unsigned();
}

/// Forward cyclic transform of canonical values; `root` is canonical.
#[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
fn cyclic_ntt(values: &mut [u64], root: u64, constants: Montgomery32) {
    let len = values.len();
    let modulus = constants.modulus;
    crate::ntt::bit_reverse_permute(values);
    let twiddles = montgomery_twiddles(len, root, constants);
    let lanes = Lanes::new(constants);
    first_stage(values, modulus);
    // The stage of length four has two butterflies per chunk: too short for four lanes.
    if len >= 4 {
        let quarter_turn = crate::modular::mod_pow_u64(root, (len / 4) as u64, modulus);
        for chunk in values.chunks_exact_mut(4) {
            for (index, factor) in [(0_usize, 1_u64), (1, quarter_turn)] {
                let (left, right) = (chunk[index], chunk[index + 2]);
                // Single-word arithmetic: values and twiddles are below 2^32.
                let product = right * factor % modulus;
                chunk[index] = crate::constant_time::add_mod_canonical_u64(left, product, modulus);
                chunk[index + 2] =
                    crate::constant_time::sub_mod_canonical_u64(left, product, modulus);
            }
        }
    }
    let mut stage_len = 8_usize;
    while stage_len <= len {
        let half = stage_len / 2;
        let stride = len / stage_len;
        for chunk in values.chunks_exact_mut(stage_len) {
            let (lo, hi) = chunk.split_at_mut(half);
            for index in (0..half).step_by(LANES) {
                let twiddle = _mm256_set_epi64x(
                    twiddles[(index + 3) * stride].cast_signed(),
                    twiddles[(index + 2) * stride].cast_signed(),
                    twiddles[(index + 1) * stride].cast_signed(),
                    twiddles[index * stride].cast_signed(),
                );
                let left = load(lo, index);
                let product = lanes.multiply_reduce(load(hi, index), twiddle);
                store(lo, index, lanes.add(left, product));
                store(hi, index, lanes.sub(left, product));
            }
        }
        stage_len *= 2;
    }
}

/// `lhs[i] = lhs[i] * rhs[i] mod modulus` for canonical operands of equal length.
#[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
fn mul_mod_assign(lhs: &mut [u64], rhs: &[u64], constants: Montgomery32) {
    let lanes = Lanes::new(constants);
    let r_squared = _mm256_set1_epi64x(constants.r_squared.cast_signed());
    let vector_len = lhs.len() - lhs.len() % LANES;
    for index in (0..vector_len).step_by(LANES) {
        let reduced = lanes.multiply_reduce(load(lhs, index), load(rhs, index));
        store(lhs, index, lanes.multiply_reduce(reduced, r_squared));
    }
    for index in vector_len..lhs.len() {
        lhs[index] = lhs[index] * rhs[index] % constants.modulus;
    }
}

/// `values[i] = values[i] * scalar mod modulus` for canonical operands.
#[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
fn mul_scalar_mod(values: &mut [u64], scalar: u64, constants: Montgomery32) {
    let lanes = Lanes::new(constants);
    let scalar_montgomery = _mm256_set1_epi64x(constants.to_montgomery(scalar).cast_signed());
    let vector_len = values.len() - values.len() % LANES;
    for index in (0..vector_len).step_by(LANES) {
        let product = lanes.multiply_reduce(load(values, index), scalar_montgomery);
        store(values, index, product);
    }
    for value in &mut values[vector_len..] {
        *value = *value * scalar % constants.modulus;
    }
}

/// `lhs[i] = lhs[i] + rhs[i] mod modulus` for canonical operands and a modulus below `2^62`.
#[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
fn add_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64) {
    let modulus_lanes = _mm256_set1_epi64x(modulus.cast_signed());
    let vector_len = lhs.len() - lhs.len() % LANES;
    for index in (0..vector_len).step_by(LANES) {
        let sum = _mm256_add_epi64(load(lhs, index), load(rhs, index));
        let below = _mm256_cmpgt_epi64(modulus_lanes, sum);
        store(
            lhs,
            index,
            _mm256_sub_epi64(sum, _mm256_andnot_si256(below, modulus_lanes)),
        );
    }
    for index in vector_len..lhs.len() {
        lhs[index] = crate::constant_time::add_mod_canonical_u64(lhs[index], rhs[index], modulus);
    }
}

/// `lhs[i] = lhs[i] - rhs[i] mod modulus` for canonical operands and a modulus below `2^62`.
#[cfg_attr(target_arch = "x86_64", target_feature(enable = "avx2"))]
fn sub_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64) {
    let modulus_lanes = _mm256_set1_epi64x(modulus.cast_signed());
    let vector_len = lhs.len() - lhs.len() % LANES;
    for index in (0..vector_len).step_by(LANES) {
        let (left, right) = (load(lhs, index), load(rhs, index));
        let borrow = _mm256_cmpgt_epi64(right, left);
        store(
            lhs,
            index,
            _mm256_add_epi64(
                _mm256_sub_epi64(left, right),
                _mm256_and_si256(borrow, modulus_lanes),
            ),
        );
    }
    for index in vector_len..lhs.len() {
        lhs[index] = crate::constant_time::sub_mod_canonical_u64(lhs[index], rhs[index], modulus);
    }
}

macro_rules! detected {
    ($(#[$meta:meta])* $name:ident => $kernel:ident($($argument:ident: $kind:ty),*)) => {
        $(#[$meta])*
        #[cfg_attr(target_arch = "x86_64", allow(unsafe_code))]
        pub(super) fn $name($($argument: $kind),*) -> bool {
            if !is_available() {
                return false;
            }
            #[cfg(target_arch = "x86_64")]
            // SAFETY: AVX2 support was detected on this CPU immediately above, which is the only
            // requirement of the `#[target_feature(enable = "avx2")]` kernel.
            unsafe {
                $kernel($($argument),*)
            };
            // The lane model has no CPU requirement.
            #[cfg(not(target_arch = "x86_64"))]
            $kernel($($argument),*);
            true
        }
    };
}

detected!(
    /// Forward cyclic transform of canonical values when AVX2 is available.
    try_cyclic_ntt => cyclic_ntt(values: &mut [u64], root: u64, constants: Montgomery32)
);
detected!(
    /// Pointwise product of canonical operands when AVX2 is available.
    try_mul_mod_assign => mul_mod_assign(lhs: &mut [u64], rhs: &[u64], constants: Montgomery32)
);
detected!(
    /// Scalar product of canonical operands when AVX2 is available.
    try_mul_scalar_mod => mul_scalar_mod(values: &mut [u64], scalar: u64, constants: Montgomery32)
);
detected!(
    /// Pointwise sum of canonical operands when AVX2 is available.
    try_add_mod_assign => add_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64)
);
detected!(
    /// Pointwise difference of canonical operands when AVX2 is available.
    try_sub_mod_assign => sub_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64)
);
