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
    impl_binops_additive, impl_binops_additive_specify_output, impl_binops_multiplicative,
    impl_binops_multiplicative_mixed, new_curve_impl,
    secp256k1::{Fp, Fq},
    Coordinates, CurveAffine, CurveExt,
};

impl group::cofactor::CofactorGroup for Secp256k1 {
    type Subgroup = Secp256k1;

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

// Reference: https://neuromancer.sk/std/secg/secp256k1
const SECP_GENERATOR_X: Fp = Fp::from_raw([
    0x59F2815B16F81798,
    0x029BFCDB2DCE28D9,
    0x55A06295CE870B07,
    0x79BE667EF9DCBBAC,
]);
const SECP_GENERATOR_Y: Fp = Fp::from_raw([
    0x9C47D08FFB10D4B8,
    0xFD17B448A6855419,
    0x5DA4FBFC0E1108A8,
    0x483ADA7726A3C465,
]);

const SECP_A: Fp = Fp::from_raw([0, 0, 0, 0]);
const SECP_B: Fp = Fp::from_raw([7, 0, 0, 0]);

new_curve_impl!(
    (pub),
    Secp256k1,
    Secp256k1Affine,
    Fp,
    Fq,
    (SECP_GENERATOR_X,SECP_GENERATOR_Y),
    SECP_A,
    SECP_B,
    "secp256k1",
    |domain_prefix| hash_to_curve(domain_prefix, hash_to_curve_suite(b"secp256k1_XMD:SHA-256_SSWU_RO_")),
    crate::serde::CompressedFlagConfig::Extra,
    standard_sign
);

fn hash_to_curve_suite(domain: &[u8]) -> crate::hash_to_curve::Suite<Secp256k1, sha2::Sha256, 48> {
    // Z = -11 (reference: <https://www.rfc-editor.org/rfc/rfc9380.html#name-suites-for-secp256k1>)
    // 0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc24
    const SSWU_Z: Fp = Fp::from_raw([
        0xfffffffefffffc24,
        0xffffffffffffffff,
        0xffffffffffffffff,
        0xffffffffffffffff,
    ]);

    // E': y'^2 = x'^3 + A' * x' + B', where
    // A': 0x3f8731abdd661adca08a5558f0f5d272e953d363cb6f0e5d405447c01a444533
    // B': 1771
    // (reference: <https://www.rfc-editor.org/rfc/rfc9380.html#name-suites-for-secp256k1>)
    pub const ISO_SECP_A: Fp = Fp::from_raw([
        0x405447c01a444533,
        0xe953d363cb6f0e5d,
        0xa08a5558f0f5d272,
        0x3f8731abdd661adc,
    ]);

    pub const ISO_SECP_B: Fp = Fp::from_raw([1771, 0, 0, 0]);

    let iso_map = crate::hash_to_curve::Iso {
        a: ISO_SECP_A,
        b: ISO_SECP_B,
        map: Box::new(iso_map),
    };

    crate::hash_to_curve::Suite::new(domain, SSWU_Z, crate::hash_to_curve::Method::SSWU(iso_map))
}

#[allow(clippy::type_complexity)]
pub(crate) fn hash_to_curve<'a>(
    domain_prefix: &'a str,
    suite: crate::hash_to_curve::Suite<Secp256k1, sha2::Sha256, 48>,
) -> Box<dyn Fn(&[u8]) -> Secp256k1 + 'a> {
    Box::new(move |message| suite.hash_to_curve(domain_prefix, message))
}

/// 3-Isogeny Map for Secp256k1
/// Reference: <https://www.rfc-editor.org/rfc/rfc9380.html#name-3-isogeny-map-for-secp256k1>
pub(crate) fn iso_map(x: Fp, y: Fp, z: Fp) -> Secp256k1 {
    // constants for secp256k1 iso_map computation
    const K: [[Fp; 4]; 5] = [
        [Fp::ZERO; 4],
        [
            Fp::from_raw([
                0x8e38e38daaaaa8c7,
                0x38e38e38e38e38e3,
                0xe38e38e38e38e38e,
                0x8e38e38e38e38e38,
            ]),
            Fp::from_raw([
                0xdfff1044f17c6581,
                0xd595d2fc0bf63b92,
                0xb9f315cea7fd44c5,
                0x7d3d4c80bc321d5,
            ]),
            Fp::from_raw([
                0x4ecbd0b53d9dd262,
                0xe4506144037c4031,
                0xe2a413deca25caec,
                0x534c328d23f234e6,
            ]),
            Fp::from_raw([
                0x8e38e38daaaaa88c,
                0x38e38e38e38e38e3,
                0xe38e38e38e38e38e,
                0x8e38e38e38e38e38,
            ]),
        ],
        [
            Fp::from_raw([
                0x9fe6b745781eb49b,
                0x86cd409542f8487d,
                0x9ca34ccbb7b640dd,
                0xd35771193d94918a,
            ]),
            Fp::from_raw([
                0xc52a56612a8c6d14,
                0x06d36b641f5e41bb,
                0xf7c4b2d51b542254,
                0xedadc6f64383dc1d,
            ]),
            Fp::ZERO,
            Fp::ZERO,
        ],
        [
            Fp::from_raw([
                0xa12f684b8e38e23c,
                0x2f684bda12f684bd,
                0x684bda12f684bda1,
                0x4bda12f684bda12f,
            ]),
            Fp::from_raw([
                0xdffc90fc201d71a3,
                0x647ab046d686da6f,
                0xa9d0a54b12a0a6d5,
                0xc75e0c32d5cb7c0f,
            ]),
            Fp::from_raw([
                0xa765e85a9ecee931,
                0x722830a201be2018,
                0x715209ef6512e576,
                0x29a6194691f91a73,
            ]),
            Fp::from_raw([
                0x84bda12f38e38d84,
                0xbda12f684bda12f6,
                0xa12f684bda12f684,
                0x2f684bda12f684bd,
            ]),
        ],
        [
            Fp::from_raw([
                0xfffffffefffff93b,
                0xffffffffffffffff,
                0xffffffffffffffff,
                0xffffffffffffffff,
            ]),
            Fp::from_raw([
                0xdfb425d2685c2573,
                0x9467c1bfc8e8d978,
                0xd5e9e6632722c298,
                0x7a06534bb8bdb49f,
            ]),
            Fp::from_raw([
                0xa7bf8192bfd2a76f,
                0x0a3d21162f0d6299,
                0xf3a70c3fa8fe337e,
                0x6484aa716545ca2c,
            ]),
            Fp::ZERO,
        ],
    ];

    let z2 = z.square();
    let z3 = z2 * z;

    // iso_map logic (avoid inversion) in projective coordinates
    //   reference: <https://github.com/zcash/pasta_curves/blob/main/src/hashtocurve.rs#L80-L106>
    let x_num = ((K[1][3] * x + K[1][2] * z) * x + K[1][1] * z2) * x + K[1][0] * z3;
    let x_den = (z * x + K[2][1] * z2) * x + K[2][0] * z3;

    let y_num = (((K[3][3] * x + K[3][2] * z) * x + K[3][1] * z2) * x + K[3][0] * z3) * y;
    let y_den = (((x + K[4][2] * z) * x + K[4][1] * z2) * x + K[4][0] * z3) * z;

    let z = x_den * y_den;
    let x = x_num * y_den;
    let y = y_num * x_den;

    Secp256k1 { x, y, z }
}

#[cfg(test)]
mod test {
    use group::UncompressedEncoding;
    use rand_core::OsRng;

    use super::*;
    #[cfg(feature = "std")]
    use crate::serde::SerdeObject;
    use crate::tests::curve::TestH2C;

    crate::curve_testing_suite!(Secp256k1);
    crate::curve_testing_suite!(Secp256k1, "endo_consistency");
    crate::curve_testing_suite!(Secp256k1, "ecdsa_example");
    crate::curve_testing_suite!(
        Secp256k1,
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
        // https://www.ietf.org/archive/id/draft-irtf-cfrg-hash-to-curve-16.html#name-expand_message_xmdsha-256
        [
            TestH2C::<Secp256k1Affine>::new(
                b"",
                crate::tests::point_from_hex(
                    "c1cae290e291aee617ebaef1be6d73861479c48b841eaba9b7b5852ddfeb1346",
                    "64fa678e07ae116126f08b022a94af6de15985c996c3a91b64c406a960e51067",
                ),
            ),
            TestH2C::<Secp256k1Affine>::new(
                b"abc",
                crate::tests::point_from_hex(
                    "3377e01eab42db296b512293120c6cee72b6ecf9f9205760bd9ff11fb3cb2c4b",
                    "7f95890f33efebd1044d382a01b1bee0900fb6116f94688d487c6c7b9c8371f6",
                ),
            ),
            TestH2C::<Secp256k1Affine>::new(
                b"abcdef0123456789",
                crate::tests::point_from_hex(
                    "bac54083f293f1fe08e4a70137260aa90783a5cb84d3f35848b324d0674b0e3a",
                    "4436476085d4c3c4508b60fcf4389c40176adce756b398bdee27bca19758d828",
                ),
            ),
            TestH2C::<Secp256k1Affine>::new(
                b"q128_qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
                crate::tests::point_from_hex(
                    "e2167bc785333a37aa562f021f1e881defb853839babf52a7f72b102e41890e9",
                    "f2401dd95cc35867ffed4f367cd564763719fbc6a53e969fb8496a1e6685d873",
                ),
            ), //
            TestH2C::<Secp256k1Affine>::new(
                b"a512_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                crate::tests::point_from_hex(
                    "e3c8d35aaaf0b9b647e88a0a0a7ee5d5bed5ad38238152e4e6fd8c1f8cb7c998",
                    "8446eeb6181bf12f56a9d24e262221cc2f0c4725c7e3803024b5888ee5823aa6",
                ),
            ),
        ].iter().for_each(|test| {
            test.run("QUUX-V01-CS02-with-");
        });
    }
}
