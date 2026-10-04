//! The Vesta base field (Pallas scalar field) `Fq`.
#![allow(clippy::unreadable_literal)]

use super::cios::Modulus;
use super::safegcd::ModInfo;

/// An element of `F_q` with
/// `q = 0x40000000000000000000000000000000224698fc0994a8dd8c46eb2100000001`.
///
/// `Fq` is the base field of Vesta and the scalar field of Pallas. Values are
/// stored in Montgomery form and always fully reduced. The canonical encoding
/// is 32 little-endian bytes, identical to `pasta_curves::Fq`.
///
/// The Montgomery limbs are private: every value is constructed through a
/// reducing or checking constructor (`from_raw`, `from_u512`, `From<u64>`,
/// `PrimeField::from_repr`, `PastaField::from_canonical_limbs`), so the
/// arithmetic can rely on its operands being fully reduced. Outside the crate
/// a value cannot be built from raw limbs:
///
/// ```compile_fail
/// let unreduced = iroha_pasta::Fq([u64::MAX; 4]);
/// ```
#[derive(Clone, Copy)]
pub struct Fq(pub(crate) [u64; 4]);

/// Montgomery parameters of `q`.
pub const MODULUS: Modulus = Modulus {
    m: [
        0x8c46eb2100000001,
        0x224698fc0994a8dd,
        0,
        0x4000000000000000,
    ],
    inv: 0x8c46eb20ffffffff,
    r2: [
        0xfc9678ff0000000f,
        0x67bb433d891a16e3,
        0x7fae231004ccf590,
        0x096d41af7ccfdaa9,
    ],
    r3: [
        0x008b421c249dae4c,
        0xe13bda50dba41326,
        0x88fececb8e15cb63,
        0x07dd97a06e6792c8,
    ],
};
const _: () = assert!(MODULUS.has_pasta_shape());

/// Safegcd data for variable-time inversion.
pub const MOD_INFO: ModInfo = ModInfo::new(&MODULUS.m);

const MODULUS_STR: &str = "0x40000000000000000000000000000000224698fc0994a8dd8c46eb2100000001";

/// `q - 2`, the Fermat inversion exponent.
const INV_EXP: [u64; 4] = [
    0x8c46eb20ffffffff,
    0x224698fc0994a8dd,
    0,
    0x4000000000000000,
];

/// `(t - 1) / 2` where `q - 1 = t * 2^32`.
const T_MINUS1_OVER2: [u64; 4] = [
    0x04ca546ec6237590,
    0x0000000011234c7e,
    0,
    0x0000000020000000,
];

/// `(q + 1) / 2`.
const TWO_INV_RAW: [u64; 4] = [
    0xc623759080000001,
    0x11234c7e04ca546e,
    0,
    0x2000000000000000,
];

/// `5^t`, a primitive `2^32`-th root of unity.
const ROOT_OF_UNITY_RAW: [u64; 4] = [
    0xa70e2c1102b6d05f,
    0x9bb97ea3c106f049,
    0x9e5c4dfd492ae26e,
    0x2de6a9b8746d3f58,
];

/// The inverse of [`ROOT_OF_UNITY_RAW`].
const ROOT_OF_UNITY_INV_RAW: [u64; 4] = [
    0x57eecda0a84b6836,
    0x4ad38b9084b8a80c,
    0xf4c8f353124086c1,
    0x2235e1a7415bf936,
];

/// `5^(2^32)`, a generator of the order-`t` subgroup.
const DELTA_RAW: [u64; 4] = [
    0x8494392472d1683c,
    0xe3ac3376541d1140,
    0x06f0a88e7f7949f8,
    0x2237d54423724166,
];

/// A primitive cube root of unity; the Vesta endomorphism scales `x` by it.
const ZETA_RAW: [u64; 4] = [
    0x2aa9d2e050aa0e4f,
    0x0fed467d47c033af,
    0x511db4d81cf70f5a,
    0x06819a58283e528e,
];

/// Perfect-hash parameters `(xor, modulus)` of the square-root tables.
const SQRT_HASH: (u32, usize) = (0x116A9E, 1206);

super::impl_pasta_field!(Fq);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn montgomery_constants_are_consistent() {
        // R2 = R * R in Montgomery form, R3 = R2 * R2 / R.
        let r = Fq::one();
        assert_eq!(Fq::from_raw([1, 0, 0, 0]), r);
        assert_eq!(Fq(MODULUS.r2), Fq::from_u512([0, 0, 0, 0, 1, 0, 0, 0]));
        assert_eq!(Fq(MODULUS.r3), Fq(MODULUS.r2) * Fq(MODULUS.r2));
        assert_eq!(MODULUS.m[0].wrapping_mul(MODULUS.inv), u64::MAX);
    }

    #[test]
    fn named_constants() {
        use ff::{Field, PrimeField, WithSmallOrderMulGroup};
        assert_eq!(Fq::TWO_INV.double(), Fq::ONE);
        assert_eq!(Fq::ROOT_OF_UNITY * Fq::ROOT_OF_UNITY_INV, Fq::ONE);
        assert_eq!(Fq::ROOT_OF_UNITY.pow_vartime([1u64 << 32]), Fq::ONE);
        assert_ne!(Fq::ROOT_OF_UNITY.pow_vartime([1u64 << 31]), Fq::ONE);
        assert_eq!(
            Fq::DELTA,
            Fq::MULTIPLICATIVE_GENERATOR.pow_vartime([1u64 << 32])
        );
        assert_ne!(Fq::ZETA, Fq::ONE);
        assert_eq!(Fq::ZETA.cube(), Fq::ONE);
        assert_eq!(
            format!("{:?}", Fq::ZETA),
            "0x06819a58283e528e511db4d81cf70f5a0fed467d47c033af2aa9d2e050aa0e4f"
        );
        assert_eq!(
            Fq::from(5u64).pow_vartime(INV_EXP) * Fq::from(5u64),
            Fq::ONE
        );
        assert_eq!(MODULUS_STR.len(), 66);
    }
}
