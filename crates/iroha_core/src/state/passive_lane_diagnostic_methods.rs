<<<<<<< HEAD
macro_rules! state_passive_lane_diagnostic_methods {
    () => {
        fn durable_lane_diagnostic_execution_status(
            &self,
            session: &crate::lane_consensus::CommittedLaneBlockSession,
            current_state_height: u64,
            current_state_hash: HashOf<BlockHeader>,
        ) -> Option<crate::sumeragi::status::CommittedLaneBlockExecutionStatus> {
            use crate::sumeragi::status::CommittedLaneBlockExecutionStatus as ExecutionStatus;

            let proposal = &session.proposal;
            let application_receipt_available =
                if session.prepare_qc.payload_availability_qc.is_some() {
                    self.kura
                        .autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair(
                            proposal,
                        )
                } else {
                    self.kura
                        .lane_block_application_receipt_available_without_sidecar_repair(proposal)
                };
            if application_receipt_available {
                let descriptor = &proposal.descriptor;
                let receipt = self
                    .kura
                    .read_lane_block_application_receipt_without_sidecar_repair(
                        descriptor.lane_id,
                        descriptor.lane_block_height,
                    )?;
                return Some(
                    if receipt.format
                        == crate::kura::LaneBlockApplicationReceiptArtifactFormat::DirectExecution
                    {
                        ExecutionStatus::StateAppliedByDirectExecution
                    } else {
                        ExecutionStatus::StateAppliedByCanonicalBlock
                    },
                );
            }
            if self.certified_lane_block_session_is_applied_or_snapshot_anchored_cached(session) {
                // A replicated frontier or hash-only ordinary snapshot cannot prove
                // which receipt format produced the application. Fail closed until
                // exact durable evidence recovers through an explicit recovery gate.
                return None;
            }
            if self
                .kura
                .lane_block_application_receipt_conflicts_with_preflight_without_sidecar_repair(
                    proposal,
                )
            {
                return Some(ExecutionStatus::ApplicationReceiptConflictsWithPreflight);
            }
            if !self.certified_lane_block_session_predecessor_is_applied_cached(session) {
                return Some(ExecutionStatus::AwaitingPredecessorApplication);
            }
            if self
                .kura
                .read_preflighted_lane_block_execution_input_for_application_without_sidecar_repair(
                    proposal,
                    current_state_height,
                    Some(current_state_hash),
                )
                .is_some()
            {
                return Some(ExecutionStatus::PayloadPreflightedAwaitingStateApplication);
            }
            if self
                .kura
                .lane_block_execution_preflight_has_rejections_without_sidecar_repair(
                    proposal,
                    current_state_height,
                    Some(current_state_hash),
                )
                == Some(true)
            {
                return Some(ExecutionStatus::PayloadPreflightRejectedAwaitingStateApplication);
            }
            if self
                .kura
                .lane_block_execution_input_available_without_sidecar_repair(proposal)
            {
                return Some(ExecutionStatus::PayloadRecoveredAwaitingStateApplication);
            }
            if self.kura.lane_block_payload_is_recoverable(proposal) {
                return Some(ExecutionStatus::PayloadAvailableAwaitingExecutor);
            }
            Some(ExecutionStatus::AwaitingExecutablePayload)
        }
    };
=======
impl State {
    fn durable_lane_diagnostic_execution_status(
        &self,
        session: &crate::lane_consensus::CommittedLaneBlockSession,
        current_state_height: u64,
        current_state_hash: HashOf<BlockHeader>,
    ) -> Option<crate::sumeragi::status::CommittedLaneBlockExecutionStatus> {
        use crate::sumeragi::status::CommittedLaneBlockExecutionStatus as ExecutionStatus;

        let proposal = &session.proposal;
        let application_receipt_available = if session.prepare_qc.payload_availability_qc.is_some()
        {
            self.kura
                .autonomous_lane_block_merge_receipt_revalidates_without_sidecar_repair(proposal)
        } else {
            self.kura
                .lane_block_application_receipt_available_without_sidecar_repair(proposal)
        };
        if application_receipt_available {
            let descriptor = &proposal.descriptor;
            let receipt = self
                .kura
                .read_lane_block_application_receipt_without_sidecar_repair(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                )?;
            return Some(
                if receipt.format
                    == crate::kura::LaneBlockApplicationReceiptArtifactFormat::DirectExecution
                {
                    ExecutionStatus::StateAppliedByDirectExecution
                } else {
                    ExecutionStatus::StateAppliedByCanonicalBlock
                },
            );
        }
        if self.certified_lane_block_session_is_applied_or_snapshot_anchored_cached(session) {
            // A replicated frontier or hash-only ordinary snapshot cannot prove
            // which receipt format produced the application. Fail closed until
            // exact durable evidence recovers through an explicit recovery gate.
            return None;
        }
        if self
            .kura
            .lane_block_application_receipt_conflicts_with_preflight_without_sidecar_repair(
                proposal,
            )
        {
            return Some(ExecutionStatus::ApplicationReceiptConflictsWithPreflight);
        }
        if !self.certified_lane_block_session_predecessor_is_applied_cached(session) {
            return Some(ExecutionStatus::AwaitingPredecessorApplication);
        }
        if self
            .kura
            .read_preflighted_lane_block_execution_input_for_application_without_sidecar_repair(
                proposal,
                current_state_height,
                Some(current_state_hash),
            )
            .is_some()
        {
            return Some(ExecutionStatus::PayloadPreflightedAwaitingStateApplication);
        }
        if self
            .kura
            .lane_block_execution_preflight_has_rejections_without_sidecar_repair(
                proposal,
                current_state_height,
                Some(current_state_hash),
            )
            == Some(true)
        {
            return Some(ExecutionStatus::PayloadPreflightRejectedAwaitingStateApplication);
        }
        if self
            .kura
            .lane_block_execution_input_available_without_sidecar_repair(proposal)
        {
            return Some(ExecutionStatus::PayloadRecoveredAwaitingStateApplication);
        }
        if self.kura.lane_block_payload_is_recoverable(proposal) {
            return Some(ExecutionStatus::PayloadAvailableAwaitingExecutor);
        }
        Some(ExecutionStatus::AwaitingExecutablePayload)
    }
>>>>>>> origin/optimizations
}
