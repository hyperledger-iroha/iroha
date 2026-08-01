//! Portable Falcon-512 trapdoor and GPV sampling primitives.
//!
//! The arithmetic core is selectively vendored and adapted from Thomas
//! Pornin's `rust-fn-dsa` 0.3 implementation (Unlicense).  Only the portable
//! degree-512 key-generation and recursive ffSampling paths are compiled;
//! signature encoding, message hashing, legacy Falcon modes, and runtime
//! SIMD dispatch are deliberately outside this module.
//!
//! This is used as the concrete `[1 | h]` issuer specialization pinned by
//! LaZeR, not as an implementation of the full BLNS security reduction.

#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]

mod comm;
mod kgen;
mod sign;

use zeroize::{Zeroize, Zeroizing};

pub(super) const DEGREE: usize = 512;
pub(super) const LOG_DEGREE: u32 = 9;
pub(super) const MODULUS: u16 = 12_289;
pub(super) const SIGNATURE_NORM_SQUARED_BOUND: u32 = 34_034_726;

/// A generated Falcon-512 NTRU trapdoor and its public multiplier.
pub(super) struct Trapdoor {
    pub(super) f: Zeroizing<Box<[i8; DEGREE]>>,
    pub(super) g: Zeroizing<Box<[i8; DEGREE]>>,
    pub(super) capital_f: Zeroizing<Box<[i8; DEGREE]>>,
    pub(super) capital_g: Zeroizing<Box<[i8; DEGREE]>>,
    pub(super) h: Zeroizing<Box<[u16; DEGREE]>>,
}

impl core::fmt::Debug for Trapdoor {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("Falcon512Trapdoor(<redacted>)")
    }
}

impl Zeroize for Trapdoor {
    fn zeroize(&mut self) {
        self.f.as_mut().zeroize();
        self.g.as_mut().zeroize();
        self.capital_f.as_mut().zeroize();
        self.capital_g.as_mut().zeroize();
        self.h.as_mut().zeroize();
    }
}

impl Drop for Trapdoor {
    fn drop(&mut self) {
        self.zeroize();
    }
}

/// One short preimage `s1 + h*s2 = target (mod q)`.
pub(super) struct Preimage {
    pub(super) first: Zeroizing<Box<[i16; DEGREE]>>,
    pub(super) second: Zeroizing<Box<[i16; DEGREE]>>,
    pub(super) norm_squared: u32,
}

pub(super) fn generate_from_seed(
    seed: &[u8; 32],
    max_candidates: u32,
) -> Option<Trapdoor> {
    kgen::generate_from_seed(seed, max_candidates)
}

pub(super) fn sample_preimage_from_seed(
    trapdoor: &Trapdoor,
    target: &[u16; DEGREE],
    seed: &[u8; 56],
) -> Option<Preimage> {
    sign::sample_preimage_from_seed(trapdoor, target, seed)
}
