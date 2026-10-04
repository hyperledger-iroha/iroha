//! Multi-scalar multiplication `sum_i scalars[i] * bases[i]`.
//!
//! Entry points:
//!
//! - [`msm_public`]: public scalars (verifier MSMs, commitments to public
//!   data). The window is sized from the actual largest scalar, inversions are
//!   variable time.
//! - [`msm_secret`]: secret scalars (witness commitments). The window is sized
//!   for full 255-bit scalars so the plan does not depend on their magnitude,
//!   the shared bucket inversion is constant time, and digit and bucket
//!   buffers are zeroised. See [`pippenger`] for the remaining timing posture.
//! - [`FixedBaseTable`]: precomputed window multiples of a fixed commitment
//!   key; one bucket set and no doublings per MSM.
//!
//! Every entry point runs on the caller's Rayon pool
//! (`rayon::current_num_threads()` of the pool the call is made from). There is
//! no global lock or admission control, and no task blocks inside the pool, so
//! nested pools cannot deadlock. Scratch memory is planned against an explicit
//! [`MemoryBudget`].
//!
//! Results are exact group elements and therefore identical across thread
//! counts, budgets and architectures.

mod budget;
pub mod fixed_base;
pub mod pippenger;

pub use budget::{BudgetExceeded, MemoryBudget};
pub use fixed_base::FixedBaseTable;

use group::prime::PrimeCurveAffine;
use rayon::prelude::*;
use zeroize::Zeroize;

use crate::curve::PastaCurve;
use crate::field::PastaField;

/// MSM failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsmError {
    /// `scalars` and `bases` differ in length.
    LengthMismatch(crate::LengthMismatch),
    /// No window plan fits the memory budget.
    Budget(BudgetExceeded),
    /// More points than the engine indexes (`u32::MAX`).
    TooLarge {
        /// The number of points.
        n: usize,
    },
}

impl core::fmt::Display for MsmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::LengthMismatch(e) => write!(f, "msm: {e}"),
            Self::Budget(e) => write!(f, "msm: {e}"),
            Self::TooLarge { n } => write!(f, "msm: {n} points exceed the engine limit"),
        }
    }
}

impl std::error::Error for MsmError {}

impl From<BudgetExceeded> for MsmError {
    fn from(e: BudgetExceeded) -> Self {
        Self::Budget(e)
    }
}

/// Below this many points the MSM multiplies point by point.
const SMALL_MSM: usize = 8;

/// Computes `sum_i scalars[i] * bases[i]` for public scalars.
///
/// # Errors
///
/// [`MsmError::LengthMismatch`] for unequal lengths, [`MsmError::Budget`]
/// when no plan fits `budget`, [`MsmError::TooLarge`] beyond `u32::MAX`
/// points.
pub fn msm_public<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
) -> Result<C, MsmError> {
    msm_impl::<C, false>(scalars, bases, budget)
}

/// Computes `sum_i scalars[i] * bases[i]` for secret scalars.
///
/// The plan assumes 255-bit scalars regardless of their values, the batch
/// inversions are constant time, and scratch holding digits or bucket sums is
/// zeroised. Bucket selection still follows the digits (see [`pippenger`]).
///
/// # Errors
///
/// As [`msm_public`].
pub fn msm_secret<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
) -> Result<C, MsmError> {
    msm_impl::<C, true>(scalars, bases, budget)
}

fn msm_impl<C: PastaCurve, const SECRET: bool>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
) -> Result<C, MsmError> {
    if scalars.len() != bases.len() {
        return Err(MsmError::LengthMismatch(crate::LengthMismatch {
            left: scalars.len(),
            right: bases.len(),
        }));
    }
    let n = scalars.len();
    if u32::try_from(n).is_err() {
        return Err(MsmError::TooLarge { n });
    }
    if n == 0 {
        return Ok(C::identity());
    }
    if n <= SMALL_MSM {
        return Ok(small_msm::<C, SECRET>(scalars, bases));
    }
    let bits = if SECRET {
        255
    } else {
        scalars
            .par_iter()
            .map(PastaField::bit_length_vartime)
            .max()
            .unwrap_or(0) as usize
    };
    if bits == 0 {
        return Ok(C::identity());
    }
    let threads = rayon::current_num_threads();
    let plan = pippenger::plan(n, bits, threads, budget.bytes()).ok_or_else(|| {
        MsmError::Budget(BudgetExceeded {
            required: pippenger::min_plan_bytes(n, bits),
            budget: budget.bytes(),
        })
    })?;
    // Identity bases contribute nothing and would break the affine formulas.
    let skip: Vec<bool> = bases.iter().map(|b| bool::from(b.is_identity())).collect();
    let digits = pippenger::Digits::new(scalars, plan.c, plan.nw);
    let tasks: Vec<(usize, usize)> = (0..plan.groups)
        .flat_map(|g| (0..plan.chunks).map(move |p| (g, p)))
        .collect();
    // Waves of at most `plan.concurrency` tasks bound the live bucket memory.
    let mut partial: Vec<(usize, Vec<C>)> = Vec::with_capacity(tasks.len());
    for wave in tasks.chunks(plan.concurrency) {
        let results: Vec<(usize, Vec<C>)> = wave
            .par_iter()
            .map(|&(g, p)| pippenger::run_task::<C, SECRET>(bases, &skip, &digits, &plan, g, p))
            .collect();
        partial.extend(results);
    }
    drop(digits);
    let mut windows = vec![C::identity(); plan.nw];
    for (w0, sums) in &partial {
        for (k, s) in sums.iter().enumerate() {
            windows[w0 + k] += s;
        }
    }
    let result = pippenger::combine(&windows, plan.c);
    if SECRET {
        // Partial window sums are functions of the secret digits.
        for (_, sums) in &mut partial {
            sums.zeroize();
        }
        windows.zeroize();
    }
    Ok(result)
}

/// Point-by-point MSM for tiny inputs.
fn small_msm<C: PastaCurve, const SECRET: bool>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
) -> C {
    let mut acc = C::identity();
    for (s, b) in scalars.iter().zip(bases.iter()) {
        let p = b.to_curve();
        acc += if SECRET { p * *s } else { p.mul_vartime(s) };
    }
    acc
}

/// Reference MSM by independent scalar multiplications (tests and
/// differential checks).
pub fn msm_naive<C: PastaCurve>(scalars: &[C::ScalarExt], bases: &[C::AffineExt]) -> C {
    scalars
        .iter()
        .zip(bases.iter())
        .fold(C::identity(), |acc, (s, b)| acc + b.to_curve() * *s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::{Ep, EpAffine};
    use crate::field::Fq;
    use ff::Field;
    use group::{Curve, Group};
    use rand_chacha::ChaCha20Rng;
    use rand_core_06::SeedableRng;

    #[test]
    fn small_and_large_agree_with_naive() {
        let mut rng = ChaCha20Rng::seed_from_u64(3);
        for n in [0usize, 1, 5, 9, 100] {
            let bases: Vec<EpAffine> = (0..n).map(|_| Ep::random(&mut rng).to_affine()).collect();
            let scalars: Vec<Fq> = (0..n).map(|_| Fq::random(&mut rng)).collect();
            let expected = msm_naive::<Ep>(&scalars, &bases);
            assert_eq!(
                msm_public::<Ep>(&scalars, &bases, MemoryBudget::DEFAULT).unwrap(),
                expected
            );
            assert_eq!(
                msm_secret::<Ep>(&scalars, &bases, MemoryBudget::DEFAULT).unwrap(),
                expected
            );
        }
    }

    #[test]
    fn errors() {
        let b = [EpAffine::default()];
        let err = msm_public::<Ep>(&[], &b, MemoryBudget::DEFAULT).unwrap_err();
        assert!(matches!(err, MsmError::LengthMismatch(_)));
        let mut rng = ChaCha20Rng::seed_from_u64(4);
        let bases: Vec<EpAffine> = (0..64).map(|_| Ep::random(&mut rng).to_affine()).collect();
        let scalars: Vec<Fq> = (0..64).map(|_| Fq::random(&mut rng)).collect();
        let err = msm_public::<Ep>(&scalars, &bases, MemoryBudget::new(16)).unwrap_err();
        assert!(matches!(err, MsmError::Budget(_)));
        assert!(err.to_string().starts_with("msm: kernel needs"));
        assert_eq!(
            MsmError::from(BudgetExceeded {
                required: 1,
                budget: 0
            }),
            MsmError::Budget(BudgetExceeded {
                required: 1,
                budget: 0
            })
        );
        assert_eq!(
            small_msm::<Ep, false>(&scalars[..2], &bases[..2]),
            msm_naive::<Ep>(&scalars[..2], &bases[..2])
        );
    }
}
