//! The Pallas base field (Vesta scalar field) `Fp`.
#![allow(clippy::unreadable_literal)]

use super::cios::Modulus;
use super::safegcd::ModInfo;

/// An element of `F_p` with
/// `p = 0x40000000000000000000000000000000224698fc094cf91b992d30ed00000001`.
///
/// `Fp` is the base field of Pallas and the scalar field of Vesta. Values are
/// stored in Montgomery form and always fully reduced. The canonical encoding
/// is 32 little-endian bytes, identical to `pasta_curves::Fp`.
///
/// The Montgomery limbs are private: every value is constructed through a
/// reducing or checking constructor (`from_raw`, `from_u512`, `From<u64>`,
/// `PrimeField::from_repr`, `PastaField::from_canonical_limbs`), so the
/// arithmetic can rely on its operands being fully reduced. Outside the crate
/// a value cannot be built from raw limbs:
///
/// ```compile_fail
/// let unreduced = iroha_pasta::Fp([u64::MAX; 4]);
/// ```
#[derive(Clone, Copy)]
pub struct Fp(pub(crate) [u64; 4]);

/// Montgomery parameters of `p`.
pub const MODULUS: Modulus = Modulus {
    m: [
        0x992d30ed00000001,
        0x224698fc094cf91b,
        0,
        0x4000000000000000,
    ],
    inv: 0x992d30ecffffffff,
    r2: [
        0x8c78ecb30000000f,
        0xd7d30dbd8b0de0e7,
        0x7797a99bc3c95d18,
        0x096d41af7b9cb714,
    ],
    r3: [
        0xf185a5993a9e10f9,
        0xf6a68f3b6ac5b1d1,
        0xdf8d1014353fd42c,
        0x2ae309222d2d9910,
    ],
};
const _: () = assert!(MODULUS.has_pasta_shape());

/// Safegcd data for variable-time inversion.
pub const MOD_INFO: ModInfo = ModInfo::new(&MODULUS.m);

const MODULUS_STR: &str = "0x40000000000000000000000000000000224698fc094cf91b992d30ed00000001";

/// `p - 2`, the Fermat inversion exponent.
const INV_EXP: [u64; 4] = [
    0x992d30ecffffffff,
    0x224698fc094cf91b,
    0,
    0x4000000000000000,
];

/// `(t - 1) / 2` where `p - 1 = t * 2^32`.
const T_MINUS1_OVER2: [u64; 4] = [
    0x04a67c8dcc969876,
    0x0000000011234c7e,
    0,
    0x0000000020000000,
];

/// `(p + 1) / 2`.
const TWO_INV_RAW: [u64; 4] = [
    0xcc96987680000001,
    0x11234c7e04a67c8d,
    0,
    0x2000000000000000,
];

/// `5^t`, a primitive `2^32`-th root of unity.
const ROOT_OF_UNITY_RAW: [u64; 4] = [
    0xbdad6fabd87ea32f,
    0xea322bf2b7bb7584,
    0x362120830561f81a,
    0x2bce74deac30ebda,
];

/// The inverse of [`ROOT_OF_UNITY_RAW`].
const ROOT_OF_UNITY_INV_RAW: [u64; 4] = [
    0xf0b87c7db2ce91f6,
    0x84a0a1d8859f066f,
    0xb4ed8e647196dad1,
    0x2cd5282c53116b5c,
];

/// `5^(2^32)`, a generator of the order-`t` subgroup.
const DELTA_RAW: [u64; 4] = [
    0x6a6ccd20dd7b9ba2,
    0xf5e4f3f13eee5636,
    0xbd455b7112a5049d,
    0x0a757d0f0006ab6c,
];

/// A primitive cube root of unity; the Pallas endomorphism scales `x` by it.
const ZETA_RAW: [u64; 4] = [
    0x1dad5ebdfdfe4ab9,
    0x1d1f8bd237ad3149,
    0x2caad5dc57aab1b0,
    0x12ccca834acdba71,
];

/// Perfect-hash parameters `(xor, modulus)` of the square-root tables.
const SQRT_HASH: (u32, usize) = (0x11BE, 1098);

super::impl_pasta_field!(Fp);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn montgomery_constants_are_consistent() {
        // R2 = R * R in Montgomery form, R3 = R2 * R2 / R.
        let r = Fp::one();
        assert_eq!(Fp::from_raw([1, 0, 0, 0]), r);
        assert_eq!(Fp(MODULUS.r2), Fp::from_u512([0, 0, 0, 0, 1, 0, 0, 0]));
        assert_eq!(Fp(MODULUS.r3), Fp(MODULUS.r2) * Fp(MODULUS.r2));
        assert_eq!(MODULUS.m[0].wrapping_mul(MODULUS.inv), u64::MAX);
    }

    #[test]
    fn named_constants() {
        use ff::{Field, PrimeField, WithSmallOrderMulGroup};
        assert_eq!(Fp::TWO_INV.double(), Fp::ONE);
        assert_eq!(Fp::ROOT_OF_UNITY * Fp::ROOT_OF_UNITY_INV, Fp::ONE);
        assert_eq!(Fp::ROOT_OF_UNITY.pow_vartime([1u64 << 32]), Fp::ONE);
        assert_ne!(Fp::ROOT_OF_UNITY.pow_vartime([1u64 << 31]), Fp::ONE);
        assert_eq!(
            Fp::DELTA,
            Fp::MULTIPLICATIVE_GENERATOR.pow_vartime([1u64 << 32])
        );
        assert_ne!(Fp::ZETA, Fp::ONE);
        assert_eq!(Fp::ZETA.cube(), Fp::ONE);
        assert_eq!(
            format!("{:?}", Fp::ZETA),
            "0x12ccca834acdba712caad5dc57aab1b01d1f8bd237ad31491dad5ebdfdfe4ab9"
        );
        assert_eq!(
            Fp::from(5u64).pow_vartime(INV_EXP) * Fp::from(5u64),
            Fp::ONE
        );
        assert_eq!(MODULUS_STR.len(), 66);
    }
}
