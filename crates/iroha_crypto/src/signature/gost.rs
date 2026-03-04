//! GOST R 34.10-2012 signatures backed by `RustCrypto`'s Streebog hash.
//!
//! The elliptic-curve arithmetic uses a constant-time backend layered on top of
//! `crypto-bigint` Montgomery field arithmetic with Jacobian point operations.
//! All field operations remain deterministic across platforms.

#[cfg(test)]
use core::ops::ShrAssign;
use core::{cmp::Ordering, fmt};
use std::sync::LazyLock;

use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Zero};
use rand::RngCore;
use streebog::{Digest, Streebog256, Streebog512};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

#[cfg(feature = "gost")]
mod constant_time {
    #![allow(dead_code)]
    //! Constant-time arithmetic for the TC26 curves.
    //!
    //! This module provides field operations backed by `crypto-bigint`’s constant-time
    //! Montgomery arithmetic and Jacobian point helpers that will replace the compat
    //! `num-bigint` implementation during Task G2.

    use std::{ptr, sync::LazyLock};

    use crypto_bigint::{
        Odd, U256, U512, Uint,
        modular::{MontyForm, MontyParams},
    };
    use num_bigint::BigUint;
    use num_traits::Zero;
    use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};

    use super::{AffinePoint as OuterAffinePoint, CurveParams as OuterCurveParams};
    #[cfg(test)]
    use crate::Algorithm;

    /// Field element represented in Montgomery form with constant-time operations.
    #[derive(Clone, Copy)]
    struct FieldElement<const LIMBS: usize> {
        residue: MontyForm<LIMBS>,
    }

    impl<const LIMBS: usize> FieldElement<LIMBS> {
        fn zero(params: MontyParams<LIMBS>) -> Self {
            Self {
                residue: MontyForm::zero(params),
            }
        }

        fn one(params: MontyParams<LIMBS>) -> Self {
            Self {
                residue: MontyForm::one(params),
            }
        }

        fn from_uint(value: Uint<LIMBS>, params: MontyParams<LIMBS>) -> Self {
            Self {
                residue: MontyForm::new(&value, params),
            }
        }

        fn as_uint(&self) -> Uint<LIMBS> {
            self.residue.retrieve()
        }

        fn add(&self, rhs: &Self) -> Self {
            Self {
                residue: self.residue.add(&rhs.residue),
            }
        }

        fn sub(&self, rhs: &Self) -> Self {
            Self {
                residue: self.residue.sub(&rhs.residue),
            }
        }

        fn mul(&self, rhs: &Self) -> Self {
            Self {
                residue: self.residue.mul(&rhs.residue),
            }
        }

        fn square(&self) -> Self {
            Self {
                residue: self.residue.square(),
            }
        }

        fn double(&self) -> Self {
            self.add(self)
        }

        fn triple(&self) -> Self {
            self.double().add(self)
        }

        fn negate(&self) -> Self {
            Self {
                residue: self.residue.neg(),
            }
        }

        fn is_zero(&self) -> Choice {
            self.residue.retrieve().ct_eq(&Uint::<LIMBS>::ZERO)
        }

        fn invert(&self) -> Option<Self> {
            if bool::from(self.is_zero()) {
                return None;
            }
            let exponent = self
                .residue
                .params()
                .modulus()
                .as_ref()
                .wrapping_sub(&Uint::<LIMBS>::from_u64(2));
            Some(Self {
                residue: self.residue.pow(&exponent),
            })
        }

        fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
            Self {
                residue: MontyForm::conditional_select(&a.residue, &b.residue, choice),
            }
        }
    }

    impl<const LIMBS: usize> ConstantTimeEq for FieldElement<LIMBS> {
        fn ct_eq(&self, other: &Self) -> Choice {
            self.residue.retrieve().ct_eq(&other.residue.retrieve())
        }
    }

    #[derive(Clone, Copy)]
    struct AffinePoint<const LIMBS: usize> {
        x: FieldElement<LIMBS>,
        y: FieldElement<LIMBS>,
    }

    #[derive(Clone, Copy)]
    struct JacobianPoint<const LIMBS: usize> {
        x: FieldElement<LIMBS>,
        y: FieldElement<LIMBS>,
        z: FieldElement<LIMBS>,
    }

    impl<const LIMBS: usize> JacobianPoint<LIMBS> {
        fn infinity(params: MontyParams<LIMBS>) -> Self {
            Self {
                x: FieldElement::zero(params),
                y: FieldElement::one(params),
                z: FieldElement::zero(params),
            }
        }

        fn from_affine(point: &AffinePoint<LIMBS>, params: MontyParams<LIMBS>) -> Self {
            Self {
                x: point.x,
                y: point.y,
                z: FieldElement::one(params),
            }
        }

        fn is_infinity(&self) -> Choice {
            self.z.is_zero()
        }

        fn double(&self, curve: &CurveParameters<LIMBS>) -> Self {
            let params = curve.field_params;

            let is_inf = self.is_infinity();
            let mut result = Self::infinity(params);

            if bool::from(is_inf) {
                return result;
            }

            let xx = self.x.square();
            let yy = self.y.square();
            let yyyy = yy.square();
            let zz = self.z.square();
            let zz2 = zz.square();

            let s = self.x.mul(&yy).double().double(); // 4 * X * Y^2
            let m = xx.triple().add(&curve.a.mul(&zz2));
            let x3 = m.square().sub(&s.double());

            let s_minus_x3 = s.sub(&x3);
            let y3 = m.mul(&s_minus_x3).sub(&yyyy.double().double().double());
            let z3 = self.y.mul(&self.z).double();

            result.x = x3;
            result.y = y3;
            result.z = z3;
            result
        }

        fn add(&self, other: &Self, curve: &CurveParameters<LIMBS>) -> Self {
            let params = curve.field_params;
            let inf_self = self.is_infinity();
            let inf_other = other.is_infinity();

            let z1z1 = self.z.square();
            let z2z2 = other.z.square();
            let u1 = self.x.mul(&z2z2);
            let u2 = other.x.mul(&z1z1);

            let z1_cubed = self.z.mul(&z1z1);
            let z2_cubed = other.z.mul(&z2z2);
            let s1 = self.y.mul(&z2_cubed);
            let s2 = other.y.mul(&z1_cubed);

            let delta_x = u2.sub(&u1);
            let double_delta_y = s2.sub(&s1).double();

            let delta_x_is_zero = delta_x.is_zero();
            let double_delta_y_is_zero = double_delta_y.is_zero();

            let doubled_delta_x = delta_x.double();
            let delta_x_double_squared = doubled_delta_x.square();
            let delta_x_cubed = delta_x.mul(&delta_x_double_squared);
            let u1_scaled = u1.mul(&delta_x_double_squared);

            let x3_generic = double_delta_y
                .square()
                .sub(&delta_x_cubed)
                .sub(&u1_scaled.double());
            let y3_generic = double_delta_y
                .mul(&u1_scaled.sub(&x3_generic))
                .sub(&s1.mul(&delta_x_cubed).double());
            let z3_generic = (self.z.add(&other.z))
                .square()
                .sub(&z1z1)
                .sub(&z2z2)
                .mul(&delta_x);

            let generic = Self {
                x: x3_generic,
                y: y3_generic,
                z: z3_generic,
            };

            let infinity = Self::infinity(params);
            let doubled = self.double(curve);

            // H == 0 && R == 0  => points are equal (use doubling)
            let select_double = delta_x_is_zero & double_delta_y_is_zero;
            // H == 0 && R != 0 => result is infinity
            let select_infinity = delta_x_is_zero & (!double_delta_y_is_zero);

            let mut result = Self::conditional_select(&generic, &infinity, select_infinity);
            result = Self::conditional_select(&result, &doubled, select_double);
            result = Self::conditional_select(&result, other, inf_self);
            result = Self::conditional_select(&result, self, inf_other);
            result
        }

        fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
            Self {
                x: FieldElement::conditional_select(&a.x, &b.x, choice),
                y: FieldElement::conditional_select(&a.y, &b.y, choice),
                z: FieldElement::conditional_select(&a.z, &b.z, choice),
            }
        }

        fn as_affine(&self) -> Option<AffinePoint<LIMBS>> {
            if bool::from(self.is_infinity()) {
                return None;
            }
            let z_inv = self.z.invert()?;
            let z_inv2 = z_inv.square();
            let z_inv3 = z_inv2.mul(&z_inv);

            let x = self.x.mul(&z_inv2);
            let y = self.y.mul(&z_inv3);
            Some(AffinePoint { x, y })
        }
    }

    struct CurveParameters<const LIMBS: usize> {
        field_params: MontyParams<LIMBS>,
        a: FieldElement<LIMBS>,
        b: FieldElement<LIMBS>,
        generator: AffinePoint<LIMBS>,
        scalar_modulus: Uint<LIMBS>,
    }

    impl<const LIMBS: usize> CurveParameters<LIMBS> {
        fn generator(&self) -> JacobianPoint<LIMBS> {
            JacobianPoint::from_affine(&self.generator, self.field_params)
        }

        fn scalar_modulus(&self) -> Uint<LIMBS> {
            self.scalar_modulus
        }

        fn field_params(&self) -> MontyParams<LIMBS> {
            self.field_params
        }
    }

    fn params_from_hex<const LIMBS: usize>(hex: &str) -> MontyParams<LIMBS> {
        let modulus = Odd::new(Uint::<LIMBS>::from_be_hex(hex)).expect("curve modulus must be odd");
        MontyParams::new_vartime(modulus)
    }

    fn fe_from_hex<const LIMBS: usize>(
        hex: &str,
        params: MontyParams<LIMBS>,
    ) -> FieldElement<LIMBS> {
        FieldElement::from_uint(Uint::<LIMBS>::from_be_hex(hex), params)
    }

    fn curve_from_constants_256(
        p_hex: &str,
        q_hex: &str,
        a_hex: &str,
        b_hex: &str,
        generator_hex: (&str, &str),
    ) -> CurveParameters<{ U256::LIMBS }> {
        let field_params = params_from_hex::<{ U256::LIMBS }>(p_hex);
        let scalar_modulus = U256::from_be_hex(q_hex);
        CurveParameters {
            field_params,
            a: fe_from_hex(a_hex, field_params),
            b: fe_from_hex(b_hex, field_params),
            generator: AffinePoint {
                x: fe_from_hex(generator_hex.0, field_params),
                y: fe_from_hex(generator_hex.1, field_params),
            },
            scalar_modulus,
        }
    }

    fn curve_from_constants_512(
        p_hex: &str,
        q_hex: &str,
        a_hex: &str,
        b_hex: &str,
        generator_hex: (&str, &str),
    ) -> CurveParameters<{ U512::LIMBS }> {
        let field_params = params_from_hex::<{ U512::LIMBS }>(p_hex);
        let scalar_modulus = U512::from_be_hex(q_hex);
        CurveParameters {
            field_params,
            a: fe_from_hex(a_hex, field_params),
            b: fe_from_hex(b_hex, field_params),
            generator: AffinePoint {
                x: fe_from_hex(generator_hex.0, field_params),
                y: fe_from_hex(generator_hex.1, field_params),
            },
            scalar_modulus,
        }
    }

    static CURVE_256_A: LazyLock<CurveParameters<{ U256::LIMBS }>> = LazyLock::new(|| {
        curve_from_constants_256(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD97",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE6C611070995AD10045841B09B761B893",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFD94",
            "00000000000000000000000000000000000000000000000000000000000000A6",
            (
                "0000000000000000000000000000000000000000000000000000000000000001",
                "8D91E471E0989CDA27DF505A453F2B7635294F2DDF23E3B122ACC99C9E9F1E14",
            ),
        )
    });

    static CURVE_256_B: LazyLock<CurveParameters<{ U256::LIMBS }>> = LazyLock::new(|| {
        curve_from_constants_256(
            "8000000000000000000000000000000000000000000000000000000000000C99",
            "800000000000000000000000000000015F700CFFF1A624E5E497161BCC8A198F",
            "8000000000000000000000000000000000000000000000000000000000000C96",
            "3E1AF419A269A5F866A7D3C25C3DF80AE979259373FF2B182F49D4CE7E1BBC8B",
            (
                "0000000000000000000000000000000000000000000000000000000000000001",
                "3FA8124359F96680B83D1C3EB2C070E5C545C9858D03ECFB744BF8D717717EFC",
            ),
        )
    });

    static CURVE_256_C: LazyLock<CurveParameters<{ U256::LIMBS }>> = LazyLock::new(|| {
        curve_from_constants_256(
            "9B9F605F5A858107AB1EC85E6B41C8AACF846E86789051D37998F7B9022D759B",
            "9B9F605F5A858107AB1EC85E6B41C8AA582CA3511EDDFB74F02F3A6598980BB9",
            "9B9F605F5A858107AB1EC85E6B41C8AACF846E86789051D37998F7B9022D7598",
            "000000000000000000000000000000000000000000000000000000000000805A",
            (
                "0000000000000000000000000000000000000000000000000000000000000000",
                "41ECE55743711A8C3CBF3783CD08C0EE4D4DC440D4641A8F366E550DFDB3BB67",
            ),
        )
    });

    static CURVE_512_A: LazyLock<CurveParameters<{ U512::LIMBS }>> = LazyLock::new(|| {
        curve_from_constants_512(
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFDC7",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE27E69532F48D89116FF22B8D4E0560609B4B38ABFAD2B85DCACDB1411F10B275",
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFDC4",
            "E8C2505DEDFC86DDC1BD0B2B6667F1DA34B82574761CB0E879BD081CFD0B6265EE3CB090F30D27614CB4574010DA90DD862EF9D4EBEE4761503190785A71C760",
            (
                "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003",
                "7503CFE87A836AE3A61B8816E25450E6CE5E1C93ACF1ABC1778064FDCBEFA921DF1626BE4FD036E93D75E6A50E3A41E98028FE5FC235F5B889A589CB5215F2A4",
            ),
        )
    });

    static CURVE_512_B: LazyLock<CurveParameters<{ U512::LIMBS }>> = LazyLock::new(|| {
        curve_from_constants_512(
            "8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006F",
            "800000000000000000000000000000000000000000000000000000000000000149A1EC142565A545ACFDB77BD9D40CFA8B996712101BEA0EC6346C54374F25BD",
            "8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006C",
            "687D1B459DC841457E3E06CF6F5E2517B97C7D614AF138BCBF85DC806C4B289F3E965D2DB1416D217F8B276FAD1AB69C50F78BEE1FA3106EFB8CCBC7C5140116",
            (
                "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002",
                "1A8F7EDA389B094C2C071E3647A8940F3C123B697578C213BE6DD9E6C8EC7335DCB228FD1EDF4A39152CBCAAF8C0398828041055F94CEEEC7E21340780FE41BD",
            ),
        )
    });

    enum CurveSelection {
        Bits256(&'static CurveParameters<{ U256::LIMBS }>),
        Bits512(&'static CurveParameters<{ U512::LIMBS }>),
    }

    #[cfg(test)]
    fn curve_for_algorithm(algo: Algorithm) -> Option<CurveSelection> {
        match algo {
            Algorithm::Gost3410_2012_256ParamSetA => Some(CurveSelection::Bits256(&CURVE_256_A)),
            Algorithm::Gost3410_2012_256ParamSetB => Some(CurveSelection::Bits256(&CURVE_256_B)),
            Algorithm::Gost3410_2012_256ParamSetC => Some(CurveSelection::Bits256(&CURVE_256_C)),
            Algorithm::Gost3410_2012_512ParamSetA => Some(CurveSelection::Bits512(&CURVE_512_A)),
            Algorithm::Gost3410_2012_512ParamSetB => Some(CurveSelection::Bits512(&CURVE_512_B)),
            _ => None,
        }
    }

    fn curve_for_params(params: &OuterCurveParams) -> Option<CurveSelection> {
        if ptr::eq(
            ptr::from_ref(params),
            ptr::from_ref(LazyLock::force(&super::PARAM_256_A)),
        ) {
            return Some(CurveSelection::Bits256(&CURVE_256_A));
        }
        if ptr::eq(
            ptr::from_ref(params),
            ptr::from_ref(LazyLock::force(&super::PARAM_256_B)),
        ) {
            return Some(CurveSelection::Bits256(&CURVE_256_B));
        }
        if ptr::eq(
            ptr::from_ref(params),
            ptr::from_ref(LazyLock::force(&super::PARAM_256_C)),
        ) {
            return Some(CurveSelection::Bits256(&CURVE_256_C));
        }
        if ptr::eq(
            ptr::from_ref(params),
            ptr::from_ref(LazyLock::force(&super::PARAM_512_A)),
        ) {
            return Some(CurveSelection::Bits512(&CURVE_512_A));
        }
        if ptr::eq(
            ptr::from_ref(params),
            ptr::from_ref(LazyLock::force(&super::PARAM_512_B)),
        ) {
            return Some(CurveSelection::Bits512(&CURVE_512_B));
        }
        None
    }

    fn biguint_to_uint<const LIMBS: usize>(value: &BigUint) -> Option<Uint<LIMBS>> {
        let bytes = value.to_bytes_be();
        if bytes.len() > Uint::<LIMBS>::BYTES {
            return None;
        }
        let mut padded = vec![0u8; Uint::<LIMBS>::BYTES];
        let offset = padded.len() - bytes.len();
        padded[offset..].copy_from_slice(&bytes);
        Some(Uint::<LIMBS>::from_be_slice(&padded))
    }

    fn uint_to_biguint<const LIMBS: usize>(value: &Uint<LIMBS>) -> BigUint {
        let mut bytes = Vec::with_capacity(Uint::<LIMBS>::BYTES);
        for word in value.to_words() {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        BigUint::from_bytes_le(&bytes)
    }

    fn generator_outer_point<const LIMBS: usize>(
        curve: &CurveParameters<LIMBS>,
    ) -> OuterAffinePoint {
        let generator_affine = curve
            .generator()
            .as_affine()
            .expect("generator must not be at infinity");
        OuterAffinePoint::new(
            uint_to_biguint(&generator_affine.x.as_uint()),
            uint_to_biguint(&generator_affine.y.as_uint()),
        )
    }

    fn affine_from_outer<const LIMBS: usize>(
        curve: &CurveParameters<LIMBS>,
        point: &OuterAffinePoint,
    ) -> Option<AffinePoint<LIMBS>> {
        let params = curve.field_params();
        let x = FieldElement::from_uint(biguint_to_uint::<LIMBS>(&point.x)?, params);
        let y = FieldElement::from_uint(biguint_to_uint::<LIMBS>(&point.y)?, params);
        Some(AffinePoint { x, y })
    }

    fn scalar_mul_impl<const LIMBS: usize>(
        curve: &CurveParameters<LIMBS>,
        scalar: &BigUint,
        point: &OuterAffinePoint,
    ) -> Option<OuterAffinePoint> {
        if scalar.is_zero() {
            return None;
        }
        let scalar_uint = biguint_to_uint::<LIMBS>(scalar)?;
        if bool::from(scalar_uint.ct_eq(&Uint::<LIMBS>::ZERO)) {
            return None;
        }

        let params = curve.field_params();
        let base_affine = affine_from_outer(curve, point)?;
        let base = JacobianPoint::from_affine(&base_affine, params);
        let mut acc = JacobianPoint::infinity(params);

        for i in (0..Uint::<LIMBS>::BITS).rev() {
            let doubled = acc.double(curve);
            let added = doubled.add(&base, curve);
            let choice = scalar_uint.bit(i);
            acc = JacobianPoint::conditional_select(&doubled, &added, choice.into());
        }

        let affine = acc.as_affine()?;
        Some(OuterAffinePoint::new(
            uint_to_biguint(&affine.x.as_uint()),
            uint_to_biguint(&affine.y.as_uint()),
        ))
    }

    fn point_add_impl<const LIMBS: usize>(
        curve: &CurveParameters<LIMBS>,
        p: &OuterAffinePoint,
        q: &OuterAffinePoint,
    ) -> Option<OuterAffinePoint> {
        let params = curve.field_params();
        let p_jacobian = JacobianPoint::from_affine(&affine_from_outer(curve, p)?, params);
        let q_jacobian = JacobianPoint::from_affine(&affine_from_outer(curve, q)?, params);
        let sum = p_jacobian.add(&q_jacobian, curve);
        let affine = sum.as_affine()?;
        Some(OuterAffinePoint::new(
            uint_to_biguint(&affine.x.as_uint()),
            uint_to_biguint(&affine.y.as_uint()),
        ))
    }

    pub(super) fn scalar_mul(
        params: &OuterCurveParams,
        scalar: &BigUint,
        point: &OuterAffinePoint,
    ) -> Option<OuterAffinePoint> {
        curve_for_params(params).and_then(|selection| match selection {
            CurveSelection::Bits256(curve) => scalar_mul_impl(curve, scalar, point),
            CurveSelection::Bits512(curve) => scalar_mul_impl(curve, scalar, point),
        })
    }

    fn scalar_mul_base_impl<const LIMBS: usize>(
        curve: &CurveParameters<LIMBS>,
        scalar: &BigUint,
    ) -> Option<OuterAffinePoint> {
        if scalar.is_zero() {
            return None;
        }
        let generator = generator_outer_point(curve);
        scalar_mul_impl(curve, scalar, &generator)
    }

    pub(super) fn scalar_mul_base(
        params: &OuterCurveParams,
        scalar: &BigUint,
    ) -> Option<OuterAffinePoint> {
        curve_for_params(params).and_then(|selection| match selection {
            CurveSelection::Bits256(curve) => scalar_mul_base_impl(curve, scalar),
            CurveSelection::Bits512(curve) => scalar_mul_base_impl(curve, scalar),
        })
    }

    fn mul_add_impl<const LIMBS: usize>(
        curve: &CurveParameters<LIMBS>,
        scalar_g: &BigUint,
        scalar_q: &BigUint,
        point_q: &OuterAffinePoint,
    ) -> Option<OuterAffinePoint> {
        let generator_scalar_uint = biguint_to_uint::<LIMBS>(scalar_g)?;
        let point_scalar_uint = biguint_to_uint::<LIMBS>(scalar_q)?;
        if bool::from(
            generator_scalar_uint.ct_eq(&Uint::<LIMBS>::ZERO)
                & point_scalar_uint.ct_eq(&Uint::<LIMBS>::ZERO),
        ) {
            return None;
        }

        let params = curve.field_params();
        let base = curve.generator();
        let affine_q = affine_from_outer(curve, point_q)?;
        let point = JacobianPoint::from_affine(&affine_q, params);
        let mut table = [
            JacobianPoint::infinity(params),
            JacobianPoint::infinity(params),
            JacobianPoint::infinity(params),
            JacobianPoint::infinity(params),
        ];
        table[1] = base;
        table[2] = point;
        table[3] = table[1].add(&table[2], curve);

        let mut acc = JacobianPoint::infinity(params);
        for bit_index in (0..Uint::<LIMBS>::BITS).rev() {
            acc = acc.double(curve);

            let bit_g = Choice::from(generator_scalar_uint.bit(bit_index));
            let bit_q = Choice::from(point_scalar_uint.bit(bit_index));
            let both = bit_g & bit_q;

            let mut addend = table[0];
            addend = JacobianPoint::conditional_select(&addend, &table[1], bit_g);
            addend = JacobianPoint::conditional_select(&addend, &table[2], bit_q);
            addend = JacobianPoint::conditional_select(&addend, &table[3], both);

            acc = acc.add(&addend, curve);
        }

        let affine = acc.as_affine()?;
        Some(OuterAffinePoint::new(
            uint_to_biguint(&affine.x.as_uint()),
            uint_to_biguint(&affine.y.as_uint()),
        ))
    }

    pub(super) fn mul_add(
        params: &OuterCurveParams,
        scalar_g: &BigUint,
        scalar_q: &BigUint,
        point_q: &OuterAffinePoint,
    ) -> Option<OuterAffinePoint> {
        curve_for_params(params).and_then(|selection| match selection {
            CurveSelection::Bits256(curve) => mul_add_impl(curve, scalar_g, scalar_q, point_q),
            CurveSelection::Bits512(curve) => mul_add_impl(curve, scalar_g, scalar_q, point_q),
        })
    }

    pub(super) fn point_add(
        params: &OuterCurveParams,
        p: &OuterAffinePoint,
        q: &OuterAffinePoint,
    ) -> Option<OuterAffinePoint> {
        curve_for_params(params).and_then(|selection| match selection {
            CurveSelection::Bits256(curve) => point_add_impl(curve, p, q),
            CurveSelection::Bits512(curve) => point_add_impl(curve, p, q),
        })
    }

    #[cfg(test)]
    mod tests {
        use num_bigint::BigUint;
        use num_traits::{One, Zero};
        use rand_core::RngCore;

        use super::*;
        use crate::{
            rng::rng_from_seed,
            signature::gost::{Params, compat_point_add, compat_scalar_mul, params_for_algorithm},
        };

        #[test]
        fn generator_is_on_curve_all_params() {
            for algo in [
                Algorithm::Gost3410_2012_256ParamSetA,
                Algorithm::Gost3410_2012_256ParamSetB,
                Algorithm::Gost3410_2012_256ParamSetC,
                Algorithm::Gost3410_2012_512ParamSetA,
                Algorithm::Gost3410_2012_512ParamSetB,
            ] {
                match curve_for_algorithm(algo).unwrap() {
                    CurveSelection::Bits256(curve) => {
                        let generator_point = curve.generator();
                        let affine = generator_point
                            .as_affine()
                            .expect("generator not at infinity");
                        let compat_params = match params_for_algorithm(algo).unwrap() {
                            Params::Bits256(p) => p,
                            _ => unreachable!(),
                        };
                        let x = BigUint::from_bytes_le(&affine.x.as_uint().to_le_bytes());
                        let y = BigUint::from_bytes_le(&affine.y.as_uint().to_le_bytes());
                        let compat = compat_params.generator();
                        assert_eq!(x, compat.x);
                        assert_eq!(y, compat.y);
                    }
                    CurveSelection::Bits512(curve) => {
                        let generator_point = curve.generator();
                        let affine = generator_point
                            .as_affine()
                            .expect("generator not at infinity");
                        let compat_params = match params_for_algorithm(algo).unwrap() {
                            Params::Bits512(p) => p,
                            _ => unreachable!(),
                        };
                        let x = BigUint::from_bytes_le(&affine.x.as_uint().to_le_bytes());
                        let y = BigUint::from_bytes_le(&affine.y.as_uint().to_le_bytes());
                        let compat = compat_params.generator();
                        assert_eq!(x, compat.x);
                        assert_eq!(y, compat.y);
                    }
                }
            }
        }

        #[test]
        fn jacobian_double_matches_compat_add() {
            let curve = &*CURVE_256_A;
            let generator_point = curve.generator();
            let doubled = generator_point.double(curve);
            let affine = doubled.as_affine().expect("affine conversion");

            let compat_params =
                match params_for_algorithm(Algorithm::Gost3410_2012_256ParamSetA).unwrap() {
                    Params::Bits256(p) => p,
                    _ => unreachable!(),
                };
            let compat_gen = compat_params.generator();
            let compat_double =
                compat_point_add(compat_params, &compat_gen, &compat_gen).expect("compat double");
            let x = BigUint::from_bytes_le(&affine.x.as_uint().to_le_bytes());
            let y = BigUint::from_bytes_le(&affine.y.as_uint().to_le_bytes());
            assert_eq!(x, compat_double.x);
            assert_eq!(y, compat_double.y);
        }

        #[test]
        fn scalar_mul_matches_compat() {
            let curve = &*CURVE_256_B;
            let params = match params_for_algorithm(Algorithm::Gost3410_2012_256ParamSetB).unwrap()
            {
                Params::Bits256(p) => p,
                _ => unreachable!(),
            };

            let mut rng = rng_from_seed(b"ct-scalar-test".to_vec());
            let mut scalar_bytes = vec![0u8; params.scalar_len];
            rng.fill_bytes(&mut scalar_bytes);
            let modulus_bytes = curve.scalar_modulus().to_le_bytes();
            let modulus_big = BigUint::from_bytes_le(&modulus_bytes);
            let mut scalar_big = BigUint::from_bytes_le(&scalar_bytes);
            scalar_big %= &modulus_big;
            if scalar_big.is_zero() {
                scalar_big = BigUint::one();
            }
            let mut reduced_bytes = scalar_big.to_bytes_le();
            reduced_bytes.resize(U256::BYTES, 0);
            let scalar = U256::from_le_slice(&reduced_bytes);

            let mut acc = JacobianPoint::infinity(curve.field_params());
            let base = curve.generator();
            for i in (0..U256::BITS).rev() {
                acc = acc.double(curve);
                if bool::from(scalar.bit(i)) {
                    acc = acc.add(&base, curve);
                }
            }
            let affine = acc.as_affine().expect("affine conversion");
            let compat_point = compat_scalar_mul(params, &scalar_big, &params.generator()).unwrap();
            let x = BigUint::from_bytes_le(&affine.x.as_uint().to_le_bytes());
            let y = BigUint::from_bytes_le(&affine.y.as_uint().to_le_bytes());

            assert_eq!(x, compat_point.x);
            assert_eq!(y, compat_point.y);
        }
    }
}

trait DeterministicNonceGenerator {
    fn generate(
        &mut self,
        params: &CurveParams,
        private_scalar: &BigUint,
        message: &[u8],
        extra_entropy: Option<&[u8]>,
    ) -> BigUint;
}

const NONCE_DOMAIN_TAG: &[u8] = b"iroha:gost:nonce:v1";

struct StreebogNonceGenerator {
    domain_tag: &'static [u8],
}

impl StreebogNonceGenerator {
    fn new() -> Self {
        Self {
            domain_tag: NONCE_DOMAIN_TAG,
        }
    }

    #[cfg(test)]
    fn with_domain(domain_tag: &'static [u8]) -> Self {
        Self { domain_tag }
    }
}

impl Default for StreebogNonceGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl DeterministicNonceGenerator for StreebogNonceGenerator {
    fn generate(
        &mut self,
        params: &CurveParams,
        private_scalar: &BigUint,
        message: &[u8],
        extra_entropy: Option<&[u8]>,
    ) -> BigUint {
        use num_traits::Zero as _;

        const BLOCK_LEN: usize = 64;

        let hash_len = params.digest_len;
        let mut k = vec![0_u8; hash_len];
        let mut v = vec![0x01_u8; hash_len];

        let mut seed = Vec::with_capacity(
            self.domain_tag.len() + params.scalar_len * 2 + extra_entropy.map_or(0, <[u8]>::len),
        );
        seed.extend_from_slice(self.domain_tag);
        seed.extend_from_slice(&int_to_octets(private_scalar, params.scalar_len));
        seed.extend_from_slice(&bits_to_octets(params, message));
        if let Some(extra) = extra_entropy {
            seed.extend_from_slice(extra);
        }

        k = hmac_streebog(hash_len, BLOCK_LEN, &k, &[&v, &[0x00], &seed]);
        v = hmac_streebog(hash_len, BLOCK_LEN, &k, &[&v]);
        k = hmac_streebog(hash_len, BLOCK_LEN, &k, &[&v, &[0x01], &seed]);
        v = hmac_streebog(hash_len, BLOCK_LEN, &k, &[&v]);

        loop {
            let mut t = Vec::with_capacity(params.scalar_len);
            while t.len() < params.scalar_len {
                v = hmac_streebog(hash_len, BLOCK_LEN, &k, &[&v]);
                t.extend_from_slice(&v);
            }
            let candidate = BigUint::from_bytes_be(&t[..params.scalar_len]);
            let nonce = candidate % &params.q;
            if !nonce.is_zero() {
                return nonce;
            }
            k = hmac_streebog(hash_len, BLOCK_LEN, &k, &[&v, &[0x00]]);
            v = hmac_streebog(hash_len, BLOCK_LEN, &k, &[&v]);
        }
    }
}

fn int_to_octets(value: &BigUint, length: usize) -> Vec<u8> {
    let mut bytes = value.to_bytes_be();
    match bytes.len().cmp(&length) {
        Ordering::Greater => {
            bytes = bytes[bytes.len() - length..].to_vec();
        }
        Ordering::Less => {
            let mut padded = vec![0_u8; length - bytes.len()];
            padded.extend_from_slice(&bytes);
            bytes = padded;
        }
        Ordering::Equal => {}
    }
    bytes
}

fn bits_to_octets(params: &CurveParams, message: &[u8]) -> Vec<u8> {
    let scalar = hash_to_scalar(params, message);
    int_to_octets(&scalar, params.scalar_len)
}

fn hmac_streebog(digest_len: usize, block_len: usize, key: &[u8], data: &[&[u8]]) -> Vec<u8> {
    let mut key_block = vec![0_u8; block_len];
    if key.len() > block_len {
        let hashed = streebog_hash(digest_len, &[key]);
        key_block[..hashed.len()].copy_from_slice(&hashed);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut inner_pad = vec![0x36_u8; block_len];
    let mut outer_pad = vec![0x5c_u8; block_len];
    for i in 0..block_len {
        inner_pad[i] ^= key_block[i];
        outer_pad[i] ^= key_block[i];
    }

    let mut inner_segments = Vec::with_capacity(data.len() + 1);
    inner_segments.push(inner_pad.as_slice());
    inner_segments.extend_from_slice(data);

    let inner_hash = streebog_hash(digest_len, &inner_segments);
    streebog_hash(digest_len, &[outer_pad.as_slice(), inner_hash.as_slice()])
}

fn streebog_hash(digest_len: usize, parts: &[&[u8]]) -> Vec<u8> {
    match digest_len {
        32 => {
            let mut hasher = Streebog256::new();
            for part in parts {
                hasher.update(part);
            }
            hasher.finalize().to_vec()
        }
        64 => {
            let mut hasher = Streebog512::new();
            for part in parts {
                hasher.update(part);
            }
            hasher.finalize().to_vec()
        }
        _ => panic!("unsupported Streebog digest length: {digest_len}"),
    }
}

use crate::{
    Algorithm, Error, ParseError,
    rng::{os_rng, rng_from_seed},
};

/// Parsed GOST private key (little-endian scalar).
#[derive(Clone, PartialEq, Eq)]
pub struct PrivateKey {
    bytes_le: Zeroizing<Vec<u8>>,
}

impl PrivateKey {
    /// Borrow the little-endian scalar bytes that back this private key.
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes_le.as_ref()
    }
}

impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED GOST PrivateKey]")
    }
}

impl fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED GOST PrivateKey]")
    }
}

impl Zeroize for PrivateKey {
    fn zeroize(&mut self) {
        self.bytes_le.zeroize();
    }
}

impl ZeroizeOnDrop for PrivateKey {}

/// Parsed GOST public key (little-endian `x || y` form).
#[derive(Clone, PartialEq, Eq)]
pub struct PublicKey {
    bytes_le: Vec<u8>,
}

impl PublicKey {
    /// Borrow the little-endian concatenated affine coordinates of this public key.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes_le
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("GostPublicKey")
            .field(&hex::encode_upper(&self.bytes_le))
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AffinePoint {
    x: BigUint,
    y: BigUint,
}

impl AffinePoint {
    fn new(x: BigUint, y: BigUint) -> Self {
        Self { x, y }
    }
}

struct CurveParams {
    name: &'static str,
    p: BigUint,
    q: BigUint,
    a: BigUint,
    b: BigUint,
    gx: BigUint,
    gy: BigUint,
    scalar_len: usize,
    digest_len: usize,
}

impl CurveParams {
    fn generator(&self) -> AffinePoint {
        AffinePoint::new(self.gx.clone(), self.gy.clone())
    }
}

enum Params<'a> {
    Bits256(&'a CurveParams),
    Bits512(&'a CurveParams),
}

impl<'a> Params<'a> {
    fn curve(self) -> &'a CurveParams {
        match self {
            Params::Bits256(params) | Params::Bits512(params) => params,
        }
    }
}

fn params_for_algorithm(algorithm: Algorithm) -> Result<Params<'static>, ParseError> {
    match algorithm {
        Algorithm::Gost3410_2012_256ParamSetA => Ok(Params::Bits256(&PARAM_256_A)),
        Algorithm::Gost3410_2012_256ParamSetB => Ok(Params::Bits256(&PARAM_256_B)),
        Algorithm::Gost3410_2012_256ParamSetC => Ok(Params::Bits256(&PARAM_256_C)),
        Algorithm::Gost3410_2012_512ParamSetA => Ok(Params::Bits512(&PARAM_512_A)),
        Algorithm::Gost3410_2012_512ParamSetB => Ok(Params::Bits512(&PARAM_512_B)),
        other => Err(ParseError(format!(
            "algorithm {other:?} is not a supported GOST parameter set"
        ))),
    }
}

macro_rules! curve256 {
    ($name:ident, $algo:expr, $p:literal, $q:literal, $a:literal, $b:literal, $gx:literal, $gy:literal) => {
        static $name: LazyLock<CurveParams> = LazyLock::new(|| CurveParams {
            name: stringify!($algo),
            p: {
                let p = be_hex($p);
                p
            },
            q: be_hex($q),
            a: {
                let p = be_hex($p);
                be_hex($a) % &p
            },
            b: {
                let p = be_hex($p);
                be_hex($b) % &p
            },
            gx: {
                let p = be_hex($p);
                be_hex($gx) % &p
            },
            gy: {
                let p = be_hex($p);
                be_hex($gy) % &p
            },
            scalar_len: 32,
            digest_len: 32,
        });
    };
}

macro_rules! curve512 {
    ($name:ident, $algo:expr, $p:literal, $q:literal, $a:literal, $b:literal, $gx:literal, $gy:literal) => {
        static $name: LazyLock<CurveParams> = LazyLock::new(|| CurveParams {
            name: stringify!($algo),
            p: {
                let p = be_hex($p);
                p
            },
            q: be_hex($q),
            a: {
                let p = be_hex($p);
                be_hex($a) % &p
            },
            b: {
                let p = be_hex($p);
                be_hex($b) % &p
            },
            gx: {
                let p = be_hex($p);
                be_hex($gx) % &p
            },
            gy: {
                let p = be_hex($p);
                be_hex($gy) % &p
            },
            scalar_len: 64,
            digest_len: 64,
        });
    };
}

curve256!(
    PARAM_256_A,
    Algorithm::Gost3410_2012_256ParamSetA,
    "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd97",
    "ffffffffffffffffffffffffffffffff6c611070995ad10045841b09b761b893",
    "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd94",
    "00000000000000000000000000000000000000000000000000000000000000a6",
    "0000000000000000000000000000000000000000000000000000000000000001",
    "8d91e471e0989cda27df505a453f2b7635294f2ddf23e3b122acc99c9e9f1e14"
);

curve256!(
    PARAM_256_B,
    Algorithm::Gost3410_2012_256ParamSetB,
    "8000000000000000000000000000000000000000000000000000000000000c99",
    "800000000000000000000000000000015f700cfff1a624e5e497161bcc8a198f",
    "8000000000000000000000000000000000000000000000000000000000000c96",
    "3e1af419a269a5f866a7d3c25c3df80ae979259373ff2b182f49d4ce7e1bbc8b",
    "0000000000000000000000000000000000000000000000000000000000000001",
    "3fa8124359f96680b83d1c3eb2c070e5c545c9858d03ecfb744bf8d717717efc"
);

curve256!(
    PARAM_256_C,
    Algorithm::Gost3410_2012_256ParamSetC,
    "9b9f605f5a858107ab1ec85e6b41c8aacf846e86789051d37998f7b9022d759b",
    "9b9f605f5a858107ab1ec85e6b41c8aa582ca3511eddfb74f02f3a6598980bb9",
    "9b9f605f5a858107ab1ec85e6b41c8aacf846e86789051d37998f7b9022d7598",
    "000000000000000000000000000000000000000000000000000000000000805a",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "41ece55743711a8c3cbf3783cd08c0ee4d4dc440d4641a8f366e550dfdb3bb67"
);

curve512!(
    PARAM_512_A,
    Algorithm::Gost3410_2012_512ParamSetA,
    "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffdc7",
    "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff27e69532f48d89116ff22b8d4e0560609b4b38abfad2b85dcacdb1411f10b275",
    "fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffdc4",
    "e8c2505dedfc86ddc1bd0b2b6667f1da34b82574761cb0e879bd081cfd0b6265ee3cb090f30d27614cb4574010da90dd862ef9d4ebee4761503190785a71c760",
    "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003",
    "7503cfe87a836ae3a61b8816e25450e6ce5e1c93acf1abc1778064fdcbefa921df1626be4fd036e93d75e6a50e3a41e98028fe5fc235f5b889a589cb5215f2a4"
);

curve512!(
    PARAM_512_B,
    Algorithm::Gost3410_2012_512ParamSetB,
    "8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006f",
    "800000000000000000000000000000000000000000000000000000000000000149a1ec142565a545acfdb77bd9d40cfa8b996712101bea0ec6346c54374f25bd",
    "8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006c",
    "687d1b459dc841457e3e06cf6f5e2517b97c7d614af138bcbf85dc806c4b289f3e965d2db1416d217f8b276fad1ab69c50f78bee1fa3106efb8ccbc7c5140116",
    "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002",
    "1a8f7eda389b094c2c071e3647a8940f3c123b697578c213be6dd9e6c8ec7335dcb228fd1edf4a39152cbcaaf8c0398828041055f94ceeec7e21340780fe41bd"
);

fn be_hex(hex: &str) -> BigUint {
    BigUint::parse_bytes(hex.as_bytes(), 16).expect("valid hex")
}

fn le_bytes_to_biguint(bytes: &[u8]) -> BigUint {
    BigUint::from_bytes_le(bytes)
}

fn scalar_to_le_bytes(value: &BigUint, length: usize) -> Vec<u8> {
    let mut bytes = value.to_bytes_le();
    bytes.resize(length, 0);
    bytes
}

fn point_to_le_bytes(point: &AffinePoint, coord_len: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(coord_len * 2);
    bytes.extend_from_slice(&scalar_to_le_bytes(&point.x, coord_len));
    bytes.extend_from_slice(&scalar_to_le_bytes(&point.y, coord_len));
    bytes
}

fn mod_add(a: &BigUint, b: &BigUint, modulus: &BigUint) -> BigUint {
    let mut sum = a + b;
    if sum >= *modulus {
        sum %= modulus;
    }
    sum
}

#[cfg(test)]
fn mod_sub(a: &BigUint, b: &BigUint, modulus: &BigUint) -> BigUint {
    if a >= b {
        (a - b) % modulus
    } else {
        (modulus + a - b) % modulus
    }
}

fn mod_mul(a: &BigUint, b: &BigUint, modulus: &BigUint) -> BigUint {
    ((a % modulus) * (b % modulus)) % modulus
}

fn mod_square(a: &BigUint, modulus: &BigUint) -> BigUint {
    a.modpow(&BigUint::from(2u8), modulus)
}

fn mod_inv(value: &BigUint, modulus: &BigUint) -> Option<BigUint> {
    if value.is_zero() {
        return None;
    }
    let mut t = BigInt::zero();
    let mut new_t = BigInt::one();
    let mut r = BigInt::from_biguint(Sign::Plus, modulus.clone());
    let mut new_r = BigInt::from_biguint(Sign::Plus, value.clone());

    while !new_r.is_zero() {
        let quotient = &r / &new_r;
        let tmp_t = t - &quotient * &new_t;
        t = new_t;
        new_t = tmp_t;
        let tmp_r = r - &quotient * &new_r;
        r = new_r;
        new_r = tmp_r;
    }

    if r != BigInt::one() {
        return None;
    }

    if t.sign() == Sign::Minus {
        t += BigInt::from_biguint(Sign::Plus, modulus.clone());
    }
    t.to_biguint()
}

fn is_on_curve(params: &CurveParams, point: &AffinePoint) -> bool {
    if point.x >= params.p || point.y >= params.p {
        return false;
    }
    let lhs = mod_square(&point.y, &params.p);
    let x2 = mod_square(&point.x, &params.p);
    let x3 = mod_mul(&x2, &point.x, &params.p);
    let ax = mod_mul(&params.a, &point.x, &params.p);
    let rhs = mod_add(&mod_add(&x3, &ax, &params.p), &params.b, &params.p);
    lhs == rhs
}

#[allow(dead_code)]
fn point_add(params: &CurveParams, p: &AffinePoint, q: &AffinePoint) -> Option<AffinePoint> {
    constant_time::point_add(params, p, q)
}

fn scalar_mul(params: &CurveParams, scalar: &BigUint, point: &AffinePoint) -> Option<AffinePoint> {
    if point.x == params.gx && point.y == params.gy {
        return constant_time::scalar_mul_base(params, scalar);
    }
    constant_time::scalar_mul(params, scalar, point)
}

#[allow(dead_code)]
fn mul_add(
    params: &CurveParams,
    scalar_g: &BigUint,
    scalar_q: &BigUint,
    point_q: &AffinePoint,
) -> Option<AffinePoint> {
    constant_time::mul_add(params, scalar_g, scalar_q, point_q)
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(super) fn compat_point_add(
    params: &CurveParams,
    p: &AffinePoint,
    q: &AffinePoint,
) -> Option<AffinePoint> {
    let modulus = &params.p;
    if p.x == q.x {
        let neg_qy = mod_sub(modulus, &q.y, modulus);
        if p.y == neg_qy {
            return None;
        }
        let numerator = mod_add(
            &mod_mul(&BigUint::from(3u8), &mod_square(&p.x, modulus), modulus),
            &params.a,
            modulus,
        );
        let denominator = mod_mul(&BigUint::from(2u8), &p.y, modulus);
        let inv = mod_inv(&denominator, modulus)?;
        let lambda = mod_mul(&numerator, &inv, modulus);
        let x3 = mod_sub(
            &mod_sub(&mod_square(&lambda, modulus), &p.x, modulus),
            &q.x,
            modulus,
        );
        let y3 = mod_sub(
            &mod_mul(&lambda, &mod_sub(&p.x, &x3, modulus), modulus),
            &p.y,
            modulus,
        );
        Some(AffinePoint::new(x3, y3))
    } else {
        let numerator = mod_sub(&q.y, &p.y, modulus);
        let denominator = mod_sub(&q.x, &p.x, modulus);
        let inv = mod_inv(&denominator, modulus)?;
        let lambda = mod_mul(&numerator, &inv, modulus);
        let x3 = mod_sub(
            &mod_sub(&mod_square(&lambda, modulus), &p.x, modulus),
            &q.x,
            modulus,
        );
        let y3 = mod_sub(
            &mod_mul(&lambda, &mod_sub(&p.x, &x3, modulus), modulus),
            &p.y,
            modulus,
        );
        Some(AffinePoint::new(x3, y3))
    }
}

#[cfg(test)]
#[allow(private_interfaces)]
pub(super) fn compat_scalar_mul(
    params: &CurveParams,
    scalar: &BigUint,
    point: &AffinePoint,
) -> Option<AffinePoint> {
    if scalar.is_zero() {
        return None;
    }
    let mut k = scalar.clone();
    let mut result: Option<AffinePoint> = None;
    let mut addend = point.clone();

    let one = BigUint::one();
    while !k.is_zero() {
        if (&k & &one) == one {
            result = result.as_ref().map_or_else(
                || Some(addend.clone()),
                |current| compat_point_add(params, current, &addend),
            );
        }
        addend = compat_point_add(params, &addend, &addend)?;
        k.shr_assign(1u32);
    }
    result
}

fn hash_to_scalar(params: &CurveParams, message: &[u8]) -> BigUint {
    let mut digest = if params.digest_len == 32 {
        let mut hasher = Streebog256::new();
        hasher.update(message);
        hasher.finalize().to_vec()
    } else {
        let mut hasher = Streebog512::new();
        hasher.update(message);
        hasher.finalize().to_vec()
    };
    digest.reverse();
    digest.resize(params.scalar_len, 0);
    let mut value = BigUint::from_bytes_le(&digest);
    value %= &params.q;
    if value.is_zero() {
        BigUint::one()
    } else {
        value
    }
}

fn increment_nonce(params: &CurveParams, current: &BigUint) -> BigUint {
    let mut next = current + BigUint::one();
    next %= &params.q;
    if next.is_zero() { BigUint::one() } else { next }
}

fn parse_private_generic(params: &CurveParams, payload: &[u8]) -> Result<PrivateKey, ParseError> {
    if payload.len() != params.scalar_len {
        return Err(ParseError(format!(
            "invalid private key length for {}: expected {}, got {}",
            params.name,
            params.scalar_len,
            payload.len()
        )));
    }
    let scalar = le_bytes_to_biguint(payload);
    if scalar.is_zero() || scalar >= params.q {
        return Err(ParseError(format!(
            "private key outside [1, q-1] for {}",
            params.name
        )));
    }
    Ok(PrivateKey {
        bytes_le: Zeroizing::new(payload.to_vec()),
    })
}

fn parse_public_generic(params: &CurveParams, payload: &[u8]) -> Result<PublicKey, ParseError> {
    if payload.len() != params.scalar_len * 2 {
        return Err(ParseError(format!(
            "public key for {} must be {} bytes, got {}",
            params.name,
            params.scalar_len * 2,
            payload.len()
        )));
    }
    let (x_bytes, y_bytes) = payload.split_at(params.scalar_len);
    let point = AffinePoint::new(le_bytes_to_biguint(x_bytes), le_bytes_to_biguint(y_bytes));
    if !is_on_curve(params, &point) {
        return Err(ParseError(format!(
            "public key is not on the curve for {}",
            params.name
        )));
    }
    Ok(PublicKey {
        bytes_le: payload.to_vec(),
    })
}

fn scalar_from_private(params: &CurveParams, key: &PrivateKey) -> Result<BigUint, Error> {
    if key.as_bytes().len() != params.scalar_len {
        return Err(Error::KeyGen(format!(
            "private key length mismatch for {}",
            params.name
        )));
    }
    Ok(le_bytes_to_biguint(key.as_bytes()))
}

fn point_from_public(params: &CurveParams, key: &PublicKey) -> Result<AffinePoint, Error> {
    if key.as_bytes().len() != params.scalar_len * 2 {
        return Err(Error::BadSignature);
    }
    let (x_bytes, y_bytes) = key.as_bytes().split_at(params.scalar_len);
    let point = AffinePoint::new(le_bytes_to_biguint(x_bytes), le_bytes_to_biguint(y_bytes));
    if !is_on_curve(params, &point) {
        return Err(Error::BadSignature);
    }
    Ok(point)
}

fn sign_impl(
    params: &CurveParams,
    message: &[u8],
    private: &PrivateKey,
    nonce_gen: &mut impl DeterministicNonceGenerator,
    extra_entropy: Option<&[u8]>,
) -> Result<Vec<u8>, Error> {
    let private_scalar = scalar_from_private(params, private)?;
    if private_scalar.is_zero() || private_scalar >= params.q {
        return Err(Error::KeyGen(format!(
            "private key outside [1, q-1] for {}",
            params.name
        )));
    }
    let mut message_scalar = hash_to_scalar(params, message);
    if message_scalar.is_zero() {
        message_scalar = BigUint::one();
    }
    let mut nonce = nonce_gen.generate(params, &private_scalar, message, extra_entropy);
    let generator = params.generator();

    loop {
        let point = if let Some(point) = scalar_mul(params, &nonce, &generator) {
            point
        } else {
            nonce = increment_nonce(params, &nonce);
            continue;
        };
        let r_component = &point.x % &params.q;
        if r_component.is_zero() {
            nonce = increment_nonce(params, &nonce);
            continue;
        }
        let private_term = (&r_component * &private_scalar) % &params.q;
        let message_term = (&nonce * &message_scalar) % &params.q;
        let s_component = (private_term + message_term) % &params.q;
        if s_component.is_zero() {
            nonce = increment_nonce(params, &nonce);
            continue;
        }
        let mut signature = Vec::with_capacity(params.scalar_len * 2);
        signature.extend_from_slice(&scalar_to_le_bytes(&r_component, params.scalar_len));
        signature.extend_from_slice(&scalar_to_le_bytes(&s_component, params.scalar_len));
        return Ok(signature);
    }
}

fn verify_impl(
    params: &CurveParams,
    message: &[u8],
    signature: &[u8],
    public: &PublicKey,
) -> Result<(), Error> {
    if signature.len() != params.scalar_len * 2 {
        return Err(Error::BadSignature);
    }
    let (r_bytes, s_bytes) = signature.split_at(params.scalar_len);
    let r = le_bytes_to_biguint(r_bytes);
    let s = le_bytes_to_biguint(s_bytes);
    if r.is_zero() || r >= params.q || s.is_zero() || s >= params.q {
        return Err(Error::BadSignature);
    }
    let mut e = hash_to_scalar(params, message);
    if e.is_zero() {
        e = BigUint::one();
    }
    let inv = mod_inv(&e, &params.q).ok_or(Error::BadSignature)?;
    let z1 = (&s * &inv) % &params.q;
    let r_neg = (&params.q + &params.q - &r) % &params.q;
    let z2 = (&r_neg * &inv) % &params.q;

    let q_point = point_from_public(params, public)?;

    let sum = mul_add(params, &z1, &z2, &q_point).ok_or(Error::BadSignature)?;
    let x = sum.x % &params.q;
    if x == r {
        Ok(())
    } else {
        Err(Error::BadSignature)
    }
}

fn derive_public_impl(params: &CurveParams, private: &PrivateKey) -> Result<PublicKey, Error> {
    let scalar = scalar_from_private(params, private)?;
    let point = scalar_mul(params, &scalar, &params.generator())
        .ok_or_else(|| Error::KeyGen("failed to derive GOST public key".into()))?;
    if !is_on_curve(params, &point) {
        return Err(Error::KeyGen(
            "derived GOST public key is not on the curve".into(),
        ));
    }
    Ok(PublicKey {
        bytes_le: point_to_le_bytes(&point, params.scalar_len),
    })
}

fn random_scalar<R: RngCore>(params: &CurveParams, rng: &mut R) -> BigUint {
    let mut buf = vec![0u8; params.scalar_len];
    loop {
        rng.fill_bytes(&mut buf);
        let scalar = BigUint::from_bytes_le(&buf);
        if scalar.is_zero() || scalar >= params.q {
            continue;
        }
        return scalar;
    }
}

fn keypair_random_impl(params: &CurveParams) -> Result<(PublicKey, PrivateKey), Error> {
    let mut rng = os_rng();
    let scalar = random_scalar(params, &mut rng);
    let private = PrivateKey {
        bytes_le: Zeroizing::new(scalar_to_le_bytes(&scalar, params.scalar_len)),
    };
    let public = derive_public_impl(params, &private)?;
    Ok((public, private))
}

fn keypair_seed_impl(params: &CurveParams, seed: &[u8]) -> Result<(PublicKey, PrivateKey), Error> {
    let mut rng = rng_from_seed(seed.to_vec());
    let scalar = random_scalar(params, &mut rng);
    let private = PrivateKey {
        bytes_le: Zeroizing::new(scalar_to_le_bytes(&scalar, params.scalar_len)),
    };
    let public = derive_public_impl(params, &private)?;
    Ok((public, private))
}

/// Parse a serialized public key for the selected GOST parameter set.
///
/// # Errors
/// Returns [`ParseError`] when `payload` does not encode a valid public key for `algorithm`.
pub fn parse_public_key(algorithm: Algorithm, payload: &[u8]) -> Result<PublicKey, ParseError> {
    let params = params_for_algorithm(algorithm)?;
    parse_public_generic(params.curve(), payload)
}

/// Parse a serialized private key for the selected GOST parameter set.
///
/// # Errors
/// Returns [`ParseError`] when `payload` does not encode a valid private key for `algorithm`.
pub fn parse_private_key(algorithm: Algorithm, payload: &[u8]) -> Result<PrivateKey, ParseError> {
    let params = params_for_algorithm(algorithm)?;
    parse_private_generic(params.curve(), payload)
}

/// Generate a random key pair.
///
/// # Errors
/// Returns [`Error::KeyGen`] if the parameter set is unsupported or key generation fails.
pub fn generate_random_keypair(algorithm: Algorithm) -> Result<(PublicKey, PrivateKey), Error> {
    let params = params_for_algorithm(algorithm).map_err(|err| Error::KeyGen(err.to_string()))?;
    keypair_random_impl(params.curve())
}

/// Generate a deterministic key pair from a seed.
///
/// # Errors
/// Returns [`Error::KeyGen`] if the parameter set is unsupported or the seed is rejected.
pub fn generate_seeded_keypair(
    algorithm: Algorithm,
    seed: &[u8],
) -> Result<(PublicKey, PrivateKey), Error> {
    let params = params_for_algorithm(algorithm).map_err(|err| Error::KeyGen(err.to_string()))?;
    keypair_seed_impl(params.curve(), seed)
}

/// Derive the matching public key from the given private key.
///
/// # Errors
/// Returns [`Error::KeyGen`] if `private` is invalid for `algorithm`.
pub fn derive_public_key(algorithm: Algorithm, private: &PrivateKey) -> Result<PublicKey, Error> {
    let params = params_for_algorithm(algorithm).map_err(|err| Error::KeyGen(err.to_string()))?;
    derive_public_impl(params.curve(), private)
}

/// Validate that the supplied public and private keys form a pair.
///
/// # Errors
/// Returns [`Error::KeyGen`] when the derived public key does not match `public`.
pub fn validate_key_pair(
    algorithm: Algorithm,
    public: &PublicKey,
    private: &PrivateKey,
) -> Result<(), Error> {
    let derived = derive_public_key(algorithm, private)?;
    if derived.as_bytes() == public.as_bytes() {
        Ok(())
    } else {
        Err(Error::KeyGen("GOST key pair mismatch".into()))
    }
}

/// Sign a message with the specified private key.
///
/// # Errors
/// Returns [`Error::KeyGen`] if `algorithm` is unsupported or signing fails.
pub fn sign(algorithm: Algorithm, message: &[u8], private: &PrivateKey) -> Result<Vec<u8>, Error> {
    let params = params_for_algorithm(algorithm).map_err(|err| Error::KeyGen(err.to_string()))?;
    let curve = params.curve();
    let mut nonce_gen = StreebogNonceGenerator::new();
    let mut rng = os_rng();
    let mut entropy = Zeroizing::new(vec![0u8; curve.scalar_len]);
    rng.fill_bytes(entropy.as_mut_slice());
    sign_impl(
        curve,
        message,
        private,
        &mut nonce_gen,
        Some(entropy.as_slice()),
    )
}

/// Verify a signature produced by the specified parameter set.
///
/// # Errors
/// Returns [`Error::KeyGen`] if verification fails or the parameter set is unsupported.
pub fn verify(
    algorithm: Algorithm,
    message: &[u8],
    signature: &[u8],
    public_key: &PublicKey,
) -> Result<(), Error> {
    let params = params_for_algorithm(algorithm).map_err(|err| Error::KeyGen(err.to_string()))?;
    verify_impl(params.curve(), message, signature, public_key)
}

#[cfg(test)]
mod tests {
    use std::{hint::black_box, time::Instant};

    use num_traits::{One, ToPrimitive};
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    use super::*;

    fn seed_pair() -> (PublicKey, PrivateKey) {
        let seed = b"iroha-gost-test-seed";
        generate_seeded_keypair(Algorithm::Gost3410_2012_256ParamSetA, seed).unwrap()
    }

    #[test]
    fn sign_verify_roundtrip() {
        let (public, private) = seed_pair();
        let params = match params_for_algorithm(Algorithm::Gost3410_2012_256ParamSetA).unwrap() {
            Params::Bits256(params) => params,
            _ => unreachable!(),
        };
        let public_point = point_from_public(params, &public).unwrap();
        assert!(is_on_curve(params, &public_point));
        let message = b"test message for gost";
        let signature = sign(Algorithm::Gost3410_2012_256ParamSetA, message, &private).unwrap();
        let result = verify(
            Algorithm::Gost3410_2012_256ParamSetA,
            message,
            &signature,
            &public,
        );
        result.unwrap();
    }

    #[test]
    fn signature_deterministic_without_extra_entropy() {
        let (_, private) = seed_pair();
        let message = b"deterministic nonce check";
        let params = match params_for_algorithm(Algorithm::Gost3410_2012_256ParamSetA).unwrap() {
            Params::Bits256(params) => params,
            _ => unreachable!(),
        };
        let mut gen1 = StreebogNonceGenerator::new();
        let sig1 = sign_impl(params, message, &private, &mut gen1, None).unwrap();
        let mut gen2 = StreebogNonceGenerator::new();
        let sig2 = sign_impl(params, message, &private, &mut gen2, None).unwrap();
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn signature_changes_with_extra_entropy() {
        let (_, private) = seed_pair();
        let message = b"hedged entropy nonce check";
        let params = match params_for_algorithm(Algorithm::Gost3410_2012_256ParamSetA).unwrap() {
            Params::Bits256(params) => params,
            _ => unreachable!(),
        };
        let mut gen1 = StreebogNonceGenerator::new();
        let extra1 = vec![0xAA; params.scalar_len];
        let sig1 = sign_impl(
            params,
            message,
            &private,
            &mut gen1,
            Some(extra1.as_slice()),
        )
        .unwrap();

        let mut gen2 = StreebogNonceGenerator::new();
        let extra2 = vec![0xBB; params.scalar_len];
        let sig2 = sign_impl(
            params,
            message,
            &private,
            &mut gen2,
            Some(extra2.as_slice()),
        )
        .unwrap();

        assert_ne!(sig1, sig2);
    }

    #[test]
    fn gost_sign_constant_time_under_dudect() {
        let algorithms = [
            Algorithm::Gost3410_2012_256ParamSetA,
            Algorithm::Gost3410_2012_256ParamSetB,
            Algorithm::Gost3410_2012_256ParamSetC,
            Algorithm::Gost3410_2012_512ParamSetA,
            Algorithm::Gost3410_2012_512ParamSetB,
        ];

        for algorithm in algorithms {
            run_dudect_timing_check(algorithm);
        }
    }

    fn run_dudect_timing_check(algorithm: Algorithm) {
        const SAMPLES_PER_CLASS: usize = 40;
        const WARMUP_ITERATIONS: usize = 20;

        let params = match params_for_algorithm(algorithm).unwrap() {
            Params::Bits512(params) | Params::Bits256(params) => params,
        };
        let (_, private) = generate_seeded_keypair(algorithm, b"dudect-gost-keyseed")
            .expect("seeded keypair for dudect");

        let zero_message = vec![0u8; 64];
        let mut random_message = zero_message.clone();
        let mut rng = StdRng::from_seed([0x42; 32]);

        let mut class0 = Vec::with_capacity(SAMPLES_PER_CLASS);
        let mut class1 = Vec::with_capacity(SAMPLES_PER_CLASS);

        let total_iterations = (SAMPLES_PER_CLASS * 2) + WARMUP_ITERATIONS;
        for iteration in 0..total_iterations {
            let class_is_zero = iteration % 2 == 0;
            let message = if class_is_zero {
                zero_message.as_slice()
            } else {
                rng.fill_bytes(random_message.as_mut_slice());
                random_message.as_slice()
            };

            let mut nonce_gen = StreebogNonceGenerator::new();
            let start = Instant::now();
            let signature =
                sign_impl(params, message, &private, &mut nonce_gen, None).expect("sign");
            black_box(signature);
            let elapsed = start.elapsed().as_secs_f64();

            if iteration < WARMUP_ITERATIONS {
                continue;
            }

            if class_is_zero {
                class0.push(elapsed);
            } else {
                class1.push(elapsed);
            }
        }

        assert_eq!(
            class0.len(),
            SAMPLES_PER_CLASS,
            "class0 samples missing for {algorithm:?}"
        );
        assert_eq!(
            class1.len(),
            SAMPLES_PER_CLASS,
            "class1 samples missing for {algorithm:?}"
        );

        let t_stat = welch_t(&class0, &class1);
        assert!(
            t_stat.abs() < 5.0,
            "GOST signer for {algorithm:?} failed dudect check: t-statistic={t_stat}"
        );
    }

    #[test]
    fn seeded_keypair_reproducible() {
        let seed = b"seeded gost keypair";
        let (public1, private1) =
            generate_seeded_keypair(Algorithm::Gost3410_2012_256ParamSetB, seed).unwrap();
        let (public2, private2) =
            generate_seeded_keypair(Algorithm::Gost3410_2012_256ParamSetB, seed).unwrap();
        assert_eq!(public1.as_bytes(), public2.as_bytes());
        assert_eq!(private1.as_bytes(), private2.as_bytes());
    }

    #[test]
    fn verify_rejects_modified_message() {
        let (public, private) = seed_pair();
        let message = b"sign me";
        let signature = sign(Algorithm::Gost3410_2012_256ParamSetA, message, &private).unwrap();
        let tampered = b"sign me?";
        assert!(
            verify(
                Algorithm::Gost3410_2012_256ParamSetA,
                tampered,
                &signature,
                &public
            )
            .is_err()
        );
    }

    #[test]
    fn sign_verify_roundtrip_512() {
        let seed = b"gost-512-roundtrip";
        let (public, private) =
            generate_seeded_keypair(Algorithm::Gost3410_2012_512ParamSetB, seed).unwrap();
        let message = b"512-bit gost roundtrip";
        let signature =
            sign(Algorithm::Gost3410_2012_512ParamSetB, message, &private).expect("sign");
        verify(
            Algorithm::Gost3410_2012_512ParamSetB,
            message,
            &signature,
            &public,
        )
        .expect("verify");
    }

    #[test]
    fn reject_invalid_key_sizes() {
        let params = match params_for_algorithm(Algorithm::Gost3410_2012_256ParamSetA).unwrap() {
            Params::Bits256(params) => params,
            _ => unreachable!(),
        };
        // Private key must be exactly scalar_len bytes.
        assert!(parse_private_generic(params, &[0x01]).is_err());
        // Public key must be 2 * scalar_len bytes.
        assert!(parse_public_generic(params, &[0x02; 10]).is_err());
    }

    #[test]
    fn generator_is_on_curve() {
        let params = match params_for_algorithm(Algorithm::Gost3410_2012_256ParamSetA).unwrap() {
            Params::Bits256(params) => params,
            _ => unreachable!(),
        };
        assert!(is_on_curve(params, &params.generator()));
    }

    #[test]
    fn generator_is_on_curve_512() {
        let params = match params_for_algorithm(Algorithm::Gost3410_2012_512ParamSetA).unwrap() {
            Params::Bits512(params) => params,
            _ => unreachable!(),
        };
        let generator = params.generator();
        let lhs = mod_square(&generator.y, &params.p);
        let x2 = mod_square(&generator.x, &params.p);
        let x3 = mod_mul(&x2, &generator.x, &params.p);
        let ax = mod_mul(&params.a, &generator.x, &params.p);
        let rhs = mod_add(&mod_add(&x3, &ax, &params.p), &params.b, &params.p);
        assert_eq!(lhs, rhs);
        assert!(is_on_curve(params, &generator));
    }

    #[test]
    fn scalar_mul_matches_repeated_addition() {
        let params = match params_for_algorithm(Algorithm::Gost3410_2012_256ParamSetA).unwrap() {
            Params::Bits256(params) => params,
            _ => unreachable!(),
        };
        let generator = params.generator();
        let two = scalar_mul(params, &BigUint::from(2u8), &generator).unwrap();
        let expected_two = point_add(params, &generator, &generator).unwrap();
        assert_eq!(two.x, expected_two.x);
        assert_eq!(two.y, expected_two.y);

        let three = scalar_mul(params, &BigUint::from(3u8), &generator).unwrap();
        let expected_three = point_add(params, &expected_two, &generator).unwrap();
        assert_eq!(three.x, expected_three.x);
        assert_eq!(three.y, expected_three.y);
    }

    #[test]
    fn mul_add_matches_compat() {
        let mut rng = rng_from_seed(b"gost-mul-add".to_vec());
        for algorithm in [
            Algorithm::Gost3410_2012_256ParamSetA,
            Algorithm::Gost3410_2012_256ParamSetB,
            Algorithm::Gost3410_2012_256ParamSetC,
            Algorithm::Gost3410_2012_512ParamSetA,
            Algorithm::Gost3410_2012_512ParamSetB,
        ] {
            let params_variant = params_for_algorithm(algorithm).unwrap();
            let params = params_variant.curve();

            for _ in 0..8 {
                let scalar_g = random_scalar(params, &mut rng);
                let scalar_q = random_scalar(params, &mut rng);
                let point_scalar = random_scalar(params, &mut rng);

                let q_point = match compat_scalar_mul(params, &point_scalar, &params.generator()) {
                    Some(point) => point,
                    None => continue,
                };

                let actual = mul_add(params, &scalar_g, &scalar_q, &q_point);

                let expected = {
                    let part_g = compat_scalar_mul(params, &scalar_g, &params.generator());
                    let part_q = compat_scalar_mul(params, &scalar_q, &q_point);
                    match (part_g, part_q) {
                        (Some(g), Some(q)) => compat_point_add(params, &g, &q),
                        (Some(g), None) => Some(g),
                        (None, Some(q)) => Some(q),
                        (None, None) => None,
                    }
                };

                match (actual, expected) {
                    (None, None) => {}
                    (Some(a), Some(e)) => {
                        assert_eq!(a.x, e.x, "algorithm {algorithm:?}");
                        assert_eq!(a.y, e.y, "algorithm {algorithm:?}");
                        assert!(is_on_curve(params, &a));
                    }
                    (left, right) => {
                        panic!(
                            "mul_add mismatch for {algorithm:?}: actual={left:?}, expected={right:?}"
                        )
                    }
                }
            }
        }
    }

    #[test]
    fn deterministic_nonce_uses_domain_separation() {
        let params = match params_for_algorithm(Algorithm::Gost3410_2012_256ParamSetA).unwrap() {
            Params::Bits256(params) => params,
            _ => unreachable!(),
        };
        let secret = BigUint::from(42u32);
        let message = b"domain separation check";

        let mut generator_with_tag = StreebogNonceGenerator::new();
        let nonce_with_domain = generator_with_tag.generate(params, &secret, message, None);

        let mut generator_without_tag = StreebogNonceGenerator::with_domain(b"");
        let nonce_without_domain = generator_without_tag.generate(params, &secret, message, None);

        assert_ne!(nonce_with_domain, nonce_without_domain);
    }

    #[test]
    #[ignore = "emits fixture data for Wycheproof compatibility checks"]
    fn dump_wycheproof_vectors() {
        let message = b"Wycheproof deterministic message";
        for (algorithm, seed) in [
            (
                Algorithm::Gost3410_2012_256ParamSetA,
                b"wycheproof-gost-256-a".as_slice(),
            ),
            (
                Algorithm::Gost3410_2012_256ParamSetB,
                b"wycheproof-gost-256-b".as_slice(),
            ),
            (
                Algorithm::Gost3410_2012_256ParamSetC,
                b"wycheproof-gost-256-c".as_slice(),
            ),
            (
                Algorithm::Gost3410_2012_512ParamSetA,
                b"wycheproof-gost-512-a".as_slice(),
            ),
            (
                Algorithm::Gost3410_2012_512ParamSetB,
                b"wycheproof-gost-512-b".as_slice(),
            ),
        ] {
            let (public, private) = generate_seeded_keypair(algorithm, seed).expect("keypair");
            let params = params_for_algorithm(algorithm).expect("params").curve();
            let mut generator = StreebogNonceGenerator::new();
            let signature =
                sign_impl(params, message, &private, &mut generator, None).expect("sign");
            let mut invalid_signature = signature.clone();
            invalid_signature[0] ^= 0x01;

            println!(
                "{{\"algorithm\":\"{alg:?}\",\"public\":\"{public}\",\"message\":\"{msg}\",\"valid\":\"{sig}\",\"invalid\":\"{bad}\"}}",
                alg = algorithm,
                public = hex::encode(public.as_bytes()),
                msg = hex::encode(message),
                sig = hex::encode(signature),
                bad = hex::encode(invalid_signature),
            );
        }
    }

    fn welch_t(class0: &[f64], class1: &[f64]) -> f64 {
        fn mean(samples: &[f64]) -> f64 {
            let len = samples.len();
            if len == 0 {
                return 0.0;
            }
            let len = len.to_f64().expect("sample length fits into f64 mantissa");
            samples.iter().sum::<f64>() / len
        }

        fn variance(samples: &[f64], mean: f64) -> f64 {
            if samples.len() < 2 {
                return 0.0;
            }
            let sum = samples
                .iter()
                .map(|value| {
                    let diff = value - mean;
                    diff * diff
                })
                .sum::<f64>();
            let denom = (samples.len() - 1)
                .to_f64()
                .expect("sample length fits into f64 mantissa");
            sum / denom
        }

        let mean0 = mean(class0);
        let mean1 = mean(class1);
        let var0 = variance(class0, mean0);
        let var1 = variance(class1, mean1);

        let len0 = class0
            .len()
            .to_f64()
            .expect("class length fits into f64 mantissa");
        let len1 = class1
            .len()
            .to_f64()
            .expect("class length fits into f64 mantissa");
        let denom = (var0 / len0) + (var1 / len1);
        if denom == 0.0 {
            0.0
        } else {
            (mean0 - mean1) / denom.sqrt()
        }
    }

    fn increment_le_bytes(bytes: &mut [u8]) {
        let mut carry = 1u16;
        for byte in bytes.iter_mut() {
            let sum = u16::from(*byte) + carry;
            *byte = u8::try_from(sum & 0x00ff).expect("low byte must fit");
            carry = sum >> 8;
            if carry == 0 {
                break;
            }
        }
    }

    struct StubRng {
        samples: Vec<Vec<u8>>,
        idx: usize,
    }

    impl StubRng {
        fn new(samples: Vec<Vec<u8>>) -> Self {
            Self { samples, idx: 0 }
        }
    }

    impl RngCore for StubRng {
        fn next_u32(&mut self) -> u32 {
            panic!("next_u32 not supported in StubRng");
        }

        fn next_u64(&mut self) -> u64 {
            panic!("next_u64 not supported in StubRng");
        }

        fn fill_bytes(&mut self, dest: &mut [u8]) {
            let sample = self
                .samples
                .get(self.idx)
                .expect("StubRng ran out of samples");
            self.idx += 1;
            assert_eq!(
                sample.len(),
                dest.len(),
                "StubRng sample length must match destination length"
            );
            dest.copy_from_slice(sample);
        }
    }

    #[test]
    fn random_scalar_rejects_zero_and_out_of_range_samples() {
        let params = match params_for_algorithm(Algorithm::Gost3410_2012_256ParamSetA).unwrap() {
            Params::Bits256(params) => params,
            _ => unreachable!(),
        };

        let zero = vec![0u8; params.scalar_len];
        let mut q_bytes = params.q.to_bytes_le();
        q_bytes.resize(params.scalar_len, 0);
        let mut q_plus_one = q_bytes.clone();
        increment_le_bytes(&mut q_plus_one);
        let mut high = vec![0xFF; params.scalar_len];
        // Ensure `high` is still >= q by setting high bit if needed.
        if high < q_bytes {
            high[params.scalar_len - 1] = 0xFF;
        }
        let mut valid = (&params.q - BigUint::one()).to_bytes_le();
        valid.resize(params.scalar_len, 0);

        let samples = vec![zero, q_bytes, q_plus_one, high, valid.clone()];
        let mut rng = StubRng::new(samples);

        let scalar = random_scalar(params, &mut rng);
        assert_eq!(scalar, &params.q - BigUint::one());
    }

    #[test]
    fn biguint_mod_matches_python() {
        use num_bigint::BigUint as NumBigUint;
        let p = NumBigUint::parse_bytes(
            b"fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffdc7",
            16,
        )
        .unwrap();
        let gy = NumBigUint::parse_bytes(
            b"7503cfe87a836ae3a61b8816e25450e6ce5e1c93acf1abc1778064fdcbefa921df1626be4fd036e93d75e6a50e3a41e98028fe5fc235f5b889a589cb5215f2a4",
            16,
        )
        .unwrap();
        let params = match params_for_algorithm(Algorithm::Gost3410_2012_512ParamSetA).unwrap() {
            Params::Bits512(params) => params,
            _ => unreachable!(),
        };
        assert_eq!(params.gy, gy);
        assert_eq!(params.p, p);
        let lhs = (&gy * &gy) % &p;
        assert_eq!(
            lhs.to_str_radix(16),
            "e8c2505dedfc86ddc1bd0b2b6667f1da34b82574761cb0e879bd081cfd0b6265ee3cb090f30d27614cb4574010da90dd862ef9d4ebee4761503190785a71c772"
        );
    }
}
