use crate::arithmetic::mul_512;
use crate::arithmetic::sbb;
use crate::arithmetic::EndoParameters;
use crate::arithmetic::{CurveAffineExt, CurveEndo};
use crate::ff::WithSmallOrderMulGroup;
use crate::ff::{Field, PrimeField};
use crate::group::Curve;
use crate::group::{prime::PrimeCurveAffine, Group, GroupEncoding};
use crate::grumpkin::Fq;
use crate::grumpkin::Fr;
use crate::hash_to_curve::svdw_hash_to_curve;
use crate::{
    endo, impl_add_binop_specify_output, impl_binops_additive, impl_binops_additive_specify_output,
    impl_binops_multiplicative, impl_binops_multiplicative_mixed, impl_sub_binop_specify_output,
    new_curve_impl,
};
use crate::{Coordinates, CurveAffine, CurveExt};
use core::cmp;
use core::fmt::Debug;
use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};
use rand::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

#[cfg(feature = "derive_serde")]
use serde::{Deserialize, Serialize};

new_curve_impl!(
    (pub),
    G1,
    G1Affine,
    false,
    Fq,
    Fr,
    (G1_GENERATOR_X, G1_GENERATOR_Y),
    G1_A,
    G1_B,
    "grumpkin_g1",
    |curve_id, domain_prefix| svdw_hash_to_curve(curve_id, domain_prefix, G1::SVDW_Z),
);

// Parameters in montgomery form taken from
// https://github.com/AztecProtocol/barretenberg/blob/97ccf76c42db581a8b8f8bfbcffe8ca015a3dd22/cpp/src/barretenberg/ecc/curves/grumpkin/grumpkin.hpp#L14
const G1_GENERATOR_X: Fq = Fq::one();
const G1_GENERATOR_Y: Fq = Fq([
    0x11b2dff1448c41d8,
    0x23d3446f21c77dc3,
    0xaa7b8cf435dfafbb,
    0x14b34cf69dc25d68,
]);
const G1_A: Fq = Fq::zero();
const G1_B: Fq = Fq([
    0xdd7056026000005a,
    0x223fa97acb319311,
    0xcc388229877910c0,
    0x034394632b724eaa,
]);

// Generated using https://github.com/ConsenSys/gnark-crypto/blob/master/ecc/utils.go
// with `bn256::Fq::ZETA`
// See https://github.com/demining/Endomorphism-Secp256k1/blob/main/README.md
// to have more details about the endomorphism.
const ENDO_PARAMS_GRUMPKIN: EndoParameters = EndoParameters {
    gamma1: [0xd91d232ec7e0b3d2, 0x2, 0, 0],
    gamma2: [0x5398fd0300ff655f, 0x4ccef014a773d2d2, 0x02, 0],
    b1: [0x89d3256894d213e2, 0, 0, 0],
    b2: [0x0be4e1541221250b, 0x6f4d8248eeb859fd, 0, 0],
};

endo!(G1, Fr, ENDO_PARAMS_GRUMPKIN);

impl group::cofactor::CofactorGroup for G1 {
    type Subgroup = G1;

    fn clear_cofactor(&self) -> Self {
        *self
    }

    fn into_subgroup(self) -> CtOption<Self::Subgroup> {
        CtOption::new(self, 1.into())
    }

    fn is_torsion_free(&self) -> Choice {
        1.into()
    }
}

impl G1 {
    const SVDW_Z: Fq = Fq::ONE;
}

#[cfg(test)]
mod tests {
    use crate::arithmetic::CurveEndo;
    use crate::grumpkin::{Fr, G1};
    use crate::CurveExt;
    use ff::{Field, PrimeField, WithSmallOrderMulGroup};
    use rand_core::OsRng;

    #[test]
    fn test_hash_to_curve() {
        crate::tests::curve::hash_to_curve_test::<G1>();
    }

    #[test]
    fn test_curve() {
        crate::tests::curve::curve_tests::<G1>();
    }

    #[test]
    fn test_endo() {
        let z_impl = Fr::ZETA;
        let z_other = Fr::from_raw([
            0xe4bd44e5607cfd48,
            0xc28f069fbb966e3d,
            0x5e6dd9e7e0acccb0,
            0x30644e72e131a029,
        ]);

        assert_eq!(z_impl * z_impl + z_impl, -Fr::ONE);
        assert_eq!(z_other * z_other + z_other, -Fr::ONE);

        let g = G1::generator();
        assert_eq!(g * Fr::ZETA, g.endo());

        for _ in 0..100000 {
            let k = Fr::random(OsRng);
            let (k1, k1_neg, k2, k2_neg) = G1::decompose_scalar(&k);
            if k1_neg & k2_neg {
                assert_eq!(k, -Fr::from_u128(k1) + Fr::ZETA * Fr::from_u128(k2))
            } else if k1_neg {
                assert_eq!(k, -Fr::from_u128(k1) - Fr::ZETA * Fr::from_u128(k2))
            } else if k2_neg {
                assert_eq!(k, Fr::from_u128(k1) + Fr::ZETA * Fr::from_u128(k2))
            } else {
                assert_eq!(k, Fr::from_u128(k1) - Fr::ZETA * Fr::from_u128(k2))
            }
        }
    }

    #[test]
    fn test_serialization() {
        crate::tests::curve::random_serialization_test::<G1>();
        #[cfg(feature = "derive_serde")]
        crate::tests::curve::random_serde_test::<G1>();
    }
}
