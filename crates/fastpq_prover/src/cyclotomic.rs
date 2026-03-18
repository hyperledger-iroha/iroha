//! Cyclotomic FFT utilities over the Goldilocks field.
//!
//! This module provides minimal FFT primitives needed for the FASTPQ
//! prover. Implementations are intentionally straightforward; they can be
//! replaced with a production-grade backend (e.g., hedgehog/dasher) once
//! those crates land in the workspace.

#![allow(dead_code)]

use rayon::prelude::*;

use crate::poseidon::FIELD_MODULUS as GOLDILOCKS_MODULUS;

const PARALLEL_THRESHOLD: usize = 1 << 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Domain {
    pub log_size: u32,
    pub generator: u64,
}

impl Domain {
    pub fn size(&self) -> usize {
        1usize << self.log_size
    }

    pub fn element(&self, index: usize) -> u64 {
        pow_mod(self.generator, index)
    }
}

pub fn fft(values: &mut [u64], domain: Domain) {
    let n = values.len();
    assert_eq!(n, domain.size(), "values must match domain size");
    bit_reverse(values);

    let mut len = 2usize;
    for _ in 0..domain.log_size {
        let half = len / 2;
        let step = n / len;
        let stage_root = pow_mod(domain.generator, step);
        apply_stage(values, len, half, stage_root);
        len <<= 1;
    }
}

pub fn ifft(values: &mut [u64], domain: Domain) {
    let inv_domain = Domain {
        log_size: domain.log_size,
        generator: multiplicative_inverse(domain.generator),
    };
    fft(values, inv_domain);
    let inv_size = multiplicative_inverse(domain.size() as u64);
    for value in values.iter_mut() {
        *value = mul_mod(*value, inv_size);
    }
}

fn apply_stage(values: &mut [u64], len: usize, half: usize, stage_root: u64) {
    let worker = |chunk: &mut [u64]| {
        let mut twiddle = 1u64;
        for i in 0..half {
            let u = chunk[i];
            let v = mul_mod(chunk[i + half], twiddle);
            chunk[i] = add_mod(u, v);
            chunk[i + half] = sub_mod(u, v);
            twiddle = mul_mod(twiddle, stage_root);
        }
    };

    if len < PARALLEL_THRESHOLD {
        for chunk in values.chunks_mut(len) {
            worker(chunk);
        }
    } else {
        values.par_chunks_mut(len).for_each(worker);
    }
}

fn bit_reverse(values: &mut [u64]) {
    let n = values.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            values.swap(i, j);
        }
    }
}

#[inline]
fn add_mod(a: u64, b: u64) -> u64 {
    let sum = a.wrapping_add(b);
    let mut result = sum;
    if sum < a {
        result = result.wrapping_sub(GOLDILOCKS_MODULUS);
    }
    if result >= GOLDILOCKS_MODULUS {
        result - GOLDILOCKS_MODULUS
    } else {
        result
    }
}

#[inline]
fn sub_mod(a: u64, b: u64) -> u64 {
    if a >= b {
        a - b
    } else {
        GOLDILOCKS_MODULUS - (b - a)
    }
}

#[inline]
fn mul_mod(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    let lo = u64::try_from(product & u128::from(u64::MAX)).expect("masked to 64 bits");
    let hi = u64::try_from(product >> 64).expect("shifted value fits in u64");
    reduce_wide(lo, hi)
}

#[inline]
fn reduce_wide(lo: u64, hi: u64) -> u64 {
    let hi_lo = i128::from(hi & 0xffff_ffff);
    let hi_hi = i128::from(hi >> 32);
    let mut acc = i128::from(lo);
    acc += hi_lo << 32;
    acc -= hi_lo;
    acc -= hi_hi;
    let modulus = i128::from(GOLDILOCKS_MODULUS);
    if acc < 0 {
        acc += modulus;
        if acc < 0 {
            acc += modulus;
        }
    }
    if acc >= modulus {
        acc -= modulus;
        if acc >= modulus {
            acc -= modulus;
        }
    }
    u64::try_from(acc).expect("reduced value fits in u64")
}

fn pow_mod(base: u64, exponent: usize) -> u64 {
    let mut result = 1u64;
    let mut power = base;
    let mut exp = exponent;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, power);
        }
        power = mul_mod(power, power);
        exp >>= 1;
    }
    result
}

fn multiplicative_inverse(value: u64) -> u64 {
    power_mod(value, GOLDILOCKS_MODULUS - 2)
}

fn power_mod(base: u64, exponent: u64) -> u64 {
    let mut result = 1u64;
    let mut power = base;
    let mut exp = exponent;
    while exp > 0 {
        if exp & 1 == 1 {
            result = mul_mod(result, power);
        }
        power = mul_mod(power, power);
        exp >>= 1;
    }
    result
}
