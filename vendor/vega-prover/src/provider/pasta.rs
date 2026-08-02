// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: MIT
// This file is part of the vega-prover project.
// See the LICENSE file in the project root for full license information.
// Source repository: https://github.com/Microsoft/vega-prover

//! This module implements the Spartan traits for `pallas::Point`, `pallas::Scalar`, `vesta::Point`, `vesta::Scalar`.
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
    pasta::{Pallas, PallasAffine, Vesta, VestaAffine},
};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{Num, ToPrimitive};
use rayon::prelude::*;
use sha3::Shake256;
use std::io::Read;

/// Re-exports that give access to the standard aliases used in the code base, for pallas
pub mod pallas {
    pub use halo2curves::pasta::{
        Fp as Base, Fq as Scalar, Pallas as Point, PallasAffine as Affine,
    };
}

/// Re-exports that give access to the standard aliases used in the code base, for vesta
pub mod vesta {
    pub use halo2curves::pasta::{Fp as Scalar, Fq as Base, Vesta as Point, VestaAffine as Affine};
}

impl_traits!(
    pallas,
    Pallas,
    PallasAffine,
    "40000000000000000000000000000000224698fc0994a8dd8c46eb2100000001",
    "40000000000000000000000000000000224698fc094cf91b992d30ed00000001"
);

impl_traits!(
    vesta,
    Vesta,
    VestaAffine,
    "40000000000000000000000000000000224698fc094cf91b992d30ed00000001",
    "40000000000000000000000000000000224698fc0994a8dd8c46eb2100000001"
);

// Implement big_num traits for Pasta scalar fields
// Pallas scalar = Fq, Vesta scalar = Fp
crate::impl_field_reduction_constants!(pallas::Scalar);
crate::impl_montgomery_limbs!(pallas::Scalar);
crate::impl_field_reduction_constants!(vesta::Scalar);
crate::impl_montgomery_limbs!(vesta::Scalar);

#[cfg(test)]
mod tests {
    use crate::provider::{pasta::pallas, traits::DlogGroup};
    use digest::{ExtendableOutput, Update};
    use halo2curves::{CurveExt, group::Curve};
    use sha3::Shake256;
    use std::io::Read;

    macro_rules! impl_cycle_pair_test {
        ($curve:ident) => {
            fn from_label_serial(label: &'static [u8], n: usize) -> Vec<$curve::Affine> {
                let mut shake = Shake256::default();
                shake.update(label);
                let mut reader = shake.finalize_xof();
                (0..n)
                    .map(|_| {
                        let mut uniform_bytes = [0u8; 32];
                        reader.read_exact(&mut uniform_bytes).unwrap();
                        let hash = $curve::Point::hash_to_curve("from_uniform_bytes");
                        hash(&uniform_bytes).to_affine()
                    })
                    .collect()
            }

            let label = b"test_from_label";
            for n in [
                1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 1021,
            ] {
                let ck_par = <$curve::Point as DlogGroup>::from_label(label, n);
                let ck_ser = from_label_serial(label, n);
                assert_eq!(ck_par.len(), n);
                assert_eq!(ck_ser.len(), n);
                assert_eq!(ck_par, ck_ser);
            }
        };
    }

    #[test]
    fn test_pallas_from_label() {
        impl_cycle_pair_test!(pallas);
    }
}

#[cfg(test)]
mod big_num_tests {
    crate::test_field_reduction_constants!(pallas_frc, crate::provider::pasta::pallas::Scalar);
    crate::test_montgomery!(pallas_mont, crate::provider::pasta::pallas::Scalar);
    crate::test_delayed_reduction!(pallas_dr, crate::provider::pasta::pallas::Scalar);
    crate::test_field_reduction_constants!(vesta_frc, crate::provider::pasta::vesta::Scalar);
    crate::test_montgomery!(vesta_mont, crate::provider::pasta::vesta::Scalar);
    crate::test_delayed_reduction!(vesta_dr, crate::provider::pasta::vesta::Scalar);
}
