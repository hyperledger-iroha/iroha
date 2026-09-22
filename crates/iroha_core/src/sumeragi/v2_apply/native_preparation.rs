//! Join original Native execution to this service's archive predecessor owners.
//!
//! The synchronous result still owns State writers. Complete resource admission
//! and journal detachment are required before retaining it across worker waits;
//! this adapter grants neither a validation marker nor publication authority.

use super::{V2ApplyError, V2ApplyService, VerifiedHeightContext};
use crate::{
    block::{BlockValidationError, valid::NativeCandidatePreparationError},
    query::{
        provider_ingest_finalized::ProviderCandidateCapture,
        reputation_finalized::ReputationCandidateCapture,
    },
    state::{MergeLedgerCommitError, PreparedCarrier, PreparedNativeLaneBatchSourceV1},
    sumeragi::v2_body_store::LocalValidationRefusal,
};
use iroha_data_model::block::SignedBlock;
use iroha_primitives::time::TimeSource;

/// One executed candidate and the archive predecessors reserved before execution.
/// Field order retires all State writers before either logical archive owner.
#[must_use = "detach the original journals synchronously or abandon the complete candidate"]
pub(crate) struct PreparedNativeServiceCandidate<'state> {
    carrier: PreparedCarrier<'state>,
    provider: Option<ProviderCandidateCapture>,
    reputation: Option<ReputationCandidateCapture>,
    service: &'state V2ApplyService,
}

impl<'state> PreparedNativeServiceCandidate<'state> {
    /// Move the actual execution and its original captures into journal preparation.
    /// The caller must provide the real journal admission policy; this handoff
    /// does not replace it with a scalar receipt or an empty reservation.
    /// A local post-execution refusal returns this same synchronous owner. Its
    /// State writers cannot cross an async wait; the retained validator must
    /// detach this exact execution before scheduling a retry.
    #[allow(clippy::type_complexity)]
    pub(crate) fn try_into_parts(
        self,
    ) -> Result<
        (
            PreparedCarrier<'state>,
            Option<ProviderCandidateCapture>,
            Option<ReputationCandidateCapture>,
        ),
        (Self, V2ApplyError),
    > {
        let readiness = self
            .service
            .try_validate_prospective_autoscale_retirement_queue(
                self.carrier.block(),
                self.carrier.state(),
            )
            .and_then(|()| {
                self.service
                    .kura
                    .validate_native_amx_participant_application_evidence_byte_budget(
                        self.carrier.native_amx_manifest(),
                        None,
                    )
                    .map_err(V2ApplyService::classify_native_amx_evidence_byte_budget_error)
            });
        if let Err(error) = readiness {
            return Err((self, error));
        }
        Ok((self.carrier, self.provider, self.reputation))
    }
}

impl V2ApplyService {
    /// Execute authenticated Native sources once after reserving both archives.
    /// A stale source returns no owner and never becomes an invalid-body marker.
    /// The exact State and proposal are checked before any archive reservation;
    /// callers cannot pair a foreign source with an otherwise identical service.
    pub(crate) fn prepare_native_source<'state>(
        &'state self,
        body: &SignedBlock,
        source: PreparedNativeLaneBatchSourceV1<'state>,
        context: VerifiedHeightContext,
    ) -> Result<Option<PreparedNativeServiceCandidate<'state>>, V2ApplyError> {
        crate::sumeragi::witness::ensure_state_access_without_exec_witness().map_err(|reason| {
            LocalValidationRefusal::RecoveryRequired(format!(
                "Native service execution recorder ownership conflict: {reason}"
            ))
        })?;
        let Some((state, original, _)) = source.preparation_input() else {
            return Ok(None);
        };
        if !std::ptr::eq(state, self.state.as_ref()) || original != body {
            return Err(V2ApplyError::TaskMismatch);
        }
        let archives = self.try_reserve_candidate_archives(context.context(), body)?;
        let (provider, reputation) = archives
            .into_captures(self, context.context(), body)
            .map_err(|(_, error)| error)?;
        #[cfg(test)]
        self.test_failures
            .candidate_executions
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let carrier = match source.prepare_candidate(
            context,
            &self.genesis_account,
            &TimeSource::new_system(),
            self.block_cadence,
        ) {
            Ok(Some(carrier)) => carrier,
            Ok(None)
            | Err(NativeCandidatePreparationError::Execution(
                MergeLedgerCommitError::ExecutionObservationChanged,
            )) => return Ok(None),
            Err(error) => return Err(self.classify_native_preparation_failure(body, error)),
        };
        Ok(Some(PreparedNativeServiceCandidate {
            carrier,
            provider,
            reputation,
            service: self,
        }))
    }

    /// Count actual execution attempts, excluding source and archive refusals.
    #[cfg(test)]
    pub(crate) fn candidate_executions_for_test(&self) -> usize {
        self.test_failures
            .candidate_executions
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Preserve original local dependencies and semantic validation provenance.
    pub(super) fn classify_native_preparation_failure(
        &self,
        body: &SignedBlock,
        error: NativeCandidatePreparationError,
    ) -> V2ApplyError {
        match error {
            NativeCandidatePreparationError::Preflight(error)
            | NativeCandidatePreparationError::Execution(
                MergeLedgerCommitError::NativeControlValidation(error),
            ) => self.classify_validation_failure(None, body, &error),
            NativeCandidatePreparationError::Execution(
                MergeLedgerCommitError::BlockHashAdmission(error),
            ) => self.classify_validation_failure(
                None,
                body,
                &BlockValidationError::BlockHashAdmission(error),
            ),
            NativeCandidatePreparationError::Execution(
                error @ MergeLedgerCommitError::ExecutionBatchFull { .. },
            ) => V2ApplyError::Validation(error.to_string()),
            // Only the global preflight and explicit governed batch limit above
            // establish a semantic body verdict. Remaining source-execution and
            // metadata errors include local State/recorder recovery failures.
            // Diagnostic strings must never authorize a negative marker.
            error => LocalValidationRefusal::RecoveryRequired(error.to_string()).into(),
        }
    }
}
