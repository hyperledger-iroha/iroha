//! Lane-local block message ingress.

use super::*;

impl Actor {
    pub(super) fn handle_lane_block_proposal(
        &mut self,
        proposal: crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> Result<()> {
        if !self.lane_block_route_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_block_height,
        ) {
            self.record_inactive_lane_block_route_drop(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
            );
            return Ok(());
        }
        if !self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.validator_count,
            proposal.descriptor.min_quorum,
            proposal.descriptor.validator_set_hash,
            Some(proposal.descriptor.validator_set.as_slice()),
            None,
        ) {
            self.record_unauthorized_lane_block_committee_drop(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.validator_count,
                proposal.descriptor.min_quorum,
            );
            return Ok(());
        }

        match self.subsystems.lane_blocks.insert_proposal(proposal) {
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Inserted) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Deferred,
                    super::status::ConsensusMessageReason::PayloadUnapplied,
                );
                debug!("cached validated lane-block proposal for live lane session");
                self.broadcast_newly_sealed_lane_block_qcs();
                self.broadcast_lane_block_commit_votes_for_prepared_sessions();
                self.queue_committed_lane_block_sessions();
            }
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Duplicate) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::Duplicate,
                );
            }
            Err(err) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    lane_block_session_error_reason(err),
                );
                warn!(?err, "dropping invalid lane-block proposal");
            }
        }
        Ok(())
    }

    pub(super) fn handle_lane_block_vote(
        &mut self,
        vote: crate::lane_consensus::LaneBlockVoteV1,
        sender: Option<&PeerId>,
    ) -> Result<()> {
        if !self.lane_block_route_accepts_ingress(
            vote.body.lane_id,
            vote.body.dataspace_id,
            vote.body.lane_block_height,
        ) {
            self.record_inactive_lane_block_route_drop(
                super::status::ConsensusMessageKind::LaneBlockVote,
                vote.body.lane_id,
                vote.body.dataspace_id,
            );
            return Ok(());
        }
        let Some(sender) = sender else {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidSignature,
            );
            warn!(
                signer = %vote.signer,
                lane_id = vote.body.lane_id.as_u32(),
                dataspace_id = vote.body.dataspace_id.as_u64(),
                "dropping lane-block vote without authenticated sender"
            );
            return Ok(());
        };
        if !self.lane_block_authority_accepts_ingress(
            vote.body.lane_id,
            vote.body.dataspace_id,
            vote.body.validator_count,
            vote.body.min_quorum,
            vote.body.validator_set_hash,
            None,
            Some(&vote.signer),
        ) {
            self.record_unauthorized_lane_block_committee_drop(
                super::status::ConsensusMessageKind::LaneBlockVote,
                vote.body.lane_id,
                vote.body.dataspace_id,
                vote.body.validator_count,
                vote.body.min_quorum,
            );
            return Ok(());
        }

        match self.subsystems.lane_blocks.insert_vote(vote, Some(sender)) {
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Inserted) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockVote,
                    super::status::ConsensusMessageOutcome::Deferred,
                    super::status::ConsensusMessageReason::PayloadUnapplied,
                );
                debug!("cached validated lane-block vote for live lane session");
                self.broadcast_newly_sealed_lane_block_qcs();
                self.broadcast_lane_block_commit_votes_for_prepared_sessions();
                self.queue_committed_lane_block_sessions();
            }
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Duplicate) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::Duplicate,
                );
            }
            Err(err) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    lane_block_session_error_reason(err),
                );
                warn!(?err, "dropping invalid lane-block vote");
            }
        }
        Ok(())
    }

    pub(super) fn handle_lane_block_qc(
        &mut self,
        qc: crate::sumeragi::consensus::LaneBlockQcV1,
    ) -> Result<()> {
        if !self.lane_block_route_accepts_ingress(
            qc.body.lane_id,
            qc.body.dataspace_id,
            qc.body.lane_block_height,
        ) {
            self.record_inactive_lane_block_route_drop(
                super::status::ConsensusMessageKind::LaneBlockQc,
                qc.body.lane_id,
                qc.body.dataspace_id,
            );
            return Ok(());
        }
        if !self.lane_block_authority_accepts_ingress(
            qc.body.lane_id,
            qc.body.dataspace_id,
            qc.body.validator_count,
            qc.body.min_quorum,
            qc.body.validator_set_hash,
            Some(qc.validator_set.as_slice()),
            None,
        ) {
            self.record_unauthorized_lane_block_committee_drop(
                super::status::ConsensusMessageKind::LaneBlockQc,
                qc.body.lane_id,
                qc.body.dataspace_id,
                qc.body.validator_count,
                qc.body.min_quorum,
            );
            return Ok(());
        }

        let pops = self.lane_block_qc_signer_pops(&qc);
        match self.subsystems.lane_blocks.insert_qc_with_pops(qc, &pops) {
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Inserted) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockQc,
                    super::status::ConsensusMessageOutcome::Deferred,
                    super::status::ConsensusMessageReason::PayloadUnapplied,
                );
                debug!("cached validated lane-block QC for live lane session");
                self.broadcast_lane_block_commit_votes_for_prepared_sessions();
                self.queue_committed_lane_block_sessions();
            }
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Duplicate) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockQc,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::Duplicate,
                );
            }
            Err(err) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockQc,
                    super::status::ConsensusMessageOutcome::Dropped,
                    lane_block_session_error_reason(err),
                );
                warn!(?err, "dropping invalid lane-block QC");
            }
        }
        Ok(())
    }

    pub(super) fn lane_block_artifact_targets_active_route(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_block_height: u64,
    ) -> bool {
        let nexus = self.state.nexus_snapshot();
        if !nexus.enabled {
            return true;
        }
        let state_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        crate::state::nexus_active_lane_dataspace_at_height(lane_id, &nexus, state_height)
            == Some(dataspace_id)
            && self
                .state
                .da_shard_reset_heights_snapshot_cached()
                .get(&lane_id)
                .is_none_or(|reset_height| lane_block_height > *reset_height)
    }

    fn lane_block_route_accepts_ingress(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_block_height: u64,
    ) -> bool {
        self.lane_block_artifact_targets_active_route(lane_id, dataspace_id, lane_block_height)
    }

    fn lane_block_authority_accepts_ingress(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        validator_count: u32,
        min_quorum: u32,
        validator_set_hash: HashOf<Vec<PeerId>>,
        validator_set: Option<&[PeerId]>,
        signer: Option<&PeerId>,
    ) -> bool {
        let nexus = self.state.nexus_snapshot();
        if !nexus.enabled {
            return true;
        }
        let state_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        if crate::state::nexus_active_lane_dataspace_at_height(lane_id, &nexus, state_height)
            != Some(dataspace_id)
        {
            return false;
        }
        let mut expected =
            if super::lane_scheduler::proposal_lookahead_enabled(&nexus, state_height) {
                self.state.authoritative_lane_peer_ids(lane_id)
            } else {
                self.shared_lane_block_authority_for_ingress(state_height)
            };
        expected.sort();
        expected.dedup();
        if expected.is_empty() {
            return false;
        }
        let Ok(expected_count) = u32::try_from(expected.len()) else {
            return false;
        };
        let Ok(expected_quorum) = u32::try_from(
            crate::sumeragi::network_topology::commit_quorum_from_len(expected.len()).max(1),
        ) else {
            return false;
        };
        if validator_count != expected_count
            || min_quorum != expected_quorum
            || validator_set_hash != HashOf::new(&expected)
        {
            return false;
        }
        if signer.is_some_and(|signer| !expected.contains(signer)) {
            return false;
        }
        validator_set.is_none_or(|validator_set| validator_set == expected.as_slice())
    }

    pub(super) fn lane_block_proposal_accepts_local_broadcast(
        &self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> bool {
        self.lane_block_route_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_block_height,
        ) && self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.validator_count,
            proposal.descriptor.min_quorum,
            proposal.descriptor.validator_set_hash,
            Some(proposal.descriptor.validator_set.as_slice()),
            None,
        ) && self
            .subsystems
            .lane_blocks
            .can_accept_proposal(proposal)
            .is_ok()
            && !self.subsystems.lane_blocks.contains_proposal(proposal)
    }

    pub(super) fn lane_block_vote_accepts_local_broadcast(
        &self,
        vote: &crate::lane_consensus::LaneBlockVoteV1,
        sender: &PeerId,
    ) -> bool {
        self.lane_block_vote_body_targets_authorized_local_signer(&vote.body, &vote.signer)
            && self
                .subsystems
                .lane_blocks
                .can_accept_vote(vote, Some(sender))
                .is_ok()
            && !self.subsystems.lane_blocks.contains_vote(vote)
    }

    pub(super) fn lane_block_vote_body_targets_authorized_local_signer(
        &self,
        body: &crate::sumeragi::consensus::LaneBlockVoteBodyV1,
        signer: &PeerId,
    ) -> bool {
        self.lane_block_route_accepts_ingress(
            body.lane_id,
            body.dataspace_id,
            body.lane_block_height,
        ) && self.lane_block_authority_accepts_ingress(
            body.lane_id,
            body.dataspace_id,
            body.validator_count,
            body.min_quorum,
            body.validator_set_hash,
            None,
            Some(signer),
        )
    }

    fn shared_lane_block_authority_for_ingress(&self, state_height: u64) -> Vec<PeerId> {
        let target_height = state_height.saturating_add(1);
        let (consensus_mode, _mode_tag, _prf_seed) =
            self.consensus_context_for_height(target_height);
        let mut validators = self.roster_for_live_vote_with_mode(target_height, consensus_mode);
        if validators.is_empty() {
            validators = self.effective_commit_topology();
        }
        validators
    }

    fn record_inactive_lane_block_route_drop(
        &mut self,
        kind: super::status::ConsensusMessageKind,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
    ) {
        self.record_consensus_message_handling(
            kind,
            super::status::ConsensusMessageOutcome::Dropped,
            super::status::ConsensusMessageReason::InvalidPayload,
        );
        warn!(
            lane_id = lane_id.as_u32(),
            dataspace_id = dataspace_id.as_u64(),
            "dropping lane-block message for inactive Nexus lane route"
        );
    }

    fn record_unauthorized_lane_block_committee_drop(
        &mut self,
        kind: super::status::ConsensusMessageKind,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        validator_count: u32,
        min_quorum: u32,
    ) {
        self.record_consensus_message_handling(
            kind,
            super::status::ConsensusMessageOutcome::Dropped,
            super::status::ConsensusMessageReason::InvalidPayload,
        );
        warn!(
            lane_id = lane_id.as_u32(),
            dataspace_id = dataspace_id.as_u64(),
            validator_count,
            min_quorum,
            "dropping lane-block message for non-authoritative Nexus lane committee"
        );
    }

    fn local_lane_block_commit_vote(
        &self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> Option<crate::lane_consensus::LaneBlockVoteV1> {
        let local_peer = self.common_config.peer.id();
        if !proposal.descriptor.validator_set.contains(local_peer) {
            return None;
        }
        if let Err(err) = crate::lane_consensus::validate_lane_block_proposal(proposal) {
            warn!(
                ?err,
                lane_id = proposal.descriptor.lane_id.as_u32(),
                dataspace_id = proposal.descriptor.dataspace_id.as_u64(),
                lane_block_height = proposal.descriptor.lane_block_height,
                lane_block_view = proposal.descriptor.lane_block_view,
                "skipping local lane-block commit vote for invalid proposal"
            );
            return None;
        }
        if !self.lane_block_route_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_block_height,
        ) || !self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.validator_count,
            proposal.descriptor.min_quorum,
            proposal.descriptor.validator_set_hash,
            Some(proposal.descriptor.validator_set.as_slice()),
            Some(local_peer),
        ) {
            warn!(
                lane_id = proposal.descriptor.lane_id.as_u32(),
                dataspace_id = proposal.descriptor.dataspace_id.as_u64(),
                validator_count = proposal.descriptor.validator_count,
                min_quorum = proposal.descriptor.min_quorum,
                "skipping local lane-block commit vote for non-authoritative route or committee"
            );
            return None;
        }
        match self.common_config.key_pair.public_key().try_algorithm() {
            Ok(iroha_crypto::Algorithm::BlsNormal) => {}
            Ok(algorithm) => {
                warn!(
                    ?algorithm,
                    "skipping local lane-block commit vote broadcast with non-BLS consensus key"
                );
                return None;
            }
            Err(err) => {
                warn!(
                    ?err,
                    "skipping local lane-block commit vote broadcast with unrecognized consensus key"
                );
                return None;
            }
        }
        if local_peer.public_key() != self.common_config.key_pair.public_key() {
            warn!("skipping local lane-block commit vote for mismatched local peer key");
            return None;
        }

        let body = proposal.vote_body(crate::sumeragi::consensus::Phase::Commit);
        let signature = match Signature::try_new(
            self.common_config.key_pair.private_key(),
            &body.signature_preimage(),
        ) {
            Ok(signature) => signature,
            Err(err) => {
                warn!(
                    ?err,
                    lane = body.lane_id.as_u32(),
                    lane_block_height = body.lane_block_height,
                    lane_block_view = body.lane_block_view,
                    "skipping local lane-block commit vote after signing failure"
                );
                return None;
            }
        };

        Some(crate::lane_consensus::LaneBlockVoteV1 {
            body,
            signer: local_peer.clone(),
            bls_signature: signature.payload().to_vec(),
        })
    }

    pub(super) fn broadcast_lane_block_commit_votes_for_prepared_sessions(&mut self) {
        let local_peer = self.common_config.peer.id().clone();
        let requests = self
            .subsystems
            .lane_blocks
            .drain_commit_vote_requests_for(&local_peer);

        for request in requests {
            let Some(vote) = self.local_lane_block_commit_vote(&request.proposal) else {
                continue;
            };
            if self.lane_block_vote_accepts_local_broadcast(&vote, &local_peer) {
                debug!(
                    lane_id = ?request.prepare_qc.body.lane_id,
                    dataspace_id = ?request.prepare_qc.body.dataspace_id,
                    lane_block_height = request.prepare_qc.body.lane_block_height,
                    lane_block_view = request.prepare_qc.body.lane_block_view,
                    "broadcasting local lane-block commit vote after prepare QC"
                );
                self.schedule_background(BackgroundRequest::Broadcast {
                    msg: BlockMessageWire::new(BlockMessage::LaneBlockVote(vote.clone())),
                });
            }
            if let Err(err) = self.handle_lane_block_vote(vote, Some(&local_peer)) {
                warn!(
                    ?err,
                    "failed to cache local lane-block commit vote after prepare QC"
                );
            }
        }
    }

    pub(super) fn broadcast_newly_sealed_lane_block_qcs(&mut self) {
        for qc in self.subsystems.lane_blocks.drain_newly_sealed_qcs() {
            if !self.lane_block_route_accepts_ingress(
                qc.body.lane_id,
                qc.body.dataspace_id,
                qc.body.lane_block_height,
            ) {
                self.record_inactive_lane_block_route_drop(
                    super::status::ConsensusMessageKind::LaneBlockQc,
                    qc.body.lane_id,
                    qc.body.dataspace_id,
                );
                continue;
            }
            if !self.lane_block_authority_accepts_ingress(
                qc.body.lane_id,
                qc.body.dataspace_id,
                qc.body.validator_count,
                qc.body.min_quorum,
                qc.body.validator_set_hash,
                Some(qc.validator_set.as_slice()),
                None,
            ) {
                self.record_unauthorized_lane_block_committee_drop(
                    super::status::ConsensusMessageKind::LaneBlockQc,
                    qc.body.lane_id,
                    qc.body.dataspace_id,
                    qc.body.validator_count,
                    qc.body.min_quorum,
                );
                continue;
            }
            debug!(
                lane_id = ?qc.body.lane_id,
                dataspace_id = ?qc.body.dataspace_id,
                lane_block_height = qc.body.lane_block_height,
                lane_block_view = qc.body.lane_block_view,
                phase = ?qc.body.phase,
                "broadcasting sealed lane-block QC"
            );
            self.schedule_background(BackgroundRequest::Broadcast {
                msg: BlockMessageWire::new(BlockMessage::LaneBlockQc(qc)),
            });
        }
    }

    fn prune_lane_block_sessions_for_inactive_lanes(&mut self) -> usize {
        let nexus = self.state.nexus_snapshot();
        if !nexus.enabled {
            return 0;
        }
        let state_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let reset_heights = self.state.da_shard_reset_heights_snapshot_cached();
        let admissible_lane =
            |lane_id: LaneId, dataspace_id: DataSpaceId, lane_block_height: u64| {
                crate::state::nexus_active_lane_dataspace_at_height(lane_id, &nexus, state_height)
                    == Some(dataspace_id)
                    && reset_heights
                        .get(&lane_id)
                        .is_none_or(|reset_height| lane_block_height > *reset_height)
            };
        let pruned_lane_sessions = self
            .subsystems
            .lane_blocks
            .retain_sessions_for_admissible_lanes(&admissible_lane);
        let pruned_committed_sessions = self
            .subsystems
            .committed_lane_blocks
            .retain_sessions_for_admissible_lanes(&admissible_lane);
        let pruned = pruned_lane_sessions.saturating_add(pruned_committed_sessions);
        if pruned > 0 {
            debug!(
                pruned_lane_sessions,
                pruned_committed_sessions,
                "pruned cached lane-block sessions for inactive lane routes"
            );
        }
        pruned
    }

    pub(super) fn queue_committed_lane_block_sessions(&mut self) -> bool {
        let pruned_inactive_lane_sessions = self.prune_lane_block_sessions_for_inactive_lanes();
        let pruned_applied_before = self
            .subsystems
            .committed_lane_blocks
            .prune_application_receipted_sessions(self.state.kura());
        let queued = self.enqueue_ready_committed_lane_block_sessions();
        let pending_before_processing = self.subsystems.committed_lane_blocks.len();
        if queued == 0
            && pruned_inactive_lane_sessions == 0
            && pruned_applied_before == 0
            && pending_before_processing == 0
        {
            return false;
        }
        let recovered_inputs = self
            .subsystems
            .committed_lane_blocks
            .recover_available_payloads_into_kura(self.state.kura());
        let direct_preflights = self
            .subsystems
            .committed_lane_blocks
            .preflight_recovered_execution_inputs_into_kura(&self.state);
        let mut direct_applications = 0_usize;
        let mut direct_preflights = direct_preflights;
        let direct_application_limit = self.subsystems.committed_lane_blocks.len();
        for _ in 0..direct_application_limit {
            let applied = self
                .subsystems
                .committed_lane_blocks
                .apply_preflighted_execution_inputs_to_state(&self.state);
            if applied == 0 {
                break;
            }
            direct_applications = direct_applications.saturating_add(applied);
            direct_preflights = direct_preflights.saturating_add(
                self.subsystems
                    .committed_lane_blocks
                    .preflight_recovered_execution_inputs_into_kura(&self.state),
            );
        }
        let repaired_direct_receipts =
            super::CommittedLaneBlockQueue::repair_missing_direct_application_receipts_from_state(
                &self.state,
            );
        let application_receipts = self
            .subsystems
            .committed_lane_blocks
            .record_available_payload_application_receipts_into_kura(self.state.kura());
        let committed_status_before_prune = self
            .subsystems
            .committed_lane_blocks
            .status_snapshot_with_payload_availability(
                self.state.kura(),
                u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX),
                Some(self.state.lane_execution_state_hash()),
            );
        let pruned_applied_after = self
            .subsystems
            .committed_lane_blocks
            .prune_application_receipted_sessions(self.state.kura());
        let queued_after_prune = if pruned_applied_after > 0 {
            self.enqueue_ready_committed_lane_block_sessions()
        } else {
            0
        };
        let committed_status = self
            .subsystems
            .committed_lane_blocks
            .status_snapshot_for_state(&self.state);
        let committed_status = super::merge_committed_lane_block_statuses(
            committed_status_before_prune,
            committed_status,
        );
        let pending = self.subsystems.committed_lane_blocks.len();
        super::status::set_committed_lane_blocks(committed_status);
        debug!(
            queued,
            recovered_inputs,
            direct_preflights,
            direct_applications,
            repaired_direct_receipts,
            application_receipts,
            pruned_inactive_lane_sessions,
            pruned_applied_before,
            pruned_applied_after,
            queued_after_prune,
            pending,
            "queued committed lane-block sessions for execution"
        );
        queued > 0
            || recovered_inputs > 0
            || direct_preflights > 0
            || direct_applications > 0
            || repaired_direct_receipts > 0
            || application_receipts > 0
            || pruned_inactive_lane_sessions > 0
            || pruned_applied_before > 0
            || pruned_applied_after > 0
            || queued_after_prune > 0
    }

    fn enqueue_ready_committed_lane_block_sessions(&mut self) -> usize {
        let ready = {
            let ActorSubsystems {
                lane_blocks,
                committed_lane_blocks,
                ..
            } = &mut self.subsystems;
            committed_lane_blocks.enqueue_ready_sessions(lane_blocks)
        };
        for session in &ready {
            let signer_pops = self.committed_lane_block_session_pops(session);
            if let Err(err) = self
                .state
                .kura()
                .persist_committed_lane_block_session(session, &signer_pops)
            {
                warn!(
                    ?err,
                    lane_id = ?session.proposal.descriptor.lane_id,
                    dataspace_id = ?session.proposal.descriptor.dataspace_id,
                    lane_block_height = session.proposal.descriptor.lane_block_height,
                    lane_block_view = session.proposal.descriptor.lane_block_view,
                    "failed to persist certified lane-block session"
                );
            }
        }
        ready.len()
    }

    fn committed_lane_block_session_pops(
        &self,
        session: &crate::lane_consensus::CommittedLaneBlockSession,
    ) -> BTreeMap<PublicKey, Vec<u8>> {
        let mut pops = self.lane_block_qc_signer_pops(&session.prepare_qc);
        pops.extend(self.lane_block_qc_signer_pops(&session.commit_qc));
        pops
    }

    fn lane_block_qc_signer_pops(
        &self,
        qc: &crate::sumeragi::consensus::LaneBlockQcV1,
    ) -> BTreeMap<PublicKey, Vec<u8>> {
        let trusted = self.common_config.trusted_peers.value();
        let mut pops = BTreeMap::new();
        for (byte_index, byte) in qc.signers_bitmap.iter().copied().enumerate() {
            if byte == 0 {
                continue;
            }
            for bit in 0..8 {
                if byte & (1_u8 << bit) == 0 {
                    continue;
                }
                let signer_index = byte_index * 8 + bit;
                let Some(signer) = qc.validator_set.get(signer_index) else {
                    continue;
                };
                let pk = signer.public_key();
                if let Some(pop) = self
                    .roster_validation_cache
                    .pops
                    .get(pk)
                    .or_else(|| trusted.pops.get(pk))
                {
                    pops.insert(pk.clone(), pop.clone());
                }
            }
        }
        pops
    }
}

fn lane_block_session_error_reason(
    err: crate::lane_consensus::LaneBlockSessionError,
) -> super::status::ConsensusMessageReason {
    match err {
        crate::lane_consensus::LaneBlockSessionError::InvalidVote(
            crate::lane_consensus::LaneBlockVoteIngressError::InvalidSignature
            | crate::lane_consensus::LaneBlockVoteIngressError::SenderMismatch
            | crate::lane_consensus::LaneBlockVoteIngressError::SignerNotBlsNormal,
        )
        | crate::lane_consensus::LaneBlockSessionError::InvalidQc(
            crate::lane_consensus::LaneBlockQcIngressError::AggregateSignatureInvalid
            | crate::lane_consensus::LaneBlockQcIngressError::AggregateSignatureMissing
            | crate::lane_consensus::LaneBlockQcIngressError::SignerNotBlsNormal
            | crate::lane_consensus::LaneBlockQcIngressError::SignerPopInvalid
            | crate::lane_consensus::LaneBlockQcIngressError::SignerPopMissing,
        ) => super::status::ConsensusMessageReason::InvalidSignature,
        crate::lane_consensus::LaneBlockSessionError::ConflictingVote => {
            super::status::ConsensusMessageReason::ConflictingVote
        }
        crate::lane_consensus::LaneBlockSessionError::InvalidProposal(_)
        | crate::lane_consensus::LaneBlockSessionError::InvalidVote(_)
        | crate::lane_consensus::LaneBlockSessionError::InvalidQc(_)
        | crate::lane_consensus::LaneBlockSessionError::ConflictingProposal
        | crate::lane_consensus::LaneBlockSessionError::VoteProposalMismatch
        | crate::lane_consensus::LaneBlockSessionError::VoteSignerNotInValidatorSet
        | crate::lane_consensus::LaneBlockSessionError::QcProposalMismatch
        | crate::lane_consensus::LaneBlockSessionError::ConflictingQc => {
            super::status::ConsensusMessageReason::InvalidPayload
        }
    }
}
