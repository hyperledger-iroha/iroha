//! Consuming lookup compression over exact-key expressions and authenticated bounded tiles.
//!
//! Theta is squeezed once from the original transcript after public preflight. Input and table
//! sides follow retained-key order and include every base-domain row. This stage draws no proof
//! randomness and writes no commitments. Every original and partial owner drops on refusal or
//! unwind; a successful result still owns all advice and coefficient receipts and original blinds.
//! TODO: implement canonical permutation, argument commitments/products, quotient and openings
//! from this continuation before exposing a complete stored proof or claiming process bounds.

use super::{
    CoefficientPendingStoredIpaProverV1,
    auxiliary::{KeyBaseInputs, key_expression_metadata, phase_error},
};
use crate::{
    arithmetic::CurveAffine,
    plonk::{
        ChallengeTheta,
        stored::{
            StoredAuxiliarySourceV1, StoredExpressionErrorV1, StoredExpressionPlanV1,
            StoredRowTileV1, prepare_stored_expression_v1,
        },
    },
    poly::stored_advice::{
        STORED_SCALAR_BYTES_V1, STORED_SCALARS_PER_CHUNK_V1, StoredLookupSideV1,
        StoredPolynomialBasisV1, StoredPolynomialErrorV1, StoredPolynomialLayoutV1,
        StoredPolynomialProviderV1, StoredPolynomialRoleV1, StoredPolynomialSnapshotV1,
        StoredPolynomialWriterV1,
        assignment::StoredAssignmentFieldV1,
        phase::{CoefficientStoredAdviceV1, StoredPhaseErrorV1},
    },
    transcript::{EncodedChallenge, Transcript},
};
use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use std::{
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

const TILE: usize = STORED_SCALARS_PER_CHUNK_V1;
type SnapshotOf<P> =
    <<P as StoredPolynomialProviderV1>::Writer as StoredPolynomialWriterV1>::Snapshot;

/// Coarse terminal refusal of the complete owned lookup-compression transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoredLookupErrorV1 {
    /// Key/parameter geometry, lookup coordinates or allocation arithmetic disagree.
    Context,
    /// The original transcript refused a permutation commitment point.
    Transcript,
    /// A distinct sorted input has no matching table occurrence.
    Membership,
    /// The explicitly supplied scratch budget cannot hold one bounded output tile.
    ScratchLimit,
    /// A checked public metadata or guarded tile allocation failed.
    Allocation,
    /// Authenticated storage or canonical encoding failed.
    Store(StoredPolynomialErrorV1),
    /// Exact-key expression planning or evaluation failed.
    Expression(StoredExpressionErrorV1),
    /// Original advice/coefficient ownership or continuation allocation failed.
    Phase(StoredPhaseErrorV1),
}
impl From<StoredPolynomialErrorV1> for StoredLookupErrorV1 {
    fn from(error: StoredPolynomialErrorV1) -> Self {
        Self::Store(error)
    }
}
impl From<StoredPhaseErrorV1> for StoredLookupErrorV1 {
    fn from(error: StoredPhaseErrorV1) -> Self {
        Self::Phase(error)
    }
}
impl From<StoredExpressionErrorV1> for StoredLookupErrorV1 {
    fn from(error: StoredExpressionErrorV1) -> Self {
        match error {
            StoredExpressionErrorV1::ScratchLimit => Self::ScratchLimit,
            other => Self::Expression(other),
        }
    }
}

/// One sealed role-bound compression result, with no commitment or independently copied blind.
pub(super) struct CompressedLookupColumnV1<S> {
    pub(super) layout: StoredPolynomialLayoutV1,
    pub(super) snapshot: S,
}
/// Input and table receipts for one retained-key lookup, in that order.
pub(super) struct CompressedLookupV1<S> {
    pub(super) input: CompressedLookupColumnV1<S>,
    pub(super) table: CompressedLookupColumnV1<S>,
}

/// Inseparable original protocol owner plus theta and compressed lookup receipts.
///
/// No detached constructor, clone, snapshot accessor, proof accessor or second theta transition
/// exists. Later argument construction must consume this owner and preserve ordinary ordering.
#[allow(dead_code)]
pub(crate) struct LookupCompressedPendingStoredIpaProverV1<
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
    pub(super) inner: CoefficientPendingStoredIpaProverV1<
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
    pub(super) theta: ChallengeTheta<C>,
    pub(super) lookups: Vec<CompressedLookupV1<SnapshotOf<P>>>,
}

struct LookupPlans<'metadata, F> {
    index: u32,
    input: Vec<StoredExpressionPlanV1<'metadata, F>>,
    table: Vec<StoredExpressionPlanV1<'metadata, F>>,
}

fn reserved<T>(count: usize) -> Result<Vec<T>, StoredLookupErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| StoredLookupErrorV1::Allocation)?;
    Ok(result)
}

fn tile_payload_bytes<F>() -> Result<usize, StoredLookupErrorV1> {
    TILE.checked_mul(std::mem::size_of::<F>())
        .and_then(|fields| {
            TILE.checked_mul(STORED_SCALAR_BYTES_V1)
                .and_then(|encoded| fields.checked_add(encoded))
        })
        .ok_or(StoredLookupErrorV1::Context)
}

struct FieldTile<F: StoredAssignmentFieldV1>(Vec<F>);
impl<F: StoredAssignmentFieldV1> FieldTile<F> {
    fn new() -> Result<Self, StoredLookupErrorV1> {
        let mut result = Self(reserved(TILE)?);
        result.0.resize(TILE, F::ZERO);
        Ok(result)
    }
    fn clear(&mut self) {
        for value in &mut self.0 {
            // SAFETY: exclusively owned initialized Copy Pasta scalar slots; ZERO is valid.
            unsafe {
                ptr::write_volatile(value, F::ZERO);
            }
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        FIELD_CLEAR_OBSERVATION.with(|record| {
            let (count, all_zero) = record.get();
            record.set((
                count + self.0.len(),
                all_zero && self.0.iter().all(|value| *value == F::ZERO),
            ));
        });
    }
}
impl<F: StoredAssignmentFieldV1> Drop for FieldTile<F> {
    fn drop(&mut self) {
        self.clear();
    }
}
struct EncodedTile(Vec<[u8; STORED_SCALAR_BYTES_V1]>);
impl EncodedTile {
    fn new() -> Result<Self, StoredLookupErrorV1> {
        let mut result = Self(reserved(TILE)?);
        result.0.resize(TILE, [0; STORED_SCALAR_BYTES_V1]);
        Ok(result)
    }
    fn clear(&mut self) {
        for value in &mut self.0 {
            // SAFETY: exclusively owned initialized byte arrays; every bit pattern is valid.
            unsafe {
                ptr::write_volatile(value, [0; STORED_SCALAR_BYTES_V1]);
            }
        }
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        ENCODED_CLEAR_OBSERVATION.with(|record| {
            let (count, all_zero) = record.get();
            record.set((
                count + self.0.len(),
                all_zero
                    && self
                        .0
                        .iter()
                        .all(|value| *value == [0; STORED_SCALAR_BYTES_V1]),
            ));
        });
    }
}
impl Drop for EncodedTile {
    fn drop(&mut self) {
        self.clear();
    }
}

pub(super) fn validate_outputs<S: StoredPolynomialSnapshotV1>(
    completed: &[CompressedLookupV1<S>],
    input: Option<&CompressedLookupColumnV1<S>>,
) -> Result<(), StoredLookupErrorV1> {
    let mut last: Option<StoredPolynomialLayoutV1> = None;
    let mut check = |column: &CompressedLookupColumnV1<S>, lookup: u32, side| {
        let expected = column.layout;
        if expected.role() != (StoredPolynomialRoleV1::LookupCompressed { lookup, side })
            || expected.basis() != StoredPolynomialBasisV1::Lagrange
            || column.snapshot.layout() != expected
            || last.is_some_and(|old| {
                !old.same_proof_context(expected)
                    || old.field() != expected.field()
                    || old.k() != expected.k()
                    || old.ordinal() >= expected.ordinal()
            })
        {
            return Err(StoredLookupErrorV1::Store(StoredPolynomialErrorV1::Context));
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

#[allow(clippy::too_many_arguments)]
fn compress_side<'params, C, P, A>(
    advice: CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>,
    provider: &mut P,
    plans: &[StoredExpressionPlanV1<'_, C::Scalar>],
    auxiliary: &mut A,
    challenges: &[C::Scalar],
    theta: C::Scalar,
    role: StoredPolynomialRoleV1,
    completed: &[CompressedLookupV1<SnapshotOf<P>>],
    input: Option<&CompressedLookupColumnV1<SnapshotOf<P>>>,
) -> Result<
    (
        CoefficientStoredAdviceV1<'params, C, SnapshotOf<P>>,
        CompressedLookupColumnV1<SnapshotOf<P>>,
    ),
    StoredLookupErrorV1,
>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    P: StoredPolynomialProviderV1,
    A: StoredAuxiliarySourceV1<C::Scalar>,
{
    validate_outputs(completed, input)?;
    let (mut advice, mut writer, destination) = advice.create_output_writer(provider, role)?;
    let mut accumulator = FieldTile::<C::Scalar>::new()?;
    // No encoding allocation overlaps an expression evaluator's callback. It is allocated
    // after each evaluation tile below; the conservative public budget includes both buffers.
    for chunk in 0..destination.chunk_count() as u64 {
        if writer.layout() != destination {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        advice.validate_live_receipts()?;
        validate_outputs(completed, input)?;
        let len = destination.chunk_scalar_count(chunk)?;
        let start = usize::try_from(chunk)
            .ok()
            .and_then(|chunk| chunk.checked_mul(TILE))
            .ok_or(StoredLookupErrorV1::Context)?;
        let tile = StoredRowTileV1 { start, len };
        for plan in plans {
            if writer.layout() != destination {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            let (restored, ()) =
                advice.with_expression_sources(plan, tile, auxiliary, challenges, |values| {
                    if values.len() != len {
                        return Err(StoredExpressionErrorV1::Context);
                    }
                    for (acc, value) in accumulator.0[..len].iter_mut().zip(values) {
                        *acc = *acc * theta + *value;
                    }
                    Ok(())
                })?;
            advice = restored;
            if writer.layout() != destination {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            validate_outputs(completed, input)?;
        }
        // Empty expression lists write an actual ZERO polynomial, including inactive rows.
        // All authenticated read callbacks have ended before canonical encoding and writing.
        let mut encoded = EncodedTile::new()?;
        for (out, value) in encoded.0[..len].iter_mut().zip(&accumulator.0[..len]) {
            *out = value.to_repr();
        }
        if writer.layout() != destination {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        writer.write_chunk(chunk, &encoded.0[..len])?;
        if writer.layout() != destination {
            return Err(StoredPolynomialErrorV1::Context.into());
        }
        advice.validate_live_receipts()?;
        validate_outputs(completed, input)?;
        accumulator.clear();
        drop(encoded);
    }
    if writer.layout() != destination {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    let snapshot = writer.seal()?;
    if snapshot.layout() != destination {
        return Err(StoredPolynomialErrorV1::Context.into());
    }
    advice.validate_live_receipts()?;
    validate_outputs(completed, input)?;
    Ok((
        advice,
        CompressedLookupColumnV1 {
            layout: destination,
            snapshot,
        },
    ))
}

impl<'params, 'instances, C, P, R, T, E, const QUERY_INSTANCE: bool, const INSTANCE_MASK: u64>
    CoefficientPendingStoredIpaProverV1<
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
    E: EncodedChallenge<C>,
    T: Transcript<C, E>,
{
    /// Consume the original coefficient owner into exact-key compressed lookup receipts.
    ///
    /// A scratch limit is the only caller input. Planning and geometry checks precede theta
    /// and witness reads. No proof RNG call or point write occurs. The successful owner cannot
    /// squeeze theta again; failures and unwinding destroy original and partial owners together.
    pub(crate) fn compress_lookups(
        self,
        scratch_limit_bytes: usize,
    ) -> Result<
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
        >,
        StoredLookupErrorV1,
    > {
        let Self {
            params,
            pk,
            mut advice,
            mut provider,
            rng,
            mut transcript,
            instances,
            _challenge,
        } = self;
        advice.validate_for_lookup(&pk.vk.domain)?;
        if !std::ptr::eq(advice.params()?, params) {
            return Err(StoredLookupErrorV1::Context);
        }
        let metadata = key_expression_metadata(
            &pk,
            params,
            instances,
            advice.layouts().map_err(phase_error)?,
            advice.challenges().map_err(phase_error)?,
        )?;
        let count = pk.vk.cs.lookups.len();
        count.checked_mul(2).ok_or(StoredLookupErrorV1::Context)?;
        let mut plans = reserved(count)?;
        let mut lookups = reserved(count)?;
        let expression_budget = if count == 0 {
            0
        } else {
            scratch_limit_bytes
                .checked_sub(tile_payload_bytes::<C::Scalar>()?)
                .ok_or(StoredLookupErrorV1::ScratchLimit)?
        };
        for (index, lookup) in pk.vk.cs.lookups.iter().enumerate() {
            let prepare = |expressions: &[crate::plonk::Expression<C::Scalar>]| {
                let mut result = reserved(expressions.len())?;
                for expression in expressions {
                    result.push(prepare_stored_expression_v1(
                        expression,
                        metadata.context(),
                        expression_budget,
                    )?);
                }
                Ok::<_, StoredLookupErrorV1>(result)
            };
            plans.push(LookupPlans {
                index: u32::try_from(index).map_err(|_| StoredLookupErrorV1::Context)?,
                input: prepare(&lookup.input_expressions)?,
                table: prepare(&lookup.table_expressions)?,
            });
        }
        // Ordinary proving squeezes theta even for zero lookups. This exact transcript owner
        // advances only here; no metadata, storage entropy or canonical chunk enters it.
        let theta: ChallengeTheta<C> = transcript.squeeze_challenge_scalar();
        {
            let mut auxiliary = KeyBaseInputs::new(&pk, params, instances, metadata.domain);
            for plan in &plans {
                let (restored, input) = compress_side::<C, P, _>(
                    advice,
                    &mut provider,
                    &plan.input,
                    &mut auxiliary,
                    &metadata.challenges,
                    *theta,
                    StoredPolynomialRoleV1::LookupCompressed {
                        lookup: plan.index,
                        side: StoredLookupSideV1::Input,
                    },
                    &lookups,
                    None,
                )?;
                advice = restored;
                let (restored, table) = compress_side::<C, P, _>(
                    advice,
                    &mut provider,
                    &plan.table,
                    &mut auxiliary,
                    &metadata.challenges,
                    *theta,
                    StoredPolynomialRoleV1::LookupCompressed {
                        lookup: plan.index,
                        side: StoredLookupSideV1::Table,
                    },
                    &lookups,
                    Some(&input),
                )?;
                advice = restored;
                lookups.push(CompressedLookupV1 { input, table });
            }
        }
        advice.validate_live_receipts()?;
        validate_outputs(&lookups, None)?;
        drop(plans);
        drop(metadata);
        Ok(LookupCompressedPendingStoredIpaProverV1 {
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
        })
    }
}

#[cfg(test)]
thread_local! {
    static FIELD_CLEAR_OBSERVATION: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, true)) };
    static ENCODED_CLEAR_OBSERVATION: std::cell::Cell<(usize, bool)> = const { std::cell::Cell::new((0, true)) };
}

#[cfg(test)]
#[path = "lookup_unit_tests.rs"]
mod unit_tests;
