impl V2EffectServices for ProductionV2Services {
    type Error = String;
    fn finish_runtime_step_reconciliation(
        &mut self,
        decided_subject: Option<wire::BlockSubject>,
    ) -> Result<(), Self::Error> {
        if decided_subject.is_some() {
            let next = self.leader_wire_recovery_authority.with_durable_decision();
            self.leader_wire_ingress
                .advance_leader_wire_recovery_cut(next)?;
            self.leader_wire_recovery_authority = next;
        }
        Ok(())
    }
    fn complete_leader_wire_runtime_terminal(
        &mut self,
        terminal: LeaderWireRuntimeTerminal,
    ) -> Result<(), Self::Error> {
        match terminal {
            LeaderWireRuntimeTerminal::Volatile(runtime) => self
                .leader_wire_ingress
                .mark_leader_wire_volatile_terminal(&runtime),
            LeaderWireRuntimeTerminal::Producer { runtime, terminal } => self
                .leader_wire_ingress
                .mark_leader_wire_producer_terminal(&runtime, terminal),
        }
    }
    fn enqueue_consensus_sign(&mut self, task: ConsensusSignTask) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let restore_outbound_payload = match task.request() {
            super::v2::SignRequest::Proposal(proposal) => !self
                .outbound_chunks
                .contains_key(&HashOf::new(&proposal.manifest)),
            super::v2::SignRequest::Vote(_) | super::v2::SignRequest::TimeoutVote(_) => false,
        };
        let prepared = match task.request() {
            super::v2::SignRequest::Vote(vote) if vote.phase == wire::GlobalPhase::Prepare => {
                Some(PreparedCandidateBody {
                    tag: task.tag(),
                    subject: vote.subject,
                })
            }
            super::v2::SignRequest::Proposal(_)
            | super::v2::SignRequest::Vote(_)
            | super::v2::SignRequest::TimeoutVote(_) => None,
        };
        self.io()?.enqueue(V2IoCommand::Sign {
            task,
            restore_outbound_payload,
        })?;
        if let Some(prepared) = prepared
            && self.prepared_candidates.len() < self.max_orphan_chunks
        {
            self.prepared_candidates.push_back(prepared);
        }
        operation.complete();
        Ok(())
    }
    fn cancel_consensus_sign(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
        self.io()?.cancel(work_id, V2IoCancellableKind::Sign)?;
        Ok(())
    }
    fn retire_outbound_payload_for_subject(
        &mut self,
        subject: wire::BlockSubject,
    ) -> Result<(), Self::Error> {
        self.outbound_chunks
            .retain(|_, retained| retained.subject != subject);
        Ok(())
    }
    fn retire_all_outbound_payloads(&mut self) -> Result<(), Self::Error> {
        self.outbound_chunks.clear();
        Ok(())
    }
    fn retire_candidate_work_after_decision(
        &mut self,
        decision_round: wire::ConsensusRound,
        decision_subject: wire::BlockSubject,
    ) -> Result<(), Self::Error> {
        self.proposal_work_retired = true;
        self.locked_candidate_acquisition = None;
        self.prepared_candidates.clear();
        self.validation_rejections.clear();
        self.merge_sidecar_deferrals.retain(|deferred| {
            deferred.round() == decision_round && deferred.subject() == decision_subject
        });
        Ok(())
    }
    #[allow(clippy::too_many_lines)]
    fn broadcast_consensus(
        &mut self,
        message: wire::ConsensusMessageV2,
    ) -> Result<ConsensusBroadcastDisposition, Self::Error> {
        if self.proposal_work_retired
            && matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::Proposal(_)
            )
        {
            return Err("Sumeragi v2 Proposal output is terminal after Decision".to_owned());
        }
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        message
            .validate_version()
            .map_err(|error| error.to_string())?;
        let control_targets = match &message.payload {
            wire::ConsensusMessageV2Payload::Proposal(_)
            | wire::ConsensusMessageV2Payload::Vote(_)
            | wire::ConsensusMessageV2Payload::QuorumCertificate(_)
            | wire::ConsensusMessageV2Payload::TimeoutVote(_)
            | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
            | wire::ConsensusMessageV2Payload::PayloadManifest(_)
            | wire::ConsensusMessageV2Payload::PayloadChunk(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
            | wire::ConsensusMessageV2Payload::CertifiedBodyResponse(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
            | wire::ConsensusMessageV2Payload::CommitCertificateResponse(_)
            | wire::ConsensusMessageV2Payload::VrfCommit(_)
            | wire::ConsensusMessageV2Payload::VrfReveal(_) => self.remote_voters(),
        };
        if let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload {
            let manifest_hash = HashOf::new(&proposal.manifest);
            let chunks = self
                .outbound_chunks
                .get(&manifest_hash)
                .ok_or_else(|| "local proposal has no retained Sumeragi v2 chunks".to_owned())?;
            if chunks.owner != self.active_tag || chunks.round != proposal.round {
                return Err(
                    "local proposal chunks belong to another reducer incarnation".to_owned(),
                );
            }
            let encoded_chunks = chunks
                .messages
                .iter()
                .cloned()
                .map(Self::preencode_v2_network_message)
                .collect::<Result<Vec<_>, _>>()?;
            let committee = self.committee_for_round(proposal.round)?;
            let first_fast_path_send = !self.fast_path_proposals.contains(&proposal.round);
            let payload_targets = if first_fast_path_send {
                self.remote_voters_for_indices(committee.set_a())?
            } else {
                self.remote_voters()
            };
            let control = PendingExactFanout::claimed(
                vec![Self::preencode_v2_network_message(message.clone())?],
                control_targets,
                ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
            )?;
            let chunks = PendingExactFanout::claimed(
                encoded_chunks,
                payload_targets,
                ExactOutputRolloverClaim::PayloadChunks {
                    scope: self.exact_output_scope(),
                    manifest: proposal.manifest.clone(),
                },
            )?;
            let ownership = self.enqueue_atomic_fanout_batch_while_guarded(
                control.into_iter().chain(chunks).collect(),
                operation.permit(),
            )?;
            if ownership == ExactFanoutOwnership::Owned && first_fast_path_send {
                self.fast_path_proposals.insert(proposal.round);
            }
            if ownership == ExactFanoutOwnership::SourceRetained {
                iroha_logger::debug!(
                    "deferred atomic Sumeragi v2 Proposal control/chunk fanout to reducer retransmission"
                );
            }
            operation.complete();
            return Ok(if ownership == ExactFanoutOwnership::SourceRetained {
                ConsensusBroadcastDisposition::SourceRetained
            } else {
                ConsensusBroadcastDisposition::ExactServiceAccepted
            });
        }
        let control = vec![Self::preencode_v2_network_message(message)?];
        let source_retained = self.enqueue_exact_fanout_while_guarded(
            control,
            control_targets,
            ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
            operation.permit(),
        )? == ExactFanoutOwnership::SourceRetained;
        if source_retained {
            iroha_logger::debug!("deferred Sumeragi v2 control fanout to reducer retransmission");
        }
        operation.complete();
        Ok(if source_retained {
            ConsensusBroadcastDisposition::SourceRetained
        } else {
            ConsensusBroadcastDisposition::ExactServiceAccepted
        })
    }
    fn sign_body_request(&mut self, preimage: &[u8]) -> Result<Vec<u8>, Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let signature = Signature::try_new(self.key_pair.private_key(), preimage)
            .map(|signature| signature.payload().to_vec())
            .map_err(|error| error.to_string())?;
        operation.complete();
        Ok(signature)
    }
    fn enqueue_body_fetch(&mut self, task: BodyFetchTask) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard.begin_fail_stop_operation().ok_or_else(|| {
            "Sumeragi v2 canonical persistence requires restart recovery".to_owned()
        })?;
        match self.body_fetch_service_owner(task.id())? {
            BodyFetchServiceOwner::Reconstructed(index) => {
                let LocalCompletion::Reconstructed {
                    task: queued_task, ..
                } = self
                    .local_completions
                    .get(index)
                    .expect("queued reconstruction owner was classified above");
                if task != *queued_task && !task.monotonically_extends(queued_task) {
                    return Err(format!(
                        "conflicting Sumeragi v2 body-fetch retransmission for completed work {}",
                        task.id().get()
                    ));
                }
                let LocalCompletion::Reconstructed {
                    task: queued_task, ..
                } = self
                    .local_completions
                    .get_mut(index)
                    .expect("queued reconstruction owner was classified above");
                *queued_task = task;
                operation.complete();
                return Ok(());
            }
            BodyFetchServiceOwner::Live => {
                let existing_task = self
                    .fetches
                    .get(&task.id())
                    .map(|fetch| fetch.task.clone())
                    .ok_or_else(|| {
                        "classified Sumeragi v2 body-fetch owner disappeared".to_owned()
                    })?;
                if task != existing_task && !task.monotonically_extends(&existing_task) {
                    return Err("conflicting Sumeragi v2 body-fetch task".to_owned());
                }
                let manifest_upgrade =
                    existing_task.manifest().is_none() && task.manifest().is_some();
                let manifest_hash = manifest_upgrade.then(|| {
                    HashOf::new(task.manifest().expect("manifest upgrade was checked above"))
                });
                if manifest_hash.is_some_and(|hash| self.fetch_by_manifest.contains_key(&hash)) {
                    return Err("duplicate Sumeragi v2 fetch manifest".to_owned());
                }
                let certified_message = task
                    .certified_request()
                    .map(|request| {
                        Self::preencode_v2_network_message(wire::ConsensusMessageV2::new(
                            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
                        ))
                    })
                    .transpose()?;
                let certified_sources = certified_message
                    .as_ref()
                    .map(|_| task.sources().to_vec())
                    .unwrap_or_default();
                let opened_chunks = manifest_upgrade
                    .then(|| {
                        V2ChunkSession::open(
                            &self.chunk_root,
                            &self.context,
                            task.manifest()
                                .expect("manifest upgrade was checked above")
                                .clone(),
                        )
                    })
                    .transpose()
                    .map_err(|error| error.to_string())?;
                let fetch = self.fetches.get_mut(&task.id()).ok_or_else(|| {
                    "preflighted Sumeragi v2 body-fetch owner disappeared".to_owned()
                })?;
                if let (Some(chunks), Some(manifest_hash)) = (opened_chunks, manifest_hash) {
                    self.fetch_by_manifest.insert(manifest_hash, task.id());
                    fetch.chunks = Some(chunks);
                }
                fetch.task = task;
                let fetch_work_id = fetch.task.id();
                if let Some(data) = certified_message {
                    let peers = certified_sources
                        .into_iter()
                        .filter(|peer| peer != &self.local_peer)
                        .collect();
                    if self.enqueue_exact_fanout_while_guarded(
                        vec![data],
                        peers,
                        ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
                        operation.permit(),
                    )? == ExactFanoutOwnership::SourceRetained
                    {
                        iroha_logger::debug!(
                            work_id = fetch_work_id.get(),
                            "deferred certified body request to retained fetch ownership"
                        );
                    }
                }
                operation.complete();
                return Ok(());
            }
            BodyFetchServiceOwner::None => {}
        }
        if task.manifest().is_none() && task.certified_request().is_none() {
            return Err("Sumeragi v2 body-fetch task has no acquisition authority".to_owned());
        }
        let manifest_hash = task.manifest().map(HashOf::new);
        if manifest_hash.is_some_and(|hash| self.fetch_by_manifest.contains_key(&hash)) {
            return Err("duplicate Sumeragi v2 fetch manifest".to_owned());
        }
        let certified_message = task
            .certified_request()
            .map(|request| {
                Self::preencode_v2_network_message(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
                ))
            })
            .transpose()?;
        let certified_sources = certified_message
            .as_ref()
            .map(|_| task.sources().to_vec())
            .unwrap_or_default();
        let chunks = task
            .manifest()
            .cloned()
            .map(|manifest| V2ChunkSession::open(&self.chunk_root, &self.context, manifest))
            .transpose()
            .map_err(|error| error.to_string())?;
        if let Some(hash) = manifest_hash {
            self.fetch_by_manifest.insert(hash, task.id());
        }
        let work_id = task.id();
        self.fetches.insert(work_id, FetchSession { task, chunks });
        if let Some(data) = certified_message {
            let peers = certified_sources
                .into_iter()
                .filter(|peer| peer != &self.local_peer)
                .collect();
            if self.enqueue_exact_fanout_while_guarded(
                vec![data],
                peers,
                ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope()),
                operation.permit(),
            )? == ExactFanoutOwnership::SourceRetained
            {
                iroha_logger::debug!(
                    work_id = work_id.get(),
                    "deferred certified body request to retained fetch ownership"
                );
            }
        }
        operation.complete();
        Ok(())
    }
    fn rebind_body_fetch(
        &mut self,
        previous: &BodyFetchTask,
        rebound: BodyFetchTask,
    ) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard.begin_fail_stop_operation().ok_or_else(|| {
            "Sumeragi v2 canonical persistence requires restart recovery".to_owned()
        })?;
        if !rebound.rebinds_consumer_of(previous) {
            return Err(format!(
                "Sumeragi v2 body-fetch work {} has an invalid consumer rebind",
                previous.id().get()
            ));
        }
        match self.body_fetch_service_owner(previous.id())? {
            BodyFetchServiceOwner::Live => {
                let fetch = self
                    .fetches
                    .get_mut(&previous.id())
                    .expect("live body-fetch owner was classified above");
                if fetch.task != *previous {
                    return Err(format!(
                        "Sumeragi v2 body-fetch work {} differs from live service ownership",
                        previous.id().get()
                    ));
                }
                fetch.task = rebound;
            }
            BodyFetchServiceOwner::Reconstructed(index) => {
                let LocalCompletion::Reconstructed { task, .. } = self
                    .local_completions
                    .get_mut(index)
                    .expect("queued body-fetch owner was classified above");
                if task != previous {
                    return Err(format!(
                        "Sumeragi v2 body-fetch work {} differs from queued completion ownership",
                        previous.id().get()
                    ));
                }
                *task = rebound;
            }
            BodyFetchServiceOwner::None => {
                return Err(format!(
                    "Sumeragi v2 body-fetch work {} has no service owner to rebind",
                    previous.id().get()
                ));
            }
        }
        operation.complete();
        Ok(())
    }
    fn cancel_body_fetch(&mut self, task: &BodyFetchTask) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.remove_exact_body_fetch_owner(task)?;
        operation.complete();
        Ok(())
    }
    fn complete_body_reconstruction_fetch(
        &mut self,
        task: &BodyFetchTask,
    ) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        self.remove_exact_body_fetch_owner(task)?;
        operation.complete();
        Ok(())
    }
    #[cfg(test)]
    fn complete_certified_body_fetch(
        &mut self,
        task: &BodyFetchTask,
    ) -> Result<CertifiedBodyFetchCompletionDisposition, Self::Error> {
        // Complete every fallible ownership check before arming the fail-stop
        // boundary. The guarded tail is then one infallible removal, so every
        // returned error leaves the exact service owner byte-for-byte intact.
        let output_guard = Arc::clone(&self.output_guard);
        let prepared = self.prepare_certified_body_fetch_owner_removal(task)?;
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let disposition = prepared.commit(operation.permit());
        operation.complete();
        Ok(disposition)
    }
    fn accept_authenticated_chunk(
        &mut self,
        task: &BodyFetchTask,
        chunk: AuthenticatedPayloadChunk,
    ) -> Result<AuthenticatedChunkDisposition, Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if self.body_fetch_service_owner(task.id())? != BodyFetchServiceOwner::Live {
            return Err("Sumeragi v2 chunk fetch has no exact live owner".to_owned());
        }
        let reconstruction = {
            let fetch = self
                .fetches
                .get_mut(&task.id())
                .expect("live body-fetch owner was classified above");
            if fetch.task != *task {
                return Err(format!(
                    "Sumeragi v2 chunk task {} differs from service ownership",
                    task.id().get()
                ));
            }
            let session = fetch.chunks.as_mut().ok_or_else(|| {
                "manifest-less certified body fetch cannot accept chunks".to_owned()
            })?;
            session
                .admit(chunk.chunk())
                .map_err(|error| error.to_string())?;
            session.reconstruct()
        };
        let body = match reconstruction {
            Ok(Some(body)) => body,
            Ok(None) => {
                operation.complete();
                return Ok(AuthenticatedChunkDisposition::Accepted);
            }
            Err(V2ChunkError::PayloadMismatch | V2ChunkError::ReconstructionFailed) => {
                operation.complete();
                return Ok(AuthenticatedChunkDisposition::Rejected);
            }
            Err(error) => return Err(error.to_string()),
        };
        let manifest = task
            .manifest()
            .expect("chunk reconstruction requires proposal manifest authority")
            .clone();
        let canonical_manifest =
            encode_payload(&self.context, manifest.round, manifest.subject, &body)
                .map_err(|error| error.to_string())?
                .manifest()
                .clone();
        if canonical_manifest != manifest {
            operation.complete();
            return Ok(AuthenticatedChunkDisposition::Rejected);
        }
        if self.body_fetch_service_owner(task.id())? != BodyFetchServiceOwner::Live {
            return Err("Sumeragi v2 reconstructed fetch lost its exact live owner".to_owned());
        }
        let removed = self.fetch_by_manifest.remove(&HashOf::new(&manifest));
        if removed != Some(task.id()) {
            return Err(format!(
                "Sumeragi v2 reconstructed work {} lost its manifest index",
                task.id().get()
            ));
        }
        let fetch = self
            .fetches
            .remove(&task.id())
            .expect("live body-fetch owner was classified above");
        if fetch.task != *task {
            return Err(format!(
                "Sumeragi v2 reconstructed work {} changed task ownership",
                task.id().get()
            ));
        }
        self.local_completions
            .push_back(LocalCompletion::Reconstructed {
                task: fetch.task,
                manifest,
                body: body.into(),
            });
        operation.complete();
        Ok(AuthenticatedChunkDisposition::Accepted)
    }
    fn enqueue_body_store(&mut self, task: BodyStoreTask) -> Result<(), Self::Error> {
        self.enqueue_fail_stop_io(V2IoCommand::Store(task))
    }
    fn cancel_body_store(&mut self, work_id: EffectWorkId) -> Result<bool, Self::Error> {
        self.io()?.cancel(work_id, V2IoCancellableKind::Store)
    }
    fn enqueue_body_validation(&mut self, task: BodyValidationTask) -> Result<(), Self::Error> {
        self.enqueue_fail_stop_io(V2IoCommand::Validate(task))
    }
    fn cancel_body_validation(&mut self, work_id: EffectWorkId) -> Result<(), Self::Error> {
        self.io()?.cancel(work_id, V2IoCancellableKind::Validate)?;
        Ok(())
    }
    fn work_deferred_for_merge_sidecar(
        &mut self,
        work_id: EffectWorkId,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        reference: &CertifiedMergeLedgerReference,
    ) -> Result<(), Self::Error> {
        self.requeue_merge_sidecar_deferral(DeferredMergeSidecarWork {
            work_id,
            round,
            subject,
            reference: reference.clone(),
        })
    }
    fn enqueue_apply(&mut self, task: ApplyTask) -> Result<(), Self::Error> {
        self.enqueue_fail_stop_io(V2IoCommand::Apply(task))
    }
    fn entered_view(
        &mut self,
        tag: EventTag,
        certificate: wire::TimeoutCertificate,
    ) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        if tag.height() != self.context.height
            || certificate.round.context_id != self.context.id()
            || certificate.round.height != self.context.height
            || certificate.round.view.checked_add(1) != Some(tag.view())
            || !tag.strictly_advances(self.active_tag)
        {
            return Err(
                "Sumeragi v2 service rejected non-monotonic certified view ownership".to_owned(),
            );
        }
        let next_recovery_authority = self
            .leader_wire_recovery_authority
            .advance_view(tag.view())?;
        self.leader_wire_ingress
            .advance_leader_wire_recovery_cut(next_recovery_authority)?;
        self.leader_wire_recovery_authority = next_recovery_authority;
        // The old view's active Sign command may still complete after its
        // executor owner is cancelled. Prune first and publish the new owner
        // second; completion handling classifies the old work ID before it is
        // ever allowed to restore payload bytes.
        self.outbound_chunks.clear();
        self.fast_path_proposals.clear();
        self.active_tag = tag;
        iroha_logger::debug!(
            height = tag.height(),
            view = tag.view(),
            generation = tag.generation().get(),
            "installed certified Sumeragi v2 view"
        );
        Ok(())
    }
    fn report_equivocation(
        &mut self,
        evidence: wire::SumeragiV2Equivocation,
    ) -> Result<(), Self::Error> {
        let _permit = self.output_permit()?;
        if self.state.network_id_ref() != &self.context.network_id {
            return Err(
                "Sumeragi v2 equivocation context is not anchored to the active network".to_owned(),
            );
        }
        let inserted = super::evidence::persist_sumeragi_v2_equivocation(
            self.state.as_ref(),
            &self.context,
            &self.validator_set_pops,
            evidence.clone(),
        )
        .map_err(|error| format!("invalid Sumeragi v2 equivocation evidence: {error:?}"))?;
        if inserted {
            iroha_logger::warn!(
                ?evidence,
                "persisted authenticated Sumeragi v2 equivocation evidence"
            );
        }
        Ok(())
    }
    fn report_invalid_certified_body(
        &mut self,
        subject: wire::BlockSubject,
        certificate: wire::QuorumCertificate,
    ) -> Result<(), Self::Error> {
        let _permit = self.output_permit()?;
        iroha_logger::error!(
            ?subject,
            ?certificate,
            "invalid body certified by Sumeragi v2 PrepareQC"
        );
        Ok(())
    }
    fn validation_rejected(
        &mut self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        reason: &str,
    ) {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(_permit) = output_guard.acquire() else {
            return;
        };
        if self.validation_rejections.len() < self.max_orphan_chunks {
            self.validation_rejections.push_back(RejectedCandidateBody {
                round,
                subject,
                reason: reason.to_owned(),
            });
        }
        iroha_logger::warn!(
            ?round,
            ?subject,
            reason,
            "Sumeragi v2 proposal validation rejected"
        );
    }
    fn publish_effect_status(&mut self, status: &EffectExecutorStatus) -> Result<(), Self::Error> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or_else(|| "Sumeragi v2 consensus requires process restart".to_owned())?;
        let mut status = status.clone();
        status.pending_candidate_loads = self
            .locked_candidate_acquisition
            .as_ref()
            .map_or(0, LockedCandidateAcquisition::pending_count);
        let captured_at = status.captured_at;
        status.effect_completion_queue = self.io.as_ref().map_or(
            RuntimeQueueLaneSnapshot {
                depth: 0,
                capacity: 1,
                oldest_age: None,
                max_service_debt: 0,
            },
            |io| io.completion_snapshot(captured_at),
        );
        let recovery_changed = self.last_status.as_ref().is_none_or(|previous| {
            previous.pending_tip_recovery_stage != status.pending_tip_recovery_stage
                || previous.pending_tip_recovery_last_result
                    != status.pending_tip_recovery_last_result
        });
        if recovery_changed && status.pending_tip_recovery_stage.is_some() {
            match status.pending_tip_recovery_last_result {
                Some(PendingTipRecoveryAttemptResult::Completed) => {
                    iroha_logger::info!(
                        height = status.height,
                        stage = ?status.pending_tip_recovery_stage,
                        attempts = status.pending_tip_recovery_attempts,
                        result = ?status.pending_tip_recovery_last_result,
                        "completed bounded Sumeragi v2 interrupted-tip recovery"
                    );
                }
                Some(PendingTipRecoveryAttemptResult::DeadlineExceeded) => {
                    iroha_logger::warn!(
                        height = status.height,
                        stage = ?status.pending_tip_recovery_stage,
                        attempts = status.pending_tip_recovery_attempts,
                        result = ?status.pending_tip_recovery_last_result,
                        "exhausted bounded Sumeragi v2 interrupted-tip recovery"
                    );
                }
                _ => {
                    iroha_logger::debug!(
                        height = status.height,
                        stage = ?status.pending_tip_recovery_stage,
                        attempts = status.pending_tip_recovery_attempts,
                        result = ?status.pending_tip_recovery_last_result,
                        "advanced bounded Sumeragi v2 interrupted-tip recovery"
                    );
                }
            }
        }
        self.last_status = Some(status.clone());
        super::status::set_v2_effect_status(status);
        Ok(())
    }
    fn fail_closed(&mut self, reason: &str) {
        self.output_guard.activate_restart_required();
        self.fatal_reason = Some(reason.to_owned());
        iroha_logger::error!(reason, "Sumeragi v2 effect services failed closed");
    }
}

/// Recover the exact global round carried by one view-scoped v2 output.
///
/// Height-only recovery requests and epoch-wide VRF traffic deliberately
/// return `None`: a certified view does not supersede those owners.
fn global_v2_output_round(message: &NetworkMessage) -> Option<wire::ConsensusRound> {
    let NetworkMessage::SumeragiBlock(envelope) = message else {
        return None;
    };
    let BlockMessage::V2(message) = envelope.as_message() else {
        return None;
    };
    match &message.payload {
        wire::ConsensusMessageV2Payload::Proposal(proposal) => Some(proposal.round),
        wire::ConsensusMessageV2Payload::Vote(vote) => Some(vote.round),
        wire::ConsensusMessageV2Payload::QuorumCertificate(certificate) => Some(certificate.round),
        wire::ConsensusMessageV2Payload::TimeoutVote(vote) => Some(vote.round),
        wire::ConsensusMessageV2Payload::TimeoutCertificate(certificate) => Some(certificate.round),
        wire::ConsensusMessageV2Payload::PayloadManifest(manifest) => Some(manifest.round),
        wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request) => Some(request.round),
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) => {
            Some(response.manifest.round)
        }
        wire::ConsensusMessageV2Payload::CommitCertificateResponse(response) => {
            Some(response.certificate.round)
        }
        wire::ConsensusMessageV2Payload::PayloadChunk(_)
        | wire::ConsensusMessageV2Payload::CommitCertificateRequest(_)
        | wire::ConsensusMessageV2Payload::VrfCommit(_)
        | wire::ConsensusMessageV2Payload::VrfReveal(_) => None,
    }
}
impl PendingExactFanout {
    fn is_global_pacemaker_fanout(&self) -> bool {
        !self.messages.is_empty()
            && self.messages.iter().all(|message| {
                let NetworkMessage::SumeragiBlock(envelope) = message else {
                    return false;
                };
                let BlockMessage::V2(message) = envelope.as_message() else {
                    return false;
                };
                matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::TimeoutVote(_)
                        | wire::ConsensusMessageV2Payload::TimeoutCertificate(_)
                )
            })
    }
    /// Whether a retained topology send already owns this exact retry.
    ///
    /// Reducer and lane-work retransmission retain their source authority until
    /// the protocol progresses. Periodic retries therefore reuse an identical
    /// worker owner instead of allocating an unbounded fanout for a silent
    /// peer. Once the incumbent drains, a later retry creates a fresh owner.
    fn can_coalesce_exact_topology_retry(&self, candidate: &Self) -> bool {
        self.rollover_claim == candidate.rollover_claim
            && self.message_hashes == candidate.message_hashes
            && self.semantic_peers() == candidate.semantic_peers()
            && self.reply_routes.is_none()
            && candidate.reply_routes.is_none()
            && self.ingress_ownership.is_none()
            && candidate.ingress_ownership.is_none()
            && self
                .targets
                .iter()
                .chain(&candidate.targets)
                .all(|target| matches!(&target.route, ExactTargetRoute::Topology))
    }
}
impl PendingExactOutput {
    fn retain_native_amx_round(
        &mut self,
        retained_round: wire::ConsensusRound,
        terminal: bool,
        expected_requests: &BTreeMap<NativeAmxAttestationBodyV2, BTreeSet<PeerId>>,
    ) -> Result<usize, String> {
        self.remove_fanouts_matching(
            |fanout| {
                let ExactOutputRolloverClaim::NativeAmx { round, .. } = &fanout.rollover_claim
                else {
                    return false;
                };
                if terminal || *round != retained_round {
                    return true;
                }
                let [NetworkMessage::NativeAmx(message)] = fanout.messages.as_slice() else {
                    return false;
                };
                let body = match message.as_ref() {
                    NativeAmxMessage::PrepareRequest(request) => Some(request.body),
                    NativeAmxMessage::CommitRequest(request) => Some(request.request.body),
                    NativeAmxMessage::PrepareVote(_) | NativeAmxMessage::CommitVote(_) => None,
                };
                body.is_some_and(|body| {
                    let peers = fanout.semantic_peers();
                    match peers.as_slice() {
                        [peer] => !expected_requests
                            .get(&body)
                            .is_some_and(|expected| expected.contains(peer)),
                        _ => true,
                    }
                })
            },
            |fanout| {
                fanout
                    .rollover_claim
                    .validate_fanout(&fanout.messages, &fanout.semantic_peers())
            },
            "Native AMX round retirement",
        )
    }
    /// Retire global control, payload, and merge-share fanouts made obsolete
    /// by a certified higher view.
    ///
    /// A dead topology target can retain old-view output indefinitely. Without
    /// this cut, repeated view changes consume the finite shared ownership
    /// pool until a current Proposal or timeout fanout can no longer reach any
    /// responsive peer. Every removed message is bound to this exact height
    /// context and a strictly lower view; height-only and epoch-wide traffic is
    /// retained.
    fn retain_certified_global_view_output(
        &mut self,
        retained_round: wire::ConsensusRound,
    ) -> Result<usize, String> {
        self.remove_fanouts_matching(
            |fanout| {
                let scope_matches = |scope: &ExactOutputCreationScope| {
                    scope.context_id == retained_round.context_id
                        && scope.height == retained_round.height
                };
                match &fanout.rollover_claim {
                    ExactOutputRolloverClaim::GlobalV2(scope) => {
                        scope_matches(scope)
                            && !fanout.messages.is_empty()
                            && fanout.messages.iter().all(|message| {
                                global_v2_output_round(message).is_some_and(|round| {
                                    round.context_id == retained_round.context_id
                                        && round.height == retained_round.height
                                        && round.view < retained_round.view
                                })
                            })
                    }
                    ExactOutputRolloverClaim::PayloadChunks { scope, manifest } => {
                        scope_matches(scope)
                            && manifest.round.context_id == retained_round.context_id
                            && manifest.round.height == retained_round.height
                            && manifest.round.view < retained_round.view
                    }
                    ExactOutputRolloverClaim::MergeShare { scope, .. } => {
                        scope_matches(scope)
                            && matches!(
                                fanout.messages.as_slice(),
                                [NetworkMessage::MergeCommitteeSignature(signature)]
                                    if signature.view < retained_round.view
                            )
                    }
                    _ => false,
                }
            },
            |fanout| {
                fanout
                    .rollover_claim
                    .validate_fanout(&fanout.messages, &fanout.semantic_peers())
            },
            "global v2 certified-view retirement",
        )
    }
}
impl ProductionV2Services {
    /// Retain only Native-AMX output owned by the active certified round.
    ///
    /// A terminal Decision retires every height-local Native-AMX occurrence.
    /// Matching by the complete round also removes stale predecessor-height
    /// output carried through the exact-output rollover corridor.
    pub(crate) fn retain_native_amx_round(
        &self,
        retained_round: wire::ConsensusRound,
        terminal: bool,
        expected_requests: &BTreeMap<NativeAmxAttestationBodyV2, BTreeSet<PeerId>>,
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.retain_native_amx_round(retained_round, terminal, expected_requests)
    }
    /// Retire view-scoped global control, payload, and merge output below the
    /// active certified round.
    pub(crate) fn retain_certified_global_view_output(
        &self,
        retained_round: wire::ConsensusRound,
    ) -> Result<usize, String> {
        let mut pending = self.lock_pending_exact_output()?;
        if self.exact_output_handoff_owner.is_sealed() {
            debug_assert!(!pending.is_pending());
            return Ok(0);
        }
        pending.retain_certified_global_view_output(retained_round)
    }
}
