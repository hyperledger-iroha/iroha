//! Conversions between the vendored Pasta types and the `iroha_pasta` types.
//!
//! The vendored stack (`halo2-axiom` over `halo2curves-axiom`, which re-exports
//! `pasta_curves` 0.5.2) and `iroha_pasta` define distinct Rust types for the
//! same mathematical objects. Their canonical encodings are identical:
//!
//! - field elements: 32-byte little-endian `PrimeField::to_repr`;
//! - affine points: the canonical affine coordinates `(x, y)` in those field
//!   encodings, the identity having no coordinates; the compressed 32-byte
//!   `GroupEncoding::to_bytes` form follows from them.
//!
//! Every conversion here goes through those canonical forms and panics when
//! the receiving side rejects them (a non-canonical field encoding, or a point
//! that is not on the curve). Each conversion is therefore an encoding parity
//! assertion in itself. Point conversion uses coordinates rather than the
//! compressed form so that converting large generator vectors needs no square
//! roots; the compressed encodings are compared separately by the parity tests.
//!
//! A [`CurveBridge`] names one half of the Pasta cycle on both sides:
//! [`Vesta`] (`EqAffine`, scalars `Fp`, coordinates in `Fq`) and [`Pallas`]
//! (`EpAffine`, scalars `Fq`, coordinates in `Fp`). Generic parity tests run
//! once per bridge.

use halo2_axiom::halo2curves::{
    Coordinates, CurveAffine,
    ff::{PrimeField, WithSmallOrderMulGroup},
    group::{Curve, GroupEncoding, prime::PrimeCurveAffine},
    pasta,
};
use iroha_pasta::{PastaAffine, PastaCurve};

/// One half of the Pasta cycle, named on the vendored and the native side.
pub trait CurveBridge: Copy + Send + Sync + 'static {
    /// The vendored scalar field (`pasta_curves::Fp` or `Fq`).
    type VScalar: PrimeField<Repr = [u8; 32]> + WithSmallOrderMulGroup<3> + Ord;
    /// The vendored base field, in which the point coordinates live.
    type VBase: PrimeField<Repr = [u8; 32]> + WithSmallOrderMulGroup<3> + Ord;
    /// The vendored affine point type.
    type Vendored: CurveAffine<ScalarExt = Self::VScalar, Base = Self::VBase>
        + GroupEncoding<Repr = [u8; 32]>;
    /// The native projective point type.
    type Native: PastaCurve;
    /// Short curve label used in vectors and messages (`"eq"` or `"ep"`).
    const NAME: &'static str;
}

/// The Vesta half of the cycle: `EqAffine`, scalars `Fp`, coordinates in `Fq`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vesta {}

/// The Pallas half of the cycle: `EpAffine`, scalars `Fq`, coordinates in `Fp`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pallas {}

impl CurveBridge for Vesta {
    type VScalar = pasta::Fp;
    type VBase = pasta::Fq;
    type Vendored = pasta::EqAffine;
    type Native = iroha_pasta::Eq;
    const NAME: &'static str = "eq";
}

impl CurveBridge for Pallas {
    type VScalar = pasta::Fq;
    type VBase = pasta::Fp;
    type Vendored = pasta::EpAffine;
    type Native = iroha_pasta::Ep;
    const NAME: &'static str = "ep";
}

/// The vendored projective point type of `B`.
pub type VendoredCurve<B> = <<B as CurveBridge>::Vendored as CurveAffine>::CurveExt;
/// The native scalar field of `B`.
pub type NativeScalar<B> = <<B as CurveBridge>::Native as PastaCurve>::ScalarExt;
/// The native base field of `B`.
pub type NativeBase<B> = <<B as CurveBridge>::Native as PastaCurve>::Base;
/// The native affine point type of `B`.
pub type NativeAffine<B> = <<B as CurveBridge>::Native as PastaCurve>::AffineExt;

/// Re-encodes a canonical field element as another field type.
///
/// Panics unless `to` accepts the 32-byte canonical encoding of `value`.
fn reencode<From, To>(value: &From, what: &str) -> To
where
    From: PrimeField<Repr = [u8; 32]>,
    To: PrimeField<Repr = [u8; 32]>,
{
    Option::from(To::from_repr(value.to_repr())).unwrap_or_else(|| {
        panic!("{what}: canonical encoding rejected by the other implementation")
    })
}

/// The native scalar equal to a vendored scalar.
pub fn native_scalar<B: CurveBridge>(value: &B::VScalar) -> NativeScalar<B> {
    reencode(value, "scalar")
}

/// The vendored scalar equal to a native scalar.
pub fn vendored_scalar<B: CurveBridge>(value: &NativeScalar<B>) -> B::VScalar {
    reencode(value, "scalar")
}

/// The native base field element equal to a vendored one.
pub fn native_base<B: CurveBridge>(value: &B::VBase) -> NativeBase<B> {
    reencode(value, "base field element")
}

/// The vendored base field element equal to a native one.
pub fn vendored_base<B: CurveBridge>(value: &NativeBase<B>) -> B::VBase {
    reencode(value, "base field element")
}

/// The native affine point equal to a vendored one.
///
/// Panics if the native side rejects the vendored coordinates.
pub fn native_affine<B: CurveBridge>(point: &B::Vendored) -> NativeAffine<B> {
    let coordinates: Option<Coordinates<B::Vendored>> = point.coordinates().into();
    coordinates.map_or_else(NativeAffine::<B>::identity, |c| {
        Option::from(NativeAffine::<B>::from_xy(
            native_base::<B>(c.x()),
            native_base::<B>(c.y()),
        ))
        .expect("vendored point is on the native curve")
    })
}

/// The vendored affine point equal to a native one.
///
/// Panics if the vendored side rejects the native coordinates.
pub fn vendored_affine<B: CurveBridge>(point: &NativeAffine<B>) -> B::Vendored {
    let coordinates: Option<_> = point.coordinates().into();
    coordinates.map_or_else(B::Vendored::identity, |(x, y)| {
        Option::from(B::Vendored::from_xy(
            vendored_base::<B>(&x),
            vendored_base::<B>(&y),
        ))
        .expect("native point is on the vendored curve")
    })
}

/// The native projective point equal to a vendored one.
pub fn native_point<B: CurveBridge>(point: &VendoredCurve<B>) -> B::Native {
    native_affine::<B>(&point.to_affine()).to_curve()
}

/// The vendored projective point equal to a native one.
pub fn vendored_point<B: CurveBridge>(point: &B::Native) -> VendoredCurve<B> {
    vendored_affine::<B>(&point.to_affine()).to_curve()
}

/// Converts a slice of vendored scalars.
pub fn native_scalars<B: CurveBridge>(values: &[B::VScalar]) -> Vec<NativeScalar<B>> {
    values.iter().map(native_scalar::<B>).collect()
}

/// Converts a slice of native scalars.
pub fn vendored_scalars<B: CurveBridge>(values: &[NativeScalar<B>]) -> Vec<B::VScalar> {
    values.iter().map(vendored_scalar::<B>).collect()
}

/// Converts a slice of vendored affine points.
pub fn native_affines<B: CurveBridge>(points: &[B::Vendored]) -> Vec<NativeAffine<B>> {
    points.iter().map(native_affine::<B>).collect()
}

/// Converts a slice of native affine points.
pub fn vendored_affines<B: CurveBridge>(points: &[NativeAffine<B>]) -> Vec<B::Vendored> {
    points.iter().map(vendored_affine::<B>).collect()
}

#[cfg(test)]
mod tests {
    use halo2_axiom::halo2curves::{
        CurveExt,
        ff::Field,
        group::{Group, prime::PrimeCurveAffine},
    };
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    use super::*;

    fn round_trips<B: CurveBridge>() {
        let mut rng = ChaCha20Rng::seed_from_u64(7);
        for scalar in [
            B::VScalar::ZERO,
            B::VScalar::ONE,
            -B::VScalar::ONE,
            B::VScalar::random(&mut rng),
        ] {
            let native = native_scalar::<B>(&scalar);
            assert_eq!(native.to_repr(), scalar.to_repr());
            assert_eq!(vendored_scalar::<B>(&native), scalar);
        }
        for base in [B::VBase::ZERO, -B::VBase::ONE, B::VBase::random(&mut rng)] {
            let native = native_base::<B>(&base);
            assert_eq!(native.to_repr(), base.to_repr());
            assert_eq!(vendored_base::<B>(&native), base);
        }
        let generator = B::Vendored::generator();
        let random = (VendoredCurve::<B>::random(&mut rng)).to_affine();
        for point in [B::Vendored::identity(), generator, -generator, random] {
            let native = native_affine::<B>(&point);
            assert_eq!(native.to_bytes(), point.to_bytes());
            assert_eq!(vendored_affine::<B>(&native), point);
            let projective = point.to_curve();
            assert_eq!(native_point::<B>(&projective), native.to_curve());
            assert_eq!(vendored_point::<B>(&native.to_curve()), projective);
        }
        let points = [generator, random];
        let natives = native_affines::<B>(&points);
        assert_eq!(vendored_affines::<B>(&natives), points);
        let scalars = [B::VScalar::ONE, B::VScalar::random(&mut rng)];
        assert_eq!(
            vendored_scalars::<B>(&native_scalars::<B>(&scalars)),
            scalars
        );
        // The generator is the same point on both sides.
        assert_eq!(
            native_affine::<B>(&generator),
            NativeAffine::<B>::generator()
        );
        assert_eq!(VendoredCurve::<B>::generator().to_affine(), generator);
        // Hash-to-curve domain separation uses the curve identifier.
        assert_eq!(
            <VendoredCurve<B> as CurveExt>::CURVE_ID,
            <B::Native as PastaCurve>::CURVE_ID
        );
    }

    #[test]
    fn conversions_round_trip_on_both_curves() {
        round_trips::<Vesta>();
        round_trips::<Pallas>();
    }

    #[test]
    #[should_panic(expected = "canonical encoding rejected")]
    fn reencode_rejects_values_beyond_the_smaller_modulus() {
        // p < q, so q - 1 is not a canonical Fp encoding.
        let _: pasta::Fp = reencode(&-pasta::Fq::ONE, "test");
    }

    #[test]
    fn bridge_names_match_the_vector_labels() {
        assert_eq!(Vesta::NAME, "eq");
        assert_eq!(Pallas::NAME, "ep");
    }
}
