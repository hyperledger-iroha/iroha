//! Hashing to Pallas and Vesta, identical to `pasta_curves` 0.5.2.
//!
//! The construction follows the hash-to-curve draft (version 10) with
//! `expand_message_xmd` over BLAKE2b-512, two field elements per message, the
//! simplified SWU map onto an isogenous curve `E'` (`a' != 0`), point addition
//! on `E'`, and the 3-isogeny back to the Pasta curve. The domain separation
//! tag is `domain_prefix || "-" || curve_id || "_XMD:BLAKE2b_SSWU_RO_"`.
//!
//! Every constant and every operation order matches `pasta_curves`, and the
//! result is compared with it point by point in `tests/curve_oracle.rs`.
//!
//! Timing posture: public inputs only. The simplified SWU map has no
//! value-dependent branches, but its square root (`sqrt_ratio`) uses the
//! table method with value-indexed lookups (see `crate::field`), which is not
//! hardened against cache-timing observers. The addition on `E'` also branches
//! on the (negligible-probability) cases `q0 = +-q1`, as the reference does.
//! Use it for public messages such as generator indices, never for
//! secret-derived values; a constant-time variant would have to scan the whole
//! square-root tables with `conditional_select`.
//!
//! Failure posture: fail closed. If the isogeny output ever failed the curve
//! check (an arithmetic or constant error),
//! [`PastaCurve::hash_to_curve`](crate::curve::PastaCurve::hash_to_curve)
//! returns [`HashToCurveError::OffCurve`] instead of a point, and [`hasher`]
//! panics, as the reference `iso_map` does.
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use blake2::{Blake2b512, Digest};
use ff::{Field, FromUniformBytes, PrimeField};
use subtle::{ConditionallySelectable, ConstantTimeEq};

use crate::curve::PastaCurve;

/// Hash-to-curve failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashToCurveError {
    /// The domain separation tag
    /// (`domain_prefix || "-" || curve_id || "_XMD:BLAKE2b_SSWU_RO_"`) would
    /// exceed 255 bytes.
    DomainTooLong,
    /// The mapped point failed the curve check. This indicates an internal
    /// arithmetic or constant error; the map fails closed instead of returning
    /// a point.
    OffCurve,
}

impl core::fmt::Display for HashToCurveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DomainTooLong => {
                write!(f, "hash-to-curve domain separation tag exceeds 255 bytes")
            }
            Self::OffCurve => write!(f, "hash-to-curve produced a point off the curve"),
        }
    }
}

impl std::error::Error for HashToCurveError {}

/// Per-curve constants of the simplified SWU map and the 3-isogeny.
pub(crate) trait HashToCurveParams: PastaCurve {
    /// `a'` of the isogenous curve `E': y^2 = x^3 + a' x + b'`.
    const ISO_A: <Self as PastaCurve>::Base;
    /// `b' = 1265`.
    const ISO_B: <Self as PastaCurve>::Base;
    /// The SWU non-square `Z = -13`.
    const Z: <Self as PastaCurve>::Base;
    /// A square root of `Z / ROOT_OF_UNITY`.
    const THETA: <Self as PastaCurve>::Base;
    /// Coefficients of the rational maps of the 3-isogeny `E' -> E`.
    const ISOGENY_CONSTANTS: [<Self as PastaCurve>::Base; 13];
}

/// Number of bytes of each expanded output block.
const CHUNKLEN: usize = 64;
/// Input block size of `BLAKE2b` in bytes.
const R_IN_BYTES: usize = 128;
/// Suffix of the domain separation tag.
const DST_SUFFIX: &[u8] = b"_XMD:BLAKE2b_SSWU_RO_";

/// Hashes `message` to two field elements (`expand_message_xmd` with BLAKE2b-512).
///
/// # Errors
///
/// [`HashToCurveError::DomainTooLong`] when the tag exceeds 255 bytes.
pub fn hash_to_field<F: FromUniformBytes<64>>(
    curve_id: &str,
    domain_prefix: &str,
    message: &[u8],
) -> Result<[F; 2], HashToCurveError> {
    let dst_len = DST_SUFFIX
        .len()
        .checked_add(1)
        .and_then(|n| n.checked_add(curve_id.len()))
        .and_then(|n| n.checked_add(domain_prefix.len()))
        .ok_or(HashToCurveError::DomainTooLong)?;
    let dst_len_byte = u8::try_from(dst_len).map_err(|_| HashToCurveError::DomainTooLong)?;
    let dst = |h: &mut Blake2b512| {
        h.update(domain_prefix.as_bytes());
        h.update(b"-");
        h.update(curve_id.as_bytes());
        h.update(DST_SUFFIX);
        h.update([dst_len_byte]);
    };

    let mut h0 = Blake2b512::new();
    h0.update([0u8; R_IN_BYTES]);
    h0.update(message);
    // l_i_b_str = I2OSP(2 * CHUNKLEN, 2) followed by I2OSP(0, 1).
    h0.update([0u8, 128, 0]);
    dst(&mut h0);
    let b0 = h0.finalize();

    let mut h1 = Blake2b512::new();
    h1.update(b0);
    h1.update([1u8]);
    dst(&mut h1);
    let b1 = h1.finalize();

    let mut h2 = Blake2b512::new();
    let mut xored = [0u8; CHUNKLEN];
    for (out, (l, r)) in xored.iter_mut().zip(b0.iter().zip(b1.iter())) {
        *out = l ^ r;
    }
    h2.update(xored);
    h2.update([2u8]);
    dst(&mut h2);
    let b2 = h2.finalize();

    let mut out = [F::ZERO; 2];
    for (block, slot) in [b1, b2].iter().zip(out.iter_mut()) {
        // The reference reverses the big-endian block and reduces it as a
        // little-endian 512-bit integer.
        let mut little = [0u8; CHUNKLEN];
        little.copy_from_slice(block);
        little.reverse();
        *slot = F::from_uniform_bytes(&little);
    }
    Ok(out)
}

/// A point on the isogenous curve `E'` in Jacobian coordinates
/// `(x / z^2, y / z^3)`; `z = 0` is the identity.
#[derive(Clone, Copy, Debug)]
struct Jacobian<F> {
    x: F,
    y: F,
    z: F,
}

impl<F: Field> Jacobian<F> {
    /// Doubling on `y^2 = x^3 + a x + b` (dbl-2007-bl).
    fn double(&self, a: &F) -> Self {
        let xx = self.x.square();
        let yy = self.y.square();
        let yyyy = yy.square();
        let zz = self.z.square();
        let s = ((self.x + yy).square() - xx - yyyy).double();
        let m = xx.double() + xx + *a * zz.square();
        let x3 = m.square() - s.double();
        let y3 = m * (s - x3) - yyyy.double().double().double();
        let z3 = (self.y + self.z).square() - yy - zz;
        let r = Self {
            x: x3,
            y: y3,
            z: z3,
        };
        Self::select(
            &r,
            &Self {
                x: F::ZERO,
                y: F::ZERO,
                z: F::ZERO,
            },
            self.z.is_zero(),
        )
    }

    /// Addition (add-2007-bl) with the reference's handling of equal and
    /// opposite inputs.
    fn add(&self, rhs: &Self, a: &F) -> Self {
        if bool::from(self.z.is_zero()) {
            return *rhs;
        }
        if bool::from(rhs.z.is_zero()) {
            return *self;
        }
        let z1z1 = self.z.square();
        let z2z2 = rhs.z.square();
        let u1 = self.x * z2z2;
        let u2 = rhs.x * z1z1;
        let s1 = self.y * z2z2 * rhs.z;
        let s2 = rhs.y * z1z1 * self.z;
        if u1 == u2 {
            if s1 == s2 {
                return self.double(a);
            }
            return Self {
                x: F::ZERO,
                y: F::ZERO,
                z: F::ZERO,
            };
        }
        let h = u2 - u1;
        let i = h.double().square();
        let j = h * i;
        let r = (s2 - s1).double();
        let v = u1 * i;
        let x3 = r.square() - j - v.double();
        let y3 = r * (v - x3) - (s1 * j).double();
        let z3 = ((self.z + rhs.z).square() - z1z1 - z2z2) * h;
        Self {
            x: x3,
            y: y3,
            z: z3,
        }
    }

    fn select(a: &Self, b: &Self, choice: subtle::Choice) -> Self {
        Self {
            x: F::conditional_select(&a.x, &b.x, choice),
            y: F::conditional_select(&a.y, &b.y, choice),
            z: F::conditional_select(&a.z, &b.z, choice),
        }
    }
}

/// The simplified SWU map onto `E'` for one field element (constant time).
fn map_to_curve_simple_swu<C: HashToCurveParams>(u: &C::Base) -> Jacobian<C::Base> {
    let a = C::ISO_A;
    let b = C::ISO_B;
    let z = C::Z;
    let z_u2 = z * u.square();
    let ta = z_u2.square() + z_u2;
    let num_x1 = b * (ta + C::Base::ONE);
    let div = a * C::Base::conditional_select(&-ta, &z, ta.is_zero());
    let num2_x1 = num_x1.square();
    let div2 = div.square();
    let div3 = div2 * div;
    let num_gx1 = (num2_x1 + a * div2) * num_x1 + b * div3;
    let num_x2 = z_u2 * num_x1;
    let (gx1_square, y1) = C::Base::sqrt_ratio(&num_gx1, &div3);
    let y2 = C::THETA * z_u2 * *u * y1;
    let num_x = C::Base::conditional_select(&num_x2, &num_x1, gx1_square);
    let y = C::Base::conditional_select(&y2, &y1, gx1_square);
    let y = C::Base::conditional_select(&(-y), &y, u.is_odd().ct_eq(&y.is_odd()));
    Jacobian {
        x: num_x * div,
        y: y * div3,
        z: div,
    }
}

/// The 3-isogeny `E' -> E` in Jacobian coordinates ("avoiding inversions").
fn iso_map<C: HashToCurveParams>(p: &Jacobian<C::Base>) -> Jacobian<C::Base> {
    let iso = &C::ISOGENY_CONSTANTS;
    let (x, y, z) = (p.x, p.y, p.z);
    let z2 = z.square();
    let z3 = z2 * z;
    let z4 = z2.square();
    let z6 = z3.square();
    let num_x = ((iso[0] * x + iso[1] * z2) * x + iso[2] * z4) * x + iso[3] * z6;
    let div_x = (z2 * x + iso[4] * z4) * x + iso[5] * z6;
    let num_y = (((iso[6] * x + iso[7] * z2) * x + iso[8] * z4) * x + iso[9] * z6) * y;
    let div_y = (((x + iso[10] * z2) * x + iso[11] * z4) * x + iso[12] * z6) * z3;
    let zo = div_x * div_y;
    let xo = num_x * div_y * zo;
    let yo = num_y * div_x * zo.square();
    Jacobian {
        x: xo,
        y: yo,
        z: zo,
    }
}

/// Hashes `message` to a point of `C` under `domain_prefix`.
///
/// # Errors
///
/// [`HashToCurveError::DomainTooLong`] when the tag exceeds 255 bytes;
/// [`HashToCurveError::OffCurve`] if the result failed the curve check.
pub(crate) fn hash_to_curve<C: HashToCurveParams>(
    domain_prefix: &str,
    message: &[u8],
) -> Result<C, HashToCurveError> {
    let us = hash_to_field::<C::Base>(C::CURVE_ID, domain_prefix, message)?;
    let q0 = map_to_curve_simple_swu::<C>(&us[0]);
    let q1 = map_to_curve_simple_swu::<C>(&us[1]);
    let r = q0.add(&q1, &C::ISO_A);
    jacobian_to_curve::<C>(&iso_map::<C>(&r))
}

/// Converts the isogeny output to a point of `C`, failing closed when it is
/// not on the curve.
///
/// `z = 0` is the identity (`q0 = -q1` on the isogenous curve, negligible
/// probability), as in the reference.
fn jacobian_to_curve<C: PastaCurve>(j: &Jacobian<C::Base>) -> Result<C, HashToCurveError> {
    if bool::from(j.z.is_zero()) {
        return Ok(C::identity());
    }
    // Jacobian (x / z^2, y / z^3) to homogeneous (X / Z, Y / Z): (x z, y, z^3).
    Option::from(C::from_projective_coordinates(
        j.x * j.z,
        j.y,
        j.z.square() * j.z,
    ))
    .ok_or(HashToCurveError::OffCurve)
}

/// Returns a reusable hasher for one domain, mirroring
/// `pasta_curves::arithmetic::CurveExt::hash_to_curve(domain_prefix)`.
///
/// # Errors
///
/// [`HashToCurveError::DomainTooLong`] when the tag exceeds 255 bytes.
///
/// # Panics
///
/// The returned closure panics if a result fails the curve check
/// ([`HashToCurveError::OffCurve`], an internal error), exactly where the
/// reference `iso_map` panics; it never returns a substitute point.
pub fn hasher<C: PastaCurve>(
    domain_prefix: &str,
) -> Result<impl Fn(&[u8]) -> C + '_, HashToCurveError> {
    // Validate the domain once; afterwards only an internal error remains.
    C::hash_to_curve(domain_prefix, b"")?;
    Ok(
        move |message: &[u8]| match C::hash_to_curve(domain_prefix, message) {
            Ok(point) => point,
            Err(error) => panic!("hash_to_curve({domain_prefix}): {error}"),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{Ep, Eq};
    use crate::field::{Fp, Fq};
    use group::Group;

    #[test]
    fn hash_to_field_is_deterministic_and_domain_separated() {
        let a = hash_to_field::<Fp>("pallas", "dom", b"msg").unwrap();
        let b = hash_to_field::<Fp>("pallas", "dom", b"msg").unwrap();
        let c = hash_to_field::<Fp>("pallas", "dom2", b"msg").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a[0], a[1]);
        let long = "x".repeat(240);
        assert_eq!(
            hash_to_field::<Fq>("vesta", &long, b"m"),
            Err(HashToCurveError::DomainTooLong)
        );
        assert_eq!(
            HashToCurveError::DomainTooLong.to_string(),
            "hash-to-curve domain separation tag exceeds 255 bytes"
        );
        assert_eq!(
            HashToCurveError::OffCurve.to_string(),
            "hash-to-curve produced a point off the curve"
        );
    }

    #[test]
    fn off_curve_isogeny_output_fails_closed() {
        let one = Fp::ONE;
        // (1, 1, 1) is not on Pallas (1 != 1 + 5): an error, never a point.
        let bad = Jacobian {
            x: one,
            y: one,
            z: one,
        };
        assert_eq!(
            jacobian_to_curve::<Ep>(&bad),
            Err(HashToCurveError::OffCurve)
        );
        let identity = Jacobian {
            x: one,
            y: one,
            z: Fp::ZERO,
        };
        assert_eq!(jacobian_to_curve::<Ep>(&identity), Ok(Ep::identity()));
        let q = iso_map::<Ep>(&map_to_curve_simple_swu::<Ep>(&Fp::from(7u64)));
        let p = jacobian_to_curve::<Ep>(&q).unwrap();
        assert!(bool::from(crate::curve::PastaCurve::is_on_curve(&p)));
    }

    #[test]
    fn map_and_isogeny_land_on_curve() {
        let u = Fp::from(42u64);
        let q = map_to_curve_simple_swu::<Ep>(&u);
        // q is on E': y^2 = x^3 + a' x z^4 + b' z^6.
        let z2 = q.z.square();
        let z4 = z2.square();
        let lhs = q.y.square();
        let rhs = q.x.square() * q.x + Ep::ISO_A * q.x * z4 + Ep::ISO_B * z4 * z2;
        assert_eq!(lhs, rhs);
        let j = iso_map::<Ep>(&q);
        let p = Ep::from_projective_coordinates(j.x * j.z, j.y, j.z.square() * j.z);
        assert!(bool::from(p.is_some()));
        let doubled = q.double(&Ep::ISO_A);
        let added = q.add(&q, &Ep::ISO_A);
        assert_eq!(doubled.x * added.z.square(), added.x * doubled.z.square());
    }

    #[test]
    fn hash_to_curve_points_are_valid_and_distinct() {
        let h = hasher::<Eq>("Halo2-Parameters").unwrap();
        let p0 = h(&[0, 0, 0, 0, 0]);
        let p1 = h(&[0, 1, 0, 0, 0]);
        assert_ne!(p0, p1);
        assert!(!bool::from(p0.is_identity()));
        assert!(bool::from(crate::curve::PastaCurve::is_on_curve(&p0)));
        assert!(hasher::<Ep>(&"y".repeat(250)).is_err());
    }
}
