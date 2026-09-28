//! Join original Native execution to this service's finite journal-shell admission.
//!
//! The synchronous result still owns State writers. Complete resource admission
//! and journal detachment are required before retaining it across worker waits;
//! this adapter grants neither a validation marker nor publication authority.

use super::{V2ApplyError, V2ApplyService, VerifiedHeightContext};
use crate::{
    block::{BlockValidationError, valid::NativeCandidatePreparationError},
    state::{MergeLedgerCommitError, PreparedCarrier, PreparedNativeLaneBatchSourceV1},
    sumeragi::v2_body_store::LocalValidationRefusal,
};
use iroha_data_model::block::SignedBlock;
use iroha_primitives::time::TimeSource;

/// One executed candidate and its original finite journal-shell admission.
#[must_use = "detach the original journals synchronously or abandon the complete candidate"]
pub(crate) struct PreparedNativeServiceCandidate<'state> {
    carrier: PreparedCarrier<'state>,
    shell_admission: super::native_validation::CarrierShellAdmission,
}

impl<'state> PreparedNativeServiceCandidate<'state> {
    /// Detach readiness from synchronous execution: publication checks Queue ownership,
    /// while the retained validator checks AMX evidence after releasing all State writers.
    pub(crate) fn into_parts(
        self,
    ) -> (
        PreparedCarrier<'state>,
        super::native_validation::CarrierShellAdmission,
    ) {
        (self.carrier, self.shell_admission)
    }
}

impl V2ApplyService {

    pub(super) fn prepare_native_source_admitted<'state>(
        &'state self,
        body: &SignedBlock,
        source: PreparedNativeLaneBatchSourceV1<'state>,
        context: VerifiedHeightContext,
        shell_admission: &mut Option<super::native_validation::CarrierShellAdmission>,
    ) -> Result<Option<PreparedNativeServiceCandidate<'state>>, V2ApplyError> {
        crate::exec_witness::ensure_state_access_without_exec_witness().map_err(|reason| {
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
        self.validate_candidate_state_binding(context.context(), body)?;
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
            shell_admission: shell_admission
                .take()
                .expect("Native shell admission remains owned until execution succeeds"),
        }))
    }

    /// Execute genesis, direct ordinary inputs, or current control work through the common
    /// authenticated validator, keeping the same pre-execution shell admission.
    /// The source-class boundary excludes retired payloads and Native mixtures
    /// before this producer is selected. Common validation still checks
    /// signatures, commitments, exact State/context, useful work and every control.
    pub(super) fn prepare_current_control_source_admitted<'state>(
        &'state self,
        body: &SignedBlock,
        context: &VerifiedHeightContext,
        shell_admission: super::native_validation::CarrierShellAdmission,
    ) -> Result<PreparedNativeServiceCandidate<'state>, V2ApplyError> {
        crate::exec_witness::ensure_state_access_without_exec_witness().map_err(|reason| {
            LocalValidationRefusal::RecoveryRequired(format!(
                "control service execution recorder ownership conflict: {reason}"
            ))
        })?;
        self.validate_candidate_state_binding(context.context(), body)?;
        let topology = crate::sumeragi::network_topology::Topology::new(
            context
                .context()
                .roster
                .iter()
                .map(|entry| entry.validator.clone()),
        );
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
            )
            .map_err(|(failed, error)| {
                self.classify_validation_failure(None, failed.as_ref(), error.as_ref())
            })?;
        Ok(PreparedNativeServiceCandidate {
            carrier,
            shell_admission,
        })
    }

    /// Preserve the exact native owner and proposal checks after retiring the old archive hook.
    fn validate_candidate_state_binding(
        &self,
        context: &iroha_data_model::block::consensus_v2::HeightContext,
        body: &SignedBlock,
    ) -> Result<(), V2ApplyError> {
        if !self.state.matches_kura_instance(&self.kura) {
            return Err(LocalValidationRefusal::RecoveryRequired(
                "candidate execution requires the original State/Kura pair".into(),
            )
            .into());
        }
        if !body.is_resultless_proposal() {
            return Err(V2ApplyError::ResultBearingProposal);
        }
        let parent = context
            .parent_commit_qc
            .as_ref()
            .map(|certificate| certificate.subject.block_hash)
            .or_else(|| {
                context
                    .snapshot_bootstrap
                    .map(|anchor| anchor.snapshot_block_hash)
            });
        if context.network_id != self.network_id
            || body.header().height().get() != context.height
            || body.header().prev_block_hash() != parent
        {
            return Err(V2ApplyError::TaskMismatch);
        }
        Ok(())
    }

    /// Count actual execution attempts, excluding source and admission refusals.
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
                MergeLedgerCommitError::StateStorageAdmission(error),
            )
            | NativeCandidatePreparationError::Preparation(
                MergeLedgerCommitError::StateStorageAdmission(error),
            ) => self.classify_validation_failure(
                None,
                body,
                &BlockValidationError::StateStorageAdmission(error),
            ),
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
                MergeLedgerCommitError::MembershipAdmission(error),
            )
            | NativeCandidatePreparationError::Preparation(
                MergeLedgerCommitError::MembershipAdmission(error),
            ) => self.classify_validation_failure(
                None,
                body,
                &BlockValidationError::MembershipAdmission(error),
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
