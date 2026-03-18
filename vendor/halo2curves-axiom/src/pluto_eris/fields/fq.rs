use crate::arithmetic::{adc, mac, sbb};
use crate::ff::{FromUniformBytes, PrimeField, WithSmallOrderMulGroup};
use crate::{
    extend_field_legendre, field_arithmetic_7_limbs, field_bits_7_limbs, field_common_7_limbs,
    impl_from_u64_7_limbs,
};
use crate::{
    impl_add_binop_specify_output, impl_binops_additive, impl_binops_additive_specify_output,
    impl_binops_multiplicative, impl_binops_multiplicative_mixed, impl_sub_binop_specify_output,
    impl_sum_prod,
};
use core::convert::TryInto;
use core::fmt;
use core::ops::{Add, Mul, Neg, Sub};
use rand::RngCore;
use std::slice::Iter;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

#[cfg(feature = "derive_serde")]
use serde::{Deserialize, Serialize};

/// This represents an element of $\mathbb{F}_q$ where
///
/// `q = 0x24000000000024000130e0000d7f70e4a803ca76f439266f443f9a5c7a8a6c7be4a775fe8e177fd69ca7e85d60050af41ffffcd300000001`
///
/// is the scalar field of the Pluto curve (and the base field of the Eris curve).
/// The internal representation of this type is seven 64-bit unsigned
/// integers in little-endian order which account for the 446 bits required to be represented.
/// `Fq` values are always in Montgomery form; i.e., Fq(a) = aR mod q, with R = 2^448.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
pub struct Fq(pub(crate) [u64; 7]);

/// Size of `Fq` element in bytes
const SIZE: usize = 56;

/// Constant representing the modulus
/// `p = 0x24000000000024000130e0000d7f70e4a803ca76f439266f443f9a5c7a8a6c7be4a775fe8e177fd69ca7e85d60050af41ffffcd300000001`
const MODULUS: Fq = Fq([
    0x1ffffcd300000001,
    0x9ca7e85d60050af4,
    0xe4a775fe8e177fd6,
    0x443f9a5c7a8a6c7b,
    0xa803ca76f439266f,
    0x0130e0000d7f70e4,
    0x2400000000002400,
]);

/// The modulus as u32 limbs.
#[cfg(not(target_pointer_width = "64"))]
const MODULUS_LIMBS_32: [u32; 14] = [
    0x00000001, 0x1ffffcd3, 0x60050af4, 0x9ca7e85d, 0x8e177fd6, 0xe4a775fe, 0x7a8a6c7b, 0x443f9a5c,
    0xf439266f, 0xa803ca76, 0x0d7f70e4, 0x0130e000, 0x00002400, 0x24000000,
];

const MODULUS_STR: &str = "0x24000000000024000130e0000d7f70e4a803ca76f439266f443f9a5c7a8a6c7be4a775fe8e177fd69ca7e85d60050af41ffffcd300000001";

/// INV = -(q^{-1} mod 2^64) mod 2^64
/// `0x1ffffcd2ffffffff`
const INV: u64 = 0x1ffffcd2ffffffff;

/// Let M be the power of `2^64` nearest to `Self::MODULUS_BITS`. Then `R = M % Self::MODULUS`.
/// `R = 2^448 mod q`
/// `0x3ffffffffff03fff7a9dfffa183e9bf67e576bf526ff2f52242c778a637089cbf6bc60a1d5b8121b768a5725fdcb3532000163afffffff9`
const R: Fq = Fq([
    0x2000163afffffff9,
    0xb768a5725fdcb353,
    0xbf6bc60a1d5b8121,
    0x2242c778a637089c,
    0x67e576bf526ff2f5,
    0xf7a9dfffa183e9bf,
    0x3ffffffffff03ff,
]);

/// `R^2 = 2^896 mod q`
/// `0x50d7c998f46144ee436895a5a630ff544d51e923f64695651da4da1c97f716419bd905e6e4ff6c2bc64e865fe4552ad740808c831022522`
const R2: Fq = Fq([
    0x740808c831022522,
    0xbc64e865fe4552ad,
    0x19bd905e6e4ff6c2,
    0x51da4da1c97f7164,
    0x44d51e923f646956,
    0xe436895a5a630ff5,
    0x050d7c998f46144e,
]);

/// `R^3 = 2^1792 mod q`
/// `0x2f2c41fb476072baa10b8225e69f7de3b2c1031e6d01279e65191fab1f6ce25295c3c8bd6945406c89b51b218477a6f7252704d7495b38a`
const R3: Fq = Fq([
    0x7252704d7495b38a,
    0xc89b51b218477a6f,
    0x295c3c8bd6945406,
    0xe65191fab1f6ce25,
    0x3b2c1031e6d01279,
    0xaa10b8225e69f7de,
    0x02f2c41fb476072b,
]);

/// `GENERATOR = 7 mod q` is a generator of the `q - 1` order multiplicative
/// subgroup, or in other words a primitive root of the field.
const GENERATOR: Fq = Fq::from_raw([0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

/// Size of the 2-adic sub-group of the field.
const S: u32 = 32;

/// GENERATOR^t where t * 2^s + 1 = q
/// with t odd. In other words, this is a 2^s root of unity.
/// `0x0a5e6f78289fd24b1c64c90821c44cdce9ba1b3e90f2e88957f869667f6dfdbdbce6bb9ed38a8c2382fa11e3d3810fcc3c7bb406ec7bce04`

const ROOT_OF_UNITY: Fq = Fq::from_raw([
    0x3c7bb406ec7bce04,
    0x82fa11e3d3810fcc,
    0xbce6bb9ed38a8c23,
    0x57f869667f6dfdbd,
    0xe9ba1b3e90f2e889,
    0x1c64c90821c44cdc,
    0x0a5e6f78289fd24b,
]);

/// 1 / ROOT_OF_UNITY mod q
/// `0x1a8c636e293fe9928f85aa6ec68f950ebb57e3f0502dd05667c990c1c2f57128c77768be1824fd3f60869f410287a1879ec16a35ca69b6fb`

const ROOT_OF_UNITY_INV: Fq = Fq::from_raw([
    0x9ec16a35ca69b6fb,
    0x60869f410287a187,
    0xc77768be1824fd3f,
    0x67c990c1c2f57128,
    0xbb57e3f0502dd056,
    0x8f85aa6ec68f950e,
    0x1a8c636e293fe992,
]);

/// 1 / 2 mod q
/// `0x12000000000012000098700006bfb8725401e53b7a1c9337a21fcd2e3d45363df253baff470bbfeb4e53f42eb002857a0ffffe6980000001`
const TWO_INV: Fq = Fq::from_raw([
    0x0ffffe6980000001,
    0x4e53f42eb002857a,
    0xf253baff470bbfeb,
    0xa21fcd2e3d45363d,
    0x5401e53b7a1c9337,
    0x0098700006bfb872,
    0x1200000000001200,
]);

/// GENERATOR^{2^s} where t * 2^s + 1 = q with t odd. In other words, this is a t root of unity.
/// 0x657946fe07116ceca983fe28713a2b257ab7a7866c95121e727f3776c3e84cb0a14f6a7f83f8cdaeadb479c657bdf2de4589640faf72e67
const DELTA: Fq = Fq::from_raw([
    0xe4589640faf72e67,
    0xeadb479c657bdf2d,
    0x0a14f6a7f83f8cda,
    0xe727f3776c3e84cb,
    0x57ab7a7866c95121,
    0xca983fe28713a2b2,
    0x657946fe07116ce,
]);

/// `ZETA^3 = 1 mod q` where `ZETA^2 != 1 mod q`
/// `0x9000000000006c000392a0001afee1c9500792ae3039253e641ba35817a29ffaf50be000032cfffffffe`

const ZETA: Fq = Fq::from_raw([
    0xe000032cfffffffe,
    0xa35817a29ffaf50b,
    0x92ae3039253e641b,
    0xa0001afee1c95007,
    0x000000006c000392,
    0x0000000000009000,
    0x0000000000000000,
]);

impl_binops_additive!(Fq, Fq);
impl_binops_multiplicative!(Fq, Fq);
field_common_7_limbs!(
    Fq,
    FqRepr,
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
impl_sum_prod!(Fq);
impl_from_u64_7_limbs!(Fq, R2);
field_arithmetic_7_limbs!(Fq, MODULUS, INV, sparse);

#[cfg(target_pointer_width = "64")]
field_bits_7_limbs!(Fq, MODULUS);
#[cfg(not(target_pointer_width = "64"))]
field_bits_7_limbs!(Fq, MODULUS, MODULUS_LIMBS_32);

extend_field_legendre!(Fq);

impl Fq {
    /// Return field element size in bytes.
    pub const fn size() -> usize {
        SIZE
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

    /// Computes the multiplicative inverse of this element,
    /// failing if the element is zero.
    fn invert(&self) -> CtOption<Self> {
        // self^(q - 2)
        let tmp = self.pow_vartime([
            0x1ffffcd2ffffffff,
            0x9ca7e85d60050af4,
            0xe4a775fe8e177fd6,
            0x443f9a5c7a8a6c7b,
            0xa803ca76f439266f,
            0x0130e0000d7f70e4,
            0x2400000000002400,
        ]);

        CtOption::new(tmp, !self.ct_eq(&Self::zero()))
    }

    fn sqrt(&self) -> CtOption<Self> {
        /// `(t - 1) // 2` where t * 2^s + 1 = q with t odd.
        const T_MINUS1_OVER2: [u64; 7] = [
            0xb002857a0ffffe69,
            0x470bbfeb4e53f42e,
            0x3d45363df253baff,
            0x7a1c9337a21fcd2e,
            0x06bfb8725401e53b,
            0x0000120000987000,
            0x0000000012000000,
        ];
        ff::helpers::sqrt_tonelli_shanks(self, T_MINUS1_OVER2)
    }

    fn sqrt_ratio(num: &Self, div: &Self) -> (Choice, Self) {
        ff::helpers::sqrt_ratio_generic(num, div)
    }
}

#[derive(Clone, Copy, Debug)]
/// Canonical little-endian representation of a `Fq` element.
pub struct FqRepr {
    pub repr: [u8; SIZE],
}

impl FqRepr {
    /// Returns an iterator over the bytes of the canoncial representation of the element.
    pub fn iter(&self) -> Iter<'_, u8> {
        self.repr.iter()
    }
}

impl Default for FqRepr {
    fn default() -> Self {
        FqRepr { repr: [0u8; SIZE] }
    }
}

impl AsRef<[u8]> for FqRepr {
    fn as_ref(&self) -> &[u8] {
        self.repr.as_ref()
    }
}

impl AsMut<[u8]> for FqRepr {
    fn as_mut(&mut self) -> &mut [u8] {
        self.repr.as_mut()
    }
}
impl From<[u8; SIZE]> for FqRepr {
    fn from(repr: [u8; SIZE]) -> Self {
        Self { repr }
    }
}

impl ff::PrimeField for Fq {
    type Repr = FqRepr;

    const NUM_BITS: u32 = 446;
    const CAPACITY: u32 = 445;
    const MODULUS: &'static str = MODULUS_STR;
    const MULTIPLICATIVE_GENERATOR: Self = GENERATOR;
    const ROOT_OF_UNITY: Self = ROOT_OF_UNITY;
    const ROOT_OF_UNITY_INV: Self = ROOT_OF_UNITY_INV;
    const TWO_INV: Self = TWO_INV;
    const DELTA: Self = DELTA;
    const S: u32 = S;

    fn from_repr(repr: Self::Repr) -> CtOption<Self> {
        let mut tmp = Self([0, 0, 0, 0, 0, 0, 0]);
        let repr = repr.repr;

        tmp.0[0] = u64::from_le_bytes(repr[0..8].try_into().unwrap());
        tmp.0[1] = u64::from_le_bytes(repr[8..16].try_into().unwrap());
        tmp.0[2] = u64::from_le_bytes(repr[16..24].try_into().unwrap());
        tmp.0[3] = u64::from_le_bytes(repr[24..32].try_into().unwrap());
        tmp.0[4] = u64::from_le_bytes(repr[32..40].try_into().unwrap());
        tmp.0[5] = u64::from_le_bytes(repr[40..48].try_into().unwrap());
        tmp.0[6] = u64::from_le_bytes(repr[48..56].try_into().unwrap());

        // Try to subtract the modulus
        let (_, borrow) = sbb(tmp.0[0], MODULUS.0[0], 0);
        let (_, borrow) = sbb(tmp.0[1], MODULUS.0[1], borrow);
        let (_, borrow) = sbb(tmp.0[2], MODULUS.0[2], borrow);
        let (_, borrow) = sbb(tmp.0[3], MODULUS.0[3], borrow);
        let (_, borrow) = sbb(tmp.0[4], MODULUS.0[4], borrow);
        let (_, borrow) = sbb(tmp.0[5], MODULUS.0[5], borrow);
        let (_, borrow) = sbb(tmp.0[6], MODULUS.0[6], borrow);

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
        let tmp = Self::montgomery_reduce(&[
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], 0, 0, 0,
            0, 0, 0, 0,
        ]);

        let mut res = [0; SIZE];
        res[0..8].copy_from_slice(&tmp.0[0].to_le_bytes());
        res[8..16].copy_from_slice(&tmp.0[1].to_le_bytes());
        res[16..24].copy_from_slice(&tmp.0[2].to_le_bytes());
        res[24..32].copy_from_slice(&tmp.0[3].to_le_bytes());
        res[32..40].copy_from_slice(&tmp.0[4].to_le_bytes());
        res[40..48].copy_from_slice(&tmp.0[5].to_le_bytes());
        res[48..56].copy_from_slice(&tmp.0[6].to_le_bytes());
        res.into()
    }

    fn is_odd(&self) -> Choice {
        Choice::from(self.to_repr().repr[0] & 1)
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
    use crate::serde::SerdeObject;

    use super::*;
    use ark_std::{end_timer, start_timer};
    use ff::Field;
    use rand::SeedableRng;
    use rand_core::OsRng;
    use rand_xorshift::XorShiftRng;

    #[test]
    fn test_sqrt() {
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
    fn test_field() {
        crate::tests::field::random_field_tests::<Fq>("Pluto scalar".to_string());
    }

    #[test]
    fn test_zeta() {
        assert_eq!(Fq::ZETA * Fq::ZETA * Fq::ZETA, Fq::ONE);
        assert_ne!(Fq::ZETA * Fq::ZETA, Fq::ONE);
    }

    #[test]
    fn test_delta() {
        assert_eq!(Fq::DELTA, GENERATOR.pow([1u64 << Fq::S]));
        assert_eq!(Fq::DELTA, Fq::MULTIPLICATIVE_GENERATOR.pow([1u64 << Fq::S]));
    }

    #[test]
    fn test_from_u512() {
        const N_VECS: usize = 10;
        let expected_results = [
            Fq::from_raw([
                0x93638251ffeffed3,
                0xb17ab6ae332352b4,
                0xbf2731af91057325,
                0x7b700ef5a22260d0,
                0xc97c59318d325250,
                0xd7bc83d286537318,
                0x01d4a87b24f91154,
            ]),
            Fq::from_raw([
                0x63e0a8f1beefc612,
                0x080f69572a9ddaae,
                0xb9ff1cf0e1f7c067,
                0xd8d8bf5b522bc48b,
                0xa7607085c7065359,
                0x617d8b0cda3f6328,
                0x03096ea964e009c0,
            ]),
            Fq::from_raw([
                0x5eaedbda63b3e431,
                0x90ebbfa6f11a9266,
                0x4528cf4d506c9f9b,
                0x8c6ac679e9ac3856,
                0x001666755d9c2c57,
                0x9f7f457a48d6d322,
                0x20b2fadc6bf4004d,
            ]),
            Fq::from_raw([
                0xeeea9cbd68b174cf,
                0x84af9e4ce5a781a5,
                0x3578772b5b482647,
                0x6b202eb54b7df723,
                0x55f541b1436b7660,
                0x2045de539849b035,
                0x1d5d7b5f6e8cc333,
            ]),
            Fq::from_raw([
                0xe73df0f69b71a763,
                0xbccfb84010979d9d,
                0x1ce3c87be8bf3247,
                0x695fde61877cb617,
                0x5006663bd0944209,
                0xd7ead2b7c71e460d,
                0x0f7c36b781cba9ed,
            ]),
            Fq::from_raw([
                0xaeed10e8f00b189d,
                0x5190807038915743,
                0x90b840c0a13b0307,
                0x20fa8cc52c3a9a28,
                0xc3f229646be29c1d,
                0xb1d2bb5373270c43,
                0x0e18a3597be61302,
            ]),
            Fq::from_raw([
                0xffbbc6b3e494ca68,
                0x30d4a100158c1751,
                0x0328dae560dff403,
                0x1495c3ce50cce340,
                0x93efc4d4d6ea0079,
                0x0a791ad7698655a7,
                0x22b10d5c1090eec8,
            ]),
            Fq::from_raw([
                0xd96eec60211ad67b,
                0x4d081a969b3d8488,
                0x57c9b5abbeec4cf0,
                0x13ced15637e4b0eb,
                0xb98a559f49b0071c,
                0x819ba919d0b6e9b5,
                0x20f73876330a90e8,
            ]),
            Fq::from_raw([
                0xbade57a48e2d9868,
                0xc688e43e21f9d2fc,
                0x848a82da9e1d75dc,
                0xae5f4536b9d60aa7,
                0x7957f2028c96467b,
                0xf850f49359458652,
                0x17ba9f9aa08b9ee2,
            ]),
            Fq::from_raw([
                0xd0239c8282ccc372,
                0x4a777ad0b66181ea,
                0x53737d5f19e61bfc,
                0x5340b579fe7c4c83,
                0x8406f69a0f89f90a,
                0xd7d5d8bc4497465a,
                0x08ce8bee1323d4f9,
            ]),
        ];

        let mut seeded_rng = XorShiftRng::seed_from_u64(0u64);
        let uniform_bytes = std::iter::from_fn(|| {
            let mut bytes = [0u8; 64];
            seeded_rng.fill_bytes(&mut bytes);
            Some(bytes)
        })
        .take(N_VECS)
        .collect::<Vec<_>>();

        for i in 0..N_VECS {
            let q = Fq::from_uniform_bytes(&uniform_bytes[i]);
            assert_eq!(expected_results[i], q);
        }
    }

    #[test]
    #[cfg(feature = "bits")]
    fn test_bits() {
        crate::tests::field::random_bits_tests::<Fq>("Fq".to_string());
    }

    #[test]
    fn test_serialization() {
        crate::tests::field::random_serialization_test::<Fq>("Fq".to_string());
        #[cfg(feature = "derive_serde")]
        crate::tests::field::random_serde_test::<Fq>("Fq".to_string());
    }

    fn is_less_than(x: &[u64; 7], y: &[u64; 7]) -> bool {
        match x[6].cmp(&y[6]) {
            core::cmp::Ordering::Less => return true,
            core::cmp::Ordering::Greater => return false,
            _ => {}
        }
        match x[5].cmp(&y[5]) {
            core::cmp::Ordering::Less => return true,
            core::cmp::Ordering::Greater => return false,
            _ => {}
        }
        match x[4].cmp(&y[4]) {
            core::cmp::Ordering::Less => return true,
            core::cmp::Ordering::Greater => return false,
            _ => {}
        }
        match x[3].cmp(&y[3]) {
            core::cmp::Ordering::Less => return true,
            core::cmp::Ordering::Greater => return false,
            _ => {}
        }
        match x[2].cmp(&y[2]) {
            core::cmp::Ordering::Less => return true,
            core::cmp::Ordering::Greater => return false,
            _ => {}
        }
        match x[1].cmp(&y[1]) {
            core::cmp::Ordering::Less => return true,
            core::cmp::Ordering::Greater => return false,
            _ => {}
        }
        x[0].lt(&y[0])
    }

    #[test]
    fn test_serialization_check() {
        let mut rng = XorShiftRng::from_seed([
            0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06,
            0xbc, 0xe5,
        ]);
        let start = start_timer!(|| "serialize Fq");
        // failure check
        for _ in 0..1000000 {
            let rand_word = [(); 7].map(|_| rng.next_u64());
            let a = Fq(rand_word);
            let rand_bytes = a.to_raw_bytes();
            match is_less_than(&rand_word, &MODULUS.0) {
                false => {
                    assert!(Fq::from_raw_bytes(&rand_bytes).is_none());
                }
                _ => {
                    assert_eq!(Fq::from_raw_bytes(&rand_bytes), Some(a));
                }
            }
        }
        end_timer!(start);
    }
}
