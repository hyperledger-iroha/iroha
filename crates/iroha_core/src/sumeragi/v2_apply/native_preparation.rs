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
    shell_admission: super::native_validation::CarrierShellAdmission,
}

impl<'state> PreparedNativeServiceCandidate<'state> {
    /// Detach readiness from synchronous execution: publication checks Queue ownership,
    /// while the retained validator checks AMX evidence after releasing all State writers.
    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedCarrier<'state>,
        Option<ProviderCandidateCapture>,
        Option<ReputationCandidateCapture>,
        super::native_validation::CarrierShellAdmission,
    ) {
        (
            self.carrier,
            self.provider,
            self.reputation,
            self.shell_admission,
        )
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
        let shell_admission = self.reserve_carrier_shells()?;
        self.prepare_native_source_admitted(body, source, context, shell_admission)
    }

    pub(super) fn prepare_native_source_admitted<'state>(
        &'state self,
        body: &SignedBlock,
        source: PreparedNativeLaneBatchSourceV1<'state>,
        context: VerifiedHeightContext,
        shell_admission: super::native_validation::CarrierShellAdmission,
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
            shell_admission,
        }))
    }

    /// Execute the disjoint genesis or current control source through the common
    /// authenticated validator, keeping the same pre-execution shell/archive owners.
    /// The source-class boundary excludes retired payloads and post-genesis direct
    /// economics before this producer is selected. Common validation still checks
    /// signatures, commitments, exact State/context, useful work and every control.
    pub(super) fn prepare_current_control_source_admitted<'state>(
        &'state self,
        body: &SignedBlock,
        context: &VerifiedHeightContext,
        shell_admission: super::native_validation::CarrierShellAdmission,
    ) -> Result<PreparedNativeServiceCandidate<'state>, V2ApplyError> {
        crate::sumeragi::witness::ensure_state_access_without_exec_witness().map_err(|reason| {
            LocalValidationRefusal::RecoveryRequired(format!(
                "control service execution recorder ownership conflict: {reason}"
            ))
        })?;
        let archives = self.try_reserve_candidate_archives(context.context(), body)?;
        let (provider, reputation) = archives
            .into_captures(self, context.context(), body)
            .map_err(|(_, error)| error)?;
        let topology = crate::sumeragi::network_topology::Topology::new(
            context
                .context()
                .roster
                .iter()
                .map(|entry| entry.validator.clone()),
        );
        let mut voting_block = None;
        #[cfg(test)]
        self.test_failures
            .candidate_executions
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let carrier =
            crate::block::ValidBlock::validate_and_prepare_sumeragi_v2_candidate_keep_voting_block(
                body.clone(),
                &topology,
                &self.genesis_account,
                &TimeSource::new_system(),
                self.block_cadence,
                crate::block::valid::SumeragiV2ValidationContext::from_height_context(
                    context.context(),
                ),
                self.state.as_ref(),
                &mut voting_block,
            )
            .map_err(|(failed, error)| {
                self.classify_validation_failure(None, failed.as_ref(), error.as_ref())
            })?;
        Ok(PreparedNativeServiceCandidate {
            carrier,
            provider,
            reputation,
            shell_admission,
        })
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
            )
            | NativeCandidatePreparationError::Preparation(
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
