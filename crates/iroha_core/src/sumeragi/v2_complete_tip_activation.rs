//! Adapter-owned authority for a decided, unapplied CompleteTip successor.

use super::*;

/// Move-only proof that a successor's pending Decision came from its durable WAL.
///
/// Canonical Kura remains at the parent height. This authority permits truthful
/// Decision diagnostics at H+1 without claiming that H+1 has been applied.
#[must_use]
pub(crate) struct RecoveredSuccessorDecisionActivationAuthorityV1 {
    wal_identity: RecoveredWalFrameIdentity,
    parent_context: wire::HeightContext,
    parent_commit_qc: wire::QuorumCertificate,
    context_id: wire::HeightContextId,
    height: wire::Height,
    decision: wire::QuorumCertificate,
    decision_status: wire::SumeragiV2CommitQcStatus,
}

impl RecoveredSuccessorDecisionActivationAuthorityV1 {
    /// Bind the sealed WAL Decision to the complete canonical parent and status.
    pub(in crate::sumeragi) fn authorizes(
        &self,
        parent: &wire::HeightContext,
        parent_commit_qc: &wire::QuorumCertificate,
        successor_context_id: wire::HeightContextId,
        status: &wire::SumeragiV2Status,
    ) -> bool {
        self.wal_identity.is_exact()
            && self.parent_context == *parent
            && self.parent_commit_qc == *parent_commit_qc
            && parent.height.checked_add(1) == Some(self.height)
            && successor_context_id == self.context_id
            && status.height_context_id == self.context_id
            && status.height == self.height
            && status.last_committed_height == self.height
            && status.last_committed_subject == Some(self.decision.subject)
            && status.last_commit_qc.as_ref() == Some(&self.decision_status)
            && status.phase == wire::SumeragiV2StatusPhase::PendingApply
            && status.body_state == wire::SumeragiV2BodyState::PendingApply
            && status.pending_persistence_id.is_none()
            && !status.restart_required
    }
}

impl SumeragiV2Adapter {
    /// Retain the exact durable successor Decision before live activation.
    ///
    /// This cannot be minted from a caller-provided status or QC. The reducer's
    /// committed WAL state and registry provide the certificate; the verified
    /// predecessor provides the parent. Already-applied and in-flight WAL
    /// states cannot borrow this pending-Decision exception.
    pub(in crate::sumeragi) fn recovered_successor_decision_activation_authority(
        &mut self,
    ) -> Result<Option<RecoveredSuccessorDecisionActivationAuthorityV1>, AdapterError> {
        self.ensure_ingress()?;
        let Some(decision) = self.reducer.durable_state().decision() else {
            return Ok(None);
        };
        if self.reducer.durable_state().last_id().get() == 0
            || self.pending_persistence_id.is_some()
            || self.reducer.applied_subject().is_some()
        {
            return Err(AdapterError::RecoveredSuccessorDecisionActivationMismatch);
        }
        self.authenticate_recovered_wal_frontier()?;
        let parent = self
            .parent_verification
            .as_ref()
            .ok_or(AdapterError::RecoveredSuccessorDecisionActivationMismatch)?;
        let parent_commit_qc = self
            .wire_context
            .parent_commit_qc
            .as_ref()
            .ok_or(AdapterError::RecoveredSuccessorDecisionActivationMismatch)?;
        let decision = self
            .registry
            .qc_to_wire(decision, self.aggregator.as_ref())?;
        let decision_status = commit_qc_status(&decision, &self.wire_context)?;
        let mut wal_identity = None;
        for frame in self.wal.recovered_records().iter().rev() {
            let (identity, envelope) = self.authenticate_recovered_wal_frame(frame)?;
            if matches!(envelope.record, WalRecordV2::Decision(candidate) if candidate == decision)
            {
                wal_identity = Some(identity);
                break;
            }
        }
        let wal_identity =
            wal_identity.ok_or(AdapterError::RecoveredSuccessorDecisionActivationMismatch)?;
        if parent.context.height.checked_add(1) != Some(self.wire_context.height)
            || parent_commit_qc.round.context_id != parent.context.id()
            || parent_commit_qc.round.height != parent.context.height
        {
            return Err(AdapterError::RecoveredSuccessorDecisionActivationMismatch);
        }
        Ok(Some(RecoveredSuccessorDecisionActivationAuthorityV1 {
            wal_identity,
            parent_context: parent.context.clone(),
            parent_commit_qc: parent_commit_qc.clone(),
            context_id: self.wire_context.id(),
            height: self.wire_context.height,
            decision,
            decision_status,
        }))
    }
}
