//! Consuming, bounded external sorting of the exact key's usable lookup prefixes.
//!
//! Authenticated scratch passes contain sorted power-of-two runs and canonical ZERO padding
//! outside the original key's usable prefix. Padding is never part of the sorting population.
//! The complete compressed owner, including original inactive rows, theta, sole advice blinds,
//! RNG and transcript, remains inseparable. This stage draws no randomness or commitments.
//! The membership continuation consumes this owner into authenticated forward leftovers.
//! TODO: consume that owner to draw all input tails then table tails, convert/commit each side
//! and write both ordinary commitment points.
//! Sorting alone is neither a lookup permutation nor a complete stored proof or RSS guarantee.

use super::{
    CoefficientPendingStoredIpaProverV1,
    lookup::{
        CompressedLookupV1, LookupCompressedPendingStoredIpaProverV1, StoredLookupErrorV1,
        validate_outputs,
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
use ff::{PrimeField, WithSmallOrderMulGroup};
use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

const TILE: usize = STORED_SCALARS_PER_CHUNK_V1;
type SnapshotOf<P> =
    <<P as StoredPolynomialProviderV1>::Writer as StoredPolynomialWriterV1>::Snapshot;

/// A scratch sequence, never an independently authorized argument polynomial.
pub(super) struct SortedLookupColumnV1<S> {
    pub(super) layout: StoredPolynomialLayoutV1,
    pub(super) snapshot: S,
}
/// Final sorted active prefixes for one retained-key lookup, in input/table order.
pub(super) struct SortedLookupV1<S> {
    pub(super) input: SortedLookupColumnV1<S>,
    pub(super) table: SortedLookupColumnV1<S>,
}
/// Original protocol owner and sorted scratch receipts, with no detachable constructor.
#[allow(dead_code)]
pub(crate) struct LookupSortedPendingStoredIpaProverV1<
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
    pub(super) sorted: Vec<SortedLookupV1<SnapshotOf<P>>>,
}

fn reserved<T>(count: usize) -> Result<Vec<T>, StoredLookupErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| StoredLookupErrorV1::Allocation)?;
    Ok(values)
}

/// Checked temporary payload plus retained result-vector metadata; backend storage is separate.
pub(super) fn scratch_bytes<F, S>(count: usize) -> Result<usize, StoredLookupErrorV1> {
    if count == 0 {
        return Ok(0);
    }
    std::mem::size_of::<F>()
        .checked_mul(3)
        .and_then(|bytes| bytes.checked_add(STORED_SCALAR_BYTES_V1))
        .and_then(|bytes| bytes.checked_mul(TILE))
        .and_then(|bytes| {
            count
                .checked_mul(std::mem::size_of::<SortedLookupV1<S>>())
                .and_then(|metadata| bytes.checked_add(metadata))
        })
        .ok_or(StoredLookupErrorV1::Context)
}

pub(super) struct Scalars<F: StoredAssignmentFieldV1>(pub(super) Vec<F>);
impl<F: StoredAssignmentFieldV1> Scalars<F> {
    pub(super) fn new() -> Result<Self, StoredLookupErrorV1> {
        let mut result = Self(reserved(TILE)?);
        result.0.resize(TILE, F::ZERO);
        Ok(result)
    }
    pub(super) fn clear(&mut self) {
        for value in &mut self.0 {
            // SAFETY: these exclusively owned initialized Copy scalars admit ZERO.
            unsafe {
                ptr::write_volatile(value, F::ZERO);
            }
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        FIELD_CLEARS.with(|state| {
            let (slots, zero) = state.get();
            state.set((
                slots + self.0.len(),
                zero && self.0.iter().all(|v| *v == F::ZERO),
            ));
        });
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Scalars<F> {
    fn drop(&mut self) {
        self.clear();
    }
}
pub(super) struct Encoded(pub(super) Vec<[u8; STORED_SCALAR_BYTES_V1]>);
impl Encoded {
    pub(super) fn new() -> Result<Self, StoredLookupErrorV1> {
        let mut result = Self(reserved(TILE)?);
        result.0.resize(TILE, [0; STORED_SCALAR_BYTES_V1]);
        Ok(result)
    }
    pub(super) fn clear(&mut self) {
        for value in &mut self.0 {
            // SAFETY: initialized, exclusively owned byte arrays accept every bit pattern.
            unsafe {
                ptr::write_volatile(value, [0; STORED_SCALAR_BYTES_V1]);
            }
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        ENCODED_CLEARS.with(|state| {
            let (slots, zero) = state.get();
            state.set((
                slots + self.0.len(),
                zero && self.0.iter().all(|v| *v == [0; STORED_SCALAR_BYTES_V1]),
            ));
        });
    }
}
impl Drop for Encoded {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
thread_local! {
    static FIELD_CLEARS: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, true)) };
    static ENCODED_CLEARS: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, true)) };
}
#[cfg(test)]
pub(super) fn take_clear_observations() -> (usize, bool, usize, bool) {
    let (fields, fields_zero) = FIELD_CLEARS.with(|s| s.replace((0, true)));
    let (bytes, bytes_zero) = ENCODED_CLEARS.with(|s| s.replace((0, true)));
    (fields, fields_zero, bytes, bytes_zero)
}

fn role(lookup: u32, side: StoredLookupSideV1, run_log: u32) -> StoredPolynomialRoleV1 {
    StoredPolynomialRoleV1::LookupSorted {
        lookup,
        side,
        run_log,
    }
}

fn validate_banks<C, S>(
    advice: &CoefficientStoredAdviceV1<'_, C, S>,
    originals: &[CompressedLookupV1<S>],
    completed: &[SortedLookupV1<S>],
    input: Option<&SortedLookupColumnV1<S>>,
) -> Result<(), StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredPolynomialSnapshotV1,
{
    advice.validate_live_receipts()?;
    validate_outputs(originals, None)?;
    let mut last = originals.last().map(|pair| pair.table.layout);
    let mut check =
        |column: &SortedLookupColumnV1<S>, lookup, side| -> Result<(), StoredLookupErrorV1> {
            let expected = column.layout;
            if expected.role() != role(lookup, side, expected.k())
                || expected.basis() != StoredPolynomialBasisV1::Lagrange
                || column.snapshot.layout() != expected
                || last.is_none_or(|old| {
                    !old.same_proof_context(expected)
                        || old.field() != expected.field()
                        || old.k() != expected.k()
                        || old.ordinal() >= expected.ordinal()
                })
            {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            last = Some(expected);
            Ok(())
        };
    for (index, pair) in completed.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| StoredLookupErrorV1::Context)?;
        check(&pair.input, index, StoredLookupSideV1::Input)?;
        check(&pair.table, index, StoredLookupSideV1::Table)?;
    }
    if let Some(input) = input {
        check(
            input,
            u32::try_from(completed.len()).map_err(|_| StoredLookupErrorV1::Context)?,
            StoredLookupSideV1::Input,
        )?;
    }
    Ok(())
}

fn read_active<F: StoredAssignmentFieldV1, S: StoredPolynomialSnapshotV1>(
    source: &mut S,
    expected: StoredPolynomialLayoutV1,
    chunk: u64,
    usable_rows: usize,
    values: &mut Scalars<F>,
) -> Result<(), StoredLookupErrorV1> {
    if source.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    let start = usize::try_from(chunk)
        .ok()
        .and_then(|v| v.checked_mul(TILE))
        .ok_or(StoredLookupErrorV1::Context)?;
    let len = expected.chunk_scalar_count(chunk)?;
    let active = usable_rows.saturating_sub(start).min(len);
    values.clear();
    source.with_chunk(expected, chunk, |encoded| {
        if encoded.len() != len {
            return Err(StoredPolynomialErrorV1::Context);
        }
        for (out, value) in values.0[..active].iter_mut().zip(&encoded[..active]) {
            *out =
                Option::<F>::from(F::from_repr(*value)).ok_or(StoredPolynomialErrorV1::Encoding)?;
        }
        Ok(())
    })?;
    if source.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    Ok(())
}

pub(super) fn write_tile<F: StoredAssignmentFieldV1, W: StoredPolynomialWriterV1>(
    writer: &mut W,
    expected: StoredPolynomialLayoutV1,
    chunk: u64,
    values: &Scalars<F>,
    encoded: &mut Encoded,
) -> Result<(), StoredLookupErrorV1> {
    if writer.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    let len = expected.chunk_scalar_count(chunk)?;
    encoded.clear();
    for (out, value) in encoded.0[..len].iter_mut().zip(&values.0[..len]) {
        *out = value.to_repr();
    }
    writer.write_chunk(chunk, &encoded.0[..len])?;
    if writer.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    encoded.clear();
    Ok(())
}

/// A run cursor owns one initialized chunk; cached values never cross a run boundary.
pub(super) struct Cursor<F: StoredAssignmentFieldV1> {
    values: Scalars<F>,
    pub(super) next: usize,
    end: usize,
    cached_chunk: Option<u64>,
}
impl<F: StoredAssignmentFieldV1> Cursor<F> {
    pub(super) fn new() -> Result<Self, StoredLookupErrorV1> {
        Ok(Self {
            values: Scalars::new()?,
            next: 0,
            end: 0,
            cached_chunk: None,
        })
    }
    pub(super) fn reset(&mut self, start: usize, end: usize) {
        self.values.clear();
        self.next = start;
        self.end = end;
        self.cached_chunk = None;
    }
    pub(super) fn peek<S: StoredPolynomialSnapshotV1>(
        &mut self,
        source: &mut S,
        expected: StoredPolynomialLayoutV1,
        usable_rows: usize,
    ) -> Result<(Option<F>, bool), StoredLookupErrorV1> {
        if self.next == self.end {
            return Ok((None, false));
        }
        let chunk = (self.next / TILE) as u64;
        let read = self.cached_chunk != Some(chunk);
        if read {
            read_active(source, expected, chunk, usable_rows, &mut self.values)?;
            self.cached_chunk = Some(chunk);
        }
        Ok((Some(self.values.0[self.next % TILE]), read))
    }
}

#[allow(clippy::too_many_arguments)]
fn sort_side<'params, C, P>(
    mut advice: CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>,
    provider: &mut P,
    originals: &mut [CompressedLookupV1<SnapshotOf<P>>],
    completed: &[SortedLookupV1<SnapshotOf<P>>],
    input: Option<&SortedLookupColumnV1<SnapshotOf<P>>>,
    index: usize,
    side: StoredLookupSideV1,
    k: u32,
    usable_rows: usize,
) -> Result<
    (
        CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>,
        SortedLookupColumnV1<SnapshotOf<P>>,
    ),
    StoredLookupErrorV1,
>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
{
    let lookup = u32::try_from(index).map_err(|_| StoredLookupErrorV1::Context)?;
    let mut left = Cursor::<C::Scalar>::new()?;
    let mut right = Cursor::<C::Scalar>::new()?;
    let mut output = Scalars::<C::Scalar>::new()?;
    let mut encoded = Encoded::new()?;
    let mut previous: Option<SortedLookupColumnV1<SnapshotOf<P>>> = None;
    for run_log in k.min(8)..=k {
        validate_banks(&advice, originals, completed, input)?;
        let (next_advice, mut writer, destination) =
            advice.create_output_writer(provider, role(lookup, side, run_log))?;
        advice = next_advice;
        validate_banks(&advice, originals, completed, input)?;
        if let Some(source) = &previous {
            if source.layout.role() != role(lookup, side, run_log - 1)
                || source.snapshot.layout() != source.layout
                || !source.layout.same_proof_context(destination)
                || source.layout.field() != destination.field()
                || source.layout.k() != k
                || source.layout.ordinal() >= destination.ordinal()
            {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
        }
        for chunk in 0..destination.chunk_count() as u64 {
            validate_banks(&advice, originals, completed, input)?;
            if writer.layout() != destination {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            let start = chunk as usize * TILE;
            let len = destination.chunk_scalar_count(chunk)?;
            let active = usable_rows.saturating_sub(start).min(len);
            output.clear();
            if let Some(source) = &mut previous {
                let width = 1_usize << (run_log - 1);
                for row in start..start + active {
                    if row % (2 * width) == 0 {
                        left.reset(row, (row + width).min(usable_rows));
                        right.reset(
                            (row + width).min(usable_rows),
                            (row + 2 * width).min(usable_rows),
                        );
                    }
                    // Each callback ends before another cursor read, comparison or output write.
                    let (a, read) = left.peek(&mut source.snapshot, source.layout, usable_rows)?;
                    if read {
                        validate_banks(&advice, originals, completed, input)?;
                    }
                    if writer.layout() != destination || source.snapshot.layout() != source.layout {
                        return Err(StoredPolynomialErrorV1::Context.into());
                    }
                    let (b, read) = right.peek(&mut source.snapshot, source.layout, usable_rows)?;
                    if read {
                        validate_banks(&advice, originals, completed, input)?;
                    }
                    if writer.layout() != destination || source.snapshot.layout() != source.layout {
                        return Err(StoredPolynomialErrorV1::Context.into());
                    }
                    output.0[row - start] = match (a, b) {
                        (Some(a), Some(b)) if a <= b => {
                            left.next += 1;
                            a
                        }
                        (Some(a), None) => {
                            left.next += 1;
                            a
                        }
                        (_, Some(b)) => {
                            right.next += 1;
                            b
                        }
                        (None, None) => return Err(StoredLookupErrorV1::Context),
                    };
                }
                if source.snapshot.layout() != source.layout {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
            } else {
                let original = match side {
                    StoredLookupSideV1::Input => &mut originals[index].input,
                    StoredLookupSideV1::Table => &mut originals[index].table,
                };
                // The original inactive rows stay retained in their original bank; never copy
                // them into a scratch population or replace them with permutation tails.
                read_active(
                    &mut original.snapshot,
                    original.layout,
                    chunk,
                    usable_rows,
                    &mut output,
                )?;
                validate_banks(&advice, originals, completed, input)?;
                if writer.layout() != destination {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
                output.0[..active].sort_unstable();
            }
            write_tile(&mut writer, destination, chunk, &output, &mut encoded)?;
            validate_banks(&advice, originals, completed, input)?;
            if let Some(source) = &previous {
                if source.snapshot.layout() != source.layout {
                    return Err(StoredPolynomialErrorV1::Context.into());
                }
            }
        }
        if writer.layout() != destination {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        let snapshot = writer.seal()?;
        if snapshot.layout() != destination {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        validate_banks(&advice, originals, completed, input)?;
        if let Some(source) = &previous {
            if source.snapshot.layout() != source.layout {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
        }
        // Drop a prior scratch pass only after its replacement seals and all identities agree.
        previous = Some(SortedLookupColumnV1 {
            layout: destination,
            snapshot,
        });
    }
    Ok((advice, previous.ok_or(StoredLookupErrorV1::Context)?))
}

impl<'params, 'instances, C, P, R, T, E, const QUERY_INSTANCE: bool, const INSTANCE_MASK: u64>
    LookupCompressedPendingStoredIpaProverV1<
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
    /// Consume compressed lookups into sorted scratch prefixes without proof side effects.
    ///
    /// Public key/pass geometry and explicit payload budget are checked before writer creation
    /// or witness reads. Missing input membership is preserved for the next consuming stage to
    /// reject before randomness. No RNG/transcript trait bound is needed at this boundary.
    pub(crate) fn sort_lookup_values(
        self,
        scratch_limit_bytes: usize,
    ) -> Result<
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
        >,
        StoredLookupErrorV1,
    > {
        let Self {
            inner,
            theta,
            mut lookups,
        } = self;
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
        let usable_rows = pk
            .vk
            .cs
            .blinding_factors()
            .checked_add(1)
            .and_then(|tail| n.checked_sub(tail))
            .ok_or(StoredLookupErrorV1::Context)?;
        if k > STORED_MAX_K_V1
            || params.n() != n as u64
            || !std::ptr::eq(advice.params()?, params)
            || count != lookups.len()
            || (count != 0 && usable_rows == 0)
        {
            return Err(StoredLookupErrorV1::Context);
        }
        validate_banks(&advice, &lookups, &[], None)?;
        // All public pass counts and eventual ordinal/handle arithmetic are checked up front.
        // The backend remains the authority for its existing live-handle capacity; no cap is
        // raised here. Live peak is 2a + 4l + (l>0 && k>8), with old/new scratch overlapping.
        let columns = advice.layouts()?.len();
        let sides = count.checked_mul(2).ok_or(StoredLookupErrorV1::Context)?;
        u32::try_from(count).map_err(|_| StoredLookupErrorV1::Context)?;
        columns
            .checked_mul(2)
            .and_then(|v| count.checked_mul(4).and_then(|c| v.checked_add(c)))
            .and_then(|v| v.checked_add(usize::from(count > 0 && k > 8)))
            .ok_or(StoredLookupErrorV1::Context)?;
        let writers = sides
            .checked_mul((k - k.min(8) + 1) as usize)
            .ok_or(StoredLookupErrorV1::Context)?;
        if let Some(last) = lookups.last() {
            last.table
                .layout
                .ordinal()
                .checked_add(u64::try_from(writers).map_err(|_| StoredLookupErrorV1::Context)?)
                .ok_or(StoredLookupErrorV1::Context)?;
            if last.table.layout.k() != k || last.table.layout.field() != C::Scalar::STORED_FIELD {
                return Err(StoredLookupErrorV1::Context);
            }
        }
        if scratch_bytes::<C::Scalar, SnapshotOf<P>>(count)? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let mut sorted = reserved(count)?;
        for index in 0..count {
            let (next, input) = sort_side::<C, P>(
                advice,
                &mut provider,
                &mut lookups,
                &sorted,
                None,
                index,
                StoredLookupSideV1::Input,
                k,
                usable_rows,
            )?;
            advice = next;
            let (next, table) = sort_side::<C, P>(
                advice,
                &mut provider,
                &mut lookups,
                &sorted,
                Some(&input),
                index,
                StoredLookupSideV1::Table,
                k,
                usable_rows,
            )?;
            advice = next;
            sorted.push(SortedLookupV1 { input, table });
        }
        validate_banks(&advice, &lookups, &sorted, None)?;
        Ok(LookupSortedPendingStoredIpaProverV1 {
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
                lookups,
            },
            usable_rows,
            sorted,
        })
    }
}
