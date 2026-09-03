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

    /// Enqueue a local proposal successor under the immutable lifecycle Validate owner.
    ///
    /// The coordinator ordinal is already minted by the shared actor-global
    /// authority; this boundary validates and reuses it rather than allocating
    /// a second logical lifecycle position.
    pub(in crate::sumeragi) fn enqueue_local_proposal_with_lifecycle_pending(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
        pending: &PendingRuntimeEffectBinding,
        lifecycle_ordinal: u128,
        physical_completion: Option<
            super::v2_worker::LifecycleValidatePhysicalCompletionV1,
        >,
    ) -> Result<LocalProposalReadyCommandAdmission, EnqueueError> {
        if self.fail_closed || lifecycle_ordinal == 0 {
            return Err(EnqueueError::FailClosed);
        }
        if physical_completion.is_some_and(|completion| {
            completion.dispatch_key().lifecycle_ordinal() != lifecycle_ordinal
        }) {
            self.latch_fail_closed(
                "lifecycle local-proposal completion changed its guarded Validate owner",
            );
            return Err(EnqueueError::FailClosed);
        }
        let identity = LocalProposalReadyCommandIdentity::from_exact_pending_handoff(
            tag,
            &manifest,
            &durable_receipt,
            &validated_receipt,
            pending,
        )
        .ok_or(EnqueueError::FailClosed)?;
        let command = AdapterCommand::LocalProposalReady {
            manifest,
            durable_receipt,
            validated_receipt,
        };
        let admitted_at = Instant::now();
        let worker_completed_before_deadline = self.clocks_armed
            && tag == self.round_tag
            && physical_completion.is_some_and(|completion| {
                let retained_at = completion.retained_at();
                retained_at <= admitted_at
                    && retained_at
                    .checked_duration_since(self.round_started_at)
                    .is_some_and(|elapsed| {
                        elapsed
                            < round_timeout_for_view(
                                self.base_round_timeout,
                                self.round_tag.view(),
                            )
                    })
            });
        let preflight =
            self.command_admission_preflight(tag, CommandClass::Completion, &command)?;
        iroha_logger::warn!(
            ?preflight,
            ?tag,
            current_tag = ?self.driver.current_tag(),
            clocks_armed = self.clocks_armed,
            elapsed_ms = admitted_at
                .checked_duration_since(self.round_started_at)
                .map(|elapsed| elapsed.as_millis()),
            timeout_ms = round_timeout_for_view(self.base_round_timeout, self.round_tag.view())
                .as_millis(),
            worker_completed_before_deadline,
            "TEMP local ProposalReady runtime admission trace"
        );
        let mut tagged = match preflight {
            RuntimeCommandAdmissionPreflight::Coalesce
            | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => {
                // Neither form installs or replaces a runtime owner. The
                // executor consumes this typed outcome without retaining the
                // new linear replay value, so a semantically redundant
                // lifecycle may safely stutter beside an older producer.
                return Ok(LocalProposalReadyCommandAdmission::Coalesced(identity));
            }
            RuntimeCommandAdmissionPreflight::Admit => {
                let owner = self.restored_command_owner(
                    tag,
                    CommandClass::Completion,
                    &command,
                    None,
                    *pending.causal_lifecycle_key(),
                    lifecycle_ordinal,
                )?;
                TaggedCommand::with_causal_origin(
                    tag,
                    CommandClass::Completion,
                    command,
                    admitted_at,
                    owner.causal_origin().clone(),
                    owner.lifecycle_ordinal(),
                )?
            }
            RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key,
                admission_ordinal,
                producer_stage,
            } => {
                if causal_lifecycle_key != *pending.causal_lifecycle_key()
                    || admission_ordinal != lifecycle_ordinal
                    || producer_stage != 0
                {
                    self.latch_fail_closed(
                        "lifecycle local-proposal completion changed its dormant owner",
                    );
                    return Err(EnqueueError::FailClosed);
                }
                self.restored_tagged_command(
                    tag,
                    CommandClass::Completion,
                    command,
                    admitted_at,
                    causal_lifecycle_key,
                    admission_ordinal,
                    producer_stage,
                )?
            }
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        tagged.local_proposal_worker_completed_before_deadline =
            worker_completed_before_deadline;
        tagged.candidate_semantic_statement = pending.candidate_statement();
        if !tagged.validate_admission_identity() {
            self.latch_fail_closed(
                "lifecycle local-proposal completion carried an invalid candidate statement",
            );
            return Err(EnqueueError::FailClosed);
        }
        let result = self.enqueue_after_clock_reservation(tagged);
        if result == Err(EnqueueError::FailClosed) {
            self.latch_fail_closed(
                "lifecycle local-proposal completion failed exact queue ownership",
            );
        }
        result.map(|()| LocalProposalReadyCommandAdmission::Admitted(identity))
    }
}

/// Result of publishing one lifecycle-owned `LocalProposalReady` command.
///
/// A coalesced completion consumed no FIFO slot. Its caller must therefore
/// discard the new linear replay value and leave any older replay incumbent
/// unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::sumeragi) enum LocalProposalReadyCommandAdmission {
    /// The command acquired one new serialized FIFO owner.
    Admitted(LocalProposalReadyCommandIdentity),
    /// Existing reducer or producer state made the command a semantic stutter.
    Coalesced(LocalProposalReadyCommandIdentity),
}

impl LocalProposalReadyCommandAdmission {
    /// Borrow the inert command identity shared by both outcomes.
    pub(in crate::sumeragi) const fn command_identity(self) -> LocalProposalReadyCommandIdentity {
        match self {
            Self::Admitted(identity) | Self::Coalesced(identity) => identity,
        }
    }

    /// Return whether publication consumed no new runtime owner.
    pub(in crate::sumeragi) const fn was_coalesced(self) -> bool {
        matches!(self, Self::Coalesced(_))
    }
}
