// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module implements the Spartan traits for BN254 (also known as BN256 or alt_bn128).
use crate::{
    impl_traits,
    provider::{
        msm::{msm, msm_shared_weights, msm_small},
        traits::{DlogGroup, DlogGroupExt},
    },
    traits::{Group, PrimeFieldExt, transcript::TranscriptReprTrait},
};
use digest::{ExtendableOutput, Update};
use ff::FromUniformBytes;
use halo2curves::{
    CurveAffine, CurveExt,
    bn256::{G1 as Bn256G1, G1Affine as Bn256G1Affine},
    group::{Curve, Group as AnotherGroup, cofactor::CofactorCurveAffine},
};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{Num, ToPrimitive};
use rayon::prelude::*;
use sha3::Shake256;
use std::io::Read;

/// Re-exports that give access to the standard aliases used in the code base, for bn254
pub mod types {
    pub use halo2curves::bn256::{Fq as Base, Fr as Scalar, G1 as Point, G1Affine as Affine};
}

impl_traits!(
    types,
    Bn256G1,
    Bn256G1Affine,
    // Fr (scalar field) modulus
    "30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001",
    // Fq (base field) modulus
    "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47"
);

// Implement big_num traits for BN254 scalar field (Fr)
crate::impl_field_reduction_constants!(types::Scalar);
crate::impl_montgomery_limbs!(types::Scalar);

// BN254 is not a cycle pair, so we need to manually implement TranscriptReprTrait for the Base field
impl<G: Group> TranscriptReprTrait<G> for types::Base {
    fn to_transcript_bytes(&self) -> Vec<u8> {
        self.to_bytes().into_iter().rev().collect()
    }
}

#[cfg(test)]
mod big_num_tests {
    crate::test_field_reduction_constants!(scalar_frc, crate::provider::bn254::types::Scalar);
    crate::test_montgomery!(scalar_mont, crate::provider::bn254::types::Scalar);
    crate::test_delayed_reduction!(scalar_dr, crate::provider::bn254::types::Scalar);
    crate::test_small_value!(scalar_sv, crate::provider::bn254::types::Scalar);
}
