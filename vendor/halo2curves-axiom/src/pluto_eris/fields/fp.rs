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

/// This represents an element of $\mathbb{F}_p$ where
///
/// `p = 0x24000000000024000130e0000d7f70e4a803ca76f439266f443f9a5cda8a6c7be4a7a5fe8fadffd6a2a7e8c30006b9459ffffcd300000001`
///
/// is the base field of the Pluto curve.
/// The internal representation of this type is seven 64-bit unsigned
/// integers in little-endian order which account for the 446 bits required to be represented.
/// `Fp` values are always in Montgomery form; i.e., Fp(a) = aR mod p, with R = 2^448.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
pub struct Fp(pub(crate) [u64; 7]);

/// Size of `Fp` element in bytes
const SIZE: usize = 56;

/// Constant representing the modulus
/// p = 0x24000000000024000130e0000d7f70e4a803ca76f439266f443f9a5cda8a6c7be4a7a5fe8fadffd6a2a7e8c30006b9459ffffcd300000001
const MODULUS: Fp = Fp([
    0x9ffffcd300000001,
    0xa2a7e8c30006b945,
    0xe4a7a5fe8fadffd6,
    0x443f9a5cda8a6c7b,
    0xa803ca76f439266f,
    0x0130e0000d7f70e4,
    0x2400000000002400,
]);

/// The modulus as u32 limbs.
#[cfg(not(target_pointer_width = "64"))]
const MODULUS_LIMBS_32: [u32; 14] = [
    0x00000001, 0x9ffffcd3, 0x0006b945, 0xa2a7e8c3, 0x8fadffd6, 0xe4a7a5fe, 0xda8a6c7b, 0x443f9a5c,
    0xf439266f, 0xa803ca76, 0x0d7f70e4, 0x0130e000, 0x00002400, 0x24000000,
];

// pub const NEGATIVE_ONE: Fp = Fp([]);

pub(crate) const MODULUS_STR: &str = "0x24000000000024000130e0000d7f70e4a803ca76f439266f443f9a5cda8a6c7be4a7a5fe8fadffd6a2a7e8c30006b9459ffffcd300000001";

/// INV = -r^{-1} mod 2^64
/// `0x9ffffcd2ffffffff`
const INV: u64 = 0x9ffffcd2ffffffff;

/// Let M be the power of `2^64` nearest to `Self::MODULUS_BITS`. Then `R = M % Self::MODULUS`.
/// `R = 2^448 mod p`
/// `0x3ffffffffff03fff7a9dfffa183e9bf67e576bf526ff2f52242c7760637089cbf6a760a123e01218d68a2aaffd0ef18a000163afffffff9`
const R: Fp = Fp([
    0xa000163afffffff9,
    0x8d68a2aaffd0ef18,
    0xbf6a760a123e0121,
    0x2242c7760637089c,
    0x67e576bf526ff2f5,
    0xf7a9dfffa183e9bf,
    0x03ffffffffff03ff,
]);

/// `R^2 = 2^896 mod p`
/// `0x1a4b16581f66e3cc8bcb0f20758aec8520b6db3d7481a84c734fd363b575c23e7a42067a8ccd154b4b20c07277ae01f1d9702c6d54dc0598`
const R2: Fp = Fp([
    0xd9702c6d54dc0598,
    0x4b20c07277ae01f1,
    0x7a42067a8ccd154b,
    0x734fd363b575c23e,
    0x20b6db3d7481a84c,
    0x8bcb0f20758aec85,
    0x1a4b16581f66e3cc,
]);

/// `R^3 = 2^1792 mod p`
/// `0x1f51e40a048ddc1789010189f4df0ae1f3bc57efac4b3280b25aa8b46a40b225e5446680e4c4ea0449937d6b40e58f05c67afa3fe916dd69`
const R3: Fp = Fp([
    0xc67afa3fe916dd69,
    0x49937d6b40e58f05,
    0xe5446680e4c4ea04,
    0xb25aa8b46a40b225,
    0xf3bc57efac4b3280,
    0x89010189f4df0ae1,
    0x1f51e40a048ddc17,
]);

/// `GENERATOR = 10 mod p` is a generator of the `p - 1` order multiplicative
/// subgroup, or in other words a primitive root of the field.
const GENERATOR: Fp = Fp::from_raw([0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

/// Size of the 2-adic sub-group of the field.
const S: u32 = 32;

/// GENERATOR^t where t * 2^s + 1 = p
/// with t odd. In other words, this
/// is a 2^s root of unity.
/// `0x2d39f8c5f9adb3f35fe3f4222db17451ddd9602a013af5276bdbe3903ec85fc889232f5c8bc6857060c75e6f399661d6c7b82d31d563091`
const ROOT_OF_UNITY: Fp = Fp::from_raw([
    0x6c7b82d31d563091,
    0x060c75e6f399661d,
    0x889232f5c8bc6857,
    0x76bdbe3903ec85fc,
    0x1ddd9602a013af52,
    0x35fe3f4222db1745,
    0x02d39f8c5f9adb3f,
]);

/// 1 / ROOT_OF_UNITY mod p
/// `0x17725d635b00cda4153eb10c7105919d012822bd86c08691803272fbc5c9f8378055eb56ae2d55f9272bf208aad57f666deaead2c693ff66`
const ROOT_OF_UNITY_INV: Fp = Fp::from_raw([
    0x6deaead2c693ff66,
    0x272bf208aad57f66,
    0x8055eb56ae2d55f9,
    0x803272fbc5c9f837,
    0x012822bd86c08691,
    0x153eb10c7105919d,
    0x17725d635b00cda4,
]);

/// 1 / 2 mod p
/// `0x12000000000012000098700006bfb8725401e53b7a1c9337a21fcd2e6d45363df253d2ff47d6ffeb5153f46180035ca2cffffe6980000001`
pub(crate) const TWO_INV: Fp = Fp::from_raw([
    0xcffffe6980000001,
    0x5153f46180035ca2,
    0xf253d2ff47d6ffeb,
    0xa21fcd2e6d45363d,
    0x5401e53b7a1c9337,
    0x0098700006bfb872,
    0x1200000000001200,
]);
/// GENERATOR^{2^s} where t * 2^s + 1 = r with t odd. In other words, this is a t root of unity.
/// `0xeacefc6504d028d42ed23fc8766d5a5f195b456887e1e0021fb760c53233e9170c23749b459b95cc6cbb5faf3754a1e1916b2007775db04`
const DELTA: Fp = Fp::from_raw([
    0x1916b2007775db04,
    0xc6cbb5faf3754a1e,
    0x70c23749b459b95c,
    0x21fb760c53233e91,
    0xf195b456887e1e00,
    0x42ed23fc8766d5a5,
    0x0eacefc6504d028d,
]);

/// `ZETA^3 = 1 mod p` where `ZETA^2 != 1 mod p`
/// `0x480000000000360001c950000d7ee0e4a803c956d01c903d720dc8ad8b38dffaf50c100004c37ffffffe`
const ZETA: Fp = Fp::from_raw([
    0x100004c37ffffffe,
    0xc8ad8b38dffaf50c,
    0xc956d01c903d720d,
    0x50000d7ee0e4a803,
    0x00000000360001c9,
    0x0000000000004800,
    0x0000000000000000,
]);

/// NEG_ONE ; -1 mod p
pub(crate) const NEG_ONE: Fp = Fp::from_raw([
    0x9ffffcd300000000,
    0xa2a7e8c30006b945,
    0xe4a7a5fe8fadffd6,
    0x443f9a5cda8a6c7b,
    0xa803ca76f439266f,
    0x0130e0000d7f70e4,
    0x2400000000002400,
]);

impl_binops_additive!(Fp, Fp);
impl_binops_multiplicative!(Fp, Fp);
field_common_7_limbs!(
    Fp,
    FpRepr,
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
impl_sum_prod!(Fp);
impl_from_u64_7_limbs!(Fp, R2);
field_arithmetic_7_limbs!(Fp, MODULUS, INV, sparse);

#[cfg(target_pointer_width = "64")]
field_bits_7_limbs!(Fp, MODULUS);
#[cfg(not(target_pointer_width = "64"))]
field_bits_7_limbs!(Fp, MODULUS, MODULUS_LIMBS_32);

extend_field_legendre!(Fp);

impl Fp {
    pub const fn size() -> usize {
        SIZE
    }
}

impl ff::Field for Fp {
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
        // self^(p - 2)
        let tmp = self.pow([
            0x9ffffcd2ffffffff,
            0xa2a7e8c30006b945,
            0xe4a7a5fe8fadffd6,
            0x443f9a5cda8a6c7b,
            0xa803ca76f439266f,
            0x0130e0000d7f70e4,
            0x2400000000002400,
        ]);

        CtOption::new(tmp, !self.ct_eq(&Self::zero()))
    }

    fn sqrt(&self) -> CtOption<Self> {
        /// `(t - 1) // 2` where t * 2^s + 1 = p with t odd.
        const T_MINUS1_OVER2: [u64; 7] = [
            0x80035ca2cffffe69,
            0x47d6ffeb5153f461,
            0x6d45363df253d2ff,
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
/// Canonical little-endian representation of a `Fp` element.
pub struct FpRepr {
    pub repr: [u8; SIZE],
}

impl FpRepr {
    /// Returns an iterator over the bytes of the canoncial representation of the element.
    pub fn iter(&self) -> Iter<'_, u8> {
        self.repr.iter()
    }
}

impl Default for FpRepr {
    fn default() -> Self {
        FpRepr { repr: [0u8; SIZE] }
    }
}

impl AsRef<[u8]> for FpRepr {
    fn as_ref(&self) -> &[u8] {
        self.repr.as_ref()
    }
}

impl AsMut<[u8]> for FpRepr {
    fn as_mut(&mut self) -> &mut [u8] {
        self.repr.as_mut()
    }
}
impl From<[u8; SIZE]> for FpRepr {
    fn from(repr: [u8; SIZE]) -> Self {
        Self { repr }
    }
}

impl ff::PrimeField for Fp {
    type Repr = FpRepr;

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

impl FromUniformBytes<64> for Fp {
    /// Converts a 512-bit little endian integer into
    /// an `Fp` by reducing by the modulus.
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

impl WithSmallOrderMulGroup<3> for Fp {
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
        let v = (Fp::TWO_INV).square().sqrt().unwrap();
        assert!(v == Fp::TWO_INV || (-v) == Fp::TWO_INV);

        for _ in 0..10000 {
            let a = Fp::random(OsRng);
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
        crate::tests::field::random_field_tests::<Fp>("Eris scalar".to_string());
    }

    #[test]
    fn test_delta() {
        assert_eq!(Fp::DELTA, GENERATOR.pow([1u64 << Fp::S]));
        assert_eq!(Fp::DELTA, Fp::MULTIPLICATIVE_GENERATOR.pow([1u64 << Fp::S]));
    }

    #[test]
    fn test_zeta() {
        assert_eq!(Fp::ZETA * Fp::ZETA * Fp::ZETA, Fp::ONE);
        assert_ne!(Fp::ZETA * Fp::ZETA, Fp::ONE);
    }

    #[test]
    fn test_from_u512() {
        const N_VECS: usize = 10;
        let expected_results = [
            Fp::from_raw([
                0x93638251ffeffed3,
                0x4d32f4d20020be11,
                0x9c39ee168df390f0,
                0xaeef355d313cce4b,
                0xc97c592ef6030675,
                0xd7bc83d286537318,
                0x01d4a87b24f91154,
            ]),
            Fp::from_raw([
                0x63e0a8f1beefc612,
                0xbb28d56dae950a42,
                0x5264111f4a5ea3ad,
                0xbebe71c829f662f7,
                0xa760708568d6060c,
                0x617d8b0cda3f6328,
                0x03096ea964e009c0,
            ]),
            Fp::from_raw([
                0xdeaedbda63b3e431,
                0x65892bc45ec174c8,
                0x83ad8d96c18556c7,
                0x3fce5f9d2c537fbe,
                0x001666753a4972d1,
                0x9f7f457a48d6d322,
                0x20b2fadc6bf4004d,
            ]),
            Fp::from_raw([
                0x6eea9cbd68b174cf,
                0x63aa4abda18f73e6,
                0x0a6ccc999b1c7864,
                0x0f90b43928625cc2,
                0x55f541b0680af76b,
                0x2045de539849b035,
                0x1d5d7b5f6e8cc333,
            ]),
            Fp::from_raw([
                0x673df0f69b71a763,
                0x215a1362cfe53e1e,
                0x7028d2b3766b0f40,
                0x996ac521f57a7f05,
                0x5006663a5c8cea53,
                0xd7ead2b7c71e460d,
                0x0f7c36b781cba9ed,
            ]),
            Fp::from_raw([
                0x2eed10e8f00b189d,
                0xe6c79fb4600e94d4,
                0x2a9066b23daac6d4,
                0x476d275780b553fe,
                0xc3f2296317f71051,
                0xb1d2bb5373270c43,
                0x0e18a3597be61302,
            ]),
            Fp::from_raw([
                0x7fbbc6b3e494ca68,
                0x2afcc7335152430b,
                0x93d5bd3acbccf3b3,
                0x61a76bb383622b8c,
                0x93efc4d40d7fac4d,
                0x0a791ad7698655a7,
                0x22b10d5c1090eec8,
            ]),
            Fp::from_raw([
                0x596eec60211ad67b,
                0xf23f57b9f9db8c07,
                0x33e66f105ffc5e45,
                0xb10ef45226f3ae42,
                0xb98a559ccfc0ba32,
                0x819ba919d0b6e9b5,
                0x20f73876330a90e8,
            ]),
            Fp::from_raw([
                0xbade57a48e2d9868,
                0xe61829ffe983fcfc,
                0xd0d080b774c31996,
                0xa1d712ef206b4a2f,
                0x7957f20173071cf6,
                0xf850f49359458652,
                0x17ba9f9aa08b9ee2,
            ]),
            Fp::from_raw([
                0xd0239c8282ccc372,
                0xfa20a695ee8f6288,
                0x269f2ef315e029a5,
                0xcc915da35e10b4e6,
                0x8406f6977aadce0f,
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
            let p = Fp::from_uniform_bytes(&uniform_bytes[i]);
            assert_eq!(expected_results[i], p);
        }
    }

    #[test]
    #[cfg(feature = "bits")]
    fn test_bits() {
        crate::tests::field::random_bits_tests::<Fp>("Fp".to_string());
    }

    #[test]
    fn test_serialization() {
        crate::tests::field::random_serialization_test::<Fp>("Fp".to_string());
        #[cfg(feature = "derive_serde")]
        crate::tests::field::random_serde_test::<Fp>("Fp".to_string());
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
        let start = start_timer!(|| "serialize Fp");
        // failure check
        for _ in 0..1000000 {
            let rand_word = [(); 7].map(|_| rng.next_u64());
            let a = Fp(rand_word);
            let rand_bytes = a.to_raw_bytes();
            match is_less_than(&rand_word, &MODULUS.0) {
                false => {
                    assert!(Fp::from_raw_bytes(&rand_bytes).is_none());
                }
                _ => {
                    assert_eq!(Fp::from_raw_bytes(&rand_bytes), Some(a));
                }
            }
        }
        end_timer!(start);
    }
}
