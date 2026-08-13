//! Canonical prime-field and curve-cycle primitives for FCMP++.
//!
//! The Helios/Selene formulas and constants are derived from the MIT-licensed
//! `helioselene` and `monero-fcmp-plus-plus` crates by Luke Parker.  They are
//! kept in-tree so validator consensus does not depend on an unpinned or
//! unavailable crate graph.  Arithmetic uses the constant-modulus bigint
//! implementation already re-exported by the pinned `p256` dependency.
use super::FcmpNativeErrorV1;
use curve25519_dalek::{
    edwards::{CompressedEdwardsY, EdwardsPoint},
    traits::Identity,
};
use p256::elliptic_curve::bigint::{
    CtChoice, Encoding, U256, impl_modulus,
    modular::constant_mod::{Residue, ResidueParams},
};
use p256::elliptic_curve::subtle::{Choice, ConditionallySelectable, ConstantTimeEq};
use sha3::{Digest as _, Keccak256};
use std::{
    ops::{Add, AddAssign, Mul, MulAssign, Neg, Sub, SubAssign},
    sync::OnceLock,
};
use zeroize::{Zeroize, Zeroizing};
/// Clears one function-owned secret slot on success, error, and unwind.
struct BorrowedZeroizingCopySlot<'a, T: Zeroize>(&'a mut T);
impl<T: Zeroize> BorrowedZeroizingCopySlot<'_, T> {
    fn as_ref(&self) -> &T {
        self.0
    }
}
impl<T: Zeroize> Drop for BorrowedZeroizingCopySlot<'_, T> {
    fn drop(&mut self) {
        self.0.zeroize();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut *self.0);
    }
}
struct SecretCopyValueV1<T: Copy + Zeroize>(T);
impl<T: Copy + Zeroize> SecretCopyValueV1<T> {
    fn new(mut value: T) -> Self {
        let incoming = BorrowedZeroizingCopySlot(&mut value);
        let owned = Self(*incoming.as_ref());
        drop(incoming);
        owned
    }
    fn as_ref(&self) -> &T {
        &self.0
    }
    fn take(value: &mut T) -> Self {
        let incoming = BorrowedZeroizingCopySlot(value);
        let owned = Self(*incoming.as_ref());
        drop(incoming);
        owned
    }
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
    fn expose_copy(&self) -> T {
        self.0
    }
}
impl<T: Copy + Zeroize> Drop for SecretCopyValueV1<T> {
    fn drop(&mut self) {
        self.0.zeroize();
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
        #[cfg(test)]
        let _ = SECRET_COPY_VALUE_DROPS_V1.try_with(|drops| {
            drops.set(drops.get().saturating_add(1));
        });
    }
}
#[cfg(test)]
std::thread_local! {
    static SECRET_COPY_VALUE_DROPS_V1: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}
/// Opaque owner for a private canonical scalar encoding.
pub(super) struct SecretEncodedScalarV1(SecretCopyValueV1<[u8; 32]>);
impl SecretEncodedScalarV1 {
    pub(super) fn as_ref(&self) -> &[u8; 32] {
        self.0.as_ref()
    }
    pub(super) fn as_mut(&mut self) -> &mut [u8; 32] {
        self.0.as_mut()
    }
    /// Publish one canonical encoding only at an explicitly reviewed output
    /// boundary. All private branch insertions borrow instead.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    pub(super) fn expose_public_copy_v1(&self) -> [u8; 32] {
        self.0.expose_copy()
    }
}
/// Move-only owner for a private coordinate in either cycle field.
pub(super) struct SecretCycleScalarV1<F: Copy + Zeroize>(SecretCopyValueV1<F>);
impl<F: Copy + Zeroize> SecretCycleScalarV1<F> {
    pub(super) fn as_ref(&self) -> &F {
        self.0.as_ref()
    }
    #[cfg(test)]
    pub(super) fn expose_ref(&self) -> &F {
        self.as_ref()
    }
}
/// Move-only owner for both private affine coordinates of a cycle point.
///
/// Coordinates may only be lent together to the final circuit-witness
/// insertion boundary. Both retained `Copy` slots are erased on every exit.
pub(super) struct SecretCycleCoordinatesV1<F: Copy + Zeroize> {
    x: SecretCopyValueV1<F>,
    y: SecretCopyValueV1<F>,
}
impl<F: Copy + Zeroize> SecretCycleCoordinatesV1<F> {
    pub(super) fn component_refs(&self) -> (&F, &F) {
        (self.x.as_ref(), self.y.as_ref())
    }
}
struct SecretU256V1(U256);
impl Drop for SecretU256V1 {
    fn drop(&mut self) {
        self.0 = U256::ZERO;
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        let _ = core::hint::black_box(&mut self.0);
        #[cfg(test)]
        let _ = SECRET_U256_DROPS_V1.try_with(|drops| {
            drops.set(drops.get().saturating_add(1));
        });
    }
}
#[cfg(test)]
std::thread_local! {
    static SECRET_U256_DROPS_V1: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}
impl_modulus!(
    Field25519Modulus,
    U256,
    "7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffed"
);
impl_modulus!(
    HelioseleneModulus,
    U256,
    "7fffffffffffffffffffffffffffffffbf7f782cb7656b586eb6d2727927c79f"
);
type Field25519Residue = Residue<Field25519Modulus, { Field25519Modulus::LIMBS }>;
type HelioseleneResidue = Residue<HelioseleneModulus, { HelioseleneModulus::LIMBS }>;
macro_rules! define_local_field {
    ($name:ident, $residue:ty) => {
        /// Local transparent field boundary used by the reusable proof backend.
        ///
        /// Keeping the newtype local makes its cryptographic trait
        /// implementations coherent while every operation continues to
        /// delegate to the same constant-modulus residue arithmetic.
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub(super) struct $name($residue);
        impl $name {
            pub(super) const ZERO: Self = Self(<$residue>::ZERO);
            pub(super) const ONE: Self = Self(<$residue>::ONE);
            pub(super) const fn new(value: &U256) -> Self {
                Self(<$residue>::new(value))
            }
            pub(super) const fn retrieve(&self) -> U256 {
                self.0.retrieve()
            }
            pub(super) const fn square(&self) -> Self {
                Self(self.0.square())
            }
            pub(super) fn mul_ref(&self, rhs: &Self) -> Self {
                Self(self.0 * rhs.0)
            }
            #[cfg_attr(
                not(test),
                allow(dead_code, reason = "secret-field reference seam is test-constrained")
            )]
            pub(super) fn add_ref(&self, rhs: &Self) -> Self {
                Self(self.0 + rhs.0)
            }
            pub(super) fn sub_ref(&self, rhs: &Self) -> Self {
                Self(self.0 - rhs.0)
            }
            pub(super) fn neg_ref(&self) -> Self {
                Self(-self.0)
            }
            #[cfg_attr(
                not(test),
                allow(dead_code, reason = "secret-field parity seam is test-constrained")
            )]
            pub(super) fn is_odd_ref(&self) -> bool {
                self.retrieve().to_le_bytes()[0] & 1 == 1
            }
            #[cfg_attr(
                not(test),
                allow(dead_code, reason = "secret-field equality seam is test-constrained")
            )]
            pub(super) fn eq_ref(&self, rhs: &Self) -> bool {
                self.0 == rhs.0
            }
            pub(super) const fn pow(&self, exponent: &U256) -> Self {
                Self(self.0.pow(exponent))
            }
            pub(super) const fn invert(&self) -> (Self, CtChoice) {
                let (inverse, is_some) = self.0.invert();
                (Self(inverse), is_some)
            }
            pub(super) fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
                Self(<$residue>::conditional_select(&a.0, &b.0, choice))
            }
            pub(super) fn ct_is_zero(&self) -> Choice {
                self.0.ct_eq(&<$residue>::ZERO)
            }
        }
        impl Add for $name {
            type Output = Self;
            fn add(self, rhs: Self) -> Self::Output {
                Self(self.0 + rhs.0)
            }
        }
        impl AddAssign for $name {
            fn add_assign(&mut self, rhs: Self) {
                self.0 += rhs.0;
            }
        }
        impl Sub for $name {
            type Output = Self;
            fn sub(self, rhs: Self) -> Self::Output {
                Self(self.0 - rhs.0)
            }
        }
        impl SubAssign for $name {
            fn sub_assign(&mut self, rhs: Self) {
                self.0 -= rhs.0;
            }
        }
        impl Mul for $name {
            type Output = Self;
            fn mul(self, rhs: Self) -> Self::Output {
                Self(self.0 * rhs.0)
            }
        }
        impl MulAssign for $name {
            fn mul_assign(&mut self, rhs: Self) {
                self.0 *= rhs.0;
            }
        }
        impl Neg for $name {
            type Output = Self;
            fn neg(self) -> Self::Output {
                Self(-self.0)
            }
        }
        impl Zeroize for $name {
            fn zeroize(&mut self) {
                *self = Self::ZERO;
                core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
                let _ = core::hint::black_box(&mut *self);
            }
        }
    };
}
define_local_field!(Field25519, Field25519Residue);
define_local_field!(HelioseleneField, HelioseleneResidue);
const FIELD25519_MODULUS: U256 =
    U256::from_be_hex("7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffed");
const HELIOSELENE_MODULUS: U256 =
    U256::from_be_hex("7fffffffffffffffffffffffffffffffbf7f782cb7656b586eb6d2727927c79f");
const FIELD25519_SQRT_EXPONENT: U256 =
    U256::from_be_hex("0ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe");
const HELIOSELENE_SQRT_EXPONENT: U256 =
    U256::from_be_hex("1fffffffffffffffffffffffffffffffefdfde0b2dd95ad61badb49c9e49f1e8");
const FIELD25519_SQRT_M1: U256 =
    U256::from_be_hex("2b8324804fc1df0b2b4d00993dfbd7a72f431806ad2fe478c4ee1b274a0ea0b0");
const HELIOS_B: U256 =
    U256::from_be_hex("22e8c739b0ea70b8be94a76b3ebb7b3b043f6f384113bf3522b49ee1edd73ad4");
const SELENE_B: U256 =
    U256::from_be_hex("70127713695876c17f51bba595ffe279f3944bdf06ae900e68de0983cb5a4558");
const SELENE_HASH_INITIALIZER_DOMAIN: &[u8] = b"Monero Selene Hash Initializer";
const HELIOS_HASH_INITIALIZER_DOMAIN: &[u8] = b"Monero Helios Hash Initializer";
const SELENE_GENERATOR_DOMAIN: &[u8] = b"Monero Selene G ";
const HELIOS_GENERATOR_DOMAIN: &[u8] = b"Monero Helios G ";
pub(super) const SELENE_GENERATOR_COUNT_V1: usize = 6 * super::FCMP_LAYER_ONE_LEN_V1;
pub(super) const HELIOS_GENERATOR_COUNT_V1: usize = super::FCMP_LAYER_TWO_LEN_V1;
pub(super) fn field25519_from_u64(value: u64) -> Field25519 {
    Field25519::new(&U256::from(value))
}
pub(super) fn field25519_is_zero(value: Field25519) -> bool {
    value.retrieve() == U256::ZERO
}
pub(super) fn helioselene_is_zero(value: HelioseleneField) -> bool {
    value.retrieve() == U256::ZERO
}
pub(super) fn field25519_is_odd(value: Field25519) -> bool {
    value.retrieve().to_le_bytes()[0] & 1 == 1
}
pub(super) fn helioselene_is_odd(value: HelioseleneField) -> bool {
    value.retrieve().to_le_bytes()[0] & 1 == 1
}
pub(super) fn decode_field25519(bytes: [u8; 32]) -> Option<Field25519> {
    let integer = U256::from_le_bytes(bytes);
    (integer < FIELD25519_MODULUS).then(|| Field25519::new(&integer))
}
pub(super) fn decode_helioselene(bytes: [u8; 32]) -> Option<HelioseleneField> {
    let integer = U256::from_le_bytes(bytes);
    (integer < HELIOSELENE_MODULUS).then(|| HelioseleneField::new(&integer))
}
pub(super) fn encode_field25519(value: Field25519) -> [u8; 32] {
    value.retrieve().to_le_bytes()
}
pub(super) fn encode_helioselene(value: HelioseleneField) -> [u8; 32] {
    value.retrieve().to_le_bytes()
}
/// Encode a private field element while keeping the retrieved integer and
/// encoded bytes in erasing owners. The returned bytes remain private until
/// the caller deliberately installs them in its final witness owner.
pub(super) fn encode_secret_field25519_scalar_v1(value: &Field25519) -> SecretEncodedScalarV1 {
    let integer = SecretU256V1(value.retrieve());
    SecretEncodedScalarV1(SecretCopyValueV1::new(integer.0.to_le_bytes()))
}
/// Secret-owned counterpart of [`encode_helioselene`].
pub(super) fn encode_secret_helioselene_scalar_v1(
    value: &HelioseleneField,
) -> SecretEncodedScalarV1 {
    let integer = SecretU256V1(value.retrieve());
    SecretEncodedScalarV1(SecretCopyValueV1::new(integer.0.to_le_bytes()))
}
pub(super) fn invert_field25519(value: Field25519) -> Option<Field25519> {
    let (inverse, is_some) = value.invert();
    bool::from(is_some).then_some(inverse)
}
pub(super) fn invert_helioselene(value: HelioseleneField) -> Option<HelioseleneField> {
    let (inverse, is_some) = value.invert();
    bool::from(is_some).then_some(inverse)
}
pub(super) fn sqrt_field25519(value: Field25519) -> Option<Field25519> {
    let first = value.pow(&FIELD25519_SQRT_EXPONENT);
    let candidate = if first.square() == value {
        first
    } else {
        first * Field25519::new(&FIELD25519_SQRT_M1)
    };
    (candidate.square() == value).then_some(candidate)
}
pub(super) fn sqrt_helioselene(value: HelioseleneField) -> Option<HelioseleneField> {
    let candidate = value.pow(&HELIOSELENE_SQRT_EXPONENT);
    (candidate.square() == value).then_some(candidate)
}
macro_rules! define_cycle_point {
    (
        $name:ident,
        $field:ty,
        $scalar:ty,
        $decode_field:ident,
        $encode_field:ident,
        $encode_scalar:ident,
        $sqrt_field:ident,
        $is_zero:ident,
        $is_odd:ident,
        $b:ident
    ) => {
        #[derive(Clone, Copy, Debug)]
        pub(super) struct $name {
            x: $field,
            y: $field,
            z: $field,
        }
        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                (self.is_identity() && other.is_identity())
                    || ((self.x * other.z == other.x * self.z)
                        && (self.y * other.z == other.y * self.z))
            }
        }
        impl Eq for $name {}
        impl Zeroize for $name {
            fn zeroize(&mut self) {
                self.x.zeroize();
                self.y.zeroize();
                self.z.zeroize();
            }
        }
        impl $name {
            pub(super) fn identity() -> Self {
                Self {
                    x: <$field>::ZERO,
                    y: <$field>::ONE,
                    z: <$field>::ZERO,
                }
            }
            pub(super) fn is_identity(self) -> bool {
                bool::from(self.x.ct_is_zero())
            }
            pub(super) fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
                Self {
                    x: <$field>::conditional_select(&a.x, &b.x, choice),
                    y: <$field>::conditional_select(&a.y, &b.y, choice),
                    z: <$field>::conditional_select(&a.z, &b.z, choice),
                }
            }
            pub(super) fn clear_secret(&mut self) {
                self.zeroize();
                core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
                let _ = core::hint::black_box(&mut *self);
            }
            pub(super) fn decode(
                mut bytes: [u8; 32],
                allow_identity: bool,
            ) -> Result<Self, FcmpNativeErrorV1> {
                let sign = bytes[31] >> 7;
                bytes[31] &= 0x7f;
                let x = $decode_field(bytes).ok_or(FcmpNativeErrorV1::CyclePointEncoding)?;
                if $is_zero(x) {
                    if sign != 0 || !allow_identity {
                        return Err(FcmpNativeErrorV1::CyclePointIdentity);
                    }
                    return Ok(Self::identity());
                }
                let three_x = x + x + x;
                let rhs = (x.square() * x) - three_x + <$field>::new(&$b);
                let mut y = $sqrt_field(rhs).ok_or(FcmpNativeErrorV1::CyclePointEncoding)?;
                if $is_odd(y) != (sign == 1) {
                    y = -y;
                }
                let point = Self {
                    x,
                    y,
                    z: <$field>::ONE,
                };
                if point.encode() != {
                    let mut canonical = bytes;
                    canonical[31] |= sign << 7;
                    canonical
                } {
                    return Err(FcmpNativeErrorV1::CyclePointEncoding);
                }
                Ok(point)
            }
            pub(super) fn encode(self) -> [u8; 32] {
                if self.is_identity() {
                    return [0; 32];
                }
                let (x, y) = self
                    .coordinates()
                    .expect("non-identity projective point has an affine representation");
                let mut encoded = $encode_field(x);
                encoded[31] |= u8::from($is_odd(y)) << 7;
                encoded
            }
            pub(super) fn coordinates(self) -> Option<($field, $field)> {
                let (inverse, is_some) = self.z.invert();
                if !bool::from(is_some) {
                    return None;
                }
                Some((self.x * inverse, self.y * inverse))
            }
            /// Convert a secret-derived owned projective point to the two
            /// intentional witness coordinates while erasing both the point
            /// slot and its projective inverse on every exit path.
            pub(super) fn secret_coordinates_v1(
                mut self,
            ) -> Option<SecretCycleCoordinatesV1<$field>> {
                let point = BorrowedZeroizingCopySlot(&mut self);
                let (mut inverse, is_some) = point.as_ref().z.invert();
                let inverse = BorrowedZeroizingCopySlot(&mut inverse);
                if !bool::from(is_some) {
                    return None;
                }
                let coordinates = SecretCycleCoordinatesV1 {
                    x: SecretCopyValueV1::new(point.as_ref().x.mul_ref(inverse.as_ref())),
                    y: SecretCopyValueV1::new(point.as_ref().y.mul_ref(inverse.as_ref())),
                };
                drop(inverse);
                drop(point);
                Some(coordinates)
            }
            /// Extract a private affine x-coordinate without returning it
            /// through a raw Copy slot. The projective point and inverse are
            /// erased before this move-only coordinate owner leaves.
            pub(super) fn secret_x_v1(mut self) -> Option<SecretCycleScalarV1<$field>> {
                let point = BorrowedZeroizingCopySlot(&mut self);
                let (mut inverse, is_some) = point.as_ref().z.invert();
                let inverse = BorrowedZeroizingCopySlot(&mut inverse);
                if !bool::from(is_some) {
                    return None;
                }
                let x = SecretCycleScalarV1(SecretCopyValueV1::new(
                    point.as_ref().x.mul_ref(inverse.as_ref()),
                ));
                drop(inverse);
                drop(point);
                Some(x)
            }
            /// Encode a private projective point into a move-only owner while
            /// erasing projective, inverse, affine, and integer scratch.
            pub(super) fn secret_encode_v1(mut self) -> Option<SecretEncodedScalarV1> {
                let point = BorrowedZeroizingCopySlot(&mut self);
                let (mut inverse, is_some) = point.as_ref().z.invert();
                let inverse = BorrowedZeroizingCopySlot(&mut inverse);
                if !bool::from(is_some) {
                    return None;
                }
                let x = SecretCopyValueV1::new(point.as_ref().x.mul_ref(inverse.as_ref()));
                let y = SecretCopyValueV1::new(point.as_ref().y.mul_ref(inverse.as_ref()));
                let integer = SecretU256V1(x.as_ref().retrieve());
                let mut encoded =
                    SecretEncodedScalarV1(SecretCopyValueV1::new(integer.0.to_le_bytes()));
                let y_integer = SecretU256V1(y.as_ref().retrieve());
                let y_bytes = SecretCopyValueV1::new(y_integer.0.to_le_bytes());
                encoded.as_mut()[31] |= (y_bytes.as_ref()[0] & 1) << 7;
                drop(inverse);
                drop(point);
                Some(encoded)
            }
            pub(super) fn x(self) -> Option<$field> {
                self.coordinates().map(|(x, _)| x)
            }
            pub(super) fn add(self, other: Self) -> Self {
                // Renes-Costello-Batina complete addition, add-2015-rcb-3,
                // specialized to short Weierstrass a = -3.
                let t0 = self.x * other.x;
                let t1 = self.y * other.y;
                let t2 = self.z * other.z;
                let t3 = self.x + self.y;
                let t4 = other.x + other.y;
                let t3 = t3 * t4;
                let t4 = t0 + t1;
                let t3 = t3 - t4;
                let t4 = self.y + self.z;
                let x3 = other.y + other.z;
                let t4 = t4 * x3;
                let x3 = t1 + t2;
                let t4 = t4 - x3;
                let x3 = self.x + self.z;
                let y3 = other.x + other.z;
                let x3 = x3 * y3;
                let y3 = t0 + t2;
                let y3 = x3 - y3;
                let z3 = <$field>::new(&$b) * t2;
                let x3 = y3 - z3;
                let z3 = x3 + x3;
                let x3 = x3 + z3;
                let z3 = t1 - x3;
                let x3 = t1 + x3;
                let y3 = <$field>::new(&$b) * y3;
                let t1 = t2 + t2;
                let t2 = t1 + t2;
                let y3 = y3 - t2;
                let y3 = y3 - t0;
                let t1 = y3 + y3;
                let y3 = t1 + y3;
                let t1 = t0 + t0;
                let t0 = t1 + t0;
                let t0 = t0 - t2;
                let t1 = t4 * y3;
                let t2 = t0 * y3;
                let y3 = (x3 * z3) + t2;
                let x3 = (t3 * x3) - t1;
                let z3 = (t4 * z3) + (t3 * t0);
                Self {
                    x: x3,
                    y: y3,
                    z: z3,
                }
            }
            pub(super) fn double(self) -> Self {
                // Bernstein-Lange dbl-2007-bl-2, specialized to a = -3.
                let w_base = (self.x - self.z) * (self.x + self.z);
                let w = w_base + w_base + w_base;
                let s = (self.y * self.z) + (self.y * self.z);
                let ss = s.square();
                let sss = s * ss;
                let r = self.y * s;
                let rr = r.square();
                let b_twice = (self.x * r) + (self.x * r);
                let h = w.square() - b_twice - b_twice;
                let doubled = Self {
                    x: h * s,
                    y: w * (b_twice - h) - rr - rr,
                    z: sss,
                };
                Self::conditional_select(&doubled, &Self::identity(), self.x.ct_is_zero())
            }
            pub(super) fn negate(self) -> Self {
                let negated = Self {
                    x: self.x,
                    y: -self.y,
                    z: self.z,
                };
                Self::conditional_select(&negated, &Self::identity(), self.x.ct_is_zero())
            }
            pub(super) fn mul(self, scalar: $scalar) -> Self {
                let scalar = Zeroizing::new(scalar);
                let bytes = Zeroizing::new($encode_scalar(*scalar));
                let mut result = Self::identity();
                for bit in (0..256).rev() {
                    let doubled = result.double();
                    let added = doubled.add(self);
                    let choice = Choice::from((bytes[bit / 8] >> (bit % 8)) & 1);
                    result = Self::conditional_select(&doubled, &added, choice);
                }
                result
            }
        }
    };
}
define_cycle_point!(
    HeliosPoint,
    Field25519,
    HelioseleneField,
    decode_field25519,
    encode_field25519,
    encode_helioselene,
    sqrt_field25519,
    field25519_is_zero,
    field25519_is_odd,
    HELIOS_B
);
define_cycle_point!(
    SelenePoint,
    HelioseleneField,
    Field25519,
    decode_helioselene,
    encode_helioselene,
    encode_field25519,
    sqrt_helioselene,
    helioselene_is_zero,
    helioselene_is_odd,
    SELENE_B
);
pub(super) fn decode_edwards_point(
    bytes: [u8; 32],
    allow_identity: bool,
) -> Result<EdwardsPoint, FcmpNativeErrorV1> {
    let point = CompressedEdwardsY(bytes)
        .decompress()
        .ok_or(FcmpNativeErrorV1::EdwardsPointEncoding)?;
    if point.compress().to_bytes() != bytes || !point.is_torsion_free() {
        return Err(FcmpNativeErrorV1::EdwardsPointEncoding);
    }
    if !allow_identity && point == EdwardsPoint::identity() {
        return Err(FcmpNativeErrorV1::EdwardsPointIdentity);
    }
    Ok(point)
}
pub(super) fn edwards_to_wei25519(
    bytes: [u8; 32],
) -> Result<(Field25519, Field25519), FcmpNativeErrorV1> {
    decode_edwards_point(bytes, false)?;
    let x_sign = bytes[31] >> 7;
    let mut y_bytes = bytes;
    y_bytes[31] &= 0x7f;
    let y = decode_field25519(y_bytes).ok_or(FcmpNativeErrorV1::EdwardsPointEncoding)?;
    let y_squared = y.square();
    let d = -field25519_from_u64(121_665)
        * invert_field25519(field25519_from_u64(121_666))
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    let denominator = (d * y_squared) + Field25519::ONE;
    let mut x = sqrt_field25519(
        (y_squared - Field25519::ONE)
            * invert_field25519(denominator).ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
    )
    .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    if field25519_is_odd(x) != (x_sign == 1) {
        x = -x;
    }
    let y_plus_one = Field25519::ONE + y;
    let one_minus_y = Field25519::ONE - y;
    let wei_x = (y_plus_one
        * invert_field25519(one_minus_y).ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?)
        + (field25519_from_u64(486_662)
            * invert_field25519(field25519_from_u64(3))
                .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?);
    let c = sqrt_field25519(-field25519_from_u64(486_664))
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    let wei_y = c
        * y_plus_one
        * invert_field25519(one_minus_y * x).ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    Ok((wei_x, wei_y))
}
fn secret_decode_field25519_v1(bytes: &[u8; 32]) -> Option<Field25519> {
    let integer = SecretU256V1(U256::from_le_slice(bytes));
    (integer.0 < FIELD25519_MODULUS).then(|| Field25519::new(&integer.0))
}
fn secret_invert_field25519_v1(value: &Field25519) -> Option<SecretCopyValueV1<Field25519>> {
    let (mut inverse, is_some) = value.invert();
    let inverse = SecretCopyValueV1::take(&mut inverse);
    bool::from(is_some).then_some(inverse)
}
fn secret_sqrt_field25519_v1(value: &Field25519) -> Option<SecretCopyValueV1<Field25519>> {
    let first = SecretCopyValueV1::new(value.pow(&FIELD25519_SQRT_EXPONENT));
    let candidate = if first.as_ref().square().eq_ref(value) {
        first
    } else {
        SecretCopyValueV1::new(
            first
                .as_ref()
                .mul_ref(&Field25519::new(&FIELD25519_SQRT_M1)),
        )
    };
    candidate
        .as_ref()
        .square()
        .eq_ref(value)
        .then_some(candidate)
}
/// Secret-safe Edwards-to-Weierstrass conversion for prover blind points.
/// Every named compressed-point, point, field, inverse, and coordinate slot is
/// owned until the final intentional coordinate tuple is returned.
pub(super) fn secret_edwards_to_wei25519_v1(
    bytes: &[u8; 32],
) -> Result<(Field25519, Field25519), FcmpNativeErrorV1> {
    let compressed = SecretCopyValueV1::new(CompressedEdwardsY(*bytes));
    let point = SecretCopyValueV1::new(
        compressed
            .as_ref()
            .decompress()
            .ok_or(FcmpNativeErrorV1::EdwardsPointEncoding)?,
    );
    let recompressed = SecretCopyValueV1::new(point.as_ref().compress());
    if recompressed.as_ref().as_bytes() != bytes || !point.as_ref().is_torsion_free() {
        return Err(FcmpNativeErrorV1::EdwardsPointEncoding);
    }
    if *point.as_ref() == EdwardsPoint::identity() {
        return Err(FcmpNativeErrorV1::EdwardsPointIdentity);
    }
    let x_sign = SecretCopyValueV1::new(bytes[31] >> 7);
    let mut y_bytes = SecretCopyValueV1::new(*bytes);
    y_bytes.as_mut()[31] &= 0x7f;
    let y = SecretCopyValueV1::new(
        secret_decode_field25519_v1(y_bytes.as_ref())
            .ok_or(FcmpNativeErrorV1::EdwardsPointEncoding)?,
    );
    let y_squared = SecretCopyValueV1::new(y.as_ref().square());
    let d = -field25519_from_u64(121_665)
        * invert_field25519(field25519_from_u64(121_666))
            .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    let denominator =
        SecretCopyValueV1::new(d.mul_ref(y_squared.as_ref()).add_ref(&Field25519::ONE));
    let denominator_inverse = secret_invert_field25519_v1(denominator.as_ref())
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    let x_argument = SecretCopyValueV1::new(
        y_squared
            .as_ref()
            .sub_ref(&Field25519::ONE)
            .mul_ref(denominator_inverse.as_ref()),
    );
    let mut x = secret_sqrt_field25519_v1(x_argument.as_ref())
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    if x.as_ref().is_odd_ref() != (x_sign.as_ref() == &1) {
        *x.as_mut() = x.as_ref().neg_ref();
    }
    let y_plus_one = SecretCopyValueV1::new(Field25519::ONE.add_ref(y.as_ref()));
    let one_minus_y = SecretCopyValueV1::new(Field25519::ONE.sub_ref(y.as_ref()));
    let one_minus_y_inverse = secret_invert_field25519_v1(one_minus_y.as_ref())
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    let wei_x = SecretCopyValueV1::new(
        y_plus_one
            .as_ref()
            .mul_ref(one_minus_y_inverse.as_ref())
            .add_ref(
                &field25519_from_u64(486_662).mul_ref(
                    &invert_field25519(field25519_from_u64(3))
                        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?,
                ),
            ),
    );
    let c = sqrt_field25519(-field25519_from_u64(486_664))
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    let wei_y_denominator = SecretCopyValueV1::new(one_minus_y.as_ref().mul_ref(x.as_ref()));
    let wei_y_inverse = secret_invert_field25519_v1(wei_y_denominator.as_ref())
        .ok_or(FcmpNativeErrorV1::ArithmeticInvariant)?;
    let wei_y = SecretCopyValueV1::new(
        c.mul_ref(y_plus_one.as_ref())
            .mul_ref(wei_y_inverse.as_ref()),
    );
    Ok((wei_x.expose_copy(), wei_y.expose_copy()))
}
pub(super) fn monero_varint(mut value: u32) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(5);
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        encoded.push(byte);
        if value == 0 {
            return encoded;
        }
    }
}
fn keccak256(bytes: &[u8]) -> [u8; 32] {
    Keccak256::digest(bytes).into()
}
fn hash_to_selene(mut bytes: [u8; 32]) -> SelenePoint {
    loop {
        if let Ok(point) = SelenePoint::decode(bytes, true) {
            return point;
        }
        bytes = keccak256(&bytes);
    }
}
fn hash_to_helios(mut bytes: [u8; 32]) -> HeliosPoint {
    loop {
        if let Ok(point) = HeliosPoint::decode(bytes, true) {
            return point;
        }
        bytes = keccak256(&bytes);
    }
}
pub(super) fn hash_bytes_to_selene(bytes: &[u8]) -> SelenePoint {
    hash_to_selene(keccak256(bytes))
}
pub(super) fn hash_bytes_to_helios(bytes: &[u8]) -> HeliosPoint {
    hash_to_helios(keccak256(bytes))
}
static SELENE_HASH_INITIALIZER: OnceLock<SelenePoint> = OnceLock::new();
static HELIOS_HASH_INITIALIZER: OnceLock<HeliosPoint> = OnceLock::new();
static SELENE_GENERATORS: OnceLock<Vec<SelenePoint>> = OnceLock::new();
static HELIOS_GENERATORS: OnceLock<Vec<HeliosPoint>> = OnceLock::new();
pub(super) fn selene_hash_initializer() -> SelenePoint {
    *SELENE_HASH_INITIALIZER.get_or_init(|| hash_bytes_to_selene(SELENE_HASH_INITIALIZER_DOMAIN))
}
pub(super) fn helios_hash_initializer() -> HeliosPoint {
    *HELIOS_HASH_INITIALIZER.get_or_init(|| hash_bytes_to_helios(HELIOS_HASH_INITIALIZER_DOMAIN))
}
pub(super) fn selene_generators() -> &'static [SelenePoint] {
    SELENE_GENERATORS.get_or_init(|| {
        (0..SELENE_GENERATOR_COUNT_V1)
            .map(|index| {
                let mut domain = SELENE_GENERATOR_DOMAIN.to_vec();
                domain.extend(monero_varint(
                    u32::try_from(index).expect("compiled generator count fits u32"),
                ));
                hash_bytes_to_selene(&domain)
            })
            .collect()
    })
}
pub(super) fn helios_generators() -> &'static [HeliosPoint] {
    HELIOS_GENERATORS.get_or_init(|| {
        (0..HELIOS_GENERATOR_COUNT_V1)
            .map(|index| {
                let mut domain = HELIOS_GENERATOR_DOMAIN.to_vec();
                domain.extend(monero_varint(
                    u32::try_from(index).expect("compiled generator count fits u32"),
                ));
                hash_bytes_to_helios(&domain)
            })
            .collect()
    })
}
pub(super) fn hash_selene(values: &[Field25519]) -> Result<SelenePoint, FcmpNativeErrorV1> {
    if values.is_empty() || values.len() > SELENE_GENERATOR_COUNT_V1 {
        return Err(FcmpNativeErrorV1::BranchWidth);
    }
    Ok(values
        .iter()
        .zip(selene_generators())
        .fold(selene_hash_initializer(), |hash, (scalar, generator)| {
            hash.add(generator.mul(*scalar))
        }))
}
pub(super) fn hash_helios(values: &[HelioseleneField]) -> Result<HeliosPoint, FcmpNativeErrorV1> {
    if values.is_empty() || values.len() > HELIOS_GENERATOR_COUNT_V1 {
        return Err(FcmpNativeErrorV1::BranchWidth);
    }
    Ok(values
        .iter()
        .zip(helios_generators())
        .fold(helios_hash_initializer(), |hash, (scalar, generator)| {
            hash.add(generator.mul(*scalar))
        }))
}
pub(super) fn encode_field25519_scalar(value: Field25519) -> [u8; 32] {
    encode_field25519(value)
}
pub(super) fn encode_helioselene_scalar(value: HelioseleneField) -> [u8; 32] {
    encode_helioselene(value)
}
pub(super) fn decode_field25519_scalar(bytes: [u8; 32]) -> Result<Field25519, FcmpNativeErrorV1> {
    decode_field25519(bytes).ok_or(FcmpNativeErrorV1::ScalarEncoding)
}
pub(super) fn decode_helioselene_scalar(
    bytes: [u8; 32],
) -> Result<HelioseleneField, FcmpNativeErrorV1> {
    decode_helioselene(bytes).ok_or(FcmpNativeErrorV1::ScalarEncoding)
}
/// Decode a private Field25519 scalar without creating a by-value encoded
/// input slot or leaving the decoded integer outside an erasing owner.
pub(super) fn decode_secret_field25519_scalar_v1(
    bytes: &[u8; 32],
) -> Result<Field25519, FcmpNativeErrorV1> {
    let integer = SecretU256V1(U256::from_le_slice(bytes));
    if integer.0 >= FIELD25519_MODULUS {
        return Err(FcmpNativeErrorV1::ScalarEncoding);
    }
    let scalar = SecretCopyValueV1::new(Field25519::new(&integer.0));
    Ok(scalar.expose_copy())
}
/// Decode a private Helioselene scalar without creating a by-value encoded
/// input slot or leaving the decoded integer outside an erasing owner.
pub(super) fn decode_secret_helioselene_scalar_v1(
    bytes: &[u8; 32],
) -> Result<HelioseleneField, FcmpNativeErrorV1> {
    let integer = SecretU256V1(U256::from_le_slice(bytes));
    if integer.0 >= HELIOSELENE_MODULUS {
        return Err(FcmpNativeErrorV1::ScalarEncoding);
    }
    let scalar = SecretCopyValueV1::new(HelioseleneField::new(&integer.0));
    Ok(scalar.expose_copy())
}
pub(super) fn validate_edwards_scalar(bytes: [u8; 32]) -> Result<(), FcmpNativeErrorV1> {
    Option::<curve25519_dalek::scalar::Scalar>::from(
        curve25519_dalek::scalar::Scalar::from_canonical_bytes(bytes),
    )
    .map(|_| ())
    .ok_or(FcmpNativeErrorV1::ScalarEncoding)
}
#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;
    use curve25519_dalek::{constants::ED25519_BASEPOINT_POINT, scalar::Scalar};
    thread_local! {
        static TRACKING_CLEARS: Cell<usize> = const { Cell::new(0) };
    }
    #[derive(Clone, Copy)]
    struct TrackingCopy(u64);
    impl Zeroize for TrackingCopy {
        fn zeroize(&mut self) {
            self.0 = 0;
            TRACKING_CLEARS.with(|calls| calls.set(calls.get() + 1));
        }
    }
    #[test]
    fn secret_copy_take_clears_source_and_owned_slots() {
        TRACKING_CLEARS.with(|calls| calls.set(0));
        let mut source = TrackingCopy(7);
        let owned = SecretCopyValueV1::take(&mut source);
        assert_eq!(source.0, 0);
        assert_eq!(owned.as_ref().0, 7);
        assert_eq!(TRACKING_CLEARS.with(Cell::get), 1);
        drop(owned);
        assert_eq!(TRACKING_CLEARS.with(Cell::get), 2);
    }
    #[test]
    fn secret_cycle_coordinates_are_move_only_borrowed_and_drop_both_slots() {
        let point = hash_selene(&[Field25519::ONE]).expect("nonidentity Selene point");
        let expected = point.coordinates().expect("public affine coordinates");
        SECRET_COPY_VALUE_DROPS_V1.with(|drops| drops.set(0));
        let coordinates = point
            .secret_coordinates_v1()
            .expect("owned secret affine coordinates");
        let borrowed = coordinates.component_refs();
        assert_eq!(borrowed, (&expected.0, &expected.1));
        assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 0);
        drop(coordinates);
        assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 2);

        SECRET_COPY_VALUE_DROPS_V1.with(|drops| drops.set(0));
        assert!(SelenePoint::identity().secret_coordinates_v1().is_none());
        assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 0);

        SECRET_COPY_VALUE_DROPS_V1.with(|drops| drops.set(0));
        let unwind = std::panic::catch_unwind(|| {
            let coordinates = point
                .secret_coordinates_v1()
                .expect("coordinate owner before unwind");
            assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 0);
            let _ = core::hint::black_box(coordinates.component_refs());
            panic!("exercise coordinate-owner cleanup during unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 2);

        let source = include_str!("field.rs");
        let owner = source
            .split_once("pub(super) struct SecretCycleCoordinatesV1")
            .expect("coordinate owner")
            .1
            .split_once("struct SecretU256V1")
            .expect("coordinate owner boundary")
            .0;
        assert!(owner.contains("x: SecretCopyValueV1<F>"));
        assert!(owner.contains("y: SecretCopyValueV1<F>"));
        assert!(owner.contains("pub(super) fn component_refs(&self) -> (&F, &F)"));
        for forbidden in [
            "derive(Clone",
            "derive(Copy",
            "fn get",
            "callback",
            "into_parts",
        ] {
            assert!(
                !owner.contains(forbidden),
                "forbidden owner API: {forbidden}"
            );
        }
    }
    #[test]
    fn secret_scalar_encoding_owns_integer_and_byte_scratch_on_every_exit() {
        SECRET_COPY_VALUE_DROPS_V1.with(|drops| drops.set(0));
        SECRET_U256_DROPS_V1.with(|drops| drops.set(0));
        let encoded = encode_secret_field25519_scalar_v1(&Field25519::ONE);
        assert_eq!(encoded.as_ref(), &encode_field25519(Field25519::ONE));
        assert_eq!(SECRET_U256_DROPS_V1.with(Cell::get), 1);
        assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 0);
        drop(encoded);
        assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 1);

        SECRET_COPY_VALUE_DROPS_V1.with(|drops| drops.set(0));
        SECRET_U256_DROPS_V1.with(|drops| drops.set(0));
        let unwind = std::panic::catch_unwind(|| {
            let encoded = encode_secret_helioselene_scalar_v1(&HelioseleneField::ONE);
            assert_eq!(encoded.as_ref(), &encode_helioselene(HelioseleneField::ONE));
            assert_eq!(SECRET_U256_DROPS_V1.with(Cell::get), 1);
            assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 0);
            let _ = core::hint::black_box(&encoded);
            panic!("exercise secret scalar encoding unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(SECRET_U256_DROPS_V1.with(Cell::get), 1);
        assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 1);
    }
    #[test]
    fn secret_point_encoding_owns_integer_and_byte_scratch_on_every_exit() {
        let point = hash_selene(&[Field25519::ONE]).expect("nonidentity Selene point");
        let expected = point.encode();
        SECRET_COPY_VALUE_DROPS_V1.with(|drops| drops.set(0));
        SECRET_U256_DROPS_V1.with(|drops| drops.set(0));
        let encoded = point.secret_encode_v1().expect("secret point encoding");
        assert_eq!(encoded.as_ref(), &expected);
        assert_eq!(SECRET_U256_DROPS_V1.with(Cell::get), 2);
        assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 3);
        drop(encoded);
        assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 4);

        SECRET_COPY_VALUE_DROPS_V1.with(|drops| drops.set(0));
        SECRET_U256_DROPS_V1.with(|drops| drops.set(0));
        assert!(SelenePoint::identity().secret_encode_v1().is_none());
        assert_eq!(SECRET_U256_DROPS_V1.with(Cell::get), 0);
        assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 0);

        SECRET_COPY_VALUE_DROPS_V1.with(|drops| drops.set(0));
        SECRET_U256_DROPS_V1.with(|drops| drops.set(0));
        let unwind = std::panic::catch_unwind(|| {
            let encoded = point
                .secret_encode_v1()
                .expect("owned encoding before unwind");
            assert_eq!(SECRET_U256_DROPS_V1.with(Cell::get), 2);
            assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 3);
            let _ = core::hint::black_box(&encoded);
            panic!("exercise secret encoding unwind");
        });
        assert!(unwind.is_err());
        assert_eq!(SECRET_U256_DROPS_V1.with(Cell::get), 2);
        assert_eq!(SECRET_COPY_VALUE_DROPS_V1.with(Cell::get), 4);
    }
    #[test]
    fn private_scalar_decoders_borrow_bytes_and_own_integer_scratch() {
        let source = include_str!("field.rs");
        let field_decoder = source
            .split_once("pub(super) fn decode_secret_field25519_scalar_v1(")
            .expect("secret Field25519 decoder")
            .1
            .split_once("/// Decode a private Helioselene scalar")
            .expect("Field25519 decoder boundary")
            .0;
        let helios_decoder = source
            .split_once("pub(super) fn decode_secret_helioselene_scalar_v1(")
            .expect("secret Helioselene decoder")
            .1
            .split_once("pub(super) fn validate_edwards_scalar")
            .expect("Helioselene decoder boundary")
            .0;
        for decoder in [field_decoder, helios_decoder] {
            assert!(decoder.contains("bytes: &[u8; 32]"));
            assert!(decoder.contains("SecretU256V1(U256::from_le_slice(bytes))"));
            assert!(decoder.contains("SecretCopyValueV1::new("));
            assert!(decoder.contains("Ok(scalar.expose_copy())"));
            assert!(!decoder.contains("from_le_bytes"));
        }
        let one = U256::ONE.to_le_bytes();
        assert_eq!(
            decode_secret_field25519_scalar_v1(&one).expect("canonical Field25519"),
            decode_field25519_scalar(one).expect("public Field25519 decoder")
        );
        assert_eq!(
            decode_secret_helioselene_scalar_v1(&one).expect("canonical Helioselene"),
            decode_helioselene_scalar(one).expect("public Helioselene decoder")
        );
        assert_eq!(
            decode_secret_field25519_scalar_v1(&FIELD25519_MODULUS.to_le_bytes()),
            Err(FcmpNativeErrorV1::ScalarEncoding)
        );
        assert_eq!(
            decode_secret_helioselene_scalar_v1(&HELIOSELENE_MODULUS.to_le_bytes()),
            Err(FcmpNativeErrorV1::ScalarEncoding)
        );
    }
    fn vector(encoded: &str) -> [u8; 32] {
        assert_eq!(encoded.len(), 64);
        let mut bytes = [0; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&encoded[(2 * index)..(2 * index) + 2], 16)
                .expect("pinned reference vector is hexadecimal");
        }
        bytes
    }
    #[test]
    fn edwards_codec_rejects_identity_torsion_and_noncanonical_y() {
        assert_eq!(
            decode_edwards_point(
                [
                    1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    0, 0, 0, 0, 0, 0
                ],
                false
            ),
            Err(FcmpNativeErrorV1::EdwardsPointIdentity)
        );
        let torsion = curve25519_dalek::constants::EIGHT_TORSION[1]
            .compress()
            .to_bytes();
        assert_eq!(
            decode_edwards_point(torsion, false),
            Err(FcmpNativeErrorV1::EdwardsPointEncoding)
        );
        assert_eq!(
            decode_edwards_point([u8::MAX; 32], false),
            Err(FcmpNativeErrorV1::EdwardsPointEncoding)
        );
    }
    #[test]
    fn wei25519_conversion_is_sign_sensitive_and_deterministic() {
        let point = ED25519_BASEPOINT_POINT * Scalar::from(17_u64);
        let bytes = point.compress().to_bytes();
        let first = edwards_to_wei25519(bytes).expect("valid point");
        assert_eq!(
            first,
            edwards_to_wei25519(bytes).expect("deterministic conversion")
        );
        let negated = (-point).compress().to_bytes();
        let second = edwards_to_wei25519(negated).expect("valid negated point");
        assert_eq!(first.0, second.0);
        assert_eq!(first.1, -second.1);
    }
    #[test]
    fn cycle_point_codecs_are_canonical_and_curve_separated() {
        let selene = selene_hash_initializer();
        let helios = helios_hash_initializer();
        assert_eq!(
            SelenePoint::decode(selene.encode(), false).expect("Selene roundtrip"),
            selene
        );
        assert_eq!(
            HeliosPoint::decode(helios.encode(), false).expect("Helios roundtrip"),
            helios
        );
        assert!(SelenePoint::decode(helios.encode(), false).is_err());
        assert!(HeliosPoint::decode(selene.encode(), false).is_err());
        let mut negative_zero = [0; 32];
        negative_zero[31] = 0x80;
        assert_eq!(
            SelenePoint::decode(negative_zero, true),
            Err(FcmpNativeErrorV1::CyclePointIdentity)
        );
        assert_eq!(
            HeliosPoint::decode(negative_zero, true),
            Err(FcmpNativeErrorV1::CyclePointIdentity)
        );
        assert_eq!(
            HeliosPoint::decode(FIELD25519_MODULUS.to_le_bytes(), false),
            Err(FcmpNativeErrorV1::CyclePointEncoding)
        );
        assert_eq!(
            SelenePoint::decode(HELIOSELENE_MODULUS.to_le_bytes(), false),
            Err(FcmpNativeErrorV1::CyclePointEncoding)
        );
    }
    #[test]
    fn complete_cycle_arithmetic_handles_infinity_and_projective_equivalence() {
        let point = selene_hash_initializer();
        let identity = SelenePoint::identity();
        assert_eq!(identity.add(point), point);
        assert_eq!(point.add(identity), point);
        assert_eq!(point.double(), point.add(point));
        assert_eq!(point.mul(Field25519::ZERO), identity);
        assert_eq!(point.mul(Field25519::ONE), point);
        assert_eq!(point.mul(field25519_from_u64(2)), point.double());
        let field25519_max = Field25519::new(&FIELD25519_MODULUS.wrapping_sub(&U256::ONE));
        assert_eq!(point.mul(field25519_max), point.negate());
        let mut negative_encoding = point.encode();
        negative_encoding[31] ^= 0x80;
        let negative =
            SelenePoint::decode(negative_encoding, false).expect("opposite y is on curve");
        let infinity = point.add(negative);
        assert!(infinity.is_identity());
        assert_eq!(infinity.encode(), [0; 32]);
        assert_eq!(infinity.double(), identity);
        let helios = helios_hash_initializer();
        let mut negative_encoding = helios.encode();
        negative_encoding[31] ^= 0x80;
        let negative =
            HeliosPoint::decode(negative_encoding, false).expect("opposite y is on curve");
        assert!(helios.add(negative).is_identity());
        assert_eq!(helios.double(), helios.add(helios));
        assert_eq!(helios.mul(HelioseleneField::ZERO), HeliosPoint::identity());
        assert_eq!(helios.mul(HelioseleneField::ONE), helios);
        assert_eq!(
            helios.mul(HelioseleneField::new(&U256::from(2_u8))),
            helios.double()
        );
        let helioselene_max = HelioseleneField::new(&HELIOSELENE_MODULUS.wrapping_sub(&U256::ONE));
        assert_eq!(helios.mul(helioselene_max), helios.negate());
        assert_eq!(
            SelenePoint::conditional_select(&identity, &point, Choice::from(0)),
            identity
        );
        assert_eq!(
            SelenePoint::conditional_select(&identity, &point, Choice::from(1)),
            point
        );
    }
    #[test]
    fn generator_domains_are_indexed_and_curve_separated() {
        let selene = selene_generators();
        let helios = helios_generators();
        assert_eq!(selene.len(), SELENE_GENERATOR_COUNT_V1);
        assert_eq!(helios.len(), HELIOS_GENERATOR_COUNT_V1);
        assert_ne!(selene[0], selene[1]);
        assert_ne!(helios[0], helios[1]);
        assert_ne!(selene[0].encode(), helios[0].encode());
    }
    #[test]
    fn native_projective_and_affine_operations_match_upstream_vectors() {
        // Generated directly with monero-fcmp-plus-plus 0.1.0 at 15ef711.
        let selene = selene_hash_initializer();
        assert_eq!(
            selene.encode(),
            vector("8681759fee95c1c97169b8d1476cfab7da101edef5932cf03053ae56f7081d07")
        );
        assert_eq!(
            helios_hash_initializer().encode(),
            vector("fb7f67f7b09edb24431a1358e19884b0a0a34c35ff9908613c6755ac73383429")
        );
        assert_eq!(
            selene_generators()[0].encode(),
            vector("0309b7c9617a6e23ee31dca34b3f081a9382da8b51af6f59a0152c70098eebba")
        );
        assert_eq!(
            selene_generators()[1].encode(),
            vector("bd9376c1d91a8ccc329601ee7c4d22ded432e1e058f799d2d3aa034f119fac18")
        );
        assert_eq!(
            helios_generators()[0].encode(),
            vector("1fe84eb52f8eb10de7f866c7eb0ec76bd0f1798d5a68fde362000d71c0a80125")
        );
        assert_eq!(
            helios_generators()[1].encode(),
            vector("7fa87b44d63a6402f4f42b59dddd6248affa9d1b985e8f3019c34a98c4ed1794")
        );
        assert_eq!(
            selene.add(selene_generators()[0]).encode(),
            vector("11a3e4be74ddb6d7b8761bb70a36cab17f760861dc4fb309a02554c35ef7d760")
        );
        assert_eq!(
            selene.double().encode(),
            vector("079114a9f363ee36918cbdd84b276a8cf6c6a4722be4fc269c47935f4d4779a3")
        );
        let (x, y) = edwards_to_wei25519(ED25519_BASEPOINT_POINT.compress().to_bytes())
            .expect("basepoint conversion");
        assert_eq!(
            encode_field25519(x),
            vector("5a24adaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2a")
        );
        assert_eq!(
            encode_field25519(y),
            vector("142c31815d3a16d64d9e839281b2c26db32eb788d322e11f4b795f475ee6515f")
        );
    }
}
