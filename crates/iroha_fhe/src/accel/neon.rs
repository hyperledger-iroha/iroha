//! `AArch64` NEON kernels: two 64-bit lanes, Montgomery products with `R = 2^32`.
//!
//! NEON is part of the baseline of every supported `AArch64` target, but the
//! intrinsics still require an explicit `#[target_feature]` context, so every
//! entry point checks CPU support at run time and reports `false` when the
//! scalar path must run. The only `unsafe` code is the call into a
//! `#[target_feature]` function after that check; lanes are assembled and read
//! back with safe lane intrinsics.
use super::{Montgomery32, first_stage, montgomery_twiddles};
use core::arch::aarch64::{
    uint32x2_t, uint64x2_t, vaddq_u64, vandq_u64, vcgeq_u64, vcltq_u64, vcombine_u64, vcreate_u64,
    vdup_n_u32, vdupq_n_u64, vgetq_lane_u64, vmovn_u64, vmul_u32, vmull_u32, vshrq_n_u64,
    vsubq_u64, vtstq_u64,
};

const LANES: usize = 2;

/// Whether this CPU supports the NEON kernels.
pub(super) fn is_available() -> bool {
    std::arch::is_aarch64_feature_detected!("neon")
}

#[derive(Clone, Copy)]
struct Lanes {
    modulus: uint64x2_t,
    modulus32: uint32x2_t,
    negative_inverse: uint32x2_t,
    low_mask: uint64x2_t,
}

impl Lanes {
    #[target_feature(enable = "neon")]
    fn new(constants: Montgomery32) -> Self {
        Self {
            modulus: vdupq_n_u64(constants.modulus),
            // The modulus is below 2^32, so its low word is the modulus.
            modulus32: vmovn_u64(vdupq_n_u64(constants.modulus)),
            negative_inverse: vdup_n_u32(constants.negative_inverse),
            low_mask: vdupq_n_u64(0xFFFF_FFFF),
        }
    }

    /// `lhs * rhs * 2^-32 mod modulus` for lanes below `2^32` whose product is below `modulus * 2^32`.
    #[target_feature(enable = "neon")]
    fn multiply_reduce(self, lhs: uint64x2_t, rhs: uint64x2_t) -> uint64x2_t {
        let product = vmull_u32(vmovn_u64(lhs), vmovn_u64(rhs));
        let correction = vmul_u32(vmovn_u64(product), self.negative_inverse);
        let multiple = vmull_u32(correction, self.modulus32);
        // The low words of `product` and `multiple` sum to 0 or 2^32; the carry is one exactly
        // when the low word of `product` is non-zero. An all-ones lane subtracts as +1.
        let carry = vtstq_u64(product, self.low_mask);
        let reduced = vsubq_u64(
            vaddq_u64(vshrq_n_u64::<32>(product), vshrq_n_u64::<32>(multiple)),
            carry,
        );
        self.reduce_once(reduced)
    }

    #[target_feature(enable = "neon")]
    fn reduce_once(self, value: uint64x2_t) -> uint64x2_t {
        vsubq_u64(
            value,
            vandq_u64(vcgeq_u64(value, self.modulus), self.modulus),
        )
    }

    #[target_feature(enable = "neon")]
    fn add(self, lhs: uint64x2_t, rhs: uint64x2_t) -> uint64x2_t {
        self.reduce_once(vaddq_u64(lhs, rhs))
    }

    #[target_feature(enable = "neon")]
    fn sub(self, lhs: uint64x2_t, rhs: uint64x2_t) -> uint64x2_t {
        vaddq_u64(
            vsubq_u64(lhs, rhs),
            vandq_u64(vcltq_u64(lhs, rhs), self.modulus),
        )
    }
}

#[target_feature(enable = "neon")]
fn load(values: &[u64], index: usize) -> uint64x2_t {
    vcombine_u64(vcreate_u64(values[index]), vcreate_u64(values[index + 1]))
}

#[target_feature(enable = "neon")]
fn store(values: &mut [u64], index: usize, lanes: uint64x2_t) {
    values[index] = vgetq_lane_u64::<0>(lanes);
    values[index + 1] = vgetq_lane_u64::<1>(lanes);
}

/// Forward cyclic transform of canonical values; `root` is canonical.
#[target_feature(enable = "neon")]
fn cyclic_ntt(values: &mut [u64], root: u64, constants: Montgomery32) {
    let len = values.len();
    crate::ntt::bit_reverse_permute(values);
    let twiddles = montgomery_twiddles(len, root, constants);
    let lanes = Lanes::new(constants);
    first_stage(values, constants.modulus);
    let mut stage_len = 4_usize;
    while stage_len <= len {
        let half = stage_len / 2;
        let stride = len / stage_len;
        for chunk in values.chunks_exact_mut(stage_len) {
            let (lo, hi) = chunk.split_at_mut(half);
            for index in (0..half).step_by(LANES) {
                let twiddle = vcombine_u64(
                    vcreate_u64(twiddles[index * stride]),
                    vcreate_u64(twiddles[(index + 1) * stride]),
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
#[target_feature(enable = "neon")]
fn mul_mod_assign(lhs: &mut [u64], rhs: &[u64], constants: Montgomery32) {
    let lanes = Lanes::new(constants);
    let r_squared = vdupq_n_u64(constants.r_squared);
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
#[target_feature(enable = "neon")]
fn mul_scalar_mod(values: &mut [u64], scalar: u64, constants: Montgomery32) {
    let lanes = Lanes::new(constants);
    let scalar_montgomery = vdupq_n_u64(constants.to_montgomery(scalar));
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
#[target_feature(enable = "neon")]
fn add_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64) {
    let modulus_lanes = vdupq_n_u64(modulus);
    let vector_len = lhs.len() - lhs.len() % LANES;
    for index in (0..vector_len).step_by(LANES) {
        let sum = vaddq_u64(load(lhs, index), load(rhs, index));
        let reduced = vsubq_u64(sum, vandq_u64(vcgeq_u64(sum, modulus_lanes), modulus_lanes));
        store(lhs, index, reduced);
    }
    for index in vector_len..lhs.len() {
        lhs[index] = crate::constant_time::add_mod_canonical_u64(lhs[index], rhs[index], modulus);
    }
}

/// `lhs[i] = lhs[i] - rhs[i] mod modulus` for canonical operands and a modulus below `2^62`.
#[target_feature(enable = "neon")]
fn sub_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64) {
    let modulus_lanes = vdupq_n_u64(modulus);
    let vector_len = lhs.len() - lhs.len() % LANES;
    for index in (0..vector_len).step_by(LANES) {
        let (left, right) = (load(lhs, index), load(rhs, index));
        let difference = vaddq_u64(
            vsubq_u64(left, right),
            vandq_u64(vcltq_u64(left, right), modulus_lanes),
        );
        store(lhs, index, difference);
    }
    for index in vector_len..lhs.len() {
        lhs[index] = crate::constant_time::sub_mod_canonical_u64(lhs[index], rhs[index], modulus);
    }
}

macro_rules! detected {
    ($(#[$meta:meta])* $name:ident => $kernel:ident($($argument:ident: $kind:ty),*)) => {
        $(#[$meta])*
        #[allow(unsafe_code)]
        pub(super) fn $name($($argument: $kind),*) -> bool {
            if !is_available() {
                return false;
            }
            // SAFETY: NEON support was detected on this CPU immediately above, which is the only
            // requirement of the `#[target_feature(enable = "neon")]` kernel.
            unsafe { $kernel($($argument),*) };
            true
        }
    };
}

detected!(
    /// Forward cyclic transform of canonical values when NEON is available.
    try_cyclic_ntt => cyclic_ntt(values: &mut [u64], root: u64, constants: Montgomery32)
);
detected!(
    /// Pointwise product of canonical operands when NEON is available.
    try_mul_mod_assign => mul_mod_assign(lhs: &mut [u64], rhs: &[u64], constants: Montgomery32)
);
detected!(
    /// Scalar product of canonical operands when NEON is available.
    try_mul_scalar_mod => mul_scalar_mod(values: &mut [u64], scalar: u64, constants: Montgomery32)
);
detected!(
    /// Pointwise sum of canonical operands when NEON is available.
    try_add_mod_assign => add_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64)
);
detected!(
    /// Pointwise difference of canonical operands when NEON is available.
    try_sub_mod_assign => sub_mod_assign(lhs: &mut [u64], rhs: &[u64], modulus: u64)
);
