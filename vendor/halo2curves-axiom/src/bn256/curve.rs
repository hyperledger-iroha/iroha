use crate::arithmetic::mul_512;
use crate::arithmetic::sbb;
use crate::arithmetic::EndoParameters;
use crate::arithmetic::{CurveAffineExt, CurveEndo};
use crate::bn256::Fq;
use crate::bn256::Fq2;
use crate::bn256::Fr;
use crate::endo;
use crate::ff::WithSmallOrderMulGroup;
use crate::ff::{Field, PrimeField};
use crate::group::Curve;
use crate::group::{cofactor::CofactorGroup, prime::PrimeCurveAffine, Group, GroupEncoding};
use crate::hash_to_curve::svdw_hash_to_curve;
use crate::{
    impl_add_binop_specify_output, impl_binops_additive, impl_binops_additive_specify_output,
    impl_binops_multiplicative, impl_binops_multiplicative_mixed, impl_sub_binop_specify_output,
    new_curve_impl,
};
use crate::{Coordinates, CurveAffine, CurveExt};
use core::cmp;
use core::fmt::Debug;
use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};
use rand::RngCore;
use std::convert::TryInto;
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
    (G1_GENERATOR_X,G1_GENERATOR_Y),
    G1_A,
    G1_B,
    "bn256_g1",
    |curve_id, domain_prefix| svdw_hash_to_curve(curve_id, domain_prefix, G1::SVDW_Z),
);

new_curve_impl!(
    (pub),
    G2,
    G2Affine,
    false,
    Fq2,
    Fr,
    (G2_GENERATOR_X, G2_GENERATOR_Y),
    G2_A,
    G2_B,
    "bn256_g2",
    |_, _| unimplemented!(),
);

const G1_GENERATOR_X: Fq = Fq::one();
const G1_GENERATOR_Y: Fq = Fq::from_raw([2, 0, 0, 0]);
const G1_A: Fq = Fq::from_raw([0, 0, 0, 0]);
const G1_B: Fq = Fq::from_raw([3, 0, 0, 0]);

const G2_A: Fq2 = Fq2 {
    c0: Fq::from_raw([0, 0, 0, 0]),
    c1: Fq::from_raw([0, 0, 0, 0]),
};

const G2_B: Fq2 = Fq2 {
    c0: Fq::from_raw([
        0x3267e6dc24a138e5,
        0xb5b4c5e559dbefa3,
        0x81be18991be06ac3,
        0x2b149d40ceb8aaae,
    ]),
    c1: Fq::from_raw([
        0xe4a2bd0685c315d2,
        0xa74fa084e52d1852,
        0xcd2cafadeed8fdf4,
        0x009713b03af0fed4,
    ]),
};

const G2_GENERATOR_X: Fq2 = Fq2 {
    c0: Fq::from_raw([
        0x46debd5cd992f6ed,
        0x674322d4f75edadd,
        0x426a00665e5c4479,
        0x1800deef121f1e76,
    ]),
    c1: Fq::from_raw([
        0x97e485b7aef312c2,
        0xf1aa493335a9e712,
        0x7260bfb731fb5d25,
        0x198e9393920d483a,
    ]),
};

const G2_GENERATOR_Y: Fq2 = Fq2 {
    c0: Fq::from_raw([
        0x4ce6cc0166fa7daa,
        0xe3d1e7690c43d37b,
        0x4aab71808dcb408f,
        0x12c85ea5db8c6deb,
    ]),

    c1: Fq::from_raw([
        0x55acdadcd122975b,
        0xbc4b313370b38ef3,
        0xec9e99ad690c3395,
        0x090689d0585ff075,
    ]),
};

// Generated using https://github.com/ConsenSys/gnark-crypto/blob/master/ecc/utils.go
// with `bn256::Fr::ZETA`
// See https://github.com/demining/Endomorphism-Secp256k1/blob/main/README.md
// to have more details about the endomorphism.
const ENDO_PARAMS_BN: EndoParameters = EndoParameters {
    // round(b2/n)
    gamma1: [0xd91d232ec7e0b3d7, 0x2, 0, 0],
    // round(-b1/n)
    gamma2: [0x5398fd0300ff6565, 0x4ccef014a773d2d2, 0x02, 0],
    b1: [0x89d3256894d213e3, 0, 0, 0],
    b2: [0x0be4e1541221250b, 0x6f4d8248eeb859fd, 0, 0],
};

endo!(G1, Fr, ENDO_PARAMS_BN);

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

impl CofactorGroup for G2 {
    type Subgroup = G2;

    fn clear_cofactor(&self) -> Self {
        // "0x30644e72e131a029b85045b68181585e06ceecda572a2489345f2299c0f9fa8d"
        let e: [u8; 32] = [
            0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81,
            0x58, 0x5e, 0x06, 0xce, 0xec, 0xda, 0x57, 0x2a, 0x24, 0x89, 0x34, 0x5f, 0x22, 0x99,
            0xc0, 0xf9, 0xfa, 0x8d,
        ];

        // self * COFACTOR_G2
        let mut acc = G2::identity();
        for bit in e
            .iter()
            .flat_map(|byte| (0..8).rev().map(move |i| Choice::from((byte >> i) & 1u8)))
            .skip(1)
        {
            acc = acc.double();
            acc = G2::conditional_select(&acc, &(acc + self), bit);
        }
        acc
    }

    fn into_subgroup(self) -> CtOption<Self::Subgroup> {
        unimplemented!();
    }

    fn is_torsion_free(&self) -> Choice {
        // "0x30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001"
        let e: [u8; 32] = [
            0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81,
            0x58, 0x5d, 0x28, 0x33, 0xe8, 0x48, 0x79, 0xb9, 0x70, 0x91, 0x43, 0xe1, 0xf5, 0x93,
            0xf0, 0x00, 0x00, 0x01,
        ];

        // self * GROUP_ORDER;

        let mut acc = G2::identity();
        for bit in e
            .iter()
            .flat_map(|byte| (0..8).rev().map(move |i| Choice::from((byte >> i) & 1u8)))
            .skip(1)
        {
            acc = acc.double();
            acc = G2::conditional_select(&acc, &(acc + self), bit);
        }
        acc.is_identity()
    }
}

impl G1 {
    const SVDW_Z: Fq = Fq::ONE;
}

#[cfg(test)]
mod tests {
    use crate::arithmetic::CurveEndo;
    use crate::bn256::{Fr, G1, G2};
    use crate::CurveExt;
    use ff::Field;
    use ff::{PrimeField, WithSmallOrderMulGroup};
    use rand_core::OsRng;

    #[test]
    fn test_hash_to_curve() {
        crate::tests::curve::hash_to_curve_test::<G1>();
    }

    #[test]
    fn test_map_to_curve() {
        crate::tests::curve::svdw_map_to_curve_test::<G1>(
            G1::SVDW_Z,
            // Precomputed constants taken from https://github.com/ConsenSys/gnark-crypto/blob/441dc0ffe639294b8d09e394f24ba7575577229c/internal/generator/config/bn254.go#L26-L32.
            [
                "4",
                "10944121435919637611123202872628637544348155578648911831344518947322613104291",
                "8815841940592487685674414971303048083897117035520822607866",
                "7296080957279758407415468581752425029565437052432607887563012631548408736189",
            ],
            // List of (u, (Q.x, Q.y)) taken from https://github.com/ConsenSys/gnark-crypto/blob/441dc0ffe639294b8d09e394f24ba7575577229c/ecc/bn254/hash_vectors_test.go#L4-L28
            [
                (
                    "0xcb81538a98a2e3580076eed495256611813f6dae9e16d3d4f8de7af0e9833e1",
                    (
                        "0x1bb8810e2ceaf04786d4efd216fc2820ddd9363712efc736ada11049d8af5925",
                        "0x1efbf8d54c60d865cce08437668ea30f5bf90d287dbd9b5af31da852915e8f11",
                    ),
                ),
                (
                    "0xba35e127276e9000b33011860904ddee28f1d48ddd3577e2a797ef4a5e62319",
                    (
                        "0xda4a96147df1f35b0f820bd35c6fac3b80e8e320de7c536b1e054667b22c332",
                        "0x189bd3fbffe4c8740d6543754d95c790e44cd2d162858e3b733d2b8387983bb7",
                    ),
                ),
                (
                    "0x11852286660cd970e9d7f46f99c7cca2b75554245e91b9b19d537aa6147c28fc",
                    (
                        "0x2ff727cfaaadb3acab713fa22d91f5fddab3ed77948f3ef6233d7ea9b03f4da1",
                        "0x304080768fd2f87a852155b727f97db84b191e41970506f0326ed4046d1141aa",
                    ),
                ),
                (
                    "0x174d1c85d8a690a876cc1deba0166d30569fafdb49cb3ed28405bd1c5357a1cc",
                    (
                        "0x11a2eaa8e3e89de056d1b3a288a7f733c8a1282efa41d28e71af065ab245df9b",
                        "0x60f37c447ac29fd97b9bb83be98ddccf15e34831a9cdf5493b7fede0777ae06",
                    ),
                ),
                (
                    "0x73b81432b4cf3a8a9076201500d1b94159539f052a6e0928db7f2df74bff672",
                    (
                        "0x27409dccc6ee4ce90e24744fda8d72c0bc64e79766f778da0c1c0ef1c186ea84",
                        "0x1ac201a542feca15e77f30370da183514dc99d8a0b2c136d64ede35cd0b51dc0",
                    ),
                ),
            ],
        );
    }

    #[test]
    fn test_curve() {
        crate::tests::curve::curve_tests::<G1>();
        crate::tests::curve::curve_tests::<G2>();
    }

    #[test]
    fn test_endo() {
        let z_impl = Fr::ZETA;
        let z_other = Fr::from_raw([
            0x8b17ea66b99c90dd,
            0x5bfc41088d8daaa7,
            0xb3c4d79d41a91758,
            0x00,
        ]);
        assert_eq!(z_impl * z_impl + z_impl, -Fr::ONE);
        assert_eq!(z_other * z_other + z_other, -Fr::ONE);

        let g = G1::generator();
        assert_eq!(g * Fr::ZETA, g.endo());
        let g = G2::generator();
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
        crate::tests::curve::random_serialization_test::<G2>();
        #[cfg(feature = "derive_serde")]
        {
            crate::tests::curve::random_serde_test::<G1>();
            crate::tests::curve::random_serde_test::<G2>();
        }
    }
}
