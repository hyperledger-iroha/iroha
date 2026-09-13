//! Fallible phase commitments for an explicitly admitted, discard-only IPA producer.
//!
//! This owner does not implement general `Assignment`, replay synthesis, handle instances, or
//! create a proof. The caller retains the existing phase-zero instance transcript prefix and
//! drops its final owned producer before post-synthesis final-phase randomness. All tail draws
//! precede all commitment-blind draws, and all advice points precede every phase challenge.
//! Final absorption can be consumed into globally ordered receipts with bounded chunk access;
//! this retains the original snapshots and blind guards without enabling another phase.
//!
//! Pending assignments retain two scalar chunks per column. Commitment reads retain one guarded
//! complete polynomial plus the backend chunk, never an advice-polynomial bank. Actual
//! `ParamsIPA::commit_lagrange` still allocates witness-dependent MSM scratch whose cleanup and
//! memory are outside this adapter's guards. This is not confidential-MSM, RSS, full-proof or
//! production qualification. TODO: bound and wipe MSM scratch, and integrate stored arguments,
//! quotient queries and openings before adopting this component in a consuming prover.

use std::{
    fmt,
    marker::PhantomData,
    ptr,
    sync::atomic::{Ordering, compiler_fence},
};

use ff::{Field, PrimeField, WithSmallOrderMulGroup};
use group::Curve;
use halo2curves::CurveAffine;
use rand_core::RngCore;

use super::{
    STORED_MAX_K_V1, STORED_SCALARS_PER_CHUNK_V1, StoredAdviceErrorV1, StoredAdviceLayoutV1,
    StoredAdviceSnapshotV1, StoredAdviceWriterV1, StoredPastaFieldV1, StoredPolynomialBasisV1,
    assignment::{StoredAdviceAssignmentV1, StoredAssignmentErrorV1, StoredAssignmentFieldV1},
};
use crate::{
    plonk::{Assigned, ConstraintSystem},
    poly::{
        EvaluationDomain, LagrangeCoeff, Polynomial,
        commitment::{Blind, Params},
        ipa::commitment::ParamsIPA,
    },
    transcript::{EncodedChallenge, TranscriptWrite},
};

/// Coarse errors at the complete phase boundary; no witness values or backend text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StoredPhaseErrorV1 {
    /// Constraint-system, phase, parameter or immutable-column admission mismatch.
    Admission,
    /// A per-column assignment failed.
    Assignment(StoredAssignmentErrorV1),
    /// An authenticated backend operation failed.
    Store(StoredAdviceErrorV1),
    /// A previous failed or unwound assignment or completed-column read destroyed this owner.
    Poisoned,
    /// A reference-returning assignment was refused.
    ReferenceReturn,
    /// An unknown witness value was refused.
    UnknownValue,
    /// Transcript writing failed; the complete proof transcript must be discarded.
    Transcript,
}

impl fmt::Display for StoredPhaseErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "stored phase: {self:?}")
    }
}
impl std::error::Error for StoredPhaseErrorV1 {}
impl From<StoredAdviceErrorV1> for StoredPhaseErrorV1 {
    fn from(error: StoredAdviceErrorV1) -> Self {
        Self::Store(error)
    }
}
impl From<StoredAssignmentErrorV1> for StoredPhaseErrorV1 {
    fn from(error: StoredAssignmentErrorV1) -> Self {
        Self::Assignment(error)
    }
}

fn reserved<T>(count: usize) -> Result<Vec<T>, StoredPhaseErrorV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| StoredAdviceErrorV1::Allocation)?;
    Ok(values)
}

#[derive(Debug)]
struct PhasePlan {
    columns: Vec<usize>,
    challenges: Vec<usize>,
}

/// Immutable schedule derived directly from the authoritative optimized constraint system.
pub(crate) struct StoredPhasePlanV1<'params, C: CurveAffine> {
    params: &'params ParamsIPA<C>,
    field: StoredPastaFieldV1,
    k: u32,
    usable_rows: usize,
    columns: usize,
    instance_columns: usize,
    challenges: usize,
    phases: Vec<PhasePlan>,
}

impl<C: CurveAffine> fmt::Debug for StoredPhasePlanV1<'_, C> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Parameters contain complete public generator tables; keep diagnostics bounded.
        out.debug_struct("StoredPhasePlanV1")
            .field("field", &self.field)
            .field("k", &self.k)
            .field("usable_rows", &self.usable_rows)
            .field("columns", &self.columns)
            .field("challenges", &self.challenges)
            .field("phases", &self.phases)
            .finish_non_exhaustive()
    }
}

/// Validate parameter geometry and derive unique phase/column/challenge order before side effects.
///
/// The immutable parameter reference is retained through all phase transitions, so a later
/// phase cannot substitute a same-degree parameter set or mutate the admitted one. The caller
/// must pass the proving key's matching parameters and optimized constraint system. Configuration-wide
/// producer admission is separate: no arbitrary `Circuit` is admitted by this constructor.
pub(crate) fn admit_stored_phase_plan_v1<'params, C>(
    params: &'params ParamsIPA<C>,
    domain: &EvaluationDomain<C::Scalar>,
    meta: &ConstraintSystem<C::Scalar>,
) -> Result<StoredPhasePlanV1<'params, C>, StoredPhaseErrorV1>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
{
    let k = domain.k();
    if k > STORED_MAX_K_V1
        || params.k() != k
        || params.n() != 1_u64 << k
        || params.get_g_lagrange().len() != 1_usize << k
        || meta.advice_column_phase.len() != meta.num_advice_columns
        || meta.challenge_phase.len() != meta.num_challenges
        || meta.num_advice_columns > u32::MAX as usize
    {
        return Err(StoredPhaseErrorV1::Admission);
    }
    let unusable = meta
        .blinding_factors()
        .checked_add(1)
        .ok_or(StoredPhaseErrorV1::Admission)?;
    let usable_rows = (1_usize << k)
        .checked_sub(unusable)
        .ok_or(StoredPhaseErrorV1::Admission)?;
    let mut counts = [0_usize; 3];
    let mut challenge_counts = [0_usize; 3];
    let mut last_phase = 0;
    for phase in &meta.advice_column_phase {
        let index = phase.to_u8() as usize;
        if index >= 3 {
            return Err(StoredPhaseErrorV1::Admission);
        }
        counts[index] += 1;
        last_phase = last_phase.max(index);
    }
    // Public CS construction requires every predecessor advice phase to exist. Preserve
    // that invariant when admitting optimized metadata; only the zero-advice CS has an
    // empty phase zero.
    if meta.num_advice_columns != 0 && counts[..=last_phase].iter().any(|count| *count == 0) {
        return Err(StoredPhaseErrorV1::Admission);
    }
    for phase in &meta.challenge_phase {
        let index = phase.to_u8() as usize;
        if index >= 3 || index > last_phase || counts[index] == 0 {
            return Err(StoredPhaseErrorV1::Admission);
        }
        challenge_counts[index] += 1;
    }
    let mut phases = reserved(last_phase + 1)?;
    for phase in 0..=last_phase {
        phases.push(PhasePlan {
            columns: reserved(counts[phase])?,
            challenges: reserved(challenge_counts[phase])?,
        });
    }
    for (column, phase) in meta.advice_column_phase.iter().enumerate() {
        phases[phase.to_u8() as usize].columns.push(column);
    }
    for (challenge, phase) in meta.challenge_phase.iter().enumerate() {
        phases[phase.to_u8() as usize].challenges.push(challenge);
    }
    Ok(StoredPhasePlanV1 {
        params,
        field: C::Scalar::STORED_FIELD,
        k,
        usable_rows,
        columns: meta.num_advice_columns,
        instance_columns: meta.num_instance_columns,
        challenges: meta.num_challenges,
        phases,
    })
}

struct SecretBlind<F: StoredAssignmentFieldV1>(Blind<F>);
impl<F: StoredAssignmentFieldV1> Drop for SecretBlind<F> {
    fn drop(&mut self) {
        // SAFETY: this exclusively owned initialized Pasta scalar is Copy and has no destructor.
        unsafe { ptr::write_volatile(&mut self.0.0, F::ZERO) };
        compiler_fence(Ordering::SeqCst);
        #[cfg(test)]
        tests::record_blind_drop(self.0.0);
    }
}

struct GuardedPolynomial<F: StoredAssignmentFieldV1>(Polynomial<F, LagrangeCoeff>);
impl<F: StoredAssignmentFieldV1> GuardedPolynomial<F> {
    fn zeroed(count: usize) -> Result<Self, StoredPhaseErrorV1> {
        let mut values = reserved(count)?;
        values.resize(count, F::ZERO);
        Ok(Self(Polynomial {
            values,
            _marker: PhantomData,
        }))
    }
    fn clear(&mut self) {
        for value in &mut self.0.values {
            // SAFETY: each exclusively owned, initialized field slot is a sealed Copy Pasta field.
            unsafe { ptr::write_volatile(value, F::ZERO) };
        }
        compiler_fence(Ordering::SeqCst);
    }
}
impl<F: StoredAssignmentFieldV1> Drop for GuardedPolynomial<F> {
    fn drop(&mut self) {
        self.clear();
    }
}

struct StoredColumn<C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    layout: StoredAdviceLayoutV1,
    snapshot: S,
    blind: SecretBlind<C::Scalar>,
}

struct Session<'params, C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    plan: StoredPhasePlanV1<'params, C>,
    next_phase: usize,
    proof_context: Option<[u8; 32]>,
    greatest_ordinal: Option<u64>,
    columns: Vec<StoredColumn<C, S>>,
    challenges: Vec<Option<C::Scalar>>,
}

struct Active<'params, C: CurveAffine, W: StoredAdviceWriterV1>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    session: Session<'params, C, W::Snapshot>,
    assignments: Vec<StoredAdviceAssignmentV1<C::Scalar, W>>,
}

/// One current phase plus all prior receipts; any refused assignment destroys the whole owner.
///
/// No general synthesis callback is accepted. Callers supply only known values from a reviewed
/// producer and propagate errors. A caught panic also leaves this owner poisoned.
pub(crate) struct StoredPhaseAssignmentsV1<'params, C: CurveAffine, W: StoredAdviceWriterV1>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    active: Option<Active<'params, C, W>>,
}

/// Finalized and committed columns awaiting fallible transcript absorption; no challenge exists yet.
pub(crate) struct PreparedStoredPhaseV1<'params, C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    session: Session<'params, C, S>,
    columns: Vec<StoredColumn<C, S>>,
    commitments: Vec<C>,
}

/// Successfully absorbed phase, holding the only cursor allowed to begin the next phase.
pub(crate) struct CommittedStoredPhaseV1<'params, C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    session: Session<'params, C, S>,
}

/// Complete, globally ordered advice receipts for bounded argument and opening consumers.
///
/// This move-only owner retains the exact admitted parameter reference, immutable layouts,
/// authenticated snapshots, guarded commitment blinds and transcript challenges. It does not
/// admit a producer, create a proof, reconstruct a circuit, or materialize a polynomial bank.
/// Only a chunk callback can inspect encoded witness values; no snapshot handle is exposed.
/// Every failed or unwound chunk operation destroys all receipts and poisons this owner.
pub(crate) struct CompleteStoredAdviceV1<'params, C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    session: Option<Session<'params, C, S>>,
}

impl<'params, C, W> StoredPhaseAssignmentsV1<'params, C, W>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    W: StoredAdviceWriterV1,
{
    /// Begin phase zero from an admitted schedule and exact ordered writer set.
    ///
    /// All vectors are reserved before any assignment, proof RNG, or transcript operation.
    pub(crate) fn begin(
        plan: StoredPhasePlanV1<'params, C>,
        writers: Vec<W>,
    ) -> Result<Self, StoredPhaseErrorV1> {
        if plan.field != C::Scalar::STORED_FIELD {
            return Err(StoredPhaseErrorV1::Admission);
        }
        let mut challenges = reserved(plan.challenges)?;
        challenges.resize(plan.challenges, None);
        let columns = reserved(plan.columns)?;
        Self::begin_session(
            Session {
                plan,
                next_phase: 0,
                proof_context: None,
                greatest_ordinal: None,
                columns,
                challenges,
            },
            writers,
        )
    }

    fn begin_session(
        mut session: Session<'params, C, W::Snapshot>,
        writers: Vec<W>,
    ) -> Result<Self, StoredPhaseErrorV1> {
        let phase = session
            .plan
            .phases
            .get(session.next_phase)
            .ok_or(StoredPhaseErrorV1::Admission)?;
        if writers.len() != phase.columns.len() {
            return Err(StoredPhaseErrorV1::Admission);
        }
        // Validate every writer before constructing any per-column owner. Strictly increasing
        // ordinals across phases imply uniqueness, including a provider's burned ordinals.
        let mut admitted_layouts = reserved(writers.len())?;
        for (writer, column) in writers.iter().zip(&phase.columns) {
            let layout = writer.layout();
            if layout.proof_context == [0; 32]
                || layout.field() != session.plan.field
                || layout.k() != session.plan.k
                || layout.basis() != StoredPolynomialBasisV1::Lagrange
                || layout.phase() as usize != session.next_phase
                || layout.column() as usize != *column
                || session
                    .proof_context
                    .is_some_and(|context| context != layout.proof_context)
                || session
                    .greatest_ordinal
                    .is_some_and(|ordinal| ordinal >= layout.ordinal())
            {
                return Err(StoredPhaseErrorV1::Admission);
            }
            session.proof_context = Some(layout.proof_context);
            session.greatest_ordinal = Some(layout.ordinal());
            admitted_layouts.push(layout);
        }
        let mut assignments = reserved(writers.len())?;
        for (writer, layout) in writers.into_iter().zip(admitted_layouts) {
            // Reusing the complete first-pass identity prevents a backend from substituting
            // a previously admitted ordinal while the per-column owners are constructed.
            if writer.layout() != layout {
                return Err(StoredPhaseErrorV1::Admission);
            }
            assignments.push(StoredAdviceAssignmentV1::new(
                writer,
                layout,
                session.plan.usable_rows,
            )?);
        }
        Ok(Self {
            active: Some(Active {
                session,
                assignments,
            }),
        })
    }

    /// Assign an admitted current-phase column. Every error, including duplicate/backwards
    /// rows and wrong-phase columns, destroys all current and previously committed snapshots.
    pub(crate) fn assign_discarding_value(
        &mut self,
        column: usize,
        row: usize,
        value: Assigned<C::Scalar>,
    ) -> Result<(), StoredPhaseErrorV1> {
        let mut active = self.active.take().ok_or(StoredPhaseErrorV1::Poisoned)?;
        let phase = &active.session.plan.phases[active.session.next_phase];
        let index = phase
            .columns
            .binary_search(&column)
            .map_err(|_| StoredPhaseErrorV1::Admission)?;
        active.assignments[index].assign_discarding_value(row, value)?;
        self.active = Some(active);
        Ok(())
    }

    /// Refuse an escaping reference and invalidate the complete proof owner.
    pub(crate) fn reject_reference_return(&mut self) -> Result<(), StoredPhaseErrorV1> {
        let _active = self.active.take().ok_or(StoredPhaseErrorV1::Poisoned)?;
        Err(StoredPhaseErrorV1::ReferenceReturn)
    }

    /// Refuse an unknown witness without substituting zero or allowing later recovery.
    pub(crate) fn reject_unknown_value(&mut self) -> Result<(), StoredPhaseErrorV1> {
        let _active = self.active.take().ok_or(StoredPhaseErrorV1::Poisoned)?;
        Err(StoredPhaseErrorV1::UnknownValue)
    }

    /// Finish all tails, draw all blinds, and commit one guarded Lagrange column at a time.
    /// Uses only the immutable parameter reference retained during initial admission.
    ///
    /// The caller must already have dropped its final owned producer before invoking final
    /// post-synthesis commitment. Storage failure may consume RNG; discard this complete proof
    /// attempt rather than rewinding RNG or retrying with a partially used transcript.
    pub(crate) fn finish<R: RngCore>(
        mut self,
        rng: &mut R,
    ) -> Result<PreparedStoredPhaseV1<'params, C, W::Snapshot>, StoredPhaseErrorV1> {
        let active = self.active.take().ok_or(StoredPhaseErrorV1::Poisoned)?;
        let Active {
            session,
            assignments,
        } = active;
        let params = session.plan.params;
        if params.k() != session.plan.k
            || params.n() != 1_u64 << session.plan.k
            || params.get_g_lagrange().len() != 1_usize << session.plan.k
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        let count = assignments.len();
        let mut snapshots = reserved(count)?;
        let mut blinds = reserved(count)?;
        let mut projective = reserved(count)?;
        let mut commitments = reserved(count)?;
        commitments.resize(count, C::identity());
        let mut columns = reserved(count)?;
        // Never interleave a column's tail and its commitment blind.
        for assignment in assignments {
            let expected = assignment.layout();
            let snapshot = assignment.finish_with_tail(|_| Ok(C::Scalar::random(&mut *rng)))?;
            snapshots.push((expected, snapshot));
        }
        for _ in 0..count {
            blinds.push(SecretBlind(Blind(C::Scalar::random(&mut *rng))));
        }
        for ((expected, mut snapshot), blind) in snapshots.into_iter().zip(blinds) {
            let mut polynomial = GuardedPolynomial::<C::Scalar>::zeroed(expected.scalar_count())?;
            for chunk in 0..expected.chunk_count() as u64 {
                if snapshot.layout() != expected {
                    return Err(StoredAdviceErrorV1::Context.into());
                }
                let count = expected.chunk_scalar_count(chunk)?;
                let start = chunk as usize * STORED_SCALARS_PER_CHUNK_V1;
                snapshot.with_chunk(expected, chunk, |encoded| {
                    if encoded.len() != count {
                        return Err(StoredAdviceErrorV1::Encoding);
                    }
                    for (index, value) in encoded.iter().enumerate() {
                        polynomial.0.values[start + index] =
                            Option::<C::Scalar>::from(C::Scalar::from_repr(*value))
                                .ok_or(StoredAdviceErrorV1::Encoding)?;
                    }
                    Ok(())
                })?;
            }
            if snapshot.layout() != expected {
                return Err(StoredAdviceErrorV1::Context.into());
            }
            projective.push(params.commit_lagrange(&polynomial.0, blind.0));
            // The polynomial guard drops before the next snapshot materializes. MSM scratch
            // is owned by the existing commitment backend and is explicitly outside this wipe.
            drop(polynomial);
            columns.push(StoredColumn {
                layout: expected,
                snapshot,
                blind,
            });
        }
        C::Curve::batch_normalize(&projective, &mut commitments);
        Ok(PreparedStoredPhaseV1 {
            session,
            columns,
            commitments,
        })
    }
}

impl<'params, C, S> PreparedStoredPhaseV1<'params, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredAdviceSnapshotV1,
{
    /// Write all advice points before squeezing any phase challenge. On an I/O error the
    /// owner is consumed, no challenge is squeezed, and the caller must discard the transcript.
    /// There is no rollback promise for arbitrary transcript implementations or their panics.
    pub(crate) fn absorb<E, T>(
        mut self,
        transcript: &mut T,
    ) -> Result<CommittedStoredPhaseV1<'params, C, S>, StoredPhaseErrorV1>
    where
        E: EncodedChallenge<C>,
        T: TranscriptWrite<C, E>,
    {
        let phase = &self.session.plan.phases[self.session.next_phase];
        if self.columns.len() != phase.columns.len()
            || self.commitments.len() != phase.columns.len()
            || phase
                .challenges
                .iter()
                .any(|index| self.session.challenges[*index].is_some())
            || self
                .columns
                .iter()
                .zip(&phase.columns)
                .any(|(column, index)| {
                    column.layout.column() as usize != *index
                        || column.snapshot.layout() != column.layout
                })
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        // No storage, allocation, or remaining fallible checks after the first challenge.
        for point in self.commitments {
            transcript
                .write_point(point)
                .map_err(|_| StoredPhaseErrorV1::Transcript)?;
        }
        self.session.columns.extend(self.columns);
        for index in &phase.challenges {
            self.session.challenges[*index] = Some(*transcript.squeeze_challenge_scalar::<()>());
        }
        self.session.next_phase += 1;
        Ok(CommittedStoredPhaseV1 {
            session: self.session,
        })
    }
}

impl<'params, C, S> CommittedStoredPhaseV1<'params, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredAdviceSnapshotV1,
{
    /// Return a challenge only after its authoritative phase has successfully absorbed.
    pub(crate) fn challenge(&self, index: usize) -> Option<C::Scalar> {
        self.session.challenges.get(index).copied().flatten()
    }

    /// Whether every configured phase, including an empty zero-advice phase, absorbed once.
    pub(crate) fn is_complete(&self) -> bool {
        self.session.next_phase == self.session.plan.phases.len()
    }

    /// Consume the final absorbed phase and move its receipts into global advice-column order.
    ///
    /// The complete authoritative schedule and each retained snapshot identity are checked
    /// before access is enabled. No storage read, RNG draw, transcript operation, witness
    /// allocation or blind copy occurs here. An incomplete or inconsistent cursor is consumed
    /// on error, releasing every snapshot and wiping every owned blind. A zero-advice circuit
    /// must still absorb its empty phase and retains no invented proof context.
    pub(crate) fn into_complete(
        self,
    ) -> Result<CompleteStoredAdviceV1<'params, C, S>, StoredPhaseErrorV1> {
        let mut session = self.session;
        let plan = &session.plan;
        if plan.phases.is_empty()
            || plan.phases.len() > 3
            || session.next_phase != plan.phases.len()
            || session.columns.len() != plan.columns
            || session.challenges.len() != plan.challenges
            || session.challenges.iter().any(Option::is_none)
            || plan.field != C::Scalar::STORED_FIELD
            || plan.k > STORED_MAX_K_V1
            || plan.params.k() != plan.k
            || plan.params.n() != 1_u64 << plan.k
            || plan.params.get_g_lagrange().len() != 1_usize << plan.k
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        // Receipts accumulate in phase order, which need not be global column order. Verify
        // that original order before moving them, including the cross-phase ordinal chain.
        let mut columns = session.columns.iter();
        let mut greatest_ordinal = None;
        for (phase_index, phase) in plan.phases.iter().enumerate() {
            for index in &phase.columns {
                let column = columns.next().ok_or(StoredPhaseErrorV1::Admission)?;
                let layout = column.layout;
                if *index >= plan.columns
                    || layout.column() as usize != *index
                    || layout.phase() as usize != phase_index
                    || layout.field() != plan.field
                    || layout.k() != plan.k
                    || layout.basis() != StoredPolynomialBasisV1::Lagrange
                    || layout.proof_context == [0; 32]
                    || session.proof_context != Some(layout.proof_context)
                    || greatest_ordinal.is_some_and(|ordinal| ordinal >= layout.ordinal())
                    || column.snapshot.layout() != layout
                {
                    return Err(StoredPhaseErrorV1::Admission);
                }
                greatest_ordinal = Some(layout.ordinal());
            }
        }
        if columns.next().is_some()
            || greatest_ordinal != session.greatest_ordinal
            || (plan.columns == 0 && session.proof_context.is_some())
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        // In-place sorting moves the original snapshots and SecretBlind guards. It never
        // copies a blind or allocates storage proportional to the witness scalar count.
        session
            .columns
            .sort_unstable_by_key(|column| column.layout.column());
        if session
            .columns
            .iter()
            .enumerate()
            .any(|(index, column)| column.layout.column() as usize != index)
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        Ok(CompleteStoredAdviceV1 {
            session: Some(session),
        })
    }

    /// Consume the only phase cursor to begin the next exact writer set. Calling after the
    /// final phase fails and consumes the owner; the adapter does not restart synthesis.
    pub(crate) fn begin_next<W>(
        self,
        writers: Vec<W>,
    ) -> Result<StoredPhaseAssignmentsV1<'params, C, W>, StoredPhaseErrorV1>
    where
        W: StoredAdviceWriterV1<Snapshot = S>,
    {
        StoredPhaseAssignmentsV1::begin_session(self.session, writers)
    }
}

impl<'params, C, S> CompleteStoredAdviceV1<'params, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1,
    S: StoredAdviceSnapshotV1,
{
    fn session(&self) -> Result<&Session<'params, C, S>, StoredPhaseErrorV1> {
        self.session.as_ref().ok_or(StoredPhaseErrorV1::Poisoned)
    }

    /// Return the exact immutable parameter reference retained at schedule admission.
    pub(crate) fn params(&self) -> Result<&'params ParamsIPA<C>, StoredPhaseErrorV1> {
        Ok(self.session()?.plan.params)
    }

    /// Return the admitted proof context, or `None` for the absorbed zero-advice circuit.
    pub(crate) fn proof_context(&self) -> Result<Option<[u8; 32]>, StoredPhaseErrorV1> {
        Ok(self.session()?.proof_context)
    }

    /// Iterate immutable layouts in global advice-column order, independent of phase order.
    pub(crate) fn layouts(
        &self,
    ) -> Result<impl ExactSizeIterator<Item = StoredAdviceLayoutV1> + '_, StoredPhaseErrorV1> {
        Ok(self.session()?.columns.iter().map(|column| column.layout))
    }

    /// Return public transcript challenges in global challenge-index order.
    ///
    /// Completion has already checked that every slot was squeezed. Returning copied public
    /// challenges does not expose references into advice witnesses or commitment blinds.
    pub(crate) fn challenges(
        &self,
    ) -> Result<impl Iterator<Item = C::Scalar> + '_, StoredPhaseErrorV1> {
        Ok(self.session()?.challenges.iter().copied().flatten())
    }

    /// Inspect one canonical authenticated chunk and its original commitment blind.
    ///
    /// The caller supplies the exact receipt layout, binding field, proof context, ordinal,
    /// basis, phase and global column. The callback cannot return a borrowed chunk or obtain
    /// a mutable backend handle. It may copy values: consumers remain responsible for wiping
    /// their own scalar/blind copies and bounded scratch. This owner wipes its original blind
    /// guard on drop. No whole-column read or witness-dependent allocation occurs here.
    ///
    /// Any invalid request, backend or callback error, or unwind destroys every receipt.
    /// Callers must propagate failure and discard partial downstream argument/transcript state;
    /// this method cannot roll back side effects performed by a callback.
    pub(crate) fn with_chunk<R>(
        &mut self,
        expected: StoredAdviceLayoutV1,
        chunk: u64,
        consume: impl FnOnce(&[[u8; 32]], Blind<C::Scalar>) -> Result<R, StoredAdviceErrorV1>,
    ) -> Result<R, StoredPhaseErrorV1> {
        let mut session = self.session.take().ok_or(StoredPhaseErrorV1::Poisoned)?;
        let column = session
            .columns
            .get_mut(expected.column() as usize)
            .ok_or(StoredPhaseErrorV1::Admission)?;
        if column.layout != expected || column.snapshot.layout() != expected {
            return Err(StoredAdviceErrorV1::Context.into());
        }
        let count = expected.chunk_scalar_count(chunk)?;
        let result = column.snapshot.with_chunk(expected, chunk, |encoded| {
            if encoded.len() != count
                || encoded
                    .iter()
                    .any(|value| !expected.field().is_canonical(value))
            {
                return Err(StoredAdviceErrorV1::Encoding);
            }
            consume(encoded, column.blind.0)
        })?;
        if column.snapshot.layout() != expected {
            return Err(StoredAdviceErrorV1::Context.into());
        }
        self.session = Some(session);
        Ok(result)
    }
}

/// Strict internal single-phase bridge; complete producer admission is a separate boundary.
pub(crate) mod synthesis;

#[cfg(test)]
mod tests;
