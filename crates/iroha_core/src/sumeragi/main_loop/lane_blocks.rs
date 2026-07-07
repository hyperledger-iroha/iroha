//! Lane-local block message ingress.

use super::*;

impl Actor {
    pub(super) fn handle_lane_block_proposal(
        &mut self,
        proposal: crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> Result<()> {
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
        match self.subsystems.lane_blocks.insert_vote(vote, sender) {
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

    fn local_lane_block_commit_vote(
        &self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> Option<crate::lane_consensus::LaneBlockVoteV1> {
        let local_peer = self.common_config.peer.id();
        if !proposal.descriptor.validator_set.contains(local_peer) {
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

    fn broadcast_lane_block_commit_votes_for_prepared_sessions(&mut self) {
        let local_peer = self.common_config.peer.id().clone();
        let requests = self
            .subsystems
            .lane_blocks
            .drain_commit_vote_requests_for(&local_peer);

        for request in requests {
            let Some(vote) = self.local_lane_block_commit_vote(&request.proposal) else {
                continue;
            };
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
            if let Err(err) = self.handle_lane_block_vote(vote, Some(&local_peer)) {
                warn!(
                    ?err,
                    "failed to cache local lane-block commit vote after prepare QC"
                );
            }
        }
    }

    fn broadcast_newly_sealed_lane_block_qcs(&mut self) {
        for qc in self.subsystems.lane_blocks.drain_newly_sealed_qcs() {
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
        let active_lanes = nexus
            .lane_config
            .entries()
            .iter()
            .map(|entry| (entry.lane_id, entry.dataspace_id))
            .collect::<BTreeSet<_>>();
        let pruned_lane_sessions = self
            .subsystems
            .lane_blocks
            .retain_sessions_for_active_lanes(|lane_id, dataspace_id| {
                active_lanes.contains(&(lane_id, dataspace_id))
            });
        let pruned_committed_sessions = self
            .subsystems
            .committed_lane_blocks
            .retain_sessions_for_active_lanes(|lane_id, dataspace_id| {
                active_lanes.contains(&(lane_id, dataspace_id))
            });
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
