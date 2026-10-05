//! The Vesta curve `y^2 = x^3 + 5` over `Fq`, with scalar field `Fp`.
#![allow(clippy::unreadable_literal)]

use crate::field::{Fp, Fq};

/// Hash-to-curve identifier, as in `pasta_curves`.
const CURVE_ID: &str = "vesta";

/// `Fq::ZETA`: `(ZETA * x, y) = [Fp::ZETA] (x, y)` on Vesta.
const ENDO_BETA_RAW: [u64; 4] = [
    0x2aa9d2e050aa0e4f,
    0x0fed467d47c033af,
    0x511db4d81cf70f5a,
    0x06819a58283e528e,
];

super::impl_pasta_curve!(Eq, EqAffine, Fq, Fp);

/// A Vesta point in projective form (alias matching `pasta_curves::vesta`).
pub type Point = Eq;
/// A Vesta point in affine form.
pub type Affine = EqAffine;
/// The Vesta base field.
pub type Base = Fq;
/// The Vesta scalar field.
pub type Scalar = Fp;

impl crate::curve::hash_to_curve::HashToCurveParams for Eq {
    const ISO_A: Fq = Fq::from_raw([
        0xc515ad7242eaa6b1,
        0x9673928c7d01b212,
        0x81639c4d96f78773,
        0x267f9b2ee592271a,
    ]);
    const ISO_B: Fq = Fq::from_raw([1265, 0, 0, 0]);
    const Z: Fq = Fq::from_raw([
        0x8c46eb20fffffff4,
        0x224698fc0994a8dd,
        0x0000000000000000,
        0x4000000000000000,
    ]);
    const THETA: Fq = Fq::from_raw([
        0x632cae9872df1b5d,
        0x38578ccadf03ac27,
        0x53c3808d9e2f2357,
        0x2b3483a1ee9a382f,
    ]);
    const ISOGENY_CONSTANTS: [Fq; 13] = [
        Fq::from_raw([
            0x43cd42c800000001,
            0x0205dd51cfa0961a,
            0x8e38e38e38e38e39,
            0x38e38e38e38e38e3,
        ]),
        Fq::from_raw([
            0x8b95c6aaf703bcc5,
            0x216b8861ec72bd5d,
            0xacecf10f5f7c09a2,
            0x1d935247b4473d17,
        ]),
        Fq::from_raw([
            0xaeac67bbeb586a3d,
            0xd59d03d23b39cb11,
            0xed7ee4a9cdf78f8f,
            0x18760c7f7a9ad20d,
        ]),
        Fq::from_raw([
            0xfb539a6f0000002b,
            0xe1c521a795ac8356,
            0x1c71c71c71c71c71,
            0x31c71c71c71c71c7,
        ]),
        Fq::from_raw([
            0xb7284f7eaf21a2e9,
            0xa3ad678129b604d3,
            0x1454798a5b5c56b2,
            0x0a2de485568125d5,
        ]),
        Fq::from_raw([
            0xf169c187d2533465,
            0x30cd6d53df49d235,
            0x0c621de8b91c242a,
            0x14735171ee542778,
        ]),
        Fq::from_raw([
            0x6bef1642aaaaaaab,
            0x5601f4709a8adcb3,
            0xda12f684bda12f68,
            0x12f684bda12f684b,
        ]),
        Fq::from_raw([
            0x8bee58e5fb81de63,
            0x21d910aefb03b31d,
            0xd6767887afbe04d1,
            0x2ec9a923da239e8b,
        ]),
        Fq::from_raw([
            0x4986913ab4443034,
            0x97a3ca5c24e9ea63,
            0x66d1466e9de10e64,
            0x19b0d87e16e25788,
        ]),
        Fq::from_raw([
            0x8f64842c55555533,
            0x8bc32d36fb21a6a3,
            0x425ed097b425ed09,
            0x1ed097b425ed097b,
        ]),
        Fq::from_raw([
            0x58dfecce86b2745e,
            0x06a767bfc35b5bac,
            0x9e7eb64f890a820c,
            0x2f44d6c801c1b8bf,
        ]),
        Fq::from_raw([
            0xd43d449776f99d2f,
            0x926847fb9ddd76a1,
            0x252659ba2b546c7e,
            0x3d59f455cafc7668,
        ]),
        Fq::from_raw([
            0x8c46eb20fffffde5,
            0x224698fc0994a8dd,
            0x0000000000000000,
            0x4000000000000000,
        ]),
    ];
}

impl crate::curve::endo::GlvParams for Eq {
    const GAMMA1: [u64; 4] = [0x32c49e4c00000003, 0x279a745902a2654e, 0x1, 0x0];
    const GAMMA2: [u64; 4] = [0x31f0256800000002, 0x4f34e8b2066389a4, 0x2, 0x0];
    const B1: [u64; 4] = [0x8cb1279300000001, 0x49e69d1640a89953, 0x0, 0x0];
    const B2: [u64; 4] = [0x0c7c095a00000001, 0x93cd3a2c8198e269, 0x0, 0x0];
}
