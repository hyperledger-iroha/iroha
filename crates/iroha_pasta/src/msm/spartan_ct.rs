//! Constant-time multi-scalar multiplication for private witness scalars.
//!
//! The Pippenger kernels behind [`super::msm_secret`] and the bounded secret
//! entry points pick a bucket by each scalar digit and skip zero digits, so
//! their memory accesses and timing follow the digits. This module is the
//! reference kernel for private scalars that must not leak that way. It was
//! introduced for the Spartan prover's witness rows, round values, opening
//! vectors and generator restructuring; nothing in it is specific to Spartan.
//!
//! # Algorithm
//!
//! A fixed-window Straus sum over complete projective formulas:
//!
//! - each public base gets a table of its successive multiples, built with
//!   complete doublings and mixed additions;
//! - four-bit windows use signed radix-16 digits from overlapping public bit
//!   positions. Their magnitudes use the table from 0 through 8. Smaller
//!   windows use unsigned digits and the table from 0 through `2^w - 1`;
//! - a digit selects its entry by scanning every possible nonzero digit with
//!   constant-time conditional selection, and the entry (the identity for a
//!   zero digit) is added with the complete addition formula;
//! - windows run from the most significant, and each window's `w` complete
//!   doublings are shared by all bases of the chunk.
//!
//! A column's sum is `sum_j scalars[j] * bases[j]` exactly. A column's public
//! bound fixes the number of windows, including a final signed radix-16 carry.
//! The caller must have
//! established the bound (see [`BoundedSecretScalars`]): digits above it are
//! never read.
//!
//! # Timing posture
//!
//! Window width, number of windows, chunk size, number of claiming loops and
//! the table entries scanned are functions of public data only: the input
//! length, the caller's public bit bounds, the column count, the Rayon pool
//! size and scratch admission. No bucket or table entry is addressed by a
//! digit, no digit, scalar or column is skipped because of its value, and no
//! width is derived from a value. Worker timing decides which loop claims which
//! chunk, never the scalars. Identity bases need no special case: their table
//! holds identities, which the complete formulas add like any other point.
//!
//! Canonical limbs and partial sums live in [`Zeroizing`] buffers, which are
//! wiped on every exit, including cancellation, refusal and unwinding. Tables
//! hold public multiples of public bases and are wiped as well. A digit held in
//! a register is not wiped.
//!
//! # Memory
//!
//! A claiming loop holds one chunk's tables (up to nine projective points per base),
//! the chunk's canonical limbs and one accumulator per column; the merged sums
//! add one point per column. A plan never exceeds the per-kernel
//! [`MemoryBudget`] and is admitted against the [`SharedMemoryBudget`] without
//! waiting. Contention lowers the loop count and then the chunk size. When not
//! even one base on one loop can be admitted, the sum runs on the calling
//! thread with a stack table and no heap scratch. Only a per-kernel budget
//! below that single-base plan is an error.
//!
//! Throughput is deliberately not claimed here; measure it against the
//! variable-time kernels on the target devices.

use group::CurveAffine;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};

use super::pippenger::window_bits;
use super::{BoundedSecretScalars, MemoryBudget, MsmError, ScratchReservation, SharedMemoryBudget};
use crate::curve::PastaCurve;
use crate::field::PastaField;
use crate::{CancellationToken, Cancelled};

#[cfg(test)]
#[path = "spartan_ct_tests.rs"]
mod ct_tests;

/// Widest window, using magnitudes up to eight.
pub const MAX_CT_WINDOW: usize = 4;

/// Most bases sharing one set of doublings in a claimed chunk.
pub const CT_CHUNK_BASES: usize = 128;

/// Bits of every canonical scalar: both Pasta moduli are below `2^255`.
const FULL_BITS: usize = 255;

/// Bytes of one scalar's canonical limbs.
const LIMB_BYTES: usize = size_of::<[u64; 4]>();

/// Constant-time `sum_i scalars[i] * bases[i]` for full-width private scalars.
///
/// Every scalar is read in all windows of 255 bits. The result is the exact
/// group element that [`super::msm_secret`] and [`super::msm_naive`] return.
///
/// # Errors
/// [`MsmError::LengthMismatch`] for unequal lengths, [`MsmError::TooLarge`]
/// beyond `u32::MAX` bases, [`MsmError::Budget`] when one base on one loop
/// exceeds `budget`, or [`MsmError::Cancelled`] after every loop has joined
/// and all private scratch is wiped.
pub fn msm_secret_ct_cancellable<C: PastaCurve>(
    scalars: &[C::ScalarExt],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<C, MsmError> {
    let mut sum = Zeroizing::new([C::identity()]);
    columns_into::<C>(
        &[(scalars, FULL_BITS)],
        bases,
        budget,
        shared,
        cancellation,
        &mut sum[..],
    )?;
    Ok(sum[0])
}

/// Constant-time MSM of private scalars below an independently public bound.
///
/// The public bound determines every window, including the radix-16 carry.
/// Construction checks that bound in constant time. A zero bound gives the
/// identity without reading any scalar.
///
/// # Errors
/// As [`msm_secret_ct_cancellable`].
pub fn msm_secret_ct_bounded_cancellable<C: PastaCurve>(
    scalars: &BoundedSecretScalars<'_, C::ScalarExt>,
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<C, MsmError> {
    let mut sum = Zeroizing::new([C::identity()]);
    columns_into::<C>(
        &[(scalars.scalars, scalars.bits)],
        bases,
        budget,
        shared,
        cancellation,
        &mut sum[..],
    )?;
    Ok(sum[0])
}

/// One constant-time MSM per bounded column over the same `bases`, in column
/// order.
///
/// Each chunk's tables are built once and read by every column, so their cost
/// is shared, and each column reads only its own public number of windows.
/// One window width, chosen from the public bounds, serves the whole call.
///
/// # Errors
/// As [`msm_secret_ct_cancellable`]. Every column length is checked before
/// any work.
pub fn msm_secret_ct_bounded_columns_cancellable<C: PastaCurve>(
    columns: &[BoundedSecretScalars<'_, C::ScalarExt>],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<Vec<C>, MsmError> {
    let columns: Vec<(&[C::ScalarExt], usize)> = columns
        .iter()
        .map(|column| (column.scalars, column.bits))
        .collect();
    let mut sums = Zeroizing::new(vec![C::identity(); columns.len()]);
    columns_into::<C>(&columns, bases, budget, shared, cancellation, &mut sums[..])?;
    Ok(sums.to_vec())
}

/// The window width for columns of public bounds `bits`.
///
/// Minimises a public operation count in units of 1/32 complete addition:
/// `2^w` table additions per base under the unsigned cost model, and per
/// column `ceil(bits / w)` additions, each after a scan of `2^w` entries
/// counted as 1/32 addition per entry. Ties take the narrower window.
pub(crate) fn ct_window(bits: impl IntoIterator<Item = usize>) -> usize {
    let mut lookups = [0_usize; MAX_CT_WINDOW];
    for b in bits {
        for (window, total) in (1..).zip(lookups.iter_mut()) {
            *total = total.saturating_add(b.div_ceil(window));
        }
    }
    (1..=MAX_CT_WINDOW)
        .zip(lookups)
        .min_by_key(|&(window, lookups)| {
            let entries = 1_usize << window;
            (32 * entries).saturating_add(lookups.saturating_mul(32 + entries))
        })
        .map_or(MAX_CT_WINDOW, |(window, _)| window)
}

/// An admitted plan: window width, bases per chunk, claiming loops and the
/// reserved scratch bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CtPlan {
    /// Window width `w`.
    pub(crate) window: usize,
    /// Bases per claimed chunk.
    pub(crate) chunk: usize,
    /// Claiming loops running at once.
    pub(crate) loops: usize,
    /// Scratch bytes reserved for the plan.
    pub(crate) bytes: usize,
}

/// Four-bit digits range from -8 through 8. Narrower unsigned digits
/// retain their original tables and public window schedule.
const fn table_entries(window: usize) -> usize {
    if window == 4 { 9 } else { 1 << window }
}

/// Heap scratch of one claiming loop: a chunk's tables and canonical limbs,
/// and one accumulator per column.
fn loop_bytes<C: PastaCurve>(chunk: usize, window: usize, columns: usize) -> Option<usize> {
    let tables = chunk
        .checked_mul(table_entries(window))?
        .checked_mul(size_of::<C>())?;
    let limbs = chunk.checked_mul(LIMB_BYTES)?;
    tables
        .checked_add(limbs)?
        .checked_add(columns.checked_mul(size_of::<C>())?)
}

/// All scratch of a plan: its loops and the merged sum of every column.
pub(crate) fn plan_bytes<C: PastaCurve>(
    chunk: usize,
    window: usize,
    columns: usize,
    loops: usize,
) -> Option<usize> {
    loop_bytes::<C>(chunk, window, columns)?
        .checked_mul(loops)?
        .checked_add(columns.checked_mul(size_of::<C>())?)
}

/// Admits the largest plan that fits, without waiting.
///
/// Starts from [`CT_CHUNK_BASES`] bases per chunk and one loop per worker,
/// and halves the loops, then the chunk, until a reservation succeeds. Every
/// candidate depends only on public lengths, the window, the pool size, the
/// budget and contention.
///
/// `Err` when one base on one loop exceeds the per-kernel `budget`; `Ok(None)`
/// when it fits the budget but the shared ceiling has no room for it now.
fn admit<C: PastaCurve>(
    n: usize,
    window: usize,
    columns: usize,
    threads: usize,
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
) -> Result<Option<(CtPlan, ScratchReservation<'_>)>, MsmError> {
    budget.check(plan_bytes::<C>(1, window, columns, 1).unwrap_or(usize::MAX))?;
    let fixed = columns.saturating_mul(size_of::<C>());
    let mut chunk = n.clamp(1, CT_CHUNK_BASES);
    loop {
        let per_loop = loop_bytes::<C>(chunk, window, columns).unwrap_or(usize::MAX);
        let room = budget
            .bytes()
            .min(shared.available_bytes())
            .saturating_sub(fixed);
        let mut loops = threads
            .max(1)
            .min(n.div_ceil(chunk))
            .min(room / per_loop.max(1));
        while loops > 0 {
            if let Some(bytes) = plan_bytes::<C>(chunk, window, columns, loops)
                && let Some(scratch) = shared.try_reserve(bytes)
            {
                let plan = CtPlan {
                    window,
                    chunk,
                    loops,
                    bytes,
                };
                return Ok(Some((plan, scratch)));
            }
            loops /= 2;
        }
        if chunk == 1 {
            return Ok(None);
        }
        chunk /= 2;
    }
}

/// `sums[c] += sum_i columns[c].0[i] * bases[i]` for every column, each given
/// as `(scalars, public bound)`.
///
/// On error `sums` holds no meaningful value; callers keep it in a
/// [`Zeroizing`] buffer and discard it.
pub(crate) fn columns_into<C: PastaCurve>(
    columns: &[(&[C::ScalarExt], usize)],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
    cancellation: Option<&CancellationToken>,
    sums: &mut [C],
) -> Result<(), MsmError> {
    CancellationToken::checkpoint(cancellation)?;
    if let Some((column, _)) = columns
        .iter()
        .find(|(column, _)| column.len() != bases.len())
    {
        return Err(MsmError::LengthMismatch(crate::LengthMismatch {
            left: column.len(),
            right: bases.len(),
        }));
    }
    if sums.len() != columns.len() {
        return Err(MsmError::LengthMismatch(crate::LengthMismatch {
            left: sums.len(),
            right: columns.len(),
        }));
    }
    if columns.iter().any(|&(_, bits)| bits > FULL_BITS) {
        return Err(MsmError::ScalarBound);
    }
    let n = bases.len();
    if u32::try_from(n).is_err() {
        return Err(MsmError::TooLarge { n });
    }
    // An empty input or zero public bounds add the identity, decided by public
    // lengths and bounds without reading any scalar.
    if n == 0 || columns.iter().all(|&(_, bits)| bits == 0) {
        return CancellationToken::checkpoint(cancellation).map_err(MsmError::from);
    }
    let window = ct_window(columns.iter().map(|&(_, bits)| bits));
    let threads = rayon::current_num_threads();
    match admit::<C>(n, window, columns.len(), threads, budget, shared)? {
        Some((plan, scratch)) => {
            debug_assert_eq!(scratch.bytes(), plan.bytes);
            let result = run_plan::<C>(columns, bases, plan, cancellation, sums);
            // Every loop joined, and its buffers were wiped and freed, inside
            // `run_plan`; the reservation outlives them.
            drop(scratch);
            result
        }
        None => run_stack::<C>(columns, bases, window, cancellation, sums),
    }
}

/// Runs an admitted plan: `plan.loops` claiming loops take the next chunk
/// until none is left, each adding its chunks into its own accumulators.
/// Group addition is exact, so the sums do not depend on which loop ran which
/// chunk. A single loop runs on the calling thread.
fn run_plan<C: PastaCurve>(
    columns: &[(&[C::ScalarExt], usize)],
    bases: &[C::AffineExt],
    plan: CtPlan,
    cancellation: Option<&CancellationToken>,
    sums: &mut [C],
) -> Result<(), MsmError> {
    let n = bases.len();
    let chunks = n.div_ceil(plan.chunk);
    let next = AtomicUsize::new(0);
    let claim = || -> Result<Zeroizing<Vec<C>>, Cancelled> {
        let mut tables =
            Zeroizing::new(vec![C::identity(); plan.chunk * table_entries(plan.window)]);
        let mut limbs = Zeroizing::new(vec![[0_u64; 4]; plan.chunk]);
        let mut partial = Zeroizing::new(vec![C::identity(); columns.len()]);
        loop {
            CancellationToken::checkpoint(cancellation)?;
            let index = next.fetch_add(1, Ordering::Relaxed);
            if index >= chunks {
                return Ok(partial);
            }
            let start = index * plan.chunk;
            let end = (start + plan.chunk).min(n);
            let mut chunk = Chunk {
                tables: &mut tables[..],
                limbs: &mut limbs[..],
            };
            add_chunk::<C>(
                columns,
                &bases[start..end],
                start,
                plan.window,
                &mut chunk,
                &mut partial[..],
                cancellation,
            )?;
        }
    };
    let partials: Vec<Result<Zeroizing<Vec<C>>, Cancelled>> = if plan.loops == 1 {
        vec![claim()]
    } else {
        (0..plan.loops).into_par_iter().map(|_| claim()).collect()
    };
    // Every loop has joined. A cancelled loop wiped its buffers on return, and
    // dropping `partials` wipes the other loops' sums.
    if partials.iter().any(Result::is_err) {
        return Err(MsmError::Cancelled);
    }
    CancellationToken::checkpoint(cancellation)?;
    for partial in partials.iter().flatten() {
        for (sum, value) in sums.iter_mut().zip(partial.iter()) {
            *sum += value;
        }
    }
    Ok(())
}

/// The allocation-free path when the shared ceiling refuses even one base on
/// one loop: one base at a time on the calling thread, with its table and
/// limbs on the stack, wiped on every exit.
fn run_stack<C: PastaCurve>(
    columns: &[(&[C::ScalarExt], usize)],
    bases: &[C::AffineExt],
    window: usize,
    cancellation: Option<&CancellationToken>,
    sums: &mut [C],
) -> Result<(), MsmError> {
    let mut tables = Zeroizing::new([C::identity(); table_entries(MAX_CT_WINDOW)]);
    let mut limbs = Zeroizing::new([[0_u64; 4]; 1]);
    for (start, base) in bases.iter().enumerate() {
        CancellationToken::checkpoint(cancellation)?;
        let mut chunk = Chunk {
            tables: &mut tables[..],
            limbs: &mut limbs[..],
        };
        add_chunk::<C>(
            columns,
            core::slice::from_ref(base),
            start,
            window,
            &mut chunk,
            sums,
            cancellation,
        )?;
    }
    CancellationToken::checkpoint(cancellation)?;
    Ok(())
}

/// One claiming loop's chunk buffers: `bases * table_entries(window)` table entries
/// and `bases` canonical limbs for the chunk being added.
struct Chunk<'a, C> {
    tables: &'a mut [C],
    limbs: &'a mut [[u64; 4]],
}

/// Adds `sum_j columns[c].0[start + j] * bases[j]` to `sums[c]` for every
/// column with a nonzero public bound; `bases` is the chunk.
///
/// Control flow and memory accesses depend only on the chunk length, the
/// window and the public bounds. The limbs are wiped after every column.
fn add_chunk<C: PastaCurve>(
    columns: &[(&[C::ScalarExt], usize)],
    bases: &[C::AffineExt],
    start: usize,
    window: usize,
    chunk: &mut Chunk<'_, C>,
    sums: &mut [C],
    cancellation: Option<&CancellationToken>,
) -> Result<(), Cancelled> {
    let signed = window == 4;
    let entries = table_entries(window);
    let tables = &mut chunk.tables[..bases.len() * entries];
    let limbs = &mut chunk.limbs[..bases.len()];
    // Public multiples of public bases: entry `k` of a base's table is `k P`.
    for (table, base) in tables.chunks_exact_mut(entries).zip(bases) {
        table[0] = C::identity();
        table[1] = base.to_curve();
        // (2i)P by doubling, (2i+1)P by complete mixed addition.
        for i in 1..entries / 2 {
            let even = table[i].double();
            table[2 * i] = even;
            table[2 * i + 1] = even + *base;
        }
        if signed {
            table[8] = table[4].double();
        }
    }
    let tables = &*tables;
    for (&(scalars, bits), sum) in columns.iter().zip(sums.iter_mut()) {
        // A zero bound admits only zero: public, and no scalar is read.
        if bits == 0 {
            continue;
        }
        let windows = (bits + usize::from(signed)).div_ceil(window);
        CancellationToken::checkpoint(cancellation)?;
        for (limb, scalar) in limbs.iter_mut().zip(&scalars[start..start + bases.len()]) {
            *limb = PastaField::to_canonical_limbs(scalar);
        }
        let mut column = Zeroizing::new(C::identity());
        let mut entry = Zeroizing::new(C::identity());
        for w in (0..windows).rev() {
            if w + 1 < windows {
                for _ in 0..window {
                    *column = column.double();
                }
            }
            let start_bit = w * window;
            let digit_bits = window.min(bits.saturating_sub(start_bit));
            // Every scan length and every address below depend only on the
            // public window index and bound. A shorter top window includes
            // the possible carry from the preceding signed digit.
            let largest = if signed && digit_bits == window {
                8
            } else {
                (1_usize << digit_bits) - 1 + usize::from(signed && w != 0)
            };
            for (limb, table) in limbs.iter().zip(tables.chunks_exact(entries)) {
                let raw = window_bits(limb, start_bit, digit_bits);
                // The previous window's high bit is its carry. Subtracting
                // radix * sign here and adding that carry to the next digit
                // preserves the integer, including the most significant bit.
                let (magnitude, sign) = if signed {
                    let carry = if w == 0 {
                        0
                    } else {
                        window_bits(limb, start_bit - 1, 1)
                    };
                    let sign = raw >> 3;
                    let value = (raw + carry).wrapping_sub(sign << 4);
                    ((value ^ 0_u64.wrapping_sub(sign)).wrapping_add(sign), sign)
                } else {
                    (raw, 0)
                };
                *entry = C::identity();
                for (index, candidate) in (1_u64..).zip(&table[1..=largest]) {
                    *entry = C::conditional_select(&*entry, candidate, index.ct_eq(&magnitude));
                }
                if signed && digit_bits == window {
                    *entry = C::conditional_select(
                        &*entry,
                        &(-*entry),
                        subtle::Choice::from(sign as u8),
                    );
                }
                *column += *entry;
            }
        }
        *sum += *column;
        limbs.iter_mut().zeroize();
    }
    Ok(())
}
