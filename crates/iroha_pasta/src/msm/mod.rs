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
//! - [`msm_secret_bounded_cancellable`]: the same secret kernel with a bound
//!   chosen from public protocol data and checked against every scalar before
//!   planning. It does not choose a width from witness values.
//! - [`msm_secret_columns_cancellable`]: one secret MSM per column over shared
//!   bases, run as a bounded number of admitted single-task kernels.
//! - [`FixedBaseTable`]: precomputed window multiples of a fixed commitment
//!   key; one bucket set and no doublings per MSM.
//!
//! Every entry point runs on the caller's Rayon pool
//! (`rayon::current_num_threads()` of the pool the call is made from). Scratch
//! admission never blocks. [`MemoryBudget`] limits each kernel,
//! while [`SharedMemoryBudget`] bounds all concurrent MSM scratch in the process
//! to 64 MiB. Contention lowers the plan, streams contiguous chunks through bounded
//! kernels, or selects allocation-free windowed multiplication when no chunk fits.
//! Nested Rayon work never waits for memory admission.
//!
//! Results are exact group elements and therefore identical across thread
//! counts, budgets and architectures.

mod budget;
pub mod fixed_base;
pub mod pippenger;

pub use budget::{
    BudgetExceeded, MemoryBudget, PROCESS_MSM_SCRATCH_BYTES, ScratchReservation, SharedMemoryBudget,
};
pub use fixed_base::FixedBaseTable;

use crate::{CancellationToken, Cancelled};
use group::CurveAffine;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use zeroize::{Zeroize, Zeroizing};

use crate::curve::PastaCurve;
use crate::field::PastaField;

/// MSM failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsmError {
    /// The caller cancelled this MSM; no partial group result is returned.
    Cancelled,
    /// `scalars` and `bases` differ in length.
    LengthMismatch(crate::LengthMismatch),
    /// No window plan fits the memory budget.
    Budget(BudgetExceeded),
    /// A public bit bound exceeds 255, or a scalar does not fit that bound.
    ScalarBound,
    /// More points than the engine indexes (`u32::MAX`).
    TooLarge {
        /// The number of points.
        n: usize,
    },
}

impl core::fmt::Display for MsmError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Cancelled => f.write_str("msm: operation cancelled"),
            Self::LengthMismatch(e) => write!(f, "msm: {e}"),
            Self::Budget(e) => write!(f, "msm: {e}"),
            Self::ScalarBound => f.write_str("msm: scalar exceeds the public bit bound"),
            Self::TooLarge { n } => write!(f, "msm: {n} points exceed the engine limit"),
        }
    }
}

impl std::error::Error for MsmError {}

impl From<Cancelled> for MsmError {
    fn from(_: Cancelled) -> Self {
        Self::Cancelled
    }
}

impl From<BudgetExceeded> for MsmError {
    fn from(e: BudgetExceeded) -> Self {
        Self::Budget(e)
    }
}

/// Below this many points the MSM multiplies point by point.
const SMALL_MSM: usize = 8;

/// Window width of the allocation-free fallback: `2^6 - 1` stack buckets.
const STACK_WINDOW: usize = 6;

/// Above this many points the allocation-free fallback uses windows: one bucket
/// addition per point and window costs less than point-by-point multiplication.
const STACK_WINDOWED_MIN: usize = 32;

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
    msm_public_with_shared_budget(
        scalars,
        bases,
        budget,
        &SharedMemoryBudget::process_default(),
    )
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
    msm_secret_with_shared_budget(
        scalars,
        bases,
        budget,
        &SharedMemoryBudget::process_default(),
    )
}

/// Public MSM with an explicitly shared caller scratch ceiling.
///
/// Contention may change the execution plan, never the result or input validity.
///
/// # Errors
/// As [`msm_public`]; contention alone does not return an error.
pub fn msm_public_with_shared_budget<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
) -> Result<C, MsmError> {
    msm_impl::<C, false>(scalars, bases, budget, shared, None, None)
}

/// Secret MSM with an explicitly shared caller scratch ceiling.
///
/// # Errors
/// As [`msm_secret`]; contention alone does not return an error.
pub fn msm_secret_with_shared_budget<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
) -> Result<C, MsmError> {
    msm_impl::<C, true>(scalars, bases, budget, shared, None, None)
}

/// Public MSM with an explicit cancellation signal and scratch ceiling.
///
/// # Errors
/// As [`msm_public`], or [`MsmError::Cancelled`] after all kernel tasks join.
pub fn msm_public_cancellable<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<C, MsmError> {
    msm_impl::<C, false>(scalars, bases, budget, shared, cancellation, None)
}

/// Secret MSM with an explicit cancellation signal and scratch ceiling.
///
/// # Errors
/// As [`msm_secret`], or [`MsmError::Cancelled`]; all secret scratch is wiped
/// and every Rayon task has joined before the error is returned.
pub fn msm_secret_cancellable<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<C, MsmError> {
    msm_impl::<C, true>(scalars, bases, budget, shared, cancellation, None)
}

/// One secret MSM per column over the same `bases`, returned in column order.
///
/// Committing many columns concurrently through [`msm_secret_cancellable`] lets
/// nested work stealing open more kernels than the shared ceiling admits; the
/// refused ones stream or take the allocation-free path. Here one reservation
/// admits `lanes` single-task plans at once (at most the pool size and the column
/// count). Each lane claims the next column and runs it on its own thread, so no
/// more kernels exist than were admitted, and fast workers claim more columns.
/// Columns left after the last full round of lanes, and every column when fewer
/// than two lanes fit, run one at a time on the whole pool through
/// [`msm_secret_cancellable`]. Every result is the exact group element that
/// [`msm_secret_cancellable`] returns for its column.
///
/// # Errors
/// [`MsmError::LengthMismatch`] (every column is checked before any work) or
/// [`MsmError::TooLarge`]; otherwise as [`msm_secret_cancellable`], for the
/// lowest failing column. Every Rayon task has joined and all secret scratch is
/// wiped before an error is returned.
pub fn msm_secret_columns_cancellable<C: PastaCurve>(
    columns: &[&[C::ScalarExt]],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<Vec<C>, MsmError> {
    CancellationToken::checkpoint(cancellation)?;
    if let Some(column) = columns.iter().find(|column| column.len() != bases.len()) {
        return Err(MsmError::LengthMismatch(crate::LengthMismatch {
            left: column.len(),
            right: bases.len(),
        }));
    }
    let n = bases.len();
    if u32::try_from(n).is_err() {
        return Err(MsmError::TooLarge { n });
    }
    let whole_pool = |column: &[C::ScalarExt]| {
        msm_impl::<C, true>(column, bases, budget, shared, cancellation, None)
    };
    let wanted = rayon::current_num_threads().min(columns.len());
    // Secret plans assume full-width scalars, never their values.
    let admitted = if wanted < 2 || n <= SMALL_MSM {
        None
    } else {
        pippenger::plan(n, 255, 1, budget.bytes()).and_then(|plan| {
            admit_lanes(plan.bytes, wanted, shared).map(|(lanes, scratch)| (plan, lanes, scratch))
        })
    };
    let Some((plan, lanes, scratch)) = admitted else {
        return columns.iter().map(|column| whole_pool(column)).collect();
    };
    let head = columns.len() - columns.len() % lanes;
    let next = AtomicUsize::new(0);
    let claimed: Vec<Vec<(usize, Result<C, MsmError>)>> = (0..lanes)
        .into_par_iter()
        .map(|_| {
            let mut done = Vec::new();
            loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(column) = columns[..head].get(index) else {
                    break;
                };
                let sum = run_plan::<C, true>(column, bases, plan, cancellation);
                let failed = sum.is_err();
                done.push((index, sum));
                if failed {
                    // Every lane stops at its next claim; the error is reported below.
                    next.store(head, Ordering::Relaxed);
                    break;
                }
            }
            done
        })
        .collect();
    drop(scratch);
    let mut sums: Vec<Option<C>> = vec![None; head];
    let mut failure: Option<(usize, MsmError)> = None;
    for (index, sum) in claimed.into_iter().flatten() {
        match sum {
            Ok(point) => sums[index] = Some(point),
            Err(error) if failure.is_none_or(|(lowest, _)| index < lowest) => {
                failure = Some((index, error));
            }
            Err(_) => {}
        }
    }
    if let Some((_, error)) = failure {
        return Err(error);
    }
    // Without a failure every head column was claimed exactly once; computing any
    // unclaimed one here keeps the result total without trusting that invariant.
    let mut out = Vec::with_capacity(columns.len());
    for (sum, column) in sums.into_iter().zip(columns) {
        out.push(match sum {
            Some(point) => point,
            None => whole_pool(column)?,
        });
    }
    for column in &columns[head..] {
        out.push(whole_pool(column)?);
    }
    Ok(out)
}

/// Reserve scratch for `wanted` concurrent kernels of `plan_bytes` each, or as many
/// as fit; two is the fewest worth running instead of one whole-pool kernel.
fn admit_lanes(
    plan_bytes: usize,
    wanted: usize,
    shared: &SharedMemoryBudget,
) -> Option<(usize, ScratchReservation<'_>)> {
    let mut lanes = wanted.min(shared.available_bytes() / plan_bytes.max(1));
    while lanes >= 2 {
        if let Some(scratch) = plan_bytes
            .checked_mul(lanes)
            .and_then(|bytes| shared.try_reserve(bytes))
        {
            return Some((lanes, scratch));
        }
        lanes -= 1;
    }
    None
}

/// Borrowed secret scalars checked against one independently chosen public bit bound.
///
/// Construction checks every scalar before any small-input, zero-width or scratch
/// fallback can run. It never chooses a width from the values or exposes a private
/// maximum. Callers must derive `bits` from public protocol data, not a witness.
/// This type proves only the numerical bound; it authenticates no protocol data.
/// The borrow prevents mutation of the checked scalars while the proof is held.
pub struct BoundedSecretScalars<'a, F: PastaField> {
    scalars: &'a [F],
    bits: usize,
}

impl<'a, F: PastaField> BoundedSecretScalars<'a, F> {
    /// Check all canonical integers are below `2^bits`, with no allocation.
    /// Zero bits admit only zero. No scalar-dependent early exit or bit-length
    /// scan is used; stack limbs and the accumulated overflow mask are cleared.
    ///
    /// # Errors
    /// [`MsmError::ScalarBound`] for `bits > 255` or any out-of-bound value;
    /// [`MsmError::Cancelled`] at a cooperative chunk boundary.
    pub fn new(
        scalars: &'a [F],
        bits: usize,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Self, MsmError> {
        CancellationToken::checkpoint(cancellation)?;
        if bits > 255 {
            return Err(MsmError::ScalarBound);
        }
        let masks: [u64; 4] = core::array::from_fn(|index| {
            let kept = bits.saturating_sub(index * 64).min(64);
            if kept == 64 { 0 } else { u64::MAX << kept }
        });
        let mut overflow = Zeroizing::new(0_u64);
        for chunk in scalars.chunks(1024) {
            CancellationToken::checkpoint(cancellation)?;
            for scalar in chunk {
                let limbs = Zeroizing::new(scalar.to_canonical_limbs());
                for (limb, mask) in limbs.iter().zip(masks) {
                    *overflow |= limb & mask;
                }
            }
        }
        CancellationToken::checkpoint(cancellation)?;
        if *overflow != 0 {
            return Err(MsmError::ScalarBound);
        }
        Ok(Self { scalars, bits })
    }
}

/// Secret MSM whose window width is bounded by independently public data.
///
/// Uses the same secret kernel: constant-time inversion and clearing of secret
/// scratch. Digit buckets, skips and conflict handling remain variable-time,
/// exactly as in [`msm_secret_cancellable`]. No witness-derived width is chosen.
///
/// # Errors
/// As [`msm_secret_cancellable`]. The checked prefix is already fully validated
/// before this function considers a small/zero-width or scratch fallback.
pub fn msm_secret_bounded_cancellable<C: PastaCurve>(
    scalars: &BoundedSecretScalars<'_, C::ScalarExt>,
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<C, MsmError> {
    msm_impl::<C, true>(
        scalars.scalars,
        bases,
        budget,
        shared,
        cancellation,
        Some(scalars.bits),
    )
}

fn msm_impl<C: PastaCurve, const SECRET: bool>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
    cancellation: Option<&CancellationToken>,
    public_bits: Option<usize>,
) -> Result<C, MsmError> {
    CancellationToken::checkpoint(cancellation)?;
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
        return small_msm::<C, SECRET>(scalars, bases, cancellation);
    }
    let bits = if SECRET {
        public_bits.unwrap_or(255)
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
    let mut plan = pippenger::plan(n, bits, threads, budget.bytes()).ok_or_else(|| {
        MsmError::Budget(BudgetExceeded {
            required: pippenger::min_plan_bytes(n, bits),
            budget: budget.bytes(),
        })
    })?;
    // Admission never waits: nested Rayon work may need this worker to release
    // another permit. If the whole input cannot be admitted, stream smaller
    // contiguous inputs through this same kernel before using the scalar path.
    if plan.bytes > shared.available_bytes() {
        if let Some(smaller) = pippenger::plan(
            n,
            bits,
            threads,
            budget.bytes().min(shared.available_bytes()),
        ) {
            plan = smaller;
        } else {
            return streaming_msm::<C, SECRET>(
                scalars,
                bases,
                bits,
                threads,
                budget,
                shared,
                cancellation,
            );
        }
    }
    shared.try_reserve(plan.bytes).map_or_else(
        || streaming_msm::<C, SECRET>(scalars, bases, bits, threads, budget, shared, cancellation),
        |scratch| planned_msm::<C, SECRET>(scalars, bases, plan, scratch, cancellation),
    )
}

/// Try strictly smaller public input lengths without waiting or allocating.
/// Every failed admission halves the candidate, so contention cannot spin.
fn streaming_plan(
    maximum: usize,
    bits: usize,
    threads: usize,
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
) -> Option<(usize, pippenger::Plan, ScratchReservation<'_>)> {
    let mut length = maximum;
    while length > SMALL_MSM {
        let available = budget.bytes().min(shared.available_bytes());
        if let Some(plan) = pippenger::plan(length, bits, threads, available)
            && let Some(scratch) = shared.try_reserve(plan.bytes)
        {
            return Some((length, plan, scratch));
        }
        length /= 2;
    }
    None
}

/// Sum admitted contiguous chunks using the unchanged public/secret kernel.
/// Chunk lengths depend only on public length, width, workers and admission.
fn streaming_msm<C: PastaCurve, const SECRET: bool>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    bits: usize,
    threads: usize,
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<C, MsmError> {
    let maximum = scalars.len() / 2;
    let mut position = 0;
    let mut accumulator = Zeroizing::new(C::identity());
    while position < scalars.len() {
        CancellationToken::checkpoint(cancellation)?;
        let remaining = scalars.len() - position;
        let Some((length, plan, scratch)) =
            streaming_plan(maximum.min(remaining), bits, threads, budget, shared)
        else {
            // Zero available scratch, a lost series of admission races, or a
            // tiny remainder: no heap allocation or waiting is needed.
            let sum = Zeroizing::new(stack_msm::<C, SECRET>(
                &scalars[position..],
                &bases[position..],
                cancellation,
            )?);
            *accumulator += *sum;
            CancellationToken::checkpoint(cancellation)?;
            return Ok(*accumulator);
        };
        let end = position + length;
        let sum = Zeroizing::new(planned_msm::<C, SECRET>(
            &scalars[position..end],
            &bases[position..end],
            plan,
            scratch,
            cancellation,
        )?);
        *accumulator += *sum;
        position = end;
        // planned_msm joined its jobs and dropped all scratch before returning;
        // the next chunk never reserves while this chunk still owns a permit.
    }
    CancellationToken::checkpoint(cancellation)?;
    Ok(*accumulator)
}

/// Execute one already admitted plan. The permit outlives all kernel buffers,
/// including during cancellation/unwinding, and no nested admission occurs.
fn planned_msm<C: PastaCurve, const SECRET: bool>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    plan: pippenger::Plan,
    _scratch: ScratchReservation<'_>,
    cancellation: Option<&CancellationToken>,
) -> Result<C, MsmError> {
    run_plan::<C, SECRET>(scalars, bases, plan, cancellation)
}

/// The kernel of [`planned_msm`], for scratch its caller already holds. A plan
/// with one concurrent task runs entirely on the calling thread.
fn run_plan<C: PastaCurve, const SECRET: bool>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    plan: pippenger::Plan,
    cancellation: Option<&CancellationToken>,
) -> Result<C, MsmError> {
    CancellationToken::checkpoint(cancellation)?;
    // Identity bases contribute nothing and would break the affine formulas.
    let skip: Vec<bool> = bases.iter().map(|b| bool::from(b.is_identity())).collect();
    let digits = if plan.concurrency == 1 {
        pippenger::Digits::new_serial_cancellable(scalars, plan.c, plan.nw, cancellation)?
    } else {
        pippenger::Digits::new_cancellable(scalars, plan.c, plan.nw, cancellation)?
    };
    let tasks = plan.groups * plan.chunks;
    // Waves of at most `plan.concurrency` tasks bound the live bucket memory.
    let mut windows = Zeroizing::new(vec![C::identity(); plan.nw]);
    for start in (0..tasks).step_by(plan.concurrency) {
        CancellationToken::checkpoint(cancellation)?;
        let mut results: Vec<(usize, Zeroizing<Vec<C>>)> = (start
            ..(start + plan.concurrency).min(tasks))
            .into_par_iter()
            .map(|task| {
                pippenger::run_task_cancellable::<C, SECRET>(
                    bases,
                    &skip,
                    &digits,
                    &plan,
                    task / plan.chunks,
                    task % plan.chunks,
                    cancellation,
                )
            })
            .collect();
        CancellationToken::checkpoint(cancellation)?;
        for (w0, sums) in &mut results {
            for (k, s) in sums.iter().enumerate() {
                windows[*w0 + k] += s;
            }
            if SECRET {
                sums.zeroize();
            }
        }
    }
    let result = pippenger::combine(&windows, plan.c);
    if SECRET {
        windows.zeroize();
    }
    CancellationToken::checkpoint(cancellation)?;
    Ok(result)
}

/// Point-by-point MSM for tiny inputs or zero-fit shared scratch.
fn small_msm<C: PastaCurve, const SECRET: bool>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    cancellation: Option<&CancellationToken>,
) -> Result<C, MsmError> {
    let mut acc = Zeroizing::new(C::identity());
    for (s, b) in scalars.iter().zip(bases.iter()) {
        CancellationToken::checkpoint(cancellation)?;
        let p = b.to_curve();
        // The public GLV multiplier allocates wNAF digit vectors. Use the
        // stack-only complete multiplier for both modes: this path also runs
        // when the shared scratch ceiling has no free bytes.
        *acc += p * *s;
    }
    CancellationToken::checkpoint(cancellation)?;
    Ok(*acc)
}

/// Allocation-free MSM for zero-fit shared scratch: point by point for tiny
/// inputs, otherwise [`windowed_msm`].
fn stack_msm<C: PastaCurve, const SECRET: bool>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    cancellation: Option<&CancellationToken>,
) -> Result<C, MsmError> {
    if scalars.len() <= STACK_WINDOWED_MIN {
        small_msm::<C, SECRET>(scalars, bases, cancellation)
    } else {
        windowed_msm::<C, SECRET>(scalars, bases, cancellation)
    }
}

/// Unsigned [`STACK_WINDOW`]-bit windows with `2^STACK_WINDOW - 1` projective
/// buckets on the stack, on the calling thread.
///
/// From the top window down, the accumulator is doubled `STACK_WINDOW` times and
/// gains `sum_d d * B_d`, where bucket `B_d` sums the finite bases whose window
/// digit is `d` (complete mixed additions, so equal and opposite points are
/// exact). Per point and window this is one addition instead of the constant-time
/// multiplier's doublings, additions and table scans. Bucket selection follows
/// the digits, as in the planned secret kernel; buckets, sums and limbs are wiped.
fn windowed_msm<C: PastaCurve, const SECRET: bool>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    cancellation: Option<&CancellationToken>,
) -> Result<C, MsmError> {
    const BUCKETS: usize = (1 << STACK_WINDOW) - 1;
    let mut acc = Zeroizing::new(C::identity());
    let mut buckets = Zeroizing::new([C::identity(); BUCKETS]);
    for window in (0..256_usize.div_ceil(STACK_WINDOW)).rev() {
        CancellationToken::checkpoint(cancellation)?;
        for _ in 0..STACK_WINDOW {
            *acc = acc.double();
        }
        buckets.fill(C::identity());
        for (index, (scalar, base)) in scalars.iter().zip(bases).enumerate() {
            if index % 1024 == 0 {
                CancellationToken::checkpoint(cancellation)?;
            }
            // Identity bases contribute nothing and would break the mixed formula.
            if bool::from(base.is_identity()) {
                continue;
            }
            let limbs = Zeroizing::new(scalar.to_canonical_limbs());
            let digit = pippenger::window_bits(&limbs, window * STACK_WINDOW, STACK_WINDOW);
            if let Some(slot) = usize::try_from(digit)
                .ok()
                .and_then(|digit| digit.checked_sub(1))
            {
                buckets[slot] = crate::curve::add_mixed_finite(&buckets[slot], base);
            }
        }
        let mut running = Zeroizing::new(C::identity());
        let mut sum = Zeroizing::new(C::identity());
        for bucket in buckets.iter().rev() {
            *running += bucket;
            *sum += *running;
        }
        *acc += *sum;
    }
    CancellationToken::checkpoint(cancellation)?;
    Ok(*acc)
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
    use rand_core::SeedableRng;

    fn shared_msm_parity<C: PastaCurve>() {
        let scalars = (0..48)
            .map(|i| match i % 3 {
                0 => C::ScalarExt::ZERO,
                1 => -C::ScalarExt::ONE,
                _ => C::ScalarExt::from(i + 1),
            })
            .collect::<Vec<_>>();
        let bases = (0..48)
            .map(|i| match i % 4 {
                0 => C::AffineExt::identity(),
                1 => C::generator().to_affine(),
                2 => (-C::generator()).to_affine(),
                _ => (C::generator() * C::ScalarExt::from(i + 1)).to_affine(),
            })
            .collect::<Vec<_>>();
        let expected = msm_naive::<C>(&scalars, &bases).to_affine();
        let shared = SharedMemoryBudget::new(128 << 10);
        for threads in [1, 4] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            // Holding the complete caller budget while entering nested Rayon
            // work must not wait for this same stack frame to release it.
            let held = shared.try_reserve(shared.limit_bytes()).unwrap();
            pool.install(|| {
                rayon::join(
                    || {
                        assert_eq!(
                            msm_public_with_shared_budget::<C>(
                                &scalars,
                                &bases,
                                MemoryBudget::DEFAULT,
                                &shared
                            )
                            .unwrap()
                            .to_affine(),
                            expected
                        )
                    },
                    || {
                        assert_eq!(
                            msm_secret_with_shared_budget::<C>(
                                &scalars,
                                &bases,
                                MemoryBudget::DEFAULT,
                                &shared
                            )
                            .unwrap()
                            .to_affine(),
                            expected
                        )
                    },
                );
            });
            drop(held);
            pool.install(|| {
                (0..8).into_par_iter().for_each(|_| {
                    assert_eq!(
                        msm_secret_with_shared_budget::<C>(
                            &scalars,
                            &bases,
                            MemoryBudget::DEFAULT,
                            &shared
                        )
                        .unwrap()
                        .to_affine(),
                        expected
                    );
                });
            });
            assert_eq!(shared.in_use_bytes(), 0);
            assert!(shared.peak_bytes() <= shared.limit_bytes());
        }
    }

    #[test]
    fn shared_budget_preserves_both_curves_under_nested_contention() {
        shared_msm_parity::<Ep>();
        shared_msm_parity::<crate::Eq>();
    }

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
            small_msm::<Ep, false>(&scalars[..2], &bases[..2], None).unwrap(),
            msm_naive::<Ep>(&scalars[..2], &bases[..2])
        );
    }
}

#[cfg(test)]
mod bounded_tests;

#[cfg(test)]
#[path = "columns_tests.rs"]
mod columns_tests;

#[cfg(test)]
#[path = "streaming_tests.rs"]
mod streaming_tests;
