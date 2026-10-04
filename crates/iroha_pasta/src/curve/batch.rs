//! Batch normalisation and the lockstep batch-affine multiplication kernel.
//!
//! Batch-affine arithmetic adds or doubles many independent affine points with
//! one shared field inversion (Montgomery's trick), which costs about three
//! multiplications per point instead of a full inversion. The formulas are
//! incomplete: an addition `P + Q` needs `x(P) != x(Q)`. Every kernel here
//! detects the exceptional lanes (equal or opposite points, the identity) and
//! recomputes them with the complete projective formulas, so results are always
//! exact.
#![allow(clippy::many_single_char_names, clippy::similar_names)]

use ff::Field;
use group::prime::PrimeCurveAffine as _;
use rayon::prelude::*;
use subtle::ConditionallySelectable;

use crate::curve::{PastaAffine, PastaCurve};
use crate::field::PastaField;

/// Writes the affine form of every point of `p` into `q` (constant time).
///
/// One constant-time inversion for the whole slice; identities map to the
/// affine identity. Panics if the lengths differ, as `group::Curve` requires.
pub fn normalize_into<C: PastaCurve>(p: &[C], q: &mut [C::AffineExt]) {
    assert_eq!(p.len(), q.len(), "batch_normalize: length mismatch");
    let mut acc = C::Base::ONE;
    for (pt, out) in p.iter().zip(q.iter_mut()) {
        // Store the running product of z coordinates in the x slot.
        *out = super::affine_unchecked::<C::AffineExt>(acc, C::Base::ZERO);
        let (_, _, z) = pt.projective_coordinates();
        acc = C::Base::conditional_select(&(acc * z), &acc, z.is_zero());
    }
    let mut inv = acc.invert().unwrap_or(C::Base::ZERO);
    for (pt, out) in p.iter().zip(q.iter_mut()).rev() {
        let (x, y, z) = pt.projective_coordinates();
        let skip = z.is_zero();
        let zinv = inv * out.x();
        inv = C::Base::conditional_select(&(inv * z), &inv, skip);
        let affine = super::affine_unchecked::<C::AffineExt>(x * zinv, y * zinv);
        *out = C::AffineExt::conditional_select(&affine, &C::AffineExt::default(), skip);
    }
}

/// Returns the affine forms of `points` (constant time, one inversion).
pub fn batch_normalize<C: PastaCurve>(points: &[C]) -> Vec<C::AffineExt> {
    let mut out = vec![C::AffineExt::default(); points.len()];
    normalize_into(points, &mut out);
    out
}

/// Returns the affine forms of public `points`, normalising chunks in
/// parallel on the caller's Rayon pool with variable-time inversion.
pub fn batch_normalize_vartime<C: PastaCurve>(points: &[C]) -> Vec<C::AffineExt> {
    const CHUNK: usize = 4096;
    let mut out = vec![C::AffineExt::default(); points.len()];
    out.par_chunks_mut(CHUNK)
        .zip(points.par_chunks(CHUNK))
        .for_each(|(o, p)| normalize_vartime_into(p, o));
    out
}

/// Writes the affine form of every public point of `p` into `q` with one
/// variable-time inversion. `q` must have the length of `p`.
///
/// Scratch: two base-field elements per point (the `z` coordinates and the
/// batch-inversion prefix).
pub fn normalize_vartime_into<C: PastaCurve>(p: &[C], q: &mut [C::AffineExt]) {
    debug_assert_eq!(p.len(), q.len(), "normalize_vartime_into: length mismatch");
    let mut zs: Vec<C::Base> = p.iter().map(|pt| pt.projective_coordinates().2).collect();
    crate::field::batch_invert_vartime(&mut zs);
    for ((pt, zinv), out) in p.iter().zip(zs.iter()).zip(q.iter_mut()) {
        let (x, y, z) = pt.projective_coordinates();
        *out = if z.is_zero_vartime() {
            C::AffineExt::default()
        } else {
            super::affine_unchecked::<C::AffineExt>(x * zinv, y * zinv)
        };
    }
}

/// Reusable buffers for batch-affine steps.
#[derive(Debug, Default)]
pub struct BatchScratch<F> {
    den: Vec<F>,
    prefix: Vec<F>,
    lanes: Vec<usize>,
}

/// Inverts every element of `den` in place (variable time). All elements
/// must be nonzero.
pub fn invert_nonzero_vartime<F: PastaField>(den: &mut [F], prefix: &mut Vec<F>) {
    prefix.clear();
    let mut acc = F::ONE;
    for d in den.iter() {
        prefix.push(acc);
        acc *= d;
    }
    let mut inv = acc.invert_vartime().unwrap_or(F::ZERO);
    for (d, p) in den.iter_mut().zip(prefix.iter()).rev() {
        let t = inv * p;
        inv *= *d;
        *d = t;
    }
}

/// Doubles `(x[i], y[i])` for every lane in `lanes` (batch affine).
///
/// Requires non-identity points; Pasta curves have no points of order two, so
/// `2y != 0`.
pub fn double_lanes<F: PastaField>(
    x: &mut [F],
    y: &mut [F],
    lanes: &[usize],
    s: &mut BatchScratch<F>,
) {
    s.den.clear();
    s.den.extend(lanes.iter().map(|&i| y[i].double()));
    invert_nonzero_vartime(&mut s.den, &mut s.prefix);
    for (&i, inv) in lanes.iter().zip(s.den.iter()) {
        let xx = x[i].square();
        let lambda = (xx.double() + xx) * inv;
        let x3 = lambda.square() - x[i].double();
        y[i] = lambda * (x[i] - x3) - y[i];
        x[i] = x3;
    }
}

/// Adds the lane points `(qx(i), qy(i))` to `(x[i], y[i])` for every lane in
/// `lanes` (batch affine). Lanes with `x[i] == qx(i)` are exceptional: they are
/// flagged in `exceptional` and left unchanged.
pub fn add_lanes<F: PastaField>(
    x: &mut [F],
    y: &mut [F],
    lanes: &[usize],
    q: impl Fn(usize) -> (F, F),
    exceptional: &mut [bool],
    s: &mut BatchScratch<F>,
) {
    s.den.clear();
    s.lanes.clear();
    for &i in lanes {
        let (qx, _) = q(i);
        let d = qx - x[i];
        if d.is_zero_vartime() {
            exceptional[i] = true;
        } else {
            s.den.push(d);
            s.lanes.push(i);
        }
    }
    invert_nonzero_vartime(&mut s.den, &mut s.prefix);
    for (&i, inv) in s.lanes.iter().zip(s.den.iter()) {
        let (qx, qy) = q(i);
        let lambda = (qy - y[i]) * inv;
        let x3 = lambda.square() - x[i] - qx;
        y[i] = lambda * (x[i] - x3) - y[i];
        x[i] = x3;
    }
}

/// Number of lanes processed together by [`batch_mul_vartime`].
const MUL_LANES: usize = 512;
/// Signed-digit window width of [`batch_mul_vartime`].
const MUL_WINDOW: u32 = 4;
/// Signed base-16 digits of a 128-bit magnitude, plus one carry digit.
const MUL_DIGITS: usize = 33;

/// Signed base-16 recoding of a 128-bit magnitude: digits in `[-8, 8]`,
/// least significant first, with a final carry digit in `{0, 1}`.
pub fn signed_digits_u128(k: u128) -> [i8; MUL_DIGITS] {
    let mut out = [0i8; MUL_DIGITS];
    let mut carry = 0u8;
    for (j, slot) in out.iter_mut().enumerate() {
        let bits = if j < 32 {
            ((k >> (4 * j)) & 0xF) as u8
        } else {
            0
        };
        let v = bits + carry;
        if v > 8 {
            // v <= 16, so v - 16 is in [-7, 0].
            *slot = i8::try_from(i16::from(v) - 16).unwrap_or(0);
            carry = 1;
        } else {
            *slot = i8::try_from(v).unwrap_or(0);
            carry = 0;
        }
    }
    out
}

/// Computes `scalars[i] * points[i]` for public inputs and returns affine
/// results, using lockstep batch-affine GLV arithmetic.
///
/// Lanes are processed in blocks of 512 on the caller's Rayon pool. Each lane
/// splits its scalar with GLV, then runs 4-bit signed windows over both halves;
/// all lanes of a block double together and share one inversion per step.
/// Lanes that hit an exceptional case are recomputed with the complete
/// formulas, so the output is exact. Variable time: public data only.
///
/// # Errors
///
/// [`crate::LengthMismatch`] when the slices differ in length.
pub fn batch_mul_vartime<C: PastaCurve>(
    points: &[C::AffineExt],
    scalars: &[C::ScalarExt],
) -> Result<Vec<C::AffineExt>, crate::LengthMismatch> {
    if points.len() != scalars.len() {
        return Err(crate::LengthMismatch {
            left: points.len(),
            right: scalars.len(),
        });
    }
    let mut out = vec![C::AffineExt::default(); points.len()];
    out.par_chunks_mut(MUL_LANES)
        .zip(
            points
                .par_chunks(MUL_LANES)
                .zip(scalars.par_chunks(MUL_LANES)),
        )
        .for_each(|(o, (p, k))| mul_block::<C>(p, k, o));
    Ok(out)
}

/// One lockstep block of [`batch_mul_vartime`].
fn mul_block<C: PastaCurve>(
    points: &[C::AffineExt],
    scalars: &[C::ScalarExt],
    out: &mut [C::AffineExt],
) {
    let m = points.len();
    let mut exceptional = vec![false; m];
    let mut digits1 = vec![[0i8; MUL_DIGITS]; m];
    let mut digits2 = vec![[0i8; MUL_DIGITS]; m];
    let mut neg1 = vec![false; m];
    let mut neg2 = vec![false; m];
    for i in 0..m {
        if bool::from(points[i].is_identity()) {
            exceptional[i] = true;
            continue;
        }
        match C::glv_decompose(&scalars[i]) {
            Some(d) => {
                digits1[i] = signed_digits_u128(d.k1);
                digits2[i] = signed_digits_u128(d.k2);
                neg1[i] = d.k1_neg;
                neg2[i] = d.k2_neg;
            }
            None => exceptional[i] = true,
        }
    }

    // Table of multiples 1..=8 of every lane's point.
    let mut s = BatchScratch::<C::Base>::default();
    let mut tx: Vec<Vec<C::Base>> = vec![points.iter().map(PastaAffine::x).collect()];
    let mut ty: Vec<Vec<C::Base>> = vec![points.iter().map(PastaAffine::y).collect()];
    let active: Vec<usize> = (0..m).filter(|&i| !exceptional[i]).collect();
    // Build 2P, 3P = 2P + P, 4P = 2(2P), 5P = 4P + P, 6P = 2(3P), 7P = 6P + P, 8P = 2(4P).
    for mult in 2..=8usize {
        let (mut nx, mut ny);
        if mult % 2 == 0 {
            nx = tx[mult / 2 - 1].clone();
            ny = ty[mult / 2 - 1].clone();
            double_lanes(&mut nx, &mut ny, &active, &mut s);
        } else {
            nx = tx[mult - 2].clone();
            ny = ty[mult - 2].clone();
            let (px, py) = (&tx[0], &ty[0]);
            add_lanes(
                &mut nx,
                &mut ny,
                &active,
                |i| (px[i], py[i]),
                &mut exceptional,
                &mut s,
            );
        }
        tx.push(nx);
        ty.push(ny);
    }

    let beta = C::AffineExt::endo_beta();
    let mut ax = vec![C::Base::ZERO; m];
    let mut ay = vec![C::Base::ZERO; m];
    let mut started = vec![false; m];
    let mut lanes = Vec::with_capacity(m);
    for j in (0..MUL_DIGITS).rev() {
        lanes.clear();
        lanes.extend((0..m).filter(|&i| started[i] && !exceptional[i]));
        if !lanes.is_empty() {
            for _ in 0..MUL_WINDOW {
                double_lanes(&mut ax, &mut ay, &lanes, &mut s);
            }
        }
        for half in 0..2 {
            let (digits, negs) = if half == 0 {
                (&digits1, &neg1)
            } else {
                (&digits2, &neg2)
            };
            let point_of = |i: usize| -> (C::Base, C::Base) {
                let d = digits[i][j];
                let idx = usize::from(d.unsigned_abs()) - 1;
                let x = if half == 0 {
                    tx[idx][i]
                } else {
                    tx[idx][i] * beta
                };
                let y = ty[idx][i];
                let negative = (d < 0) ^ negs[i];
                (x, if negative { -y } else { y })
            };
            lanes.clear();
            for i in 0..m {
                if exceptional[i] || digits[i][j] == 0 {
                    continue;
                }
                if started[i] {
                    lanes.push(i);
                } else {
                    let (x, y) = point_of(i);
                    ax[i] = x;
                    ay[i] = y;
                    started[i] = true;
                }
            }
            add_lanes(&mut ax, &mut ay, &lanes, point_of, &mut exceptional, &mut s);
        }
    }

    for i in 0..m {
        out[i] = if exceptional[i] {
            points[i].to_curve().mul_vartime(&scalars[i]).to_affine()
        } else if started[i] {
            super::affine_unchecked::<C::AffineExt>(ax[i], ay[i])
        } else {
            C::AffineExt::default()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{Ep, EpAffine, Eq};
    use crate::field::{Fp, Fq};
    use group::{Curve, Group};
    use rand_chacha::ChaCha20Rng;
    use rand_core_06::SeedableRng;

    #[test]
    fn normalize_handles_identity_and_matches_to_affine() {
        let mut rng = ChaCha20Rng::seed_from_u64(1);
        let mut pts: Vec<Ep> = (0..9).map(|_| Ep::random(&mut rng)).collect();
        pts[3] = Ep::identity();
        let a = batch_normalize(&pts);
        let b = batch_normalize_vartime(&pts);
        for ((p, x), y) in pts.iter().zip(&a).zip(&b) {
            assert_eq!(p.to_affine(), *x);
            assert_eq!(x, y);
        }
    }

    #[test]
    fn normalize_vartime_into_matches_to_affine() {
        let mut rng = ChaCha20Rng::seed_from_u64(5);
        let mut pts: Vec<Eq> = (0..7).map(|_| Eq::random(&mut rng)).collect();
        pts[0] = Eq::identity();
        let mut out = vec![crate::EqAffine::default(); pts.len()];
        normalize_vartime_into(&pts, &mut out);
        for (p, a) in pts.iter().zip(&out) {
            assert_eq!(p.to_affine(), *a);
        }
    }

    #[test]
    fn signed_digits_reconstruct() {
        for k in [0u128, 1, 8, 9, 0xFF, u128::MAX, 0x8888_8888, 1 << 127] {
            let d = signed_digits_u128(k);
            let mut acc: i128 = 0;
            for digit in d.iter().rev() {
                assert!((-8..=8).contains(digit));
                acc = acc.wrapping_mul(16).wrapping_add(i128::from(*digit));
            }
            assert_eq!(acc.cast_unsigned(), k);
        }
    }

    #[test]
    fn batch_mul_matches_scalar_mul_including_edges() {
        let mut rng = ChaCha20Rng::seed_from_u64(2);
        let n = 600;
        let mut pts: Vec<EpAffine> = (0..n).map(|_| Ep::random(&mut rng).to_affine()).collect();
        let mut ks: Vec<Fq> = (0..n).map(|_| Fq::random(&mut rng)).collect();
        pts[5] = EpAffine::default();
        ks[6] = Fq::ZERO;
        ks[7] = Fq::ONE;
        ks[8] = -Fq::ONE;
        ks[9] = Fq::from(8u64);
        let got = batch_mul_vartime::<Ep>(&pts, &ks).unwrap();
        for i in 0..n {
            assert_eq!(got[i], (pts[i] * ks[i]).to_affine(), "lane {i}");
        }
        assert!(batch_mul_vartime::<Ep>(&pts[..2], &ks[..3]).is_err());
        let q = Eq::generator().to_affine();
        let r = batch_mul_vartime::<Eq>(&[q], &[Fp::from(3u64)]).unwrap();
        assert_eq!(r[0], (Eq::generator() * Fp::from(3u64)).to_affine());
    }

    #[test]
    fn lane_primitives_flag_exceptions() {
        let g = EpAffine::from(Ep::generator());
        let mut x = vec![g.x()];
        let mut y = vec![g.y()];
        let mut exc = vec![false];
        let mut s = BatchScratch::default();
        add_lanes(&mut x, &mut y, &[0], |_| (g.x(), g.y()), &mut exc, &mut s);
        assert!(exc[0]);
        double_lanes(&mut x, &mut y, &[0], &mut s);
        assert_eq!(
            crate::curve::affine_unchecked::<EpAffine>(x[0], y[0]),
            Ep::generator().double().to_affine()
        );
        let mut d = vec![Fp::from(2u64), Fp::from(4u64)];
        let mut prefix = Vec::new();
        invert_nonzero_vartime(&mut d, &mut prefix);
        assert_eq!(d[0] * Fp::from(2u64), Fp::ONE);
    }
}
