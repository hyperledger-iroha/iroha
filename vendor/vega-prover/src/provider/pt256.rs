// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module implements the Spartan traits for P256 and T256 curves
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
    group::{Curve, Group as AnotherGroup, cofactor::CofactorCurveAffine},
    secp256r1::{Secp256r1 as P256, Secp256r1Affine as P256Affine},
    t256::{T256, T256Affine},
};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{Num, ToPrimitive};
use rayon::prelude::*;
use sha3::Shake256;
use std::io::Read;

/// Re-exports that give access to the standard aliases used in the code base, for p256
pub mod p256 {
    pub use halo2curves::secp256r1::{
        Fp as Base, Fq as Scalar, Secp256r1 as Point, Secp256r1Affine as Affine,
    };
}

/// Re-exports that give access to the standard aliases used in the code base, for t256
pub mod t256 {
    pub use halo2curves::t256::{Fp as Base, Fq as Scalar, T256 as Point, T256Affine as Affine};
}

impl_traits!(
    p256,
    P256,
    P256Affine,
    "ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551",
    "ffffffff00000001000000000000000000000000ffffffffffffffffffffffff"
);

impl_traits!(
    t256,
    T256,
    T256Affine,
    "ffffffff00000001000000000000000000000000ffffffffffffffffffffffff",
    "ffffffff0000000100000000000000017e72b42b30e7317793135661b1c4b117"
);

// Implement big_num traits for P256/T256 scalar fields
crate::impl_field_reduction_constants!(p256::Scalar);
crate::impl_montgomery_limbs!(p256::Scalar);
crate::impl_field_reduction_constants!(t256::Scalar);
crate::impl_montgomery_limbs!(t256::Scalar);

impl<G: Group> TranscriptReprTrait<G> for t256::Base {
    fn to_transcript_bytes(&self) -> Vec<u8> {
        self.to_bytes().into_iter().rev().collect()
    }
}

#[cfg(test)]
mod big_num_tests {
    crate::test_field_reduction_constants!(p256_frc, crate::provider::pt256::p256::Scalar);
    crate::test_montgomery!(p256_mont, crate::provider::pt256::p256::Scalar);
    crate::test_delayed_reduction!(p256_dr, crate::provider::pt256::p256::Scalar);
    crate::test_field_reduction_constants!(t256_frc, crate::provider::pt256::t256::Scalar);
    crate::test_montgomery!(t256_mont, crate::provider::pt256::t256::Scalar);
    crate::test_delayed_reduction!(t256_dr, crate::provider::pt256::t256::Scalar);
    crate::test_small_value!(t256_sv, crate::provider::pt256::t256::Scalar);
    crate::test_small_value!(p256_sv, crate::provider::pt256::p256::Scalar);
}
