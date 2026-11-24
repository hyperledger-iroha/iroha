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
//! A fixed-window Straus sum:
//!
//! - each public base gets a table of its successive multiples, built with
//!   complete doublings and mixed additions, then normalised to affine form
//!   with one constant-time inversion per chunk;
//! - four-bit windows use signed radix-16 digits from overlapping public bit
//!   positions. Their magnitudes use the table from 0 through 8. Smaller
//!   windows use unsigned digits and the table from 0 through `2^w - 1`;
//! - a digit selects its entry by scanning every possible nonzero digit with
//!   constant-time conditional selection (the identity for a zero digit);
//! - each window's selected entries over the chunk are summed by a balanced
//!   tree of complete affine additions. Every tree level of a group of
//!   columns shares one constant-time inversion (Montgomery's trick), and each
//!   addition selects the chord, tangent, identity-operand or opposite case in
//!   constant time, so equal, opposite and identity operands are exact;
//! - windows run from the most significant, and each window's `w` complete
//!   doublings are shared by all bases of the chunk.
//!
//! The allocation-free fallback path keeps the original chain of complete
//! projective additions (`add_chunk`), which also serves as the tests'
//! reference. Both return the same group elements.
//!
//! A column may carry a public support ([`msm_secret_ct_bounded_columns_supported_cancellable`]):
//! it then reads only the bases its support names, by public layout.
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
//! chunk, never the scalars. A base is skipped only by a caller's public
//! support. Identity bases and zero digits carry an identity flag selected in
//! constant time with each entry; the tree additions decide their cases from
//! those flags and constant-time coordinate equality, never by branching. The
//! shared inversion is the fixed-exponent power, and its input is never zero
//! (see `affine_pair_terms`).
//!
//! Canonical limbs and partial sums live in [`Zeroizing`] buffers, which are
//! wiped on every exit, including cancellation, refusal and unwinding. Tables
//! hold public multiples of public bases and are wiped as well. A digit held in
//! a register is not wiped.
//!
//! # Memory
//!
//! A claiming loop holds one chunk's tables (up to nine projective and nine
//! affine points per base), the chunk's canonical limbs, one accumulator per
//! column, and one group's selected entries with its per-level denominators,
//! numerators and inversion prefixes (about 1.1 MiB at 128 bases and
//! four-bit windows); the merged sums add one point per column. A plan never exceeds the per-kernel
//! [`MemoryBudget`] and is admitted against the [`SharedMemoryBudget`] without
//! waiting. Contention lowers the loop count and then the chunk size. When not
//! even one base on one loop can be admitted, the sum runs on the calling
//! thread with a stack table and no heap scratch. Only a per-kernel budget
//! below that single-base plan is an error.
//!
//! Throughput is deliberately not claimed here; measure it against the
//! variable-time kernels on the target devices.

use ff::Field;
use group::CurveAffine;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};
use zeroize::{Zeroize, Zeroizing};

use super::pippenger::window_bits;
use super::{BoundedSecretScalars, MemoryBudget, MsmError, ScratchReservation, SharedMemoryBudget};
use crate::curve::{PastaAffine, PastaCurve};
use crate::field::PastaField;
use crate::{CancellationToken, Cancelled};

#[cfg(test)]
#[path = "spartan_ct_tests.rs"]
mod ct_tests;

/// Widest window, using magnitudes up to eight.
pub const MAX_CT_WINDOW: usize = 4;

/// Most bases sharing one set of doublings in a claimed chunk.
pub const CT_CHUNK_BASES: usize = 128;

/// Window sums whose batch-affine trees share one inversion per level (one
/// full-width four-bit column). Wider single columns form a group of their
/// own.
pub const CT_GROUP_WINDOWS: usize = 64;

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

/// [`msm_secret_ct_bounded_columns_cancellable`] where each column also has a
/// public support: bit `i` of `supports[c]` (bit `i % 64` of word `i / 64`)
/// is set when scalar `i` of column `c` may be nonzero.
///
/// A column reads, and adds windows for, only the bases its support names;
/// the others are skipped. The support must be public structure (a circuit's
/// layout of present cells), never derived from the scalars: which positions
/// are skipped is visible in the timing. Every scalar outside the support
/// must be zero; one that is not is dropped from the sum. Tables are still
/// built once per chunk and shared by every column.
///
/// # Errors
/// As [`msm_secret_ct_bounded_columns_cancellable`], and
/// [`MsmError::LengthMismatch`] when there is not one support per column or
/// a support has fewer bits than `bases`.
pub fn msm_secret_ct_bounded_columns_supported_cancellable<C: PastaCurve>(
    columns: &[BoundedSecretScalars<'_, C::ScalarExt>],
    supports: &[&[u64]],
    bases: &[C::AffineExt],
    budget: MemoryBudget,
    shared: &SharedMemoryBudget,
    cancellation: Option<&CancellationToken>,
) -> Result<Vec<C>, MsmError> {
    if supports.len() != columns.len() {
        return Err(MsmError::LengthMismatch(crate::LengthMismatch {
            left: supports.len(),
            right: columns.len(),
        }));
    }
    if let Some(support) = supports
        .iter()
        .find(|support| support.len().saturating_mul(64) < bases.len())
    {
        return Err(MsmError::LengthMismatch(crate::LengthMismatch {
            left: support.len().saturating_mul(64),
            right: bases.len(),
        }));
    }
    let columns: Vec<(&[C::ScalarExt], usize)> = columns
        .iter()
        .map(|column| (column.scalars, column.bits))
        .collect();
    let mut sums = Zeroizing::new(vec![C::identity(); columns.len()]);
    columns_into_supported::<C>(
        &columns,
        Some(supports),
        bases,
        budget,
        shared,
        cancellation,
        &mut sums[..],
    )?;
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

/// Windows a column of public bound `bits` reads at width `window`,
/// including the signed radix-16 carry window; zero for a zero bound.
const fn column_windows(bits: usize, window: usize) -> usize {
    if bits == 0 {
        0
    } else {
        (bits + (window == 4) as usize).div_ceil(window)
    }
}

/// Window sums one batch-affine group reduces together: at least
/// [`CT_GROUP_WINDOWS`], and always the widest (255-bit) column at `window`,
/// so every column fits one group. A function of the public window only.
const fn group_windows(window: usize) -> usize {
    let full = column_windows(FULL_BITS, window);
    if full > CT_GROUP_WINDOWS {
        full
    } else {
        CT_GROUP_WINDOWS
    }
}

/// Heap scratch of one claiming loop: a chunk's projective tables and
/// canonical limbs, one accumulator per column, and the batch-affine buffers
/// ([`batch_scratch_bytes`]).
fn loop_bytes<C: PastaCurve>(chunk: usize, window: usize, columns: usize) -> Option<usize> {
    let tables = chunk
        .checked_mul(table_entries(window))?
        .checked_mul(size_of::<C>())?;
    let limbs = chunk.checked_mul(LIMB_BYTES)?;
    tables
        .checked_add(limbs)?
        .checked_add(batch_scratch_bytes::<C>(chunk, window)?)?
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
    columns_into_supported::<C>(columns, None, bases, budget, shared, cancellation, sums)
}

/// [`columns_into`] with optional public column supports (see
/// [`msm_secret_ct_bounded_columns_supported_cancellable`]); the caller has
/// checked their count and lengths.
fn columns_into_supported<C: PastaCurve>(
    columns: &[(&[C::ScalarExt], usize)],
    supports: Option<&[&[u64]]>,
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
            let result = run_plan::<C>(columns, supports, bases, plan, cancellation, sums);
            // Every loop joined, and its buffers were wiped and freed, inside
            // `run_plan`; the reservation outlives them.
            drop(scratch);
            result
        }
        // The complete kernel reads every base: a skipped position is zero,
        // so reading it adds the identity.
        None => run_stack::<C>(columns, bases, window, cancellation, sums),
    }
}

/// Runs an admitted plan: `plan.loops` claiming loops take the next chunk
/// until none is left, each adding its chunks into its own accumulators.
/// Group addition is exact, so the sums do not depend on which loop ran which
/// chunk. A single loop runs on the calling thread.
fn run_plan<C: PastaCurve>(
    columns: &[(&[C::ScalarExt], usize)],
    supports: Option<&[&[u64]]>,
    bases: &[C::AffineExt],
    plan: CtPlan,
    cancellation: Option<&CancellationToken>,
    sums: &mut [C],
) -> Result<(), MsmError> {
    let n = bases.len();
    let chunks = n.div_ceil(plan.chunk);
    let next = AtomicUsize::new(0);
    let claim = || -> Result<Zeroizing<Vec<C>>, Cancelled> {
        let entries = plan.chunk * table_entries(plan.window);
        let mut tables = Zeroizing::new(vec![C::identity(); entries]);
        let mut limbs = Zeroizing::new(vec![[0_u64; 4]; plan.chunk]);
        let mut partial = Zeroizing::new(vec![C::identity(); columns.len()]);
        let mut scratch = BatchScratch::<C>::new(plan.chunk, plan.window);
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
            add_chunk_batch::<C>(
                columns,
                supports,
                &bases[start..end],
                start,
                plan.window,
                &mut chunk,
                &mut scratch,
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

/// Identity flag of a selected entry or tree sum (`1` for the identity).
const IDENTITY: u8 = 1;
/// Pair flags of one tree addition: the left operand is the identity.
const LEFT_IDENTITY: u8 = 1;
/// Pair flags: the right operand is the identity.
const RIGHT_IDENTITY: u8 = 2;
/// Pair flags: the operands are finite and opposite, so the sum is the identity.
const OPPOSITE: u8 = 4;

/// One claiming loop's batch-affine buffers: the chunk's affine tables and
/// their identity flags, a group's selected entries and identity flags
/// (`group_windows(window) * chunk` each), and one denominator, numerator,
/// inversion prefix and flag byte per tree addition of a level.
///
/// Every buffer derived from a digit (selections and their flags,
/// denominators, numerators, prefixes, pair flags) is wiped on drop; the
/// affine tables and their flags are public multiples of public bases and are
/// wiped as well.
struct BatchScratch<C: PastaCurve> {
    affine: Zeroizing<Vec<C::AffineExt>>,
    table_identity: Zeroizing<Vec<u8>>,
    selected: Zeroizing<Vec<C::AffineExt>>,
    selected_identity: Zeroizing<Vec<u8>>,
    /// Entries in each window sum's lane: the column's present bases in
    /// the chunk (public).
    lane_lengths: Vec<usize>,
    denominators: Zeroizing<Vec<C::Base>>,
    numerators: Zeroizing<Vec<C::Base>>,
    prefixes: Zeroizing<Vec<C::Base>>,
    pair_flags: Zeroizing<Vec<u8>>,
}

impl<C: PastaCurve> BatchScratch<C> {
    /// The buffers [`loop_bytes`] accounts for, sized by public plan data.
    fn new(chunk: usize, window: usize) -> Self {
        let entries = chunk * table_entries(window);
        let slots = group_windows(window) * chunk;
        let pairs = group_windows(window) * chunk.div_ceil(2);
        Self {
            affine: Zeroizing::new(vec![C::AffineExt::default(); entries]),
            table_identity: Zeroizing::new(vec![0; entries]),
            selected: Zeroizing::new(vec![C::AffineExt::default(); slots]),
            selected_identity: Zeroizing::new(vec![0; slots]),
            lane_lengths: vec![0; group_windows(window)],
            denominators: Zeroizing::new(vec![C::Base::ZERO; pairs]),
            numerators: Zeroizing::new(vec![C::Base::ZERO; pairs]),
            prefixes: Zeroizing::new(vec![C::Base::ZERO; pairs]),
            pair_flags: Zeroizing::new(vec![0; pairs]),
        }
    }
}

/// Bytes of [`BatchScratch`] per claiming loop.
fn batch_scratch_bytes<C: PastaCurve>(chunk: usize, window: usize) -> Option<usize> {
    let entries = chunk.checked_mul(table_entries(window))?;
    let slots = group_windows(window).checked_mul(chunk)?;
    let pairs = group_windows(window).checked_mul(chunk.div_ceil(2))?;
    entries
        .checked_mul(size_of::<C::AffineExt>() + 1)?
        .checked_add(slots.checked_mul(size_of::<C::AffineExt>() + 1)?)?
        .checked_add(group_windows(window).checked_mul(size_of::<usize>())?)?
        .checked_add(pairs.checked_mul(3 * size_of::<C::Base>() + 1)?)
}

/// [`add_chunk`] with batch-affine window sums: the same signed or unsigned
/// digits, the same whole-table constant-time scan (over the affine form of
/// the same public table) and the same Horner schedule of `window` doublings
/// per window, but each window's sum over the chunk is a balanced tree of
/// affine additions instead of a chain of complete projective additions.
///
/// Columns are grouped by their public window counts into groups of at most
/// [`group_windows`] window sums. Every tree level of a group shares one
/// constant-time field inversion (Montgomery's trick over all of the level's
/// additions in every window of every column of the group), so an affine
/// addition costs about six multiplications instead of the complete
/// projective formula's twelve.
///
/// Every tree addition is complete and branch-free ([`affine_pair_terms`],
/// [`affine_pair_sum`]): identity operands (zero digits, identity bases),
/// equal operands (doubling) and opposite operands (identity) are decided by
/// constant-time equality and selected, never branched on. Control flow and
/// every address depend only on the chunk length, the window, the public
/// bounds and the column count. The sums are exactly those of [`add_chunk`]:
/// group addition is exact.
#[allow(clippy::too_many_arguments)]
fn add_chunk_batch<C: PastaCurve>(
    columns: &[(&[C::ScalarExt], usize)],
    supports: Option<&[&[u64]]>,
    bases: &[C::AffineExt],
    start: usize,
    window: usize,
    chunk: &mut Chunk<'_, C>,
    scratch: &mut BatchScratch<C>,
    sums: &mut [C],
    cancellation: Option<&CancellationToken>,
) -> Result<(), Cancelled> {
    let signed = window == 4;
    let entries = table_entries(window);
    let n = bases.len();
    let tables = &mut chunk.tables[..n * entries];
    let limbs = &mut chunk.limbs[..n];
    // Public multiples of public bases, exactly as `add_chunk` builds them,
    // then their affine forms (identities map to the affine identity).
    for (table, base) in tables.chunks_exact_mut(entries).zip(bases) {
        table[0] = C::identity();
        table[1] = base.to_curve();
        for i in 1..entries / 2 {
            let even = table[i].double();
            table[2 * i] = even;
            table[2 * i + 1] = even + *base;
        }
        if signed {
            table[8] = table[4].double();
        }
    }
    let BatchScratch {
        affine,
        table_identity,
        selected: selections,
        selected_identity,
        lane_lengths,
        denominators,
        numerators,
        prefixes,
        pair_flags,
    } = scratch;
    let affine = &mut affine[..n * entries];
    // `Curve::batch_normalize` is the constant-time one-inversion normaliser.
    <C as group::Curve>::batch_normalize(tables, affine);
    let affine = &*affine;
    let table_identity = &mut table_identity[..n * entries];
    for (flag, point) in table_identity.iter_mut().zip(affine) {
        *flag = point.is_identity().unwrap_u8();
    }
    let table_identity = &*table_identity;
    let capacity = group_windows(window);
    let mut first = 0;
    while first < columns.len() {
        // The group: consecutive columns whose window sums fit `capacity`
        // (public bounds only). One column always fits.
        let mut last = first;
        let mut slots = 0;
        while last < columns.len() {
            let windows = column_windows(columns[last].1, window);
            if last > first && slots + windows > capacity {
                break;
            }
            slots += windows;
            last += 1;
        }
        CancellationToken::checkpoint(cancellation)?;
        let points = &mut selections[..slots * n];
        let identity = &mut selected_identity[..slots * n];
        let lengths = &mut lane_lengths[..slots];
        let mut slot = 0;
        for (c, &(scalars, bits)) in columns.iter().enumerate().take(last).skip(first) {
            // A zero bound admits only zero: public, and no scalar is read.
            let windows = column_windows(bits, window);
            if windows == 0 {
                continue;
            }
            // The column's present bases in this chunk, from its public
            // support only; without one, every base.
            let support = supports.map(|all| all[c]);
            let present = |j: usize| support.is_none_or(|bits| bits_set(bits, start + j));
            for (j, (limb, scalar)) in limbs.iter_mut().zip(&scalars[start..start + n]).enumerate()
            {
                if present(j) {
                    *limb = PastaField::to_canonical_limbs(scalar);
                }
            }
            let mut length = 0;
            for w in 0..windows {
                let lane = (slot + w) * n..(slot + w + 1) * n;
                length = select_window::<C>(
                    limbs,
                    affine,
                    table_identity,
                    entries,
                    w,
                    window,
                    bits,
                    &present,
                    &mut points[lane.clone()],
                    &mut identity[lane],
                );
            }
            lengths[slot..slot + windows].fill(length);
            limbs.iter_mut().zeroize();
            slot += windows;
        }
        reduce_window_sums::<C>(
            points,
            identity,
            n,
            lengths,
            TreeScratch {
                denominators: &mut denominators[..],
                numerators: &mut numerators[..],
                prefixes: &mut prefixes[..],
                flags: &mut pair_flags[..],
            },
        );
        let points = &*points;
        let mut slot = 0;
        for (&(_, bits), sum) in columns[first..last].iter().zip(&mut sums[first..last]) {
            let windows = column_windows(bits, window);
            if windows == 0 {
                continue;
            }
            let mut column = Zeroizing::new(C::identity());
            for w in (0..windows).rev() {
                if w + 1 < windows {
                    for _ in 0..window {
                        *column = column.double();
                    }
                }
                // Complete, identity-safe mixed addition of the window sum
                // (an identity sum has the affine identity's coordinates).
                *column += points[(slot + w) * n];
            }
            *sum += *column;
            slot += windows;
        }
        first = last;
    }
    Ok(())
}

/// Per tree level: one denominator, numerator, prefix and flag byte per
/// addition.
struct TreeScratch<'a, F> {
    denominators: &'a mut [F],
    numerators: &'a mut [F],
    prefixes: &'a mut [F],
    flags: &'a mut [u8],
}

/// Whether bit `i` of a public support bitmap is set.
#[inline]
fn bits_set(bits: &[u64], i: usize) -> bool {
    bits[i / 64] >> (i % 64) & 1 == 1
}

/// Selects, for every present base of the chunk (in order, compacted to the
/// front of `out`), its window-`w` digit's table entry (affine) and identity
/// flag, negated for a negative signed digit: the digit recoding and the
/// whole-table scan of [`add_chunk`], entry for entry. Returns the number of
/// present bases. `present` is public structure.
#[allow(clippy::too_many_arguments)]
fn select_window<C: PastaCurve>(
    limbs: &[[u64; 4]],
    affine: &[C::AffineExt],
    table_identity: &[u8],
    entries: usize,
    w: usize,
    window: usize,
    bits: usize,
    present: &impl Fn(usize) -> bool,
    out: &mut [C::AffineExt],
    out_identity: &mut [u8],
) -> usize {
    let signed = window == 4;
    let start_bit = w * window;
    let digit_bits = window.min(bits.saturating_sub(start_bit));
    // Every scan length and every address below depend only on the public
    // window index, bound and support. A shorter top window includes the
    // possible carry from the preceding signed digit.
    let largest = if signed && digit_bits == window {
        8
    } else {
        (1_usize << digit_bits) - 1 + usize::from(signed && w != 0)
    };
    let mut length = 0;
    for (j, ((limb, table), flags)) in limbs
        .iter()
        .zip(affine.chunks_exact(entries))
        .zip(table_identity.chunks_exact(entries))
        .enumerate()
    {
        if !present(j) {
            continue;
        }
        let raw = window_bits(limb, start_bit, digit_bits);
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
        // A zero digit selects nothing: the identity.
        let mut chosen = C::AffineExt::default();
        let mut chosen_identity = IDENTITY;
        for ((index, candidate), flag) in
            (1_u64..).zip(&table[1..=largest]).zip(&flags[1..=largest])
        {
            let hit = index.ct_eq(&magnitude);
            chosen = C::AffineExt::conditional_select(&chosen, candidate, hit);
            chosen_identity = u8::conditional_select(&chosen_identity, flag, hit);
        }
        if signed && digit_bits == window {
            chosen =
                C::AffineExt::conditional_select(&chosen, &(-chosen), Choice::from(sign as u8));
        }
        out[length] = chosen;
        out_identity[length] = chosen_identity;
        length += 1;
    }
    length
}

/// Reduces each lane `s` (`points[s * n..s * n + lengths[s]]`, with identity
/// flags in `identity`) to its sum at the lane's first position; an empty
/// lane becomes the identity.
///
/// Level by level, lane positions `2i` and `2i + 1` are added into `i` and an
/// odd last point moves to the next free position; the schedule depends only
/// on `n` and the public lane lengths. All additions of a level, in every
/// lane, share one constant-time inversion ([`invert_nonzero_batch`]).
fn reduce_window_sums<C: PastaCurve>(
    points: &mut [C::AffineExt],
    identity: &mut [u8],
    n: usize,
    lengths: &mut [usize],
    scratch: TreeScratch<'_, C::Base>,
) {
    for (s, length) in lengths.iter().enumerate() {
        if *length == 0 {
            points[s * n] = C::AffineExt::default();
            identity[s * n] = IDENTITY;
        }
    }
    loop {
        let count: usize = lengths.iter().map(|m| m / 2).sum();
        if count == 0 {
            break;
        }
        let denominators = &mut scratch.denominators[..count];
        let numerators = &mut scratch.numerators[..count];
        let flags = &mut scratch.flags[..count];
        let mut k = 0;
        for (s, &m) in lengths.iter().enumerate() {
            let lane = s * n;
            for i in 0..m / 2 {
                let (a, b) = (lane + 2 * i, lane + 2 * i + 1);
                (denominators[k], numerators[k], flags[k]) =
                    affine_pair_terms::<C>(&points[a], &points[b], identity[a], identity[b]);
                k += 1;
            }
        }
        invert_nonzero_batch(denominators, &mut scratch.prefixes[..count]);
        let mut k = 0;
        for (s, m) in lengths.iter_mut().enumerate() {
            let lane = s * n;
            let half = *m / 2;
            for i in 0..half {
                let (a, b) = (lane + 2 * i, lane + 2 * i + 1);
                let (sum, sum_identity) = affine_pair_sum::<C>(
                    &points[a],
                    &points[b],
                    numerators[k] * denominators[k],
                    flags[k],
                );
                points[lane + i] = sum;
                identity[lane + i] = sum_identity;
                k += 1;
            }
            if *m % 2 == 1 {
                points[lane + half] = points[lane + *m - 1];
                identity[lane + half] = identity[lane + *m - 1];
            }
            *m = half + *m % 2;
        }
    }
}

/// The slope terms of the complete affine addition `a + b` on `y^2 = x^3 + 5`
/// as `(denominator, numerator, flags)`, in constant time:
///
/// - distinct `x`: `(x_b - x_a, y_b - y_a)`, the chord;
/// - `a == b` (finite): `(2 y_a, 3 x_a^2)`, the tangent;
/// - an identity operand, or `a == -b`: denominator `1`, and the slope is
///   never used ([`affine_pair_sum`] selects the other operand or the
///   identity).
///
/// The denominator is never zero: a finite Pasta point has `y != 0`, since
/// both groups have odd prime order and so contain no point of order two.
/// `flags` records the identity operands and the opposite case for
/// [`affine_pair_sum`]. Every case is decided by constant-time equality and
/// selection.
fn affine_pair_terms<C: PastaCurve>(
    a: &C::AffineExt,
    b: &C::AffineExt,
    a_identity: u8,
    b_identity: u8,
) -> (C::Base, C::Base, u8) {
    let (xa, ya, xb, yb) = (a.x(), a.y(), b.x(), b.y());
    // Flags combine as 0/1 bytes: each `Choice` operation is an optimisation
    // barrier, so the cases are formed arithmetically and turned into a
    // `Choice` only where a selection reads them.
    let either = a_identity | b_identity;
    let same_x = xa.ct_eq(&xb).unwrap_u8();
    let same_y = ya.ct_eq(&yb).unwrap_u8();
    let finite_same_x = (either ^ 1) & same_x;
    let tangent = Choice::from(finite_same_x & same_y);
    let opposite = finite_same_x & (same_y ^ 1);
    let square = xa.square();
    let numerator = C::Base::conditional_select(&(yb - ya), &(square.double() + square), tangent);
    let denominator = C::Base::conditional_select(&(xb - xa), &ya.double(), tangent);
    let denominator =
        C::Base::conditional_select(&denominator, &C::Base::ONE, Choice::from(either | opposite));
    let flags =
        (a_identity * LEFT_IDENTITY) | (b_identity * RIGHT_IDENTITY) | (opposite * OPPOSITE);
    (denominator, numerator, flags)
}

/// The complete affine sum `a + b` and its identity flag from the slope
/// `lambda` and `flags` of [`affine_pair_terms`]: the chord or tangent
/// result, except `b` when `a` is the identity, `a` when `b` is, and the
/// identity when `a == -b`. Every case is computed and selected in constant
/// time; an identity result has the affine identity's coordinates.
fn affine_pair_sum<C: PastaCurve>(
    a: &C::AffineExt,
    b: &C::AffineExt,
    lambda: C::Base,
    flags: u8,
) -> (C::AffineExt, u8) {
    let a_identity = flags & LEFT_IDENTITY;
    let b_identity = (flags & RIGHT_IDENTITY) >> 1;
    let opposite = (flags & OPPOSITE) >> 2;
    let (xa, ya, xb) = (a.x(), a.y(), b.x());
    let x3 = lambda.square() - xa - xb;
    let y3 = lambda * (xa - x3) - ya;
    let sum = crate::curve::affine_unchecked::<C::AffineExt>(x3, y3);
    let sum = C::AffineExt::conditional_select(&sum, b, Choice::from(a_identity));
    let sum = C::AffineExt::conditional_select(&sum, a, Choice::from(b_identity));
    let sum =
        C::AffineExt::conditional_select(&sum, &C::AffineExt::default(), Choice::from(opposite));
    (sum, (a_identity & b_identity) | opposite)
}

/// Replaces every value by its inverse with one constant-time inversion
/// (Montgomery's trick): `prefixes` receives the running products.
///
/// Every value must be nonzero ([`affine_pair_terms`] guarantees it). The
/// running product is then nonzero, and the field inversion is the
/// fixed-exponent power, whose operation sequence does not depend on the
/// value. `unwrap_or` keeps the step branch-free; a zero could only come
/// from a broken caller invariant and would give a wrong sum, never a
/// value-dependent branch.
fn invert_nonzero_batch<F: PastaField>(values: &mut [F], prefixes: &mut [F]) {
    let mut run = F::ONE;
    for (value, prefix) in values.iter().zip(prefixes.iter_mut()) {
        *prefix = run;
        run *= *value;
    }
    let mut inverse = run.invert().unwrap_or(F::ZERO);
    for (value, prefix) in values.iter_mut().zip(prefixes.iter()).rev() {
        let this = inverse * *prefix;
        inverse *= *value;
        *value = this;
    }
}
