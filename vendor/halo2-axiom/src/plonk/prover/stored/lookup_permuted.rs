//! Consuming stored lookup permutation pairs through the ordinary commitment boundary.
//!
//! All membership checks precede this owner. Each pair draws all input tails, all table tails,
//! then converts/blinds/commits input followed by table using the original ParamsIPA Lagrange
//! commitment. Only after both points exist are input and table written to the original
//! transcript. Original compressed banks, advice, theta, key, instances and sole blind guards
//! remain owned. Every error or unwind destroys original and partial continuations.
//! The consuming products stage follows these pairs. TODO: finish quotient and openings. This is not a
//! complete stored proof, four-constructor integration, whole-process RSS or release claim.

use super::{
    CoefficientPendingStoredIpaProverV1,
    lookup::{
        CompressedLookupV1, LookupCompressedPendingStoredIpaProverV1, StoredLookupErrorV1,
        validate_outputs,
    },
    lookup_membership::{LookupMembershipPendingStoredIpaProverV1, MembershipLookupV1},
    lookup_sort::{Cursor, Encoded},
};
use crate::{
    arithmetic::CurveAffine,
    plonk::ProvingKey,
    poly::{
        EvaluationDomain, LagrangeCoeff, Polynomial,
        commitment::{Blind, Params},
        ipa::commitment::ParamsIPA,
        stored_advice::{
            STORED_MAX_K_V1, STORED_SCALAR_BYTES_V1, STORED_SCALARS_PER_CHUNK_V1,
            StoredLookupSideV1, StoredPolynomialBasisV1, StoredPolynomialErrorV1,
            StoredPolynomialLayoutV1, StoredPolynomialProviderV1, StoredPolynomialRoleV1,
            StoredPolynomialSnapshotV1, StoredPolynomialWriterV1,
            assignment::StoredAssignmentFieldV1, phase::CoefficientStoredAdviceV1,
        },
    },
    transcript::{EncodedChallenge, TranscriptWrite},
};
use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use group::Curve;
use rand_core::RngCore;
use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

const TILE: usize = STORED_SCALARS_PER_CHUNK_V1;
type SnapshotOf<P> =
    <<P as StoredPolynomialProviderV1>::Writer as StoredPolynomialWriterV1>::Snapshot;

/// One immutable argument receipt; semantic role is preserved across its legitimate bases.
pub(super) struct PermutedPolynomialV1<S> {
    pub(super) layout: StoredPolynomialLayoutV1,
    pub(super) snapshot: S,
}
/// Sole owning guard for one ordinary permutation commitment blind.
pub(super) struct SecretLookupBlindV1<F: StoredAssignmentFieldV1>(pub(super) Blind<F>);
impl<F: StoredAssignmentFieldV1> Drop for SecretLookupBlindV1<F> {
    fn drop(&mut self) {
        // SAFETY: this exclusive initialized Copy field admits ZERO.
        unsafe {
            ptr::write_volatile(&mut self.0.0, F::ZERO);
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        BLIND_CLEARS.with(|cell| {
            let (n, z) = cell.get();
            cell.set((n + 1, z && self.0.0 == F::ZERO));
        });
    }
}
/// Original Lagrange values, coefficient copy, sole blind and ordinary commitment for one side.
pub(super) struct PermutedLookupColumnV1<C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    pub(super) lagrange: PermutedPolynomialV1<S>,
    pub(super) coefficient: PermutedPolynomialV1<S>,
    pub(super) blind: SecretLookupBlindV1<C::Scalar>,
    pub(super) commitment: C,
}
/// Retained-key input/table pair, in the exact ordinary protocol order.
pub(super) struct PermutedLookupV1<C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    pub(super) input: PermutedLookupColumnV1<C, S>,
    pub(super) table: PermutedLookupColumnV1<C, S>,
}
/// Inseparable original continuation and all committed permutation pairs.
#[allow(dead_code)]
pub(crate) struct LookupPermutedPendingStoredIpaProverV1<
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
    pub(super) lookups: Vec<PermutedLookupV1<C, SnapshotOf<P>>>,
}

fn reserved<T>(count: usize) -> Result<Vec<T>, StoredLookupErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| StoredLookupErrorV1::Allocation)?;
    Ok(values)
}

/// Guarded n-field column reused for permutation values, coefficients and Lagrange commitment.
struct Column<F: StoredAssignmentFieldV1>(Polynomial<F, LagrangeCoeff>);
impl<F: StoredAssignmentFieldV1> Column<F> {
    fn new(domain: &EvaluationDomain<F>) -> Result<Self, StoredLookupErrorV1>
    where
        F: WithSmallOrderMulGroup<3>,
    {
        let n = usize::try_from(domain.get_n()).map_err(|_| StoredLookupErrorV1::Context)?;
        let mut values = reserved(n)?;
        values.resize(n, F::ZERO);
        Ok(Self(domain.lagrange_from_vec(values)))
    }
    fn clear(&mut self) {
        for value in &mut self.0.values {
            // SAFETY: every exclusive initialized Copy field admits ZERO.
            unsafe {
                ptr::write_volatile(value, F::ZERO);
            }
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        FIELD_CLEARS.with(|cell| {
            let (n, z) = cell.get();
            cell.set((
                n + self.0.values.len(),
                z && self.0.values.iter().all(|v| *v == F::ZERO),
            ));
        });
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Column<F> {
    fn drop(&mut self) {
        self.clear();
    }
}
struct Previous<F: StoredAssignmentFieldV1>(F);
impl<F: StoredAssignmentFieldV1> Previous<F> {
    fn clear(&mut self) {
        // SAFETY: this exclusive initialized Copy field admits ZERO.
        unsafe {
            ptr::write_volatile(&mut self.0, F::ZERO);
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        FIELD_CLEARS.with(|cell| {
            let (n, z) = cell.get();
            cell.set((n + 1, z && self.0 == F::ZERO));
        });
    }
}
impl<F: StoredAssignmentFieldV1> Drop for Previous<F> {
    fn drop(&mut self) {
        self.clear();
    }
}
struct Scratch<F: StoredAssignmentFieldV1> {
    column: Column<F>,
    input: Cursor<F>,
    leftover: Cursor<F>,
    encoded: Encoded,
    previous: Previous<F>,
}
impl<F: StoredAssignmentFieldV1> Scratch<F> {
    fn new(domain: &EvaluationDomain<F>) -> Result<Self, StoredLookupErrorV1>
    where
        F: WithSmallOrderMulGroup<3>,
    {
        Ok(Self {
            column: Column::new(domain)?,
            input: Cursor::new()?,
            leftover: Cursor::new()?,
            encoded: Encoded::new()?,
            previous: Previous(F::ZERO),
        })
    }
}
#[cfg(test)]
thread_local! {
    static FIELD_CLEARS: std::cell::Cell<(usize,bool)> = const {std::cell::Cell::new((0,true))};
    static BLIND_CLEARS: std::cell::Cell<(usize,bool)> = const {std::cell::Cell::new((0,true))};
}
/// Read/reset initialized adapter-payload cleanup observations; no register/MSM-erasure claim.
#[cfg(test)]
pub(super) fn take_clear_observations() -> (usize, bool, usize, bool, usize, bool) {
    let (cursor_n, cursor_z, encoded_n, encoded_z) = super::lookup_sort::take_clear_observations();
    let (n, z) = FIELD_CLEARS.with(|cell| cell.replace((0, true)));
    let (blind_n, blind_z) = BLIND_CLEARS.with(|cell| cell.replace((0, true)));
    (
        n + cursor_n,
        z && cursor_z,
        encoded_n,
        encoded_z,
        blind_n,
        blind_z,
    )
}

/// Checked known owned payload before proof RNG: one column, fixed cursors/encoding and metadata.
/// Backend windows/MSM, public FFT twiddles, allocator overhead and compiler copies are separate.
pub(super) fn scratch_bytes<C: CurveAffine, S>(
    k: u32,
    count: usize,
) -> Result<usize, StoredLookupErrorV1>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    if k > STORED_MAX_K_V1 {
        return Err(StoredLookupErrorV1::Context);
    }
    if count == 0 {
        return Ok(0);
    }
    let n = 1_usize.checked_shl(k).ok_or(StoredLookupErrorV1::Context)?;
    n.checked_add(2 * TILE)
        .and_then(|rows| rows.checked_mul(std::mem::size_of::<C::Scalar>()))
        .and_then(|bytes| bytes.checked_add(TILE * STORED_SCALAR_BYTES_V1))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Previous<C::Scalar>>()))
        .and_then(|bytes| {
            count
                .checked_mul(std::mem::size_of::<PermutedLookupV1<C, S>>())
                .and_then(|metadata| bytes.checked_add(metadata))
        })
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Current<C, S>>()))
        .ok_or(StoredLookupErrorV1::Context)
}

#[derive(Clone, Copy)]
struct Boundary {
    sorted_input_end: StoredPolynomialLayoutV1,
    membership_end: StoredPolynomialLayoutV1,
}
struct Current<C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    source: Option<MembershipLookupV1<S>>,
    input_lagrange: Option<PermutedPolynomialV1<S>>,
    table_lagrange: Option<PermutedPolynomialV1<S>>,
    input_coefficient: Option<PermutedPolynomialV1<S>>,
    table_coefficient: Option<PermutedPolynomialV1<S>>,
    input_blind: Option<SecretLookupBlindV1<C::Scalar>>,
    table_blind: Option<SecretLookupBlindV1<C::Scalar>>,
    input_commitment: Option<C>,
    table_commitment: Option<C>,
}
impl<C: CurveAffine, S> Current<C, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    fn new(source: MembershipLookupV1<S>) -> Self {
        Self {
            source: Some(source),
            input_lagrange: None,
            table_lagrange: None,
            input_coefficient: None,
            table_coefficient: None,
            input_blind: None,
            table_blind: None,
            input_commitment: None,
            table_commitment: None,
        }
    }
    fn lagrange_mut(
        &mut self,
        side: StoredLookupSideV1,
    ) -> Result<&mut PermutedPolynomialV1<S>, StoredLookupErrorV1> {
        match side {
            StoredLookupSideV1::Input => self.input_lagrange.as_mut(),
            StoredLookupSideV1::Table => self.table_lagrange.as_mut(),
        }
        .ok_or(StoredLookupErrorV1::Context)
    }
    fn finish(mut self) -> Result<PermutedLookupV1<C, S>, StoredLookupErrorV1> {
        if self.source.is_some() {
            return Err(StoredLookupErrorV1::Context);
        }
        Ok(PermutedLookupV1 {
            input: PermutedLookupColumnV1 {
                lagrange: self
                    .input_lagrange
                    .take()
                    .ok_or(StoredLookupErrorV1::Context)?,
                coefficient: self
                    .input_coefficient
                    .take()
                    .ok_or(StoredLookupErrorV1::Context)?,
                blind: self
                    .input_blind
                    .take()
                    .ok_or(StoredLookupErrorV1::Context)?,
                commitment: self
                    .input_commitment
                    .take()
                    .ok_or(StoredLookupErrorV1::Context)?,
            },
            table: PermutedLookupColumnV1 {
                lagrange: self
                    .table_lagrange
                    .take()
                    .ok_or(StoredLookupErrorV1::Context)?,
                coefficient: self
                    .table_coefficient
                    .take()
                    .ok_or(StoredLookupErrorV1::Context)?,
                blind: self
                    .table_blind
                    .take()
                    .ok_or(StoredLookupErrorV1::Context)?,
                commitment: self
                    .table_commitment
                    .take()
                    .ok_or(StoredLookupErrorV1::Context)?,
            },
        })
    }
}
fn role(lookup: u32, side: StoredLookupSideV1) -> StoredPolynomialRoleV1 {
    StoredPolynomialRoleV1::LookupPermuted { lookup, side }
}

#[allow(clippy::too_many_arguments)]
fn validate<C, S>(
    advice: &CoefficientStoredAdviceV1<'_, C, S>,
    originals: &[CompressedLookupV1<S>],
    completed: &[PermutedLookupV1<C, S>],
    current: Option<&Current<C, S>>,
    remaining: &[MembershipLookupV1<S>],
    boundary: Option<Boundary>,
    usable: usize,
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
        .and_then(|n| n.checked_add(remaining.len()))
        != Some(originals.len())
    {
        return Err(StoredLookupErrorV1::Context);
    }
    if originals.is_empty() {
        return if boundary.is_none() {
            Ok(())
        } else {
            Err(StoredLookupErrorV1::Context)
        };
    }
    let bounds = boundary.ok_or(StoredLookupErrorV1::Context)?;
    let end = bounds.membership_end;
    let original_end = originals
        .last()
        .ok_or(StoredLookupErrorV1::Context)?
        .table
        .layout;
    if !original_end.same_proof_context(end)
        || original_end.field() != end.field()
        || original_end.k() != end.k()
        || original_end.ordinal() >= bounds.sorted_input_end.ordinal()
        || bounds.sorted_input_end.ordinal() >= end.ordinal()
        || !bounds.sorted_input_end.same_proof_context(end)
        || bounds.sorted_input_end.field() != end.field()
        || bounds.sorted_input_end.k() != end.k()
    {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    let mut last_input = original_end.ordinal();
    let mut last_leftover = bounds.sorted_input_end.ordinal();
    let sources = current
        .into_iter()
        .filter_map(|c| c.source.as_ref())
        .map(|p| (completed.len(), p))
        .chain(
            remaining
                .iter()
                .enumerate()
                .map(|(i, p)| (completed.len() + usize::from(current.is_some()) + i, p)),
        );
    for (index, source) in sources {
        if source.distinct_inputs == 0
            || usable.checked_sub(source.distinct_inputs) != Some(source.leftover_rows)
        {
            return Err(StoredLookupErrorV1::Context);
        }
        for (column, expected_role, lower, upper) in [
            (
                &source.input,
                StoredPolynomialRoleV1::LookupSorted {
                    lookup: index as u32,
                    side: StoredLookupSideV1::Input,
                    run_log: end.k(),
                },
                last_input,
                bounds.sorted_input_end.ordinal(),
            ),
            (
                &source.leftover_table,
                StoredPolynomialRoleV1::LookupLeftoverTable {
                    lookup: index as u32,
                },
                last_leftover,
                end.ordinal(),
            ),
        ] {
            let expected = column.layout;
            if expected.role() != expected_role
                || expected.basis() != StoredPolynomialBasisV1::Lagrange
                || column.snapshot.layout() != expected
                || !expected.same_proof_context(end)
                || expected.field() != end.field()
                || expected.k() != end.k()
                || expected.ordinal() <= lower
                || expected.ordinal() > upper
            {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
        }
        last_input = source.input.layout.ordinal();
        last_leftover = source.leftover_table.layout.ordinal();
    }
    let mut last = end.ordinal();
    let mut check = |column: &PermutedPolynomialV1<S>, index: u32, side, basis| {
        let expected = column.layout;
        if expected.role() != role(index, side)
            || expected.basis() != basis
            || column.snapshot.layout() != expected
            || !expected.same_proof_context(end)
            || expected.field() != end.field()
            || expected.k() != end.k()
            || expected.ordinal() <= last
        {
            return Err(StoredLookupErrorV1::Store(StoredPolynomialErrorV1::Context));
        }
        last = expected.ordinal();
        Ok(())
    };
    for (index, pair) in completed.iter().enumerate() {
        check(
            &pair.input.lagrange,
            index as u32,
            StoredLookupSideV1::Input,
            StoredPolynomialBasisV1::Lagrange,
        )?;
        check(
            &pair.table.lagrange,
            index as u32,
            StoredLookupSideV1::Table,
            StoredPolynomialBasisV1::Lagrange,
        )?;
        check(
            &pair.input.coefficient,
            index as u32,
            StoredLookupSideV1::Input,
            StoredPolynomialBasisV1::Coefficient,
        )?;
        check(
            &pair.table.coefficient,
            index as u32,
            StoredLookupSideV1::Table,
            StoredPolynomialBasisV1::Coefficient,
        )?;
    }
    if let Some(current) = current {
        let mut missing = false;
        for (column, side, basis) in [
            (
                &current.input_lagrange,
                StoredLookupSideV1::Input,
                StoredPolynomialBasisV1::Lagrange,
            ),
            (
                &current.table_lagrange,
                StoredLookupSideV1::Table,
                StoredPolynomialBasisV1::Lagrange,
            ),
            (
                &current.input_coefficient,
                StoredLookupSideV1::Input,
                StoredPolynomialBasisV1::Coefficient,
            ),
            (
                &current.table_coefficient,
                StoredLookupSideV1::Table,
                StoredPolynomialBasisV1::Coefficient,
            ),
        ] {
            if let Some(column) = column {
                if missing {
                    return Err(StoredLookupErrorV1::Context);
                }
                check(column, completed.len() as u32, side, basis)?;
            } else {
                missing = true;
            }
        }
        if current.source.is_none()
            && (current.input_lagrange.is_none() || current.table_lagrange.is_none())
        {
            return Err(StoredLookupErrorV1::Context);
        }
    }
    Ok(())
}

fn read_chunk<F, S>(
    source: &mut S,
    expected: StoredPolynomialLayoutV1,
    chunk: u64,
    values: &mut Column<F>,
) -> Result<(), StoredLookupErrorV1>
where
    F: StoredAssignmentFieldV1,
    S: StoredPolynomialSnapshotV1,
{
    if source.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    let count = expected.chunk_scalar_count(chunk)?;
    let start = chunk as usize * TILE;
    source.with_chunk(expected, chunk, |encoded| {
        if encoded.len() != count {
            return Err(StoredPolynomialErrorV1::Encoding);
        }
        for (out, value) in values.0.values[start..start + count]
            .iter_mut()
            .zip(encoded)
        {
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
fn write_chunk<F, W>(
    writer: &mut W,
    expected: StoredPolynomialLayoutV1,
    chunk: u64,
    values: &Column<F>,
    encoded: &mut Encoded,
) -> Result<(), StoredLookupErrorV1>
where
    F: StoredAssignmentFieldV1,
    W: StoredPolynomialWriterV1,
{
    if writer.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    let count = expected.chunk_scalar_count(chunk)?;
    let start = chunk as usize * TILE;
    encoded.clear();
    for (out, value) in encoded.0[..count]
        .iter_mut()
        .zip(&values.0.values[start..start + count])
    {
        *out = value.to_repr();
    }
    writer.write_chunk(chunk, &encoded.0[..count])?;
    if writer.layout() != expected {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    encoded.clear();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_destination<C: CurveAffine, S>(
    destination: StoredPolynomialLayoutV1,
    boundary: Boundary,
    completed: &[PermutedLookupV1<C, S>],
    current: &Current<C, S>,
    side: StoredLookupSideV1,
    basis: StoredPolynomialBasisV1,
    remaining_outputs: usize,
) -> Result<(), StoredLookupErrorV1>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    let end = boundary.membership_end;
    let last = current
        .table_coefficient
        .as_ref()
        .or(current.input_coefficient.as_ref())
        .or(current.table_lagrange.as_ref())
        .or(current.input_lagrange.as_ref())
        .map(|column| column.layout.ordinal())
        .or_else(|| {
            completed
                .last()
                .map(|pair| pair.table.coefficient.layout.ordinal())
        })
        .unwrap_or(end.ordinal());
    if destination.role()
        != role(
            u32::try_from(completed.len()).map_err(|_| StoredLookupErrorV1::Context)?,
            side,
        )
        || destination.basis() != basis
        || !destination.same_proof_context(end)
        || destination.field() != end.field()
        || destination.k() != end.k()
        || destination.ordinal() <= last
        || destination
            .ordinal()
            .checked_add(
                u64::try_from(remaining_outputs).map_err(|_| StoredLookupErrorV1::Context)?,
            )
            .is_none()
    {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lagrange<'params, C, P, R>(
    advice: CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>,
    provider: &mut P,
    rng: &mut R,
    originals: &[CompressedLookupV1<SnapshotOf<P>>],
    completed: &[PermutedLookupV1<C, SnapshotOf<P>>],
    current: &mut Current<C, SnapshotOf<P>>,
    remaining: &[MembershipLookupV1<SnapshotOf<P>>],
    boundary: Boundary,
    usable: usize,
    side: StoredLookupSideV1,
    scratch: &mut Scratch<C::Scalar>,
) -> Result<CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    P: StoredPolynomialProviderV1,
    R: RngCore,
{
    validate(
        &advice,
        originals,
        completed,
        Some(current),
        remaining,
        Some(boundary),
        usable,
    )?;
    let index = u32::try_from(completed.len()).map_err(|_| StoredLookupErrorV1::Context)?;
    let (advice, mut writer, destination) =
        advice.create_permuted_writer(provider, index, side, StoredPolynomialBasisV1::Lagrange)?;
    let remaining_outputs = remaining
        .len()
        .checked_mul(4)
        .and_then(|n| {
            n.checked_add(if side == StoredLookupSideV1::Input {
                3
            } else {
                2
            })
        })
        .ok_or(StoredLookupErrorV1::Context)?;
    validate_destination(
        destination,
        boundary,
        completed,
        current,
        side,
        StoredPolynomialBasisV1::Lagrange,
        remaining_outputs,
    )?;
    macro_rules! check {
        () => {{
            validate(
                &advice,
                originals,
                completed,
                Some(current),
                remaining,
                Some(boundary),
                usable,
            )?;
            if writer.layout() != destination {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
        }};
    }
    check!();
    scratch.column.clear();
    scratch.previous.clear();
    scratch.input.reset(0, usable);
    let leftovers = current
        .source
        .as_ref()
        .ok_or(StoredLookupErrorV1::Context)?
        .leftover_rows;
    scratch.leftover.reset(0, leftovers);
    let mut distinct = 0_usize;
    for row in 0..usable {
        let source = current
            .source
            .as_mut()
            .ok_or(StoredLookupErrorV1::Context)?;
        let (a, read) =
            scratch
                .input
                .peek(&mut source.input.snapshot, source.input.layout, usable)?;
        if read {
            check!();
        }
        let a = a.ok_or(StoredLookupErrorV1::Context)?;
        scratch.input.next += 1;
        if row != 0 && a < scratch.previous.0 {
            return Err(StoredLookupErrorV1::Context);
        }
        let first = row == 0 || a != scratch.previous.0;
        if first {
            distinct += 1;
        }
        scratch.column.0.values[row] = if side == StoredLookupSideV1::Input || first {
            a
        } else {
            let source = current
                .source
                .as_mut()
                .ok_or(StoredLookupErrorV1::Context)?;
            let (value, read) = scratch.leftover.peek(
                &mut source.leftover_table.snapshot,
                source.leftover_table.layout,
                leftovers,
            )?;
            if read {
                check!();
            }
            let value = value.ok_or(StoredLookupErrorV1::Context)?;
            scratch.leftover.next += 1;
            value
        };
        scratch.previous.0 = a;
    }
    let source = current
        .source
        .as_ref()
        .ok_or(StoredLookupErrorV1::Context)?;
    if scratch.input.next != usable
        || distinct != source.distinct_inputs
        || usable.checked_sub(distinct) != Some(leftovers)
        || (side == StoredLookupSideV1::Table && scratch.leftover.next != leftovers)
    {
        return Err(StoredLookupErrorV1::Context);
    }
    check!();
    for row in usable..destination.scalar_count() {
        scratch.column.0.values[row] = C::Scalar::random(&mut *rng);
        check!();
    }
    for chunk in 0..destination.chunk_count() as u64 {
        check!();
        write_chunk(
            &mut writer,
            destination,
            chunk,
            &scratch.column,
            &mut scratch.encoded,
        )?;
        check!();
    }
    let snapshot = writer.seal()?;
    if snapshot.layout() != destination {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    validate(
        &advice,
        originals,
        completed,
        Some(current),
        remaining,
        Some(boundary),
        usable,
    )?;
    let column = PermutedPolynomialV1 {
        layout: destination,
        snapshot,
    };
    match side {
        StoredLookupSideV1::Input => current.input_lagrange = Some(column),
        StoredLookupSideV1::Table => current.table_lagrange = Some(column),
    };
    validate(
        &advice,
        originals,
        completed,
        Some(current),
        remaining,
        Some(boundary),
        usable,
    )?;
    scratch.column.clear();
    scratch.input.reset(0, 0);
    scratch.leftover.reset(0, 0);
    scratch.previous.clear();
    Ok(advice)
}

#[allow(clippy::too_many_arguments)]
fn convert_commit<'params, C, P, R>(
    advice: CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>,
    provider: &mut P,
    rng: &mut R,
    params: &'params ParamsIPA<C>,
    pk: &ProvingKey<C>,
    originals: &[CompressedLookupV1<SnapshotOf<P>>],
    completed: &[PermutedLookupV1<C, SnapshotOf<P>>],
    current: &mut Current<C, SnapshotOf<P>>,
    remaining: &[MembershipLookupV1<SnapshotOf<P>>],
    boundary: Boundary,
    usable: usize,
    side: StoredLookupSideV1,
    scratch: &mut Scratch<C::Scalar>,
) -> Result<CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>, StoredLookupErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    R: RngCore,
{
    validate(
        &advice,
        originals,
        completed,
        Some(current),
        remaining,
        Some(boundary),
        usable,
    )?;
    let index = u32::try_from(completed.len()).map_err(|_| StoredLookupErrorV1::Context)?;
    let (advice, mut writer, destination) = advice.create_permuted_writer(
        provider,
        index,
        side,
        StoredPolynomialBasisV1::Coefficient,
    )?;
    let remaining_outputs = remaining
        .len()
        .checked_mul(4)
        .and_then(|n| n.checked_add(usize::from(side == StoredLookupSideV1::Input)))
        .ok_or(StoredLookupErrorV1::Context)?;
    validate_destination(
        destination,
        boundary,
        completed,
        current,
        side,
        StoredPolynomialBasisV1::Coefficient,
        remaining_outputs,
    )?;
    macro_rules! check {
        () => {{
            validate(
                &advice,
                originals,
                completed,
                Some(current),
                remaining,
                Some(boundary),
                usable,
            )?;
            if writer.layout() != destination {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
        }};
    }
    check!();
    scratch.column.clear();
    for chunk in 0..destination.chunk_count() as u64 {
        check!();
        let source = current.lagrange_mut(side)?;
        read_chunk(
            &mut source.snapshot,
            source.layout,
            chunk,
            &mut scratch.column,
        )?;
        check!();
    }
    pk.vk
        .domain
        .stored_column_transform_in_place(&mut scratch.column.0.values, true, None);
    check!();
    for chunk in 0..destination.chunk_count() as u64 {
        check!();
        write_chunk(
            &mut writer,
            destination,
            chunk,
            &scratch.column,
            &mut scratch.encoded,
        )?;
        check!();
    }
    let snapshot = writer.seal()?;
    if snapshot.layout() != destination {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    validate(
        &advice,
        originals,
        completed,
        Some(current),
        remaining,
        Some(boundary),
        usable,
    )?;
    let coefficient = PermutedPolynomialV1 {
        layout: destination,
        snapshot,
    };
    match side {
        StoredLookupSideV1::Input => current.input_coefficient = Some(coefficient),
        StoredLookupSideV1::Table => current.table_coefficient = Some(coefficient),
    };
    validate(
        &advice,
        originals,
        completed,
        Some(current),
        remaining,
        Some(boundary),
        usable,
    )?;
    // The sole commitment blind follows completed coefficient conversion, exactly as ordinary.
    let blind = SecretLookupBlindV1(Blind(C::Scalar::random(&mut *rng)));
    match side {
        StoredLookupSideV1::Input => current.input_blind = Some(blind),
        StoredLookupSideV1::Table => current.table_blind = Some(blind),
    };
    validate(
        &advice,
        originals,
        completed,
        Some(current),
        remaining,
        Some(boundary),
        usable,
    )?;
    // Reuse the same guarded n-field allocation; commit the original Lagrange rows, not FFT output.
    scratch.column.clear();
    for chunk in 0..destination.chunk_count() as u64 {
        validate(
            &advice,
            originals,
            completed,
            Some(current),
            remaining,
            Some(boundary),
            usable,
        )?;
        let source = current.lagrange_mut(side)?;
        read_chunk(
            &mut source.snapshot,
            source.layout,
            chunk,
            &mut scratch.column,
        )?;
        validate(
            &advice,
            originals,
            completed,
            Some(current),
            remaining,
            Some(boundary),
            usable,
        )?;
    }
    let blind = match side {
        StoredLookupSideV1::Input => current.input_blind.as_ref(),
        StoredLookupSideV1::Table => current.table_blind.as_ref(),
    }
    .ok_or(StoredLookupErrorV1::Context)?;
    let point = params
        .commit_lagrange(&scratch.column.0, blind.0)
        .to_affine();
    match side {
        StoredLookupSideV1::Input => current.input_commitment = Some(point),
        StoredLookupSideV1::Table => current.table_commitment = Some(point),
    };
    scratch.column.clear();
    validate(
        &advice,
        originals,
        completed,
        Some(current),
        remaining,
        Some(boundary),
        usable,
    )?;
    Ok(advice)
}

impl<'params, 'instances, C, P, R, T, E, const QUERY_INSTANCE: bool, const INSTANCE_MASK: u64>
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
    >
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    R: RngCore,
    E: EncodedChallenge<C>,
    T: TranscriptWrite<C, E>,
{
    /// Consume all validated memberships through the original ordinary permutation commitments.
    /// Adapter allocations and public budget/geometry arithmetic precede the first proof RNG.
    /// Writer capacity and failures remain the original backend's authority; no cap is raised.
    pub(crate) fn commit_permuted_lookups(
        self,
        scratch_limit_bytes: usize,
    ) -> Result<
        LookupPermutedPendingStoredIpaProverV1<
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
            lookups: members,
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
            mut rng,
            mut transcript,
            instances,
            _challenge,
        } = inner;
        advice.validate_for_lookup(&pk.vk.domain)?;
        let k = pk.vk.domain.k();
        let count = pk.vk.cs.lookups.len();
        let n = 1_usize.checked_shl(k).ok_or(StoredLookupErrorV1::Context)?;
        let usable = pk
            .vk
            .cs
            .blinding_factors()
            .checked_add(1)
            .and_then(|tail| n.checked_sub(tail))
            .ok_or(StoredLookupErrorV1::Context)?;
        if k > STORED_MAX_K_V1
            || params.k() != k
            || params.n() != n as u64
            || params.get_g_lagrange().len() != n
            || !ptr::eq(advice.params()?, params)
            || usable != usable_rows
            || count != members.len()
            || count != originals.len()
            || (count != 0 && usable == 0)
        {
            return Err(StoredLookupErrorV1::Context);
        }
        u32::try_from(count).map_err(|_| StoredLookupErrorV1::Context)?;
        let boundary = members.last().map(|pair| Boundary {
            sorted_input_end: pair.input.layout,
            membership_end: pair.leftover_table.layout,
        });
        validate(&advice, &originals, &[], None, &members, boundary, usable)?;
        let writers = count.checked_mul(4).ok_or(StoredLookupErrorV1::Context)?;
        if let Some(bounds) = boundary {
            bounds
                .membership_end
                .ordinal()
                .checked_add(u64::try_from(writers).map_err(|_| StoredLookupErrorV1::Context)?)
                .ok_or(StoredLookupErrorV1::Context)?;
            if bounds.membership_end.k() != k
                || bounds.membership_end.field() != C::Scalar::STORED_FIELD
            {
                return Err(StoredLookupErrorV1::Context);
            }
        }
        // Two advice bases, original compressed pairs and four permutation receipts per lookup.
        // Old scratch drops after both Lagrange replacements seal, so peak is at most 2a + 6l.
        advice
            .layouts()?
            .len()
            .checked_mul(2)
            .and_then(|a| count.checked_mul(6).and_then(|l| a.checked_add(l)))
            .ok_or(StoredLookupErrorV1::Context)?;
        if scratch_bytes::<C, SnapshotOf<P>>(k, count)? > scratch_limit_bytes {
            return Err(StoredLookupErrorV1::ScratchLimit);
        }
        let mut completed = reserved(count)?;
        // Allocate every adapter-owned witness buffer before the first tail draw; allocation
        // failures inside public FFT tables or the existing MSM remain outside this payload.
        let mut scratch = if count == 0 {
            None
        } else {
            Some(Scratch::<C::Scalar>::new(&pk.vk.domain)?)
        };
        let mut remaining = members.into_iter();
        while let Some(source) = remaining.next() {
            let bounds = boundary.ok_or(StoredLookupErrorV1::Context)?;
            let scratch = scratch.as_mut().ok_or(StoredLookupErrorV1::Context)?;
            let mut current = Current::new(source);
            advice = lagrange::<C, P, R>(
                advice,
                &mut provider,
                &mut rng,
                &originals,
                &completed,
                &mut current,
                remaining.as_slice(),
                bounds,
                usable,
                StoredLookupSideV1::Input,
                scratch,
            )?;
            advice = lagrange::<C, P, R>(
                advice,
                &mut provider,
                &mut rng,
                &originals,
                &completed,
                &mut current,
                remaining.as_slice(),
                bounds,
                usable,
                StoredLookupSideV1::Table,
                scratch,
            )?;
            validate(
                &advice,
                &originals,
                &completed,
                Some(&current),
                remaining.as_slice(),
                Some(bounds),
                usable,
            )?;
            drop(current.source.take());
            validate(
                &advice,
                &originals,
                &completed,
                Some(&current),
                remaining.as_slice(),
                Some(bounds),
                usable,
            )?;
            advice = convert_commit::<C, P, R>(
                advice,
                &mut provider,
                &mut rng,
                params,
                &pk,
                &originals,
                &completed,
                &mut current,
                remaining.as_slice(),
                bounds,
                usable,
                StoredLookupSideV1::Input,
                scratch,
            )?;
            advice = convert_commit::<C, P, R>(
                advice,
                &mut provider,
                &mut rng,
                params,
                &pk,
                &originals,
                &completed,
                &mut current,
                remaining.as_slice(),
                bounds,
                usable,
                StoredLookupSideV1::Table,
                scratch,
            )?;
            let input = current
                .input_commitment
                .ok_or(StoredLookupErrorV1::Context)?;
            let table = current
                .table_commitment
                .ok_or(StoredLookupErrorV1::Context)?;
            validate(
                &advice,
                &originals,
                &completed,
                Some(&current),
                remaining.as_slice(),
                Some(bounds),
                usable,
            )?;
            transcript
                .write_point(input)
                .map_err(|_| StoredLookupErrorV1::Transcript)?;
            validate(
                &advice,
                &originals,
                &completed,
                Some(&current),
                remaining.as_slice(),
                Some(bounds),
                usable,
            )?;
            transcript
                .write_point(table)
                .map_err(|_| StoredLookupErrorV1::Transcript)?;
            validate(
                &advice,
                &originals,
                &completed,
                Some(&current),
                remaining.as_slice(),
                Some(bounds),
                usable,
            )?;
            completed.push(current.finish()?);
        }
        validate(&advice, &originals, &completed, None, &[], boundary, usable)?;
        Ok(LookupPermutedPendingStoredIpaProverV1 {
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
