#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;

use core::{
    cmp,
    fmt::Debug,
    iter::Sum,
    ops::{Add, Mul, Neg, Sub},
};

use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

use super::{fp::Fp, fq::Fq};
use crate::{
    group::{cofactor::CofactorGroup, prime::PrimeCurveAffine, Curve, Group, GroupEncoding},
    impl_binops_additive, impl_binops_additive_specify_output, impl_binops_multiplicative,
    impl_binops_multiplicative_mixed, new_curve_impl, Coordinates, CurveAffine, CurveExt,
};

new_curve_impl!(
    (pub),
    Vesta,
    VestaAffine,
    Fq,
    Fp,
    (- Fq::ONE, Fq::from_raw([2,0,0,0])),
    Fq::ZERO,// Curve a parameter
    Fq::from_raw([5,0,0,0]),// Curve b parameter
    "vesta",
    |domain_prefix| crate::hash_to_curve::hash_to_curve(domain_prefix, Vesta::default_hash_to_curve_suite()),
    crate::serde::CompressedFlagConfig::SingleSpare,
    standard_sign
);

// NOTE: Temporary impl to satisfy macro requirements

impl CofactorGroup for Vesta {
    type Subgroup = Vesta;

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

impl Vesta {
    /// Z = -13
    pub const SVDW_Z: Fq = Fq::from_raw([
        0x8c46eb20fffffff4,
        0x224698fc0994a8dd,
        0x0000000000000000,
        0x4000000000000000,
    ]);
    fn default_hash_to_curve_suite() -> crate::hash_to_curve::Suite<Self, sha2::Sha256, 48> {
        crate::hash_to_curve::Suite::<Vesta, sha2::Sha256, 48>::new(
            b"vesta:SHA-256_SVDW_RO_",
            Self::SVDW_Z,
            crate::hash_to_curve::Method::SVDW,
        )
    }
}

#[cfg(test)]
mod test {
    use group::UncompressedEncoding;
    use rand_core::OsRng;

    use super::*;
    use crate::curve_testing_suite;
    #[cfg(feature = "std")]
    use crate::serde::SerdeObject;

    curve_testing_suite!(
        Vesta,
        "constants",
        Fq::MODULUS,
        Fq::ZERO,
        Fq::from_raw([5, 0, 0, 0]),
        -Fq::ONE,
        Fq::from_raw([2, 0, 0, 0]),
        Fp::MODULUS
    );

    curve_testing_suite!(Vesta);
    curve_testing_suite!(Vesta, "endo_consistency");
    curve_testing_suite!(Vesta, "ecdsa_example");
}
