use crate::ed25519::Fq;
use crate::ed25519::Fr;
use crate::{Coordinates, CurveAffine, CurveAffineExt, CurveExt};
use core::cmp;
use core::fmt::Debug;
use core::iter::Sum;
use core::ops::{Add, Mul, Neg, Sub};
use ff::{BatchInverter, Field, PrimeField};
use group::{self, Curve};
use group::{prime::PrimeCurveAffine, GroupEncoding};
use rand::RngCore;
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq, CtOption};

#[cfg(feature = "derive_serde")]
use serde::{Deserialize, Serialize};

const ED25519_GENERATOR_X: Fq = Fq::from_raw([
    0xc956_2d60_8f25_d51a,
    0x692c_c760_9525_a7b2,
    0xc0a4_e231_fdd6_dc5c,
    0x2169_36d3_cd6e_53fe,
]);
const ED25519_GENERATOR_Y: Fq = Fq::from_raw([
    0x6666_6666_6666_6658,
    0x6666_6666_6666_6666,
    0x6666_6666_6666_6666,
    0x6666_6666_6666_6666,
]);

// `d = -(121665/121666)`
const ED25519_D: Fq = Fq::from_raw([
    0x75eb_4dca_1359_78a3,
    0x0070_0a4d_4141_d8ab,
    0x8cc7_4079_7779_e898,
    0x5203_6cee_2b6f_fe73,
]);

const FR_MODULUS_BYTES: [u8; 32] = [
    237, 211, 245, 92, 26, 99, 18, 88, 214, 156, 247, 162, 222, 249, 222, 20, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 16,
];

use crate::{
    impl_add_binop_specify_output, impl_binops_additive, impl_binops_additive_specify_output,
    impl_binops_multiplicative, impl_binops_multiplicative_mixed, impl_sub_binop_specify_output,
};

#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
pub struct Ed25519 {
    pub x: Fq,
    pub y: Fq,
    pub z: Fq,
    pub t: Fq,
}

#[derive(Copy, Clone, Debug, PartialEq, Hash)]
#[cfg_attr(feature = "derive_serde", derive(Serialize, Deserialize))]
pub struct Ed25519Affine {
    pub x: Fq,
    pub y: Fq,
}

#[derive(Copy, Clone, Hash, Default)]
pub struct Ed25519Compressed([u8; 32]);

impl Ed25519 {
    /// Constructs an extended point from the neutral element `(0, 1)`.
    pub const fn identity() -> Self {
        Ed25519 {
            x: Fq::zero(),
            y: Fq::one(),
            z: Fq::one(),
            t: Fq::zero(),
        }
    }

    /// Determines if this point is the identity.
    pub fn is_identity(&self) -> Choice {
        // If this point is the identity, then
        //     u = 0 * z = 0
        // and v = 1 * z = z
        self.x.ct_eq(&Fq::zero()) & self.y.ct_eq(&self.z)
    }

    /// Determines if this point is torsion free and so is contained
    /// in the prime order subgroup.
    pub fn is_torsion_free(&self) -> Choice {
        self.multiply(&FR_MODULUS_BYTES).is_identity()
    }

    #[inline]
    fn multiply(&self, by: &[u8; 32]) -> Ed25519 {
        let zero = Ed25519::identity();
        let mut acc = Ed25519::identity();

        // This is a simple double-and-add implementation of point
        // multiplication, moving from most significant to least
        // significant bit of the scalar.
        //
        // We skip the leading three bits because they're always
        // unset for Fr.
        for bit in by
            .iter()
            .rev()
            .flat_map(|byte| (0..8).rev().map(move |i| Choice::from((byte >> i) & 1u8)))
            .skip(3)
        {
            acc = acc.double();
            acc += Ed25519::conditional_select(&zero, self, bit);
        }

        acc
    }

    /// Multiplies this element by the cofactor `8`.
    pub fn mul_by_cofactor(&self) -> Ed25519 {
        self.double().double().double()
    }

    pub fn generator() -> Self {
        let generator = Ed25519Affine::generator();
        Self {
            x: generator.x,
            y: generator.y,
            z: Fq::one(),
            t: generator.x * generator.y,
        }
    }

    pub fn double(&self) -> Ed25519 {
        //  A = X1^2
        //  B = Y1^2
        //  C = 2*Z1^2
        //  H = A+B
        //  E = H-(X1+Y1)^2
        //  G = A-B
        //  F = C+G
        //  X3 = E*F
        //  Y3 = G*H
        //  T3 = E*H
        //  Z3 = F*G

        let a = self.x.square();
        let b = self.y.square();
        let c = self.z.square().double();

        let h = a + b;
        let e = h - (self.x + self.y).square();
        let g = a - b;
        let f = c + g;

        Ed25519 {
            x: e * f,
            y: g * h,
            z: f * g,
            t: e * h,
        }
    }
}

impl Ed25519Affine {
    /// Constructs the neutral element `(0, 1)`.
    pub const fn identity() -> Self {
        Ed25519Affine {
            x: Fq::zero(),
            y: Fq::one(),
        }
    }

    /// Determines if this point is the identity.
    pub fn is_identity(&self) -> Choice {
        Ed25519::from(*self).is_identity()
    }

    pub fn generator() -> Self {
        Self {
            x: ED25519_GENERATOR_X,
            y: ED25519_GENERATOR_Y,
        }
    }

    pub fn to_extended(&self) -> Ed25519 {
        Ed25519 {
            x: self.x,
            y: self.y,
            z: Fq::one(),
            t: self.x * self.y,
        }
    }

    pub fn random(mut rng: impl RngCore) -> Self {
        loop {
            let y = Fq::random(&mut rng);
            let flip_sign = rng.next_u32() % 2 != 0;

            let y2 = y.square();
            let p = ((y2 - Fq::one())
                * ((Fq::one() + ED25519_D * y2).invert().unwrap_or(Fq::zero())))
            .sqrt()
            .map(|x| Ed25519Affine {
                x: if flip_sign { -x } else { x },
                y,
            });

            if p.is_some().into() {
                use crate::group::cofactor::CofactorGroup;
                let p = p.unwrap().to_curve();

                if bool::from(!p.is_identity()) {
                    return p.clear_cofactor().to_affine();
                }
            }
        }
    }

    /// Converts this element into its byte representation.
    pub fn to_bytes(&self) -> [u8; 32] {
        let mut tmp = self.y.to_bytes();
        let u = self.x.to_bytes();

        // Encode the sign of the u-coordinate in the most
        // significant bit.
        tmp[31] |= u[0] << 7;

        tmp
    }

    /// Attempts to interpret a byte representation of an
    /// affine point, failing if the element is not on
    /// the curve or non-canonical.
    pub fn from_bytes(b: [u8; 32]) -> CtOption<Self> {
        Self::from_bytes_inner(b, 1.into())
    }

    fn from_bytes_inner(mut b: [u8; 32], zip_216_enabled: Choice) -> CtOption<Self> {
        // Grab the sign bit from the representation
        let sign = b[31] >> 7;

        // Mask away the sign bit
        b[31] &= 0b0111_1111;

        // Interpret what remains as the v-coordinate
        Fq::from_bytes(&b).and_then(|v| {
            // -u^2 + v^2 = 1 + d.u^2.v^2
            // -u^2 = 1 + d.u^2.v^2 - v^2    (rearrange)
            // -u^2 - d.u^2.v^2 = 1 - v^2    (rearrange)
            // u^2 + d.u^2.v^2 = v^2 - 1     (flip signs)
            // u^2 (1 + d.v^2) = v^2 - 1     (factor)
            // u^2 = (v^2 - 1) / (1 + d.v^2) (isolate u^2)
            // We know that (1 + d.v^2) is nonzero for all v:
            //   (1 + d.v^2) = 0
            //   d.v^2 = -1
            //   v^2 = -(1 / d)   No solutions, as -(1 / d) is not a square

            let v2 = v.square();

            ((v2 - Fq::one()) * ((Fq::one() + ED25519_D * v2).invert().unwrap_or(Fq::zero())))
                .sqrt()
                .and_then(|u| {
                    // Fix the sign of `u` if necessary
                    let flip_sign = Choice::from((u.to_bytes()[0] ^ sign) & 1);
                    let u_negated = -u;
                    let final_u = Fq::conditional_select(&u, &u_negated, flip_sign);

                    // If u == 0, flip_sign == sign_bit. We therefore want to reject the
                    // encoding as non-canonical if all of the following occur:
                    // - ZIP 216 is enabled
                    // - u == 0
                    // - flip_sign == true
                    let u_is_zero = u.ct_eq(&Fq::zero());
                    CtOption::new(
                        Ed25519Affine { x: final_u, y: v },
                        !(zip_216_enabled & u_is_zero & flip_sign),
                    )
                })
        })
    }
}

// Compressed
impl std::fmt::Debug for Ed25519Compressed {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0[..].fmt(f)
    }
}

impl AsRef<[u8]> for Ed25519Compressed {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl AsMut<[u8]> for Ed25519Compressed {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

// Jacobian implementations
impl<'a> From<&'a Ed25519Affine> for Ed25519 {
    fn from(p: &'a Ed25519Affine) -> Ed25519 {
        p.to_curve()
    }
}

impl From<Ed25519Affine> for Ed25519 {
    fn from(p: Ed25519Affine) -> Ed25519 {
        p.to_curve()
    }
}

impl Default for Ed25519 {
    fn default() -> Ed25519 {
        Ed25519::identity()
    }
}

impl subtle::ConstantTimeEq for Ed25519 {
    fn ct_eq(&self, other: &Self) -> Choice {
        (self.x * other.z).ct_eq(&(other.x * self.z))
            & (self.y * other.z).ct_eq(&(other.y * self.z))
    }
}

impl subtle::ConditionallySelectable for Ed25519 {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        Ed25519 {
            x: Fq::conditional_select(&a.x, &b.x, choice),
            y: Fq::conditional_select(&a.y, &b.y, choice),
            z: Fq::conditional_select(&a.z, &b.z, choice),
            t: Fq::conditional_select(&a.t, &b.t, choice),
        }
    }
}

impl PartialEq for Ed25519 {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}

impl cmp::Eq for Ed25519 {}

impl CurveExt for Ed25519 {
    type ScalarExt = Fr;
    type Base = Fq;
    type AffineExt = Ed25519Affine;

    const CURVE_ID: &'static str = "ed25519";

    fn is_on_curve(&self) -> Choice {
        let affine = Ed25519Affine::from(*self);
        !self.z.is_zero() & affine.is_on_curve() & (affine.x * affine.y * self.z).ct_eq(&self.t)
    }

    fn endo(&self) -> Self {
        unimplemented!();
    }

    fn jacobian_coordinates(&self) -> (Fq, Fq, Fq) {
        unimplemented!();
    }

    fn hash_to_curve<'a>(_: &'a str) -> Box<dyn Fn(&[u8]) -> Self + 'a> {
        unimplemented!();
    }

    fn a() -> Self::Base {
        unimplemented!()
    }

    fn b() -> Self::Base {
        unimplemented!()
    }

    fn new_jacobian(_x: Self::Base, _y: Self::Base, _z: Self::Base) -> CtOption<Self> {
        unimplemented!();
    }
}

impl group::Curve for Ed25519 {
    type AffineRepr = Ed25519Affine;

    fn batch_normalize(p: &[Self], q: &mut [Self::AffineRepr]) {
        assert_eq!(p.len(), q.len());

        for (p, q) in p.iter().zip(q.iter_mut()) {
            // We use the `u` field of `AffinePoint` to store the z-coordinate being
            // inverted, and the `v` field for scratch space.
            q.x = p.z;
        }

        BatchInverter::invert_with_internal_scratch(q, |q| &mut q.x, |q| &mut q.y);

        for (p, q) in p.iter().zip(q.iter_mut()).rev() {
            let tmp = q.x;

            // Set the coordinates to the correct value
            q.x = p.x * tmp; // Multiply by 1/z
            q.y = p.y * tmp; // Multiply by 1/z
        }
    }

    fn to_affine(&self) -> Self::AffineRepr {
        // Z coordinate is always nonzero, so this is
        // its inverse.
        let zinv = self.z.invert().unwrap();

        Ed25519Affine {
            x: self.x * zinv,
            y: self.y * zinv,
        }
    }
}

impl group::Group for Ed25519 {
    type Scalar = Fr;

    fn random(mut rng: impl RngCore) -> Self {
        Ed25519Affine::random(&mut rng).to_curve()
    }

    fn generator() -> Self {
        Ed25519::generator()
    }

    fn identity() -> Self {
        Self::identity()
    }

    fn is_identity(&self) -> Choice {
        self.is_identity()
    }

    #[must_use]
    fn double(&self) -> Self {
        self.double()
    }
}

impl GroupEncoding for Ed25519 {
    type Repr = Ed25519Compressed;

    fn from_bytes(bytes: &Self::Repr) -> CtOption<Self> {
        Ed25519Affine::from_bytes(bytes.0).map(Self::from)
    }

    fn from_bytes_unchecked(bytes: &Self::Repr) -> CtOption<Self> {
        Ed25519Affine::from_bytes(bytes.0).map(Self::from)
    }

    fn to_bytes(&self) -> Self::Repr {
        Ed25519Compressed(Ed25519Affine::from(self).to_bytes())
    }
}

impl crate::serde::SerdeObject for Ed25519 {
    fn from_raw_bytes_unchecked(bytes: &[u8]) -> Self {
        debug_assert_eq!(bytes.len(), 4 * Fq::size());
        let [x, y, z, t] = [0, 1, 2, 3]
            .map(|i| Fq::from_raw_bytes_unchecked(&bytes[i * Fq::size()..(i + 1) * Fq::size()]));
        Self { x, y, z, t }
    }
    fn from_raw_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 4 * Fq::size() {
            return None;
        }
        let [x, y, z, t] =
            [0, 1, 2, 3].map(|i| Fq::from_raw_bytes(&bytes[i * Fq::size()..(i + 1) * Fq::size()]));
        x.zip(y).zip(z).zip(t).and_then(|(((x, y), z), t)| {
            let res = Self { x, y, z, t };
            // Check that the point is on the curve.
            bool::from(res.is_on_curve()).then_some(res)
        })
    }
    fn to_raw_bytes(&self) -> Vec<u8> {
        let mut res = Vec::with_capacity(4 * Fq::size());
        Self::write_raw(self, &mut res).unwrap();
        res
    }
    fn read_raw_unchecked<R: std::io::Read>(reader: &mut R) -> Self {
        let [x, y, z, t] = [(); 4].map(|_| Fq::read_raw_unchecked(reader));
        Self { x, y, z, t }
    }
    fn read_raw<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let x = Fq::read_raw(reader)?;
        let y = Fq::read_raw(reader)?;
        let z = Fq::read_raw(reader)?;
        let t = Fq::read_raw(reader)?;
        Ok(Self { x, y, z, t })
    }
    fn write_raw<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.x.write_raw(writer)?;
        self.y.write_raw(writer)?;
        self.z.write_raw(writer)?;
        self.t.write_raw(writer)
    }
}

impl group::prime::PrimeGroup for Ed25519 {}

impl group::prime::PrimeCurve for Ed25519 {
    type Affine = Ed25519Affine;
}

impl group::cofactor::CofactorCurve for Ed25519 {
    type Affine = Ed25519Affine;
}

impl group::cofactor::CofactorGroup for Ed25519 {
    type Subgroup = Ed25519;

    fn clear_cofactor(&self) -> Self {
        self.mul_by_cofactor()
    }

    fn into_subgroup(self) -> CtOption<Self::Subgroup> {
        CtOption::new(self, self.is_torsion_free())
    }

    fn is_torsion_free(&self) -> Choice {
        self.is_torsion_free()
    }
}

impl<'a> From<&'a Ed25519> for Ed25519Affine {
    fn from(p: &'a Ed25519) -> Ed25519Affine {
        p.to_affine()
    }
}

impl From<Ed25519> for Ed25519Affine {
    fn from(p: Ed25519) -> Ed25519Affine {
        p.to_affine()
    }
}

impl Default for Ed25519Affine {
    fn default() -> Ed25519Affine {
        Ed25519Affine::identity()
    }
}

impl subtle::ConstantTimeEq for Ed25519Affine {
    fn ct_eq(&self, other: &Self) -> Choice {
        self.x.ct_eq(&other.x) & self.y.ct_eq(&other.y)
    }
}

impl subtle::ConditionallySelectable for Ed25519Affine {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        Ed25519Affine {
            x: Fq::conditional_select(&a.x, &b.x, choice),
            y: Fq::conditional_select(&a.y, &b.y, choice),
        }
    }
}

impl cmp::Eq for Ed25519Affine {}

impl group::GroupEncoding for Ed25519Affine {
    type Repr = [u8; 32];

    fn from_bytes(bytes: &Self::Repr) -> CtOption<Self> {
        Self::from_bytes(*bytes)
    }

    fn from_bytes_unchecked(bytes: &Self::Repr) -> CtOption<Self> {
        Self::from_bytes(*bytes)
    }

    fn to_bytes(&self) -> Self::Repr {
        self.to_bytes()
    }
}

impl crate::serde::SerdeObject for Ed25519Affine {
    fn from_raw_bytes_unchecked(bytes: &[u8]) -> Self {
        debug_assert_eq!(bytes.len(), 2 * Fq::size());
        let [x, y] =
            [0, Fq::size()].map(|i| Fq::from_raw_bytes_unchecked(&bytes[i..i + Fq::size()]));
        Self { x, y }
    }
    fn from_raw_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != 2 * Fq::size() {
            return None;
        }
        let [x, y] = [0, Fq::size()].map(|i| Fq::from_raw_bytes(&bytes[i..i + Fq::size()]));
        x.zip(y).and_then(|(x, y)| {
            let res = Self { x, y };
            // Check that the point is on the curve.
            bool::from(res.is_on_curve()).then_some(res)
        })
    }
    fn to_raw_bytes(&self) -> Vec<u8> {
        let mut res = Vec::with_capacity(2 * Fq::size());
        Self::write_raw(self, &mut res).unwrap();
        res
    }
    fn read_raw_unchecked<R: std::io::Read>(reader: &mut R) -> Self {
        let [x, y] = [(); 2].map(|_| Fq::read_raw_unchecked(reader));
        Self { x, y }
    }
    fn read_raw<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let x = Fq::read_raw(reader)?;
        let y = Fq::read_raw(reader)?;
        Ok(Self { x, y })
    }
    fn write_raw<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        self.x.write_raw(writer)?;
        self.y.write_raw(writer)
    }
}

impl group::prime::PrimeCurveAffine for Ed25519Affine {
    type Curve = Ed25519;
    type Scalar = Fr;

    fn generator() -> Self {
        Ed25519Affine::generator()
    }

    fn identity() -> Self {
        Ed25519Affine::identity()
    }

    fn is_identity(&self) -> Choice {
        self.is_identity()
    }

    fn to_curve(&self) -> Self::Curve {
        Ed25519 {
            x: self.x,
            y: self.y,
            z: Fq::one(),
            t: self.x * self.y,
        }
    }
}

impl group::cofactor::CofactorCurveAffine for Ed25519Affine {
    type Curve = Ed25519;
    type Scalar = Fr;

    fn identity() -> Self {
        <Self as group::prime::PrimeCurveAffine>::identity()
    }

    fn generator() -> Self {
        <Self as group::prime::PrimeCurveAffine>::generator()
    }

    fn is_identity(&self) -> Choice {
        <Self as group::prime::PrimeCurveAffine>::is_identity(self)
    }

    fn to_curve(&self) -> Self::Curve {
        <Self as group::prime::PrimeCurveAffine>::to_curve(self)
    }
}

impl CurveAffine for Ed25519Affine {
    type ScalarExt = Fr;
    type Base = Fq;
    type CurveExt = Ed25519;

    fn is_on_curve(&self) -> Choice {
        let x2 = self.x.square();
        let y2 = self.y.square();

        (y2 - x2).ct_eq(&(Fq::one() + ED25519_D * x2 * y2))
    }

    fn coordinates(&self) -> CtOption<Coordinates<Self>> {
        Coordinates::from_xy(self.x, self.y)
    }

    fn from_xy(x: Self::Base, y: Self::Base) -> CtOption<Self> {
        let p = Ed25519Affine { x, y };
        CtOption::new(p, p.is_on_curve())
    }

    fn a() -> Self::Base {
        unimplemented!()
    }

    fn b() -> Self::Base {
        unimplemented!()
    }
}

impl_binops_additive!(Ed25519, Ed25519);
impl_binops_additive!(Ed25519, Ed25519Affine);
impl_binops_additive_specify_output!(Ed25519Affine, Ed25519Affine, Ed25519);
impl_binops_additive_specify_output!(Ed25519Affine, Ed25519, Ed25519);
impl_binops_multiplicative!(Ed25519, Fr);
impl_binops_multiplicative_mixed!(Ed25519Affine, Fr, Ed25519);

impl<'a> Neg for &'a Ed25519 {
    type Output = Ed25519;

    fn neg(self) -> Ed25519 {
        Ed25519 {
            x: -self.x,
            y: self.y,
            z: self.z,
            t: -self.t,
        }
    }
}

impl Neg for Ed25519 {
    type Output = Ed25519;

    fn neg(self) -> Ed25519 {
        -&self
    }
}

impl<T> Sum<T> for Ed25519
where
    T: core::borrow::Borrow<Ed25519>,
{
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = T>,
    {
        iter.fold(Self::identity(), |acc, item| acc + item.borrow())
    }
}

impl<'a, 'b> Add<&'a Ed25519> for &'b Ed25519 {
    type Output = Ed25519;

    fn add(self, rhs: &'a Ed25519) -> Ed25519 {
        // We perform addition in the extended coordinates. Here we use
        // a formula presented by Hisil, Wong, Carter and Dawson in
        // "Twisted Edward Curves Revisited" which only requires 8M.
        //
        // A = (V1 - U1) * (V2 - U2)
        // B = (V1 + U1) * (V2 + U2)
        // C = 2d * T1 * T2
        // D = 2 * Z1 * Z2
        // E = B - A
        // F = D - C
        // G = D + C
        // H = B + A
        // U3 = E * F
        // Y3 = G * H
        // Z3 = F * G
        // T3 = E * H

        let a = (self.x - self.y) * (rhs.x - rhs.y);
        let b = (self.x + self.y) * (rhs.x + rhs.y);
        let c = (self.t * rhs.t * ED25519_D).double();
        let d = (self.z * rhs.z).double();

        let e = b - a;
        let f = d - c;
        let g = d + c;
        let h = b + a;

        Ed25519 {
            x: e * f,
            y: g * h,
            z: f * g,
            t: e * h,
        }
    }
}

impl<'a, 'b> Add<&'a Ed25519Affine> for &'b Ed25519 {
    type Output = Ed25519;

    fn add(self, rhs: &'a Ed25519Affine) -> Ed25519 {
        self + rhs.to_extended()
    }
}

impl<'a, 'b> Sub<&'a Ed25519> for &'b Ed25519 {
    type Output = Ed25519;

    fn sub(self, other: &'a Ed25519) -> Ed25519 {
        self + (-other)
    }
}

impl<'a, 'b> Sub<&'a Ed25519Affine> for &'b Ed25519 {
    type Output = Ed25519;

    fn sub(self, other: &'a Ed25519Affine) -> Ed25519 {
        self + (-other)
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl<'a, 'b> Mul<&'b Fr> for &'a Ed25519 {
    type Output = Ed25519;

    // This is a simple double-and-add implementation of point
    // multiplication, moving from most significant to least
    // significant bit of the scalar.
    //
    // We skip the leading three bits because they're always
    // unset for Fr.
    fn mul(self, other: &'b Fr) -> Self::Output {
        let mut acc = Ed25519::identity();
        for bit in other
            .to_repr()
            .iter()
            .rev()
            .flat_map(|byte| (0..8).rev().map(move |i| Choice::from((byte >> i) & 1u8)))
            .skip(3)
        {
            acc = acc.double();
            acc = Ed25519::conditional_select(&acc, &(acc + self), bit);
        }

        acc
    }
}

impl<'a> Neg for &'a Ed25519Affine {
    type Output = Ed25519Affine;

    fn neg(self) -> Ed25519Affine {
        Ed25519Affine {
            x: -self.x,
            y: self.y,
        }
    }
}

impl Neg for Ed25519Affine {
    type Output = Ed25519Affine;

    fn neg(self) -> Ed25519Affine {
        -&self
    }
}

impl<'a, 'b> Add<&'a Ed25519> for &'b Ed25519Affine {
    type Output = Ed25519;

    fn add(self, rhs: &'a Ed25519) -> Ed25519 {
        rhs + self
    }
}

impl<'a, 'b> Add<&'a Ed25519Affine> for &'b Ed25519Affine {
    type Output = Ed25519;

    fn add(self, rhs: &'a Ed25519Affine) -> Ed25519 {
        self.to_extended() + rhs.to_extended()
    }
}

impl<'a, 'b> Sub<&'a Ed25519Affine> for &'b Ed25519Affine {
    type Output = Ed25519;

    fn sub(self, other: &'a Ed25519Affine) -> Ed25519 {
        self + (-other)
    }
}

impl<'a, 'b> Sub<&'a Ed25519> for &'b Ed25519Affine {
    type Output = Ed25519;

    fn sub(self, other: &'a Ed25519) -> Ed25519 {
        self + (-other)
    }
}

#[allow(clippy::suspicious_arithmetic_impl)]
impl<'a, 'b> Mul<&'b Fr> for &'a Ed25519Affine {
    type Output = Ed25519;

    fn mul(self, other: &'b Fr) -> Self::Output {
        let mut acc = Ed25519::identity();

        // This is a simple double-and-add implementation of point
        // multiplication, moving from most significant to least
        // significant bit of the scalar.
        //
        // We skip the leading three bits because they're always
        // unset for Fr.
        for bit in other
            .to_repr()
            .iter()
            .rev()
            .flat_map(|byte| (0..8).rev().map(move |i| Choice::from((byte >> i) & 1u8)))
        {
            acc = acc.double();
            acc = Ed25519::conditional_select(&acc, &(acc + self), bit);
        }

        acc
    }
}

impl CurveAffineExt for Ed25519Affine {
    fn into_coordinates(self) -> (Self::Base, Self::Base) {
        (self.x, self.y)
    }
}

pub trait TwistedEdwardsCurveExt: CurveExt {
    fn a() -> <Self as CurveExt>::Base;
    fn d() -> <Self as CurveExt>::Base;
}

impl TwistedEdwardsCurveExt for Ed25519 {
    fn a() -> Fq {
        -Fq::ONE
    }

    fn d() -> Fq {
        ED25519_D
    }
}

pub trait TwistedEdwardsCurveAffineExt: CurveAffineExt {
    fn a() -> <Self as CurveAffine>::Base;
    fn d() -> <Self as CurveAffine>::Base;
}

impl TwistedEdwardsCurveAffineExt for Ed25519Affine {
    fn a() -> Fq {
        -Fq::ONE
    }

    fn d() -> Fq {
        ED25519_D
    }
}

#[test]
fn test_is_on_curve() {
    assert!(bool::from(Ed25519Affine::identity().is_on_curve()));
}

#[test]
fn test_d_is_non_quadratic_residue() {
    assert!(bool::from(ED25519_D.sqrt().is_none()));
    assert!(bool::from((-ED25519_D).sqrt().is_none()));
    assert!(bool::from((-ED25519_D).invert().unwrap().sqrt().is_none()));
}

#[test]
fn test_double() {
    let p = Ed25519::generator();

    assert_eq!(p.double(), p + p);
}

#[test]
fn test_assoc() {
    let p = Ed25519::from(Ed25519Affine {
        x: Fq::from_raw([
            0x4eb5_31fa_487c_0f3e,
            0x1313_5118_1c90_b35e,
            0xdb9a_afaf_f32a_26f7,
            0x5e0c_b226_a2aa_bab4,
        ]),
        y: Fq::from_raw([
            0xbf09_6275_684b_b8c9,
            0xc7ba_2458_90af_256d,
            0x5911_9f3e_8638_0eb0,
            0x3793_de18_2f9f_b1d2,
        ]),
    })
    .mul_by_cofactor();
    assert!(bool::from(p.is_on_curve()));

    assert_eq!(
        (p * Fr::from(1000u64)) * Fr::from(3938u64),
        p * (Fr::from(1000u64) * Fr::from(3938u64)),
    );
}

#[test]
fn test_curve() {
    crate::tests::curve::curve_tests::<Ed25519>();
}

#[test]
fn test_serialization() {
    crate::tests::curve::random_serialization_test::<Ed25519>();
}

// #[test]
// #[allow(non_snake_case)]
// fn eddsa_example() {
//     use crate::group::cofactor::CofactorGroup;
//     use sha2::{Digest, Sha512};

//     fn hash_to_fr(hash: Sha512) -> Fr {
//         let mut output = [0u8; 64];
//         output.copy_from_slice(hash.finalize().as_slice());

//         Fr::from_bytes_wide(&output)
//     }

//     fn seed_to_key(seed: [u8; 32]) -> (Fr, [u8; 32], [u8; 32]) {
//         // Expand the seed to a 64-byte array with SHA512.
//         let h = Sha512::digest(&seed[..]);

//         // Convert the low half to a scalar with Ed25519 "clamping"
//         let s = {
//             let mut scalar_bytes = [0u8; 32];
//             scalar_bytes[..].copy_from_slice(&h.as_slice()[0..32]);
//             // Clear the lowest three bits to make the scalar a multiple of 8
//             scalar_bytes[0] &= 248;
//             // Clear highest bit
//             scalar_bytes[31] &= 127;
//             // Set second highest bit to 1
//             scalar_bytes[31] |= 64;

//             let mut scalar_bytes_wide = [0u8; 64];
//             scalar_bytes_wide[0..32].copy_from_slice(&scalar_bytes);

//             Fr::from_bytes_wide(&scalar_bytes_wide)
//         };

//         // Extract and cache the high half.
//         let prefix = {
//             let mut prefix = [0u8; 32];
//             prefix[..].copy_from_slice(&h.as_slice()[32..64]);
//             prefix
//         };

//         // Compute the public key as A = [s]B.
//         let A = Ed25519::generator() * &s;

//         let A_bytes = A.to_bytes().0;

//         (s, prefix, A_bytes)
//     }

//     fn sign(s: Fr, prefix: [u8; 32], A_bytes: [u8; 32], msg: &[u8]) -> ([u8; 32], [u8; 32]) {
//         let r = hash_to_fr(Sha512::default().chain(&prefix[..]).chain(msg));

//         let R_bytes = (Ed25519::generator() * &r).to_bytes().0;

//         let k = hash_to_fr(
//             Sha512::default()
//                 .chain(&R_bytes[..])
//                 .chain(&A_bytes[..])
//                 .chain(msg),
//         );

//         let s_bytes = (r + s * k).to_bytes();

//         (R_bytes, s_bytes)
//     }

//     fn verify(R_bytes: [u8; 32], s_bytes: [u8; 32], A_bytes: [u8; 32], msg: &[u8]) -> Choice {
//         let k = hash_to_fr(
//             Sha512::default()
//                 .chain(&R_bytes[..])
//                 .chain(&A_bytes[..])
//                 .chain(msg),
//         );
//         verify_prehashed(R_bytes, s_bytes, A_bytes, k)
//     }

//     fn verify_prehashed(R_bytes: [u8; 32], s_bytes: [u8; 32], A_bytes: [u8; 32], k: Fr) -> Choice {
//         // `R_bytes` MUST be an encoding of a point on the twisted Edwards form of Curve25519.
//         let R = Ed25519::from_bytes(&Ed25519Compressed(R_bytes)).unwrap();
//         // `s_bytes` MUST represent an integer less than the prime `l`.
//         let s = Fr::from_bytes(&s_bytes).unwrap();
//         // `A_bytes` MUST be an encoding of a point on the twisted Edwards form of Curve25519.
//         let A = Ed25519::from_bytes(&Ed25519Compressed(A_bytes)).unwrap();

//         //       [8][s]B = [8]R + [8][k]A
//         // <=>   [8]R = [8][s]B - [8][k]A
//         // <=>   0 = [8](R - ([s]B - [k]A))
//         // <=>   0 = [8](R - R')  where R' = [s]B - [k]A
//         let R_prime = Ed25519::from(Ed25519::generator()) * s - A * k;

//         (R - R_prime).clear_cofactor().is_identity()
//     }

//     use rand_core::OsRng;
//     let mut rng = OsRng;

//     for _ in 0..1000 {
//         // Generate a key pair
//         let mut seed = [0u8; 32];
//         rng.fill_bytes(&mut seed[..]);

//         let (s, prefix, A_bytes) = seed_to_key(seed);

//         // Generate a valid signature
//         // Suppose `m` is the message
//         let msg = b"test message";

//         let (R_bytes, s_bytes) = sign(s, prefix, A_bytes, msg);

//         // Verify the signature
//         assert!(bool::from(verify(R_bytes, s_bytes, A_bytes, msg)));
//     }
// }
