impl SerializedV2Runtime<SumeragiV2Adapter> {
    /// Consume the adapter's exact live ProposalIntent Sign sidecar.
    ///
    /// Effect ownership must be transferred first, so the WAL-owned handoff
    /// cannot escape without the positional runtime batch already retained by
    /// its caller. A driver mismatch closes the serialized shell as well as
    /// the adapter.
    pub(in crate::sumeragi) fn take_live_proposal_intent_wal_sign(
        &mut self,
        effects: &[AdapterEffect],
    ) -> Result<Option<LiveProposalIntentWalSignHandoffV1>, AdapterError> {
        if self.fail_closed || self.pending_effect_ownership.is_some() {
            self.latch_fail_closed(
                "live ProposalIntent WAL Sign handoff crossed its positional ownership gate",
            );
            return Err(AdapterError::LiveWalReplayCauseMismatch);
        }
        match self.driver.take_live_proposal_intent_wal_sign(effects) {
            Ok(handoff) => Ok(handoff),
            Err(error) => {
                self.latch_fail_closed(format!(
                    "live ProposalIntent WAL Sign handoff did not match its adapter batch: {error}"
                ));
                Err(error)
            }
        }
    }

    /// Seal the adapter's exact reducer-fence source and generation.
    pub(in crate::sumeragi) fn lifecycle_reducer_fence_observation(
        &self,
    ) -> super::v2::LifecycleReducerFenceObservationV1 {
        self.driver.lifecycle_reducer_fence_observation()
    }

    fn ready_validate_runtime_gate_is_open(&self, has_local_publication: bool) -> bool {
        // Runtime ingress is a stable FIFO, not an in-flight mutable owner.
        // Completion ranks before Runtime, so appending the exact lifecycle
        // successor cannot reorder or overwrite an incumbent queued command.
        // Only active ownership transfers and leader-wire terminal reservations
        // make the adapter unsafe to preview at this boundary.
        let open = self.pending_effect_ownership.is_none()
            && self.last_scheduler_ownership.is_none()
            && self.pending_leader_wire_terminals.is_empty();
        if !open {
            iroha_logger::error!(
                ingress_len = self.ingress.len(),
                has_local_publication,
                pending_effect_ownership = self.pending_effect_ownership.is_some(),
                last_scheduler_ownership = self.last_scheduler_ownership.is_some(),
                pending_leader_wire_terminals = self.pending_leader_wire_terminals.len(),
                "Ready Validate runtime gate rejected a non-quiescent serialized adapter"
            );
        }
        open
    }

    /// Classify a Ready Validate publication without retaining adapter state.
    ///
    /// Existing queued ingress remains in FIFO order. The Ready successor is
    /// admitted only while no runtime or leader-wire mutation owner is active.
    pub(in crate::sumeragi) fn preflight_ready_durable_validate_adapter_publication(
        &mut self,
        execution: &PreparedReadyDurableValidateExecution<'_>,
        local_publication: Option<(LocalProposalReadyCommandIdentity, u128)>,
    ) -> Result<ReadyDurableValidateAdapterPublicationKind, AdapterError> {
        if self.fail_closed
            || !self.ready_validate_runtime_gate_is_open(local_publication.is_some())
        {
            return Err(AdapterError::ReadyDurableValidatePublicationContractViolation);
        }
        execution.preflight_adapter_publication_kind(&mut self.driver)
    }
}
