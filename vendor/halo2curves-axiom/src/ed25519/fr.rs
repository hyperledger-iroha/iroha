use core::convert::TryInto;
use core::fmt;
use core::ops::{Add, Mul, Neg, Sub};
use ff::{FromUniformBytes, PrimeField, WithSmallOrderMulGroup};
use rand::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

#[cfg(feature = "derive_serde")]
use serde::{Deserialize, Serialize};

use crate::arithmetic::{adc, bigint_geq, mac, macx, sbb};

/// This represents an element of $\mathbb{F}_q$ where
///
/// `r = 0x1000000000000000000000000000000014def9dea2f79cd65812631a5cf5d3ed`
///
/// is the scalar field of the ed25519 curve.
// The internal representation of this type is four 64-bit unsigned
// integers in little-endian order. `Fr` values are always in
// Montgomery form; i.e., Fr(a) = aR mod r, with R = 2^256.
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
pub struct Fr(pub(crate) [u64; 4]);

/// Constant representing the modulus
/// r = 0x1000000000000000000000000000000014def9dea2f79cd65812631a5cf5d3ed
const MODULUS: Fr = Fr([
    0x5812631a5cf5d3ed,
    0x14def9dea2f79cd6,
    0x0000000000000000,
    0x1000000000000000,
]);

/// The modulus as u32 limbs.
#[cfg(not(target_pointer_width = "64"))]
const MODULUS_LIMBS_32: [u32; 8] = [
    0x5cf5_d3ed,
    0x5812_631a,
    0xa2f7_9cd6,
    0x14de_f9de,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0x1000_0000,
];

///Constant representing the modulus as static str
const MODULUS_STR: &str = "0x1000000000000000000000000000000014def9dea2f79cd65812631a5cf5d3ed";

/// Obtained with sage:
/// `GF(r).primitive_element()`
const MULTIPLICATIVE_GENERATOR: Fr = Fr::from_raw([0x02, 0x0, 0x0, 0x0]);

/// INV = -(r^{-1} mod 2^64) mod 2^64
const INV: u64 = 0xd2b51da312547e1b;

/// R = 2^256 mod r
/// 0xffffffffffffffffffffffffffffffec6ef5bf4737dcf70d6ec31748d98951d
const R: Fr = Fr([
    0xd6ec31748d98951d,
    0xc6ef5bf4737dcf70,
    0xfffffffffffffffe,
    0x0fffffffffffffff,
]);

/// R^2 = 2^512 mod r
/// 0x399411b7c309a3dceec73d217f5be65d00e1ba768859347a40611e3449c0f01
const R2: Fr = Fr([
    0xa40611e3449c0f01,
    0xd00e1ba768859347,
    0xceec73d217f5be65,
    0x0399411b7c309a3d,
]);

/// R^3 = 2^768 mod r
/// 0xe530b773599cec78065dc6c04ec5b65278324e6aef7f3ec2a9e49687b83a2db
const R3: Fr = Fr([
    0x2a9e49687b83a2db,
    0x278324e6aef7f3ec,
    0x8065dc6c04ec5b65,
    0x0e530b773599cec7,
]);

/// 1 / 2 mod r
const TWO_INV: Fr = Fr::from_raw([
    0x2c09318d2e7ae9f7,
    0x0a6f7cef517bce6b,
    0x0000000000000000,
    0x0800000000000000,
]);

/// sqrt(-1) mod r = 2^((r - 1) / 4) mod r
const SQRT_MINUS_ONE: Fr = Fr::from_raw([
    0xbe8775dfebbe07d4,
    0x0ef0565342ce83fe,
    0x7d3d6d60abc1c27a,
    0x094a7310e07981e7,
]);

// Element in small order subgroup (3-order)
// Sage:
// `GF(r).primitive_element() ** ((r - 1) // N)` where N = 3
const ZETA: Fr = Fr::from_raw([
    0x158687e51e07e223,
    0x471dd911c6cce91e,
    0xeb08f579fb8841ae,
    0x0378d9ddc674005f,
]);
// The `2^s` root of unity.
// It can be calculated by exponentiating `MULTIPLICATIVE_GENERATOR` by `t`,
// where `2^s * t = r - 1` with `t` odd.
// Sage:
// `GF(r).primitive_element() ** t`
const ROOT_OF_UNITY: Fr = Fr::from_raw([
    0xbe8775dfebbe07d4,
    0x0ef0565342ce83fe,
    0x7d3d6d60abc1c27a,
    0x094a7310e07981e7,
]);
// Inverse of `ROOT_OF_UNITY`
const ROOT_OF_UNITY_INV: Fr = Fr::from_raw([
    0x998aed3a7137cc19,
    0x05eea38b602918d7,
    0x82c2929f543e3d86,
    0x06b58cef1f867e18,
]);
// Generator of the `t-order` multiplicative subgroup
// Sage:
// `GF(r).primitive_element() ** (2**s)`
const DELTA: Fr = Fr::from_raw([0x10, 0, 0, 0]);

use crate::{
    field_arithmetic, field_common, field_specific, impl_add_binop_specify_output,
    impl_binops_additive, impl_binops_additive_specify_output, impl_binops_multiplicative,
    impl_binops_multiplicative_mixed, impl_from_u64, impl_sub_binop_specify_output, impl_sum_prod,
};
impl_binops_additive!(Fr, Fr);
impl_binops_multiplicative!(Fr, Fr);
field_common!(
    Fr,
    MODULUS,
    INV,
    MODULUS_STR,
    TWO_INV,
    ROOT_OF_UNITY_INV,
    DELTA,
    ZETA,
    R,
    R2,
    R3
);
field_arithmetic!(Fr, MODULUS, INV, dense);
impl_sum_prod!(Fr);
impl_from_u64!(Fr, R2);

impl Fr {
    pub const fn size() -> usize {
        32
    }
}

impl ff::Field for Fr {
    const ZERO: Self = Self::zero();
    const ONE: Self = Self::one();

    fn random(mut rng: impl RngCore) -> Self {
        Self::from_u512([
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
            rng.next_u64(),
        ])
    }

    fn double(&self) -> Self {
        self.double()
    }

    #[inline(always)]
    fn square(&self) -> Self {
        self.square()
    }

    /// Computes the square root of this element, if it exists.
    fn sqrt(&self) -> CtOption<Self> {
        // Sqrt = a^((p + 3) / 8)
        //        OR
        //      = a^((p + 3) / 8) * sqrt(-1)
        //      = a^((p + 3) / 8) * (2^((p - 1) / 4))
        //        OR
        //        Doesn't exist
        let x1 = self.pow([
            0xcb024c634b9eba7e,
            0x029bdf3bd45ef39a,
            0x0000000000000000,
            0x0200000000000000,
        ]);

        let choice1 = x1.square().ct_eq(self);
        let choice2 = x1.square().ct_eq(&-self);

        let sqrt = Self::conditional_select(&x1, &(x1 * SQRT_MINUS_ONE), choice2);

        CtOption::new(sqrt, choice1 | choice2)
    }

    fn sqrt_ratio(num: &Self, div: &Self) -> (Choice, Self) {
        ff::helpers::sqrt_ratio_generic(num, div)
    }

    /// Computes the multiplicative inverse of this element,
    /// failing if the element is zero.
    fn invert(&self) -> CtOption<Self> {
        let tmp = self.pow_vartime([
            0x5812631a5cf5d3eb,
            0x14def9dea2f79cd6,
            0x0000000000000000,
            0x1000000000000000,
        ]);

        CtOption::new(tmp, !self.ct_eq(&Self::zero()))
    }

    fn pow_vartime<S: AsRef<[u64]>>(&self, exp: S) -> Self {
        let mut res = Self::one();
        let mut found_one = false;
        for e in exp.as_ref().iter().rev() {
            for i in (0..64).rev() {
                if found_one {
                    res = res.square();
                }

                if ((*e >> i) & 1) == 1 {
                    found_one = true;
                    res *= self;
                }
            }
        }
        res
    }
}

impl ff::PrimeField for Fr {
    type Repr = [u8; 32];

    const MODULUS: &'static str = MODULUS_STR;
    const NUM_BITS: u32 = 256;
    const CAPACITY: u32 = 255;
    const TWO_INV: Self = TWO_INV;
    const MULTIPLICATIVE_GENERATOR: Self = MULTIPLICATIVE_GENERATOR;
    // An integer `s` satisfying the equation `2^s * t = modulus - 1` with `t` odd.
    const S: u32 = 2;
    const ROOT_OF_UNITY: Self = ROOT_OF_UNITY;
    const ROOT_OF_UNITY_INV: Self = ROOT_OF_UNITY_INV;
    const DELTA: Self = DELTA;

    fn from_repr(repr: Self::Repr) -> CtOption<Self> {
        let mut tmp = Fr([0, 0, 0, 0]);

        tmp.0[0] = u64::from_le_bytes(repr[0..8].try_into().unwrap());
        tmp.0[1] = u64::from_le_bytes(repr[8..16].try_into().unwrap());
        tmp.0[2] = u64::from_le_bytes(repr[16..24].try_into().unwrap());
        tmp.0[3] = u64::from_le_bytes(repr[24..32].try_into().unwrap());

        // Try to subtract the modulus
        let (_, borrow) = sbb(tmp.0[0], MODULUS.0[0], 0);
        let (_, borrow) = sbb(tmp.0[1], MODULUS.0[1], borrow);
        let (_, borrow) = sbb(tmp.0[2], MODULUS.0[2], borrow);
        let (_, borrow) = sbb(tmp.0[3], MODULUS.0[3], borrow);

        // If the element is smaller than MODULUS then the
        // subtraction will underflow, producing a borrow value
        // of 0xffff...ffff. Otherwise, it'll be zero.
        let is_some = (borrow as u8) & 1;

        // Convert to Montgomery form by computing
        // (a.R^0 * R^2) / R = a.R
        tmp *= &R2;

        CtOption::new(tmp, Choice::from(is_some))
    }

    fn to_repr(&self) -> Self::Repr {
        // Turn into canonical form by computing
        // (a.R) / R = a
        let tmp = Fr::montgomery_reduce_short(&self.0);

        let mut res = [0; 32];
        res[0..8].copy_from_slice(&tmp.0[0].to_le_bytes());
        res[8..16].copy_from_slice(&tmp.0[1].to_le_bytes());
        res[16..24].copy_from_slice(&tmp.0[2].to_le_bytes());
        res[24..32].copy_from_slice(&tmp.0[3].to_le_bytes());

        res
    }

    fn is_odd(&self) -> Choice {
        Choice::from(self.to_repr()[0] & 1)
    }
}

impl FromUniformBytes<64> for Fr {
    /// Converts a 512-bit little endian integer into
    /// an `Fq` by reducing by the modulus.
    fn from_uniform_bytes(bytes: &[u8; 64]) -> Self {
        Self::from_u512([
            u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
            u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            u64::from_le_bytes(bytes[16..24].try_into().unwrap()),
            u64::from_le_bytes(bytes[24..32].try_into().unwrap()),
            u64::from_le_bytes(bytes[32..40].try_into().unwrap()),
            u64::from_le_bytes(bytes[40..48].try_into().unwrap()),
            u64::from_le_bytes(bytes[48..56].try_into().unwrap()),
            u64::from_le_bytes(bytes[56..64].try_into().unwrap()),
        ])
    }
}

impl WithSmallOrderMulGroup<3> for Fr {
    const ZETA: Self = ZETA;
}

#[cfg(test)]
mod test {
    use super::*;
    use ff::Field;
    use rand_core::OsRng;

    #[test]
    fn test_sqrt() {
        // NB: TWO_INV is standing in as a "random" field element
        let v = (Fr::TWO_INV).square().sqrt().unwrap();
        assert!(v == Fr::TWO_INV || (-v) == Fr::TWO_INV);

        for _ in 0..10000 {
            let a = Fr::random(OsRng);
            let mut b = a;
            b = b.square();

            let b = b.sqrt().unwrap();
            let mut negb = b;
            negb = negb.neg();

            assert!(a == b || a == negb);
        }
    }

    #[test]
    fn test_invert() {
        let v = Fr::one().double().invert().unwrap();
        assert!(v == Fr::TWO_INV);

        for _ in 0..10000 {
            let a = Fr::random(OsRng);
            let b = a.invert().unwrap().invert().unwrap();

            assert!(a == b);
        }
    }

    #[test]
    fn test_field() {
        crate::tests::field::random_field_tests::<Fr>("ed25519 scalar".to_string());
    }

    #[test]
    fn test_serialization() {
        crate::tests::field::random_serialization_test::<Fr>("ed25519 scalar".to_string());
    }
}
