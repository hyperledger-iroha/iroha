#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::{boxed::Box, vec::Vec};

use core::{
    cmp,
    fmt::Debug,
    iter::Sum,
    ops::{Add, Mul, Neg, Sub},
};

use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

use crate::{
    ff::{Field, PrimeField, WithSmallOrderMulGroup},
    group::{prime::PrimeCurveAffine, Curve, Group as _, GroupEncoding},
    t256::{Fp, Fq},
    Coordinates, CurveAffine, CurveExt,
};

impl group::cofactor::CofactorGroup for T256 {
    type Subgroup = T256;

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

// Generator (x = 3), the smallest x on the curve.
// Consistent with Neuromancer (https://neuromancer.sk/std/other/Tom-256)
// and the primary generator in sigpop (https://github.com/pag-crypto/sigpop).
const T256_GENERATOR_X: Fp = Fp::from_raw([
    0x0000000000000003,
    0x0000000000000000,
    0x0000000000000000,
    0x0000000000000000,
]);

const T256_GENERATOR_Y: Fp = Fp::from_raw([
    0x30B4954925B1F02D,
    0xECD9D538A351BB3C,
    0x4E97345CBE66600D,
    0x5A6DD32DF58708E6,
]);

// a  =  p − 3
const T256_A: Fp = Fp::from_raw([
    0x93135661B1C4B114,
    0x7E72B42B30E73177,
    0x0000000000000001,
    0xFFFFFFFF00000001,
]);

// b  =  0xB441071B12F4A0366FB552F8E21ED4AC36B06ACEEB354224863E60F20219FC56
const T256_B: Fp = Fp::from_raw([
    0x863E60F20219FC56,
    0x36B06ACEEB354224,
    0x6FB552F8E21ED4AC,
    0xB441071B12F4A036,
]);

use crate::{
    impl_binops_additive, impl_binops_additive_specify_output, impl_binops_multiplicative,
    impl_binops_multiplicative_mixed, new_curve_impl,
};

new_curve_impl!(
    (pub),
    T256,
    T256Affine,
    Fp,
    Fq,
    (T256_GENERATOR_X,T256_GENERATOR_Y),
    T256_A,
    T256_B,
    "t256",
    |domain_prefix| hash_to_curve(domain_prefix, hash_to_curve_suite(b"T256_XMD:SHA-256_SSWU_RO_")),
    crate::serde::CompressedFlagConfig::Extra,
    standard_sign
);

fn hash_to_curve_suite(domain: &[u8]) -> crate::hash_to_curve::Suite<T256, sha2::Sha256, 48> {
    // Z : <https://datatracker.ietf.org/doc/html/rfc9380#sswu-z-code>
    // Z = -2
    const SSWU_Z: Fp = Fp::from_raw([
        0x93135661B1C4B115,
        0x7E72B42B30E73177,
        0x0000000000000001,
        0xFFFFFFFF00000001,
    ]);

    let iso_map = crate::hash_to_curve::Iso {
        a: T256::a(),
        b: T256::b(),
        map: Box::new(move |x, y, z| T256 { x, y, z }),
    };

    crate::hash_to_curve::Suite::new(domain, SSWU_Z, crate::hash_to_curve::Method::SSWU(iso_map))
}

#[allow(clippy::type_complexity)]
pub(crate) fn hash_to_curve<'a>(
    domain_prefix: &'a str,
    suite: crate::hash_to_curve::Suite<T256, sha2::Sha256, 48>,
) -> Box<dyn Fn(&[u8]) -> T256 + 'a> {
    Box::new(move |message| suite.hash_to_curve(domain_prefix, message))
}

#[cfg(test)]
mod test {
    use group::UncompressedEncoding;
    use rand_core::OsRng;

    use super::*;
    use crate::serde::SerdeObject;
    crate::curve_testing_suite!(T256);
    crate::curve_testing_suite!(T256, "ecdsa_example");
    crate::curve_testing_suite!(
        T256,
        "constants",
        Fp::MODULUS,
        T256_A,
        T256_B,
        T256_GENERATOR_X,
        T256_GENERATOR_Y,
        Fq::MODULUS
    );
}
