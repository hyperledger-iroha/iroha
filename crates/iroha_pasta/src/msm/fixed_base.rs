//! Fixed-base MSM with precomputed window tables (commitment keys).
//!
//! For bases `P_i` and window width `c`, the table stores `2^(c w) * P_i` for
//! every window `w`, as affine points in layout `[i * nw + w]`. An MSM then
//! drops every scalar digit into a single set of `2^(c-1)` buckets: there are
//! no per-window bucket sets and no doublings. The table costs
//! `n * nw * 64` bytes (about 2.6 MiB for `n = 2^11, c = 13` and about 84 MiB
//! for `n = 2^16`), so its construction is gated by a [`MemoryBudget`].
//!
//! # Memory
//!
//! Construction is charged for everything it holds at once: the final table,
//! one skip flag per base, and the projective build scratch. Rows are built
//! and normalised in chunks of about [`BUILD_CHUNK_ENTRIES`] entries straight
//! into the final table, so the scratch is bounded by the chunk size rather
//! than by the table (160 bytes per entry while a chunk is built: the
//! projective point, its inverted `z` and the batch-inversion prefix). As many
//! chunks run at once as the budget leaves room for, at most one per Rayon
//! worker; whether construction succeeds depends only on the bases, the window
//! and the budget, never on the pool size.
//!
//! An MSM against the table holds the scalar digits and one bucket set per
//! task. The number of tasks is lowered to fit the budget, so an MSM fails
//! only when a single task does not fit, again independently of the pool size.
//!
//! Tables are built from public bases with exact arithmetic; MSM results equal
//! the variable-base MSM bit for bit.

use crate::CancellationToken;
use group::CurveAffine;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use zeroize::Zeroizing;

use super::pippenger::{Buckets, Digits, GUIDE, MAX_WINDOW, bucket_bytes, num_windows};
use super::{BoundedSecretScalars, BudgetExceeded, MemoryBudget, MsmError, SharedMemoryBudget};
use crate::curve::PastaCurve;

/// Window multiples of a fixed set of bases.
#[derive(Clone, Debug)]
pub struct FixedBaseTable<C: PastaCurve> {
    c: usize,
    nw: usize,
    n: usize,
    points: Vec<C::AffineExt>,
    skip: Vec<bool>,
}

/// Smallest window the automatic choice considers.
const AUTO_MIN_WINDOW: usize = 8;
/// Largest window the automatic choice considers.
const AUTO_MAX_WINDOW: usize = 14;
/// Table entries built and normalised together during construction.
pub const BUILD_CHUNK_ENTRIES: usize = 4096;
/// Bases of one MSM task below which the table MSM does not split further.
const MIN_BASES_PER_TASK: usize = 256;
/// Additions to reduce one bucket by summation by parts (the planner's fitted
/// reduction cost).
const TABLE_REDUCTION: usize = 3;
/// A column claim's one bucket reduction stays below one part in `share` of
/// its insertions, which sets the fewest points a claim takes. A batch uses
/// the first share at which its claims could number at least two per loop:
/// short batches (a few rows) take shorter claims and more reductions rather
/// than leave workers idle.
const TABLE_CLAIM_SHARES: [usize; 3] = [8, 4, 2];

/// The claim that starts at point `pos` of a table batch whose columns each
/// hold `n` points: its column, first point and point count. Column `j`
/// recodes `weights[j]` windows, claims at least `floors[j]` points, and the
/// columns after it hold `after[j]` window-points of work.
///
/// The count is the guided share (`1 / (GUIDE * loops)`) of the batch's
/// remaining work in the column's points, at least the floor and at most the
/// column's remainder; a claim never leaves fewer points than the floor behind.
/// Successive claims from 0 therefore cover every point exactly once.
pub(crate) fn table_claim_at(
    pos: usize,
    n: usize,
    weights: &[usize],
    floors: &[usize],
    after: &[usize],
    loops: usize,
) -> (usize, usize, usize) {
    let column = pos / n;
    let first = pos % n;
    let left = n - first;
    let weight = weights[column].max(1);
    let floor = floors[column].clamp(1, n);
    let remaining = after[column].saturating_add(left.saturating_mul(weight));
    let wanted = remaining
        .div_ceil(GUIDE * loops.max(1))
        .div_ceil(weight)
        .max(floor);
    let count = if wanted >= left || left - wanted < floor {
        left
    } else {
        wanted
    };
    (column, first, count)
}

/// Bytes of the finished table for `n` bases with window `c`.
fn table_bytes<C: PastaCurve>(n: usize, c: usize) -> Option<usize> {
    n.checked_mul(num_windows(255, c))?
        .checked_mul(core::mem::size_of::<C::AffineExt>())
}

/// Transient bytes per table entry while its chunk is built: the projective
/// point, its inverted `z` coordinate and the batch-inversion prefix.
fn build_entry_bytes<C: PastaCurve>() -> usize {
    core::mem::size_of::<C>() + 2 * core::mem::size_of::<C::Base>()
}

/// Bases per construction chunk for `nw` windows (at least one).
fn build_rows(nw: usize) -> usize {
    (BUILD_CHUNK_ENTRIES / nw.max(1)).max(1)
}

/// Memory a construction with window `c` holds at once: `(fixed, chunk)` where
/// `fixed` is the table plus the skip flags and `chunk` is the scratch of one
/// construction chunk. `None` when the table is too large to index.
fn build_bytes<C: PastaCurve>(n: usize, c: usize) -> Option<(usize, usize)> {
    let nw = num_windows(255, c);
    // The bucket engine indexes table points with u32.
    u32::try_from(n.checked_mul(nw)?).ok()?;
    let fixed = table_bytes::<C>(n, c)?.checked_add(n)?;
    let chunk = build_rows(nw)
        .min(n)
        .checked_mul(nw)?
        .checked_mul(build_entry_bytes::<C>())?;
    Some((fixed, chunk))
}

/// The smallest budget a construction with window `c` needs.
fn required_bytes<C: PastaCurve>(n: usize, c: usize) -> Option<usize> {
    let (fixed, chunk) = build_bytes::<C>(n, c)?;
    fixed.checked_add(chunk)
}

/// Fills `out` (layout `[row * nw + w]`) with `2^(c w) * rows[row]`, through a
/// projective scratch of `out.len()` points normalised with one inversion.
fn build_chunk<C: PastaCurve, E>(
    rows: &[C::AffineExt],
    c: usize,
    out: &mut [C::AffineExt],
    checkpoint: &impl Fn() -> Result<(), E>,
) -> Result<(), E> {
    checkpoint()?;
    let nw = out.len() / rows.len().max(1);
    let mut projective = Vec::with_capacity(out.len());
    for base in rows {
        checkpoint()?;
        let mut cur = base.to_curve();
        for w in 0..nw {
            projective.push(cur);
            if w + 1 < nw {
                for _ in 0..c {
                    cur = cur.double();
                }
            }
        }
    }
    checkpoint()?;
    crate::curve::normalize_vartime_into(&projective, out);
    checkpoint()
}

impl<C: PastaCurve> FixedBaseTable<C> {
    /// Builds a table, choosing the window from a cost model among the windows
    /// whose construction fits `budget`.
    ///
    /// # Errors
    ///
    /// [`BudgetExceeded`] when no considered window fits; `required` is the
    /// smallest budget that some window would accept.
    pub fn new(bases: &[C::AffineExt], budget: MemoryBudget) -> Result<Self, BudgetExceeded> {
        Self::new_with_shared_budget(bases, budget, &SharedMemoryBudget::process_default())
    }

    /// Build a table with an additional shared construction-scratch ceiling.
    ///
    /// The retained table belongs to the caller's `budget`; temporary chunk
    /// buffers additionally charge `shared` and the process-wide scratch cap.
    ///
    /// # Errors
    /// As [`Self::new`].
    pub fn new_with_shared_budget(
        bases: &[C::AffineExt],
        budget: MemoryBudget,
        shared: &SharedMemoryBudget,
    ) -> Result<Self, BudgetExceeded> {
        Self::new_impl(bases, budget, shared, &|| Ok::<(), BudgetExceeded>(()))
    }

    /// Build public fixed-base tables with cancellation at bounded construction tasks.
    ///
    /// # Errors
    /// Budget refusal as [`Self::new`], or [`MsmError::Cancelled`] after all tasks join.
    pub fn new_cancellable(
        bases: &[C::AffineExt],
        budget: MemoryBudget,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Self, MsmError> {
        Self::new_impl(
            bases,
            budget,
            &SharedMemoryBudget::process_default(),
            &|| CancellationToken::checkpoint(cancellation).map_err(MsmError::from),
        )
    }

    fn new_impl<E: From<BudgetExceeded> + Send>(
        bases: &[C::AffineExt],
        budget: MemoryBudget,
        shared: &SharedMemoryBudget,
        checkpoint: &(impl Fn() -> Result<(), E> + Sync),
    ) -> Result<Self, E> {
        checkpoint()?;
        let n = bases.len();
        let mut best: Option<(u128, usize)> = None;
        let mut smallest = usize::MAX;
        for c in AUTO_MIN_WINDOW..=AUTO_MAX_WINDOW {
            let Some(bytes) = required_bytes::<C>(n, c) else {
                continue;
            };
            smallest = smallest.min(bytes);
            if bytes > budget.bytes() {
                continue;
            }
            let nw = num_windows(255, c) as u128;
            // Batch-affine additions plus one bucket reduction per worker.
            let cost = (n as u128) * nw * 6
                + (1u128 << (c - 1)) * 25 * (rayon::current_num_threads() as u128);
            if best.is_none_or(|(b, _)| cost < b) {
                best = Some((cost, c));
            }
        }
        let Some((_, c)) = best else {
            return Err(BudgetExceeded {
                required: smallest,
                budget: budget.bytes(),
            }
            .into());
        };
        Self::with_window_impl(bases, c, budget, shared, checkpoint)
    }

    /// Builds a table with window width `c` (clamped to `2..=15`).
    ///
    /// The budget covers the finished table, the skip flags and the
    /// construction scratch (see the module documentation).
    ///
    /// # Errors
    ///
    /// [`BudgetExceeded`] when the construction does not fit `budget`.
    pub fn with_window(
        bases: &[C::AffineExt],
        c: usize,
        budget: MemoryBudget,
    ) -> Result<Self, BudgetExceeded> {
        Self::with_window_and_shared_budget(
            bases,
            c,
            budget,
            &SharedMemoryBudget::process_default(),
        )
    }

    /// Build the requested window table with shared construction admission.
    ///
    /// When no chunk fits the currently available shared scratch, normalize
    /// points individually without allocating construction scratch.
    ///
    /// # Errors
    /// As [`Self::with_window`].
    pub fn with_window_and_shared_budget(
        bases: &[C::AffineExt],
        c: usize,
        budget: MemoryBudget,
        shared: &SharedMemoryBudget,
    ) -> Result<Self, BudgetExceeded> {
        Self::with_window_impl(bases, c, budget, shared, &|| Ok::<(), BudgetExceeded>(()))
    }

    fn with_window_impl<E: From<BudgetExceeded> + Send>(
        bases: &[C::AffineExt],
        c: usize,
        budget: MemoryBudget,
        shared: &SharedMemoryBudget,
        checkpoint: &(impl Fn() -> Result<(), E> + Sync),
    ) -> Result<Self, E> {
        checkpoint()?;
        let c = c.clamp(2, MAX_WINDOW);
        let n = bases.len();
        let nw = num_windows(255, c);
        // Tables the bucket engine cannot index exceed every budget.
        let Some((fixed, chunk)) = build_bytes::<C>(n, c) else {
            return Err(BudgetExceeded {
                required: usize::MAX,
                budget: budget.bytes(),
            }
            .into());
        };
        budget.check(fixed.saturating_add(chunk))?;
        // Chunks built at the same time: as many as the budget leaves room
        // for, at most one per worker and at least one.
        let room = budget
            .bytes()
            .saturating_sub(fixed)
            .min(shared.available_bytes());
        let concurrency = room
            .checked_div(chunk)
            .unwrap_or(usize::MAX)
            .clamp(1, rayon::current_num_threads().max(1));
        let rows = build_rows(nw);
        let scratch = shared.try_reserve(chunk.saturating_mul(concurrency));
        let mut points = vec![C::AffineExt::default(); n * nw];
        if scratch.is_some() {
            // Iterate waves directly, without a heap vector of job descriptors.
            for (out, wave_bases) in points
                .chunks_mut(rows * nw * concurrency)
                .zip(bases.chunks(rows * concurrency))
            {
                checkpoint()?;
                out.par_chunks_mut(rows * nw)
                    .zip(wave_bases.par_chunks(rows))
                    .try_for_each(|(out, chunk_bases)| {
                        build_chunk::<C, E>(chunk_bases, c, out, checkpoint)
                    })?;
            }
        } else {
            for (out, base) in points.chunks_mut(nw).zip(bases) {
                checkpoint()?;
                let mut point = base.to_curve();
                for (window, slot) in out.iter_mut().enumerate() {
                    *slot = point.to_affine();
                    if window + 1 < nw {
                        for _ in 0..c {
                            point = point.double();
                        }
                    }
                }
            }
        }
        checkpoint()?;
        let skip = bases
            .iter()
            .enumerate()
            .map(|(index, base)| {
                if index % 1024 == 0 {
                    checkpoint()?;
                }
                Ok(bool::from(base.is_identity()))
            })
            .collect::<Result<Vec<_>, E>>()?;
        checkpoint()?;
        Ok(Self {
            c,
            nw,
            n,
            points,
            skip,
        })
    }

    /// The window width.
    pub fn window(&self) -> usize {
        self.c
    }

    /// The number of bases.
    pub fn len(&self) -> usize {
        self.n
    }

    /// Whether the table has no bases.
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Bytes held by the table points.
    pub fn table_bytes(&self) -> usize {
        self.points
            .len()
            .saturating_mul(core::mem::size_of::<C::AffineExt>())
    }

    /// MSM with public scalars against the table bases.
    ///
    /// # Errors
    ///
    /// [`MsmError::LengthMismatch`] unless `scalars.len()` equals the number of
    /// bases; [`MsmError::Budget`] when the digits and a single task's bucket
    /// set exceed `budget` (independent of the pool size).
    pub fn msm_public(
        &self,
        scalars: &[C::ScalarExt],
        budget: MemoryBudget,
    ) -> Result<C, MsmError> {
        self.msm_public_with_shared_budget(scalars, budget, &SharedMemoryBudget::process_default())
    }

    /// MSM with secret scalars: constant-time batch inversion and zeroised
    /// scratch, with the bucket-selection posture described in
    /// [`super::pippenger`].
    ///
    /// # Errors
    ///
    /// As [`Self::msm_public`].
    pub fn msm_secret(
        &self,
        scalars: &[C::ScalarExt],
        budget: MemoryBudget,
    ) -> Result<C, MsmError> {
        self.msm_secret_with_shared_budget(scalars, budget, &SharedMemoryBudget::process_default())
    }

    /// Public table MSM charging an explicit shared scratch ceiling.
    ///
    /// # Errors
    /// As [`Self::msm_public`]; contention selects an allocation-free fallback.
    pub fn msm_public_with_shared_budget(
        &self,
        scalars: &[C::ScalarExt],
        budget: MemoryBudget,
        shared: &SharedMemoryBudget,
    ) -> Result<C, MsmError> {
        self.msm_impl::<false>(scalars, budget, shared, None)
    }

    /// Secret table MSM charging an explicit shared scratch ceiling.
    ///
    /// # Errors
    /// As [`Self::msm_secret`]; contention selects an allocation-free fallback.
    pub fn msm_secret_with_shared_budget(
        &self,
        scalars: &[C::ScalarExt],
        budget: MemoryBudget,
        shared: &SharedMemoryBudget,
    ) -> Result<C, MsmError> {
        self.msm_impl::<true>(scalars, budget, shared, None)
    }

    /// Public table MSM with explicit cancellation and shared scratch.
    ///
    /// # Errors
    /// As [`Self::msm_public`], or [`MsmError::Cancelled`] after tasks join.
    pub fn msm_public_cancellable(
        &self,
        scalars: &[C::ScalarExt],
        budget: MemoryBudget,
        shared: &SharedMemoryBudget,
        cancellation: Option<&CancellationToken>,
    ) -> Result<C, MsmError> {
        self.msm_impl::<false>(scalars, budget, shared, cancellation)
    }

    /// Secret table MSM with explicit cancellation and shared scratch.
    ///
    /// # Errors
    /// As [`Self::msm_secret`], or [`MsmError::Cancelled`]. Secret scratch is
    /// wiped and all tasks have joined before the error is returned.
    pub fn msm_secret_cancellable(
        &self,
        scalars: &[C::ScalarExt],
        budget: MemoryBudget,
        shared: &SharedMemoryBudget,
        cancellation: Option<&CancellationToken>,
    ) -> Result<C, MsmError> {
        self.msm_impl::<true>(scalars, budget, shared, cancellation)
    }

    /// One secret MSM per bounded column against the table bases, returned in
    /// column order.
    ///
    /// Columns share one claim queue, widest first, as in
    /// [`super::msm_secret_bounded_columns_cancellable`]. One reservation admits a
    /// batch: `loops` claiming loops (at most the pool size), each holding one
    /// bucket set and the digits of its current claim, plus the columns'
    /// partial sums. A claim is a range of one column's points with all of the
    /// column's windows: the loop recodes those points just before it inserts
    /// them, still in its core's cache, and reduces the claim once into a
    /// partial sum. A column's result is the sum of its partials.
    ///
    /// Claims take about `1 / (4 * loops)` of the batch's remaining work
    /// ([`table_claim_at`]), but never so few points that the claim's
    /// reduction exceeds an eighth of its insertions (a quarter or a half in a
    /// batch too short to give every loop two claims otherwise). Early claims
    /// are therefore whole columns and the last are short, so a fast worker
    /// finishes the batch while a slow one completes its last claim. Each column is recoded only
    /// for the windows its public bound needs. Claim boundaries depend only on
    /// public lengths, bounds, the window and the admitted loop count; worker
    /// timing decides only which loop runs a claim. Every result is the exact
    /// group element [`Self::msm_secret_cancellable`] returns for its column.
    ///
    /// A batch with fewer claims than workers, and every column when only one
    /// column or one worker is given, the table is small, or no batch of two
    /// loops fits, runs one column at a time through
    /// [`Self::msm_secret_cancellable`].
    ///
    /// # Errors
    /// [`MsmError::LengthMismatch`] (every column is checked before any work);
    /// otherwise as [`Self::msm_secret_cancellable`]. All tasks have joined and
    /// secret scratch is wiped before an error is returned.
    pub fn msm_secret_bounded_columns_cancellable(
        &self,
        columns: &[BoundedSecretScalars<'_, C::ScalarExt>],
        budget: MemoryBudget,
        shared: &SharedMemoryBudget,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Vec<C>, MsmError> {
        CancellationToken::checkpoint(cancellation)?;
        if let Some(column) = columns.iter().find(|column| column.scalars.len() != self.n) {
            return Err(MsmError::LengthMismatch(crate::LengthMismatch {
                left: column.scalars.len(),
                right: self.n,
            }));
        }
        let alone = |column: &BoundedSecretScalars<'_, C::ScalarExt>| {
            if column.bits == 0 {
                // A zero bound admits only zeros, whose sum is the identity.
                CancellationToken::checkpoint(cancellation)?;
                return Ok(C::identity());
            }
            self.msm_impl::<true>(column.scalars, budget, shared, cancellation)
        };
        let threads = rayon::current_num_threads();
        let (all_digits, task) = self.msm_scratch();
        // A claiming loop holds one bucket set and the digits of at most one
        // whole column, recoded per claim. A column that would not fit the
        // per-kernel budget alone fails exactly as it does through the
        // single-column MSM.
        let loop_bytes = all_digits.saturating_add(task);
        if threads < 2
            || columns.len() < 2
            || self.n < 2 * MIN_BASES_PER_TASK
            || budget.check(loop_bytes).is_err()
        {
            return columns.iter().map(alone).collect();
        }
        let mut order: Vec<(usize, usize)> = columns
            .iter()
            .enumerate()
            .filter(|(_, column)| column.bits > 0)
            .map(|(index, column)| (index, num_windows(column.bits, self.c).min(self.nw)))
            .collect();
        // Widest first; the sort is stable, so equal widths keep column order.
        order.sort_by_key(|&(_, windows)| core::cmp::Reverse(windows));
        let mut sums: Vec<Option<C>> = vec![None; columns.len()];
        let mut next = 0;
        while next < order.len() {
            // A loop for every worker first, then the next columns in claim
            // order whose partial sums fit beside them; fewer loops only when
            // not even one column fits.
            let available = shared.available_bytes();
            let mut loops = threads;
            let (fixed, end) = loop {
                let room = available.saturating_sub(loop_bytes.saturating_mul(loops));
                let (mut fixed, mut end) = (0_usize, next);
                while let Some(&(_, windows)) = order.get(end) {
                    match fixed.checked_add(self.column_bytes(windows)) {
                        Some(with) if with <= room => (fixed, end) = (with, end + 1),
                        _ => break,
                    }
                }
                if end > next || loops <= 2 {
                    break (fixed, end);
                }
                loops -= 1;
            };
            let batch = &order[next..end];
            let claims_at = |share: usize| -> usize {
                batch
                    .iter()
                    .map(|&(_, windows)| self.n.div_ceil(self.claim_floor(windows, share)))
                    .sum()
            };
            let share = TABLE_CLAIM_SHARES
                .into_iter()
                .find(|&share| claims_at(share) >= 2 * loops)
                .unwrap_or(TABLE_CLAIM_SHARES[TABLE_CLAIM_SHARES.len() - 1]);
            let admitted = if batch.is_empty() || claims_at(share) < threads {
                None
            } else {
                super::admit_batch(fixed, loop_bytes, loops, shared)
            };
            let Some((loops, scratch)) = admitted else {
                // Fewer claims than workers, or nothing fits: one column at a
                // time, each split across the pool by the table MSM.
                let rest = if batch.is_empty() {
                    &order[next..]
                } else {
                    batch
                };
                for &(index, _) in rest {
                    sums[index] = Some(alone(&columns[index])?);
                }
                next += rest.len();
                continue;
            };
            self.run_columns(columns, batch, loops, share, cancellation, &mut sums)?;
            // run_columns joined every loop and wiped its scratch before returning.
            drop(scratch);
            next = end;
        }
        CancellationToken::checkpoint(cancellation)?;
        // Every nonzero column was summed above; computing any missing one here
        // keeps the result total without trusting that (zero bounds give the
        // identity without scratch).
        let mut out = Vec::with_capacity(columns.len());
        for (sum, column) in sums.into_iter().zip(columns) {
            out.push(match sum {
                Some(point) => point,
                None => alone(column)?,
            });
        }
        Ok(out)
    }

    /// Fewest points a claim of a column recoding `windows` windows takes: its
    /// one reduction stays below `1 / share` of its insertions.
    fn claim_floor(&self, windows: usize, share: usize) -> usize {
        (share * TABLE_REDUCTION * (1usize << (self.c - 1)))
            .div_ceil(windows.max(1))
            .clamp(1, self.n.max(1))
    }

    /// Scratch of one batch column recoding `windows` windows: one partial sum
    /// per claim at the smallest share (claims recode their own points).
    fn column_bytes(&self, windows: usize) -> usize {
        let share = TABLE_CLAIM_SHARES[TABLE_CLAIM_SHARES.len() - 1];
        self.n
            .div_ceil(self.claim_floor(windows, share))
            .saturating_mul(size_of::<(usize, C)>() + 16)
    }

    /// One admitted batch (`(column index, windows)` in claim order): `loops`
    /// claiming loops take point ranges ([`table_claim_at`], floors at `share`)
    /// until none is left, each recoding its claim's points just before it
    /// inserts them (still in that core's cache), then each column's partial
    /// sums are added into `sums`. All loops have joined, and digits, buckets
    /// and partial sums are wiped, before this returns, also on cancellation.
    fn run_columns(
        &self,
        columns: &[BoundedSecretScalars<'_, C::ScalarExt>],
        batch: &[(usize, usize)],
        loops: usize,
        share: usize,
        cancellation: Option<&CancellationToken>,
        sums: &mut [Option<C>],
    ) -> Result<(), MsmError> {
        let nb = 1usize << (self.c - 1);
        let weights: Vec<usize> = batch.iter().map(|&(_, windows)| windows).collect();
        let floors: Vec<usize> = weights
            .iter()
            .map(|&windows| self.claim_floor(windows, share))
            .collect();
        let mut after = vec![0_usize; batch.len()];
        for column in (1..batch.len()).rev() {
            after[column - 1] =
                after[column].saturating_add(self.n.saturating_mul(weights[column]));
        }
        let total = batch.len() * self.n;
        let next = AtomicUsize::new(0);
        let claim = || {
            let mut pos = next.load(Ordering::Relaxed);
            while pos < total {
                let (column, first, count) =
                    table_claim_at(pos, self.n, &weights, &floors, &after, loops);
                match next.compare_exchange_weak(
                    pos,
                    pos + count,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => return Some((column, first, count)),
                    Err(seen) => pos = seen,
                }
            }
            None
        };
        let claimed: Vec<Vec<(usize, Zeroizing<C>)>> = (0..loops)
            .into_par_iter()
            .map(|_| {
                let mut done = Vec::new();
                'claims: while CancellationToken::checkpoint(cancellation).is_ok() {
                    let Some((column, first, count)) = claim() else {
                        break;
                    };
                    let (index, windows) = batch[column];
                    // A cancelled recoding returns no digits; the checkpoint
                    // below reports it.
                    let Ok(digits) = Digits::new_serial_cancellable(
                        &columns[index].scalars[first..first + count],
                        self.c,
                        windows,
                        cancellation,
                    ) else {
                        break;
                    };
                    let mut buckets = Buckets::<C, true>::new(&self.points, nb);
                    for (local, i) in (first..first + count).enumerate() {
                        if i % 256 == 0 && CancellationToken::checkpoint(cancellation).is_err() {
                            // The checkpoint below reports the cancellation.
                            break 'claims;
                        }
                        if self.skip[i] {
                            continue;
                        }
                        for (w, &d) in digits.row(local).iter().enumerate() {
                            if d != 0 {
                                buckets.insert(
                                    usize::from(d.unsigned_abs()) - 1,
                                    i * self.nw + w,
                                    d < 0,
                                );
                            }
                        }
                    }
                    buckets.flush();
                    done.push((column, Zeroizing::new(buckets.reduce(0, nb))));
                    // Dropping `buckets` and `digits` wipes their scratch.
                }
                done
            })
            .collect();
        CancellationToken::checkpoint(cancellation)?;
        // Without cancellation every point was claimed exactly once.
        let mut totals = Zeroizing::new(vec![C::identity(); batch.len()]);
        for (column, partial) in claimed.into_iter().flatten() {
            totals[column] += *partial;
        }
        for (&(index, _), sum) in batch.iter().zip(totals.iter()) {
            sums[index] = Some(*sum);
        }
        Ok(())
    }

    /// Scratch of one MSM: `(digit_bytes, bucket_bytes)` where the second is
    /// one task's bucket set.
    fn msm_scratch(&self) -> (usize, usize) {
        let nb = 1usize << (self.c - 1);
        let digits = self.n.saturating_mul(self.nw).saturating_mul(2);
        (
            digits,
            bucket_bytes(nb)
                .unwrap_or(usize::MAX)
                .saturating_add(size_of::<C>()),
        )
    }

    /// Number of MSM tasks for `threads` workers within `budget`, or the
    /// budget error when even one task does not fit.
    fn msm_tasks(&self, threads: usize, budget: MemoryBudget) -> Result<usize, BudgetExceeded> {
        let (digits, buckets) = self.msm_scratch();
        budget.check(digits.saturating_add(buckets))?;
        let affordable = budget
            .bytes()
            .saturating_sub(digits)
            .checked_div(buckets)
            .unwrap_or(usize::MAX);
        Ok(threads
            .max(1)
            .min(self.n.div_ceil(MIN_BASES_PER_TASK))
            .min(affordable)
            .max(1))
    }

    fn msm_impl<const SECRET: bool>(
        &self,
        scalars: &[C::ScalarExt],
        budget: MemoryBudget,
        shared: &SharedMemoryBudget,
        cancellation: Option<&CancellationToken>,
    ) -> Result<C, MsmError> {
        CancellationToken::checkpoint(cancellation)?;
        if scalars.len() != self.n {
            return Err(MsmError::LengthMismatch(crate::LengthMismatch {
                left: scalars.len(),
                right: self.n,
            }));
        }
        if self.n == 0 {
            return Ok(C::identity());
        }
        let nb = 1usize << (self.c - 1);
        let threads = rayon::current_num_threads();
        let mut tasks = self.msm_tasks(threads, budget)?;
        let (digits_bytes, task_bytes) = self.msm_scratch();
        let available = MemoryBudget::new(budget.bytes().min(shared.available_bytes()));
        if let Ok(smaller) = self.msm_tasks(threads, available) {
            tasks = tasks.min(smaller);
        }
        let Some(_scratch) =
            shared.try_reserve(digits_bytes.saturating_add(tasks.saturating_mul(task_bytes)))
        else {
            let mut result = Zeroizing::new(C::identity());
            for (i, scalar) in scalars.iter().enumerate() {
                CancellationToken::checkpoint(cancellation)?;
                *result += self.points[i * self.nw].to_curve() * *scalar;
            }
            CancellationToken::checkpoint(cancellation)?;
            return Ok(*result);
        };
        let digits = Digits::new_cancellable(scalars, self.c, self.nw, cancellation)?;
        let step = self.n.div_ceil(tasks);
        let mut partial = Zeroizing::new(
            (0..tasks)
                .into_par_iter()
                .map(|t| {
                    if CancellationToken::checkpoint(cancellation).is_err() {
                        return C::identity();
                    }
                    let mut buckets = Buckets::<C, SECRET>::new(&self.points, nb);
                    let i1 = ((t + 1) * step).min(self.n);
                    for i in t * step..i1 {
                        if i % 256 == 0 && CancellationToken::checkpoint(cancellation).is_err() {
                            return C::identity();
                        }
                        if self.skip[i] {
                            continue;
                        }
                        for (w, &d) in digits.row(i).iter().enumerate() {
                            if d != 0 {
                                buckets.insert(
                                    usize::from(d.unsigned_abs()) - 1,
                                    i * self.nw + w,
                                    d < 0,
                                );
                            }
                        }
                    }
                    buckets.flush();
                    // Dropping `buckets` wipes its scratch in secret mode.
                    buckets.reduce(0, nb)
                })
                .collect::<Vec<C>>(),
        );
        CancellationToken::checkpoint(cancellation)?;
        let result = partial.iter().fold(C::identity(), |acc, p| acc + p);
        if SECRET {
            zeroize::Zeroize::zeroize(&mut partial);
        }
        Ok(result)
    }

    /// The table point `2^(c w) * P_i` (tests and diagnostics).
    pub fn entry(&self, i: usize, w: usize) -> Option<C::AffineExt> {
        if i < self.n && w < self.nw {
            self.points.get(i * self.nw + w).copied()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn cancelled_table_construction_joins_tasks_releases_scratch_and_retries() {
        fn check<C: PastaCurve>() {
            use std::sync::atomic::{AtomicUsize, Ordering};
            let mut rng = ChaCha20Rng::seed_from_u64(197);
            let bases = (0..450)
                .map(|_| C::random(&mut rng).to_affine())
                .collect::<Vec<_>>();
            // More than two chunks; both the shared-scratch path and its zero-budget
            // scalar fallback must unwind before this call returns.
            for shared_limit in [0, MemoryBudget::DEFAULT.bytes()] {
                let shared = SharedMemoryBudget::new(shared_limit);
                let token = CancellationToken::new();
                let calls = AtomicUsize::new(0);
                let result = pool(4).install(|| {
                    FixedBaseTable::<C>::with_window_impl(
                        &bases,
                        13,
                        MemoryBudget::DEFAULT,
                        &shared,
                        &|| {
                            if calls.fetch_add(1, Ordering::AcqRel) == 17 {
                                token.cancel();
                            }
                            token.check().map_err(MsmError::from)
                        },
                    )
                });
                assert!(matches!(result, Err(MsmError::Cancelled)));
                assert!(calls.load(Ordering::Acquire) >= 18);
                assert_eq!(shared.in_use_bytes(), 0);
                if shared_limit == 0 {
                    assert_eq!(shared.peak_bytes(), 0);
                } else {
                    assert!(shared.peak_bytes() > 0);
                }
                let retry = pool(4)
                    .install(|| {
                        FixedBaseTable::<C>::with_window_and_shared_budget(
                            &bases,
                            13,
                            MemoryBudget::DEFAULT,
                            &shared,
                        )
                    })
                    .unwrap();
                let expected =
                    FixedBaseTable::<C>::with_window(&bases, 13, MemoryBudget::DEFAULT).unwrap();
                assert_eq!(retry.points, expected.points);
                assert_eq!(retry.skip, expected.skip);
                assert_eq!(shared.in_use_bytes(), 0);
                assert!(matches!(
                    FixedBaseTable::<C>::new_cancellable(
                        &bases,
                        MemoryBudget::DEFAULT,
                        Some(&token)
                    ),
                    Err(MsmError::Cancelled)
                ));
                let fresh = CancellationToken::new();
                let ordinary = FixedBaseTable::<C>::new(&bases, MemoryBudget::DEFAULT).unwrap();
                let cancellable = FixedBaseTable::<C>::new_cancellable(
                    &bases,
                    MemoryBudget::DEFAULT,
                    Some(&fresh),
                )
                .unwrap();
                assert_eq!(ordinary.points, cancellable.points);
                assert_eq!(ordinary.skip, cancellable.skip);
            }
        }
        check::<crate::Ep>();
        check::<crate::Eq>();
    }

    use super::*;
    use crate::curve::{Eq, EqAffine};
    use crate::field::Fp;
    use crate::msm::msm_naive;
    use ff::Field;
    use group::{Curve, Group};
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    #[test]
    fn shared_budget_fallback_preserves_table_and_msm() {
        let bases = (1..24)
            .map(|i| (Eq::generator() * crate::Fp::from(i)).to_affine())
            .collect::<Vec<_>>();
        let scalars = (1..24).map(|i| -crate::Fp::from(i)).collect::<Vec<_>>();
        let shared = SharedMemoryBudget::new(0);
        let ordinary = FixedBaseTable::<Eq>::with_window(&bases, 8, MemoryBudget::DEFAULT).unwrap();
        let fallback = FixedBaseTable::<Eq>::with_window_and_shared_budget(
            &bases,
            8,
            MemoryBudget::DEFAULT,
            &shared,
        )
        .unwrap();
        assert_eq!(ordinary.points, fallback.points);
        let automatic =
            FixedBaseTable::<Eq>::new_with_shared_budget(&bases, MemoryBudget::DEFAULT, &shared)
                .unwrap();
        let expected = msm_naive::<Eq>(&scalars, &bases).to_affine();
        for table in [&ordinary, &fallback, &automatic] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(1)
                .build()
                .unwrap();
            pool.install(|| {
                rayon::join(
                    || {
                        assert_eq!(
                            table
                                .msm_public_with_shared_budget(
                                    &scalars,
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
                            table
                                .msm_secret_with_shared_budget(
                                    &scalars,
                                    MemoryBudget::DEFAULT,
                                    &shared
                                )
                                .unwrap()
                                .to_affine(),
                            expected
                        )
                    },
                )
            });
        }
        assert_eq!(shared.in_use_bytes(), 0);
        assert_eq!(shared.peak_bytes(), 0);
    }
    #[test]
    fn table_msm_matches_naive() {
        let mut rng = ChaCha20Rng::seed_from_u64(8);
        let mut bases: Vec<EqAffine> = (0..300).map(|_| Eq::random(&mut rng).to_affine()).collect();
        bases[10] = EqAffine::default();
        let scalars: Vec<Fp> = (0..300).map(|_| Fp::random(&mut rng)).collect();
        let expected = msm_naive::<Eq>(&scalars, &bases);
        for c in [4usize, 9, 13] {
            let t = FixedBaseTable::<Eq>::with_window(&bases, c, MemoryBudget::DEFAULT).unwrap();
            assert_eq!(t.window(), c);
            assert_eq!(t.len(), 300);
            assert!(!t.is_empty());
            assert_eq!(
                t.msm_public(&scalars, MemoryBudget::DEFAULT).unwrap(),
                expected
            );
            assert_eq!(
                t.msm_secret(&scalars, MemoryBudget::DEFAULT).unwrap(),
                expected
            );
            assert_eq!(t.entry(3, 0), Some(bases[3]));
        }
        let auto = FixedBaseTable::<Eq>::new(&bases, MemoryBudget::DEFAULT).unwrap();
        assert!(auto.table_bytes() > 0);
        assert!(auto.entry(300, 0).is_none());
        assert!(FixedBaseTable::<Eq>::new(&bases, MemoryBudget::new(10)).is_err());
        assert!(
            auto.msm_public(&scalars[..3], MemoryBudget::DEFAULT)
                .is_err()
        );
    }

    fn pool(threads: usize) -> rayon::ThreadPool {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("thread pool")
    }

    #[test]
    fn construction_budget_covers_table_flags_and_chunk_scratch() {
        let mut rng = ChaCha20Rng::seed_from_u64(9);
        // More than one construction chunk (4096 / 20 = 204 rows per chunk).
        let bases: Vec<EqAffine> = (0..450).map(|_| Eq::random(&mut rng).to_affine()).collect();
        let c = 13;
        let nw = num_windows(255, c);
        let (fixed, chunk) = build_bytes::<Eq>(bases.len(), c).unwrap();
        assert_eq!(fixed, bases.len() * nw * 64 + bases.len());
        assert_eq!(chunk, build_rows(nw) * nw * (96 + 32 + 32));
        assert_eq!(build_entry_bytes::<Eq>(), 160);
        // The old check charged only the table; the transient chunk scratch
        // must now fit as well.
        let table_only = MemoryBudget::new(fixed);
        assert_eq!(
            FixedBaseTable::<Eq>::with_window(&bases, c, table_only).unwrap_err(),
            BudgetExceeded {
                required: fixed + chunk,
                budget: fixed
            }
        );
        let exact = MemoryBudget::new(fixed + chunk);
        let reference =
            FixedBaseTable::<Eq>::with_window(&bases, c, MemoryBudget::DEFAULT).unwrap();
        for threads in [1usize, 2, 7] {
            // The exact budget admits one chunk at a time on every pool.
            let built = pool(threads)
                .install(|| FixedBaseTable::<Eq>::with_window(&bases, c, exact))
                .unwrap();
            assert_eq!(built.points, reference.points, "threads = {threads}");
            assert_eq!(built.skip, reference.skip);
        }
        for w in [0, 1, nw - 1] {
            let mut expected = bases[449].to_curve();
            for _ in 0..c * w {
                expected = expected.double();
            }
            assert_eq!(reference.entry(449, w), Some(expected.to_affine()));
        }
        assert!(required_bytes::<Eq>(usize::MAX, c).is_none());
        assert!(FixedBaseTable::<Eq>::with_window(&[], c, MemoryBudget::new(0)).is_ok());
    }

    #[test]
    fn auto_window_reports_the_smallest_feasible_budget() {
        let mut rng = ChaCha20Rng::seed_from_u64(10);
        let bases: Vec<EqAffine> = (0..64).map(|_| Eq::random(&mut rng).to_affine()).collect();
        let err = FixedBaseTable::<Eq>::new(&bases, MemoryBudget::new(1)).unwrap_err();
        let smallest = (AUTO_MIN_WINDOW..=AUTO_MAX_WINDOW)
            .filter_map(|c| required_bytes::<Eq>(64, c))
            .min()
            .unwrap();
        assert_eq!(err.required, smallest);
        assert!(FixedBaseTable::<Eq>::new(&bases, MemoryBudget::new(smallest)).is_ok());
    }

    #[test]
    fn msm_feasibility_does_not_depend_on_the_pool_size() {
        let mut rng = ChaCha20Rng::seed_from_u64(11);
        let n = 600;
        let bases: Vec<EqAffine> = (0..n).map(|_| Eq::random(&mut rng).to_affine()).collect();
        let scalars: Vec<Fp> = (0..n).map(|_| Fp::random(&mut rng)).collect();
        let expected = msm_naive::<Eq>(&scalars, &bases);
        let table = FixedBaseTable::<Eq>::with_window(&bases, 13, MemoryBudget::DEFAULT).unwrap();
        let (digits, buckets) = table.msm_scratch();
        // Room for exactly one task's buckets: 7 workers would want 3 tasks.
        let tight = MemoryBudget::new(digits + buckets);
        let short = MemoryBudget::new(digits + buckets - 1);
        for threads in [1usize, 7] {
            let p = pool(threads);
            assert_eq!(p.install(|| table.msm_tasks(threads, tight)), Ok(1));
            assert_eq!(
                p.install(|| table.msm_public(&scalars, tight)).unwrap(),
                expected,
                "threads = {threads}"
            );
            assert_eq!(
                p.install(|| table.msm_secret(&scalars, tight)).unwrap(),
                expected
            );
            assert!(matches!(
                p.install(|| table.msm_public(&scalars, short)),
                Err(MsmError::Budget(_))
            ));
        }
        assert_eq!(table.msm_tasks(7, MemoryBudget::DEFAULT), Ok(3));
        let two = MemoryBudget::new(digits + 2 * buckets);
        assert_eq!(table.msm_tasks(7, two), Ok(2));
    }

    /// A canonical integer below `2^bits` (`bits < 255`), built limb by limb
    /// from the top: the bound's largest value, or a uniform one.
    fn below<F: crate::field::PastaField>(rng: &mut ChaCha20Rng, bits: usize, largest: bool) -> F {
        use rand_core::Rng;
        let shift = F::from(u64::MAX) + F::ONE;
        let mut value = F::ZERO;
        for limb in (0..bits.div_ceil(64)).rev() {
            let kept = (bits - 64 * limb).min(64);
            let mask = if kept == 64 {
                u64::MAX
            } else {
                (1_u64 << kept) - 1
            };
            let word = if largest { mask } else { rng.next_u64() & mask };
            value = value * shift + F::from(word);
        }
        value
    }

    /// A column within `bits`: zeros, the largest value, and uniform values.
    fn bounded_column<F: crate::field::PastaField>(
        rng: &mut ChaCha20Rng,
        n: usize,
        bits: usize,
    ) -> Vec<F> {
        (0..n)
            .map(|i| match (bits, i % 5) {
                (0, _) | (_, 0) => F::ZERO,
                (255, 1) => -F::ONE,
                (255, _) => F::random(&mut *rng),
                (_, 1) => below(rng, bits, true),
                _ => below(rng, bits, false),
            })
            .collect()
    }

    fn table_columns<C: PastaCurve>() {
        const BOUNDS: [usize; 6] = [0, 1, 13, 88, 177, 255];
        let mut rng = ChaCha20Rng::seed_from_u64(61);
        for n in [9usize, 300, 600, 1031] {
            let mut bases: Vec<C::AffineExt> =
                (0..n).map(|_| C::random(&mut rng).to_affine()).collect();
            bases[n / 2] = C::AffineExt::identity();
            let tables: Vec<FixedBaseTable<C>> = [4usize, 9, 13]
                .into_iter()
                .map(|c| {
                    FixedBaseTable::<C>::with_window(&bases, c, MemoryBudget::DEFAULT).unwrap()
                })
                .collect();
            for count in [0usize, 1, 2, 7, 13] {
                let columns: Vec<(Vec<C::ScalarExt>, usize)> = (0..count)
                    .map(|j| {
                        let bits = BOUNDS[j % BOUNDS.len()];
                        (bounded_column(&mut rng, n, bits), bits)
                    })
                    .collect();
                let bounded: Vec<BoundedSecretScalars<'_, C::ScalarExt>> = columns
                    .iter()
                    .map(|(scalars, bits)| BoundedSecretScalars::new(scalars, *bits, None).unwrap())
                    .collect();
                let expected: Vec<C> = columns
                    .iter()
                    .map(|(scalars, _)| msm_naive::<C>(scalars, &bases))
                    .collect();
                for table in &tables {
                    for threads in [1usize, 3, 8] {
                        let shared =
                            SharedMemoryBudget::new(super::super::PROCESS_MSM_SCRATCH_BYTES);
                        let sums = pool(threads)
                            .install(|| {
                                table.msm_secret_bounded_columns_cancellable(
                                    &bounded,
                                    MemoryBudget::DEFAULT,
                                    &shared,
                                    None,
                                )
                            })
                            .unwrap();
                        assert_eq!(
                            sums,
                            expected,
                            "n={n} c={} columns={count} threads={threads}",
                            table.window()
                        );
                        assert_eq!(shared.in_use_bytes(), 0);
                        assert!(shared.peak_bytes() <= shared.limit_bytes());
                    }
                }
            }
        }
    }

    #[test]
    fn table_columns_equal_independent_sums_on_both_curves() {
        table_columns::<crate::Ep>();
        table_columns::<Eq>();
    }

    /// Successive claims from point 0 cover every point exactly once, stay in
    /// one column, take at least the floor unless they end their column, and
    /// never leave fewer points than the floor behind.
    #[test]
    fn table_claims_cover_every_point_once() {
        use rand_core::Rng;
        let mut rng = ChaCha20Rng::seed_from_u64(62);
        for case in 0..400 {
            let n = 1 + (rng.next_u64() % 5000) as usize;
            let count = 1 + (rng.next_u64() % 10) as usize;
            let weights: Vec<usize> = (0..count)
                .map(|_| 1 + (rng.next_u64() % 64) as usize)
                .collect();
            let floors: Vec<usize> = (0..count)
                .map(|_| 1 + (rng.next_u64() % 3000) as usize)
                .collect();
            let mut after = vec![0_usize; count];
            for column in (1..count).rev() {
                after[column - 1] = after[column] + n * weights[column];
            }
            let loops = 2 + (rng.next_u64() % 15) as usize;
            let total = n * count;
            let mut covered = vec![0_u8; total];
            let mut pos = 0;
            while pos < total {
                let (column, first, points) =
                    table_claim_at(pos, n, &weights, &floors, &after, loops);
                assert_eq!(column * n + first, pos, "case {case}");
                assert!(
                    points >= 1 && first + points <= n,
                    "case {case}: claim crosses its column"
                );
                let floor = floors[column].clamp(1, n);
                assert!(
                    points >= floor || first + points == n,
                    "case {case}: {points} below floor {floor}"
                );
                let left = n - first - points;
                assert!(
                    left == 0 || left >= floor,
                    "case {case}: leaves {left} below floor {floor}"
                );
                for slot in &mut covered[pos..pos + points] {
                    *slot += 1;
                }
                pos += points;
            }
            assert!(covered.iter().all(|&times| times == 1), "case {case}");
        }
    }

    /// A cancellation from another thread at any point of a batch returns either
    /// every exact sum or `Cancelled`, and releases all scratch.
    #[test]
    fn table_columns_cancelled_from_another_thread_return_exact_sums_or_cancelled() {
        let mut rng = ChaCha20Rng::seed_from_u64(63);
        let n = 2048;
        let bases: Vec<EqAffine> = (0..n).map(|_| Eq::random(&mut rng).to_affine()).collect();
        let table = FixedBaseTable::<Eq>::with_window(&bases, 9, MemoryBudget::DEFAULT).unwrap();
        let columns: Vec<Vec<Fp>> = (0..9)
            .map(|_| (0..n).map(|_| Fp::random(&mut rng)).collect())
            .collect();
        let bounded: Vec<BoundedSecretScalars<'_, Fp>> = columns
            .iter()
            .map(|scalars| BoundedSecretScalars::new(scalars, 255, None).unwrap())
            .collect();
        let expected: Vec<Eq> = columns
            .iter()
            .map(|scalars| msm_naive::<Eq>(scalars, &bases))
            .collect();
        let workers = pool(4);
        let mut cancelled = 0;
        for delay_us in [0_u64, 100, 300, 1_000, 3_000, 10_000, 30_000, 100_000] {
            let shared = SharedMemoryBudget::new(super::super::PROCESS_MSM_SCRATCH_BYTES);
            let token = CancellationToken::new();
            let result = std::thread::scope(|scope| {
                scope.spawn(|| {
                    std::thread::sleep(std::time::Duration::from_micros(delay_us));
                    token.cancel();
                });
                workers.install(|| {
                    table.msm_secret_bounded_columns_cancellable(
                        &bounded,
                        MemoryBudget::DEFAULT,
                        &shared,
                        Some(&token),
                    )
                })
            });
            match result {
                Ok(sums) => assert_eq!(sums, expected, "delay {delay_us} us"),
                Err(error) => {
                    assert_eq!(error, MsmError::Cancelled, "delay {delay_us} us");
                    cancelled += 1;
                }
            }
            assert_eq!(shared.in_use_bytes(), 0, "delay {delay_us} us");
        }
        // An immediate cancellation always wins the race against the whole batch.
        assert!(cancelled >= 1);
    }

    /// Lengths are checked before any work; with no shared scratch every column
    /// takes the allocation-free path; a limit that admits one column beside a
    /// loop per worker runs batches of one column and never holds more.
    #[test]
    fn table_columns_check_lengths_fall_back_and_respect_the_limit() {
        let mut rng = ChaCha20Rng::seed_from_u64(64);
        let n = 600;
        let bases: Vec<EqAffine> = (0..n).map(|_| Eq::random(&mut rng).to_affine()).collect();
        let table = FixedBaseTable::<Eq>::with_window(&bases, 4, MemoryBudget::DEFAULT).unwrap();
        let columns: Vec<Vec<Fp>> = (0..5)
            .map(|_| (0..n).map(|_| Fp::random(&mut rng)).collect())
            .collect();
        let expected: Vec<Eq> = columns
            .iter()
            .map(|scalars| msm_naive::<Eq>(scalars, &bases))
            .collect();
        let mut bounded: Vec<BoundedSecretScalars<'_, Fp>> = columns
            .iter()
            .map(|scalars| BoundedSecretScalars::new(scalars, 255, None).unwrap())
            .collect();
        bounded[4] = BoundedSecretScalars::new(&columns[4][..n - 1], 255, None).unwrap();
        let shared = SharedMemoryBudget::new(super::super::PROCESS_MSM_SCRATCH_BYTES);
        assert!(matches!(
            pool(4).install(|| {
                table.msm_secret_bounded_columns_cancellable(
                    &bounded,
                    MemoryBudget::DEFAULT,
                    &shared,
                    None,
                )
            }),
            Err(MsmError::LengthMismatch(_))
        ));
        assert_eq!(shared.peak_bytes(), 0);
        bounded[4] = BoundedSecretScalars::new(&columns[4], 255, None).unwrap();
        let none = SharedMemoryBudget::new(0);
        let sums = pool(4)
            .install(|| {
                table.msm_secret_bounded_columns_cancellable(
                    &bounded,
                    MemoryBudget::DEFAULT,
                    &none,
                    None,
                )
            })
            .unwrap();
        assert_eq!(sums, expected);
        assert_eq!(none.peak_bytes(), 0);
        let (all_digits, task) = table.msm_scratch();
        // A loop holds one bucket set and at most one whole column's digits.
        let looped = all_digits + task;
        let one = table.column_bytes(table.nw);
        // Room for one column's partial sums beside four loops, never two columns'.
        let limited = SharedMemoryBudget::new(one + 4 * looped + one / 2);
        let sums = pool(4)
            .install(|| {
                table.msm_secret_bounded_columns_cancellable(
                    &bounded,
                    MemoryBudget::DEFAULT,
                    &limited,
                    None,
                )
            })
            .unwrap();
        assert_eq!(sums, expected);
        assert_eq!(limited.in_use_bytes(), 0);
        assert_eq!(
            limited.peak_bytes(),
            one + 4 * looped,
            "each batch admits one column and four loops together"
        );
    }
}
