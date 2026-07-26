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
/// `q = 0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffed`
///
/// is the base field of the ed25519 curve.
// The internal representation of this type is four 64-bit unsigned
// integers in little-endian order. `Fq` values are always in
// Montgomery form; i.e., Fq(a) = aR mod q, with R = 2^256.
#[derive(Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
pub struct Fq(pub(crate) [u64; 4]);

/// Constant representing the modulus
/// q = 0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffed
const MODULUS: Fq = Fq([
    0xffffffffffffffed,
    0xffffffffffffffff,
    0xffffffffffffffff,
    0x7fffffffffffffff,
]);

/// The modulus as u32 limbs.
#[cfg(not(target_pointer_width = "64"))]
const MODULUS_LIMBS_32: [u32; 8] = [
    0xffff_ffed,
    0xffff_fffe,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
    0xffff_ffff,
    0x7fff_ffff,
];

/// Constant representing the modulus as static str
const MODULUS_STR: &str = "0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffed";

/// Obtained with sage:
/// `GF(q).primitive_element()`
const MULTIPLICATIVE_GENERATOR: Fq = Fq::from_raw([0x02, 0x0, 0x0, 0x0]);

/// INV = -(q^{-1} mod 2^64) mod 2^64
const INV: u64 = 0x86bca1af286bca1b;

/// R = 2^256 mod q
/// 0x26
const R: Fq = Fq([0x26, 0, 0, 0]);

/// R^2 = 2^512 mod q
/// 0x5a4
const R2: Fq = Fq([0x5a4, 0, 0, 0]);

/// R^3 = 2^768 mod q
/// 0xd658
const R3: Fq = Fq([0xd658, 0, 0, 0]);

/// 1 / 2 mod q
const TWO_INV: Fq = Fq::from_raw([
    0xfffffffffffffff7,
    0xffffffffffffffff,
    0xffffffffffffffff,
    0x3fffffffffffffff,
]);

/// sqrt(-1) mod q = 2^((q - 1) / 4) mod q
const SQRT_MINUS_ONE: Fq = Fq::from_raw([
    0xc4ee1b274a0ea0b0,
    0x2f431806ad2fe478,
    0x2b4d00993dfbd7a7,
    0x2b8324804fc1df0b,
]);

// Element in small order subgroup (3-order)
// Sage:
// `GF(q).primitive_element() ** ((q - 1) // N)` where N = 3
const ZETA: Fq = Fq::from_raw([
    0xaa86d89d8618e538,
    0x1a1aada8413a4550,
    0xd9872fccc55bd529,
    0x381cba36aa6565b5,
]);
// The `2^s` root of unity.
// It can be calculated by exponentiating `MULTIPLICATIVE_GENERATOR` by `t`,
// where `2^s * t = q - 1` with `t` odd.
// Sage:
// `GF(q).primitive_element() ** t`
const ROOT_OF_UNITY: Fq = Fq::from_raw([
    0xc4ee1b274a0ea0b0,
    0x2f431806ad2fe478,
    0x2b4d00993dfbd7a7,
    0x2b8324804fc1df0b,
]);
// Inverse of `ROOT_OF_UNITY`
const ROOT_OF_UNITY_INV: Fq = Fq::from_raw([
    0x3b11e4d8b5f15f3d,
    0xd0bce7f952d01b87,
    0xd4b2ff66c2042858,
    0x547cdb7fb03e20f4,
]);
// Generator of the `t-order` multiplicative subgroup
// Sage:
// `GF(q).primitive_element() ** (2**s)`
const DELTA: Fq = Fq::from_raw([0x10, 0, 0, 0]);

use crate::{
    field_arithmetic, field_common, field_specific, impl_add_binop_specify_output,
    impl_binops_additive, impl_binops_additive_specify_output, impl_binops_multiplicative,
    impl_binops_multiplicative_mixed, impl_from_u64, impl_sub_binop_specify_output, impl_sum_prod,
};
impl_binops_additive!(Fq, Fq);
impl_binops_multiplicative!(Fq, Fq);
field_common!(
    Fq,
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
field_arithmetic!(Fq, MODULUS, INV, dense);
impl_sum_prod!(Fq);
impl_from_u64!(Fq, R2);

impl Fq {
    pub const fn size() -> usize {
        32
    }
}

impl ff::Field for Fq {
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
        // Sqrt = a^((q + 3) / 8)
        //        OR
        //      = a^((q + 3) / 8) * sqrt(-1)
        //      = a^((q + 3) / 8) * (2^((q - 1) / 4))
        //        OR
        //        Doesn't exist
        let x1 = self.pow([
            0xfffffffffffffffe,
            0xffffffffffffffff,
            0xffffffffffffffff,
            0x0fffffffffffffff,
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
        // a^(-1) = a^(q - 2)
        let tmp = self.pow_vartime([
            0xffffffffffffffeb,
            0xffffffffffffffff,
            0xffffffffffffffff,
            0x7fffffffffffffff,
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

impl ff::PrimeField for Fq {
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
        let mut tmp = Fq([0, 0, 0, 0]);

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
        let tmp = Fq::montgomery_reduce_short(&self.0);

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

impl FromUniformBytes<64> for Fq {
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

impl WithSmallOrderMulGroup<3> for Fq {
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
        let v = (Fq::TWO_INV).square().sqrt().unwrap();
        assert!(v == Fq::TWO_INV || (-v) == Fq::TWO_INV);

        for _ in 0..10000 {
            let a = Fq::random(OsRng);
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
        let v = Fq::one().double().invert().unwrap();
        assert!(v == Fq::TWO_INV);

        for _ in 0..10000 {
            let a = Fq::random(OsRng);
            let b = a.invert().unwrap().invert().unwrap();

            assert!(a == b);
        }
    }

    #[test]
    fn test_field() {
        crate::tests::field::random_field_tests::<Fq>("ed25519 base".to_string());
    }

    #[test]
    fn test_serialization() {
        crate::tests::field::random_serialization_test::<Fq>("ed25519 base".to_string());
    }
}
