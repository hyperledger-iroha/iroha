//! Fixed-width field and point arithmetic for the five admitted GOST parameter sets.
//!
//! This module provides field operations backed by `crypto-bigint`’s constant-time
//! Montgomery arithmetic and Jacobian point helpers. Signing adapts its outer
//! integers to this relation; verification uses borrowed bytes and fixed-width
//! scalars throughout.
use super::{AffinePoint as OuterAffinePoint, CurveParams as OuterCurveParams};
use crate::Algorithm;
use crypto_bigint::{
    Odd, U256, U512, Uint,
    modular::{MontyForm, MontyParams},
};
use num_bigint::BigUint;
use num_traits::Zero;
use std::{ptr, sync::LazyLock};
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};
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
    scalar_params: MontyParams<LIMBS>,
    name: &'static str,
}
impl<const LIMBS: usize> CurveParameters<LIMBS> {
    fn generator(&self) -> JacobianPoint<LIMBS> {
        JacobianPoint::from_affine(&self.generator, self.field_params)
    }
    fn field_params(&self) -> MontyParams<LIMBS> {
        self.field_params
    }
}
fn params_from_hex<const LIMBS: usize>(hex: &str) -> MontyParams<LIMBS> {
    let modulus = Odd::new(Uint::<LIMBS>::from_be_hex(hex)).expect("curve modulus must be odd");
    MontyParams::new_vartime(modulus)
}
fn fe_from_hex<const LIMBS: usize>(hex: &str, params: MontyParams<LIMBS>) -> FieldElement<LIMBS> {
    FieldElement::from_uint(Uint::<LIMBS>::from_be_hex(hex), params)
}
fn curve_from_constants<const LIMBS: usize>(
    constants: &super::parameters::CurveConstants,
) -> CurveParameters<LIMBS> {
    assert_eq!(constants.scalar_len, Uint::<LIMBS>::BYTES);
    let field_params = params_from_hex::<LIMBS>(constants.p);
    CurveParameters {
        field_params,
        a: fe_from_hex(constants.a, field_params),
        b: fe_from_hex(constants.b, field_params),
        generator: AffinePoint {
            x: fe_from_hex(constants.gx, field_params),
            y: fe_from_hex(constants.gy, field_params),
        },
        scalar_modulus: Uint::<LIMBS>::from_be_hex(constants.q),
        scalar_params: params_from_hex(constants.q),
        name: constants.name,
    }
}
static CURVE_256_A: LazyLock<CurveParameters<{ U256::LIMBS }>> =
    LazyLock::new(|| curve_from_constants(&super::parameters::PARAM_256_A));
static CURVE_256_B: LazyLock<CurveParameters<{ U256::LIMBS }>> =
    LazyLock::new(|| curve_from_constants(&super::parameters::PARAM_256_B));
static CURVE_256_C: LazyLock<CurveParameters<{ U256::LIMBS }>> =
    LazyLock::new(|| curve_from_constants(&super::parameters::PARAM_256_C));
static CURVE_512_A: LazyLock<CurveParameters<{ U512::LIMBS }>> =
    LazyLock::new(|| curve_from_constants(&super::parameters::PARAM_512_A));
static CURVE_512_B: LazyLock<CurveParameters<{ U512::LIMBS }>> =
    LazyLock::new(|| curve_from_constants(&super::parameters::PARAM_512_B));
enum CurveSelection {
    Bits256(&'static CurveParameters<{ U256::LIMBS }>),
    Bits512(&'static CurveParameters<{ U512::LIMBS }>),
}
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
fn generator_outer_point<const LIMBS: usize>(curve: &CurveParameters<LIMBS>) -> OuterAffinePoint {
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
#[cfg(test)]
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
    generator_scalar_uint: &Uint<LIMBS>,
    point_scalar_uint: &Uint<LIMBS>,
    point_q: &AffinePoint<LIMBS>,
) -> Option<AffinePoint<LIMBS>> {
    if bool::from(
        generator_scalar_uint.ct_eq(&Uint::<LIMBS>::ZERO)
            & point_scalar_uint.ct_eq(&Uint::<LIMBS>::ZERO),
    ) {
        return None;
    }
    let params = curve.field_params();
    let base = curve.generator();
    let point = JacobianPoint::from_affine(point_q, params);
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
    acc.as_affine()
}
#[cfg(test)]
pub(super) fn mul_add_for_test(
    algorithm: Algorithm,
    scalar_g: &BigUint,
    scalar_q: &BigUint,
    point_q: &OuterAffinePoint,
) -> Option<OuterAffinePoint> {
    fn run<const LIMBS: usize>(
        curve: &CurveParameters<LIMBS>,
        scalar_g: &BigUint,
        scalar_q: &BigUint,
        point_q: &OuterAffinePoint,
    ) -> Option<OuterAffinePoint> {
        let actual = mul_add_impl(
            curve,
            &biguint_to_uint(scalar_g)?,
            &biguint_to_uint(scalar_q)?,
            &affine_from_outer(curve, point_q)?,
        )?;
        Some(OuterAffinePoint::new(
            uint_to_biguint(&actual.x.as_uint()),
            uint_to_biguint(&actual.y.as_uint()),
        ))
    }
    match curve_for_algorithm(algorithm)? {
        CurveSelection::Bits256(curve) => run(curve, scalar_g, scalar_q, point_q),
        CurveSelection::Bits512(curve) => run(curve, scalar_g, scalar_q, point_q),
    }
}
#[cfg(test)]
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
    use super::*;
    use crate::{
        rng::rng_from_seed,
        signature::gost::{Params, compat_point_add, compat_scalar_mul, params_for_algorithm},
    };
    use num_bigint::BigUint;
    use num_traits::{One, Zero};
    use rand_core::RngCore;
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
        let params = match params_for_algorithm(Algorithm::Gost3410_2012_256ParamSetB).unwrap() {
            Params::Bits256(p) => p,
            _ => unreachable!(),
        };
        let mut rng = rng_from_seed(b"ct-scalar-test".to_vec());
        let mut scalar_bytes = vec![0u8; params.scalar_len];
        rng.fill_bytes(&mut scalar_bytes);
        let modulus_big = params.q.clone();
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

    fn compare_parameter_fields<const LIMBS: usize>(
        curve: &CurveParameters<LIMBS>,
        params: &OuterCurveParams,
    ) {
        assert_eq!(
            Uint::<LIMBS>::BYTES,
            params.scalar_len,
            "{} width",
            params.name
        );
        assert_eq!(
            uint_to_biguint(curve.field_params.modulus().as_ref()),
            params.p,
            "{} p",
            params.name
        );
        assert_eq!(
            uint_to_biguint(&curve.scalar_modulus),
            params.q,
            "{} q",
            params.name
        );
        assert_eq!(
            uint_to_biguint(&curve.a.as_uint()),
            params.a,
            "{} a",
            params.name
        );
        assert_eq!(
            uint_to_biguint(&curve.b.as_uint()),
            params.b,
            "{} b",
            params.name
        );
        assert_eq!(
            uint_to_biguint(&curve.generator.x.as_uint()),
            params.gx,
            "{} gx",
            params.name
        );
        assert_eq!(
            uint_to_biguint(&curve.generator.y.as_uint()),
            params.gy,
            "{} gy",
            params.name
        );
    }

    fn assert_generator_order<const LIMBS: usize>(
        curve: &CurveParameters<LIMBS>,
        params: &OuterCurveParams,
    ) {
        let order = uint_to_biguint(&curve.scalar_modulus);
        let generator = params.generator();
        assert!(super::super::is_on_curve(params, &generator));
        assert!(
            compat_scalar_mul(params, &order, &generator).is_none(),
            "{} independent qG",
            params.name
        );
        assert!(
            scalar_mul_impl(curve, &order, &generator).is_none(),
            "{} fixed qG",
            params.name
        );
        let previous = &order - BigUint::one();
        let expected = compat_scalar_mul(params, &previous, &generator).expect("(q-1)G is nonzero");
        let actual =
            scalar_mul_impl(curve, &previous, &generator).expect("fixed (q-1)G is nonzero");
        assert_eq!(actual, expected, "{} predecessor", params.name);
    }

    const PARAMETER_ALGORITHMS: [Algorithm; 5] = [
        Algorithm::Gost3410_2012_256ParamSetA,
        Algorithm::Gost3410_2012_256ParamSetB,
        Algorithm::Gost3410_2012_256ParamSetC,
        Algorithm::Gost3410_2012_512ParamSetA,
        Algorithm::Gost3410_2012_512ParamSetB,
    ];

    #[test]
    fn fixed_and_outer_parameters_match_all_six_fields_in_all_five_sets() {
        for algorithm in PARAMETER_ALGORITHMS {
            let params = params_for_algorithm(algorithm).unwrap().curve();
            match curve_for_algorithm(algorithm).unwrap() {
                CurveSelection::Bits256(curve) => compare_parameter_fields(curve, params),
                CurveSelection::Bits512(curve) => compare_parameter_fields(curve, params),
            }
        }
    }

    #[test]
    fn canonical_generator_orders_match_independent_relation_for_all_five_sets() {
        for algorithm in PARAMETER_ALGORITHMS {
            let params = params_for_algorithm(algorithm).unwrap().curve();
            match curve_for_algorithm(algorithm).unwrap() {
                CurveSelection::Bits256(curve) => assert_generator_order(curve, params),
                CurveSelection::Bits512(curve) => assert_generator_order(curve, params),
            }
        }
    }
}

#[path = "constant_time/verification.rs"]
mod verification;
pub(crate) use verification::KeyRejection;
pub(super) use verification::{validate_public_key, verify_bytes};
