//! First-round IPA folding over an immutable, reusable public basis.

use super::straus::{Addition, add_digits, straus_block};
use super::*;
use std::{
    ops::Deref,
    sync::{Mutex, OnceLock, TryLockError},
};

// A wallet challenge's two GLV components have at most 65 bits. Splitting
// each at bit 32 nearly halves the simultaneous multiplication's doublings.
const SHIFT: u32 = 32;
const WINDOW: u32 = 4;
const ODDS: usize = 1 << (WINDOW - 2);
const MIN_COLUMNS: usize = 1 << 14;
const MAX_COLUMNS: usize = 1 << 18;

/// Maximum retained heap bytes for the first-fold table at this basis size.
/// Each entry is two 32-byte affine coordinates. Small and over-budget
/// bases use the ordinary generator fold without retaining a table.
#[must_use]
pub const fn fixed_fold_cache_bytes(columns: usize) -> usize {
    if columns >= MIN_COLUMNS && columns <= MAX_COLUMNS && columns.is_power_of_two() {
        (columns / 2) * std::mem::size_of::<crate::EpAffine>()
    } else {
        0
    }
}

/// The original generators and their lazily derived upper-half multiples.
/// The generators can only be borrowed; a new basis has its own empty table.
pub struct FixedGeneratorBasis<C: PastaCurve> {
    generators: Vec<C::AffineExt>,
    shifted: OnceLock<Vec<C::AffineExt>>,
    preparing: Mutex<()>,
}

impl<C: PastaCurve> From<Vec<C::AffineExt>> for FixedGeneratorBasis<C> {
    fn from(generators: Vec<C::AffineExt>) -> Self {
        Self {
            generators,
            shifted: OnceLock::new(),
            preparing: Mutex::new(()),
        }
    }
}

impl<C: PastaCurve> Deref for FixedGeneratorBasis<C> {
    type Target = [C::AffineExt];

    fn deref(&self) -> &Self::Target {
        &self.generators
    }
}

impl<'a, C: PastaCurve> IntoIterator for &'a FixedGeneratorBasis<C> {
    type Item = &'a C::AffineExt;
    type IntoIter = std::slice::Iter<'a, C::AffineExt>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<C: PastaCurve> FixedGeneratorBasis<C> {
    /// Additional retained bytes needed to prepare this basis for repeated openings.
    #[must_use]
    pub fn pending_cache_bytes(&self) -> usize {
        if self.shifted.get().is_none() {
            fixed_fold_cache_bytes(self.len())
        } else {
            0
        }
    }

    /// Prepare the immutable upper-half table for repeated public openings.
    /// This is optional. Callers with one-shot inputs or insufficient memory
    /// can fold directly, and repeated preparations share a single table.
    pub fn prepare_first_fold(&self) {
        if self.pending_cache_bytes() == 0 {
            return;
        }
        // A worker inside shifted_points can steal another opening from this
        // same Rayon pool. Never wait for a parallel table build here.
        let _guard = match self.preparing.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::WouldBlock) => return,
            Err(TryLockError::Poisoned(err)) => err.into_inner(),
        };
        if self.shifted.get().is_none() {
            let table = shifted_points::<C>(&self[self.len() / 2..]);
            let _ = self.shifted.set(table);
        }
    }

    /// Fold the public pairs as `lo + u * hi`. At wallet sizes, `hi` and its
    /// cached `2^32 hi` share one scalar multiplication across the low and
    /// high digits of both GLV components. Each challenge is public.
    #[must_use]
    pub fn fold_vartime(&self, u: &C::ScalarExt) -> Vec<C::AffineExt> {
        if let Some(shifted) = self.shifted.get()
            && let Some(columns) = schedule::<C>(u)
        {
            return self.fold_with_shift(shifted, u, &columns);
        }
        folded_generators_vartime::<C>(self, u)
    }

    fn fold_with_shift(
        &self,
        shifted: &[C::AffineExt],
        u: &C::ScalarExt,
        columns: &[Vec<Addition>],
    ) -> Vec<C::AffineExt> {
        let (low, high) = self.split_at(self.len() / 2);
        let mut out = low.to_vec();
        let block = block_lanes(low.len(), rayon::current_num_threads());
        out.par_chunks_mut(block)
            .zip(high.par_chunks(block))
            .zip(shifted.par_chunks(block))
            .with_max_len(1)
            .for_each(|((output, h), q)| {
                if !straus_block::<C, 2, ODDS>([h, q], output, columns) {
                    fold_block_reference::<C>(output, h, u);
                }
            });
        out
    }
}

fn schedule<C: PastaCurve>(u: &C::ScalarExt) -> Option<Vec<Vec<Addition>>> {
    let d = C::glv_decompose(u)?;
    let mask = (1u128 << SHIFT) - 1;
    if d.k1 <= mask && d.k2 <= mask {
        return None;
    }
    let mut columns = Vec::new();
    for (endo, scalar, negative) in [(false, d.k1, d.k1_neg), (true, d.k2, d.k2_neg)] {
        for (base, part) in [scalar & mask, scalar >> SHIFT].into_iter().enumerate() {
            add_digits::<2>(&mut columns, base, endo, &wnaf_u128(part, WINDOW), negative);
        }
    }
    Some(columns)
}

fn shifted_points<C: PastaCurve>(high: &[C::AffineExt]) -> Vec<C::AffineExt> {
    let block = block_lanes(high.len(), rayon::current_num_threads());
    let mut points = high.to_vec();
    points
        .par_chunks_mut(block)
        .with_max_len(1)
        .for_each(|out| {
            let mut xs: Vec<_> = out.iter().map(PastaAffine::x).collect();
            let mut ys: Vec<_> = out.iter().map(PastaAffine::y).collect();
            let mut scratch = Scratch {
                den: vec![C::Base::ZERO; out.len()],
                prefix: vec![C::Base::ZERO; out.len()],
            };
            if (0..SHIFT).all(|_| ba_double(&mut xs, &mut ys, &mut scratch)) {
                for (i, point) in out.iter_mut().enumerate() {
                    *point = crate::curve::affine_unchecked::<C::AffineExt>(xs[i], ys[i]);
                }
            } else {
                let doubled: Vec<_> = out
                    .iter()
                    .map(|p| (0..SHIFT).fold(C::from(*p), |q, _| q.double()))
                    .collect();
                C::batch_normalize(&doubled, out);
            }
        });
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Ep, Eq};
    use ff::{PrimeField, WithSmallOrderMulGroup};
    use group::{Curve, Group};
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    #[test]
    fn cold_and_warm_cache_sizes_are_bounded() {
        for n in [
            0,
            MIN_COLUMNS / 2,
            MIN_COLUMNS + 1,
            MAX_COLUMNS * 2,
            usize::MAX,
        ] {
            assert_eq!(fixed_fold_cache_bytes(n), 0);
        }
        assert_eq!(
            std::mem::size_of::<crate::EpAffine>(),
            std::mem::size_of::<crate::EqAffine>()
        );
        assert_eq!(fixed_fold_cache_bytes(MAX_COLUMNS), 8 << 20);
        assert_eq!(fixed_fold_cache_bytes(MAX_COLUMNS / 2), 4 << 20);
        let one = Ep::generator().to_affine();
        let basis = FixedGeneratorBasis::<Ep>::from(vec![one; MIN_COLUMNS]);
        assert_eq!(basis.pending_cache_bytes(), MIN_COLUMNS / 2 * 64);
        basis.prepare_first_fold();
        assert_eq!(basis.pending_cache_bytes(), 0);
        let table = basis.shifted.get().unwrap().as_ptr();
        basis.prepare_first_fold();
        assert_eq!(basis.shifted.get().unwrap().as_ptr(), table);
        let short = FixedGeneratorBasis::<Ep>::from(basis[..MIN_COLUMNS / 2].to_vec());
        short.prepare_first_fold();
        assert!(short.shifted.get().is_none());
        let fresh = FixedGeneratorBasis::<Ep>::from(basis.to_vec());
        assert!(fresh.shifted.get().is_none());
    }

    #[test]
    fn shared_first_fold_preparation_does_not_wait_on_rayon_workers() {
        let p = Ep::generator().to_affine();
        let u = crate::Fq::from((1u64 << SHIFT) + 31);
        let expected = (Ep::from(p) + Ep::from(p) * u).to_affine();
        for workers in [1, 2] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(workers)
                .build()
                .unwrap();
            let basis = FixedGeneratorBasis::<Ep>::from(vec![p; MIN_COLUMNS]);
            // An opening entering while a builder owns the claim can finish
            // the ordinary fold immediately, even on a one-worker pool.
            let claim = basis.preparing.lock().unwrap();
            pool.install(|| {
                basis.prepare_first_fold();
                assert!(basis.shifted.get().is_none());
                assert!(basis.fold_vartime(&u).iter().all(|q| *q == expected));
            });
            drop(claim);
            let open = || {
                basis.prepare_first_fold();
                assert!(basis.fold_vartime(&u).iter().all(|q| *q == expected));
            };
            pool.install(|| rayon::join(open, open));
            assert_eq!(basis.pending_cache_bytes(), 0);
        }
    }

    fn compare<C: PastaCurve>() {
        let mut rng = ChaCha20Rng::seed_from_u64(8989);
        let zeta = C::ScalarExt::ZETA;
        let limb = C::ScalarExt::from_u128((1u128 << 64) + (1 << 32) + 3);
        let wallet = (limb + C::ScalarExt::from(97)) * zeta - limb;
        let full = C::ScalarExt::random(&mut rng);
        let half = 133;
        let mut bases: Vec<_> = (0..2 * half)
            .map(|_| C::random(&mut rng).to_affine())
            .collect();
        let compare = |bases: &[C::AffineExt]| {
            let basis = FixedGeneratorBasis::<C>::from(bases.to_vec());
            let shifted = shifted_points::<C>(&basis[half..]);
            for (p, q) in basis[half..].iter().zip(&shifted) {
                assert_eq!(C::from(*p) * C::ScalarExt::from(1u64 << SHIFT), C::from(*q));
            }
            basis.shifted.set(shifted).unwrap();
            assert!(schedule::<C>(&wallet).is_some());
            for u in [
                C::ScalarExt::ZERO,
                C::ScalarExt::ONE,
                -C::ScalarExt::ONE,
                C::ScalarExt::from(1u64 << SHIFT),
                wallet,
                -wallet,
                full,
                -full,
            ] {
                let out = basis.fold_vartime(&u);
                for ((lo, hi), got) in basis.iter().zip(&basis[half..]).zip(out) {
                    assert_eq!(C::from(got), C::from(*lo) + C::from(*hi) * u);
                }
            }
        };
        compare(&bases);
        // With all inputs finite, force exceptional final denominators after
        // the entire multiplication: opposite points in lane 1 and equal
        // points in lane 2. The complete fallback must see the original lows.
        bases[1] = (-(C::from(bases[half + 1]) * wallet)).to_affine();
        bases[2] = (C::from(bases[half + 2]) * wallet).to_affine();
        compare(&bases);
        // Infinity in the upper half exercises the separate preparation and
        // input-preflight fallbacks.
        bases[half] = C::AffineExt::default();
        compare(&bases);
    }

    #[test]
    fn fixed_fold_matches_complete_products_on_both_curves() {
        compare::<Ep>();
        compare::<Eq>();
    }
}
