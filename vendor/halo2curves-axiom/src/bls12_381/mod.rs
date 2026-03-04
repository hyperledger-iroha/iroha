//! # `bls12_381`
//!
//! This crate provides an implementation of the BLS12-381 pairing-friendly elliptic
//! curve construction.
//!
//! * **This implementation has not been reviewed or audited. Use at your own risk.**
//! * This implementation targets Rust `1.36` or later.
//! * This implementation does not require the Rust standard library.
//! * All operations are constant time unless explicitly noted.
//! Source: <https://github.com/privacy-scaling-explorations/bls12_381>

// Catch documentation errors caused by code changes.
#![allow(clippy::too_many_arguments)]
#![allow(clippy::many_single_char_names)]
// This lint is described at
// https://rust-lang.github.io/rust-clippy/master/index.html#suspicious_arithmetic_impl
// In our library, some of the arithmetic involving extension fields will necessarily
// involve various binary operators, and so this lint is triggered unnecessarily.
#![allow(clippy::suspicious_arithmetic_impl)]

// #[macro_use]
// mod util;

mod scalar;

pub use fp::Fp as Fq;
pub use scalar::Scalar as Fr;

use scalar::Scalar;

mod fp;
mod fp2;
mod g1;
mod g2;

use g1::G1Projective;
use g2::G2Projective;

pub use g1::{G1Affine, G1Projective as G1};
pub use g2::{G2Affine, G2Projective as G2};

mod fp12;
mod fp6;

pub use fp12::{Fp12 as Fq12, FROBENIUS_COEFF_FQ12_C1};
pub use fp2::Fp2 as Fq2;
pub use fp6::Fp6 as Fq6;

// The BLS parameter x for BLS12-381 is -0xd201000000010000
pub const BLS_X: u64 = 0xd201_0000_0001_0000;
pub const BLS_X_IS_NEGATIVE: bool = true;

mod pairings;

pub use pairings::{multi_miller_loop, G2Prepared};
pub use pairings::{pairing, Bls12, Gt, MillerLoopResult};

mod endo;

pub(crate) use digest::generic_array;
pub mod hash_to_curve;

#[cfg(test)]
mod tests;
