#![cfg_attr(not(feature = "std"), no_std)]

mod arithmetic;
mod curve;
pub mod ff_ext;
pub mod fft;
pub mod hash_to_curve;
pub mod msm;
pub mod serde;

pub mod bls12381;
pub mod bn256;
pub mod grumpkin;
pub mod pasta;
pub mod pluto_eris;
pub mod secp256k1;
pub mod secp256r1;
pub mod secq256k1;
pub mod t256;

#[macro_use]
mod derive;

// Re-export to simplify downstream dependencies.
pub use curve::{Coordinates, CurveAffine, CurveExt};
pub use ff;
pub use group;
pub use pairing;

#[cfg(test)]
pub mod tests;
