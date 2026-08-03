impl SerializedV2Runtime<SumeragiV2Adapter> {
    /// Build the reducer-owned status which the runner will publish at the
    /// one-shot live-height activation boundary.
    ///
    /// The snapshot is unavailable until [`Self::arm_live_clocks`] succeeds,
    /// so caller ordering alone cannot publish an unarmed successor.
    pub(crate) fn successor_activation_status_snapshot(
        &mut self,
    ) -> Result<wire::SumeragiV2Status, AdapterError> {
        if !self.clocks_armed {
            return Err(AdapterError::SuccessorClocksNotArmed);
        }
        self.driver.successor_activation_status()
    }

    fn body_pipeline_completion_is_owned(
        &mut self,
        tag: EventTag,
        candidate: &BodyPipelineCompletionEvidence,
    ) -> Result<bool, EnqueueError> {
        if self.fail_closed {
            return Err(EnqueueError::FailClosed);
        }
        let (ingress_owners, ingress_exact) = self
            .ingress
            .body_pipeline_completion_ownership(tag, candidate);
        let (deferred_owners, deferred_exact) = self
            .driver
            .deferred_body_pipeline_completion_ownership(tag, candidate);
        match classify_exact_body_completion_ownership(
            ingress_owners,
            ingress_exact,
            deferred_owners,
            deferred_exact,
        ) {
            ExactBodyCompletionOwnership::Vacant => Ok(false),
            ExactBodyCompletionOwnership::Exact => Ok(true),
            ExactBodyCompletionOwnership::Invalid => {
                self.latch_fail_closed(
                    "body completion had conflicting evidence or duplicate serialized owners",
                );
                Err(EnqueueError::DuplicateCompletionOwnership)
            }
        }
    }

    fn body_pipeline_completion_is_owned_by(
        &mut self,
        tag: EventTag,
        candidate: &BodyPipelineCompletionEvidence,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<bool, EnqueueError> {
        if !self.body_pipeline_completion_is_owned(tag, candidate)? {
            return Ok(false);
        }
        let mut retained = self
            .ingress
            .exact_body_pipeline_completion_owners(tag, candidate)?;
        for ordinal in self
            .driver
            .deferred_body_pipeline_completion_exact_owner_ordinals(tag, candidate)
        {
            let Some(deferred) = self.deferred_lifecycle_ownership.get(&ordinal) else {
                self.latch_fail_closed(
                    "exact deferred body completion lost its runtime lifecycle owner",
                );
                return Err(EnqueueError::FailClosed);
            };
            retained.push(deferred.owner.clone());
        }
        if retained.len() != 1 || retained.first() != Some(ownership.owner()) {
            self.latch_fail_closed("coalesced body completion changed its exact lifecycle owner");
            return Err(EnqueueError::FailClosed);
        }
        Ok(true)
    }

    fn enqueue_body_pipeline_completion(
        &mut self,
        tag: EventTag,
        evidence: BodyPipelineCompletionEvidence,
        command: AdapterCommand,
    ) -> Result<(), EnqueueError> {
        if self.body_pipeline_completion_is_owned(tag, &evidence)? {
            return Ok(());
        }
        self.enqueue(tag, CommandClass::Completion, command)
    }

    fn enqueue_body_pipeline_completion_with_owner(
        &mut self,
        tag: EventTag,
        evidence: BodyPipelineCompletionEvidence,
        command: AdapterCommand,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        if self.body_pipeline_completion_is_owned_by(tag, &evidence, ownership)? {
            return Ok(());
        }
        self.enqueue_with_lifecycle_owner(tag, CommandClass::Completion, command, ownership)
    }

    fn body_available_is_uniquely_owned(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        match self.body_pipeline_completion_is_owned(tag, &evidence) {
            Ok(owned) => Ok(owned),
            Err(EnqueueError::DuplicateCompletionOwnership) => Err(
                "Sumeragi v2 body completion has conflicting evidence or duplicate serialized owners"
                    .to_owned(),
            ),
            Err(EnqueueError::FailClosed) => {
                Err("Sumeragi v2 runtime is fail-closed".to_owned())
            }
            Err(
                error @ (EnqueueError::ReservedCapacity
                | EnqueueError::Full),
            ) => {
                Err(error.to_string())
            }
        }
    }

    /// Take exclusive ownership of an opened adapter and preserve its recovery
    /// effects for immediate asynchronous dispatch.
    #[cfg(test)]
    pub(crate) fn new(
        adapter: SumeragiV2Adapter,
        startup_effects: Vec<AdapterEffect>,
        started_at: Instant,
        round_timeout: Duration,
        queue_config: RuntimeQueueConfig,
    ) -> Result<(Self, Vec<AdapterEffect>), RuntimeConfigError> {
        Self::with_driver(
            adapter,
            started_at,
            round_timeout,
            queue_config,
            startup_effects,
        )
    }

    /// Open a runtime whose FIFO and fresh roots share the active height's
    /// actor-global source with exact Serve ingress reservations.
    pub(crate) fn new_with_lifecycle_ordinals(
        adapter: SumeragiV2Adapter,
        startup_effects: Vec<AdapterEffect>,
        started_at: Instant,
        round_timeout: Duration,
        queue_config: RuntimeQueueConfig,
        lifecycle_ordinals: RuntimeLifecycleOrdinalSource,
    ) -> Result<(Self, Vec<AdapterEffect>), RuntimeConfigError> {
        Self::with_driver_and_lifecycle_ordinals(
            adapter,
            started_at,
            round_timeout,
            queue_config,
            startup_effects,
            lifecycle_ordinals,
        )
    }

    /// Read the reducer-owned proposal constraint without exposing mutable
    /// access to the authoritative adapter.
    pub(crate) fn local_proposal_directive(
        &self,
    ) -> Result<super::v2::LocalProposalDirective, AdapterError> {
        self.driver.local_proposal_directive()
    }

    /// Return the exact Decision key reconstructed by safety-WAL replay.
    pub(crate) fn replayed_decision_key(
        &self,
    ) -> Result<
        Option<(
            wire::ConsensusRound,
            wire::ConsensusRound,
            wire::BlockSubject,
            wire::ExecutionCommitment,
        )>,
        AdapterError,
    > {
        self.driver.replayed_decision_key()
    }

    /// Rebind one independently durable validation marker before replayed
    /// startup effects are dispatched.
    pub(crate) fn recover_validated_body(
        &mut self,
        manifest: &wire::PayloadManifest,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<(), AdapterError> {
        if self.fail_closed {
            return Err(AdapterError::FailClosed);
        }
        self.driver
            .recover_validated_body(manifest, validated_receipt)
    }

    /// Bind one live, independently durable validation marker without
    /// delivering an obsolete reducer event.
    ///
    /// Effect completions call this inside the same serialized actor turn as
    /// their catalog update. The registry mutation is exact and monotone; it
    /// does not retag or otherwise revive a retired reducer consumer.
    pub(crate) fn bind_validated_body(
        &mut self,
        manifest: &wire::PayloadManifest,
        validated_receipt: &ValidatedBodyReceipt,
    ) -> Result<(), AdapterError> {
        if self.fail_closed {
            return Err(AdapterError::FailClosed);
        }
        self.driver.bind_validated_body(manifest, validated_receipt)
    }

    /// Authenticate and enqueue one reducer-directed network message.
    ///
    /// Traffic which passes the bounded capacity check, exactly matches an
    /// already-owned authenticated envelope, or exactly matches a
    /// Busy-deferred aggregate certificate is cryptographically authenticated
    /// and then checked against canonical authority. Rejections do not poison
    /// the runtime. Once admitted, any adapter transition failure is fatal when
    /// the serialized command is executed.
    pub(crate) fn enqueue_network_with_ingress_ownership(
        &mut self,
        message: wire::ConsensusMessageV2,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> Result<EventTag, NetworkIngressError> {
        if !ingress_ownership.validate_exact() {
            self.latch_fail_closed(
                "network ingress changed its authenticated fair-queue ownership",
            );
            return Err(NetworkIngressError::FailClosed);
        }
        let observed_physical_cut = ingress_ownership.runtime_physical_cut().ok_or_else(|| {
            self.latch_fail_closed(
                "network ingress omitted its checked receiver physical admission cut",
            );
            NetworkIngressError::FailClosed
        })?;
        self.ingress_physical_cut = self.ingress_physical_cut.max(observed_physical_cut);
        let ingress_ownership =
            RuntimeIngressOwnershipEvidence::from_fair_ingress(&message, ingress_ownership)
                .ok_or_else(|| {
                    self.latch_fail_closed(
                        "network ingress changed its authenticated fair-queue ownership",
                    );
                    NetworkIngressError::FailClosed
                })?;
        if !ingress_ownership.validate_frozen_physical() {
            self.latch_fail_closed(
                "network ingress changed its checked receiver physical ownership",
            );
            return Err(NetworkIngressError::FailClosed);
        }
        match ingress_ownership.earliest_lifecycle_ordinal() {
            Ok(Some(ordinal))
                if self
                    .ingress
                    .lifecycle_ordinals
                    .recognizes_minted(ordinal)
                    .unwrap_or(false) => {}
            Ok(None) => {}
            Ok(Some(_)) | Err(_) => {
                self.latch_fail_closed(
                    "network ingress carried an unminted actor-global lifecycle ordinal",
                );
                return Err(NetworkIngressError::FailClosed);
            }
        }
        // Registration is committed only after the authenticated command, or
        // its exact Busy-deferred owner, has retained this carrier. Keeping a
        // clone here avoids publishing a runtime terminal obligation for an
        // authentication or capacity rejection.
        let leader_wire_registration = ingress_ownership.clone();
        let default_class = classify_reducer_network_ingress(self.fail_closed, &message.payload)?;
        let deferred_owner = self.driver.deferred_authenticated_message_owner(&message);
        if let wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) = &message.payload {
            let projected_owner_tag = self
                .driver
                .deferred_quorum_certificate_owner_tag(certificate);
            if projected_owner_tag != deferred_owner.map(|(tag, _)| tag) {
                self.latch_fail_closed(
                    "deferred certificate owner projection disagreed with its exact owner",
                );
                return Err(NetworkIngressError::FailClosed);
            }
        }
        // An exact queued retransmission may always spend authentication work
        // so it can release its ingress occurrence. An exact Busy-deferred
        // aggregate certificate may likewise spend authentication work without
        // claiming a second queue slot. Otherwise, only the adapter's exact
        // active-lock match may proceed after the normal prefix fills.
        // Authentication below remains mandatory before either form of
        // coalescing.
        let may_be_exact_locked_commit =
            self.driver.wire_ingress_may_use_progress(&message.payload);
        if deferred_owner.is_none() {
            self.ingress
                .check_authenticated_wire_capacity_with_ownership(
                    &message,
                    &ingress_ownership,
                    default_class,
                    may_be_exact_locked_commit,
                )
                .map_err(NetworkIngressError::Backpressure)?;
        }
        let authenticated = match self.driver.authenticate(message) {
            Ok(authenticated) => authenticated,
            Err(AdapterError::FailClosed | AdapterError::ReplayNotComplete) => {
                self.latch_fail_closed("network authentication observed a closed adapter");
                return Err(NetworkIngressError::FailClosed);
            }
            Err(error) => return Err(NetworkIngressError::Authentication(error)),
        };
        let authenticated_deferred_owner = self
            .driver
            .deferred_authenticated_message_owner(authenticated.wire_envelope());
        if authenticated_deferred_owner != deferred_owner {
            // Authentication does not mutate the adapter or envelope. Any
            // disagreement would invalidate the raw-capacity hint rather than
            // authorizing an unchecked queue insertion.
            self.latch_fail_closed(
                "network authentication changed deferred certificate ownership classification",
            );
            return Err(NetworkIngressError::FailClosed);
        }
        if let Some((owner_tag, admission_ordinal)) = authenticated_deferred_owner {
            match self
                .reconcile_deferred_ingress_ownership(Some((admission_ordinal, ingress_ownership)))
            {
                Ok(()) => {}
                Err(RuntimeIngressMergeError::Capacity) => {
                    return Err(NetworkIngressError::Backpressure(EnqueueError::Full));
                }
                Err(
                    RuntimeIngressMergeError::Conflict
                    | RuntimeIngressMergeError::IndependentOccurrence,
                ) => {
                    self.latch_fail_closed(
                        "deferred certificate admission lost authenticated ingress ownership",
                    );
                    return Err(NetworkIngressError::FailClosed);
                }
            }
            if self
                .register_leader_wire_runtime_receipt(&leader_wire_registration)
                .is_err()
            {
                self.latch_fail_closed(
                    "deferred certificate admission changed its leader-wire runtime receipt",
                );
                return Err(NetworkIngressError::FailClosed);
            }
            return Ok(owner_tag);
        }
        let class = if self
            .driver
            .authenticated_ingress_is_progress(&authenticated)
        {
            CommandClass::Progress
        } else {
            default_class
        };
        if self
            .ingress
            .conflicts_with_pending_body_available(&authenticated)
        {
            return Err(NetworkIngressError::Authentication(
                AdapterError::ConflictingManifest,
            ));
        }
        let tag = self.driver.current_tag();
        let command = AdapterCommand::Authenticated(authenticated.clone());
        let preflight = self
            .command_admission_preflight(tag, class, &command)
            .map_err(NetworkIngressError::Backpressure)?;
        let preflight = self.reject_authenticated_preflight_coalescence(preflight)?;
        let restored_owner = match preflight {
            RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key,
                admission_ordinal,
                producer_stage,
            } => Some((
                self.restored_command_owner(
                    tag,
                    class,
                    &command,
                    Some(&ingress_ownership),
                    causal_lifecycle_key,
                    admission_ordinal,
                )
                .map_err(NetworkIngressError::Backpressure)?,
                producer_stage,
            )),
            RuntimeCommandAdmissionPreflight::Admit => None,
            RuntimeCommandAdmissionPreflight::Coalesce
            | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => {
                unreachable!("handled above")
            }
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        if let Some((owner, _)) = restored_owner.as_ref() {
            match self.clock_owner_reservation_blocks(owner) {
                Ok(true) => {
                    // Preserve the exact fair-ingress occurrence outside the
                    // FIFO until the clock target transfers.  Returning
                    // ordinary backpressure keeps retries coalesced on the
                    // same transport carrier and allocates no new position.
                    return Err(NetworkIngressError::Backpressure(EnqueueError::Full));
                }
                Ok(false) => {}
                Err(_) => {
                    self.latch_fail_closed(
                        "network replay observed invalid clock reservation ownership",
                    );
                    return Err(NetworkIngressError::FailClosed);
                }
            }
        }
        match self
            .ingress
            .enqueue_authenticated_with_ingress_ownership_and_owner(
                tag,
                class,
                authenticated,
                ingress_ownership,
                restored_owner
                    .as_ref()
                    .map(|(owner, producer_stage)| (owner, *producer_stage)),
            ) {
            Ok(owner) => {
                if self
                    .register_leader_wire_runtime_receipt(&leader_wire_registration)
                    .is_err()
                {
                    self.latch_fail_closed(
                        "authenticated admission changed its leader-wire runtime receipt",
                    );
                    return Err(NetworkIngressError::FailClosed);
                }
                Ok(owner)
            }
            Err(EnqueueError::FailClosed) => {
                self.latch_fail_closed("authenticated ingress exact ownership validation failed");
                Err(NetworkIngressError::FailClosed)
            }
            Err(error) => Err(NetworkIngressError::Backpressure(error)),
        }
    }

    /// Test-only direct ingress helper. Production callers must preserve the
    /// fair-ingress carrier obtained from the authenticated network boundary.
    #[cfg(test)]
    pub(crate) fn enqueue_network(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<EventTag, NetworkIngressError> {
        let mut admitted = super::fair_v2_ingress_admit_for_test(super::InboundBlockMessage::new(
            super::message::BlockMessage::V2(message.clone()),
            None,
        ));
        let ingress_ownership = admitted
            .take_ingress_ownership()
            .expect("real test fair ingress produces exact ownership");
        self.enqueue_network_with_ingress_ownership(message, ingress_ownership)
    }

    /// Return whether the fair-ingress head can reach authentication and then
    /// either claim its exact runtime prefix or coalesce with an exact queued
    /// authenticated owner.
    fn can_admit_pre_runtime_leader_wire(
        &self,
        outer_message: &wire::ConsensusMessageV2,
        runtime_message: &wire::ConsensusMessageV2,
        default_class: CommandClass,
        ownership: &FairV2IngressOwnershipEvidence,
    ) -> Option<bool> {
        let token = ownership.leader_wire_token()?;
        if ownership.leader_wire_runtime_receipt().is_some() {
            return None;
        }

        // Productive fair ingress owns the durable Ingress token while the
        // packet is still physically queued. Its Runtime receipt can only be
        // minted by the atomic dequeue immediately after this read-only
        // predicate succeeds. Validate that exact pre-handoff state here;
        // generic runtime identity permits the absent receipt and physical cut
        // only for this read-only probe, while mutating admission still
        // requires the dequeue-frozen pair.
        let outer = super::message::BlockMessage::V2(outer_message.clone());
        if !ownership.validate_exact()
            || !ownership.matches_message(&outer)
            || ownership.runtime_physical_cut().is_some()
            || ownership.runtime_lifecycle_ordinal() != Some(token.scheduler_ordinal())
            || !self
                .ingress
                .lifecycle_ordinals
                .recognizes_minted(token.scheduler_ordinal())
                .unwrap_or(false)
        {
            // Drain malformed process-local ownership so the mutating seam
            // reports the exact invariant failure instead of pinning a fair
            // lane forever.
            return Some(true);
        }

        if self.fail_closed {
            return Some(false);
        }
        if let Some((round, _)) = self
            .driver
            .wire_ingress_missing_execution_commitment(&runtime_message.payload)
            && round.height == self.round_tag.height()
            && round.view == self.round_tag.view()
        {
            return Some(false);
        }

        if let Some((_, admission_ordinal)) = self
            .driver
            .deferred_authenticated_message_owner(runtime_message)
        {
            // A Busy-deferred aggregate already owns its sole serialized
            // occurrence. An exact restart retry may rejoin that lifecycle;
            // a distinct productive token must remain in fair ingress until
            // the deferred owner retires and a real FIFO slot is available.
            let same_token = self
                .deferred_ingress_ownership
                .get(&admission_ordinal)
                .and_then(|retained| retained.leader_wire_token().ok().flatten())
                == Some(token);
            return Some(same_token);
        }

        for queued in &self.ingress.commands {
            if !queued.command.matches_wire_envelope(runtime_message) {
                continue;
            }
            let Some(retained) = queued.ingress_ownership.as_ref() else {
                // Let the mutating seam expose a corrupt authenticated owner.
                return Some(true);
            };
            match retained.leader_wire_token() {
                Ok(Some(retained_token)) if retained_token == token => return Some(true),
                Ok(_) => {}
                Err(_) => return Some(true),
            }
        }

        let Some(source_physical_ordinal) = ownership.physical_admission_ordinal() else {
            return Some(true);
        };
        match self.clock_owner_reservation_blocks_occurrence(
            token.scheduler_ordinal(),
            source_physical_ordinal,
        ) {
            Ok(true) => return Some(false),
            Ok(false) => {}
            // Drain malformed process-local state so the mutating seam can
            // expose the invariant failure instead of pinning a fair lane.
            Err(_) => return Some(true),
        }

        let may_use_progress = self
            .driver
            .wire_ingress_may_use_progress(&runtime_message.payload);
        let capacity = match self.ingress.check_capacity(default_class) {
            Ok(()) => Ok(()),
            Err(_) if may_use_progress => self.ingress.check_capacity(CommandClass::Progress),
            Err(error) => Err(error),
        };
        Some(capacity.is_ok())
    }

    pub(crate) fn can_admit_network_message_with_ingress_ownership(
        &self,
        message: &wire::ConsensusMessageV2,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> bool {
        let outer_message = message;
        let (runtime_message, default_class) = match &message.payload {
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => (
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    response.certificate.clone(),
                )),
                CommandClass::Progress,
            ),
            payload => {
                let Some(class) = network_command_class(payload) else {
                    // Body/chunk transport does not enter the reducer FIFO.
                    return true;
                };
                (message.clone(), class)
            }
        };
        if let Some(admissible) = self.can_admit_pre_runtime_leader_wire(
            outer_message,
            &runtime_message,
            default_class,
            ingress_ownership,
        ) {
            return admissible;
        }
        let Some(ownership) = RuntimeIngressOwnershipEvidence::from_fair_ingress(
            &runtime_message,
            ingress_ownership.clone(),
        ) else {
            // Drain malformed process-local ownership so the mutating seam can
            // fail closed instead of leaving the fair queue permanently stuck.
            return true;
        };
        if matches!(
            ownership.earliest_lifecycle_ordinal(),
            Ok(Some(ordinal))
                if !self
                    .ingress
                    .lifecycle_ordinals
                    .recognizes_minted(ordinal)
                    .unwrap_or(false)
        ) {
            // As with malformed ownership, let the mutating seam consume and
            // fail closed instead of pinning a corrupt fair-ingress head.
            return true;
        }
        if self.fail_closed {
            return false;
        }
        if let (Ok(Some(lifecycle_ordinal)), Ok(Some(physical))) = (
            ownership.earliest_lifecycle_ordinal(),
            ownership.earliest_physical_carrier(),
        ) {
            match self.clock_owner_reservation_blocks_occurrence(
                lifecycle_ordinal,
                physical.source_ordinal,
            ) {
                Ok(true) => return false,
                Ok(false) => {}
                // Let the mutating seam consume malformed state and latch
                // fail-closed instead of pinning the fair-ingress head.
                Err(_) => return true,
            }
        }
        if let Some((round, _)) = self
            .driver
            .wire_ingress_missing_execution_commitment(&runtime_message.payload)
            && round.height == self.round_tag.height()
            && round.view == self.round_tag.view()
        {
            // The fair-ingress occurrence is the only durable process-local
            // owner at this boundary. Retain a current-view direct vote until
            // proposal validation binds its execution commitment. Proposal
            // and body traffic may arrive through independent source lanes
            // after the vote, and periodic retransmission remains best effort.
            // Fair ingress is bounded per source and bypasses blocked entries,
            // so an unknown subject cannot globally block later traffic. A
            // future-view vote has no certified local transition authority and
            // must drain normally; once the local view advances, an unmatched
            // current-view vote likewise drains for bounded rejection.
            return false;
        }
        if let Some((_, ordinal)) = self
            .driver
            .deferred_authenticated_message_owner(&runtime_message)
        {
            return self
                .deferred_ingress_ownership
                .get(&ordinal)
                .is_some_and(|retained| retained.can_merge_downstream(&ownership));
        }
        let may_be_exact_locked_commit = self
            .driver
            .wire_ingress_may_use_progress(&runtime_message.payload);
        self.ingress
            .check_authenticated_wire_capacity_with_ownership(
                &runtime_message,
                &ownership,
                default_class,
                may_be_exact_locked_commit,
            )
            .is_ok()
    }

    #[cfg(test)]
    pub(crate) fn can_admit_network_message(&self, message: &wire::ConsensusMessageV2) -> bool {
        let mut admitted = super::fair_v2_ingress_admit_for_test(super::InboundBlockMessage::new(
            super::message::BlockMessage::V2(message.clone()),
            None,
        ));
        let ownership = admitted
            .take_ingress_ownership()
            .expect("real test fair ingress produces exact ownership");
        self.can_admit_network_message_with_ingress_ownership(message, &ownership)
    }

    /// Enqueue a completed local proposal build with its original reducer tag.
    pub(crate) fn enqueue_local_proposal(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable_receipt.clone(),
            validated_receipt: validated_receipt.clone(),
        };
        self.enqueue_body_pipeline_completion(
            tag,
            evidence,
            AdapterCommand::LocalProposalReady {
                manifest,
                durable_receipt,
                validated_receipt,
            },
        )
    }

    /// Enqueue a completed local proposal without changing the immutable
    /// `AssembleBody` lifecycle owner minted when the proposal entered the
    /// asynchronous Store -> Validate pipeline.
    pub(crate) fn enqueue_local_proposal_with_owner(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        durable_receipt: DurableBodyReceipt,
        validated_receipt: ValidatedBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::LocalProposalReady {
            manifest: manifest.clone(),
            durable_receipt: durable_receipt.clone(),
            validated_receipt: validated_receipt.clone(),
        };
        self.enqueue_body_pipeline_completion_with_owner(
            tag,
            evidence,
            AdapterCommand::LocalProposalReady {
                manifest,
                durable_receipt,
                validated_receipt,
            },
            ownership,
        )
    }

    /// Enqueue successful canonical reconstruction with the exact fetch tag.
    ///
    /// Authenticated proposals already waiting in the FIFO are discarded only
    /// when they advertise a different manifest for this exact round and
    /// subject. Every retained command keeps its original relative order, and
    /// the completion is appended normally.
    #[cfg(test)]
    pub(crate) fn enqueue_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<(), EnqueueError> {
        let reservation = self.reserve_body_available(tag, manifest)?;
        self.commit_body_available(reservation)
    }

    /// Reserve exact runtime ownership for a reconstructed body completion.
    ///
    /// Capacity and conflicting queued proposals are evaluated without
    /// exposing a reducer command. The returned token exclusively owns any
    /// claimed completion slot until committed or terminally retired. An
    /// executor abort retains this exact unpublished owner for retry.
    pub(crate) fn reserve_body_available(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        let already_owned = self.body_pipeline_completion_is_owned(tag, &evidence)?;
        if already_owned {
            if self.ingress.reserved_body_available.is_none() {
                return Ok(BodyAvailableReservation::coalesced(tag, manifest));
            }
            let result = self
                .ingress
                .reserve_canonical_body_available_internal(tag, manifest, None, None, None);
            if matches!(
                result,
                Err(EnqueueError::FailClosed | EnqueueError::DuplicateCompletionOwnership)
            ) {
                self.latch_fail_closed("body-available reservation ownership validation failed");
            }
            return result;
        }
        let command = AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        };
        let preflight =
            self.command_admission_preflight(tag, CommandClass::Completion, &command)?;
        let restored_owner = match preflight {
            RuntimeCommandAdmissionPreflight::Coalesce
            | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => {
                return Ok(BodyAvailableReservation::coalesced(tag, manifest));
            }
            RuntimeCommandAdmissionPreflight::Admit => None,
            RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key,
                admission_ordinal,
                producer_stage,
            } => Some((
                self.restored_command_owner(
                    tag,
                    CommandClass::Completion,
                    &command,
                    None,
                    causal_lifecycle_key,
                    admission_ordinal,
                )?,
                producer_stage,
            )),
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        let result = self.ingress.reserve_canonical_body_available_internal(
            tag,
            manifest,
            restored_owner.as_ref().map(|(owner, _)| owner),
            None,
            restored_owner
                .as_ref()
                .map(|(_, producer_stage)| *producer_stage),
        );
        if matches!(
            result,
            Err(EnqueueError::FailClosed | EnqueueError::DuplicateCompletionOwnership)
        ) {
            self.latch_fail_closed("body-available reservation ownership validation failed");
        }
        result
    }

    /// Reserve a reconstructed-body successor while retaining the FetchBody
    /// lifecycle owner.
    pub(crate) fn reserve_body_available_with_owner(
        &mut self,
        tag: EventTag,
        manifest: wire::PayloadManifest,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<BodyAvailableReservation, EnqueueError> {
        if self.fail_closed || !ownership.validate_exact() {
            return Err(EnqueueError::FailClosed);
        }
        let evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        let already_owned = self.body_pipeline_completion_is_owned_by(tag, &evidence, ownership)?;
        if already_owned {
            if self.ingress.reserved_body_available.is_none() {
                return BodyAvailableReservation::coalesced_with_owner(tag, manifest, ownership);
            }
            let result = self.ingress.reserve_canonical_body_available_internal(
                tag,
                manifest,
                Some(ownership.owner()),
                ownership.candidate_semantic_statement(),
                None,
            );
            if matches!(
                result,
                Err(EnqueueError::FailClosed | EnqueueError::DuplicateCompletionOwnership)
            ) {
                self.latch_fail_closed("owned body-available reservation validation failed");
            }
            return result;
        }
        let command = AdapterCommand::BodyAvailable {
            manifest: manifest.clone(),
        };
        let preflight =
            self.command_admission_preflight(tag, CommandClass::Completion, &command)?;
        if self.owned_preflight_is_coalesced(tag, preflight, ownership)? {
            return BodyAvailableReservation::coalesced_with_owner(tag, manifest, ownership);
        }
        let restored_owner = match preflight {
            RuntimeCommandAdmissionPreflight::Admit => None,
            RuntimeCommandAdmissionPreflight::ReuseDormant {
                causal_lifecycle_key,
                admission_ordinal,
                producer_stage,
            } => Some((
                self.restored_command_owner(
                    tag,
                    CommandClass::Completion,
                    &command,
                    None,
                    causal_lifecycle_key,
                    admission_ordinal,
                )?,
                producer_stage,
            )),
            RuntimeCommandAdmissionPreflight::Coalesce
            | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => {
                unreachable!("handled above")
            }
            RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
        };
        let owner = restored_owner
            .as_ref()
            .map_or_else(|| ownership.owner(), |(owner, _)| owner);
        let result = self.ingress.reserve_canonical_body_available_internal(
            tag,
            manifest,
            Some(owner),
            ownership.candidate_semantic_statement(),
            restored_owner
                .as_ref()
                .map(|(_, producer_stage)| *producer_stage),
        );
        if matches!(
            result,
            Err(EnqueueError::FailClosed | EnqueueError::DuplicateCompletionOwnership)
        ) {
            self.latch_fail_closed("owned body-available reservation validation failed");
        }
        result
    }

    /// Publish one previously reserved completion without another capacity
    /// check. A stale or mismatched token is an internal ownership violation
    /// and permanently closes the serialized runtime.
    pub(crate) fn commit_body_available(
        &mut self,
        reservation: BodyAvailableReservation,
    ) -> Result<(), EnqueueError> {
        let result = self.ingress.commit_canonical_body_available(reservation);
        if result.is_err() {
            self.latch_fail_closed("body-available reservation commit token did not match");
        }
        result
    }

    /// Retain an unpublished completion reservation after an all-or-error
    /// service transfer rejected the operation. The exact retry reclaims the
    /// same token and ordinal; this is not a terminal release. A stale or
    /// mismatched token is an intentional no-op because abort carries no
    /// authority to clear the retained owner.
    pub(crate) fn abort_body_available(&mut self, reservation: BodyAvailableReservation) {
        self.ingress.abort_canonical_body_available(reservation);
    }

    /// Transfer one already admitted exact-body completion to a certified later incarnation.
    ///
    /// The completion can be waiting either in runtime ingress or in the adapter's Busy-deferred
    /// completion lane. `rebound` must be the runtime's installed incarnation,
    /// and source and destination slots are both checked before either queue is
    /// mutated. A single exact destination owner coalesces the transfer by
    /// retiring the unique source; conflicting evidence or duplicate ownership
    /// at either tag fails closed without mutation. Success leaves exactly one
    /// full-evidence owner at `rebound`.
    pub(crate) fn rebind_body_available(
        &mut self,
        previous: EventTag,
        rebound: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        if !rebound.strictly_advances(previous) {
            return Err(
                "Sumeragi v2 body completion rebind did not advance its incarnation".to_owned(),
            );
        }
        if rebound != self.round_tag {
            return Err(
                "Sumeragi v2 body completion rebind target is not the installed runtime incarnation"
                    .to_owned(),
            );
        }
        let source_owned = self.body_available_is_uniquely_owned(previous, manifest)?;
        let destination_owned = self.body_available_is_uniquely_owned(rebound, manifest)?;
        if !source_owned {
            return Ok(false);
        }

        let transferred = if destination_owned {
            let ingress = self
                .ingress
                .retire_canonical_body_available(previous, manifest);
            let deferred = self
                .driver
                .retire_deferred_body_available(previous, manifest);
            ingress.saturating_add(deferred)
        } else {
            let ingress = self
                .ingress
                .rebind_canonical_body_available(previous, rebound, manifest);
            let deferred = self
                .driver
                .rebind_deferred_body_available(previous, rebound, manifest);
            ingress.saturating_add(deferred)
        };
        if transferred != 1 {
            self.latch_fail_closed("body completion rebind changed its serialized owner count");
            return Err(
                "Sumeragi v2 body completion ownership changed during serialized rebind".to_owned(),
            );
        }
        if self
            .reconcile_deferred_runtime_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed("body completion rebind lost deferred runtime ownership");
            return Err(
                "Sumeragi v2 body completion rebind lost deferred runtime ownership".to_owned(),
            );
        }

        match self.body_available_is_uniquely_owned(rebound, manifest) {
            Ok(true) => {}
            Ok(false) => {
                self.latch_fail_closed("body completion rebind left no destination owner");
                return Err(
                    "Sumeragi v2 body completion rebind did not leave one destination owner"
                        .to_owned(),
                );
            }
            Err(error) => return Err(error),
        }
        Ok(true)
    }

    /// Retire one superseded exact-body completion from its serialized owner.
    ///
    /// The completion may still be waiting in runtime ingress or may already
    /// have crossed into the adapter's Busy-deferred completion lane. Exactly
    /// one owner with the exact manifest evidence is permitted across both
    /// queues, and ownership is checked before either queue is mutated.
    pub(crate) fn retire_body_available(
        &mut self,
        tag: EventTag,
        manifest: &wire::PayloadManifest,
    ) -> Result<bool, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        if !self.body_available_is_uniquely_owned(tag, manifest)? {
            return Ok(false);
        }
        let ingress = self.ingress.retire_canonical_body_available(tag, manifest);
        let deferred = self.driver.retire_deferred_body_available(tag, manifest);
        let total = ingress.saturating_add(deferred);
        if total != 1 {
            self.latch_fail_closed("body completion retirement changed its owner count");
            return Err(
                "Sumeragi v2 body completion ownership changed during serialized retirement"
                    .to_owned(),
            );
        }
        if self
            .reconcile_deferred_runtime_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed("body completion retirement lost deferred runtime ownership");
            return Err(
                "Sumeragi v2 body completion retirement lost deferred runtime ownership".to_owned(),
            );
        }
        Ok(true)
    }

    /// Retire every queued completion stage for one exact superseded body pipeline.
    ///
    /// The command may still be in runtime ingress or may have crossed into
    /// the adapter's Busy-deferred completion lane. Different stage slots can
    /// coexist, but each slot must have only one serialized owner. Both lanes
    /// are counted before mutation, so duplicate ownership fails closed while
    /// every occurrence remains available for diagnosis.
    pub(crate) fn retire_body_pipeline_completions(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<RetiredBodyPipelineCompletions, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        let expected = self
            .ingress
            .body_pipeline_completion_counts(tag, round, subject)
            .merge(
                self.driver
                    .deferred_body_pipeline_completion_counts(tag, round, subject),
            );
        let expected = match expected.validate_unique() {
            Ok(expected) => expected,
            Err(error) => {
                self.latch_fail_closed(
                    "body pipeline completion retirement found duplicate owners",
                );
                return Err(error);
            }
        };
        let ingress = self
            .ingress
            .retire_body_pipeline_completions(tag, round, subject);
        let deferred = self
            .driver
            .retire_deferred_body_pipeline_completions(tag, round, subject);
        let retired = ingress.merge(deferred);
        let remaining = self
            .ingress
            .body_pipeline_completion_counts(tag, round, subject)
            .merge(
                self.driver
                    .deferred_body_pipeline_completion_counts(tag, round, subject),
            );
        if retired != expected || remaining != RetiredBodyPipelineCompletions::default() {
            self.latch_fail_closed("body pipeline completion retirement changed ownership");
            return Err(
                "Sumeragi v2 body pipeline ownership changed during serialized retirement"
                    .to_owned(),
            );
        }
        if self
            .reconcile_deferred_runtime_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed(
                "body pipeline completion retirement lost deferred runtime ownership",
            );
            return Err(
                "Sumeragi v2 body pipeline retirement lost deferred runtime ownership".to_owned(),
            );
        }
        Ok(retired)
    }

    /// Retire proposal work made terminal by an exact durable decision.
    ///
    /// Every authenticated proposal and nonmatching local proposal completion
    /// at the decided height is removed from both serialized owners. One exact
    /// current-tag completion remains queued only when its full manifest,
    /// durable receipt, validation receipt, and execution commitment match the
    /// Decision. `decision_round` identifies the selected durable body origin;
    /// it may precede the CommitQC round. Stale exact work is removed for
    /// ordinary durable recovery.
    /// Duplicate or conflicting exact owners fail closed before mutation.
    pub(crate) fn retire_proposal_work_after_decision(
        &mut self,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
        decision_commitment: wire::ExecutionCommitment,
    ) -> Result<DecisionProposalRetirement, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        let decision_tag = self.round_tag;
        let expected = self
            .ingress
            .decided_local_proposal_counts(
                decision_tag,
                decision_round,
                decision_subject,
                decision_commitment,
            )
            .merge(self.driver.deferred_decided_local_proposal_counts(
                decision_tag,
                decision_round,
                decision_subject,
                decision_commitment,
            ));
        if expected.conflicting() != 0 {
            self.latch_fail_closed("decided local proposal evidence conflicted with Decision");
            return Err(
                "Sumeragi v2 decided local proposal evidence conflicts with the durable Decision"
                    .to_owned(),
            );
        }
        if expected.total() > 1 {
            self.latch_fail_closed("decided local proposal had duplicate serialized owners");
            return Err(
                "Sumeragi v2 decided local proposal completion has duplicate serialized owners"
                    .to_owned(),
            );
        }
        self.ingress.retire_proposal_work_after_decision(
            decision_tag,
            decision_round,
            decision_subject,
            decision_commitment,
        );
        self.driver.retire_deferred_proposal_work_after_decision(
            decision_tag,
            decision_round,
            decision_subject,
            decision_commitment,
        );
        if self
            .reconcile_deferred_runtime_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed("decided proposal retirement lost deferred runtime ownership");
            return Err(
                "Sumeragi v2 deferred proposal retirement lost runtime ownership".to_owned(),
            );
        }
        let remaining = self
            .ingress
            .decided_local_proposal_counts(
                decision_tag,
                decision_round,
                decision_subject,
                decision_commitment,
            )
            .merge(self.driver.deferred_decided_local_proposal_counts(
                decision_tag,
                decision_round,
                decision_subject,
                decision_commitment,
            ));
        if remaining.conflicting() != 0
            || remaining.recovery_only() != 0
            || remaining.retainable() != expected.retainable()
            || remaining.total() != expected.retainable()
        {
            self.latch_fail_closed("decided proposal retirement changed ownership");
            return Err(
                "Sumeragi v2 decided local proposal ownership changed during serialized retirement"
                    .to_owned(),
            );
        }
        // Decision is the other terminal arm for the active-view producer.
        // The exact durable certificate already owns recovery/application, so
        // retaining a proposal fence here would resurrect work finality that
        // the retirement above has deliberately closed.
        self.active_view_producer = None;
        Ok(DecisionProposalRetirement::new(
            (expected.retainable() == 1).then_some(decision_tag),
            expected.recovery_only(),
        ))
    }

    /// Retire authenticated proposals which a newly installed lock makes unsafe.
    ///
    /// The exact locked subject may remain queued for unchanged reproposal.
    /// A competing subject survives only with the strictly higher matching
    /// PrepareQC required by the shared safe-value rule.
    pub(crate) fn retire_unsafe_proposals_for_lock(
        &mut self,
        locked_round: wire::ConsensusRound,
        locked_subject: wire::BlockSubject,
    ) -> Result<usize, String> {
        if self.fail_closed {
            return Err("Sumeragi v2 runtime is fail-closed".to_owned());
        }
        let ingress = self
            .ingress
            .retire_unsafe_proposals_for_lock(locked_round, locked_subject);
        let deferred = self
            .driver
            .retire_deferred_unsafe_proposals_for_lock(locked_round, locked_subject);
        if self
            .reconcile_deferred_runtime_ownership_after_retirement()
            .is_err()
        {
            self.latch_fail_closed("unsafe proposal retirement lost deferred runtime ownership");
            return Err("Sumeragi v2 unsafe-proposal retirement lost runtime ownership".to_owned());
        }
        Ok(ingress.saturating_add(deferred))
    }

    /// Enqueue the durable body-store acknowledgement with its exact tag.
    pub(crate) fn enqueue_body_stored(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::BodyStored {
            round,
            subject,
            receipt: receipt.clone(),
        };
        self.enqueue_body_pipeline_completion(
            tag,
            evidence,
            AdapterCommand::BodyStored {
                round,
                subject,
                receipt,
            },
        )
    }

    pub(crate) fn enqueue_body_stored_with_owner(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: DurableBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::BodyStored {
            round,
            subject,
            receipt: receipt.clone(),
        };
        self.enqueue_body_pipeline_completion_with_owner(
            tag,
            evidence,
            AdapterCommand::BodyStored {
                round,
                subject,
                receipt,
            },
            ownership,
        )
    }

    /// Enqueue successful deterministic validation with its non-forgeable
    /// receipt and the tag of its currently attached reducer consumer.
    pub(crate) fn enqueue_validation_succeeded(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::ValidationSucceeded {
            round,
            subject,
            receipt: receipt.clone(),
        };
        self.enqueue_body_pipeline_completion(
            tag,
            evidence,
            AdapterCommand::ValidationSucceeded {
                round,
                subject,
                receipt,
            },
        )
    }

    pub(crate) fn enqueue_validation_succeeded_with_owner(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: ValidatedBodyReceipt,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::ValidationSucceeded {
            round,
            subject,
            receipt: receipt.clone(),
        };
        self.enqueue_body_pipeline_completion_with_owner(
            tag,
            evidence,
            AdapterCommand::ValidationSucceeded {
                round,
                subject,
                receipt,
            },
            ownership,
        )
    }

    /// Enqueue deterministic validation rejection for its currently attached
    /// reducer consumer.
    pub(crate) fn enqueue_validation_failed(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::ValidationFailed { round, subject };
        self.enqueue_body_pipeline_completion(
            tag,
            evidence,
            AdapterCommand::ValidationFailed { round, subject },
        )
    }

    pub(crate) fn enqueue_validation_failed_with_owner(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        let evidence = BodyPipelineCompletionEvidence::ValidationFailed { round, subject };
        self.enqueue_body_pipeline_completion_with_owner(
            tag,
            evidence,
            AdapterCommand::ValidationFailed { round, subject },
            ownership,
        )
    }

    /// Atomically enqueue a set of deterministic validation rejections.
    ///
    /// Exact pre-existing owners coalesce. Every vacant owner and the complete
    /// completion-capacity requirement are checked before any command becomes
    /// visible to the reducer.
    pub(crate) fn enqueue_validation_failures_atomically(
        &mut self,
        failures: &[(EventTag, wire::ConsensusRound, wire::BlockSubject)],
    ) -> Result<(), EnqueueError> {
        if self.fail_closed {
            return Err(EnqueueError::FailClosed);
        }
        let mut keys = BTreeSet::new();
        let mut commands = Vec::with_capacity(failures.len());
        let admitted_at = Instant::now();
        for (tag, round, subject) in failures.iter().copied() {
            if !keys.insert((round, subject)) {
                self.latch_fail_closed("validation failure batch contained duplicate body owners");
                return Err(EnqueueError::DuplicateCompletionOwnership);
            }
            let evidence = BodyPipelineCompletionEvidence::ValidationFailed { round, subject };
            if self.body_pipeline_completion_is_owned(tag, &evidence)? {
                continue;
            }
            let command = AdapterCommand::ValidationFailed { round, subject };
            let preflight =
                self.command_admission_preflight(tag, CommandClass::Completion, &command)?;
            let tagged = match preflight {
                RuntimeCommandAdmissionPreflight::Coalesce
                | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => continue,
                RuntimeCommandAdmissionPreflight::Admit => {
                    TaggedCommand::new(tag, CommandClass::Completion, command, admitted_at)
                }
                RuntimeCommandAdmissionPreflight::ReuseDormant {
                    causal_lifecycle_key,
                    admission_ordinal,
                    producer_stage,
                } => self.restored_tagged_command(
                    tag,
                    CommandClass::Completion,
                    command,
                    admitted_at,
                    causal_lifecycle_key,
                    admission_ordinal,
                    producer_stage,
                )?,
                RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
            };
            commands.push(tagged);
        }
        let result = self.ingress.enqueue_completion_batch(commands);
        if result == Err(EnqueueError::FailClosed) {
            self.latch_fail_closed("validation failure batch ownership validation failed");
        }
        result
    }

    /// Atomically enqueue validation rejections while preserving the exact
    /// lifecycle owner of every independently admitted validation task.
    pub(crate) fn enqueue_validation_failures_atomically_with_owners(
        &mut self,
        failures: &[(
            EventTag,
            wire::ConsensusRound,
            wire::BlockSubject,
            RuntimeEffectOwnership,
        )],
    ) -> Result<(), EnqueueError> {
        if self.fail_closed {
            return Err(EnqueueError::FailClosed);
        }
        let mut keys = BTreeSet::new();
        let mut commands = Vec::with_capacity(failures.len());
        let admitted_at = Instant::now();
        for (tag, round, subject, ownership) in failures {
            if !ownership.validate_exact() {
                self.latch_fail_closed(
                    "validation failure batch contained invalid lifecycle ownership",
                );
                return Err(EnqueueError::FailClosed);
            }
            if !keys.insert((*round, *subject)) {
                self.latch_fail_closed("validation failure batch contained duplicate body owners");
                return Err(EnqueueError::DuplicateCompletionOwnership);
            }
            let evidence = BodyPipelineCompletionEvidence::ValidationFailed {
                round: *round,
                subject: *subject,
            };
            if self.body_pipeline_completion_is_owned_by(*tag, &evidence, ownership)? {
                continue;
            }
            let command = AdapterCommand::ValidationFailed {
                round: *round,
                subject: *subject,
            };
            let preflight =
                self.command_admission_preflight(*tag, CommandClass::Completion, &command)?;
            if self.owned_preflight_is_coalesced(*tag, preflight, ownership)? {
                continue;
            }
            let restored_owner = match preflight {
                RuntimeCommandAdmissionPreflight::ReuseDormant {
                    causal_lifecycle_key,
                    admission_ordinal,
                    producer_stage,
                } => Some((
                    self.restored_command_owner(
                        *tag,
                        CommandClass::Completion,
                        &command,
                        None,
                        causal_lifecycle_key,
                        admission_ordinal,
                    )?,
                    producer_stage,
                )),
                RuntimeCommandAdmissionPreflight::Admit => None,
                RuntimeCommandAdmissionPreflight::Coalesce
                | RuntimeCommandAdmissionPreflight::CoalesceOwned { .. } => {
                    unreachable!("handled above")
                }
                RuntimeCommandAdmissionPreflight::Reject => unreachable!("reject handled above"),
            };
            let owner = restored_owner
                .as_ref()
                .map_or_else(|| ownership.owner(), |(owner, _)| owner);
            let mut tagged = TaggedCommand::with_causal_origin(
                *tag,
                CommandClass::Completion,
                command,
                admitted_at,
                owner.causal_origin().clone(),
                owner.lifecycle_ordinal(),
            )?;
            tagged.restored_producer_stage =
                restored_owner.map(|(_, producer_stage)| producer_stage);
            commands.push(tagged);
        }
        let result = self.ingress.enqueue_completion_batch(commands);
        if result == Err(EnqueueError::FailClosed) {
            self.latch_fail_closed("owned validation failure batch validation failed");
        }
        result
    }

    /// Enqueue a signer completion without retagging it to the current view.
    pub(crate) fn enqueue_signature(
        &mut self,
        tag: EventTag,
        signature: Vec<u8>,
    ) -> Result<(), EnqueueError> {
        self.enqueue(
            tag,
            CommandClass::Completion,
            AdapterCommand::SignatureCompleted(signature),
        )
    }

    pub(crate) fn enqueue_signature_with_owner(
        &mut self,
        tag: EventTag,
        signature: Vec<u8>,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        self.enqueue_with_lifecycle_owner(
            tag,
            CommandClass::Completion,
            AdapterCommand::SignatureCompleted(signature),
            ownership,
        )
    }

    /// Enqueue an application completion without retagging it.
    pub(crate) fn enqueue_application_completed(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
    ) -> Result<(), EnqueueError> {
        self.enqueue(
            tag,
            CommandClass::Completion,
            AdapterCommand::ApplicationCompleted(subject),
        )
    }

    pub(crate) fn enqueue_application_completed_with_owner(
        &mut self,
        tag: EventTag,
        subject: wire::BlockSubject,
        ownership: &RuntimeEffectOwnership,
    ) -> Result<(), EnqueueError> {
        self.enqueue_with_lifecycle_owner(
            tag,
            CommandClass::Completion,
            AdapterCommand::ApplicationCompleted(subject),
            ownership,
        )
    }
}

fn network_command_class(payload: &wire::ConsensusMessageV2Payload) -> Option<CommandClass> {
    match payload {
        wire::ConsensusMessageV2Payload::QuorumCertificate(_)
        | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
        | wire::ConsensusMessageV2Payload::TimeoutVote(_) => Some(CommandClass::Progress),
        wire::ConsensusMessageV2Payload::Proposal(_) | wire::ConsensusMessageV2Payload::Vote(_) => {
            Some(CommandClass::Normal)
        }
        wire::ConsensusMessageV2Payload::PayloadManifest(_)
        | wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
        | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
        | wire::ConsensusMessageV2Payload::VrfCommit(_)
        | wire::ConsensusMessageV2Payload::VrfReveal(_) => None,
    }
}

fn classify_reducer_network_ingress(
    fail_closed: bool,
    payload: &wire::ConsensusMessageV2Payload,
) -> Result<CommandClass, NetworkIngressError> {
    if fail_closed {
        return Err(NetworkIngressError::FailClosed);
    }
    network_command_class(payload).ok_or(NetworkIngressError::TransportPayload)
}

#[cfg(test)]
fn network_admission_class(payload: &wire::ConsensusMessageV2Payload) -> Option<CommandClass> {
    match payload {
        // The transport wrapper is authenticated against an outstanding
        // request, then unwrapped into the embedded CommitQC and admitted to
        // the same Progress prefix before discovery state is retired.
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(_) => {
            Some(CommandClass::Progress)
        }
        _ => network_command_class(payload),
    }
}
