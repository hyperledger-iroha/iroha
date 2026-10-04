//! The Pallas curve `y^2 = x^3 + 5` over `Fp`, with scalar field `Fq`.
#![allow(clippy::unreadable_literal)]

use crate::field::{Fp, Fq};

/// Hash-to-curve identifier, as in `pasta_curves`.
const CURVE_ID: &str = "pallas";

/// `Fp::ZETA`: `(ZETA * x, y) = [Fq::ZETA] (x, y)` on Pallas.
const ENDO_BETA_RAW: [u64; 4] = [
    0x1dad5ebdfdfe4ab9,
    0x1d1f8bd237ad3149,
    0x2caad5dc57aab1b0,
    0x12ccca834acdba71,
];

super::impl_pasta_curve!(Ep, EpAffine, Fp, Fq);

/// A Pallas point in projective form (alias matching `pasta_curves::pallas`).
pub type Point = Ep;
/// A Pallas point in affine form.
pub type Affine = EpAffine;
/// The Pallas base field.
pub type Base = Fp;
/// The Pallas scalar field.
pub type Scalar = Fq;

impl crate::curve::hash_to_curve::HashToCurveParams for Ep {
    const ISO_A: Fp = Fp::from_raw([
        0x92bb4b0b657a014b,
        0xb74134581a27a59f,
        0x49be2d7258370742,
        0x18354a2eb0ea8c9c,
    ]);
    const ISO_B: Fp = Fp::from_raw([1265, 0, 0, 0]);
    const Z: Fp = Fp::from_raw([
        0x992d30ecfffffff4,
        0x224698fc094cf91b,
        0x0000000000000000,
        0x4000000000000000,
    ]);
    const THETA: Fp = Fp::from_raw([
        0xca330bcc09ac318e,
        0x51f64fc4dc888857,
        0x4647aef782d5cdc8,
        0x0f7bdb65814179b4,
    ]);
    const ISOGENY_CONSTANTS: [Fp; 13] = [
        Fp::from_raw([
            0x775f6034aaaaaaab,
            0x4081775473d8375b,
            0xe38e38e38e38e38e,
            0x0e38e38e38e38e38,
        ]),
        Fp::from_raw([
            0x8cf863b02814fb76,
            0x0f93b82ee4b99495,
            0x267c7ffa51cf412a,
            0x3509afd51872d88e,
        ]),
        Fp::from_raw([
            0x0eb64faef37ea4f7,
            0x380af066cfeb6d69,
            0x98c7d7ac3d98fd13,
            0x17329b9ec5253753,
        ]),
        Fp::from_raw([
            0xeebec06955555580,
            0x8102eea8e7b06eb6,
            0xc71c71c71c71c71c,
            0x1c71c71c71c71c71,
        ]),
        Fp::from_raw([
            0xc47f2ab668bcd71f,
            0x9c434ac1c96b6980,
            0x5a607fcce0494a79,
            0x1d572e7ddc099cff,
        ]),
        Fp::from_raw([
            0x2aa3af1eae5b6604,
            0xb4abf9fb9a1fc81c,
            0x1d13bf2a7f22b105,
            0x325669becaecd5d1,
        ]),
        Fp::from_raw([
            0x5ad985b5e38e38e4,
            0x7642b01ad461bad2,
            0x4bda12f684bda12f,
            0x1a12f684bda12f68,
        ]),
        Fp::from_raw([
            0xc67c31d8140a7dbb,
            0x07c9dc17725cca4a,
            0x133e3ffd28e7a095,
            0x1a84d7ea8c396c47,
        ]),
        Fp::from_raw([
            0x02e2be87d225b234,
            0x1765e924f7459378,
            0x303216cce1db9ff1,
            0x3fb98ff0d2ddcadd,
        ]),
        Fp::from_raw([
            0x93e53ab371c71c4f,
            0x0ac03e8e134eb3e4,
            0x7b425ed097b425ed,
            0x025ed097b425ed09,
        ]),
        Fp::from_raw([
            0x5a28279b1d1b42ae,
            0x5941a3a4a97aa1b3,
            0x0790bfb3506defb6,
            0x0c02c5bcca0e6b7f,
        ]),
        Fp::from_raw([
            0x4d90ab820b12320a,
            0xd976bbfabbc5661d,
            0x573b3d7f7d681310,
            0x17033d3c60c68173,
        ]),
        Fp::from_raw([
            0x992d30ecfffffde5,
            0x224698fc094cf91b,
            0x0000000000000000,
            0x4000000000000000,
        ]),
    ];
}

impl crate::curve::endo::GlvParams for Ep {
    const GAMMA1: [u64; 4] = [0x32c49e4bffffffff, 0x279a745902a2654e, 0x1, 0x0];
    const GAMMA2: [u64; 4] = [0x31f0256800000002, 0x4f34e8b2066389a4, 0x2, 0x0];
    const B1: [u64; 4] = [0x8cb1279300000000, 0x49e69d1640a89953, 0x0, 0x0];
    const B2: [u64; 4] = [0x0c7c095a00000001, 0x93cd3a2c8198e269, 0x0, 0x0];
}
