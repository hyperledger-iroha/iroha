//! Source: <https://github.com/privacy-scaling-explorations/halo2curves/blob/support_bls12-381/src/bls12_381/mod.rs>

use crate::arithmetic::mul_512;
use crate::arithmetic::sbb;
use crate::{
    arithmetic::{CurveEndo, EndoParameters},
    endo,
};
use ff::PrimeField;
use ff::WithSmallOrderMulGroup;
use std::convert::TryInto;

use super::{G1Projective, Scalar};

// Obtained from https://github.com/ConsenSys/gnark-crypto/blob/master/ecc/utils.go
// See https://github.com/demining/Endomorphism-Secp256k1/blob/main/README.md
// to have more details about the endomorphism.
const ENDO_PARAMS_BLS: EndoParameters = EndoParameters {
    // round(b2/n)
    gamma2: [0x63f6e522f6cfee30u64, 0x7c6becf1e01faadd, 0x01, 0x0],
    // round(-b1/n)
    gamma1: [0x02u64, 0x0, 0x0, 0x0],
    b1: [0x01u64, 0x0, 0x0, 0x0],
    b2: [0x0000000100000000, 0xac45a4010001a402, 0x0, 0x0],
};

endo!(G1Projective, Scalar, ENDO_PARAMS_BLS);

#[test]
fn test_endo() {
    use ff::Field;
    use rand_core::OsRng;

    for _ in 0..100000 {
        let k = Scalar::random(OsRng);
        let (k1, k1_neg, k2, k2_neg) = G1Projective::decompose_scalar(&k);
        if k1_neg & k2_neg {
            assert_eq!(
                k,
                -Scalar::from_u128(k1) + Scalar::ZETA * Scalar::from_u128(k2)
            )
        } else if k1_neg {
            assert_eq!(
                k,
                -Scalar::from_u128(k1) - Scalar::ZETA * Scalar::from_u128(k2)
            )
        } else if k2_neg {
            assert_eq!(
                k,
                Scalar::from_u128(k1) + Scalar::ZETA * Scalar::from_u128(k2)
            )
        } else {
            assert_eq!(
                k,
                Scalar::from_u128(k1) - Scalar::ZETA * Scalar::from_u128(k2)
            )
        }
    }

    for _ in 0..100000 {
        let k = Scalar::random(OsRng);
        let (k1, k1_neg, k2, k2_neg) = G1Projective::decompose_scalar(&k);
        if k1_neg & k2_neg {
            assert_eq!(
                k,
                -Scalar::from_u128(k1) + Scalar::ZETA * Scalar::from_u128(k2)
            )
        } else if k1_neg {
            assert_eq!(
                k,
                -Scalar::from_u128(k1) - Scalar::ZETA * Scalar::from_u128(k2)
            )
        } else if k2_neg {
            assert_eq!(
                k,
                Scalar::from_u128(k1) + Scalar::ZETA * Scalar::from_u128(k2)
            )
        } else {
            assert_eq!(
                k,
                Scalar::from_u128(k1) - Scalar::ZETA * Scalar::from_u128(k2)
            )
        }
    }
}
