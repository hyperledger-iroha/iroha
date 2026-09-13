//! Consuming membership validation and ascending leftover-table construction for stored lookups.
//!
//! Exactly one sorted table occurrence is removed for each distinct sorted input. The remaining
//! occurrences are emitted in global ascending order, with a private exact count and ZERO padding.
//! All lookups must finish before a later stage can draw randomness. This owner retains the
//! original compressed banks, original key/parameters, advice, blinds, RNG and transcript.
//! The permutation continuation consumes these receipts through ordinary pair commitments.
//! TODO: consume those pairs to construct products, quotient and openings. This bounded
//! scratch transition is neither a complete stored proof nor an end-to-end RSS guarantee.

use super::{
    CoefficientPendingStoredIpaProverV1,
    lookup::{
        CompressedLookupV1, LookupCompressedPendingStoredIpaProverV1, StoredLookupErrorV1,
        validate_outputs,
    },
    lookup_sort::{
        Cursor, Encoded, LookupSortedPendingStoredIpaProverV1, Scalars, SortedLookupColumnV1,
        SortedLookupV1, write_tile,
    },
};
use crate::{
    arithmetic::CurveAffine,
    poly::{
        commitment::Params,
        stored_advice::{
            STORED_MAX_K_V1, STORED_SCALAR_BYTES_V1, STORED_SCALARS_PER_CHUNK_V1,
            StoredLookupSideV1, StoredPolynomialBasisV1, StoredPolynomialErrorV1,
            StoredPolynomialLayoutV1, StoredPolynomialProviderV1, StoredPolynomialRoleV1,
            StoredPolynomialSnapshotV1, StoredPolynomialWriterV1,
            assignment::StoredAssignmentFieldV1, phase::CoefficientStoredAdviceV1,
        },
    },
};
use ff::WithSmallOrderMulGroup;
use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

const TILE: usize = STORED_SCALARS_PER_CHUNK_V1;
type SnapshotOf<P> =
    <<P as StoredPolynomialProviderV1>::Writer as StoredPolynomialWriterV1>::Snapshot;

/// Retained scratch for one validated lookup, with counts inseparable from the original owner.
pub(super) struct MembershipLookupV1<S> {
    pub(super) input: SortedLookupColumnV1<S>,
    pub(super) leftover_table: SortedLookupColumnV1<S>,
    pub(super) distinct_inputs: usize,
    pub(super) leftover_rows: usize,
}

/// Original proof continuation plus membership-checked sorted inputs and ascending leftovers.
#[allow(dead_code)]
pub(crate) struct LookupMembershipPendingStoredIpaProverV1<
    'params,
    'instances,
    C,
    P,
    R,
    T,
    E,
    const QUERY_INSTANCE: bool,
    const INSTANCE_MASK: u64,
> where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    pub(super) compressed: LookupCompressedPendingStoredIpaProverV1<
        'params,
        'instances,
        C,
        P,
        R,
        T,
        E,
        QUERY_INSTANCE,
        INSTANCE_MASK,
    >,
    pub(super) usable_rows: usize,
    pub(super) lookups: Vec<MembershipLookupV1<SnapshotOf<P>>>,
}

fn reserved<T>(count: usize) -> Result<Vec<T>, StoredLookupErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| StoredLookupErrorV1::Allocation)?;
    Ok(result)
}

/// Fixed initialized tile payload, two guarded scalar predecessors and output-vector metadata.
/// Original owners/iterator allocation, provider storage/windows and allocator overhead are separate.
pub(super) fn scratch_bytes<F: StoredAssignmentFieldV1, S>(
    count: usize,
) -> Result<usize, StoredLookupErrorV1> {
    if count == 0 {
        return Ok(0);
    }
    std::mem::size_of::<F>()
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(STORED_SCALAR_BYTES_V1))
        .and_then(|bytes| bytes.checked_mul(TILE))
        .and_then(|bytes| {
            std::mem::size_of::<Previous<F>>()
                .checked_mul(2)
                .and_then(|previous| bytes.checked_add(previous))
        })
        .and_then(|bytes| {
            count
                .checked_mul(std::mem::size_of::<MembershipLookupV1<S>>())
                .and_then(|metadata| bytes.checked_add(metadata))
        })
        .ok_or(StoredLookupErrorV1::Context)
}

/// Witness predecessor lives in an initialized wiping guard across storage callbacks.
struct Previous<F: StoredAssignmentFieldV1> {
    value: F,
    present: bool,
}
impl<F: StoredAssignmentFieldV1 + Ord> Previous<F> {
    fn new() -> Self {
        Self {
            value: F::ZERO,
            present: false,
        }
    }
    fn observe(&mut self, value: F) -> Result<bool, StoredLookupErrorV1> {
        if self.present && value < self.value {
            return Err(StoredLookupErrorV1::Context);
        }
        let distinct = !self.present || value != self.value;
        self.value = value;
        self.present = true;
        Ok(distinct)
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Previous<F> {
    fn drop(&mut self) {
        // SAFETY: this exclusively owned initialized Copy scalar admits ZERO.
        unsafe {
            ptr::write_volatile(&mut self.value, F::ZERO);
        }
        compiler_fence(Ordering::SeqCst);
    }
}

fn role(lookup: u32) -> StoredPolynomialRoleV1 {
    StoredPolynomialRoleV1::LookupLeftoverTable { lookup }
}

#[allow(clippy::too_many_arguments)]
fn validate_banks<C, S>(
    advice: &CoefficientStoredAdviceV1<'_, C, S>,
    originals: &[CompressedLookupV1<S>],
    completed: &[MembershipLookupV1<S>],
    current: Option<&SortedLookupV1<S>>,
    remaining: &[SortedLookupV1<S>],
    sorted_end: Option<StoredPolynomialLayoutV1>,
    usable_rows: usize,
) -> Result<(), StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredPolynomialSnapshotV1,
{
    advice.validate_live_receipts()?;
    validate_outputs(originals, None)?;
    if completed
        .len()
        .checked_add(usize::from(current.is_some()))
        .and_then(|count| count.checked_add(remaining.len()))
        != Some(originals.len())
    {
        return Err(StoredLookupErrorV1::Context);
    }
    if originals.is_empty() {
        return if sorted_end.is_none() {
            Ok(())
        } else {
            Err(StoredLookupErrorV1::Context)
        };
    }
    let end = sorted_end.ok_or(StoredLookupErrorV1::Context)?;
    let mut previous = originals
        .last()
        .ok_or(StoredLookupErrorV1::Context)?
        .table
        .layout;
    if !previous.same_proof_context(end)
        || previous.field() != end.field()
        || previous.k() != end.k()
        || previous.ordinal() >= end.ordinal()
    {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    let mut check_sorted = |column: &SortedLookupColumnV1<S>, index, side| {
        let expected = column.layout;
        if expected.role()
            != (StoredPolynomialRoleV1::LookupSorted {
                lookup: index,
                side,
                run_log: end.k(),
            })
            || expected.basis() != StoredPolynomialBasisV1::Lagrange
            || column.snapshot.layout() != expected
            || !expected.same_proof_context(end)
            || expected.field() != end.field()
            || expected.k() != end.k()
            || expected.ordinal() <= previous.ordinal()
            || expected.ordinal() > end.ordinal()
        {
            return Err(StoredLookupErrorV1::Store(StoredPolynomialErrorV1::Context));
        }
        previous = expected;
        Ok(())
    };
    for (index, pair) in completed.iter().enumerate() {
        check_sorted(&pair.input, index as u32, StoredLookupSideV1::Input)?;
        if pair.distinct_inputs == 0
            || usable_rows.checked_sub(pair.distinct_inputs) != Some(pair.leftover_rows)
        {
            return Err(StoredLookupErrorV1::Context);
        }
    }
    for (offset, pair) in current.into_iter().chain(remaining.iter()).enumerate() {
        let index =
            u32::try_from(completed.len() + offset).map_err(|_| StoredLookupErrorV1::Context)?;
        check_sorted(&pair.input, index, StoredLookupSideV1::Input)?;
        check_sorted(&pair.table, index, StoredLookupSideV1::Table)?;
    }
    let mut last_output = end.ordinal();
    for (index, pair) in completed.iter().enumerate() {
        let column = &pair.leftover_table;
        let expected = column.layout;
        if expected.role() != role(index as u32)
            || expected.basis() != StoredPolynomialBasisV1::Lagrange
            || column.snapshot.layout() != expected
            || !expected.same_proof_context(end)
            || expected.field() != end.field()
            || expected.k() != end.k()
            || expected.ordinal() <= last_output
        {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        last_output = expected.ordinal();
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn prepare_one<'params, C, P>(
    mut advice: CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>,
    provider: &mut P,
    originals: &[CompressedLookupV1<SnapshotOf<P>>],
    completed: &[MembershipLookupV1<SnapshotOf<P>>],
    mut current: SortedLookupV1<SnapshotOf<P>>,
    remaining: &[SortedLookupV1<SnapshotOf<P>>],
    sorted_end: StoredPolynomialLayoutV1,
    usable_rows: usize,
) -> Result<
    (
        CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>,
        MembershipLookupV1<SnapshotOf<P>>,
    ),
    StoredLookupErrorV1,
>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    validate_banks(
        &advice,
        originals,
        completed,
        Some(&current),
        remaining,
        Some(sorted_end),
        usable_rows,
    )?;
    let lookup = u32::try_from(completed.len()).map_err(|_| StoredLookupErrorV1::Context)?;
    let mut input = Cursor::<C::Scalar>::new()?;
    let mut table = Cursor::<C::Scalar>::new()?;
    let mut output = Scalars::<C::Scalar>::new()?;
    let mut encoded = Encoded::new()?;
    let mut prior_input = Previous::<C::Scalar>::new();
    let mut prior_table = Previous::<C::Scalar>::new();
    input.reset(0, usable_rows);
    table.reset(0, usable_rows);
    let (next, mut writer, destination) = advice.create_output_writer(provider, role(lookup))?;
    advice = next;
    let last_output = completed.last().map_or(sorted_end.ordinal(), |pair| {
        pair.leftover_table.layout.ordinal()
    });
    if !destination.same_proof_context(sorted_end)
        || destination.field() != sorted_end.field()
        || destination.k() != sorted_end.k()
        || destination.ordinal() <= last_output
        || destination
            .ordinal()
            .checked_add(u64::try_from(remaining.len()).map_err(|_| StoredLookupErrorV1::Context)?)
            .is_none()
    {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    macro_rules! check {
        () => {{
            validate_banks(
                &advice,
                originals,
                completed,
                Some(&current),
                remaining,
                Some(sorted_end),
                usable_rows,
            )?;
            if writer.layout() != destination {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
        }};
    }
    check!();
    let mut written = 0_usize;
    let mut chunk = 0_u64;
    let mut fill = 0_usize;
    macro_rules! emit {
        ($value:expr) => {{
            if written >= usable_rows {
                return Err(StoredLookupErrorV1::Context);
            }
            output.0[fill] = $value;
            written += 1;
            fill += 1;
            if fill == destination.chunk_scalar_count(chunk)? {
                check!();
                write_tile(&mut writer, destination, chunk, &output, &mut encoded)?;
                check!();
                chunk += 1;
                fill = 0;
                output.clear();
            }
        }};
    }
    let mut distinct_inputs = 0_usize;
    loop {
        let (a, read) = input.peek(
            &mut current.input.snapshot,
            current.input.layout,
            usable_rows,
        )?;
        if read {
            check!();
        }
        let Some(a) = a else {
            break;
        };
        input.next += 1;
        if !prior_input.observe(a)? {
            continue;
        }
        distinct_inputs = distinct_inputs
            .checked_add(1)
            .ok_or(StoredLookupErrorV1::Context)?;
        loop {
            let (b, read) = table.peek(
                &mut current.table.snapshot,
                current.table.layout,
                usable_rows,
            )?;
            if read {
                check!();
            }
            let Some(b) = b else {
                return Err(StoredLookupErrorV1::Membership);
            };
            if b > a {
                return Err(StoredLookupErrorV1::Membership);
            }
            prior_table.observe(b)?;
            table.next += 1;
            if b == a {
                break;
            }
            emit!(b);
        }
    }
    loop {
        let (b, read) = table.peek(
            &mut current.table.snapshot,
            current.table.layout,
            usable_rows,
        )?;
        if read {
            check!();
        }
        let Some(b) = b else {
            break;
        };
        prior_table.observe(b)?;
        table.next += 1;
        emit!(b);
    }
    if input.next != usable_rows
        || table.next != usable_rows
        || distinct_inputs == 0
        || usable_rows.checked_sub(distinct_inputs) != Some(written)
    {
        return Err(StoredLookupErrorV1::Context);
    }
    // The private count, not scalar value, separates the leftover stream from ZERO padding.
    // `output` has remained ZERO beyond its initialized prefix after each flush.
    while chunk < destination.chunk_count() as u64 {
        check!();
        write_tile(&mut writer, destination, chunk, &output, &mut encoded)?;
        check!();
        chunk += 1;
        output.clear();
    }
    check!();
    let snapshot = writer.seal()?;
    if snapshot.layout() != destination {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    validate_banks(
        &advice,
        originals,
        completed,
        Some(&current),
        remaining,
        Some(sorted_end),
        usable_rows,
    )?;
    // Only now may the consumed sorted-table handle drop; original compressed banks remain held.
    let SortedLookupV1 { input, table } = current;
    drop(table);
    Ok((
        advice,
        MembershipLookupV1 {
            input,
            leftover_table: SortedLookupColumnV1 {
                layout: destination,
                snapshot,
            },
            distinct_inputs,
            leftover_rows: written,
        },
    ))
}

impl<'params, 'instances, C, P, R, T, E, const QUERY_INSTANCE: bool, const INSTANCE_MASK: u64>
    LookupSortedPendingStoredIpaProverV1<
        'params,
        'instances,
        C,
        P,
        R,
        T,
        E,
        QUERY_INSTANCE,
        INSTANCE_MASK,
    >
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
{
    /// Consume all sorted lookups into validated membership and ascending leftover scratch.
    ///
    /// Public geometry, output ordinal arithmetic, handle arithmetic and bounded payload budget
    /// are checked before any source read or writer creation. The backend enforces its existing
    /// capacity; no cap is raised. Missing membership, storage failure or unwind consumes every
    /// original and partial owner. No RNG or transcript trait bounds are needed at this stage.
    pub(crate) fn prepare_lookup_membership(
        self,
        scratch_limit_bytes: usize,
    ) -> Result<
        LookupMembershipPendingStoredIpaProverV1<
            'params,
            'instances,
            C,
            P,
            R,
            T,
            E,
            QUERY_INSTANCE,
            INSTANCE_MASK,
        >,
        StoredLookupErrorV1,
    > {
        let Self {
            compressed,
            usable_rows,
            sorted,
        } = self;
        let LookupCompressedPendingStoredIpaProverV1 {
            inner,
            theta,
            lookups: originals,
        } = compressed;
        let CoefficientPendingStoredIpaProverV1 {
            params,
            pk,
            mut advice,
            mut provider,
            rng,
            transcript,
            instances,
            _challenge,
        } = inner;
        advice.validate_for_lookup(&pk.vk.domain)?;
        let k = pk.vk.domain.k();
        let count = pk.vk.cs.lookups.len();
        let n = 1_usize.checked_shl(k).ok_or(StoredLookupErrorV1::Context)?;
        let expected_usable = pk
            .vk
            .cs
            .blinding_factors()
            .checked_add(1)
            .and_then(|tail| n.checked_sub(tail))
            .ok_or(StoredLookupErrorV1::Context)?;
        if k > STORED_MAX_K_V1
            || params.n() != n as u64
            || usable_rows != expected_usable
            || !ptr::eq(advice.params()?, params)
            || count != originals.len()
            || count != sorted.len()
            || (count != 0 && usable_rows == 0)
        {
            return Err(StoredLookupErrorV1::Context);
        }
        u32::try_from(count).map_err(|_| StoredLookupErrorV1::Context)?;
        let sorted_end = sorted.last().map(|pair| pair.table.layout);
        validate_banks(
            &advice,
            &originals,
            &[],
            None,
            &sorted,
            sorted_end,
            usable_rows,
        )?;
        // Initial 2a + 4l live receipts overlap with one new leftover writer. Each completed
        // table replacement releases one handle; capacity is never relaxed for this transition.
        advice
            .layouts()?
            .len()
            .checked_mul(2)
            .and_then(|v| count.checked_mul(4).and_then(|c| v.checked_add(c)))
            .and_then(|v| v.checked_add(usize::from(count != 0)))
            .ok_or(StoredLookupErrorV1::Context)?;
        if let Some(end) = sorted_end {
            end.ordinal()
                .checked_add(u64::try_from(count).map_err(|_| StoredLookupErrorV1::Context)?)
                .ok_or(StoredLookupErrorV1::Context)?;
            if end.k() != k || end.field() != C::Scalar::STORED_FIELD {
                return Err(StoredLookupErrorV1::Context);
            }
        }
        if scratch_bytes::<C::Scalar, SnapshotOf<P>>(count)? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let mut completed = reserved(count)?;
        let mut remaining = sorted.into_iter();
        while let Some(current) = remaining.next() {
            let (next, result) = prepare_one::<C, P>(
                advice,
                &mut provider,
                &originals,
                &completed,
                current,
                remaining.as_slice(),
                sorted_end.ok_or(StoredLookupErrorV1::Context)?,
                usable_rows,
            )?;
            advice = next;
            completed.push(result);
        }
        validate_banks(
            &advice,
            &originals,
            &completed,
            None,
            &[],
            sorted_end,
            usable_rows,
        )?;
        Ok(LookupMembershipPendingStoredIpaProverV1 {
            compressed: LookupCompressedPendingStoredIpaProverV1 {
                inner: CoefficientPendingStoredIpaProverV1 {
                    params,
                    pk,
                    advice,
                    provider,
                    rng,
                    transcript,
                    instances,
                    _challenge,
                },
                theta,
                lookups: originals,
            },
            usable_rows,
            lookups: completed,
        })
    }
}
