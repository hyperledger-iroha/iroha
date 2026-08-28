include!("v2_worker/pending_kura_apply_io_snapshot.rs");

/// Read-only ownership census emitted only when the outer lifecycle runner
/// has stopped reaching a non-empty fair-ingress queue.
///
/// This deliberately reports the private command FIFO and its shared
/// admission counter together. Neither is represented by ordinary effect or
/// completion status, so a pre-ledger lifecycle capacity wait would otherwise
/// be indistinguishable from a stopped scheduler.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(in crate::sumeragi) struct LifecycleIoSchedulerSnapshotV1 {
    queued_admissions: usize,
    capacity_generation: u64,
    capacity_generation_exhausted: bool,
    auxiliary_limit: usize,
    consensus_limit: usize,
    physical_capacity: usize,
    queued_commands: usize,
    queued_certified_serves: usize,
    queued_command_kinds: LifecycleIoQueuedCommandKindsV1,
    tracked_work: usize,
    tracked_lifecycle_applies: usize,
    tracked_recovered_signs: usize,
    tracked_recovered_fetches: usize,
    tracked_validates: usize,
    tracked_certified_serves: usize,
    tracked_queued: usize,
    tracked_active: usize,
    tracked_completion_pending: usize,
    completion_owners: usize,
    completion_capacity: usize,
    completion_oldest_age: Option<Duration>,
    completion_max_service_debt: u64,
    local_completions: usize,
    held_completion: bool,
    sender_open: bool,
    receiver_open: bool,
}

#[derive(Clone, Copy, Debug, Default)]
#[allow(dead_code)]
struct LifecycleIoQueuedCommandKindsV1 {
    signs: usize,
    stores: usize,
    certified_fetch_persists: usize,
    recovered_fetch_persists: usize,
    validates: usize,
    applies: usize,
    decision_applies: usize,
    recovered_signs: usize,
    certified_serves: usize,
    candidate_loads: usize,
    retires: usize,
    shutdowns: usize,
}

impl ProductionV2Services {
    /// Whether Phase B reparked a certified-Fetch result behind the service boundary.
    #[cfg(test)]
    pub(in crate::sumeragi) fn has_reparked_certified_fetch_completion_for_test(&self) -> bool {
        matches!(
            self.held_io_completion.as_ref(),
            Some(V2IoCompletion::CertifiedFetchBodyPersisted(_))
        )
    }
    /// Reserve output after a recovered Broadcast rejoins its LedgerV1 row,
    /// retaining that durable row as crash-recovery debt.
    pub(in crate::sumeragi) fn capture_recovered_lifecycle_signed_broadcast_refanout(
        &self,
        authority: super::v2_lifecycle_coordinator::RecoveredLifecycleSignedBroadcastOutputAuthorityV1,
    ) -> Result<RecoveredLifecycleSignBroadcastOutputCaptureV1<'_>, String> {
        let (context_id, height, message, cold_proposal_output) =
            authority.consume_for_service(RecoveredLifecycleSignBroadcastOutputPermitV1::new());
        if context_id != self.context.id()
            || height != self.context.height
            || self.exact_output_handoff_owner.is_sealed()
        {
            return Err(
                "recovered signed Broadcast refanout belongs to another service cut".to_owned(),
            );
        }
        if let Some(output) = cold_proposal_output {
            self.capture_recovered_lifecycle_cold_proposal_message(message, output)
        } else {
            self.capture_recovered_lifecycle_signed_broadcast_message(message)
        }
    }
    fn capture_recovered_lifecycle_signed_broadcast_message(
        &self,
        message: wire::ConsensusMessageV2,
    ) -> Result<RecoveredLifecycleSignBroadcastOutputCaptureV1<'_>, String> {
        if !matches!(
            &message.payload,
            wire::ConsensusMessageV2Payload::Vote(_)
                | wire::ConsensusMessageV2Payload::TimeoutVote(_)
        ) || self.exact_output_handoff_owner.is_sealed()
        {
            return Err(
                "recovered Sign Broadcast is outside the exact single-child service cut".to_owned(),
            );
        }
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        let encoded = Self::preencode_v2_network_message(message)?;
        let fanout = PendingExactFanout::claimed(
            vec![encoded],
            self.remote_voters(),
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
        )?;
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "recovered Sign exact output requires restart".to_owned())?;
        let pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            drop(operation);
            drop(pending);
            return Err("recovered Sign exact output sealed during capture".to_owned());
        }
        if let Some(fanout) = fanout.as_ref() {
            let available = match pending.can_enqueue(fanout) {
                Ok(available) => available,
                Err(error) => {
                    drop(operation);
                    drop(pending);
                    return Err(error);
                }
            };
            if !available {
                drop(pending);
                operation.complete();
                return Ok(RecoveredLifecycleSignBroadcastOutputCaptureV1::Unavailable);
            }
        }
        Ok(RecoveredLifecycleSignBroadcastOutputCaptureV1::Reserved(
            RecoveredLifecycleSignBroadcastOutputReservationV1 {
                operation: Some(operation),
                pending: Some(pending),
                output: Some(RecoveredLifecycleSignBroadcastPreparedOutputV1::Single(
                    fanout,
                )),
            },
        ))
    }
    #[allow(clippy::too_many_lines)]
    fn capture_recovered_lifecycle_cold_proposal_message(
        &self,
        message: wire::ConsensusMessageV2,
        output: super::v2::RecoveredLifecycleColdProposalOutputV1,
    ) -> Result<RecoveredLifecycleSignBroadcastOutputCaptureV1<'_>, String> {
        if self.proposal_work_retired || self.exact_output_handoff_owner.is_sealed() {
            return Err(
                "cold recovered Proposal output is outside the live service cut".to_owned(),
            );
        }
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
            return Err(
                "cold recovered Proposal output lost its signed control message".to_owned(),
            );
        };
        let (payload, body_store_identity) =
            output.consume_for_service(RecoveredLifecycleProposalExactOutputPermitV1::new());
        if self.io.is_none()
            || self.local_validator != Some(proposal.proposer)
            || self.local_peer.public_key() != self.key_pair.public_key()
            || self
                .lifecycle_body_store_identity
                .as_ref()
                .is_none_or(|identity| !identity.same_instance(&body_store_identity))
            || proposal.manifest != *payload.manifest()
        {
            return Err("cold recovered Proposal output belongs to another service cut".to_owned());
        }
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        proposal
            .validate(&self.context)
            .map_err(|error| error.to_string())?;
        let (manifest, chunks) = payload.into_parts();
        manifest
            .validate(&self.context)
            .map_err(|error| error.to_string())?;
        let manifest_hash = HashOf::new(&manifest);
        let sender = proposal.proposer;
        let mut chunk_messages = Vec::with_capacity(chunks.len());
        for (index, bytes) in chunks.into_iter().enumerate() {
            let mut chunk = wire::PayloadChunk {
                manifest_hash,
                index: u32::try_from(index)
                    .map_err(|_| "cold recovered Proposal chunk index overflowed".to_owned())?,
                bytes,
                sender,
                signature: Vec::new(),
            };
            let preimage = chunk
                .signature_preimage(&self.context, &manifest)
                .map_err(|error| error.to_string())?;
            chunk.signature = Signature::try_new(self.key_pair.private_key(), &preimage)
                .map_err(|error| error.to_string())?
                .payload()
                .to_vec();
            chunk_messages.push(Self::preencode_v2_network_message(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk)),
            )?);
        }
        let peers = self.remote_voters();
        let control = PendingExactFanout::claimed(
            vec![Self::preencode_v2_network_message(message)?],
            peers.clone(),
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
        )?;
        let chunks = PendingExactFanout::claimed(
            chunk_messages,
            peers,
            ExactOutputRolloverClaim::PayloadChunks {
                scope: self.exact_output_scope(),
                manifest,
            },
        )?;
        let fanouts = control.into_iter().chain(chunks).collect::<Vec<_>>();
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "cold recovered Proposal exact output requires restart".to_owned())?;
        let pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            drop(operation);
            drop(pending);
            return Err("cold recovered Proposal output sealed during capture".to_owned());
        }
        let batch = match pending.prepare_atomic_fanout_batch(fanouts) {
            Ok(batch) => batch,
            Err(error) => {
                drop(operation);
                drop(pending);
                return Err(error);
            }
        };
        let Some(batch) = batch else {
            drop(pending);
            operation.complete();
            return Ok(RecoveredLifecycleSignBroadcastOutputCaptureV1::Unavailable);
        };
        Ok(RecoveredLifecycleSignBroadcastOutputCaptureV1::Reserved(
            RecoveredLifecycleSignBroadcastOutputReservationV1 {
                operation: Some(operation),
                pending: Some(pending),
                output: Some(RecoveredLifecycleSignBroadcastPreparedOutputV1::Proposal(
                    batch,
                )),
            },
        ))
    }
    /// Atomically reserve a recovered Proposal and all chunks under one corridor
    /// lock; capacity failure leaves every FIFO/index/fanout unchanged.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::too_many_lines)]
    pub(in crate::sumeragi) fn capture_recovered_lifecycle_proposal_exact_output(
        &self,
        authority: super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1,
    ) -> Result<RecoveredLifecycleProposalExactOutputCaptureV1<'_>, String> {
        if self.proposal_work_retired {
            return Err("recovered Proposal output is terminal after Decision".to_owned());
        }
        let (dispatch_key, tag, message, payload, body_store_identity, authority_output_guard) =
            authority.consume_for_service(RecoveredLifecycleProposalExactOutputPermitV1::new());
        let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload else {
            return Err("recovered Proposal output lost its signed control message".to_owned());
        };
        if !dispatch_key.matches_height_context(&self.context)
            || tag != self.active_tag
            || proposal.round.context_id != self.context.id()
            || proposal.round.height != self.context.height
            || proposal.round.view != tag.view()
            || self.local_validator != Some(proposal.proposer)
            || self.local_peer.public_key() != self.key_pair.public_key()
            || proposal.manifest != *payload.manifest()
            || self.io.is_none()
            || self
                .lifecycle_body_store_identity
                .as_ref()
                .is_none_or(|identity| !identity.same_instance(&body_store_identity))
            || !Arc::ptr_eq(&self.output_guard, &authority_output_guard)
            || self.exact_output_handoff_owner.is_sealed()
        {
            return Err("recovered Proposal output belongs to another service cut".to_owned());
        }
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        proposal
            .validate(&self.context)
            .map_err(|error| error.to_string())?;
        let wal_append = RecoveredLifecycleProposalPrepareWalAppendSealV1 {
            dispatch_key,
            body_store_identity: body_store_identity.clone(),
            output_guard: Arc::clone(&authority_output_guard),
            attempted: false,
        };
        let retry_authority =
            super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1::from_service_retry(
                RecoveredLifecycleProposalExactOutputPermitV1::new(),
                &self.context,
                dispatch_key,
                tag,
                message.clone(),
                payload.clone(),
                body_store_identity,
                authority_output_guard,
            )
            .ok_or_else(|| {
                "recovered Proposal output could not retain its exact retry authority".to_owned()
            })?;
        let (manifest, chunks) = payload.into_parts();
        manifest
            .validate(&self.context)
            .map_err(|error| error.to_string())?;
        let manifest_hash = HashOf::new(&manifest);
        let sender = proposal.proposer;
        let mut chunk_messages = Vec::with_capacity(chunks.len());
        for (index, bytes) in chunks.into_iter().enumerate() {
            let mut chunk = wire::PayloadChunk {
                manifest_hash,
                index: u32::try_from(index)
                    .map_err(|_| "recovered Proposal chunk index overflowed".to_owned())?,
                bytes,
                sender,
                signature: Vec::new(),
            };
            let preimage = chunk
                .signature_preimage(&self.context, &manifest)
                .map_err(|error| error.to_string())?;
            chunk.signature = Signature::try_new(self.key_pair.private_key(), &preimage)
                .map_err(|error| error.to_string())?
                .payload()
                .to_vec();
            chunk_messages.push(Self::preencode_v2_network_message(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk)),
            )?);
        }
        let peers = self.remote_voters();
        let control = PendingExactFanout::claimed(
            vec![Self::preencode_v2_network_message(message)?],
            peers.clone(),
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
        )?;
        let chunks = PendingExactFanout::claimed(
            chunk_messages,
            peers,
            ExactOutputRolloverClaim::PayloadChunks {
                scope: self.exact_output_scope(),
                manifest,
            },
        )?;
        let fanouts = control.into_iter().chain(chunks).collect::<Vec<_>>();
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "recovered Proposal exact output requires restart".to_owned())?;
        let pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            // Activate fail-stop while the corridor remains locked; releasing
            // the mutex first would leave a brief open-admission window.
            drop(operation);
            drop(pending);
            return Err("recovered Proposal exact output sealed during capture".to_owned());
        }
        let batch = match pending.prepare_atomic_fanout_batch(fanouts) {
            Ok(batch) => batch,
            Err(error) => {
                drop(operation);
                drop(pending);
                return Err(error);
            }
        };
        let Some(batch) = batch else {
            drop(pending);
            operation.complete();
            return Ok(RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(
                retry_authority,
            ));
        };
        Ok(RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(
            RecoveredLifecycleProposalExactOutputReservationV1 {
                operation: Some(operation),
                pending: Some(pending),
                batch: Some(batch),
                authority: Some(retry_authority),
                wal_append,
            },
        ))
    }
    /// Consume one carrier-derived recovered Fetch through this exact service key.
    pub(in crate::sumeragi) fn authenticate_recovered_decision_fetch_request(
        &self,
        authority: RecoveredDecisionFetchRequestAuthorityV1,
    ) -> Result<RecoveredDecisionFetchRequestOwnerV1, String> {
        if self.io.is_none() || self.lifecycle_body_store_identity.is_none() {
            return Err(
                "recovered Decision Fetch requires the launched body-store worker".to_owned(),
            );
        }
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| {
                "recovered Decision Fetch request authentication requires restart".to_owned()
            })?;
        if !authority
            .identity
            .key()
            .matches_height_context(&self.context)
            || self.local_peer.public_key() != self.key_pair.public_key()
            || authority.round.context_id != self.context.id()
            || authority.round.height != self.context.height
            || authority.tag.height() != self.context.height
            || authority.sources
                != self
                    .context
                    .roster
                    .iter()
                    .map(|entry| entry.validator.clone())
                    .collect::<Vec<_>>()
        {
            return Err(
                "recovered Decision Fetch changed its fixed production service context".to_owned(),
            );
        }
        let mut request = wire::CertifiedBodyRequest {
            round: authority.round,
            subject: authority.subject,
            certificate: authority.certificate,
            requester: self.local_peer.clone(),
            signature: Vec::new(),
        };
        request.signature =
            Signature::try_new(self.key_pair.private_key(), &request.signature_preimage())
                .map_err(|error| error.to_string())?
                .payload()
                .to_vec();
        let authenticated = authenticate_certified_body_request_with_validator_pops(
            &self.context,
            &self.validator_set_pops,
            request,
            &self.local_peer,
        )
        .map_err(|error| error.to_string())?;
        let owner = RecoveredDecisionFetchRequestOwnerV1 {
            key: authority.identity.key(),
            tag: authority.tag,
            sources: authority.sources,
            authenticated,
            response_claim: None,
        };
        operation.complete();
        Ok(owner)
    }
    fn recovered_decision_fetch_fanout(
        &self,
        owner: &RecoveredDecisionFetchRequestOwnerV1,
    ) -> Result<Option<PendingExactFanout>, String> {
        if !owner.validates_exact_executor_context(&self.context, &self.local_peer)
            || self.exact_output_handoff_owner.is_sealed()
        {
            return Err(
                "recovered Decision Fetch output belongs to another service cut".to_owned(),
            );
        }
        let message =
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::CertifiedBodyRequest(
                owner.authenticated.request().clone(),
            ));
        let encoded = Self::preencode_v2_network_message(message)?;
        let peers = owner
            .sources
            .iter()
            .filter(|peer| *peer != &self.local_peer)
            .cloned()
            .collect::<Vec<_>>();
        PendingExactFanout::claimed(
            vec![encoded],
            peers,
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
        )
    }

    /// Re-offer the exact WAL-recovered Decision Fetch request retained by the
    /// executor until an authenticated response claims it.
    pub(crate) fn retry_recovered_decision_fetch(
        &self,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
    ) -> Result<bool, String> {
        let Some(owner) = executor
            .recovered_decision_fetch_retransmission_owner()
            .map_err(|error| error.to_string())?
        else {
            return Ok(false);
        };
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let Some(fanout) = self.recovered_decision_fetch_fanout(owner)? else {
            operation.complete();
            return Ok(true);
        };
        let ownership = {
            let mut pending = self.lock_pending_exact_output()?;
            if self.exact_output_handoff_owner.is_sealed() {
                return Err(
                    "Sumeragi v2 exact output is sealed after durable finality handoff".to_owned(),
                );
            }
            let ownership = pending.enqueue(fanout)?;
            if ownership == ExactFanoutOwnership::Owned {
                let _ = self.drive_pending_exact_output(&mut pending)?;
            }
            ownership
        };
        if ownership == ExactFanoutOwnership::SourceRetained {
            iroha_logger::debug!(
                request_hash = %owner.request_hash(),
                "deferred recovered Decision Fetch retransmission to its executor owner"
            );
        }
        operation.complete();
        Ok(true)
    }

    /// Freeze output/worker cuts for one preencoded recovered-Completion plan
    /// until typed selection or explicit no-selection completion.
    pub(in crate::sumeragi) fn capture_lifecycle_completion_capacity_census(
        &self,
        probes: Vec<LifecycleCompletionCapacityProbeV1>,
    ) -> Result<LifecycleCompletionCapacityCensusV1<'_>, String> {
        let io = self
            .io
            .as_ref()
            .ok_or_else(|| "lifecycle Completion census requires the launched worker".to_owned())?;
        if probes.is_empty() || self.exact_output_handoff_owner.is_sealed() {
            return Err("lifecycle Completion census has no live service cut".to_owned());
        }
        let mut candidates = BTreeMap::new();
        let mut validate_keys = BTreeSet::new();
        let mut apply_keys = BTreeSet::new();
        let mut sign_keys = BTreeSet::new();
        let mut fetch_keys = BTreeSet::new();
        for probe in probes {
            let (ordinal, candidate) = match probe {
                LifecycleCompletionCapacityProbeV1::Validate { ordinal, key } => {
                    if key.lifecycle_ordinal() != ordinal
                        || !key.matches_height_context(&self.context)
                        || !validate_keys.insert(key)
                    {
                        return Err(
                            "lifecycle Completion census changed a Validate dispatch key"
                                .to_owned(),
                        );
                    }
                    (
                        ordinal,
                        LifecycleCompletionPreparedCapacityV1::Validate {
                            key,
                            available: false,
                        },
                    )
                }
                LifecycleCompletionCapacityProbeV1::Apply {
                    ordinal,
                    key,
                    executor_available,
                } => {
                    if key.lifecycle_ordinal() != ordinal
                        || !key.matches_height_context(&self.context)
                        || !apply_keys.insert(key)
                    {
                        return Err(
                            "lifecycle Completion census changed an Apply dispatch key".to_owned()
                        );
                    }
                    (
                        ordinal,
                        LifecycleCompletionPreparedCapacityV1::Apply {
                            key,
                            available: executor_available,
                        },
                    )
                }
                LifecycleCompletionCapacityProbeV1::Sign { ordinal, key } => {
                    if key.lifecycle_ordinal() != ordinal
                        || !key.matches_height_context(&self.context)
                        || !sign_keys.insert(key)
                    {
                        return Err(
                            "lifecycle Completion census changed a Sign dispatch key".to_owned()
                        );
                    }
                    (
                        ordinal,
                        LifecycleCompletionPreparedCapacityV1::Sign {
                            key,
                            available: false,
                        },
                    )
                }
                LifecycleCompletionCapacityProbeV1::Fetch {
                    ordinal,
                    owner,
                    executor_available,
                } => {
                    if owner.dispatch_key().lifecycle_ordinal() != ordinal
                        || !fetch_keys.insert(owner.dispatch_key())
                    {
                        return Err(
                            "lifecycle Completion census repeated a Fetch dispatch key".to_owned()
                        );
                    }
                    let fanout = self.recovered_decision_fetch_fanout(&owner)?;
                    (
                        ordinal,
                        LifecycleCompletionPreparedCapacityV1::Fetch {
                            owner,
                            fanout,
                            available: executor_available,
                        },
                    )
                }
            };
            if candidates.insert(ordinal, candidate).is_some() {
                return Err("lifecycle Completion census repeated one Ready ordinal".to_owned());
            }
        }
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "lifecycle Completion census requires restart".to_owned())?;
        let pending = self.lock_pending_exact_output()?;
        let state = io.command_tx.queue.lock();
        let mut census = LifecycleCompletionCapacityCensusV1 {
            operation: Some(operation),
            pending: Some(pending),
            queue: io.command_tx.queue.as_ref(),
            state: Some(state),
            worker_predecessor_debt: 0,
            output_predecessor_debt: 0,
            candidates,
        };
        if self.exact_output_handoff_owner.is_sealed()
            || census
                .state
                .as_ref()
                .is_none_or(|state| !state.sender_open || !state.receiver_open)
        {
            return Err("lifecycle Completion service cut closed during capture".to_owned());
        }
        census.worker_predecessor_debt = u64::try_from(
            census
                .state
                .as_ref()
                .expect("armed census retains its worker cut")
                .commands
                .len(),
        )
        .map_err(|_| "lifecycle Completion worker debt overflowed".to_owned())?;
        census.output_predecessor_debt = u64::try_from(
            census
                .pending
                .as_ref()
                .expect("armed census retains its output cut")
                .fanouts
                .len(),
        )
        .map_err(|_| "lifecycle Completion output debt overflowed".to_owned())?;
        let state = census
            .state
            .as_ref()
            .expect("armed census retains its worker cut");
        let pending = census
            .pending
            .as_ref()
            .expect("armed census retains its output cut");
        for candidate in census.candidates.values_mut() {
            match candidate {
                LifecycleCompletionPreparedCapacityV1::Validate { key, available } => {
                    if state.lifecycle_validates.contains_key(key) {
                        return Err(
                            "lifecycle Completion Validate is already worker-owned".to_owned()
                        );
                    }
                    *available = io
                        .command_tx
                        .queue
                        .lifecycle_completion_worker_capacity(state);
                }
                LifecycleCompletionPreparedCapacityV1::Apply { key, available } => {
                    if state.lifecycle_decision_applies.contains_key(key) {
                        return Err("lifecycle Completion Apply is already worker-owned".to_owned());
                    }
                    *available = *available
                        && io
                            .command_tx
                            .queue
                            .lifecycle_completion_worker_capacity(state);
                }
                LifecycleCompletionPreparedCapacityV1::Sign { key, available } => {
                    if state.recovered_lifecycle_signs.contains_key(key) {
                        return Err("lifecycle Completion Sign is already worker-owned".to_owned());
                    }
                    *available = io
                        .command_tx
                        .queue
                        .lifecycle_completion_worker_capacity(state);
                }
                LifecycleCompletionPreparedCapacityV1::Fetch {
                    fanout, available, ..
                } => {
                    *available = *available
                        && fanout
                            .as_ref()
                            .map_or(Ok(true), |fanout| pending.can_enqueue(fanout))?;
                }
            }
        }
        Ok(census)
    }

    /// Return whether this service and executor share one canonical output gate.
    pub(in crate::sumeragi) fn matches_lifecycle_executor_output_guard(
        &self,
        executor: &V2EffectExecutor<SerializedV2Runtime>,
    ) -> bool {
        executor.matches_lifecycle_output_guard(&self.output_guard)
    }
    /// Return whether one lane adapter shares this exact height and storage owner.
    pub(in crate::sumeragi) fn matches_lifecycle_lane_work(
        &self,
        lane_work: &V2LaneWorkAdapter,
    ) -> bool {
        lane_work.matches_lifecycle_dependencies(
            &self.context,
            &self.state,
            &self.kura,
            &self.output_guard,
            &self.local_peer,
            &self.exact_output_handoff_owner,
        )
    }

    /// Authenticate the applied State and durable Kura tip for no-clock recovery.
    pub(in crate::sumeragi) fn matches_installed_pending_kura_tip(
        &self,
        expected: crate::sumeragi::v2_recovery::PendingKuraApply,
    ) -> bool {
        let Ok(height) = usize::try_from(expected.height()) else {
            return false;
        };
        let Some(height) = std::num::NonZeroUsize::new(height) else {
            return false;
        };
        self.context.id() == expected.context_id()
            && self.context.height == expected.height()
            && self.state.matches_kura_instance(&self.kura)
            && self.state.committed_height() == height.get()
            && self.state.latest_block_hash_fast() == Some(expected.block_hash())
            && self.kura.get_durable_block_hash(height) == Some(expected.block_hash())
    }

    fn owns_lifecycle_decision_apply_queue(&self, queue: &Arc<V2IoCommandQueue>) -> bool {
        self.io
            .as_ref()
            .is_some_and(|io| Arc::ptr_eq(&io.command_tx.queue, queue))
    }
    /// Return whether the live worker owns the exact body-store instance
    /// transferred by the lifecycle owner.
    pub(crate) fn matches_lifecycle_body_store(
        &self,
        owner_identity: &V2BodyStoreInstanceIdentity,
    ) -> bool {
        self.io.is_some()
            && self
                .lifecycle_body_store_identity
                .as_ref()
                .is_some_and(|worker_identity| worker_identity.same_instance(owner_identity))
    }

    /// Return whether the service was launched beside this exact Serve store.
    pub(in crate::sumeragi) fn matches_lifecycle_payload_store(
        &self,
        owner_identity: &CertifiedServePayloadStoreInstanceIdentity,
    ) -> bool {
        self.io.is_some()
            && self
                .lifecycle_payload_store_identity
                .as_ref()
                .is_some_and(|service_identity| service_identity.same_instance(owner_identity))
    }

    /// Refresh the live all-row Serve-retirement cut after the irreversible
    /// output seal, bound by launch permit and both store identities.
    pub(in crate::sumeragi) fn authenticate_current_lifecycle_serve_retirement(
        &self,
        permit: ProductionLifecycleServeRetirementAuthenticationPermitV1,
        verified: &super::v2::VerifiedHeightContext,
        payload_store: &CertifiedServePayloadStoreV1,
        owner_body_store_identity: &V2BodyStoreInstanceIdentity,
    ) -> Result<
        AuthenticatedCertifiedServePayloadRecoveryCut,
        CertifiedServeRetirementAuthenticationErrorV1,
    > {
        let payload_store_identity = payload_store.instance_identity();
        let roster_position = self
            .context
            .roster
            .iter()
            .position(|entry| entry.validator == self.local_peer)
            .and_then(|position| wire::ValidatorIndex::try_from(position).ok());
        if self.context != *verified.context()
            || self.validator_set_pops != verified.proofs_of_possession()
            || self.local_peer.public_key() != self.key_pair.public_key()
            || self
                .local_validator
                .is_some_and(|validator| roster_position != Some(validator))
            || !self.matches_lifecycle_body_store(owner_body_store_identity)
            || !self.matches_lifecycle_payload_store(&payload_store_identity)
            || !self.exact_output_handoff_owner.is_sealed()
        {
            return Err(CertifiedServeRetirementAuthenticationErrorV1::ForeignServiceOwner);
        }
        payload_store.authenticate_current_for_lifecycle_retirement(
            permit,
            verified,
            &self.key_pair,
        )
    }

    /// Seal an empty fixture corridor before exercising retirement census joins.
    #[cfg(test)]
    pub(in crate::sumeragi) fn seal_empty_exact_output_for_lifecycle_retirement_test(
        &self,
    ) -> Result<(), String> {
        let pending = self
            .pending_exact_output
            .lock()
            .map_err(|_| "fixture exact-output corridor lock was poisoned".to_owned())?;
        if pending.is_pending() {
            return Err("fixture exact-output corridor still owns output".to_owned());
        }
        self.exact_output_handoff_owner
            .seal()
            .map_err(|error| error.to_string())
    }

    fn recovered_lifecycle_next_vote_body_executor_permit<R: EffectRuntime>(
        &self,
        executor: &V2EffectExecutor<R>,
    ) -> Result<RecoveredLifecycleNextVoteBodyExecutorPermitV1, String> {
        let body_store_identity = self.lifecycle_body_store_identity.as_ref().ok_or_else(|| {
            "recovered next-Vote body authentication lost its launched store".to_owned()
        })?;
        if self.io.is_none()
            || !executor.matches_recovered_lifecycle_body_service(
                &self.context,
                &self.local_peer,
                &self.output_guard,
                body_store_identity,
            )
        {
            return Err(
                "recovered next-Vote body authentication found a foreign service owner".to_owned(),
            );
        }
        Ok(RecoveredLifecycleNextVoteBodyExecutorPermitV1::new(
            self.context.clone(),
            self.local_peer.clone(),
            Arc::clone(&self.output_guard),
            body_store_identity.clone(),
        ))
    }
    /// Preview recovered Sign and authenticate its successor in one joined
    /// worker/store borrow, avoiding a second executor preview.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion_with_body<'executor>(
        &self,
        executor: &'executor mut V2EffectExecutor<SerializedV2Runtime>,
        completion: RecoveredLifecycleSignAdapterCompletionAuthorityV1,
    ) -> Result<
        (
            super::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'executor>,
            super::v2::RecoveredLifecycleNextVoteBodyAuthorityV1,
        ),
        String,
    > {
        let permit = self.recovered_lifecycle_next_vote_body_executor_permit(executor)?;
        executor
            .prepare_recovered_lifecycle_sign_completion_with_body(permit, completion)
            .map_err(|error| error.to_string())
    }
    /// Publish the completion owner only through the launch stack's move-only
    /// permit during final all-or-restart runner activation.
    #[allow(dead_code)]
    pub(in crate::sumeragi) fn activate_effect_completion_observer(
        &self,
        _permit: ProductionV2CompletionObserverActivationPermitV1,
    ) -> Result<(), String> {
        let activation_guard = Arc::clone(&self.output_guard);
        let activation = activation_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| {
                "Sumeragi v2 completion observer activation requires process restart".to_owned()
            })?;
        let io = self
            .io
            .as_ref()
            .ok_or_else(|| "Sumeragi v2 completion observer lost its live worker".to_owned())?;
        super::status::set_v2_effect_completion_observer(
            self.context.id(),
            self.context.height,
            &io.admission,
        );
        activation.complete();
        Ok(())
    }
    /// Reserve a selected lifecycle target after proving live worker/output,
    /// retaining queue, admission, and fail-stop operation in one borrow.
    #[allow(clippy::result_large_err)]
    pub(crate) fn capture_lifecycle_capacity_rank<'a>(
        &'a self,
        mut prepared: PreparedLifecycleIngressSelector,
    ) -> Result<LifecycleIoCapacityCapture<'a>, LifecycleIoCapacityCaptureError> {
        let Some(io) = self.io.as_ref() else {
            return Err(LifecycleIoCapacityCaptureError {
                failure: LifecycleIoCapacityCaptureFailure::Disconnected,
                prepared,
            });
        };
        let target = match prepared.take_lifecycle_io_target() {
            Ok(target) => target,
            Err(_) => {
                return Err(LifecycleIoCapacityCaptureError {
                    failure: LifecycleIoCapacityCaptureFailure::InvalidTarget,
                    prepared,
                });
            }
        };
        let target_context = target.context();
        if target_context.height() != self.context.height
            || target_context.id().as_bytes() != self.context.id().0.as_ref()
        {
            prepared
                .restore_lifecycle_io_target(target)
                .expect("the just-consumed selector target must restore exactly");
            return Err(LifecycleIoCapacityCaptureError {
                failure: LifecycleIoCapacityCaptureFailure::ForeignContext,
                prepared,
            });
        }
        let Some(operation) = self.output_guard.begin_fail_stop_operation() else {
            prepared
                .restore_lifecycle_io_target(target)
                .expect("the output-rejected selector target must restore exactly");
            return Err(LifecycleIoCapacityCaptureError {
                failure: LifecycleIoCapacityCaptureFailure::OutputClosed,
                prepared,
            });
        };
        match io.command_tx.queue.capture_lifecycle_capacity(
            operation,
            Arc::clone(&self.output_guard),
            target,
        ) {
            Ok(V2IoLifecycleCapacityCapture::Reserved(reservation)) => {
                Ok(LifecycleIoCapacityCapture {
                    outcome: LifecycleIoCapacityOutcome::Reserved {
                        reservation,
                        prepared,
                    },
                })
            }
            Ok(V2IoLifecycleCapacityCapture::Unavailable(wait)) => Ok(LifecycleIoCapacityCapture {
                outcome: LifecycleIoCapacityOutcome::Unavailable { wait, prepared },
            }),
            Err((failure, target)) => {
                prepared
                    .restore_lifecycle_io_target(target)
                    .expect("the rejected selector target must restore exactly");
                Err(LifecycleIoCapacityCaptureError { failure, prepared })
            }
        }
    }
    /// Reserve the Consensus lane for one exact lifecycle-owned recovered Sign.
    ///
    /// This happens before coordinator claim. The locked reservation accepts
    /// only a borrow-bound registry projection with the same class-sensitive
    /// key and releases all capacity automatically on every pre-commit error.
    pub(in crate::sumeragi) fn capture_recovered_lifecycle_sign_capacity<'a>(
        &'a self,
        key: RecoveredLifecycleSignDispatchKeyV1,
    ) -> Result<
        RecoveredLifecycleSignCapacityCaptureV1<'a>,
        RecoveredLifecycleSignCapacityCaptureErrorV1,
    > {
        if !key.matches_height_context(&self.context) {
            return Err(RecoveredLifecycleSignCapacityCaptureErrorV1::ForeignContext);
        }
        let io = self
            .io
            .as_ref()
            .ok_or(RecoveredLifecycleSignCapacityCaptureErrorV1::Disconnected)?;
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or(RecoveredLifecycleSignCapacityCaptureErrorV1::OutputClosed)?;
        io.command_tx
            .queue
            .capture_recovered_lifecycle_sign_capacity(operation, key)
    }
    /// Start the ordered I/O adapter for one immutable height context.
    #[allow(clippy::too_many_arguments, dead_code)]
    pub(crate) fn start(
        context: wire::HeightContext,
        initial_tag: EventTag,
        durable_decided_subject: Option<wire::BlockSubject>,
        validator_set_pops: Vec<Vec<u8>>,
        local_peer: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        key_pair: KeyPair,
        network: IrohaNetwork,
        chunk_root: impl AsRef<Path>,
        body_store: V2BodyStore,
        state: Arc<crate::state::State>,
        queue: Arc<crate::queue::Queue>,
        kura: Arc<crate::kura::Kura>,
        provider_ingest_finalized_archive: Option<
            Arc<crate::query::provider_ingest_finalized::ProviderIngestFinalizedArchiveV1>,
        >,
        reputation_finalized_archive: Option<
            Arc<crate::query::reputation_finalized::ReputationFinalizedArchive>,
        >,
        block_cadence: Duration,
        genesis_account: iroha_data_model::account::AccountId,
        events_sender: EventsSender,
        consensus_io_capacity: usize,
        auxiliary_io_capacity: usize,
        orphan_chunk_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
        leader_wire_ingress: Arc<FairV2Ingress>,
        kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
        leader_wire_recovery_authority:
            super::serviced_candidate_store::LeaderWireRecoveryAuthority,
        exact_output_handoff_owner: DurableExactOutputServiceOwner,
    ) -> Result<Self, String> {
        let apply_service = V2ApplyService::new(
            Arc::clone(&state),
            queue,
            Arc::clone(&kura),
            provider_ingest_finalized_archive,
            reputation_finalized_archive,
            block_cadence,
            genesis_account,
            events_sender,
            validator_set_pops.clone(),
        );
        Self::start_inner(
            context,
            initial_tag,
            durable_decided_subject,
            validator_set_pops,
            local_peer,
            local_validator,
            key_pair,
            network,
            chunk_root,
            body_store,
            None,
            state,
            kura,
            apply_service,
            consensus_io_capacity,
            auxiliary_io_capacity,
            orphan_chunk_capacity,
            output_guard,
            leader_wire_ingress,
            kura_replica_advert_refresh,
            leader_wire_recovery_authority,
            exact_output_handoff_owner,
        )
    }
    /// Start with the replay application service, validating State, Kura,
    /// network identity, and roster before directories or workers exist.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::sumeragi) fn start_with_apply_service(
        _permit: super::v2_lifecycle_coordinator::ProductionLifecycleApplyServiceLaunchPermitV1,
        context: wire::HeightContext,
        initial_tag: EventTag,
        durable_decided_subject: Option<wire::BlockSubject>,
        validator_set_pops: Vec<Vec<u8>>,
        local_peer: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        key_pair: KeyPair,
        network: IrohaNetwork,
        chunk_root: impl AsRef<Path>,
        body_store: V2BodyStore,
        payload_store_identity: CertifiedServePayloadStoreInstanceIdentity,
        state: Arc<crate::state::State>,
        kura: Arc<crate::kura::Kura>,
        apply_service: V2ApplyService,
        consensus_io_capacity: usize,
        auxiliary_io_capacity: usize,
        orphan_chunk_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
        leader_wire_ingress: Arc<FairV2Ingress>,
        kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
        leader_wire_recovery_authority:
            super::serviced_candidate_store::LeaderWireRecoveryAuthority,
        exact_output_handoff_owner: DurableExactOutputServiceOwner,
    ) -> Result<Self, String> {
        if !state.matches_kura_instance(&kura)
            || !apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops)
        {
            return Err(
                "Sumeragi v2 lifecycle Decision Apply service changed lifecycle identity"
                    .to_owned(),
            );
        }
        Self::start_inner(
            context,
            initial_tag,
            durable_decided_subject,
            validator_set_pops,
            local_peer,
            local_validator,
            key_pair,
            network,
            chunk_root,
            body_store,
            Some(payload_store_identity),
            state,
            kura,
            apply_service,
            consensus_io_capacity,
            auxiliary_io_capacity,
            orphan_chunk_capacity,
            output_guard,
            leader_wire_ingress,
            kura_replica_advert_refresh,
            leader_wire_recovery_authority,
            exact_output_handoff_owner,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn start_inner(
        context: wire::HeightContext,
        initial_tag: EventTag,
        _durable_decided_subject: Option<wire::BlockSubject>,
        validator_set_pops: Vec<Vec<u8>>,
        local_peer: PeerId,
        local_validator: Option<wire::ValidatorIndex>,
        key_pair: KeyPair,
        network: IrohaNetwork,
        chunk_root: impl AsRef<Path>,
        body_store: V2BodyStore,
        lifecycle_payload_store_identity: Option<CertifiedServePayloadStoreInstanceIdentity>,
        state: Arc<crate::state::State>,
        kura: Arc<crate::kura::Kura>,
        apply_service: V2ApplyService,
        consensus_io_capacity: usize,
        auxiliary_io_capacity: usize,
        orphan_chunk_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
        leader_wire_ingress: Arc<FairV2Ingress>,
        kura_replica_advert_refresh: Arc<KuraReplicaAdvertRefreshOwner>,
        leader_wire_recovery_authority:
            super::serviced_candidate_store::LeaderWireRecoveryAuthority,
        exact_output_handoff_owner: DurableExactOutputServiceOwner,
    ) -> Result<Self, String> {
        let construction_guard = Arc::clone(&output_guard);
        let construction = construction_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if consensus_io_capacity == 0 || auxiliary_io_capacity == 0 || orphan_chunk_capacity == 0 {
            return Err("Sumeragi v2 service queue capacities must be non-zero".to_owned());
        }
        if initial_tag.height() != context.height {
            return Err(
                "Sumeragi v2 service tag is outside its immutable height context".to_owned(),
            );
        }
        let context_chunk_root = chunk_root
            .as_ref()
            .join(hex::encode(context.id().0.as_ref()));
        let max_orphan_chunk_bytes = maximum_orphan_chunk_bytes(context.da_layout);
        let max_messages_per_fanout = usize::try_from(context.da_layout.max_chunk_count)
            .map_err(|_| "Sumeragi v2 outbound chunk count is not representable".to_owned())?
            .checked_add(1)
            .ok_or_else(|| "Sumeragi v2 outbound fanout message bound overflowed".to_owned())?;
        let reply_route_source_capacity = network.reply_route_source_capacity().max(1);
        let max_peers_per_fanout = context.roster.len().max(reply_route_source_capacity).max(1);
        // Serve lifecycle storage has a frozen roster partition plus the
        // existing bounded authenticated reply-source partition. Each source
        // may own at most the already-configured auxiliary capacity; no new
        // environment or wire limit is introduced.
        // Capacity is charged per outstanding ordinary target/class occurrence.
        // Async producers and one reducer macro-step bound the shared unit pool;
        // frozen validator target/classes plus one topology-progress unit and one
        // fanout-level responder-control unit per frozen target are checked-added
        // separately. A responder control's exact authenticated routes remain
        // independently source-FIFO-indexed and bounded by the protocol fanout,
        // but cannot borrow shared capacity merely because one replay reached
        // several return paths. Only the configured authenticated-source count
        // can form an entirely non-frozen ordinary fanout, so require that
        // source-sized fanout to fit without charging the frozen roster twice.
        let shared_pending_ownership_unit_capacity =
            sumeragi_v2_exact_output_shared_ownership_capacity(
                consensus_io_capacity,
                auxiliary_io_capacity,
            )
            .map_err(|error| error.to_string())?;
        validate_shared_ownership_geometry(
            shared_pending_ownership_unit_capacity,
            reply_route_source_capacity,
        )?;
        let frozen_semantic_targets = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let pending_exact_output = PendingExactOutput::new(
            shared_pending_ownership_unit_capacity,
            max_messages_per_fanout,
            max_peers_per_fanout,
            &frozen_semantic_targets,
        )?;
        std::fs::create_dir_all(&context_chunk_root).map_err(|error| error.to_string())?;
        let durable_history = Arc::clone(&kura);
        let evidence_state = Arc::clone(&state);
        let certified_serve_validator_set_pops = validator_set_pops.clone();
        let lifecycle_body_store_identity = body_store.instance_identity();
        let io = V2IoHandle::spawn(
            body_store,
            apply_service,
            context.clone(),
            key_pair.clone(),
            local_validator,
            auxiliary_io_capacity,
            consensus_io_capacity,
            reply_route_source_capacity,
            Arc::clone(&output_guard),
        )?;
        let mut service = Self {
            context,
            validator_set_pops: certified_serve_validator_set_pops,
            state: evidence_state,
            local_peer,
            local_validator,
            key_pair,
            network,
            archive_peer_cursor: AtomicUsize::new(0),
            kura: durable_history,
            chunk_root: context_chunk_root,
            io: Some(io),
            lifecycle_body_store_identity: Some(lifecycle_body_store_identity),
            lifecycle_payload_store_identity,
            fetches: BTreeMap::new(),
            fetch_by_manifest: BTreeMap::new(),
            orphan_chunks: BTreeMap::new(),
            orphan_chunk_count: 0,
            orphan_chunk_bytes: 0,
            orphan_lifecycle_sweep_cursor: None,
            max_orphan_chunks: orphan_chunk_capacity,
            max_orphan_chunk_bytes,
            max_merge_sidecar_deferrals: consensus_io_capacity,
            local_completions: VecDeque::new(),
            held_io_completion: None,
            next_completion_source: CompletionSource::Io,
            locked_candidate_acquisition: None,
            next_locked_candidate_acquisition_id: 0,
            proposal_work_retired: false,
            prepared_candidates: VecDeque::new(),
            merge_sidecar_deferrals: VecDeque::new(),
            outbound_chunks: BTreeMap::new(),
            fast_path_proposals: BTreeSet::new(),
            pending_exact_output: Mutex::new(pending_exact_output),
            kura_replica_advert_refresh,
            exact_output_handoff_owner,
            #[cfg(test)]
            exact_output_admission_hook: None,
            #[cfg(test)]
            consensus_broadcasts: Vec::new(),
            active_tag: initial_tag,
            last_status: None,
            fatal_reason: None,
            output_guard,
            leader_wire_ingress,
            leader_wire_recovery_authority,
            // The enclosing construction operation owns abnormal-exit
            // activation until its permit is released. This avoids a nested
            // activation deadlock if `service` unwinds before construction is
            // explicitly completed.
            clean_teardown: true,
        };
        construction.complete();
        service.clean_teardown = false;
        Ok(service)
    }
    /// Sign and retain all canonical chunks for proposal and retransmission.
    pub(crate) fn register_outbound_payload(
        &mut self,
        owner: EventTag,
        payload: EncodedV2Payload,
    ) -> Result<wire::PayloadManifest, String> {
        if self.proposal_work_retired {
            return Err("Sumeragi v2 proposal work is terminal after Decision".to_owned());
        }
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard.begin_fail_stop_operation().ok_or_else(|| {
            "Sumeragi v2 canonical persistence requires restart recovery".to_owned()
        })?;
        let sender = self
            .local_validator
            .ok_or_else(|| "observer cannot disperse a Sumeragi v2 proposal".to_owned())?;
        let (manifest, chunks) = payload.into_parts();
        manifest
            .validate(&self.context)
            .map_err(|error| error.to_string())?;
        let expected_round = wire::ConsensusRound {
            context_id: self.context.id(),
            height: self.context.height,
            view: owner.view(),
        };
        if owner != self.active_tag || manifest.round != expected_round {
            return Err(
                "Sumeragi v2 outbound payload is not owned by the active reducer incarnation"
                    .to_owned(),
            );
        }
        let manifest_hash = HashOf::new(&manifest);
        let mut messages = Vec::with_capacity(chunks.len());
        for (index, bytes) in chunks.into_iter().enumerate() {
            let mut chunk = wire::PayloadChunk {
                manifest_hash,
                index: u32::try_from(index)
                    .map_err(|_| "Sumeragi v2 chunk index overflow".to_owned())?,
                bytes,
                sender,
                signature: Vec::new(),
            };
            let preimage = chunk
                .signature_preimage(&self.context, &manifest)
                .map_err(|error| error.to_string())?;
            chunk.signature = Signature::try_new(self.key_pair.private_key(), &preimage)
                .map_err(|error| error.to_string())?
                .payload()
                .to_vec();
            messages.push(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::PayloadChunk(chunk),
            ));
        }
        let retained = RetainedOutboundPayload {
            owner,
            round: manifest.round,
            subject: manifest.subject,
            messages,
        };
        if let Some(existing) = self.outbound_chunks.get(&manifest_hash) {
            if existing != &retained {
                return Err("conflicting local Sumeragi v2 payload manifest".to_owned());
            }
            self.outbound_chunks
                .retain(|hash, _| *hash == manifest_hash);
        } else {
            // There is one local proposal intent for an exact reducer owner.
            // A deterministic fallback or a higher same-tag lock supersedes
            // its old chunks before the replacement can enter signing.
            self.outbound_chunks.clear();
            self.outbound_chunks.insert(manifest_hash, retained);
        }
        operation.complete();
        Ok(manifest)
    }
    fn restore_outbound_payload_after_signature(
        &mut self,
        disposition: CompletionDisposition,
        payload: Option<EncodedV2Payload>,
    ) -> Result<(), String> {
        match disposition {
            CompletionDisposition::Accepted => {
                if let Some(payload) = payload {
                    self.register_outbound_payload(self.active_tag, payload)?;
                }
                Ok(())
            }
            CompletionDisposition::Stale => Ok(()),
            CompletionDisposition::Deferred | CompletionDisposition::Rejected => Err(
                "Sumeragi v2 signature completion returned a non-signature disposition".to_owned(),
            ),
        }
    }
    /// Work identifier waiting for a chunk from one manifest.
    pub(crate) fn fetch_work_for_manifest(
        &self,
        manifest_hash: HashOf<wire::PayloadManifest>,
    ) -> Option<EffectWorkId> {
        self.fetch_by_manifest.get(&manifest_hash).copied()
    }
    fn body_fetch_service_owner(
        &self,
        work_id: EffectWorkId,
    ) -> Result<BodyFetchServiceOwner, String> {
        let mut queued_index = None;
        for (index, completion) in self.local_completions.iter().enumerate() {
            if matches!(
                completion,
                LocalCompletion::Reconstructed {
                    task,
                    ..
                } if task.id() == work_id
            ) && queued_index.replace(index).is_some()
            {
                return Err(format!(
                    "Sumeragi v2 body-fetch work {} has duplicate queued reconstruction owners",
                    work_id.get()
                ));
            }
        }
        let live = self.fetches.get(&work_id);
        if live.is_some() && queued_index.is_some() {
            return Err(format!(
                "Sumeragi v2 body-fetch work {} has conflicting service owners",
                work_id.get()
            ));
        }
        let indexed_manifests = self
            .fetch_by_manifest
            .iter()
            .filter_map(|(manifest, owner)| (*owner == work_id).then_some(*manifest))
            .collect::<Vec<_>>();
        if let Some(fetch) = live {
            match (fetch.task.manifest(), fetch.chunks.as_ref()) {
                (Some(manifest), Some(session)) => {
                    let expected_hash = HashOf::new(manifest);
                    if session.manifest() != manifest
                        || indexed_manifests.len() != 1
                        || indexed_manifests.first() != Some(&expected_hash)
                        || self.fetch_by_manifest.get(&expected_hash) != Some(&work_id)
                    {
                        return Err(format!(
                            "Sumeragi v2 body-fetch work {} has a mismatched manifest owner",
                            work_id.get()
                        ));
                    }
                }
                (None, None) if indexed_manifests.is_empty() => {}
                _ => {
                    return Err(format!(
                        "Sumeragi v2 body-fetch work {} has inconsistent live acquisition state",
                        work_id.get()
                    ));
                }
            }
            return Ok(BodyFetchServiceOwner::Live);
        }
        if let Some(index) = queued_index {
            let LocalCompletion::Reconstructed { task, manifest, .. } = self
                .local_completions
                .get(index)
                .expect("queued reconstruction index came from this queue");
            if !task.matches_reconstructed_manifest(manifest)
                || !indexed_manifests.is_empty()
                || self.fetch_by_manifest.contains_key(&HashOf::new(manifest))
            {
                return Err(format!(
                    "Sumeragi v2 completed body-fetch work {} has inconsistent manifest ownership",
                    work_id.get()
                ));
            }
            return Ok(BodyFetchServiceOwner::Reconstructed(index));
        }
        if !indexed_manifests.is_empty() {
            return Err(format!(
                "Sumeragi v2 body-fetch work {} has an orphaned manifest owner",
                work_id.get()
            ));
        }
        Ok(BodyFetchServiceOwner::None)
    }
    fn plan_exact_body_fetch_owner_removal(
        &self,
        task: &BodyFetchTask,
    ) -> Result<BodyFetchServiceOwner, String> {
        let owner = self.body_fetch_service_owner(task.id())?;
        match owner {
            BodyFetchServiceOwner::Live => {
                let existing = self
                    .fetches
                    .get(&task.id())
                    .expect("live body-fetch owner was classified above");
                if existing.task != *task {
                    return Err(format!(
                        "Sumeragi v2 body-fetch work {} differs from executor ownership",
                        task.id().get()
                    ));
                }
            }
            BodyFetchServiceOwner::Reconstructed(index) => {
                let LocalCompletion::Reconstructed {
                    task: queued_task, ..
                } = self
                    .local_completions
                    .get(index)
                    .expect("queued body-fetch owner was classified above");
                if queued_task != task {
                    return Err(format!(
                        "Sumeragi v2 reconstructed work {} differs from executor ownership",
                        task.id().get()
                    ));
                }
            }
            BodyFetchServiceOwner::None => {
                return Err(format!(
                    "Sumeragi v2 body-fetch work {} has no service owner",
                    task.id().get()
                ));
            }
        }
        Ok(owner)
    }
    pub(in crate::sumeragi) fn prepare_certified_body_fetch_owner_removal(
        &mut self,
        task: &BodyFetchTask,
    ) -> Result<PreparedCertifiedBodyFetchOwnerRemoval<'_>, String> {
        let Some(request_hash) = task.certified_request().map(HashOf::new) else {
            return Err(format!(
                "Sumeragi v2 body-fetch work {} completed without certified authority",
                task.id().get()
            ));
        };
        let owner = self.plan_exact_body_fetch_owner_removal(task)?;
        let request_cancellation = {
            let pending = self.lock_pending_exact_output()?;
            if self.exact_output_handoff_owner.is_sealed() {
                debug_assert!(!pending.is_pending());
                None
            } else {
                Some(pending.plan_certified_body_request_cancellation(request_hash)?)
            }
        };
        Ok(PreparedCertifiedBodyFetchOwnerRemoval {
            services: self,
            task: task.clone(),
            owner,
            request_cancellation,
        })
    }
    /// Freeze cancellation of the exact recovered Decision-Fetch request fanout.
    ///
    /// A healthy archive can answer while a sibling topology target still owns
    /// a ranked actor ticket. The durable Fetch-to-Store transaction must
    /// therefore retire that now-obsolete request output together with its
    /// executor owner, before terminal rollover waits for exact-output
    /// quiescence.
    pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_request_output_retirement(
        &mut self,
        request_hash: HashOf<wire::CertifiedBodyRequest>,
    ) -> Result<PreparedRecoveredDecisionFetchRequestOutputRetirement<'_>, String> {
        let request_cancellation = {
            let pending = self.lock_pending_exact_output()?;
            if self.exact_output_handoff_owner.is_sealed() {
                debug_assert!(!pending.is_pending());
                None
            } else {
                Some(pending.plan_certified_body_request_cancellation(request_hash)?)
            }
        };
        Ok(PreparedRecoveredDecisionFetchRequestOutputRetirement {
            services: self,
            request_cancellation,
        })
    }
    /// Clone the process output guard before an exact service-removal token
    /// exclusively borrows this service owner.
    pub(in crate::sumeragi) fn lifecycle_output_guard(&self) -> Arc<ConsensusOutputGuard> {
        Arc::clone(&self.output_guard)
    }
    fn commit_exact_body_fetch_owner_removal(
        &mut self,
        task: &BodyFetchTask,
        owner: BodyFetchServiceOwner,
    ) {
        match owner {
            BodyFetchServiceOwner::Live => {
                self.fetches
                    .remove(&task.id())
                    .expect("preflighted live body-fetch owner remains present");
                if let Some(manifest_hash) = task.manifest().map(HashOf::new) {
                    let removed = self.fetch_by_manifest.remove(&manifest_hash);
                    debug_assert_eq!(removed, Some(task.id()));
                }
            }
            BodyFetchServiceOwner::Reconstructed(index) => {
                self.local_completions
                    .remove(index)
                    .expect("preflighted queued body-fetch owner remains present");
            }
            BodyFetchServiceOwner::None => {
                unreachable!("exact body-fetch removal preflight excludes an absent owner")
            }
        }
    }
    fn remove_exact_body_fetch_owner(&mut self, task: &BodyFetchTask) -> Result<(), String> {
        let owner = self.plan_exact_body_fetch_owner_removal(task)?;
        if let Some(request_hash) = task.certified_request().map(HashOf::new) {
            let mut pending = self.lock_pending_exact_output()?;
            if self.exact_output_handoff_owner.is_sealed() {
                debug_assert!(!pending.is_pending());
            } else {
                pending.cancel_certified_body_request(request_hash)?;
            }
        }
        self.commit_exact_body_fetch_owner_removal(task, owner);
        Ok(())
    }
    /// Load a lock-constrained body by immutable subject so view rebinding adds
    /// no same-subject disk read.
    pub(crate) fn request_locked_candidate(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<(), String> {
        if self.proposal_work_retired {
            return Ok(());
        }
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if tag.height() != self.context.height
            || round.context_id != self.context.id()
            || round.height != self.context.height
            || round.view > tag.view()
        {
            return Err(
                "Sumeragi v2 locked-body request has an invalid round/tag context".to_owned(),
            );
        }
        if self.locked_candidate_acquisition.is_some() {
            let rebound = self
                .locked_candidate_acquisition
                .as_mut()
                .expect("acquisition presence checked above")
                .rebind_consumer(round, subject, tag)?;
            if matches!(
                rebound,
                LockedCandidateRebind::ConsumerAdvanced
                    | LockedCandidateRebind::ReplacementDeferred
                    | LockedCandidateRebind::ReplacementRequired
            ) {
                iroha_logger::debug!(
                    height = tag.height(),
                    view = tag.view(),
                    generation = tag.generation().get(),
                    ?subject,
                    "rebound exact locked-body acquisition to current Sumeragi v2 view"
                );
            }
            if rebound == LockedCandidateRebind::ReplacementRequired {
                let acquisition_id = self.allocate_locked_candidate_acquisition_id()?;
                self.enqueue_locked_candidate_load(acquisition_id, subject)?;
                self.locked_candidate_acquisition
                    .as_mut()
                    .expect("ready acquisition remains owned during replacement")
                    .start_replacement(acquisition_id);
            }
            operation.complete();
            return Ok(());
        }
        let acquisition_id = self.allocate_locked_candidate_acquisition_id()?;
        self.enqueue_locked_candidate_load(acquisition_id, subject)?;
        self.locked_candidate_acquisition = Some(LockedCandidateAcquisition::loading(
            acquisition_id,
            round,
            subject,
            tag,
        ));
        iroha_logger::debug!(
            height = tag.height(),
            view = tag.view(),
            generation = tag.generation().get(),
            ?subject,
            "queued exact locked-body load for Sumeragi v2 re-proposal"
        );
        operation.complete();
        Ok(())
    }
    /// Borrow the immutable height-local signer only for lifecycle-owned
    /// Certified-Serve payload admission.
    pub(in crate::sumeragi) const fn lifecycle_local_signer(&self) -> &KeyPair {
        &self.key_pair
    }
    /// Return whether this exact worker can serve the authenticated request.
    ///
    /// Non-retainers are rejected as transport traffic before lifecycle
    /// publication; they must never create a Ready row which the worker can
    /// only fail after dequeue.
    pub(in crate::sumeragi) fn lifecycle_certified_serve_is_locally_authorized(
        &self,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> bool {
        self.local_validator.is_some_and(|validator| {
            authenticated
                .request()
                .certificate
                .signers
                .binary_search(&validator)
                .is_ok()
        })
    }
    /// Reserve the auxiliary worker class for an already prelocked Serve target.
    pub(in crate::sumeragi) fn capture_lifecycle_certified_serve_capacity<'a>(
        &'a self,
        target: LifecycleIngressIoTargetSeal,
    ) -> Result<LifecycleCertifiedServeCapacityCaptureV1<'a>, LifecycleIoCapacityCaptureFailure>
    {
        let Some(io) = self.io.as_ref() else {
            return Err(LifecycleIoCapacityCaptureFailure::Disconnected);
        };
        let target_context = target.context();
        if target.kind() != LifecycleIngressIoTargetKind::CertifiedServe {
            return Err(LifecycleIoCapacityCaptureFailure::InvalidTarget);
        }
        if target_context.height() != self.context.height
            || target_context.id().as_bytes() != self.context.id().0.as_ref()
        {
            return Err(LifecycleIoCapacityCaptureFailure::ForeignContext);
        }
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or(LifecycleIoCapacityCaptureFailure::OutputClosed)?;
        match io.command_tx.queue.capture_lifecycle_capacity(
            operation,
            Arc::clone(&self.output_guard),
            target,
        ) {
            Ok(V2IoLifecycleCapacityCapture::Reserved(reservation)) => Ok(
                LifecycleCertifiedServeCapacityCaptureV1::Reserved(reservation),
            ),
            Ok(V2IoLifecycleCapacityCapture::Unavailable(wait)) => {
                Ok(LifecycleCertifiedServeCapacityCaptureV1::Unavailable(
                    LifecycleCertifiedServeCapacityWaitV1 { wait },
                ))
            }
            Err((failure, _target)) => Err(failure),
        }
    }
    fn allocate_locked_candidate_acquisition_id(
        &mut self,
    ) -> Result<LockedCandidateAcquisitionId, String> {
        let acquisition_id =
            LockedCandidateAcquisitionId(self.next_locked_candidate_acquisition_id);
        self.next_locked_candidate_acquisition_id = self
            .next_locked_candidate_acquisition_id
            .checked_add(1)
            .ok_or_else(|| "Sumeragi v2 locked-body acquisition ID overflow".to_owned())?;
        Ok(acquisition_id)
    }
    fn enqueue_locked_candidate_load(
        &self,
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    ) -> Result<(), String> {
        self.io()?.enqueue(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject,
        })
    }
    fn complete_locked_candidate_load(
        &mut self,
        loaded: LockedCandidateLoad,
    ) -> Result<Option<EventTag>, String> {
        let completion = self
            .locked_candidate_acquisition
            .as_mut()
            .ok_or_else(|| {
                "Sumeragi v2 locked-body completion has no acquisition owner".to_owned()
            })?
            .complete(loaded)?;
        self.finish_locked_candidate_completion(completion)
    }
    fn locked_candidate_load_unavailable(
        &mut self,
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    ) -> Result<Option<EventTag>, String> {
        let completion = self
            .locked_candidate_acquisition
            .as_mut()
            .ok_or_else(|| {
                "Sumeragi v2 locked-body unavailability has no acquisition owner".to_owned()
            })?
            .unavailable(acquisition_id, subject)?;
        self.finish_locked_candidate_completion(completion)
    }
    fn locked_candidate_load_failed(
        &mut self,
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
        reason: String,
    ) -> Result<Option<EventTag>, String> {
        let completion = self
            .locked_candidate_acquisition
            .as_ref()
            .ok_or_else(|| "Sumeragi v2 locked-body failure has no acquisition owner".to_owned())?
            .failed(acquisition_id, subject)
            .map_err(|classification| format!("{classification}: {reason}"))?;
        self.finish_locked_candidate_completion(completion)
    }
    fn finish_locked_candidate_completion(
        &mut self,
        completion: LockedCandidateCompletion,
    ) -> Result<Option<EventTag>, String> {
        match completion {
            LockedCandidateCompletion::Ready(tag) => Ok(Some(tag)),
            LockedCandidateCompletion::Stale | LockedCandidateCompletion::Waiting => Ok(None),
            LockedCandidateCompletion::ReplacementRequired => {
                let subject = self
                    .locked_candidate_acquisition
                    .as_ref()
                    .expect("superseded acquisition remains owned during replacement")
                    .subject;
                let acquisition_id = self.allocate_locked_candidate_acquisition_id()?;
                self.enqueue_locked_candidate_load(acquisition_id, subject)?;
                self.locked_candidate_acquisition
                    .as_mut()
                    .expect("superseded acquisition remains owned during replacement")
                    .start_replacement(acquisition_id);
                Ok(None)
            }
        }
    }
    /// Retry one waiting locked-body acquisition after matching bytes become durable.
    pub(crate) fn retry_locked_candidate_after_durable_body(
        &mut self,
        subject: wire::BlockSubject,
    ) -> Result<(), String> {
        let should_retry = self
            .locked_candidate_acquisition
            .as_ref()
            .is_some_and(|acquisition| {
                acquisition.subject == subject
                    && matches!(
                        &acquisition.state,
                        LockedCandidateAcquisitionState::Waiting { .. }
                    )
            });
        if !should_retry {
            return Ok(());
        }
        let acquisition_id = self.allocate_locked_candidate_acquisition_id()?;
        self.enqueue_locked_candidate_load(acquisition_id, subject)?;
        self.locked_candidate_acquisition
            .as_mut()
            .expect("waiting acquisition remains owned during durable retry")
            .start_replacement(acquisition_id);
        Ok(())
    }
    /// Take the next locked-subject body loaded by the ordered I/O worker.
    pub(crate) fn take_loaded_candidate(&mut self) -> Option<LoadedCandidateBody> {
        if self.output_guard.restart_required() {
            return None;
        }
        self.locked_candidate_acquisition
            .as_mut()
            .and_then(LockedCandidateAcquisition::take_ready)
    }
    /// Restore one exact ready delivery after transient executor capacity
    /// prevented the current consumer from scheduling its locked proposal.
    pub(crate) fn rearm_loaded_candidate_delivery(
        &mut self,
        tag: EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<(), String> {
        if self.output_guard.restart_required() {
            return Err("Sumeragi v2 consensus requires process restart".to_owned());
        }
        self.locked_candidate_acquisition
            .as_mut()
            .ok_or_else(|| {
                "Sumeragi v2 locked-body delivery rearm has no acquisition owner".to_owned()
            })?
            .rearm_ready_delivery(tag, round, subject)
    }
    /// Take the next exact validation deferral for bounded sidecar recovery.
    pub(crate) fn take_merge_sidecar_deferral(&mut self) -> Option<DeferredMergeSidecarWork> {
        if self.output_guard.restart_required() {
            return None;
        }
        self.merge_sidecar_deferrals.pop_front()
    }
    /// Put back a transiently capacity-blocked deferral without losing its
    /// exact durable validation intent.
    pub(crate) fn requeue_merge_sidecar_deferral(
        &mut self,
        deferred: DeferredMergeSidecarWork,
    ) -> Result<(), String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if let Some(existing) = self
            .merge_sidecar_deferrals
            .iter()
            .find(|existing| existing.work_id == deferred.work_id)
        {
            if existing.round == deferred.round
                && existing.subject == deferred.subject
                && existing.reference == deferred.reference
            {
                operation.complete();
                return Ok(());
            }
            // The conflicting claim was rejected before any state or output
            // changed. Let the caller classify the service error without
            // falsely turning this local validation into ambiguous output.
            operation.complete();
            return Err(
                "Sumeragi v2 work ID claimed conflicting merge-sidecar deferrals".to_owned(),
            );
        }
        if self.merge_sidecar_deferrals.len() >= self.max_merge_sidecar_deferrals {
            // Capacity backpressure leaves the retained FIFO unchanged and
            // creates no ambiguous output at this service boundary.
            operation.complete();
            return Err("Sumeragi v2 merge-sidecar deferral queue is full".to_owned());
        }
        self.merge_sidecar_deferrals.push_back(deferred);
        operation.complete();
        Ok(())
    }
    /// Take the next reducer-authorized local Prepare intent.
    pub(crate) fn take_prepared_candidate(&mut self) -> Option<PreparedCandidateBody> {
        if self.output_guard.restart_required() {
            return None;
        }
        self.prepared_candidates.pop_front()
    }
    /// Route a possibly reordered payload chunk. Chunks received before their
    /// Proposal are retained under one explicit body-sized bound and undergo
    /// full signature/hash authentication only after the proposal manifest
    /// opens an exact fetch session.
    pub(crate) fn route_payload_chunk<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
        sender: PeerId,
        chunk: wire::PayloadChunk,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> Result<PayloadChunkDisposition, String> {
        let chunk_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone()),
        ));
        if !ingress_ownership.validate_exact()
            || !ingress_ownership.matches_message(&chunk_message)
            || !ingress_ownership.matches_semantic_origin(&sender)
        {
            return Err("payload chunk carried altered fair-ingress ownership".to_owned());
        }
        let manifest_hash = chunk.manifest_hash;
        if let Some(work_id) = self.fetch_work_for_manifest(manifest_hash) {
            return self.deliver_payload_chunk(executor, work_id, sender, chunk, ingress_ownership);
        }
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if let Some(runtime) = ingress_ownership.leader_wire_runtime_receipt() {
            if self.has_exact_reconstructed_completion(manifest_hash, &ingress_ownership)? {
                self.leader_wire_ingress
                    .mark_leader_wire_volatile_terminal(runtime)?;
                return Ok(PayloadChunkDisposition::Duplicate);
            }
            match executor
                .classify_payload_chunk_lifecycle(manifest_hash, &ingress_ownership)
                .map_err(|error| error.to_string())?
            {
                PayloadChunkLifecycleDisposition::Durable(receipt) => {
                    self.leader_wire_ingress
                        .mark_leader_wire_durable_body_terminal(runtime, &receipt)?;
                    return Ok(PayloadChunkDisposition::Duplicate);
                }
                PayloadChunkLifecycleDisposition::Volatile => {
                    self.leader_wire_ingress
                        .mark_leader_wire_volatile_terminal(runtime)?;
                    return Ok(PayloadChunkDisposition::Duplicate);
                }
                PayloadChunkLifecycleDisposition::Retain => {}
            }
        }
        let terminal_ownership = ingress_ownership.clone();
        match self.buffer_orphan_payload_chunk_owned_checked(sender, chunk, ingress_ownership) {
            OrphanPayloadChunkBufferResult::Disposition(disposition) => {
                if disposition == PayloadChunkDisposition::Rejected
                    && let Some(runtime) = terminal_ownership.leader_wire_runtime_receipt()
                {
                    self.leader_wire_ingress
                        .mark_leader_wire_volatile_terminal(runtime)?;
                }
                Ok(disposition)
            }
            OrphanPayloadChunkBufferResult::ProductiveRetentionConflict => {
                Err("bounded orphan storage could not retain an exact leader-wire owner".to_owned())
            }
        }
    }
    fn has_exact_reconstructed_completion(
        &self,
        manifest_hash: HashOf<wire::PayloadManifest>,
        ingress_ownership: &FairV2IngressOwnershipEvidence,
    ) -> Result<bool, String> {
        let runtime = ingress_ownership
            .leader_wire_runtime_receipt()
            .ok_or_else(|| {
                "productive payload chunk lost its leader-wire runtime receipt".to_owned()
            })?;
        let token = runtime.token();
        if !token.matches_chunk_manifest(manifest_hash) {
            return Err(
                "reconstructed payload completion changed its leader-wire manifest".to_owned(),
            );
        }
        for completion in &self.local_completions {
            let LocalCompletion::Reconstructed { task, manifest, .. } = completion;
            if token.matches_exact_body(manifest.round, manifest.subject, HashOf::new(manifest)) {
                if !task.matches_reconstructed_manifest(manifest) {
                    return Err(
                        "queued payload reconstruction differs from its exact task".to_owned()
                    );
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn buffer_orphan_payload_chunk_owned_checked(
        &mut self,
        sender: PeerId,
        chunk: wire::PayloadChunk,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> OrphanPayloadChunkBufferResult {
        self.buffer_orphan_payload_chunk_inner(sender, chunk, Some(ingress_ownership))
    }
    #[cfg(test)]
    fn buffer_orphan_payload_chunk_owned(
        &mut self,
        sender: PeerId,
        chunk: wire::PayloadChunk,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> PayloadChunkDisposition {
        self.buffer_orphan_payload_chunk_owned_checked(sender, chunk, ingress_ownership)
            .public_disposition()
    }
    #[cfg(test)]
    fn buffer_orphan_payload_chunk(
        &mut self,
        sender: PeerId,
        chunk: wire::PayloadChunk,
    ) -> PayloadChunkDisposition {
        self.buffer_orphan_payload_chunk_inner(sender, chunk, None)
            .public_disposition()
    }
    fn buffer_orphan_payload_chunk_inner(
        &mut self,
        sender: PeerId,
        chunk: wire::PayloadChunk,
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
    ) -> OrphanPayloadChunkBufferResult {
        let manifest_hash = chunk.manifest_hash;
        let productive_owner = ingress_ownership
            .as_ref()
            .is_some_and(|ownership| ownership.leader_wire_runtime_receipt().is_some());
        let sender_index = usize::try_from(chunk.sender).ok();
        let sender_matches = sender_index
            .and_then(|index| self.context.roster.get(index))
            .is_some_and(|entry| entry.validator == sender);
        let chunk_len = u64::try_from(chunk.bytes.len()).unwrap_or(u64::MAX);
        let max_chunk_count =
            usize::try_from(self.context.da_layout.max_chunk_count).unwrap_or(usize::MAX);
        let index_in_range = usize::try_from(chunk.index)
            .ok()
            .is_some_and(|index| index < max_chunk_count);
        if !sender_matches
            || !index_in_range
            || chunk.bytes.is_empty()
            || chunk_len > u64::from(self.context.da_layout.chunk_size_bytes)
            || chunk.signature.is_empty()
            || chunk.signature.len() > wire::MAX_CONSENSUS_SIGNATURE_BYTES
        {
            return OrphanPayloadChunkBufferResult::Disposition(PayloadChunkDisposition::Rejected);
        }
        let mut replaced_proofless = None;
        if let Some(buffered) = self.orphan_chunks.get_mut(&manifest_hash) {
            if let Some(existing) = buffered.iter_mut().find(|existing| {
                existing.sender == sender
                    && existing.chunk.index == chunk.index
                    && existing.chunk == chunk
            }) {
                let incumbent_productive = existing
                    .ingress_ownership
                    .as_ref()
                    .is_some_and(|ownership| ownership.leader_wire_runtime_receipt().is_some());
                if productive_owner && !incumbent_productive {
                    let Some(candidate) = ingress_ownership else {
                        return OrphanPayloadChunkBufferResult::ProductiveRetentionConflict;
                    };
                    // Proposal processing has now bound the same physical
                    // bytes to their immutable leader-wire lifecycle. Promote
                    // that exact carrier in place: count/byte geometry stays
                    // unchanged, and proofless eviction can no longer discard
                    // the canonical runtime owner.
                    existing.ingress_ownership = Some(candidate);
                    return OrphanPayloadChunkBufferResult::Disposition(
                        PayloadChunkDisposition::Duplicate,
                    );
                }
                match (&mut existing.ingress_ownership, ingress_ownership) {
                    (Some(retained), Some(candidate)) => {
                        if !retained.merge_downstream(candidate) {
                            return OrphanPayloadChunkBufferResult::Disposition(
                                PayloadChunkDisposition::Rejected,
                            );
                        }
                    }
                    (None, None) if cfg!(test) => {}
                    (Some(_), None) | (None, Some(_)) | (None, None) => {
                        return OrphanPayloadChunkBufferResult::Disposition(
                            PayloadChunkDisposition::Rejected,
                        );
                    }
                }
                return OrphanPayloadChunkBufferResult::Disposition(
                    PayloadChunkDisposition::Duplicate,
                );
            }
            // Retain at most one claim per authenticated outer sender/index. A
            // productive, manifest-bound owner replaces a proofless reordered
            // claim in the same slot. Otherwise the conflict cannot be
            // resolved until an existing productive owner retires.
            if let Some(position) = buffered.iter().position(|existing| {
                existing.sender == sender && existing.chunk.index == chunk.index
            }) {
                let incumbent_productive = buffered[position]
                    .ingress_ownership
                    .as_ref()
                    .is_some_and(|ownership| ownership.leader_wire_runtime_receipt().is_some());
                if productive_owner && !incumbent_productive {
                    replaced_proofless = buffered.remove(position);
                } else {
                    return if productive_owner {
                        OrphanPayloadChunkBufferResult::ProductiveRetentionConflict
                    } else {
                        OrphanPayloadChunkBufferResult::Disposition(
                            PayloadChunkDisposition::Rejected,
                        )
                    };
                }
            }
        }
        if let Some(replaced) = replaced_proofless {
            let replaced_bytes = u64::try_from(replaced.chunk.bytes.len()).unwrap_or(u64::MAX);
            self.orphan_chunk_count = self.orphan_chunk_count.saturating_sub(1);
            self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_sub(replaced_bytes);
            if self
                .orphan_chunks
                .get(&manifest_hash)
                .is_some_and(VecDeque::is_empty)
            {
                self.orphan_chunks.remove(&manifest_hash);
            }
        }
        while productive_owner
            && (self.orphan_chunk_count >= self.max_orphan_chunks
                || self.orphan_chunk_bytes.saturating_add(chunk_len) > self.max_orphan_chunk_bytes)
        {
            if !self.evict_one_proofless_orphan_chunk() {
                return OrphanPayloadChunkBufferResult::ProductiveRetentionConflict;
            }
        }
        if self.orphan_chunk_count >= self.max_orphan_chunks
            || self.orphan_chunk_bytes.saturating_add(chunk_len) > self.max_orphan_chunk_bytes
        {
            return OrphanPayloadChunkBufferResult::Disposition(PayloadChunkDisposition::Rejected);
        }
        let buffered = self.orphan_chunks.entry(manifest_hash).or_default();
        buffered.push_back(BufferedPayloadChunk {
            sender,
            chunk,
            ingress_ownership,
        });
        self.orphan_chunk_count = self.orphan_chunk_count.saturating_add(1);
        self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_add(chunk_len);
        OrphanPayloadChunkBufferResult::Disposition(PayloadChunkDisposition::Buffered)
    }
    fn evict_one_proofless_orphan_chunk(&mut self) -> bool {
        let selected = self
            .orphan_chunks
            .iter()
            .find_map(|(manifest_hash, chunks)| {
                chunks
                    .iter()
                    .position(|buffered| {
                        buffered.ingress_ownership.as_ref().is_none_or(|ownership| {
                            ownership.leader_wire_runtime_receipt().is_none()
                        })
                    })
                    .map(|position| (*manifest_hash, position))
            });
        let Some((manifest_hash, position)) = selected else {
            return false;
        };
        let (removed, remove_manifest) = {
            let chunks = self
                .orphan_chunks
                .get_mut(&manifest_hash)
                .expect("selected orphan manifest remains present");
            let removed = chunks
                .remove(position)
                .expect("selected proofless orphan remains present");
            (removed, chunks.is_empty())
        };
        if remove_manifest {
            self.orphan_chunks.remove(&manifest_hash);
        }
        let removed_bytes = u64::try_from(removed.chunk.bytes.len()).unwrap_or(u64::MAX);
        self.orphan_chunk_count = self.orphan_chunk_count.saturating_sub(1);
        self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_sub(removed_bytes);
        true
    }
    fn next_orphan_payload_lifecycle_sweep_position(
        &self,
    ) -> Option<OrphanPayloadLifecycleSweepCursor> {
        let first = || {
            self.orphan_chunks
                .iter()
                .find(|(_, chunks)| !chunks.is_empty())
                .map(|(manifest_hash, _)| OrphanPayloadLifecycleSweepCursor {
                    manifest_hash: *manifest_hash,
                    chunk_offset: 0,
                })
        };
        let Some(cursor) = self.orphan_lifecycle_sweep_cursor else {
            return first();
        };
        if self
            .orphan_chunks
            .get(&cursor.manifest_hash)
            .is_some_and(|chunks| cursor.chunk_offset < chunks.len())
        {
            return Some(cursor);
        }
        self.orphan_chunks
            .range((
                std::ops::Bound::Excluded(cursor.manifest_hash),
                std::ops::Bound::Unbounded,
            ))
            .find(|(_, chunks)| !chunks.is_empty())
            .map(|(manifest_hash, _)| OrphanPayloadLifecycleSweepCursor {
                manifest_hash: *manifest_hash,
                chunk_offset: 0,
            })
            .or_else(first)
    }
    fn terminalize_buffered_payload_chunk_if_complete<R: EffectRuntime>(
        &self,
        executor: &V2EffectExecutor<R>,
        manifest_hash: HashOf<wire::PayloadManifest>,
        buffered: &BufferedPayloadChunk,
    ) -> Result<bool, String> {
        let Some(ingress_ownership) = buffered.ingress_ownership.as_ref() else {
            return Ok(false);
        };
        let Some(runtime) = ingress_ownership.leader_wire_runtime_receipt() else {
            return Ok(false);
        };
        let disposition =
            match self.has_exact_reconstructed_completion(manifest_hash, ingress_ownership) {
                Ok(true) => PayloadChunkLifecycleDisposition::Volatile,
                Ok(false) => executor
                    .classify_payload_chunk_lifecycle(manifest_hash, ingress_ownership)
                    .map_err(|error| error.to_string())?,
                Err(error) => return Err(error),
            };
        match disposition {
            PayloadChunkLifecycleDisposition::Durable(receipt) => self
                .leader_wire_ingress
                .mark_leader_wire_durable_body_terminal(runtime, &receipt)?,
            PayloadChunkLifecycleDisposition::Volatile => self
                .leader_wire_ingress
                .mark_leader_wire_volatile_terminal(runtime)?,
            PayloadChunkLifecycleDisposition::Retain => return Ok(false),
        }
        Ok(true)
    }
    fn sweep_buffered_payload_chunk_lifecycles<R: EffectRuntime>(
        &mut self,
        executor: &V2EffectExecutor<R>,
    ) -> Result<usize, String> {
        let mut retired = 0usize;
        let mut first_error = None;
        let visits = self
            .orphan_chunk_count
            .min(MAX_ORPHAN_LIFECYCLE_VISITS_PER_REPLAY);
        for _ in 0..visits {
            let Some(cursor) = self.next_orphan_payload_lifecycle_sweep_position() else {
                self.orphan_lifecycle_sweep_cursor = None;
                break;
            };
            let classification = {
                let buffered = self
                    .orphan_chunks
                    .get(&cursor.manifest_hash)
                    .and_then(|chunks| chunks.get(cursor.chunk_offset))
                    .expect("orphan lifecycle cursor resolves an existing buffered chunk");
                self.terminalize_buffered_payload_chunk_if_complete(
                    executor,
                    cursor.manifest_hash,
                    buffered,
                )
            };
            match classification {
                Ok(true) => {
                    let (removed, remove_manifest) = {
                        let chunks = self
                            .orphan_chunks
                            .get_mut(&cursor.manifest_hash)
                            .expect("classified orphan manifest remains present");
                        let removed = chunks
                            .remove(cursor.chunk_offset)
                            .expect("classified orphan chunk remains present");
                        (removed, chunks.is_empty())
                    };
                    if remove_manifest {
                        self.orphan_chunks.remove(&cursor.manifest_hash);
                    }
                    let bytes = u64::try_from(removed.chunk.bytes.len()).unwrap_or(u64::MAX);
                    self.orphan_chunk_count = self.orphan_chunk_count.saturating_sub(1);
                    self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_sub(bytes);
                    retired = retired.saturating_add(1);
                    self.orphan_lifecycle_sweep_cursor = Some(cursor);
                }
                Ok(false) => {
                    self.orphan_lifecycle_sweep_cursor = Some(OrphanPayloadLifecycleSweepCursor {
                        manifest_hash: cursor.manifest_hash,
                        chunk_offset: cursor.chunk_offset.saturating_add(1),
                    });
                }
                Err(error) => {
                    self.orphan_lifecycle_sweep_cursor = Some(OrphanPayloadLifecycleSweepCursor {
                        manifest_hash: cursor.manifest_hash,
                        chunk_offset: cursor.chunk_offset.saturating_add(1),
                    });
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }
        first_error.map_or(Ok(retired), Err)
    }
    /// Replay all chunks whose proposal manifests have now opened sessions.
    pub(crate) fn replay_buffered_chunks<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
    ) -> Result<usize, String> {
        if self.output_guard.restart_required() {
            return Err("Sumeragi v2 consensus requires process restart".to_owned());
        }
        self.sweep_buffered_payload_chunk_lifecycles(executor)?;
        let ready = self
            .orphan_chunks
            .keys()
            .filter_map(|hash| {
                self.fetch_work_for_manifest(*hash)
                    .map(|work_id| (*hash, work_id))
            })
            .collect::<Vec<_>>();
        let mut delivered = 0usize;
        for (manifest_hash, work_id) in ready {
            let Some(mut chunks) = self.orphan_chunks.remove(&manifest_hash) else {
                continue;
            };
            while let Some(buffered) = chunks.pop_front() {
                let bytes = u64::try_from(buffered.chunk.bytes.len()).unwrap_or(u64::MAX);
                self.orphan_chunk_count = self.orphan_chunk_count.saturating_sub(1);
                self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_sub(bytes);
                if self.fetch_work_for_manifest(manifest_hash) != Some(work_id) {
                    if let Some(runtime) = buffered
                        .ingress_ownership
                        .as_ref()
                        .and_then(FairV2IngressOwnershipEvidence::leader_wire_runtime_receipt)
                        && let Err(error) = self
                            .leader_wire_ingress
                            .mark_leader_wire_volatile_terminal(runtime)
                    {
                        if let Err(tail_error) = self.retire_buffered_payload_chunk_tail(chunks) {
                            return Err(format!(
                                "{error}; additionally failed to retire buffered payload tail: {tail_error}"
                            ));
                        }
                        return Err(error);
                    }
                    continue;
                }
                let Some(ingress_ownership) = buffered.ingress_ownership else {
                    let tail_result = self.retire_buffered_payload_chunk_tail(chunks);
                    return Err(tail_result.err().unwrap_or_else(|| {
                        "buffered payload chunk lost fair-ingress ownership".to_owned()
                    }));
                };
                match self.deliver_payload_chunk(
                    executor,
                    work_id,
                    buffered.sender,
                    buffered.chunk,
                    ingress_ownership,
                ) {
                    Ok(PayloadChunkDisposition::Delivered) => {
                        delivered = delivered.saturating_add(1);
                    }
                    Ok(_) => {}
                    Err(error) => {
                        if let Err(tail_error) = self.retire_buffered_payload_chunk_tail(chunks) {
                            return Err(format!(
                                "{error}; additionally failed to retire buffered payload tail: {tail_error}"
                            ));
                        }
                        return Err(error);
                    }
                }
            }
        }
        Ok(delivered)
    }
    fn retire_buffered_payload_chunk_tail(
        &mut self,
        mut chunks: VecDeque<BufferedPayloadChunk>,
    ) -> Result<(), String> {
        let mut first_error = None;
        while let Some(buffered) = chunks.pop_front() {
            let bytes = u64::try_from(buffered.chunk.bytes.len()).unwrap_or(u64::MAX);
            self.orphan_chunk_count = self.orphan_chunk_count.saturating_sub(1);
            self.orphan_chunk_bytes = self.orphan_chunk_bytes.saturating_sub(bytes);
            let Some(runtime) = buffered
                .ingress_ownership
                .as_ref()
                .and_then(FairV2IngressOwnershipEvidence::leader_wire_runtime_receipt)
            else {
                continue;
            };
            if let Err(error) = self
                .leader_wire_ingress
                .mark_leader_wire_volatile_terminal(runtime)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }
    fn take_io_completion(&mut self, runtime_capacity_available: bool) -> IoCompletionTake {
        if self.held_io_completion.as_ref().is_some_and(|completion| {
            matches!(
                completion,
                V2IoCompletion::LifecycleDecisionApply(_)
                    | V2IoCompletion::RecoveredLifecycleSign(_)
                    | V2IoCompletion::RecoveredDecisionFetchBodyPersisted(_)
                    | V2IoCompletion::LifecycleValidate(_)
                    | V2IoCompletion::LifecycleCertifiedServe(_)
            )
        }) {
            return IoCompletionTake::retained_runtime();
        }
        if runtime_capacity_available && let Some(completion) = self.held_io_completion.take() {
            return IoCompletionTake::ready(PendingServiceCompletion::Io {
                completion,
                ownership_position: 0,
            });
        }
        let ownership_position =
            usize::from(!runtime_capacity_available && self.held_io_completion.is_some());
        let Some(io) = self.io.as_ref() else {
            return IoCompletionTake::unavailable();
        };
        if ownership_position != 0
            && io
                .completion_ownership_at(ownership_position)
                .is_some_and(|owned| {
                    owned.lifecycle_decision_apply.is_some()
                        || owned.recovered_lifecycle_sign.is_some()
                        || owned.recovered_decision_fetch.is_some()
                        || owned.lifecycle_validate.is_some()
                        || owned.lifecycle_certified_serve.is_some()
                })
        {
            // There is only one payload parking slot. A lifecycle-owned
            // completion behind an already-held runtime result must remain in
            // the physical channel until that result is serviced; receiving it
            // here would detach the payload from its keyed owner or overwrite
            // the held result.
            return IoCompletionTake::retained_runtime();
        }
        // Once the oldest runtime-producing result has crossed the physical
        // channel boundary, keep exactly that one result unacknowledged. The
        // ownership tracker lets us look past it only when the next published
        // result is known not to require a reducer-completion slot.
        if !runtime_capacity_available
            && ownership_position != 0
            && io.completion_requires_runtime_capacity_at(ownership_position) != Some(false)
        {
            return IoCompletionTake::unavailable();
        }
        let Ok(completion) = io.try_recv_completion_unacknowledged() else {
            return IoCompletionTake::unavailable();
        };
        if matches!(
            &completion,
            V2IoCompletion::LifecycleDecisionApply(_)
                | V2IoCompletion::RecoveredLifecycleSign(_)
                | V2IoCompletion::RecoveredDecisionFetchBodyPersisted(_)
                | V2IoCompletion::LifecycleValidate(_)
                | V2IoCompletion::LifecycleCertifiedServe(_)
        ) {
            assert!(
                self.held_io_completion.is_none(),
                "completion ownership metadata must preserve one recovered lifecycle head"
            );
            self.held_io_completion = Some(completion);
            return IoCompletionTake::retained_runtime();
        }
        if !runtime_capacity_available && completion.requires_runtime_capacity() {
            assert!(
                self.held_io_completion.is_none(),
                "completion ownership metadata must prevent a second held runtime result"
            );
            self.held_io_completion = Some(completion);
            return IoCompletionTake::retained_runtime();
        }
        IoCompletionTake::ready(PendingServiceCompletion::Io {
            completion,
            ownership_position,
        })
    }
    fn take_recovered_lifecycle_sign_completion(&mut self) -> IoCompletionTake {
        if let Some(completion) = self.held_io_completion.take() {
            if matches!(&completion, V2IoCompletion::RecoveredLifecycleSign(_)) {
                return IoCompletionTake::ready(PendingServiceCompletion::Io {
                    completion,
                    ownership_position: 0,
                });
            }
            self.held_io_completion = Some(completion);
            return IoCompletionTake::unavailable();
        }
        let Some(io) = self.io.as_ref() else {
            return IoCompletionTake::unavailable();
        };
        let Ok(completion) = io.try_recv_completion_unacknowledged() else {
            return IoCompletionTake::unavailable();
        };
        if matches!(&completion, V2IoCompletion::RecoveredLifecycleSign(_)) {
            IoCompletionTake::ready(PendingServiceCompletion::Io {
                completion,
                ownership_position: 0,
            })
        } else {
            self.held_io_completion = Some(completion);
            IoCompletionTake::unavailable()
        }
    }
    fn take_lifecycle_certified_serve_completion(&mut self) -> IoCompletionTake {
        if let Some(completion) = self.held_io_completion.take() {
            if matches!(&completion, V2IoCompletion::LifecycleCertifiedServe(_)) {
                return IoCompletionTake::ready(PendingServiceCompletion::Io {
                    completion,
                    ownership_position: 0,
                });
            }
            self.held_io_completion = Some(completion);
            return IoCompletionTake::unavailable();
        }
        let Some(io) = self.io.as_ref() else {
            return IoCompletionTake::unavailable();
        };
        let Ok(completion) = io.try_recv_completion_unacknowledged() else {
            return IoCompletionTake::unavailable();
        };
        if matches!(&completion, V2IoCompletion::LifecycleCertifiedServe(_)) {
            IoCompletionTake::ready(PendingServiceCompletion::Io {
                completion,
                ownership_position: 0,
            })
        } else {
            self.held_io_completion = Some(completion);
            IoCompletionTake::unavailable()
        }
    }
    fn take_next_completion(&mut self, runtime_capacity_available: bool) -> IoCompletionTake {
        let completion = if runtime_capacity_available && self.held_io_completion.is_some() {
            // Once capacity returns, the exact runtime result which first
            // encountered backpressure precedes both later I/O and the local
            // reconstruction source.
            self.take_io_completion(true)
        } else {
            match self.next_completion_source {
                CompletionSource::Io => match self.take_io_completion(runtime_capacity_available) {
                    IoCompletionTake {
                        completion: None,
                        retained_runtime: false,
                    } if runtime_capacity_available => self
                        .local_completions
                        .front()
                        .cloned()
                        .map_or_else(IoCompletionTake::unavailable, |completion| {
                            IoCompletionTake::ready(PendingServiceCompletion::Local(completion))
                        }),
                    completion => completion,
                },
                CompletionSource::Local if runtime_capacity_available => {
                    self.local_completions.front().cloned().map_or_else(
                        || self.take_io_completion(true),
                        |completion| {
                            IoCompletionTake::ready(PendingServiceCompletion::Local(completion))
                        },
                    )
                }
                CompletionSource::Local => self.take_io_completion(false),
            }
        };
        if let Some(completion) = &completion.completion {
            self.next_completion_source = match completion {
                PendingServiceCompletion::Io { .. } => CompletionSource::Local,
                PendingServiceCompletion::Local(_) => CompletionSource::Io,
            };
        }
        completion
    }
    fn retire_held_io_completion(&mut self) {
        let Some(completion) = self.held_io_completion.take() else {
            return;
        };
        if matches!(
            &completion,
            V2IoCompletion::RecoveredLifecycleSign(_)
                | V2IoCompletion::LifecycleValidate(_)
                | V2IoCompletion::LifecycleCertifiedServe(_)
        ) {
            // Dropping the armed completion closes output while `self.io`
            // still retains the dedicated queue/index owner. It must never be
            // acknowledged or removed by generic teardown.
            return;
        }
        if let Some(io) = self.io.as_ref() {
            io.acknowledge_completion(&completion)
                .expect("completion acknowledgement is infallible");
        }
    }
    /// Drain tagged I/O/reconstruction completions while runtime has capacity;
    /// backpressured responses transfer to exact output or remain reconstructible.
    pub(crate) fn drain_completions<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
    ) -> Result<usize, EffectExecutorError> {
        let outcome = self.drain_completions_with_lifecycle(executor)?;
        self.require_no_unowned_lifecycle_completion(executor, outcome)
    }
    /// Drain exactly the physical head already classified as an ordinary
    /// lifecycle pass-through.
    ///
    /// The one-item bound prevents this ordinary owner from crossing into a
    /// lifecycle completion which arrived immediately behind it. The next
    /// outer Completion turn must classify that lifecycle owner itself.
    pub(in crate::sumeragi) fn drain_one_ordinary_completion_after_lifecycle_pass_through<
        R: EffectRuntime,
    >(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
    ) -> Result<usize, EffectExecutorError> {
        let outcome = self.drain_completions_inner(executor, 1)?;
        self.require_no_unowned_lifecycle_completion(executor, outcome)
    }

    /// Return whether the inert ordinary Completion test head remains retained.
    #[cfg(test)]
    pub(in crate::sumeragi) fn has_auxiliary_completion_head_for_test(&self) -> bool {
        matches!(
            self.held_io_completion.as_ref(),
            Some(V2IoCompletion::AuxiliaryNoop)
        )
    }
    /// Prepare one ordinary Completion head while a same-address Validate successor waits.
    ///
    /// The waiting successor remains the logical owner. This method only
    /// restores an ordinary I/O head into the existing held slot (or observes
    /// the already-selected local source), so the caller can use the normal
    /// one-item pass-through drain. Dedicated lifecycle completions are never
    /// transferred, acknowledged, or exposed while that successor is parked.
    pub(in crate::sumeragi) fn prepare_ordinary_completion_behind_validate_fence(
        &mut self,
    ) -> Result<bool, String> {
        if self.output_guard.restart_required() {
            return Err("Sumeragi v2 consensus requires process restart".to_owned());
        }
        if self.held_io_completion.is_none()
            && self.next_completion_source == CompletionSource::Local
            && !self.local_completions.is_empty()
        {
            return Ok(true);
        }
        if let Some(completion) = self.held_io_completion.as_ref() {
            return Ok(!completion.is_dedicated_lifecycle_completion());
        }
        let Some(io) = self.io.as_ref() else {
            return Ok(!self.local_completions.is_empty());
        };
        let completion = match io.try_recv_completion_unacknowledged() {
            Ok(completion) => completion,
            Err(_) => return Ok(!self.local_completions.is_empty()),
        };
        let ordinary = !completion.is_dedicated_lifecycle_completion();
        assert!(
            self.held_io_completion.is_none(),
            "fence pass-through classification retains at most one I/O head"
        );
        self.held_io_completion = Some(completion);
        Ok(ordinary)
    }
    /// Take and classify the oldest Completion-lane owner in one operation.
    ///
    /// This is the lifecycle driver's sole ownership-transferring physical-head
    /// classifier. It does not probe three mutually exclusive drains. A pending
    /// local completion, or an ordinary I/O head, returns `PassThrough` without
    /// acknowledgement or ownership-position removal. A recovered result
    /// transfers exactly its dedicated guarded token and advances
    /// completion-source rotation once.
    pub(in crate::sumeragi) fn take_next_lifecycle_completion(
        &mut self,
    ) -> Result<LifecycleCompletionTakeV1, String> {
        if self.output_guard.restart_required() {
            return Err("Sumeragi v2 consensus requires process restart".to_owned());
        }
        if self.held_io_completion.is_none()
            && self.next_completion_source == CompletionSource::Local
            && !self.local_completions.is_empty()
        {
            return Ok(LifecycleCompletionTakeV1::PassThrough);
        }

        let completion = if let Some(completion) = self.held_io_completion.take() {
            completion
        } else {
            let Some(io) = self.io.as_ref() else {
                return Ok(LifecycleCompletionTakeV1::None);
            };
            let Ok(completion) = io.try_recv_completion_unacknowledged() else {
                return if self.local_completions.is_empty() {
                    Ok(LifecycleCompletionTakeV1::None)
                } else {
                    Ok(LifecycleCompletionTakeV1::PassThrough)
                };
            };
            completion
        };

        match completion {
            V2IoCompletion::CertifiedFetchBodyPersisted(guarded) => {
                let work_ack = match self.io.as_ref().ok_or_else(|| {
                    "persisted certified-Fetch body lost its I/O command owner".to_owned()
                }) {
                    Ok(io) => match io.prepare_certified_fetch_body_persistence_ack(
                        guarded.completion(),
                        Arc::clone(&self.output_guard),
                    ) {
                        Ok(work_ack) => work_ack,
                        Err(error) => {
                            self.held_io_completion =
                                Some(V2IoCompletion::CertifiedFetchBodyPersisted(guarded));
                            return Err(error);
                        }
                    },
                    Err(error) => {
                        self.held_io_completion =
                            Some(V2IoCompletion::CertifiedFetchBodyPersisted(guarded));
                        return Err(error);
                    }
                };
                let Some(io) = self.io.as_ref() else {
                    unreachable!("prepared certified-Fetch work ack retains its I/O owner")
                };
                if let Err(error) = io.acknowledge_completion_at(
                    V2IoCompletionAcknowledgement::LifecycleWorkRetained,
                    0,
                ) {
                    self.held_io_completion =
                        Some(V2IoCompletion::CertifiedFetchBodyPersisted(guarded));
                    return Err(error);
                }
                self.next_completion_source = CompletionSource::Local;
                Ok(LifecycleCompletionTakeV1::CertifiedFetch(
                    PreparedCertifiedFetchBodyPersistenceCompletion {
                        completion: guarded.into_completion(),
                        work_ack,
                    },
                ))
            }
            V2IoCompletion::LifecycleDecisionApply(guarded) => {
                let key = guarded.result().dispatch_key();
                let work_ack = match self.io.as_ref().ok_or_else(|| {
                    "lifecycle Decision Apply completion lost its I/O service owner".to_owned()
                }) {
                    Ok(io) => match io
                        .prepare_lifecycle_decision_apply_ack(key, Arc::clone(&self.output_guard))
                    {
                        Ok(work_ack) => work_ack,
                        Err(error) => {
                            self.held_io_completion =
                                Some(V2IoCompletion::LifecycleDecisionApply(guarded));
                            return Err(error);
                        }
                    },
                    Err(error) => {
                        self.held_io_completion =
                            Some(V2IoCompletion::LifecycleDecisionApply(guarded));
                        return Err(error);
                    }
                };
                self.next_completion_source = CompletionSource::Local;
                Ok(LifecycleCompletionTakeV1::Apply(
                    PreparedLifecycleDecisionApplyCompletionV1 { guarded, work_ack },
                ))
            }
            V2IoCompletion::RecoveredLifecycleSign(guarded) => {
                let completion = self
                    .io
                    .as_ref()
                    .and_then(|io| io.prepare_recovered_lifecycle_sign_completion(guarded, 0))
                    .ok_or_else(|| {
                        "recovered Sign completion lost its exact dedicated owner".to_owned()
                    })?;
                self.next_completion_source = CompletionSource::Local;
                Ok(LifecycleCompletionTakeV1::Sign(completion))
            }
            V2IoCompletion::RecoveredDecisionFetchBodyPersisted(guarded) => {
                let completion = self
                    .io
                    .as_ref()
                    .and_then(|io| io.prepare_recovered_decision_fetch_body_completion(guarded, 0))
                    .ok_or_else(|| {
                        "recovered Decision Fetch body completion lost its exact dedicated owner"
                            .to_owned()
                    })?;
                self.next_completion_source = CompletionSource::Local;
                Ok(LifecycleCompletionTakeV1::DecisionFetch(completion))
            }
            V2IoCompletion::LifecycleValidate(guarded) => {
                let completion = self
                    .io
                    .as_ref()
                    .and_then(|io| io.prepare_lifecycle_validate_completion(guarded, 0))
                    .ok_or_else(|| {
                        "lifecycle Validate completion lost its exact dedicated owner".to_owned()
                    })?;
                self.next_completion_source = CompletionSource::Local;
                Ok(LifecycleCompletionTakeV1::Validate(completion))
            }
            V2IoCompletion::LifecycleCertifiedServe(guarded) => {
                let completion = self
                    .io
                    .as_ref()
                    .and_then(|io| io.prepare_lifecycle_certified_serve_completion(guarded, 0))
                    .ok_or_else(|| {
                        "lifecycle Certified-Serve completion lost its exact dedicated owner"
                            .to_owned()
                    })?;
                self.next_completion_source = CompletionSource::Local;
                Ok(LifecycleCompletionTakeV1::CertifiedServe(completion))
            }
            ordinary => {
                assert!(
                    self.held_io_completion.is_none(),
                    "ordinary pass-through must restore the sole held completion slot"
                );
                self.held_io_completion = Some(ordinary);
                Ok(LifecycleCompletionTakeV1::PassThrough)
            }
        }
    }

    /// Drain only the oldest recovered-Sign guard; other heads remain parked and
    /// generic drains cannot acknowledge this completion.
    pub(in crate::sumeragi) fn drain_recovered_lifecycle_sign_completion(
        &mut self,
    ) -> Result<RecoveredLifecycleSignCompletionDrainV1, String> {
        if self.output_guard.restart_required() {
            return Err("Sumeragi v2 consensus requires process restart".to_owned());
        }
        let take = self.take_recovered_lifecycle_sign_completion();
        let Some(PendingServiceCompletion::Io {
            completion: V2IoCompletion::RecoveredLifecycleSign(guarded),
            ownership_position,
        }) = take.completion
        else {
            return Ok(RecoveredLifecycleSignCompletionDrainV1 { completion: None });
        };
        let completion = self
            .io
            .as_ref()
            .and_then(|io| {
                io.prepare_recovered_lifecycle_sign_completion(guarded, ownership_position)
            })
            .ok_or_else(|| "recovered Sign completion lost its exact dedicated owner".to_owned())?;
        Ok(RecoveredLifecycleSignCompletionDrainV1 {
            completion: Some(completion),
        })
    }
    /// Drain the oldest lifecycle Serve; restore every other head unchanged.
    pub(in crate::sumeragi) fn drain_lifecycle_certified_serve_completion(
        &mut self,
    ) -> Result<LifecycleCertifiedServeCompletionDrainV1, String> {
        if self.output_guard.restart_required() {
            return Err("Sumeragi v2 consensus requires process restart".to_owned());
        }
        let take = self.take_lifecycle_certified_serve_completion();
        let Some(PendingServiceCompletion::Io {
            completion: V2IoCompletion::LifecycleCertifiedServe(guarded),
            ownership_position,
        }) = take.completion
        else {
            return Ok(LifecycleCertifiedServeCompletionDrainV1 { completion: None });
        };
        let completion = self
            .io
            .as_ref()
            .and_then(|io| {
                io.prepare_lifecycle_certified_serve_completion(guarded, ownership_position)
            })
            .ok_or_else(|| {
                "lifecycle Certified-Serve completion lost its exact dedicated owner".to_owned()
            })?;
        Ok(LifecycleCertifiedServeCompletionDrainV1 {
            completion: Some(completion),
        })
    }
    /// Drain the ordinary bounded completion source while returning a
    /// persisted certified-Fetch body directly to its serialized owner.
    ///
    /// The lifecycle driver consumes the typed certified-Fetch outcome through
    /// its Phase-B LedgerV1 publication before an ordinary drain can continue.
    pub(crate) fn drain_completions_with_lifecycle<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
    ) -> Result<V2CompletionDrainOutcome, EffectExecutorError> {
        self.drain_completions_inner(executor, MAX_COMPLETION_DRAIN_BATCH)
    }
    fn drain_completions_inner<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
        limit: usize,
    ) -> Result<V2CompletionDrainOutcome, EffectExecutorError> {
        if self.output_guard.restart_required() {
            return Err(executor
                .external_service_failed("Sumeragi v2 consensus requires process restart", self));
        }
        let mut count = 0usize;
        let mut attempts = 0usize;
        let mut worker_completion_deferred = false;
        let mut local_completion_deferred = false;
        let mut certified_fetch_body = None;
        while attempts < limit {
            let runtime_capacity_available = executor.remaining_completion_capacity() != 0;
            let take = self.take_next_completion(runtime_capacity_available);
            let completion = match take.completion {
                Some(completion) => completion,
                None if take.retained_runtime => {
                    attempts = attempts.saturating_add(1);
                    if !worker_completion_deferred {
                        worker_completion_deferred = self
                            .io
                            .as_ref()
                            .is_some_and(|io| io.record_completion_service_attempt(0));
                    }
                    continue;
                }
                None => {
                    if !runtime_capacity_available
                        && !worker_completion_deferred
                        && (self.held_io_completion.is_some()
                            || self.io.as_ref().is_some_and(|io| {
                                io.completion_requires_runtime_capacity_at(0) == Some(true)
                            }))
                    {
                        worker_completion_deferred = self
                            .io
                            .as_ref()
                            .is_some_and(|io| io.record_completion_service_attempt(0));
                    }
                    break;
                }
            };
            attempts = attempts.saturating_add(1);
            let io_acknowledgement = match &completion {
                PendingServiceCompletion::Io {
                    completion,
                    ownership_position,
                } => Some((completion.acknowledgement(), *ownership_position)),
                PendingServiceCompletion::Local(_) => None,
            };
            let mut certified_fetch_work_ack = match &completion {
                PendingServiceCompletion::Io {
                    completion: V2IoCompletion::CertifiedFetchBodyPersisted(completion),
                    ..
                } => {
                    let prepared = self.io.as_ref().map_or_else(
                        || {
                            Err("persisted certified-Fetch body lost its I/O command owner"
                                .to_owned())
                        },
                        |io| {
                            io.prepare_certified_fetch_body_persistence_ack(
                                completion.completion(),
                                Arc::clone(&self.output_guard),
                            )
                        },
                    );
                    match prepared {
                        Ok(prepared) => Some(prepared),
                        Err(reason) => {
                            return Err(executor.external_service_failed(reason, self));
                        }
                    }
                }
                PendingServiceCompletion::Io { .. } | PendingServiceCompletion::Local(_) => None,
            };
            let serviced: Result<(), EffectExecutorError> = (|| {
                match completion {
                    PendingServiceCompletion::Io {
                        completion:
                            V2IoCompletion::Signature {
                                work_id,
                                signature,
                                outbound_payload,
                            },
                        ..
                    } => {
                        let disposition =
                            executor.complete_consensus_signature(work_id, signature, self)?;
                        if let Err(reason) = self
                            .restore_outbound_payload_after_signature(disposition, outbound_payload)
                        {
                            return Err(executor.external_service_failed(reason, self));
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::Stored(completion),
                        ..
                    } => {
                        let stored_subject = completion.manifest().subject;
                        let _ = executor.complete_body_store(completion, self)?;
                        if let Err(reason) =
                            self.retry_locked_candidate_after_durable_body(stored_subject)
                        {
                            return Err(executor.external_service_failed(reason, self));
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::CertifiedFetchBodyPersisted(completion),
                        ..
                    } => {
                        assert!(
                            certified_fetch_body.is_none(),
                            "one bounded drain turn returns at most one lifecycle completion"
                        );
                        certified_fetch_body =
                            Some(PreparedCertifiedFetchBodyPersistenceCompletion {
                                completion: completion.into_completion(),
                                work_ack: certified_fetch_work_ack.take().expect(
                                    "persisted Fetch completion retains its exact work ack",
                                ),
                            });
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::Applied(completion),
                        ..
                    } => {
                        let source_height = completion.artifact().height;
                        let source_block_hash = completion.artifact().block_hash;
                        let disposition = executor.complete_application(*completion, self)?;
                        if disposition == CompletionDisposition::Accepted {
                            self.kura_replica_advert_refresh
                                .note_durable_tip(
                                    Some((source_height, source_block_hash)),
                                    Instant::now(),
                                )
                                .map_err(|reason| executor.external_service_failed(reason, self))?;
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::ApplyDeferred { work_id, reference },
                        ..
                    } => {
                        let _ = executor
                            .defer_application_for_merge_sidecar(work_id, &reference, self)?;
                    }
                    #[cfg(test)]
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::AuxiliaryNoop,
                        ..
                    } => {}
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::CandidateLoaded(candidate),
                        ..
                    } => {
                        if !self.proposal_work_retired {
                            let subject = candidate.subject;
                            let tag = match self.complete_locked_candidate_load(candidate) {
                                Ok(tag) => tag,
                                Err(reason) => {
                                    return Err(executor.external_service_failed(reason, self));
                                }
                            };
                            if let Some(tag) = tag {
                                iroha_logger::debug!(
                                    height = tag.height(),
                                    view = tag.view(),
                                    generation = tag.generation().get(),
                                    ?subject,
                                    "loaded exact locked body for Sumeragi v2 re-proposal"
                                );
                            } else {
                                iroha_logger::debug!(
                                    ?subject,
                                    "retired superseded locked-body load before Sumeragi v2 re-proposal"
                                );
                            }
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::LifecycleDecisionApply(_),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            "lifecycle Decision Apply completion crossed the generic executor drain",
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::RecoveredLifecycleSign(_),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            "recovered Sign completion crossed the generic executor drain",
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::RecoveredDecisionFetchBodyPersisted(_),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            "recovered Decision Fetch body crossed the generic executor drain",
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::LifecycleValidate(_),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            "lifecycle Validate completion crossed the generic executor drain",
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::LifecycleCertifiedServe(_),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            "lifecycle Certified-Serve completion crossed the generic executor drain",
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion:
                            V2IoCompletion::CandidateLoadUnavailable {
                                acquisition_id,
                                subject,
                            },
                        ..
                    } => {
                        if !self.proposal_work_retired {
                            if let Err(reason) =
                                self.locked_candidate_load_unavailable(acquisition_id, subject)
                            {
                                return Err(executor.external_service_failed(reason, self));
                            }
                            iroha_logger::debug!(
                                ?subject,
                                "locked Sumeragi v2 body is not durable yet; waiting for body-store recovery"
                            );
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion:
                            V2IoCompletion::CandidateLoadFailed {
                                acquisition_id,
                                subject,
                                reason,
                            },
                        ..
                    } => {
                        if !self.proposal_work_retired {
                            if let Err(reason) =
                                self.locked_candidate_load_failed(acquisition_id, subject, reason)
                            {
                                return Err(executor.external_service_failed(reason, self));
                            }
                            iroha_logger::debug!(
                                ?subject,
                                "retired failed superseded locked-body load before Sumeragi v2 re-proposal"
                            );
                        }
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::Failed(reason),
                        ..
                    } => {
                        return Err(executor.external_service_failed(reason, self));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::Retired,
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            "unexpected early Sumeragi v2 storage retirement",
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::RetirementFailed(reason),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            format!(
                                "unexpected early Sumeragi v2 storage retirement failure: {reason}"
                            ),
                            self,
                        ));
                    }
                    PendingServiceCompletion::Io {
                        completion: V2IoCompletion::RecoveryRequired(reason),
                        ..
                    } => {
                        return Err(executor.external_service_failed(
                            format!("canonical persistence requires restart recovery: {reason}"),
                            self,
                        ));
                    }
                    PendingServiceCompletion::Local(LocalCompletion::Reconstructed {
                        task,
                        manifest,
                        body,
                    }) => {
                        match executor.complete_body_reconstruction(&task, manifest, body, self) {
                            Ok(CompletionDisposition::Rejected) => {
                                iroha_logger::debug!(
                                    work_id = task.id().get(),
                                    "rejected noncanonical reconstructed Sumeragi v2 body"
                                );
                            }
                            Ok(_) => {}
                            Err(EffectTransportError::Backpressure) => {
                                local_completion_deferred = true;
                            }
                            Err(error) => {
                                return Err(executor.external_service_failed(error, self));
                            }
                        }
                    }
                }
                Ok(())
            })();
            if let Some((acknowledgement, ownership_position)) = io_acknowledgement {
                let acknowledge = match &acknowledgement {
                    V2IoCompletionAcknowledgement::RecoveredLifecycleSignRetained
                    | V2IoCompletionAcknowledgement::RecoveredDecisionFetchRetained
                    | V2IoCompletionAcknowledgement::LifecycleServeRetained
                    | V2IoCompletionAcknowledgement::LifecycleValidateRetained => false,
                    V2IoCompletionAcknowledgement::Work(_)
                    | V2IoCompletionAcknowledgement::LifecycleWorkRetained
                    | V2IoCompletionAcknowledgement::LifecycleDecisionApplyRetained
                    | V2IoCompletionAcknowledgement::Untracked => true,
                };
                if acknowledge && let Some(io) = self.io.as_ref() {
                    let acknowledged =
                        io.acknowledge_completion_at(acknowledgement, ownership_position);
                    if let Err(reason) = acknowledged {
                        return Err(executor.external_service_failed(reason, self));
                    }
                }
            }
            serviced?;
            if local_completion_deferred {
                worker_completion_deferred = true;
                break;
            }
            count = count.saturating_add(1);
            if certified_fetch_body.is_some() {
                break;
            }
        }
        if count != 0 || worker_completion_deferred {
            let status = executor.status();
            if executor.remaining_completion_capacity() == 0
                && (status.pending_signatures != 0
                    || status.pending_fetches != 0
                    || status.pending_stores != 0
                    || status.pending_validations != 0
                    || status.pending_outputs != 0
                    || status.pending_applications != 0
                    || !self.local_completions.is_empty()
                    || self.held_io_completion.is_some())
            {
                iroha_logger::debug!(
                    queued_runtime_commands = status.queued_runtime_completions,
                    pending_signatures = status.pending_signatures,
                    pending_fetches = status.pending_fetches,
                    pending_stores = status.pending_stores,
                    pending_validations = status.pending_validations,
                    pending_outputs = status.pending_outputs,
                    pending_applications = status.pending_applications,
                    local_completions = self.local_completions.len(),
                    held_io_completion = self.held_io_completion.is_some(),
                    "deferred Sumeragi v2 service completion behind a full runtime FIFO"
                );
            }
            if let Err(reason) = self.publish_effect_status(&status) {
                return Err(executor.external_service_failed(reason, self));
            }
        }
        Ok(V2CompletionDrainOutcome {
            serviced: count,
            certified_fetch_body,
        })
    }
    fn require_no_unowned_lifecycle_completion<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
        outcome: V2CompletionDrainOutcome,
    ) -> Result<usize, EffectExecutorError> {
        let (serviced, completion) = outcome.into_parts();
        if completion.is_some() {
            return Err(executor.external_service_failed(
                "persisted certified-Fetch body has no live lifecycle coordinator owner",
                self,
            ));
        }
        Ok(serviced)
    }
    /// After Kura receipt verification, hand cleanup to the bounded janitor;
    /// failures retain files for reconciliation without delaying successors.
    pub(crate) fn finish_height(
        mut self,
        receipt: KuraV2CommitReceipt,
        cleanup_timeout: Duration,
        supervisor: &mut V2CleanupSupervisor,
    ) -> PostFinalityCleanupOutcome {
        let mut outcome = PostFinalityCleanupOutcome::default();
        let incomplete_exact_output_handoff = match self.pending_exact_output.lock() {
            Ok(_) if !self.exact_output_handoff_owner.is_sealed() => {
                Some("durable exact-output handoff was not sealed before finalized cleanup")
            }
            Ok(pending) if pending.is_pending() => {
                Some("durable exact-output handoff was sealed with pending output")
            }
            Ok(_) => None,
            Err(_) => {
                Some("durable exact-output corridor lock was poisoned before finalized cleanup")
            }
        };
        if let Some(reason) = incomplete_exact_output_handoff {
            outcome.record(PostFinalityCleanupTarget::CleanupWorker, reason);
            self.output_guard.activate_restart_required();
        } else {
            self.clean_teardown = true;
        }
        let deadline = Instant::now()
            .checked_add(cleanup_timeout)
            .unwrap_or_else(Instant::now);
        self.retire_held_io_completion();
        if let Some(mut io) = self.io.take() {
            let mut command = V2IoCommand::Retire(V2RetireCommand {
                receipt,
                cleanup: supervisor.submission(),
                chunk_root: self.chunk_root.clone(),
            });
            let retirement_guard = Arc::clone(&self.output_guard);
            'enqueue: loop {
                let Some(retirement_enqueue_permit) = retirement_guard.acquire() else {
                    outcome.record(
                        PostFinalityCleanupTarget::CleanupWorker,
                        "process restart became required before body retirement enqueue",
                    );
                    break;
                };
                let enqueue = io.try_enqueue(command);
                // Waiting for an older completion while holding this permit
                // would prevent fatal activation from draining output.
                drop(retirement_enqueue_permit);
                match enqueue {
                    Ok(()) => break,
                    Err(V2IoTrySendError::Full(returned)) => {
                        command = returned;
                        match recv_cleanup_completion(&io, deadline) {
                            Ok(V2IoCompletion::Failed(reason)) => outcome.record(
                                PostFinalityCleanupTarget::CleanupWorker,
                                format!(
                                    "pending I/O work failed while enqueueing body retirement: {reason}"
                                ),
                            ),
                            Ok(V2IoCompletion::Retired) => {
                                outcome.record(
                                    PostFinalityCleanupTarget::CleanupWorker,
                                    "I/O worker reported retirement before accepting the retirement request",
                                );
                                break 'enqueue;
                            }
                            Ok(V2IoCompletion::RetirementFailed(reason)) => {
                                outcome.record(
                                    PostFinalityCleanupTarget::CleanupWorker,
                                    "Sumeragi v2 I/O worker reported body retirement failure",
                                );
                                outcome.record(PostFinalityCleanupTarget::DurableBodies, reason);
                                break 'enqueue;
                            }
                            Ok(_) => {}
                            Err(CleanupCompletionWaitError::DeadlineElapsed) => {
                                outcome.record(
                                    PostFinalityCleanupTarget::CleanupWorker,
                                    format!(
                                        "Sumeragi v2 body retirement enqueue exceeded the configured {cleanup_timeout:?} post-finality cleanup deadline"
                                    ),
                                );
                                // Typed finality is already durable, but the
                                // full command queue prevented Retire from
                                // being enqueued before the cleanup deadline.
                                // Authorize only the ensuing normal producer
                                // disconnect, before dropping the last sender.
                                io.allow_finalized_disconnect
                                    .store(true, AtomicOrdering::Release);
                                break 'enqueue;
                            }
                            Err(CleanupCompletionWaitError::Disconnected) => {
                                outcome.record(
                                    PostFinalityCleanupTarget::CleanupWorker,
                                    "Sumeragi v2 I/O worker disconnected before body retirement",
                                );
                                break 'enqueue;
                            }
                        }
                    }
                    Err(V2IoTrySendError::Disconnected(_)) => {
                        outcome.record(
                            PostFinalityCleanupTarget::CleanupWorker,
                            "Sumeragi v2 I/O worker disconnected before body retirement",
                        );
                        break;
                    }
                    Err(
                        V2IoTrySendError::ConflictingWorkId { .. }
                        | V2IoTrySendError::UnreservedLifecycleDecisionApply { .. },
                    ) => {
                        unreachable!("retirement commands do not carry work identifiers")
                    }
                }
            }
            let join = io.join.take();
            // A successfully accepted Retire moves all blocking filesystem
            // work to the one runner-lifetime janitor before this worker
            // exits. Never join a running context worker on the consensus
            // thread; dropping its handle only detaches the already-closing
            // worker and cannot create another cleanup thread.
            drop(io);
            if let Some(join) = join {
                if join.is_finished() && join.join().is_err() {
                    outcome.record(
                        PostFinalityCleanupTarget::CleanupWorker,
                        "Sumeragi v2 I/O worker panicked during finalized cleanup",
                    );
                }
            }
        } else {
            outcome.record(
                PostFinalityCleanupTarget::CleanupWorker,
                "Sumeragi v2 I/O worker was unavailable for cleanup handoff",
            );
        }
        outcome
    }
    fn io(&self) -> Result<&V2IoHandle, String> {
        self.io
            .as_ref()
            .ok_or_else(|| "Sumeragi v2 I/O worker is unavailable".to_owned())
    }
    fn output_permit(&self) -> Result<ConsensusOutputPermit<'_>, String> {
        self.output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 canonical persistence requires restart recovery".to_owned())
    }
    fn lock_pending_exact_output(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, PendingExactOutput>, String> {
        self.pending_exact_output
            .lock()
            .map_err(|_| "Sumeragi v2 outbound corridor lock was poisoned".to_owned())
    }
    /// Replace actor admission with a deterministic recoverable test boundary.
    #[cfg(test)]
    pub(in crate::sumeragi) fn set_exact_output_admission_hook(
        &mut self,
        mut hook: impl FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
        ) -> Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>>
        + Send
        + 'static,
    ) {
        self.exact_output_admission_hook = Some(Mutex::new(Box::new(move |post, ticket| {
            hook(post, ticket).map(|()| ExactOutputTestAdmission::Admitted)
        })));
    }
    /// Replace reply admission with a controllable writer-flush test boundary.
    #[cfg(test)]
    pub(in crate::sumeragi) fn set_exact_output_flush_admission_hook(
        &mut self,
        hook: impl FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
        ) -> Result<
            ExactOutputTestAdmission,
            NetworkActorAdmissionError<Post<NetworkMessage>>,
        > + Send
        + 'static,
    ) {
        self.exact_output_admission_hook = Some(Mutex::new(Box::new(hook)));
    }
    /// Replace an empty exact-output corridor with a small production-shaped test geometry.
    #[cfg(test)]
    pub(in crate::sumeragi) fn set_exact_output_shared_unit_capacity_for_test(
        &self,
        shared_ownership_unit_capacity: usize,
    ) -> Result<(), String> {
        let max_messages_per_fanout = usize::try_from(self.context.da_layout.max_chunk_count)
            .map_err(|_| "Sumeragi v2 test outbound chunk count is not representable".to_owned())?
            .checked_add(1)
            .ok_or_else(|| "Sumeragi v2 test outbound fanout bound overflowed".to_owned())?;
        let max_peers_per_fanout = self
            .context
            .roster
            .len()
            .max(self.network.reply_route_source_capacity())
            .max(1);
        let frozen_semantic_targets = self
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let replacement = PendingExactOutput::new(
            shared_ownership_unit_capacity,
            max_messages_per_fanout,
            max_peers_per_fanout,
            &frozen_semantic_targets,
        )?;
        let mut pending = self.lock_pending_exact_output()?;
        if !pending.fanouts.is_empty() || !pending.admitted_sidecar_chunks.is_empty() {
            return Err("cannot replace a non-empty Sumeragi v2 exact-output corridor".to_owned());
        }
        *pending = replacement;
        Ok(())
    }
    /// Test whether the exact-output corridor retained a particular opaque
    /// reply tenure after a production service handoff.
    #[cfg(test)]
    pub(in crate::sumeragi) fn retains_reply_route_for_test(
        &self,
        expected: &NetworkReplyRoute,
    ) -> Result<bool, String> {
        self.lock_pending_exact_output().map(|pending| {
            pending.fanouts.iter().any(|fanout| {
                fanout.targets.iter().any(|target| {
                    matches!(
                        &target.route,
                        ExactTargetRoute::Reply(route) if route.same_tenure(expected)
                    )
                })
            })
        })
    }
    #[cfg(test)]
    /// Return whether fail-stop output handling requires a process restart.
    pub(in crate::sumeragi) fn exact_output_restart_required_for_test(&self) -> bool {
        self.output_guard.restart_required()
    }
    /// Snapshot ordinary Apply worker ownership without taking any queue item.
    #[cfg(test)]
    pub(in crate::sumeragi) fn pending_kura_apply_io_snapshot_for_test(
        &self,
    ) -> Option<PendingKuraApplyIoSnapshotV1> {
        let io = self.io.as_ref()?;
        let state = io.command_tx.queue.lock();
        let queued_commands = state
            .commands
            .iter()
            .filter(|command| matches!(command, V2IoCommand::Apply(_)))
            .count();
        let tracked = |expected| {
            state
                .work
                .values()
                .filter(|tracked| {
                    matches!(tracked.descriptor, V2IoWorkDescriptor::Apply { .. })
                        && tracked.state == expected
                })
                .count()
        };
        let snapshot = PendingKuraApplyIoSnapshotV1 {
            queued_commands,
            tracked_queued: tracked(V2IoWorkState::Queued),
            tracked_active: tracked(V2IoWorkState::Active),
            tracked_completion_pending: tracked(V2IoWorkState::CompletionPending),
            completion_owners: io.admission.completion_snapshot(Instant::now()).depth,
            local_completions: self.local_completions.len(),
            held_completion: self.held_io_completion.is_some(),
        };
        drop(state);
        Some(snapshot)
    }
    /// Snapshot the otherwise private lifecycle I/O corridor without taking a
    /// command or completion. This is used only by the rate-limited outer
    /// scheduler-starvation diagnostic.
    pub(in crate::sumeragi) fn lifecycle_io_scheduler_snapshot(
        &self,
    ) -> Option<LifecycleIoSchedulerSnapshotV1> {
        let io = self.io.as_ref()?;
        let state = io.command_tx.queue.lock();
        let mut tracked_queued = 0usize;
        let mut tracked_active = 0usize;
        let mut tracked_completion_pending = 0usize;
        for tracked_state in state
            .work
            .values()
            .map(|tracked| tracked.state)
            .chain(
                state
                    .lifecycle_decision_applies
                    .values()
                    .map(|tracked| tracked.state),
            )
            .chain(
                state
                    .recovered_lifecycle_signs
                    .values()
                    .map(|tracked| tracked.state),
            )
            .chain(
                state
                    .recovered_decision_fetch_bodies
                    .values()
                    .map(|tracked| tracked.state),
            )
            .chain(
                state
                    .lifecycle_validates
                    .values()
                    .map(|tracked| tracked.state),
            )
            .chain(state.lifecycle_serves.values().map(|tracked| tracked.state))
        {
            match tracked_state {
                V2IoWorkState::Queued => {
                    tracked_queued = tracked_queued.saturating_add(1);
                }
                V2IoWorkState::Active => {
                    tracked_active = tracked_active.saturating_add(1);
                }
                V2IoWorkState::CompletionPending => {
                    tracked_completion_pending = tracked_completion_pending.saturating_add(1);
                }
            }
        }
        let queued_admissions = io.admission.queued();
        let capacity_generation = io.admission.lifecycle_capacity_generation();
        let capacity_generation_exhausted = io.admission.lifecycle_capacity_generation_exhausted();
        let queued_commands = state.commands.len();
        let mut queued_command_kinds = LifecycleIoQueuedCommandKindsV1::default();
        for command in &state.commands {
            let count = match command {
                V2IoCommand::Sign { .. } => &mut queued_command_kinds.signs,
                V2IoCommand::Store(_) => &mut queued_command_kinds.stores,
                V2IoCommand::PersistCertifiedFetchBody(_) => {
                    &mut queued_command_kinds.certified_fetch_persists
                }
                V2IoCommand::PersistRecoveredDecisionFetchBody(_) => {
                    &mut queued_command_kinds.recovered_fetch_persists
                }
                V2IoCommand::LifecycleValidate(_) => &mut queued_command_kinds.validates,
                V2IoCommand::Apply(_) => &mut queued_command_kinds.applies,
                V2IoCommand::LifecycleDecisionApply(_) => {
                    &mut queued_command_kinds.decision_applies
                }
                V2IoCommand::RecoveredLifecycleSign(_) => &mut queued_command_kinds.recovered_signs,
                #[cfg(test)]
                V2IoCommand::LifecycleDecisionApplyFixture(_) => {
                    &mut queued_command_kinds.decision_applies
                }
                V2IoCommand::LifecycleCertifiedServe(_) => {
                    &mut queued_command_kinds.certified_serves
                }
                V2IoCommand::LoadCandidate { .. } => &mut queued_command_kinds.candidate_loads,
                V2IoCommand::Retire(_) => &mut queued_command_kinds.retires,
                V2IoCommand::Shutdown => &mut queued_command_kinds.shutdowns,
            };
            *count = count.saturating_add(1);
        }
        let queued_certified_serves = queued_command_kinds.certified_serves;
        let tracked_work = state.work.len();
        let tracked_lifecycle_applies = state.lifecycle_decision_applies.len();
        let tracked_recovered_signs = state.recovered_lifecycle_signs.len();
        let tracked_recovered_fetches = state.recovered_decision_fetch_bodies.len();
        let tracked_validates = state.lifecycle_validates.len();
        let tracked_certified_serves = state.lifecycle_serves.len();
        let sender_open = state.sender_open;
        let receiver_open = state.receiver_open;
        drop(state);
        let completion = io.admission.completion_snapshot(Instant::now());
        Some(LifecycleIoSchedulerSnapshotV1 {
            queued_admissions,
            capacity_generation,
            capacity_generation_exhausted,
            auxiliary_limit: io.admission.auxiliary_limit,
            consensus_limit: io.admission.consensus_limit,
            physical_capacity: io.admission.capacity,
            queued_commands,
            queued_certified_serves,
            queued_command_kinds,
            tracked_work,
            tracked_lifecycle_applies,
            tracked_recovered_signs,
            tracked_recovered_fetches,
            tracked_validates,
            tracked_certified_serves,
            tracked_queued,
            tracked_active,
            tracked_completion_pending,
            completion_owners: completion.depth,
            completion_capacity: completion.capacity,
            completion_oldest_age: completion.oldest_age,
            completion_max_service_debt: completion.max_service_debt,
            local_completions: self.local_completions.len(),
            held_completion: self.held_io_completion.is_some(),
            sender_open,
            receiver_open,
        })
    }
    /// Snapshot scalar exact-output ownership without exposing payloads,
    /// semantic targets, reply capabilities, or process-local source keys.
    pub(in crate::sumeragi) fn exact_output_scheduler_snapshot(
        &self,
    ) -> Option<ExactOutputSchedulerSnapshotV1> {
        let pending = self.lock_pending_exact_output().ok()?;
        Some(pending.scheduler_snapshot(self.exact_output_handoff_owner.is_sealed()))
    }
    /// Count accepted service calls carrying this exact canonical consensus envelope.
    #[cfg(test)]
    pub(in crate::sumeragi) fn consensus_broadcast_count_for_test(
        &self,
        expected: &wire::ConsensusMessageV2,
    ) -> usize {
        self.consensus_broadcasts
            .iter()
            .filter(|message| *message == expected)
            .count()
    }
    /// Count all retained exact fanouts and those carrying one exact PrepareQC.
    #[cfg(test)]
    pub(in crate::sumeragi) fn pending_exact_prepare_qc_fanouts_for_test(
        &self,
        expected: &wire::QuorumCertificate,
    ) -> Result<(usize, usize), String> {
        let expected = Self::preencode_v2_network_message(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(expected.clone()),
        ))?;
        let expected_hash = HashOf::new(&expected);
        let remote_voters = self.remote_voters();
        self.lock_pending_exact_output().map(|pending| {
            let matching = pending
                .fanouts
                .iter()
                .filter(|fanout| {
                    matches!(fanout.message_hashes.as_slice(), [hash] if *hash == expected_hash)
                        && fanout.semantic_peers() == remote_voters
                        && matches!(
                            &fanout.rollover_claim,
                            ExactOutputRolloverClaim::GlobalV2(_)
                        )
                })
                .count();
            (pending.fanouts.len(), matching)
        })
    }
    /// Hold one auxiliary I/O unit without fabricating a queue command.
    #[cfg(test)]
    pub(in crate::sumeragi) fn hold_auxiliary_io_admission_for_test(
        &self,
    ) -> Result<ProductionAuxiliaryIoAdmissionHoldV1, String> {
        let io = self
            .io
            .as_ref()
            .ok_or_else(|| "Sumeragi v2 I/O worker is unavailable".to_owned())?;
        if !io.admission.try_reserve(V2IoAdmissionClass::Auxiliary) {
            return Err("Sumeragi v2 auxiliary I/O admission is full".to_owned());
        }
        Ok(ProductionAuxiliaryIoAdmissionHoldV1 {
            admission: Arc::clone(&io.admission),
        })
    }

    fn admit_network_exact_output(
        &self,
        post: Post<NetworkMessage>,
        ticket: Option<NetworkActorAdmissionTicket>,
        route: &ExactTargetRoute,
        reply_writer_timeout_attempt: u8,
    ) -> Result<ExactOutputAttemptOutcome, NetworkActorAdmissionError<Post<NetworkMessage>>> {
        match route {
            ExactTargetRoute::Topology => self
                .network
                .post_recoverable(post, ticket)
                .map(|()| ExactOutputAttemptOutcome::Admitted),
            ExactTargetRoute::Reply(reply_route) => {
                let requires_sidecar_flush = matches!(
                    &post.data,
                    NetworkMessage::CertifiedMergeSidecar(message)
                        if matches!(message.as_ref(), CertifiedMergeSidecarMessage::Chunk(_))
                );
                match self
                    .network
                    .post_reply_recoverable_with_flush_ack_at_attempt(
                        post,
                        reply_route,
                        ticket,
                        reply_writer_timeout_attempt,
                    )? {
                    Some(flush_ack) if requires_sidecar_flush => {
                        Ok(ExactOutputAttemptOutcome::SidecarFlush(flush_ack))
                    }
                    Some(flush_ack) => Ok(ExactOutputAttemptOutcome::ReplyFlush(flush_ack)),
                    None if reply_route.is_active() && !reply_route.is_reply_writable() => {
                        Ok(ExactOutputAttemptOutcome::Unavailable)
                    }
                    None => Ok(ExactOutputAttemptOutcome::Retired),
                }
            }
        }
    }
    fn drive_pending_exact_output(&self, pending: &mut PendingExactOutput) -> Result<bool, String> {
        if pending.applied_height_finality.is_none()
            && u64::try_from(self.state.committed_height())
                .is_ok_and(|height| height >= self.context.height)
        {
            let artifact = self
                .kura
                .v2_finality_artifact(self.context.height)
                .map_err(|error| error.to_string())?;
            if let Some(artifact) = artifact {
                if artifact.height_context != self.context {
                    return Err(
                        "Sumeragi v2 committed exact-output height differs from Kura finality"
                            .to_owned(),
                    );
                }
                pending.applied_height_finality = Some(artifact);
            }
        }
        pending.poll_reply_flushes()?;
        let outcome = {
            #[cfg(test)]
            {
                if let Some(hook) = &self.exact_output_admission_hook {
                    let mut hook = hook.lock().map_err(|_| {
                        "Sumeragi v2 exact-output admission hook was poisoned".to_owned()
                    })?;
                    pending.drive_bounded_with_ack(|post, ticket, route, _timeout_attempt| {
                        hook(post, ticket).map(|outcome| match outcome {
                            ExactOutputTestAdmission::Admitted
                                if matches!(route, ExactTargetRoute::Reply(_)) =>
                            {
                                ExactOutputAttemptOutcome::TestReplyFlushed
                            }
                            ExactOutputTestAdmission::Admitted => {
                                ExactOutputAttemptOutcome::Admitted
                            }
                            ExactOutputTestAdmission::SidecarFlush(flush_ack) => {
                                ExactOutputAttemptOutcome::SidecarFlush(flush_ack)
                            }
                            ExactOutputTestAdmission::Retired => ExactOutputAttemptOutcome::Retired,
                        })
                    })?
                } else {
                    pending.drive_bounded_with_ack(|post, ticket, route, timeout_attempt| {
                        self.admit_network_exact_output(post, ticket, route, timeout_attempt)
                    })?
                }
            }
            #[cfg(not(test))]
            {
                pending.drive_bounded_with_ack(|post, ticket, route, timeout_attempt| {
                    self.admit_network_exact_output(post, ticket, route, timeout_attempt)
                })?
            }
        };
        pending.poll_reply_flushes()?;
        match outcome {
            ExactOutputDriveOutcome::Drained => {}
            ExactOutputDriveOutcome::ReceiptBackpressured => {
                iroha_logger::debug!(
                    pending_receipts = pending.sidecar_control_units(),
                    pending_flushes = pending.pending_sidecar_flushes(),
                    receipt_capacity = pending.sidecar_admission_capacity,
                    "retained exact Sumeragi v2 output behind sidecar receipt backpressure"
                );
            }
            ExactOutputDriveOutcome::Backpressured { closest_rank } => {
                iroha_logger::debug!(
                    rank = closest_rank,
                    pending_fanouts = pending.fanouts.len(),
                    "retained exact Sumeragi v2 output behind network-actor backpressure"
                );
            }
            ExactOutputDriveOutcome::BudgetExhausted {
                closest_backpressure_rank,
            } => {
                iroha_logger::debug!(
                    rank = ?closest_backpressure_rank,
                    pending_fanouts = pending.fanouts.len(),
                    attempt_budget = pending.drive_attempt_budget,
                    "yielded a bounded exact Sumeragi v2 output admission slice"
                );
            }
        }
        Ok(pending.is_pending())
    }
    fn enqueue_exact_fanout_while_guarded(
        &self,
        messages: Vec<NetworkMessage>,
        peers: Vec<PeerId>,
        rollover_claim: ExactOutputRolloverClaim,
        _permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        let Some(fanout) = PendingExactFanout::claimed(messages, peers, rollover_claim)? else {
            return Ok(ExactFanoutOwnership::Owned);
        };
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            return Err(
                "Sumeragi v2 exact output is sealed after durable finality handoff".to_owned(),
            );
        }
        let ownership = pending.enqueue(fanout)?;
        if ownership == ExactFanoutOwnership::Owned {
            let _ = self.drive_pending_exact_output(&mut pending)?;
        }
        Ok(ownership)
    }
    /// Transfer an inseparable topology batch after same-lock bound/capacity/FIFO
    /// checks, returning it whole when full.
    fn enqueue_atomic_fanout_batch_while_guarded(
        &self,
        fanouts: Vec<PendingExactFanout>,
        _permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            return Err(
                "Sumeragi v2 exact output is sealed after durable finality handoff".to_owned(),
            );
        }
        let Some(batch) = pending.prepare_atomic_fanout_batch(fanouts)? else {
            return Ok(ExactFanoutOwnership::SourceRetained);
        };
        pending.commit_atomic_fanout_batch(batch);
        let _ = self.drive_pending_exact_output(&mut pending)?;
        Ok(ExactFanoutOwnership::Owned)
    }
    fn enqueue_owned_exact_reply_routes_while_guarded(
        &self,
        message: NetworkMessage,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
        rollover_claim: ExactOutputRolloverClaim,
        _permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        if reply_routes.semantic_target() != &peer {
            return Err(
                "Sumeragi v2 reply route does not match its semantic output target".to_owned(),
            );
        }
        let Some(fanout) = PendingExactFanout::claimed_with_reply_routes_and_ingress_ownership(
            vec![message],
            peer,
            reply_routes,
            ingress_ownership,
            rollover_claim,
        )?
        else {
            return Ok(ExactFanoutOwnership::Owned);
        };
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            return Err(
                "Sumeragi v2 exact output is sealed after durable finality handoff".to_owned(),
            );
        }
        let ownership = pending.enqueue_owned_reply_transfer(fanout)?;
        if ownership == ExactFanoutOwnership::Owned {
            let _ = self.drive_pending_exact_output(&mut pending)?;
        }
        Ok(ownership)
    }
    fn exact_output_scope(&self) -> ExactOutputCreationScope {
        ExactOutputCreationScope {
            context_id: self.context.id(),
            height: self.context.height,
        }
    }
    /// Advance the shared process-lifetime advert refresher by one bounded
    /// turn.  A retained refresh token is independent of `PendingExactOutput`;
    /// only an accepted enqueue gains an exact rollover claim.
    pub(crate) fn service_kura_replica_advert_refresh_turn(
        &self,
        now: Instant,
    ) -> Result<KuraReplicaAdvertRefreshTurnOutcome, String> {
        if self.exact_output_handoff_owner.is_sealed() {
            return Ok(KuraReplicaAdvertRefreshTurnOutcome::default());
        }
        let durable_tip = self
            .kura
            .exact_kura_replica_advert_tip()
            .map_err(|error| error.to_string())?;
        self.kura_replica_advert_refresh
            .note_durable_tip(durable_tip, now)?;
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let outcome = self.kura_replica_advert_refresh.drive_turn(
            now,
            |source_height| {
                self.kura
                    .probe_kura_replica_advert_source(source_height, &self.key_pair)
                    .map_err(|error| error.to_string())
            },
            |source| self.post_kura_replica_advert_while_guarded(source, operation.permit()),
        )?;
        operation.complete();
        Ok(outcome)
    }
    /// Retry every currently schedulable exact semantic-output target.
    ///
    /// Returns `true` while an exact actor-backpressured target remains owned.
    pub(crate) fn retry_pending_exact_output(&self) -> Result<bool, String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let pending_remains = {
            let mut pending = self.lock_pending_exact_output()?;
            if self.exact_output_handoff_owner.is_sealed() {
                debug_assert!(!pending.is_pending());
                operation.complete();
                return Ok(false);
            }
            self.drive_pending_exact_output(&mut pending)?
        };
        operation.complete();
        Ok(pending_remains)
    }
    /// After exact Kura/finality authority, transfer finalized height-local,
    /// durable lane, Kura-backed response, and exact-scope sidecar output to
    /// reconstruction; manual or cross-scope output stays owned.
    pub(crate) fn handoff_applied_height_output_to_durable_reconstruction(
        &self,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
        durable_lane_authority: &DurableLaneRolloverAuthority,
    ) -> Result<usize, String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.validate_applied_height_output_handoff_authority(receipt, artifact)?;
        let (retired, retired_kura_replica_advert_heights) = {
            let mut pending = self.lock_pending_exact_output()?;
            if self.exact_output_handoff_owner.is_sealed() {
                return Err(
                    "Sumeragi v2 applied-height output handoff is already sealed".to_owned(),
                );
            }
            let retired_kura_replica_advert_heights =
                pending.pending_kura_replica_advert_heights()?;
            let retired = pending.handoff_applied_height_to_durable_reconstruction(
                artifact,
                Some(durable_lane_authority),
                Some(self.kura.as_ref()),
            )?;
            (retired, retired_kura_replica_advert_heights)
        };
        let scheduled_kura_replica_adverts = self
            .kura_replica_advert_refresh
            .schedule_retired_exact_output_heights(
                retired_kura_replica_advert_heights,
                Instant::now(),
            )?;
        if retired != 0 {
            iroha_logger::debug!(
                height = receipt.height(),
                retired_posts = retired,
                scheduled_kura_replica_adverts,
                "handed backpressured finalized-height output to durable reconstruction"
            );
        }
        operation.complete();
        Ok(retired)
    }
    /// After lane handoff quiesces, revalidate authority, perform the final
    /// atomic handoff, require emptiness, seal enqueue, and mint one receipt.
    pub(crate) fn seal_applied_height_output_handoff(
        &self,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
        durable_lane_authority: &DurableLaneRolloverAuthority,
    ) -> Result<DurableExactOutputHandoffReceipt, String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.validate_applied_height_output_handoff_authority(receipt, artifact)?;
        let retired = {
            let mut pending = self.lock_pending_exact_output()?;
            let retired = pending.handoff_applied_height_to_durable_reconstruction(
                artifact,
                Some(durable_lane_authority),
                Some(self.kura.as_ref()),
            )?;
            if pending.is_pending() {
                return Err(
                    "Sumeragi v2 final exact-output handoff did not clear its corridor".to_owned(),
                );
            }
            if retired != 0 {
                return Err(
                    "Sumeragi v2 final exact-output seal observed newly retained output".to_owned(),
                );
            }
            self.exact_output_handoff_owner.seal()?;
            retired
        };
        debug_assert_eq!(retired, 0);
        let handoff = DurableExactOutputHandoffReceipt {
            owner: Arc::clone(&self.exact_output_handoff_owner.0),
            predecessor_context_hash: HashOf::new(&self.context),
            predecessor_context_id: self.context.id(),
            predecessor_height: self.context.height,
            predecessor_network_id: self.context.network_id,
            finality_artifact_hash: HashOf::new(artifact),
            finality_commit_qc: artifact.commit_qc.clone(),
        };
        operation.complete();
        Ok(handoff)
    }
    fn validate_applied_height_output_handoff_authority(
        &self,
        receipt: &KuraV2CommitReceipt,
        artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<(), String> {
        artifact.validate().map_err(|error| error.to_string())?;
        if artifact.height_context != self.context
            || receipt.height() != self.context.height
            || receipt.context_id() != self.context.id()
            || receipt.subject() != artifact.subject
            || receipt.block_hash() != artifact.block_hash
            || receipt.certificate() != artifact.commit_qc.as_ref()
            || receipt.artifact_hash() != HashOf::new(artifact)
        {
            return Err(
                "Sumeragi v2 applied-height output handoff has mismatched finality authority"
                    .to_owned(),
            );
        }
        Ok(())
    }
    /// Drain process-local sidecar receipts after the exact peer writer flushes
    /// their response chunks.
    pub(crate) fn drain_certified_merge_sidecar_chunk_admissions(
        &self,
        limit: usize,
    ) -> Result<Vec<CertifiedMergeSidecarChunkAdmission>, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(Vec::new());
        }
        pending.poll_reply_flushes()?;
        let count = limit.min(pending.admitted_sidecar_chunks.len());
        Ok(pending.admitted_sidecar_chunks.drain(..count).collect())
    }
    /// Cancel every queued or writer-pending response occurrence covered by an
    /// authenticated cumulative close for the exact durable stream incarnation
    /// before any newer output is dispatched.
    pub(crate) fn close_certified_merge_sidecar_prefix(
        &self,
        prefix: &CertifiedMergeSidecarClosedPrefix,
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.close_certified_sidecar_prefix(prefix)
    }
    /// Cancel every exact-output occurrence whose historical request owner
    /// completed through another authenticated source.
    pub(crate) fn cancel_historical_lane_recovery_requests(
        &self,
        request_hashes: &BTreeSet<HashOf<LaneHistoricalRecoveryRequestV1>>,
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.cancel_historical_lane_recovery_requests(request_hashes)
    }
    /// Cancel every retained transport fanout for one completed or superseded
    /// CommitQC discovery request.
    pub(crate) fn cancel_block_sync_request(
        &self,
        request_hash: HashOf<wire::CommitCertificateRequest>,
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.cancel_commit_certificate_request(request_hash)
    }
    /// Cancel requester-side sidecar output after its transport attempt retires.
    pub(crate) fn cancel_certified_merge_sidecar_requests(
        &self,
        request_hashes: &BTreeSet<HashOf<CertifiedMergeSidecarRequestV1>>,
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.cancel_certified_merge_sidecar_requests(request_hashes)
    }
    /// Cancel canonical requester Request/Close output made obsolete by each
    /// authenticated successor-generation fence for its exact endpoint pair.
    pub(crate) fn cancel_obsolete_certified_merge_sidecar_generation_hints(
        &self,
        hints: &[CertifiedMergeSidecarGenerationHintV1],
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.cancel_obsolete_certified_merge_sidecar_generation_hints(hints)
    }
    /// Cancel requester-side Close retries covered by cumulative acknowledgements.
    pub(crate) fn cancel_acknowledged_certified_merge_sidecar_closes(
        &self,
        acknowledgements: &[CertifiedMergeSidecarCloseAckV1],
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.cancel_acknowledged_certified_merge_sidecar_closes(acknowledgements)
    }
    /// Check the exact target/class/kind reservation for the next lane-work effect.
    pub(crate) fn can_retain_lane_work_effect_from_snapshot(
        &self,
        effect: &V2LaneWorkEffect,
        queue_plan_sources: Option<&mut QueuePlanBatchSources>,
    ) -> Result<bool, String> {
        let (messages, peers, routes, reply_route_history, ingress_ownership, rollover_claim) =
            match effect {
                V2LaneWorkEffect::PostLaneBlock { peer, message } => {
                    let rollover_claim = match message {
                        BlockMessage::LaneHistoricalRecoveryRequest(request) => {
                            ExactOutputRolloverClaim::HistoricalLaneRecoveryRequest {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                request_hash: HashOf::new(request.as_ref()),
                            }
                        }
                        BlockMessage::LaneHistoricalRecoveryResponse(response) => {
                            ExactOutputRolloverClaim::HistoricalLaneRecoveryResponse {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                request_hash: response.request_hash,
                                response_hash: HashOf::new(response.as_ref()),
                            }
                        }
                        _ => self.current_lane_output_rollover_claim(message, peer)?,
                    };
                    let wire = BlockMessageWire::try_preencoded(Arc::new(message.clone()))
                        .map_err(|error| error.to_string())?;
                    (
                        vec![NetworkMessage::SumeragiBlock(Arc::new(wire))],
                        vec![peer.clone()],
                        vec![ExactTargetRoute::Topology],
                        None,
                        None,
                        rollover_claim,
                    )
                }
                V2LaneWorkEffect::PostDurableLaneCertificate {
                    peer,
                    reply_routes,
                    ingress_ownership,
                    certificate,
                } => {
                    let reply_routes = reply_routes.as_ref().ok_or_else(|| {
                        "durable lane-certificate response lost its authenticated reply routes"
                            .to_owned()
                    })?;
                    let ingress_ownership = ingress_ownership.as_ref().ok_or_else(|| {
                        "durable lane-certificate response lost its fair-ingress ownership"
                            .to_owned()
                    })?;
                    if !ingress_ownership.validate_exact()
                        || !ingress_ownership.matches_reply_routes(Some(reply_routes))
                    {
                        return Err(
                            "durable lane-certificate response has altered fair-ingress ownership"
                                .to_owned(),
                        );
                    }
                    let (peers, routes, reply_route_history) =
                        Self::exact_target_geometry(peer, Some(reply_routes))?;
                    let wire = BlockMessageWire::try_preencoded(Arc::new(
                        BlockMessage::LaneBlockCertificate(Box::new(certificate.clone())),
                    ))
                    .map_err(|error| error.to_string())?;
                    let descriptor = &certificate.proposal.descriptor;
                    (
                        vec![NetworkMessage::SumeragiBlock(Arc::new(wire))],
                        peers,
                        routes,
                        reply_route_history,
                        Some(ingress_ownership.clone()),
                        ExactOutputRolloverClaim::DurableLaneCertificateResponse {
                            scope: self.exact_output_scope(),
                            target: peer.clone(),
                            lane_id: descriptor.lane_id,
                            lane_block_height: descriptor.lane_block_height,
                            proposal_height: descriptor.proposal_height,
                            proposal_hash: certificate.proposal.proposal_hash,
                            certificate_hash: HashOf::new(certificate),
                        },
                    )
                }
                V2LaneWorkEffect::PostNativeAmx {
                    peer,
                    reply_routes,
                    message,
                } => {
                    let valid = match message {
                        NativeAmxMessage::PrepareRequest(_)
                        | NativeAmxMessage::CommitRequest(_) => reply_routes.is_none(),
                        NativeAmxMessage::PrepareVote(_) | NativeAmxMessage::CommitVote(_) => {
                            reply_routes.is_some()
                        }
                    };
                    if !valid {
                        return Err(
                            "Native AMX effect has invalid reply-route ownership".to_owned()
                        );
                    }
                    let body = native_amx_message_body(message)?;
                    let (peers, routes, reply_route_history) =
                        Self::exact_target_geometry(peer, reply_routes.as_ref())?;
                    (
                        vec![NetworkMessage::NativeAmx(Arc::new(message.clone()))],
                        peers,
                        routes,
                        reply_route_history,
                        None,
                        ExactOutputRolloverClaim::NativeAmx {
                            scope: self.exact_output_scope(),
                            round: body.round,
                            message_hash: HashOf::new(message),
                        },
                    )
                }
                V2LaneWorkEffect::PostLaneDrainVote { peer, vote } => {
                    vote.validate_ingress().map_err(|error| {
                        format!("lane-drain effect has invalid vote evidence: {error}")
                    })?;
                    (
                        vec![NetworkMessage::LaneDrainVote(Box::new(vote.clone()))],
                        vec![peer.clone()],
                        vec![ExactTargetRoute::Topology],
                        None,
                        None,
                        ExactOutputRolloverClaim::LaneDrainVote {
                            scope: self.exact_output_scope(),
                            target: peer.clone(),
                            vote_hash: HashOf::new(vote),
                        },
                    )
                }
                V2LaneWorkEffect::BroadcastMerge(signature) => {
                    let peers = self.remote_voters();
                    let routes = vec![ExactTargetRoute::Topology; peers.len()];
                    (
                        vec![NetworkMessage::MergeCommitteeSignature(Arc::new(
                            signature.clone(),
                        ))],
                        peers,
                        routes,
                        None,
                        None,
                        ExactOutputRolloverClaim::MergeShare {
                            scope: self.exact_output_scope(),
                            share_hash: HashOf::new(signature),
                        },
                    )
                }
                V2LaneWorkEffect::PostQueuePlanAdmissionCertificate {
                    peer,
                    view,
                    certificate,
                } => self.queue_plan_effect_parts(
                    peer,
                    *view,
                    certificate,
                    queue_plan_sources.ok_or_else(|| {
                        "QueuePlan admission handoff lacks its Kura batch snapshot".to_owned()
                    })?,
                )?,
                V2LaneWorkEffect::PostCertifiedMergeSidecar {
                    peer,
                    reply_routes,
                    message,
                } => {
                    let valid = match message.as_ref() {
                        CertifiedMergeSidecarMessage::Request(_)
                        | CertifiedMergeSidecarMessage::Close(_) => reply_routes.is_none(),
                        CertifiedMergeSidecarMessage::CloseAck(_)
                        | CertifiedMergeSidecarMessage::GenerationHint(_)
                        | CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_some(),
                    };
                    if !valid {
                        return Err(
                            "certified merge-sidecar effect has invalid reply-route ownership"
                                .to_owned(),
                        );
                    }
                    let rollover_claim = match message.as_ref() {
                        CertifiedMergeSidecarMessage::Request(request)
                            if request.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                                && request.requester == self.local_peer
                                && request.responder == *peer =>
                        {
                            ExactOutputRolloverClaim::CertifiedSidecarRequest {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                transfer: CertifiedSidecarTransferIdentity::from_request(request),
                                request_hash: HashOf::new(request),
                            }
                        }
                        CertifiedMergeSidecarMessage::Close(close)
                            if close.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                                && close.closed_through != 0
                                && close.close_id == close.canonical_close_id()
                                && close.requester == self.local_peer
                                && close.responder == *peer =>
                        {
                            ExactOutputRolloverClaim::CertifiedSidecarControl {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                message_hash: HashOf::new(message.as_ref()),
                            }
                        }
                        CertifiedMergeSidecarMessage::CloseAck(ack)
                            if ack.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                                && ack.closed_through != 0
                                && ack.close_id == ack.canonical_close_id()
                                && ack.responder == self.local_peer
                                && ack.requester == *peer =>
                        {
                            ExactOutputRolloverClaim::CertifiedSidecarControl {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                message_hash: HashOf::new(message.as_ref()),
                            }
                        }
                        CertifiedMergeSidecarMessage::GenerationHint(hint)
                            if hint.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                                && hint.hint_id == hint.canonical_hint_id()
                                && hint.responder == self.local_peer
                                && hint.requester == *peer =>
                        {
                            ExactOutputRolloverClaim::CertifiedSidecarControl {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                message_hash: HashOf::new(message.as_ref()),
                            }
                        }
                        CertifiedMergeSidecarMessage::Chunk(chunk)
                            if chunk.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                                && chunk.responder == self.local_peer
                                && chunk.requester == *peer
                                && chunk.chunk_count != 0
                                && chunk.chunk_index < chunk.chunk_count =>
                        {
                            ExactOutputRolloverClaim::CertifiedSidecarChunk {
                                scope: self.exact_output_scope(),
                                target: peer.clone(),
                                transfer: CertifiedSidecarTransferIdentity::from_chunk(chunk),
                                chunk_index: chunk.chunk_index,
                                chunk_count: chunk.chunk_count,
                                response_hash: HashOf::new(chunk),
                            }
                        }
                        _ => {
                            return Err(
                                "certified merge-sidecar effect has no valid rollover claim"
                                    .to_owned(),
                            );
                        }
                    };
                    let (peers, routes, reply_route_history) =
                        Self::exact_target_geometry(peer, reply_routes.as_ref())?;
                    (
                        vec![NetworkMessage::CertifiedMergeSidecar(Arc::clone(message))],
                        peers,
                        routes,
                        reply_route_history,
                        None,
                        rollover_claim,
                    )
                }
            };
        let Some(fanout) = PendingExactFanout::classified_with_route_history(
            messages,
            peers,
            routes,
            reply_route_history,
        )?
        else {
            return Ok(true);
        };
        rollover_claim.validate_fanout(&fanout.messages, &fanout.semantic_peers())?;
        let mut fanout = fanout;
        fanout.ingress_ownership = ingress_ownership;
        fanout.rollover_claim = rollover_claim;
        let pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Err(
                "Sumeragi v2 exact output is sealed after durable finality handoff".to_owned(),
            );
        }
        if fanout
            .targets
            .iter()
            .all(|target| matches!(&target.route, ExactTargetRoute::Reply(_)))
        {
            pending.can_enqueue_owned_reply_transfer(fanout)
        } else {
            pending.can_enqueue(&fanout)
        }
    }
    /// Publish one exact signed body-keeper advert from durable Kura state.
    ///
    /// The advert is rebuilt only after canonical application completes, then
    /// independently revalidated before entering the exact-output corridor.
    /// Its rollover claim remains reconstructible from the same body/finality
    /// source and the frozen height roster.
    fn post_kura_replica_advert_while_guarded(
        &self,
        source: &KuraReplicaAdvertSourceV1,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        let source_height = source.height();
        if source_height == 0 || source_height > self.context.height {
            return Err(
                "Kura replica advert source is outside the active height authority".to_owned(),
            );
        }
        let advert = self
            .kura
            .build_signed_kura_replica_advert_from_source(source, &self.key_pair)
            .map_err(|error| error.to_string())?;
        let rollover_claim = ExactOutputRolloverClaim::DurableKuraReplicaAdvert {
            scope: self.exact_output_scope(),
            source_height,
            advert_hash: HashOf::new(&advert),
        };
        let wire =
            BlockMessageWire::try_preencoded(Arc::new(BlockMessage::KuraReplicaAdvert(advert)))
                .map_err(|error| {
                    format!("failed to encode durable Kura replica advert: {error}")
                })?;
        // The active immutable roster is the only live, bounded transport
        // authority available under validator rotation. Historical departed
        // validators are not guessed or contacted; Kura pins bodies outside
        // the configured proactive horizon fail-closed.
        self.enqueue_exact_fanout_while_guarded(
            vec![NetworkMessage::SumeragiBlock(Arc::new(wire))],
            self.remote_voters(),
            rollover_claim,
            permit,
        )
    }
    fn committee_for_round(&self, round: wire::ConsensusRound) -> Result<Committee, String> {
        if round.context_id != self.context.id() || round.height != self.context.height {
            return Err("Sumeragi v2 committee routing received a foreign round".to_owned());
        }
        Committee::project_indices(
            self.context.height,
            round.view,
            self.context.roster.len(),
            self.context.leader(round.view),
        )
        .map_err(|error| error.to_string())
    }
    fn remote_voters_for_indices(
        &self,
        indices: &[wire::ValidatorIndex],
    ) -> Result<Vec<PeerId>, String> {
        let mut peers = Vec::with_capacity(indices.len());
        for index in indices {
            let roster_index = usize::try_from(*index)
                .map_err(|_| "Sumeragi v2 committee index does not fit usize".to_owned())?;
            let peer = self
                .context
                .roster
                .get(roster_index)
                .ok_or_else(|| "Sumeragi v2 committee index is outside the roster".to_owned())?
                .validator
                .clone();
            if peer != self.local_peer {
                peers.push(peer);
            }
        }
        Ok(peers)
    }
    fn enqueue_fail_stop_io(&self, command: V2IoCommand) -> Result<(), String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.io()?.enqueue(command)?;
        operation.complete();
        Ok(())
    }
    /// Mark an operator-requested shutdown as non-fatal before dropping services.
    pub(crate) fn allow_clean_shutdown(&mut self) {
        self.clean_teardown = true;
    }
    fn deliver_payload_chunk<R: EffectRuntime>(
        &mut self,
        executor: &mut V2EffectExecutor<R>,
        work_id: EffectWorkId,
        sender: PeerId,
        chunk: wire::PayloadChunk,
        ingress_ownership: FairV2IngressOwnershipEvidence,
    ) -> Result<PayloadChunkDisposition, String> {
        let result = executor.accept_payload_chunk_with_ingress_ownership(
            work_id,
            chunk,
            &sender,
            &ingress_ownership,
            self,
        );
        if let Some(runtime) = ingress_ownership.leader_wire_runtime_receipt() {
            self.leader_wire_ingress
                .mark_leader_wire_volatile_terminal(runtime)?;
        }
        match result {
            Ok(()) => Ok(PayloadChunkDisposition::Delivered),
            Err(EffectTransportError::FailClosed(reason)) => Err(reason),
            Err(error) => {
                iroha_logger::debug!(%sender, %error, "rejected Sumeragi v2 payload chunk");
                Ok(PayloadChunkDisposition::Rejected)
            }
        }
    }
    /// Send one response through every retained authenticated source route.
    pub(crate) fn post_to_peer_on_reply_routes(
        &self,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: FairV2IngressOwnershipEvidence,
        message: wire::ConsensusMessageV2,
    ) -> Result<(), String> {
        if reply_routes.semantic_target() != &peer
            || !ingress_ownership.validate_exact()
            || !ingress_ownership.matches_reply_routes(Some(&reply_routes))
        {
            return Err(
                "certified-body response carried altered fair-ingress ownership".to_owned(),
            );
        }
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if reply_routes.is_empty() {
            iroha_logger::debug!(
                "deferred certified Sumeragi v2 response after all retained reply routes retired"
            );
            operation.complete();
            return Ok(());
        }
        let ownership = self.post_block_message_on_reply_routes_while_guarded(
            peer,
            reply_routes,
            ingress_ownership,
            BlockMessage::V2(message),
            operation.permit(),
        )?;
        if ownership == ExactFanoutOwnership::SourceRetained {
            iroha_logger::debug!(
                "deferred certified Sumeragi v2 response to requester reconstruction"
            );
        }
        operation.complete();
        Ok(())
    }
    /// Send one response whose exact payload can be rebuilt from immutable Kura history.
    #[cfg(test)]
    pub(crate) fn post_durable_history_response_with_permit(
        &self,
        peer: PeerId,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        self.post_durable_history_response_with_routes(peer, None, None, message, permit)
    }
    /// Send a durable historical response through all authenticated source routes.
    pub(crate) fn post_durable_history_response_on_reply_routes_with_permit(
        &self,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: FairV2IngressOwnershipEvidence,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        self.post_durable_history_response_with_routes(
            peer,
            Some(reply_routes),
            Some(ingress_ownership),
            message,
            permit,
        )
    }
    fn post_durable_history_response_with_routes(
        &self,
        peer: PeerId,
        reply_routes: Option<NetworkReplyRoutes>,
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        match (&reply_routes, &ingress_ownership) {
            (Some(routes), Some(ownership))
                if ownership.validate_exact() && ownership.matches_reply_routes(Some(routes)) => {}
            (None, None) => {}
            (Some(_), Some(_)) => {
                return Err(
                    "durable history response carried altered fair-ingress ownership".to_owned(),
                );
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err("durable history response lost its fair-ingress ownership".to_owned());
            }
        }
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        let rollover_claim = match &message.payload {
            wire::ConsensusMessageV2Payload::CommitCertificateResponse(response)
                if response.certificate.round.height <= self.context.height
                    && response.responder == self.local_peer =>
            {
                ExactOutputRolloverClaim::DurableCommitCertificateResponse {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    responder: self.local_peer.clone(),
                    source_height: response.certificate.round.height,
                    source_context_id: response.certificate.round.context_id,
                    response_hash: HashOf::new(response),
                }
            }
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response)
                if response.manifest.round.height <= self.context.height =>
            {
                ExactOutputRolloverClaim::DurableCertifiedBodyResponse {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    responder: self.local_peer.clone(),
                    source_round: response.manifest.round,
                    source_subject: response.manifest.subject,
                    response_hash: HashOf::new(response),
                }
            }
            _ => {
                return Err(
                    "guarded durable-history output is not a non-future Kura response".to_owned(),
                );
            }
        };
        let block_message = Arc::new(BlockMessage::V2(message));
        let wire = BlockMessageWire::try_preencoded(block_message).map_err(|error| {
            format!("failed to encode guarded durable-history response for {peer}: {error}")
        })?;
        let messages = vec![NetworkMessage::SumeragiBlock(Arc::new(wire))];
        let peers = vec![peer];
        rollover_claim.validate_fanout(&messages, &peers)?;
        durable_history_source_covers(
            &messages,
            &rollover_claim,
            &self.context.network_id,
            self.context.height,
            self.kura.as_ref(),
        )?;
        let ownership = match reply_routes {
            Some(reply_routes) => self.enqueue_owned_exact_reply_routes_while_guarded(
                messages
                    .into_iter()
                    .next()
                    .expect("durable response is a singleton"),
                peers
                    .into_iter()
                    .next()
                    .expect("durable response has one target"),
                reply_routes,
                ingress_ownership,
                rollover_claim,
                permit,
            )?,
            None => {
                self.enqueue_exact_fanout_while_guarded(messages, peers, rollover_claim, permit)?
            }
        };
        if ownership == ExactFanoutOwnership::SourceRetained {
            iroha_logger::debug!(
                "deferred historical Sumeragi v2 response to requester reconstruction"
            );
        }
        Ok(())
    }
    /// Send retained lane-local traffic selected by `BlockMessage::is_lane_local`
    /// through the common exact-output corridor.
    pub(crate) fn post_lane_block(
        &self,
        peer: PeerId,
        message: BlockMessage,
    ) -> Result<(), String> {
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if !message.is_lane_local() {
            return Err("v2 lane transport rejected a non-lane block message".to_owned());
        }
        let ownership = self.post_block_message_while_guarded(peer, message, operation.permit())?;
        if ownership == ExactFanoutOwnership::SourceRetained {
            return Err(
                "Sumeragi v2 lane output reached an unreserved corridor boundary".to_owned(),
            );
        }
        operation.complete();
        Ok(())
    }
    /// Send one exact lane certificate reconstructed from its certified Kura artifact.
    #[cfg(test)]
    pub(crate) fn post_durable_lane_certificate(
        &self,
        peer: PeerId,
        certificate: LaneBlockCertificateV1,
    ) -> Result<(), String> {
        self.post_durable_lane_certificate_with_routes(peer, None, None, certificate)
    }
    /// Send a Kura-backed lane certificate through every retained source route.
    pub(crate) fn post_durable_lane_certificate_on_reply_routes(
        &self,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: FairV2IngressOwnershipEvidence,
        certificate: LaneBlockCertificateV1,
    ) -> Result<(), String> {
        self.post_durable_lane_certificate_with_routes(
            peer,
            Some(reply_routes),
            Some(ingress_ownership),
            certificate,
        )
    }
    fn post_durable_lane_certificate_with_routes(
        &self,
        peer: PeerId,
        reply_routes: Option<NetworkReplyRoutes>,
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
        certificate: LaneBlockCertificateV1,
    ) -> Result<(), String> {
        let operation = self
            .output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        match (&reply_routes, &ingress_ownership) {
            (Some(routes), Some(ownership))
                if ownership.validate_exact() && ownership.matches_reply_routes(Some(routes)) => {}
            (None, None) => {}
            (Some(_), Some(_)) => {
                return Err(
                    "durable lane certificate carried altered fair-ingress ownership".to_owned(),
                );
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err("durable lane certificate lost its fair-ingress ownership".to_owned());
            }
        }
        let descriptor = &certificate.proposal.descriptor;
        if descriptor.proposal_height > self.context.height {
            return Err("durable lane certificate belongs to a future global height".to_owned());
        }
        let rollover_claim = ExactOutputRolloverClaim::DurableLaneCertificateResponse {
            scope: self.exact_output_scope(),
            target: peer.clone(),
            lane_id: descriptor.lane_id,
            lane_block_height: descriptor.lane_block_height,
            proposal_height: descriptor.proposal_height,
            proposal_hash: certificate.proposal.proposal_hash,
            certificate_hash: HashOf::new(&certificate),
        };
        let message = Arc::new(BlockMessage::LaneBlockCertificate(Box::new(certificate)));
        let wire = BlockMessageWire::try_preencoded(message).map_err(|error| {
            format!("failed to encode guarded durable lane certificate for {peer}: {error}")
        })?;
        let messages = vec![NetworkMessage::SumeragiBlock(Arc::new(wire))];
        let peers = vec![peer];
        rollover_claim.validate_fanout(&messages, &peers)?;
        durable_history_source_covers(
            &messages,
            &rollover_claim,
            &self.context.network_id,
            self.context.height,
            self.kura.as_ref(),
        )?;
        let ownership = match reply_routes {
            Some(reply_routes) => self.enqueue_owned_exact_reply_routes_while_guarded(
                messages
                    .into_iter()
                    .next()
                    .expect("durable lane response is a singleton"),
                peers
                    .into_iter()
                    .next()
                    .expect("durable lane response has one target"),
                reply_routes,
                ingress_ownership,
                rollover_claim,
                operation.permit(),
            )?,
            None => self.enqueue_exact_fanout_while_guarded(
                messages,
                peers,
                rollover_claim,
                operation.permit(),
            )?,
        };
        if ownership == ExactFanoutOwnership::SourceRetained {
            return Err(
                "durable lane certificate reached an unreserved corridor boundary".to_owned(),
            );
        }
        operation.complete();
        Ok(())
    }
    /// Send one bounded certified merge-sidecar request or response through
    /// the dedicated authenticated network envelope.
    #[cfg(test)]
    pub(crate) fn post_certified_merge_sidecar(
        &self,
        peer: PeerId,
        message: CertifiedMergeSidecarMessage,
    ) {
        let _ = self.post_certified_merge_sidecar_with_reply_routes(peer, None, Arc::new(message));
    }
    /// Send a sidecar request normally or a response on its exact request route.
    pub(crate) fn post_certified_merge_sidecar_with_reply_routes(
        &self,
        peer: PeerId,
        reply_routes: Option<NetworkReplyRoutes>,
        message: Arc<CertifiedMergeSidecarMessage>,
    ) -> Result<ExactFanoutOwnership, String> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let route_shape_is_valid = match message.as_ref() {
            CertifiedMergeSidecarMessage::Request(_) | CertifiedMergeSidecarMessage::Close(_) => {
                reply_routes.is_none()
            }
            CertifiedMergeSidecarMessage::CloseAck(_)
            | CertifiedMergeSidecarMessage::GenerationHint(_)
            | CertifiedMergeSidecarMessage::Chunk(_) => reply_routes.is_some(),
        };
        if !route_shape_is_valid {
            return Err(
                "certified merge-sidecar request/response has invalid reply-route ownership"
                    .to_owned(),
            );
        }
        let rollover_claim = match message.as_ref() {
            CertifiedMergeSidecarMessage::Request(request)
                if request.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                    && request.requester == self.local_peer
                    && request.responder == peer =>
            {
                ExactOutputRolloverClaim::CertifiedSidecarRequest {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    transfer: CertifiedSidecarTransferIdentity::from_request(request),
                    request_hash: HashOf::new(request),
                }
            }
            CertifiedMergeSidecarMessage::Close(close)
                if close.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                    && close.closed_through != 0
                    && close.close_id == close.canonical_close_id()
                    && close.requester == self.local_peer
                    && close.responder == peer =>
            {
                ExactOutputRolloverClaim::CertifiedSidecarControl {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    message_hash: HashOf::new(message.as_ref()),
                }
            }
            CertifiedMergeSidecarMessage::CloseAck(ack)
                if ack.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                    && ack.closed_through != 0
                    && ack.close_id == ack.canonical_close_id()
                    && ack.responder == self.local_peer
                    && ack.requester == peer =>
            {
                ExactOutputRolloverClaim::CertifiedSidecarControl {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    message_hash: HashOf::new(message.as_ref()),
                }
            }
            CertifiedMergeSidecarMessage::GenerationHint(hint)
                if hint.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                    && hint.hint_id == hint.canonical_hint_id()
                    && hint.responder == self.local_peer
                    && hint.requester == peer =>
            {
                ExactOutputRolloverClaim::CertifiedSidecarControl {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    message_hash: HashOf::new(message.as_ref()),
                }
            }
            CertifiedMergeSidecarMessage::Chunk(chunk)
                if chunk.version == CERTIFIED_MERGE_SIDECAR_VERSION_V1
                    && chunk.responder == self.local_peer
                    && chunk.requester == peer
                    && chunk.chunk_count != 0
                    && chunk.chunk_index < chunk.chunk_count =>
            {
                ExactOutputRolloverClaim::CertifiedSidecarChunk {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    transfer: CertifiedSidecarTransferIdentity::from_chunk(chunk),
                    chunk_index: chunk.chunk_index,
                    chunk_count: chunk.chunk_count,
                    response_hash: HashOf::new(chunk),
                }
            }
            _ => {
                return Err(
                    "certified merge-sidecar post has no valid semantic rollover claim".to_owned(),
                );
            }
        };
        let data = NetworkMessage::CertifiedMergeSidecar(message);
        let result = match reply_routes {
            Some(reply_routes) => self.enqueue_owned_exact_reply_routes_while_guarded(
                data,
                peer,
                reply_routes,
                None,
                rollover_claim,
                operation.permit(),
            ),
            None => self.enqueue_exact_fanout_while_guarded(
                vec![data],
                vec![peer],
                rollover_claim,
                operation.permit(),
            ),
        };
        let ownership = result?;
        // A concurrent producer can consume the capacity observed by runner
        // preflight. Source retention is bounded backpressure, not loss of the
        // already-owned lane effect, so disarm fail-stop and let the runner
        // return the exact effect to its fair queue.
        operation.complete();
        Ok(ownership)
    }
    /// Send one context-bound Native AMX v2 message to a participant peer.
    #[cfg(test)]
    pub(crate) fn post_native_amx(&self, peer: PeerId, message: NativeAmxMessage) {
        self.post_native_amx_with_reply_routes(peer, None, message);
    }
    /// Send a Native AMX request normally or a request-induced vote on its exact route.
    pub(crate) fn post_native_amx_with_reply_routes(
        &self,
        peer: PeerId,
        reply_routes: Option<NetworkReplyRoutes>,
        message: NativeAmxMessage,
    ) {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            return;
        };
        let route_shape_is_valid = match &message {
            NativeAmxMessage::PrepareRequest(_) | NativeAmxMessage::CommitRequest(_) => {
                reply_routes.is_none()
            }
            NativeAmxMessage::PrepareVote(_) | NativeAmxMessage::CommitVote(_) => {
                reply_routes.is_some()
            }
        };
        if !route_shape_is_valid {
            iroha_logger::error!("Native AMX request/vote has invalid reply-route ownership");
            return;
        }
        let body = match native_amx_message_body(&message) {
            Ok(body)
                if body.round.context_id == self.context.id()
                    && body.round.height == self.context.height =>
            {
                body
            }
            Ok(_) | Err(_) => {
                iroha_logger::error!("Native AMX post has no valid embedded height round");
                return;
            }
        };
        let rollover_claim = ExactOutputRolloverClaim::NativeAmx {
            scope: self.exact_output_scope(),
            round: body.round,
            message_hash: HashOf::new(&message),
        };
        let data = NetworkMessage::NativeAmx(Arc::new(message));
        let result = match reply_routes {
            Some(reply_routes) => self.enqueue_owned_exact_reply_routes_while_guarded(
                data,
                peer,
                reply_routes,
                None,
                rollover_claim,
                operation.permit(),
            ),
            None => self.enqueue_exact_fanout_while_guarded(
                vec![data],
                vec![peer],
                rollover_claim,
                operation.permit(),
            ),
        };
        match result {
            Ok(ExactFanoutOwnership::Owned) => operation.complete(),
            Ok(ExactFanoutOwnership::SourceRetained) => {
                iroha_logger::error!(
                    "Native AMX post reached an unreserved outbound corridor boundary"
                );
            }
            Err(error) => {
                iroha_logger::error!(%error, "Native AMX output failed closed");
            }
        }
    }
    /// Send one exact durably authorized lane-drain vote to a selected peer.
    pub(crate) fn post_lane_drain_vote(&self, peer: PeerId, vote: LaneDrainVoteV1) {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            return;
        };
        if let Err(error) = vote.validate_ingress() {
            iroha_logger::error!(%error, "lane-drain vote output failed validation");
            return;
        }
        let rollover_claim = ExactOutputRolloverClaim::LaneDrainVote {
            scope: self.exact_output_scope(),
            target: peer.clone(),
            vote_hash: HashOf::new(&vote),
        };
        match self.enqueue_exact_fanout_while_guarded(
            vec![NetworkMessage::LaneDrainVote(Box::new(vote))],
            vec![peer],
            rollover_claim,
            operation.permit(),
        ) {
            Ok(ExactFanoutOwnership::Owned) => operation.complete(),
            Ok(ExactFanoutOwnership::SourceRetained) => {
                iroha_logger::error!(
                    "lane-drain vote fanout reached an unreserved outbound corridor boundary"
                );
            }
            Err(error) => {
                iroha_logger::error!(%error, "lane-drain vote output failed closed");
            }
        }
    }
    /// Broadcast one merge signature share to every other frozen voter.
    pub(crate) fn broadcast_merge_to_voters(&self, signature: MergeCommitteeSignature) {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            return;
        };
        let rollover_claim = ExactOutputRolloverClaim::MergeShare {
            scope: self.exact_output_scope(),
            share_hash: HashOf::new(&signature),
        };
        match self.enqueue_exact_fanout_while_guarded(
            vec![NetworkMessage::MergeCommitteeSignature(Arc::new(signature))],
            self.remote_voters(),
            rollover_claim,
            operation.permit(),
        ) {
            Ok(ExactFanoutOwnership::Owned) => operation.complete(),
            Ok(ExactFanoutOwnership::SourceRetained) => {
                iroha_logger::error!(
                    "merge-share fanout reached an unreserved outbound corridor boundary"
                );
            }
            Err(error) => {
                iroha_logger::error!(%error, "merge-share output failed closed");
            }
        }
    }
    fn post_block_message_while_guarded(
        &self,
        peer: PeerId,
        message: BlockMessage,
        _permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        let rollover_claim = match &message {
            BlockMessage::V2(_) => ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
            BlockMessage::LaneHistoricalRecoveryRequest(request) => {
                ExactOutputRolloverClaim::HistoricalLaneRecoveryRequest {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    request_hash: HashOf::new(request.as_ref()),
                }
            }
            BlockMessage::LaneHistoricalRecoveryResponse(response) => {
                ExactOutputRolloverClaim::HistoricalLaneRecoveryResponse {
                    scope: self.exact_output_scope(),
                    target: peer.clone(),
                    request_hash: response.request_hash,
                    response_hash: HashOf::new(response.as_ref()),
                }
            }
            message if message.is_lane_local() => {
                self.current_lane_output_rollover_claim(message, &peer)?
            }
            _ => return Err("guarded v2 output has no typed rollover claim".to_owned()),
        };
        let block_message = Arc::new(message);
        let wire = BlockMessageWire::try_preencoded(block_message).map_err(|error| {
            format!("failed to encode guarded Sumeragi v2 message for {peer}: {error}")
        })?;
        let data = NetworkMessage::SumeragiBlock(Arc::new(wire));
        self.enqueue_exact_fanout_while_guarded(vec![data], vec![peer], rollover_claim, _permit)
    }
    fn post_block_message_on_reply_routes_while_guarded(
        &self,
        peer: PeerId,
        reply_routes: NetworkReplyRoutes,
        ingress_ownership: FairV2IngressOwnershipEvidence,
        message: BlockMessage,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        let rollover_claim = match &message {
            BlockMessage::V2(_) => ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
            BlockMessage::LaneBlockProposal(_)
            | BlockMessage::LaneExecutablePayload(_)
            | BlockMessage::LaneBlockNewViewVote(_)
            | BlockMessage::LaneBlockNewViewCertificate(_)
            | BlockMessage::LaneBlockVote(_)
            | BlockMessage::LaneBlockQc(_)
            | BlockMessage::LaneBlockCertificate(_) => {
                self.current_lane_output_rollover_claim(&message, &peer)?
            }
            _ => return Err("guarded v2 reply has no typed rollover claim".to_owned()),
        };
        let wire = BlockMessageWire::try_preencoded(Arc::new(message)).map_err(|error| {
            format!("failed to encode guarded Sumeragi v2 reply for {peer}: {error}")
        })?;
        self.enqueue_owned_exact_reply_routes_while_guarded(
            NetworkMessage::SumeragiBlock(Arc::new(wire)),
            peer,
            reply_routes,
            Some(ingress_ownership),
            rollover_claim,
            permit,
        )
    }
    fn preencode_v2_network_message(
        message: wire::ConsensusMessageV2,
    ) -> Result<NetworkMessage, String> {
        let wire = BlockMessageWire::try_preencoded(Arc::new(BlockMessage::V2(message)))
            .map_err(|error| format!("failed to encode guarded Sumeragi v2 message: {error}"))?;
        Ok(NetworkMessage::SumeragiBlock(Arc::new(wire)))
    }
    fn broadcast_preencoded_to_voters_while_guarded(
        &self,
        data: &NetworkMessage,
        _permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        self.enqueue_exact_fanout_while_guarded(
            vec![data.clone()],
            self.remote_voters(),
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
            _permit,
        )
    }
    fn broadcast_preencoded_to_archive_peers_while_guarded(
        &self,
        data: &NetworkMessage,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<ExactFanoutOwnership, String> {
        let frozen_sources = self.remote_voters();
        self.enqueue_exact_fanout_while_guarded(
            vec![data.clone()],
            self.current_archive_targets_with_frozen_fallback(&frozen_sources),
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
            permit,
        )
    }
    /// Broadcast historical block-sync discovery to the live authenticated
    /// topology, retaining the immutable height roster as an empty-snapshot
    /// fallback.
    pub(crate) fn broadcast_block_sync_while_guarded(
        &self,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        if !matches!(
            &message.payload,
            wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        ) {
            return Err("historical block-sync broadcast received another payload kind".to_owned());
        }
        let data = Self::preencode_v2_network_message(message)?;
        if self.broadcast_preencoded_to_archive_peers_while_guarded(&data, permit)?
            == ExactFanoutOwnership::SourceRetained
        {
            iroha_logger::debug!("deferred block-sync request to its retained discovery source");
        }
        Ok(())
    }
    /// Broadcast under a caller-owned output permit without reacquiring it.
    pub(crate) fn broadcast_to_voters_while_guarded(
        &self,
        message: wire::ConsensusMessageV2,
        permit: &ConsensusOutputPermit<'_>,
    ) -> Result<(), String> {
        let data = Self::preencode_v2_network_message(message)?;
        if self.broadcast_preencoded_to_voters_while_guarded(&data, permit)?
            == ExactFanoutOwnership::SourceRetained
        {
            iroha_logger::debug!("deferred block-sync request to its retained discovery source");
        }
        Ok(())
    }
}
include!("v2_worker/current_lane_output_rollover_claim.rs");
include!("v2_worker/production_services_drop_impl.rs");
include!("v2_worker/effect_services_impl.rs");
