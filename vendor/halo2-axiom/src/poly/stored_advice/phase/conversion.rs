//! Consuming coefficient staging that retains original advice for real argument construction.
//!
//! This is a concrete phase-owned conversion, never a mutable-snapshot callback. Its caller
//! retains the original PK/protocol owner and supplies that key's domain. All source snapshots
//! and original blind guards survive successful staging; partial outputs and the entire source
//! session drop on any failure or unwind. No argument-complete or quotient-ready state follows.

use ff::WithSmallOrderMulGroup;

use super::{CompleteStoredAdviceV1, StoredPhaseErrorV1, reserved};
use crate::{
    arithmetic::CurveAffine,
    poly::{
        EvaluationDomain,
        commitment::Params,
        stored_advice::{
            STORED_MAX_K_V1, StoredPastaFieldV1, StoredPolynomialBasisV1, StoredPolynomialErrorV1,
            StoredPolynomialLayoutV1, StoredPolynomialProviderV1, StoredPolynomialRoleV1,
            StoredPolynomialSnapshotV1, StoredPolynomialWriterV1,
            assignment::StoredAssignmentFieldV1, transform::convert_stored_advice_with_writer_v1,
        },
    },
};

/// A coefficient copy has no new commitment blind: its original Lagrange owner retains it.
pub(super) struct CoefficientColumnV1<S> {
    pub(super) layout: StoredPolynomialLayoutV1,
    pub(super) snapshot: S,
}

/// Original Lagrange receipts plus staged coefficients for the same polynomial identities.
///
/// Stored lookup/copy-permutation stages finish before the concrete vanishing/y handoff. Later
/// coset/opening transitions must move the original blind guards, never duplicate or drop them
/// with the old snapshots. This owner supplies no detached construction or snapshot accessor.
pub(crate) struct CoefficientStoredAdviceV1<'params, C: CurveAffine, S>
where
    C::Scalar: StoredAssignmentFieldV1,
{
    pub(super) lagrange: CompleteStoredAdviceV1<'params, C, S>,
    pub(super) coefficients: Vec<CoefficientColumnV1<S>>,
    // The first actual lookup writer establishes this for a zero-advice proof. The original
    // Lagrange session remains context-free in that case; public geometry is not a receipt.
    pub(super) proof_context: Option<[u8; 32]>,
    pub(super) greatest_ordinal: Option<u64>,
}

#[path = "lookup.rs"]
mod lookup;

#[path = "retirement.rs"]
pub(super) mod retirement;

// Check global freshness before the raw converter can read source witness values. It already
// checks every exact source/destination coordinate; its relative source ordinal alone cannot
// detect reuse of another column's or previously staged output's ordinal.
struct MonotoneProvider<'provider, P> {
    provider: &'provider mut P,
    context: Option<[u8; 32]>,
    greatest: &'provider mut Option<u64>,
    last_destination: Option<StoredPolynomialLayoutV1>,
}
impl<P: StoredPolynomialProviderV1> StoredPolynomialProviderV1 for MonotoneProvider<'_, P> {
    type Writer = P::Writer;

    fn create(
        &mut self,
        field: StoredPastaFieldV1,
        basis: StoredPolynomialBasisV1,
        k: u32,
        role: StoredPolynomialRoleV1,
    ) -> Result<Self::Writer, StoredPolynomialErrorV1> {
        let writer = self.provider.create(field, basis, k, role)?;
        let layout = writer.layout();
        if self.context != Some(layout.proof_context)
            || layout.field() != field
            || layout.basis() != basis
            || layout.k() != k
            || layout.role() != role
            || self
                .greatest
                .is_some_and(|greatest| layout.ordinal() <= greatest)
        {
            return Err(StoredPolynomialErrorV1::Context);
        }
        // A created destination burns its ordinal even if a later read/write/seal fails.
        *self.greatest = Some(layout.ordinal());
        self.last_destination = Some(layout);
        Ok(writer)
    }
}

impl<'params, C, S> CompleteStoredAdviceV1<'params, C, S>
where
    C: CurveAffine,
    C::Scalar: StoredAssignmentFieldV1 + WithSmallOrderMulGroup<3>,
    S: StoredPolynomialSnapshotV1,
{
    /// Stage coefficient copies serially, consuming all original receipts on failure/unwind.
    ///
    /// The caller must pass its retained PK domain and same per-proof provider. This internal
    /// geometry check is not authentication of a same-k parameter artifact or circuit relation.
    /// Original Lagrange snapshots, challenges and sole blind guards remain intact on success.
    /// No proof RNG/transcript is accepted, and zero advice invents no context or output.
    ///
    /// Backend storage grows by one n-scalar coefficient snapshot per advice column; transient
    /// adapter fields remain one decoded column plus one encoding chunk in the raw converter.
    /// This is not a full-process memory/erasure bound or a completed argument/proof transition.
    pub(crate) fn stage_coefficients<P>(
        mut self,
        domain: &EvaluationDomain<C::Scalar>,
        provider: &mut P,
    ) -> Result<CoefficientStoredAdviceV1<'params, C, S>, StoredPhaseErrorV1>
    where
        P: StoredPolynomialProviderV1,
        P::Writer: StoredPolynomialWriterV1<Snapshot = S>,
    {
        let mut session = self.session.take().ok_or(StoredPhaseErrorV1::Poisoned)?;
        let plan = &session.plan;
        if plan.k > STORED_MAX_K_V1
            || domain.k() != plan.k
            || plan.params.k() != plan.k
            || plan.params.n() != 1_u64 << plan.k
            || plan.field != C::Scalar::STORED_FIELD
            || session.columns.len() != plan.columns
            || session.next_phase != plan.phases.len()
            || plan.phases.is_empty()
            || plan.phases.len() > 3
            || session.challenges.len() != plan.challenges
            || session.challenges.iter().any(Option::is_none)
            || plan.params.get_g_lagrange().len() != 1_usize << plan.k
            || (plan.columns == 0 && (plan.phases.len() != 1 || plan.challenges != 0))
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        // Recover the original phase-order ordinal chain from globally sorted receipts.
        // Strict increase also rejects duplicate scheduled columns without a witness bank.
        let mut greatest_source = None;
        let mut scheduled_columns = 0_usize;
        let mut scheduled_challenges = 0_usize;
        let mut seen_challenges = reserved(plan.challenges)?;
        seen_challenges.resize(plan.challenges, false);
        for (phase_index, phase) in plan.phases.iter().enumerate() {
            if (phase.columns.is_empty() && (plan.columns != 0 || !phase.challenges.is_empty()))
                || phase.columns.windows(2).any(|pair| pair[0] >= pair[1])
                || phase.challenges.windows(2).any(|pair| pair[0] >= pair[1])
            {
                return Err(StoredPhaseErrorV1::Admission);
            }
            scheduled_columns = scheduled_columns
                .checked_add(phase.columns.len())
                .ok_or(StoredPhaseErrorV1::Admission)?;
            scheduled_challenges = scheduled_challenges
                .checked_add(phase.challenges.len())
                .ok_or(StoredPhaseErrorV1::Admission)?;
            for index in &phase.challenges {
                let seen = seen_challenges
                    .get_mut(*index)
                    .ok_or(StoredPhaseErrorV1::Admission)?;
                if std::mem::replace(seen, true) {
                    return Err(StoredPhaseErrorV1::Admission);
                }
            }
            for index in &phase.columns {
                let column = session
                    .columns
                    .get(*index)
                    .ok_or(StoredPhaseErrorV1::Admission)?;
                if column
                    .layout
                    .advice_coordinates()
                    .map_err(|_| StoredPhaseErrorV1::Admission)?
                    .1 as usize
                    != phase_index
                    || greatest_source.is_some_and(|old| column.layout.ordinal() <= old)
                {
                    return Err(StoredPhaseErrorV1::Admission);
                }
                greatest_source = Some(column.layout.ordinal());
            }
        }
        if scheduled_columns != plan.columns || scheduled_challenges != plan.challenges {
            return Err(StoredPhaseErrorV1::Admission);
        }
        drop(seen_challenges);
        for (index, column) in session.columns.iter().enumerate() {
            let layout = column.layout;
            if layout
                .advice_coordinates()
                .map_err(|_| StoredPhaseErrorV1::Admission)?
                .0 as usize
                != index
                || layout.field() != plan.field
                || layout.k() != plan.k
                || layout.basis() != StoredPolynomialBasisV1::Lagrange
                || layout.proof_context == [0; 32]
                || session.proof_context != Some(layout.proof_context)
                || column.snapshot.layout() != layout
            {
                return Err(StoredPhaseErrorV1::Admission);
            }
        }
        if greatest_source != session.greatest_ordinal
            || (session.columns.is_empty() && session.proof_context.is_some())
        {
            return Err(StoredPhaseErrorV1::Admission);
        }
        let mut coefficients = reserved(session.columns.len())?;
        let mut greatest_ordinal = session.greatest_ordinal;
        let mut provider = MonotoneProvider {
            provider,
            context: session.proof_context,
            greatest: &mut greatest_ordinal,
            last_destination: None,
        };
        for column in &mut session.columns {
            let writer = provider.create(
                column.layout.field(),
                StoredPolynomialBasisV1::Coefficient,
                column.layout.k(),
                column.layout.role(),
            )?;
            let destination = provider
                .last_destination
                .ok_or(StoredPolynomialErrorV1::Context)?;
            let snapshot = convert_stored_advice_with_writer_v1::<C::Scalar, _, _>(
                domain,
                &mut column.snapshot,
                column.layout,
                StoredPolynomialBasisV1::Coefficient,
                writer,
                destination,
            )?;
            let layout = snapshot.layout();
            if Some(layout) != provider.last_destination {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
            // The reserved metadata vector cannot reallocate while retained source/output
            // snapshots coexist. No blind is copied or moved during coefficient staging.
            coefficients.push(CoefficientColumnV1 { layout, snapshot });
        }
        drop(provider);
        // A later operation on a shared backend must not substitute an earlier receipt.
        // Enclose this final all-receipt validation in the same consuming transaction.
        for column in &session.columns {
            if column.snapshot.layout() != column.layout {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
        }
        for column in &coefficients {
            if column.snapshot.layout() != column.layout {
                return Err(StoredPolynomialErrorV1::Context.into());
            }
        }
        Ok(CoefficientStoredAdviceV1 {
            proof_context: session.proof_context,
            lagrange: CompleteStoredAdviceV1 {
                session: Some(session),
            },
            coefficients,
            greatest_ordinal,
        })
    }
}
