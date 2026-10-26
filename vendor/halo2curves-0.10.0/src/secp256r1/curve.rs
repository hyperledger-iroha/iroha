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

use rand_core::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

use crate::{
    ff::{Field, PrimeField, WithSmallOrderMulGroup},
    group::{prime::PrimeCurveAffine, Curve, Group as _, GroupEncoding},
    secp256r1::{Fp, Fq},
    Coordinates, CurveAffine, CurveExt,
};

impl group::cofactor::CofactorGroup for Secp256r1 {
    type Subgroup = Secp256r1;

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

// Reference: https://neuromancer.sk/std/secg/secp256r1
const SECP_GENERATOR_X: Fp = Fp::from_raw([
    0xF4A13945D898C296,
    0x77037D812DEB33A0,
    0xF8BCE6E563A440F2,
    0x6B17D1F2E12C4247,
]);

const SECP_GENERATOR_Y: Fp = Fp::from_raw([
    0xCBB6406837BF51F5,
    0x2BCE33576B315ECE,
    0x8EE7EB4A7C0F9E16,
    0x4FE342E2FE1A7F9B,
]);

const SECP_A: Fp = Fp::from_raw([
    0xFFFFFFFFFFFFFFFC,
    0x00000000FFFFFFFF,
    0x0000000000000000,
    0xFFFFFFFF00000001,
]);
const SECP_B: Fp = Fp::from_raw([
    0x3BCE3C3E27D2604B,
    0x651D06B0CC53B0F6,
    0xB3EBBD55769886BC,
    0x5AC635D8AA3A93E7,
]);

use crate::{
    impl_binops_additive, impl_binops_additive_specify_output, impl_binops_multiplicative,
    impl_binops_multiplicative_mixed, new_curve_impl,
};

new_curve_impl!(
    (pub),
    Secp256r1,
    Secp256r1Affine,
    Fp,
    Fq,
    (SECP_GENERATOR_X,SECP_GENERATOR_Y),
    SECP_A,
    SECP_B,
    "secp256r1",
    |domain_prefix| hash_to_curve(domain_prefix, hash_to_curve_suite(b"P256_XMD:SHA-256_SSWU_RO_")),
    crate::serde::CompressedFlagConfig::Extra,
    standard_sign
);

fn hash_to_curve_suite(domain: &[u8]) -> crate::hash_to_curve::Suite<Secp256r1, sha2::Sha256, 48> {
    // Optimal Z with: <https://datatracker.ietf.org/doc/html/rfc9380#sswu-z-code>
    // 0xffffffff00000001000000000000000000000000fffffffffffffffffffffff5
    // Z = -10 (reference: <https://www.rfc-editor.org/rfc/rfc9380.html#section-8.2>)
    const SSWU_Z: Fp = Fp::from_raw([
        0xfffffffffffffff5,
        0x00000000ffffffff,
        0x0000000000000000,
        0xffffffff00000001,
    ]);

    let iso_map = crate::hash_to_curve::Iso {
        a: Secp256r1::a(),
        b: Secp256r1::b(),
        map: Box::new(move |x, y, z| Secp256r1 { x, y, z }),
    };

    crate::hash_to_curve::Suite::new(domain, SSWU_Z, crate::hash_to_curve::Method::SSWU(iso_map))
}

#[allow(clippy::type_complexity)]
pub(crate) fn hash_to_curve<'a>(
    domain_prefix: &'a str,
    suite: crate::hash_to_curve::Suite<Secp256r1, sha2::Sha256, 48>,
) -> Box<dyn Fn(&[u8]) -> Secp256r1 + 'a> {
    Box::new(move |message| suite.hash_to_curve(domain_prefix, message))
}

#[cfg(test)]
mod test {
    use group::UncompressedEncoding;
    use rand_core::OsRng;

    use super::*;
    #[cfg(feature = "std")]
    use crate::serde::SerdeObject;
    use crate::tests::curve::TestH2C;
    crate::curve_testing_suite!(Secp256r1);
    crate::curve_testing_suite!(Secp256r1, "ecdsa_example");
    crate::curve_testing_suite!(
        Secp256r1,
        "constants",
        Fp::MODULUS,
        SECP_A,
        SECP_B,
        SECP_GENERATOR_X,
        SECP_GENERATOR_Y,
        Fq::MODULUS
    );

    #[test]
    fn test_hash_to_curve() {
        // Test vectors are taken from
        // https://www.ietf.org/archive/id/draft-irtf-cfrg-hash-to-curve-16.html#name-p256_xmdsha-256_sswu_ro_
        [
            TestH2C::<Secp256r1Affine>::new(
                b"",
                crate::tests::point_from_hex(
                    "2c15230b26dbc6fc9a37051158c95b79656e17a1a920b11394ca91c44247d3e4",
                    "8a7a74985cc5c776cdfe4b1f19884970453912e9d31528c060be9ab5c43e8415",
                ),
            ),
            TestH2C::<Secp256r1Affine>::new(
                b"abc",
                crate::tests::point_from_hex(
                    "0bb8b87485551aa43ed54f009230450b492fead5f1cc91658775dac4a3388a0f",
                    "5c41b3d0731a27a7b14bc0bf0ccded2d8751f83493404c84a88e71ffd424212e",
                ),
            ),
            TestH2C::<Secp256r1Affine>::new(
                b"abcdef0123456789",
                crate::tests::point_from_hex(
                    "65038ac8f2b1def042a5df0b33b1f4eca6bff7cb0f9c6c1526811864e544ed80",
                    "cad44d40a656e7aff4002a8de287abc8ae0482b5ae825822bb870d6df9b56ca3",
                ),
            ),
            TestH2C::<Secp256r1Affine>::new(
                b"q128_qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
                crate::tests::point_from_hex(
                    "4be61ee205094282ba8a2042bcb48d88dfbb609301c49aa8b078533dc65a0b5d",
                    "98f8df449a072c4721d241a3b1236d3caccba603f916ca680f4539d2bfb3c29e",
                ),
            ), //
            TestH2C::<Secp256r1Affine>::new(
                b"a512_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                crate::tests::point_from_hex(
                    "457ae2981f70ca85d8e24c308b14db22f3e3862c5ea0f652ca38b5e49cd64bc5",
                    "ecb9f0eadc9aeed232dabc53235368c1394c78de05dd96893eefa62b0f4757dc",
                ),
            ),
        ].iter().for_each(|test| {
            test.run("QUUX-V01-CS02-with-");
        });
    }
}
