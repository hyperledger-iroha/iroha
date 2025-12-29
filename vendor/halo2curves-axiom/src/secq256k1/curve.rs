use crate::ff::WithSmallOrderMulGroup;
use crate::ff::{Field, PrimeField};
use crate::group::Curve;
use crate::group::{prime::PrimeCurveAffine, Group, GroupEncoding};
use crate::hash_to_curve::svdw_hash_to_curve;
use crate::secp256k1::{Fp, Fq};
use crate::{
    impl_add_binop_specify_output, impl_binops_additive, impl_binops_additive_specify_output,
    impl_binops_multiplicative, impl_binops_multiplicative_mixed, impl_sub_binop_specify_output,
    new_curve_impl,
};
use crate::{Coordinates, CurveAffine, CurveAffineExt, CurveExt};
use core::cmp;
use core::fmt::Debug;
use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};
use rand::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

#[cfg(feature = "derive_serde")]
use serde::{Deserialize, Serialize};

const SECQ_GENERATOR_X: Fq = Fq::from_raw([
    0xA24288E37702EDA6,
    0x3134E45A097781A6,
    0xB6B06C87A2CE32E2,
    0x76C39F5585CB160E,
]);

const SECQ_GENERATOR_Y: Fq = Fq::from_raw([
    0xA4120DDAD952677F,
    0xD18983D26E8DC055,
    0xDC2D265A8E82A7F7,
    0x3FFC646C7B2918B5,
]);

const SECQ_A: Fq = Fq::from_raw([0, 0, 0, 0]);
const SECQ_B: Fq = Fq::from_raw([7, 0, 0, 0]);

new_curve_impl!(
    (pub),
    Secq256k1,
    Secq256k1Affine,
    true,
    Fq,
    Fp,
    (SECQ_GENERATOR_X, SECQ_GENERATOR_Y),
    SECQ_A,
    SECQ_B,
    "secq256k1",
    |curve_id, domain_prefix| svdw_hash_to_curve(curve_id, domain_prefix, Secq256k1::SVDW_Z),
);

impl group::cofactor::CofactorGroup for Secq256k1 {
    type Subgroup = Secq256k1;

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

impl Secq256k1 {
    const SVDW_Z: Fq = Fq::ONE;
}

#[cfg(test)]
mod tests {
    use crate::secq256k1::Fq;
    use crate::CurveExt;
    use ff::WithSmallOrderMulGroup;

    use super::Secq256k1;

    #[test]
    fn test_hash_to_curve() {
        crate::tests::curve::hash_to_curve_test::<Secq256k1>();
    }

    #[test]
    fn test_curve() {
        crate::tests::curve::curve_tests::<Secq256k1>();
    }

    #[test]
    fn test_endo_consistency() {
        let g = Secq256k1::generator();
        assert_eq!(g * Fq::ZETA, g.endo());
    }

    #[test]
    fn test_serialization() {
        crate::tests::curve::random_serialization_test::<Secq256k1>();
        #[cfg(feature = "derive_serde")]
        crate::tests::curve::random_serde_test::<Secq256k1>();
    }
}
