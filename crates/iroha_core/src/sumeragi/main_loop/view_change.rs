//! View-change (`NEW_VIEW` / proofs / evidence) message handlers.

use iroha_logger::prelude::*;

use super::*;

#[derive(Debug)]
pub(super) enum ViewChangeProgress {
    Advanced { next_index: usize },
    NoChange,
}

pub(super) fn merge_view_change_proof(
    chain: &mut crate::sumeragi::view_change::ProofChain,
    proof: crate::sumeragi::view_change::SignedViewChangeProof,
    topology: &super::network_topology::Topology,
    latest_block_hash: HashOf<BlockHeader>,
) -> Result<ViewChangeProgress, crate::sumeragi::view_change::Error> {
    let baseline = chain.verify_with_state(topology, latest_block_hash);
    if let Err(err) = chain.insert_proof(proof, topology, latest_block_hash) {
        return match err {
            crate::sumeragi::view_change::Error::ViewChangeOutdated => {
                Ok(ViewChangeProgress::NoChange)
            }
            other => Err(other),
        };
    }
    let advanced = chain.verify_with_state(topology, latest_block_hash);
    if advanced > baseline {
        Ok(ViewChangeProgress::Advanced {
            next_index: advanced,
        })
    } else {
        Ok(ViewChangeProgress::NoChange)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ViewChangeProofOutcome {
    Accepted { next_index: usize },
    Stale,
    Rejected,
}

pub(super) fn note_view_change_proof_status(outcome: &ViewChangeProofOutcome) {
    use ViewChangeProofOutcome as Outcome;

    match outcome {
        Outcome::Accepted { next_index } => {
            let next_index = u64::try_from(*next_index).unwrap_or(u64::MAX);
            super::status::set_view_change_index(next_index);
            super::status::inc_view_change_proof_accepted();
            super::status::inc_view_change_install();
        }
        Outcome::Stale => super::status::inc_view_change_proof_stale(),
        Outcome::Rejected => super::status::inc_view_change_proof_rejected(),
    }
}

#[cfg(feature = "telemetry")]
fn note_view_change_proof_telemetry(outcome: &ViewChangeProofOutcome, telemetry: &Telemetry) {
    use ViewChangeProofOutcome as Outcome;

    match outcome {
        Outcome::Accepted { .. } => {
            telemetry.inc_view_change_proof_accepted();
            telemetry.inc_view_change_install();
        }
        Outcome::Stale => telemetry.inc_view_change_proof_stale(),
        Outcome::Rejected => telemetry.inc_view_change_proof_rejected(),
    }
}

impl Actor {
    #[allow(clippy::too_many_lines)]
    pub(super) fn handle_new_view(
        &mut self,
        frame: crate::sumeragi::consensus::NewView,
    ) -> Result<()> {
        let topology_peers = self.effective_commit_topology();
        if topology_peers.is_empty() {
            debug!(
                height = frame.height,
                view = frame.view,
                "dropping NEW_VIEW: empty commit topology"
            );
            return Ok(());
        }
        let committed_qc = self.latest_committed_qc();
        let committed_height = committed_qc.as_ref().map_or_else(
            || {
                let view_snapshot = self.state.view();
                view_snapshot.height() as u64
            },
            |qc| qc.height,
        );
        let active_height =
            super::active_round_height(self.highest_qc, committed_qc, committed_height);
        if frame.height < active_height {
            debug!(
                height = frame.height,
                view = frame.view,
                active_height,
                "dropping NEW_VIEW: height below active round"
            );
            return Ok(());
        }
        if let Err(err) =
            validate_new_view_signature(&self.chain_id, self.mode_tag(), &topology_peers, &frame)
        {
            warn!(
                ?err,
                height = frame.height,
                view = frame.view,
                sender = frame.sender,
                "rejecting NEW_VIEW frame due to failed signature validation"
            );
            return Ok(());
        }
        let topology = super::network_topology::Topology::new(topology_peers.clone());
        let (qc_prf_seed, leader_prf_seed) = if matches!(self.consensus_mode, ConsensusMode::Npos) {
            let view = self.state.view();
            (
                Some(super::npos_seed_for_height(&view, frame.highest_qc.height)),
                Some(super::npos_seed_for_height(&view, frame.height)),
            )
        } else {
            (None, None)
        };
        let topology_len = topology.as_ref().len();
        let is_genesis_stub = self.is_genesis_qc_stub(&frame.highest_qc, topology_len);
        let qc_topology = if is_genesis_stub {
            None
        } else {
            let qc_roster =
                self.roster_for_vote(frame.highest_qc.subject_block_hash, frame.highest_qc.height);
            if qc_roster.is_empty() {
                warn!(
                    height = frame.height,
                    view = frame.view,
                    highest_height = frame.highest_qc.height,
                    highest_view = frame.highest_qc.view,
                    "dropping NEW_VIEW: empty roster for highest QC"
                );
                return Ok(());
            }
            Some(super::network_topology::Topology::new(qc_roster))
        };
        let (validation, evidence) = if is_genesis_stub {
            debug!(
                height = frame.height,
                view = frame.view,
                highest_height = frame.highest_qc.height,
                highest_view = frame.highest_qc.view,
                "accepting NEW_VIEW with genesis QC stub"
            );
            (
                Ok(QcValidationOutcome {
                    signers: Vec::new(),
                    missing_votes: 0,
                    present_signers: 0,
                }),
                None,
            )
        } else {
            let qc_topology = qc_topology.as_ref().expect("QC topology present");
            validate_qc_with_evidence(
                &self.vote_log,
                &frame.highest_qc,
                qc_topology,
                &self.common_config.chain,
                self.mode_tag(),
                qc_prf_seed,
            )
        };
        let validation = match validation {
            Ok(outcome) => outcome,
            Err(err) => {
                let qc_topology = qc_topology.as_ref().expect("QC topology present");
                if let Some(outcome) =
                    self.recover_qc_from_aggregate(&frame.highest_qc, qc_topology, &err)
                {
                    outcome
                } else {
                    record_qc_validation_error(self.telemetry_handle(), &err);
                    if let Some(evidence) = evidence {
                        let _ = self.handle_evidence(evidence);
                    }
                    warn!(
                        ?err,
                        height = frame.height,
                        view = frame.view,
                        sender = frame.sender,
                        highest_height = frame.highest_qc.height,
                        highest_view = frame.highest_qc.view,
                        "dropping NEW_VIEW: highest QC is not verifiable"
                    );
                    return Ok(());
                }
            }
        };
        debug_assert_eq!(
            validation.missing_votes, 0,
            "QC validation should fail when votes are missing"
        );
        if !new_view_highest_qc_phase_valid(&frame.highest_qc) {
            warn!(
                phase = ?frame.highest_qc.phase,
                height = frame.highest_qc.height,
                view = frame.highest_qc.view,
                sender = frame.sender,
                "dropping NEW_VIEW: highest QC must be a precommit QC"
            );
            return Ok(());
        }
        let qc_ref = Self::qc_to_header_ref(&frame.highest_qc);
        let Some(expected_height) = qc_ref.height.checked_add(1) else {
            warn!(
                height = frame.height,
                view = frame.view,
                sender = frame.sender,
                highest_height = qc_ref.height,
                highest_view = qc_ref.view,
                "dropping NEW_VIEW: highest QC height overflow"
            );
            return Ok(());
        };
        if frame.height != expected_height {
            warn!(
                height = frame.height,
                view = frame.view,
                sender = frame.sender,
                highest_height = qc_ref.height,
                highest_view = qc_ref.view,
                expected_height,
                "dropping NEW_VIEW: height does not match highest QC"
            );
            return Ok(());
        }
        let local_view = self.phase_tracker.current_view(frame.height);
        if let Err(err) = check_new_view_freshness(local_view, self.locked_qc, frame.view, qc_ref) {
            match err {
                NewViewFreshnessError::StaleView { local_view } => {
                    debug!(
                        height = frame.height,
                        view = frame.view,
                        local_view,
                        sender = frame.sender,
                        "dropping NEW_VIEW: view regressed relative to local round"
                    );
                    if !self.is_genesis_qc_stub(&frame.highest_qc, topology_len) {
                        if let Err(err) = self.handle_qc(frame.highest_qc.clone()) {
                            warn!(
                                ?err,
                                height = frame.height,
                                view = frame.view,
                                highest_height = qc_ref.height,
                                highest_view = qc_ref.view,
                                "failed to process highest QC from stale NEW_VIEW"
                            );
                        }
                    }
                }
                NewViewFreshnessError::LockedQcConflict(reason) => {
                    let reason = *reason;
                    warn!(
                        ?reason,
                        height = frame.height,
                        view = frame.view,
                        sender = frame.sender,
                        highest_height = qc_ref.height,
                        highest_hash = %qc_ref.subject_block_hash,
                        "dropping NEW_VIEW: highest QC conflicts with locked QC"
                    );
                }
            }
            return Ok(());
        }
        if let Some(lock) = self.locked_qc {
            if qc_ref.height < lock.height {
                debug!(
                    height = frame.height,
                    view = frame.view,
                    sender = frame.sender,
                    highest_height = qc_ref.height,
                    highest_hash = %qc_ref.subject_block_hash,
                    locked_height = lock.height,
                    locked_hash = %lock.subject_block_hash,
                    "accepting NEW_VIEW with highest QC below locked QC"
                );
            }
        }
        self.note_new_view_received();
        let now = Instant::now();
        let prior_count = self
            .propose
            .new_view_tracker
            .count(frame.height, frame.view);
        let local_count =
            self.propose
                .new_view_tracker
                .record(frame.height, frame.view, frame.sender, qc_ref);
        let is_new_sender = local_count > prior_count;
        let should_catch_up = local_view.map_or(true, |current| frame.view > current);
        let local_idx = self.local_validator_index_for_topology(&topology);
        if should_catch_up {
            self.phase_tracker
                .on_view_change(frame.height, frame.view, now);
            self.propose
                .new_view_tracker
                .drop_below_view(frame.height, frame.view);
            if let Some((forced_height, forced_view)) = self.propose.forced_view_after_timeout {
                if forced_height == frame.height && forced_view < frame.view {
                    self.propose.forced_view_after_timeout = Some((forced_height, frame.view));
                }
            }
            self.maybe_broadcast_new_view(qc_ref, Some(frame.height), Some(frame.view));
        }
        if is_new_sender && local_idx != Some(frame.sender) {
            let sender_peer = usize::try_from(frame.sender)
                .ok()
                .and_then(|idx| topology_peers.get(idx));
            let mut targets = self.new_view_gossip_targets_for_view(
                &topology_peers,
                Some(frame.sender),
                self.block_sync_gossip_limit,
                frame.height,
                frame.view,
                0,
            );
            if let Some(leader_peer) = topology_for_view(
                &topology,
                frame.height,
                frame.view,
                self.mode_tag(),
                leader_prf_seed,
            )
            .as_ref()
            .first()
            .cloned()
            {
                let local_peer_id = self.common_config.peer.id();
                if leader_peer != *local_peer_id
                    && sender_peer.map_or(true, |sender| sender != &leader_peer)
                {
                    self.ensure_gossip_target(&mut targets, leader_peer);
                }
            }
            for peer in targets {
                self.schedule_background(BackgroundRequest::PostControlFlow {
                    peer,
                    frame: super::message::ControlFlow::NewView(frame.clone()),
                });
            }
        }
        let required = topology.min_votes_for_commit();
        let count_with_local =
            self.propose
                .new_view_tracker
                .count_with_local(frame.height, frame.view, local_idx);
        let current_view = self.phase_tracker.current_view(frame.height);
        let quorum_reached = count_with_local >= required;
        let advanced = if quorum_reached && current_view != Some(frame.view) {
            if let Some(prev_view) = current_view {
                self.assert_proposal_seen(frame.height, prev_view, "new_view_quorum");
            }
            self.phase_tracker
                .on_view_change(frame.height, frame.view, now);
            self.propose
                .new_view_tracker
                .drop_below_view(frame.height, frame.view);
            self.record_phase_sample(PipelinePhase::Propose, frame.height, frame.view);
            true
        } else {
            if quorum_reached && current_view == Some(frame.view) {
                self.record_phase_sample(PipelinePhase::Propose, frame.height, frame.view);
            }
            false
        };
        let count =
            crate::sumeragi::new_view_stats::note_receipt(frame.height, frame.view, frame.sender);
        #[cfg(feature = "telemetry")]
        {
            self.telemetry.set_view_changes(count);
            self.telemetry
                .set_new_view_receipts(frame.height, frame.view, count);
        }
        #[cfg(not(feature = "telemetry"))]
        let _ = count;
        trace!(
            height = frame.height,
            view = frame.view,
            sender = frame.sender,
            local_count,
            count_with_local,
            required,
            advanced,
            "registered NEW_VIEW frame"
        );
        Ok(())
    }

    #[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
    pub(super) fn handle_evidence(
        &mut self,
        evidence: crate::sumeragi::consensus::Evidence,
    ) -> Result<()> {
        let topology_peers = self.effective_commit_topology();
        if topology_peers.is_empty() {
            debug!("dropping evidence: empty commit topology");
            return Ok(());
        }
        let topology = super::network_topology::Topology::new(topology_peers);
        let (subject_height, _) = super::evidence::evidence_subject_height_view(&evidence);
        let prf_seed = if matches!(self.consensus_mode, ConsensusMode::Npos) {
            let view = self.state.view();
            let height =
                subject_height.unwrap_or_else(|| u64::try_from(view.height()).unwrap_or(0));
            Some(super::npos_seed_for_height(&view, height))
        } else {
            None
        };
        let evidence_context = super::evidence::EvidenceValidationContext {
            topology: &topology,
            chain_id: &self.common_config.chain,
            mode_tag: self.mode_tag(),
            prf_seed,
        };
        if let Err(err) = super::evidence::validate_evidence(&evidence, &evidence_context) {
            debug!(?err, "dropping structurally invalid evidence");
            return Ok(());
        }
        if !self.evidence_is_fresh(&evidence) {
            return Ok(());
        }
        if self.evidence_store.insert(&evidence, &evidence_context) {
            let _ =
                super::evidence::persist_record(self.state.as_ref(), &evidence, &evidence_context);
        }
        Ok(())
    }

    pub(super) fn handle_view_change_proof(
        &mut self,
        proof: crate::sumeragi::view_change::SignedViewChangeProof,
    ) -> Result<()> {
        let view = self.state.view();
        let latest_hash_opt = view.latest_block_hash();
        let topology_peers = self.effective_commit_topology();
        drop(view);

        let Some(latest_hash) = latest_hash_opt else {
            debug!("dropping view-change proof: no committed blocks yet");
            return Ok(());
        };
        if topology_peers.is_empty() {
            debug!(
                ?latest_hash,
                "dropping view-change proof: empty commit topology",
            );
            return Ok(());
        }

        self.view_change_chain.prune(latest_hash);
        let topology = super::network_topology::Topology::new(topology_peers);
        let outcome = match merge_view_change_proof(
            &mut self.view_change_chain,
            proof,
            &topology,
            latest_hash,
        ) {
            Ok(ViewChangeProgress::Advanced { next_index }) => {
                trace!(
                    ?latest_hash,
                    next_index, "accepted view-change proof and advanced chain"
                );
                ViewChangeProofOutcome::Accepted { next_index }
            }
            Ok(ViewChangeProgress::NoChange) => {
                trace!(
                    ?latest_hash,
                    "view-change proof did not advance proof chain (duplicate or outdated)"
                );
                ViewChangeProofOutcome::Stale
            }
            Err(crate::sumeragi::view_change::Error::BlockHashMismatch) => {
                debug!(
                    ?latest_hash,
                    "rejecting view-change proof: latest block hash mismatch"
                );
                ViewChangeProofOutcome::Rejected
            }
            Err(crate::sumeragi::view_change::Error::InvalidProof(err)) => {
                warn!(?err, "rejecting view-change proof: invalid signatures");
                ViewChangeProofOutcome::Rejected
            }
            Err(crate::sumeragi::view_change::Error::ViewChangeOutdated) => {
                trace!(
                    ?latest_hash,
                    "dropping view-change proof: proof is behind current chain"
                );
                ViewChangeProofOutcome::Stale
            }
        };

        note_view_change_proof_status(&outcome);
        #[cfg(feature = "telemetry")]
        note_view_change_proof_telemetry(&outcome, &self.telemetry);

        Ok(())
    }

    pub(super) fn record_and_broadcast_evidence(
        &mut self,
        evidence: crate::sumeragi::consensus::Evidence,
    ) -> Result<()> {
        self.handle_evidence(evidence.clone())?;
        let frame = super::message::ControlFlow::Evidence(evidence);
        let broadcast = Broadcast {
            data: NetworkMessage::SumeragiControlFlow(Box::new(frame)),
            priority: Priority::High,
        };
        self.network.broadcast(broadcast);
        Ok(())
    }

    /// Return `true` if the local peer needs to sign the proof before broadcasting it.
    pub(super) fn should_sign_view_change_proof(
        existing: Option<&crate::sumeragi::view_change::SignedViewChangeProof>,
        latest_block: HashOf<BlockHeader>,
        local_pk: &PublicKey,
    ) -> bool {
        existing
            .is_none_or(|proof| !proof.has_signer(local_pk) || proof.latest_block() != latest_block)
    }

    pub(super) fn record_and_broadcast_view_change_proof(
        &mut self,
        height: u64,
        view_idx: u64,
    ) -> Result<()> {
        let latest_hash = {
            let view = self.state.view();
            if let Some(hash) = view.latest_block_hash() {
                hash
            } else {
                trace!(
                    height,
                    view = view_idx,
                    "skipping view-change proof broadcast: no committed blocks yet"
                );
                return Ok(());
            }
        };

        let Ok(view_usize) = usize::try_from(view_idx) else {
            warn!(
                height,
                view = view_idx,
                "skipping view-change proof broadcast: view index exceeds usize limits"
            );
            return Ok(());
        };

        let local_pk = self.common_config.key_pair.public_key().clone();
        let mut proof_opt = self.view_change_chain.get_proof_for_view_change(view_usize);

        let mut signed_locally = false;
        let needs_signature =
            Self::should_sign_view_change_proof(proof_opt.as_ref(), latest_hash, &local_pk);

        if needs_signature {
            let builder = crate::sumeragi::view_change::ProofBuilder::new(latest_hash, view_usize);
            let signed = builder.sign(&self.common_config.key_pair);
            self.handle_view_change_proof(signed.clone())?;
            proof_opt = self
                .view_change_chain
                .get_proof_for_view_change(view_usize)
                .or(Some(signed));
            super::status::inc_view_change_suggest();
            #[cfg(feature = "telemetry")]
            self.telemetry.inc_view_change_suggest();
            signed_locally = true;
        }

        let Some(proof) = proof_opt else {
            trace!(
                height,
                view = view_idx,
                "view-change proof unavailable after merge; skipping broadcast"
            );
            return Ok(());
        };

        if proof.latest_block() != latest_hash {
            trace!(
                height,
                view = view_idx,
                proof_hash = ?proof.latest_block(),
                latest = ?latest_hash,
                "skipping view-change proof broadcast: payload references different block"
            );
            return Ok(());
        }

        let topology_peers = self.effective_commit_topology();
        let local_peer_id = self.common_config.peer.id().clone();
        for peer in &topology_peers {
            if peer == &local_peer_id {
                continue;
            }
            self.schedule_background(BackgroundRequest::PostControlFlow {
                peer: peer.clone(),
                frame: super::message::ControlFlow::ViewChangeProof(proof.clone()),
            });
        }
        trace!(
            height,
            view = view_idx,
            signed_locally,
            "enqueued view-change proof posts to commit topology"
        );
        Ok(())
    }
}
