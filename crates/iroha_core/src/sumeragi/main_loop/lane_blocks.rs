//! Lane-local block message ingress.

use super::*;
use iroha_data_model::block::consensus::{
    LaneBlockProposalPayloadHintV1, SumeragiLanePayloadOwnership,
};

impl Actor {
    pub(super) fn handle_lane_block_proposal(
        &mut self,
        proposal: crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> Result<()> {
        self.cache_lane_block_proposal(proposal);
        self.publish_lane_block_session_status();
        Ok(())
    }

    pub(super) fn handle_incoming_lane_block_proposal(
        &mut self,
        proposal: crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> Result<()> {
        let outcome = self.cache_lane_block_proposal(proposal.clone());
        if matches!(
            outcome,
            LaneBlockProposalIngressOutcome::Inserted | LaneBlockProposalIngressOutcome::Duplicate
        ) {
            self.broadcast_local_prepare_vote_for_incoming_lane_block_proposal(&proposal);
        }
        self.publish_lane_block_session_status();
        Ok(())
    }

    fn publish_lane_block_session_status(&self) {
        super::status::set_lane_block_sessions(self.subsystems.lane_blocks.status_snapshot());
    }

    fn cache_lane_block_proposal(
        &mut self,
        proposal: crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> LaneBlockProposalIngressOutcome {
        if !self.lane_block_route_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
        ) {
            self.record_inactive_lane_block_route_drop(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
            );
            return LaneBlockProposalIngressOutcome::Dropped;
        }
        if !self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
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
            return LaneBlockProposalIngressOutcome::Dropped;
        }

        let proposal_for_repair = proposal.clone();
        match self.subsystems.lane_blocks.insert_proposal(proposal) {
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Inserted) => {
                self.request_lane_block_payload_hint_repair(
                    &proposal_for_repair,
                    "lane_block_proposal_inserted",
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Deferred,
                    super::status::ConsensusMessageReason::PayloadUnapplied,
                );
                debug!("cached validated lane-block proposal for live lane session");
                self.broadcast_newly_sealed_lane_block_qcs();
                self.broadcast_lane_block_commit_votes_for_prepared_sessions();
                self.queue_committed_lane_block_sessions();
                LaneBlockProposalIngressOutcome::Inserted
            }
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Duplicate) => {
                self.request_lane_block_payload_hint_repair(
                    &proposal_for_repair,
                    "lane_block_proposal_duplicate",
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::Duplicate,
                );
                LaneBlockProposalIngressOutcome::Duplicate
            }
            Err(err) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    lane_block_session_error_reason(err),
                );
                warn!(?err, "dropping invalid lane-block proposal");
                LaneBlockProposalIngressOutcome::Dropped
            }
        }
    }

    fn cache_recovered_lane_block_artifact_proposal(
        &mut self,
        proposal: crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> LaneBlockProposalIngressOutcome {
        if !self.lane_block_route_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
        ) {
            self.record_inactive_lane_block_route_drop(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
            );
            return LaneBlockProposalIngressOutcome::Dropped;
        }
        if !self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
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
            return LaneBlockProposalIngressOutcome::Dropped;
        }

        let proposal_for_repair = proposal.clone();
        match self
            .subsystems
            .lane_blocks
            .insert_recovered_proposal_replacing_uncommitted_conflict(proposal)
        {
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Inserted) => {
                self.request_lane_block_payload_hint_repair(
                    &proposal_for_repair,
                    "recovered_lane_block_proposal_inserted",
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Deferred,
                    super::status::ConsensusMessageReason::PayloadUnapplied,
                );
                debug!("cached recovered lane-block proposal for live lane session");
                self.broadcast_newly_sealed_lane_block_qcs();
                self.broadcast_lane_block_commit_votes_for_prepared_sessions();
                self.queue_committed_lane_block_sessions();
                LaneBlockProposalIngressOutcome::Inserted
            }
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Duplicate) => {
                self.request_lane_block_payload_hint_repair(
                    &proposal_for_repair,
                    "recovered_lane_block_proposal_duplicate",
                );
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::Duplicate,
                );
                LaneBlockProposalIngressOutcome::Duplicate
            }
            Err(err) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    lane_block_session_error_reason(err),
                );
                warn!(?err, "dropping invalid recovered lane-block proposal");
                LaneBlockProposalIngressOutcome::Dropped
            }
        }
    }

    fn request_lane_block_payload_hint_repair(
        &mut self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
        reason: &'static str,
    ) -> usize {
        let Some(hint) = proposal.payload_block_hint.as_ref() else {
            return 0;
        };
        if hint.proposal_height == 0 {
            return 0;
        }
        let kura = self.state.kura();
        if kura.lane_block_application_receipt_available(proposal)
            || kura.lane_block_execution_input_available(proposal)
            || kura
                .lane_block_payload_availability(proposal)
                .is_available()
        {
            return 0;
        }
        let sent = super::send_certified_block_fetch_request(
            &self.network,
            self.common_config.peer.id(),
            hint.proposal_block_hash,
            hint.proposal_height,
            hint.proposal_view,
            proposal.descriptor.validator_set.as_slice(),
        );
        if sent > 0 {
            debug!(
                sent,
                reason,
                lane_id = ?proposal.descriptor.lane_id,
                dataspace_id = ?proposal.descriptor.dataspace_id,
                lane_block_height = proposal.descriptor.lane_block_height,
                lane_block_view = proposal.descriptor.lane_block_view,
                proposal_height = hint.proposal_height,
                proposal_view = hint.proposal_view,
                proposal_block_hash = %hint.proposal_block_hash,
                "requested certified block body for hinted lane-block payload repair"
            );
        }
        sent
    }

    pub(super) fn handle_lane_block_vote(
        &mut self,
        vote: crate::lane_consensus::LaneBlockVoteV1,
        sender: Option<&PeerId>,
    ) -> Result<()> {
        if !self.lane_block_route_accepts_ingress(
            vote.body.lane_id,
            vote.body.dataspace_id,
            vote.body.proposal_height,
            vote.body.lane_block_height,
        ) {
            self.record_inactive_lane_block_route_drop(
                super::status::ConsensusMessageKind::LaneBlockVote,
                vote.body.lane_id,
                vote.body.dataspace_id,
            );
            return Ok(());
        }
        if !self.lane_block_authority_accepts_ingress(
            vote.body.lane_id,
            vote.body.dataspace_id,
            vote.body.proposal_height,
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
        self.publish_lane_block_session_status();
        Ok(())
    }

    pub(super) fn handle_lane_block_qc(
        &mut self,
        qc: crate::sumeragi::consensus::LaneBlockQcV1,
    ) -> Result<()> {
        if !self.lane_block_route_accepts_ingress(
            qc.body.lane_id,
            qc.body.dataspace_id,
            qc.body.proposal_height,
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
            qc.body.proposal_height,
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
        self.publish_lane_block_session_status();
        Ok(())
    }

    pub(super) fn lane_block_artifact_targets_active_route(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        proposal_height: u64,
        lane_block_height: u64,
    ) -> bool {
        let nexus = self.state.nexus_snapshot();
        if !nexus.enabled {
            return true;
        }
        crate::state::nexus_active_lane_dataspace_at_height(lane_id, &nexus, proposal_height)
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
        proposal_height: u64,
        lane_block_height: u64,
    ) -> bool {
        self.lane_block_artifact_targets_active_route(
            lane_id,
            dataspace_id,
            proposal_height,
            lane_block_height,
        )
    }

    fn lane_block_authority_accepts_ingress(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        proposal_height: u64,
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
        if crate::state::nexus_active_lane_dataspace_at_height(lane_id, &nexus, proposal_height)
            != Some(dataspace_id)
        {
            return false;
        }
        let mut expected =
            if super::lane_scheduler::proposal_lookahead_enabled(&nexus, proposal_height) {
                self.state
                    .authoritative_lane_peer_ids_at_height(lane_id, proposal_height)
            } else {
                self.shared_lane_block_authority_for_ingress(proposal_height)
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
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
        ) && self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
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
            body.proposal_height,
            body.lane_block_height,
        ) && self.lane_block_authority_accepts_ingress(
            body.lane_id,
            body.dataspace_id,
            body.proposal_height,
            body.validator_count,
            body.min_quorum,
            body.validator_set_hash,
            None,
            Some(signer),
        )
    }

    fn lane_block_proposal_from_payload_ownership(
        ownership: &SumeragiLanePayloadOwnership,
        proposal_block_hash: Option<HashOf<BlockHeader>>,
    ) -> Option<crate::sumeragi::consensus::LaneBlockProposalV1> {
        let descriptor_hash = ownership.lane_block_descriptor_hash?;
        let descriptor = crate::sumeragi::consensus::LaneBlockDescriptorV1 {
            lane_id: ownership.lane_id,
            dataspace_id: ownership.dataspace_id,
            proposal_height: ownership.proposal_height,
            previous_lane_block_height: ownership.previous_lane_block_height,
            previous_lane_block_descriptor_hash: ownership.previous_lane_block_descriptor_hash,
            lane_block_height: ownership.lane_block_height,
            lane_block_view: ownership.lane_block_view,
            subject_hash: ownership.subject_hash,
            payload_ownership_hash: ownership.payload_ownership_hash,
            rbc_instance_hash: ownership.rbc_instance_hash,
            accepted_candidate_indices: ownership.accepted_candidate_indices.clone(),
            accepted_transaction_hashes: ownership.accepted_transaction_hashes.clone(),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&ownership.lane_block_descriptor_validator_set),
            validator_set: ownership.lane_block_descriptor_validator_set.clone(),
            validator_count: ownership.lane_block_descriptor_validator_count,
            min_quorum: ownership.lane_block_descriptor_min_quorum,
            qc_mode_tag: ownership.qc_mode_tag.clone(),
            descriptor_hash,
        };
        if descriptor.computed_descriptor_hash() != descriptor_hash
            || descriptor.computed_validator_set_hash() != descriptor.validator_set_hash
        {
            return None;
        }
        let proposal_hash = crate::sumeragi::consensus::LaneBlockProposalV1 {
            descriptor: descriptor.clone(),
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        }
        .computed_proposal_hash();
        let payload_block_hint =
            proposal_block_hash.map(|proposal_block_hash| LaneBlockProposalPayloadHintV1 {
                proposal_height: ownership.proposal_height,
                proposal_view: ownership.proposal_view,
                proposal_block_hash,
            });
        Some(crate::sumeragi::consensus::LaneBlockProposalV1 {
            descriptor,
            proposal_hash,
            payload_block_hint,
        })
    }

    fn cache_unapplied_lane_block_artifact_proposals(&mut self) -> usize {
        let mut cached = 0_usize;
        for artifact in self.state.kura().lane_block_artifacts_snapshot() {
            if Self::lane_block_artifact_has_matching_application_receipt(
                self.state.kura(),
                &artifact,
            ) {
                continue;
            }
            let lane_id = artifact.ownership.lane_id;
            let dataspace_id = artifact.ownership.dataspace_id;
            let lane_block_height = artifact.ownership.lane_block_height;
            if !self.lane_block_artifact_targets_active_route(
                lane_id,
                dataspace_id,
                artifact.ownership.proposal_height,
                lane_block_height,
            ) {
                continue;
            }
            let Some(proposal) = Self::lane_block_proposal_from_payload_ownership(
                &artifact.ownership,
                Some(artifact.proposal_block_hash),
            ) else {
                warn!(
                    lane_id = ?lane_id,
                    dataspace_id = ?dataspace_id,
                    lane_block_height,
                    "skipping malformed lane-block proposal reconstructed from ownership artifact"
                );
                continue;
            };
            if matches!(
                self.cache_recovered_lane_block_artifact_proposal(proposal),
                LaneBlockProposalIngressOutcome::Inserted
            ) {
                cached = cached.saturating_add(1);
            }
        }
        cached
    }

    fn lane_block_artifact_has_matching_application_receipt(
        kura: &crate::kura::Kura,
        artifact: &crate::kura::LaneBlockArtifact,
    ) -> bool {
        let ownership = &artifact.ownership;
        let Some(receipt) = kura
            .read_lane_block_application_receipt(ownership.lane_id, ownership.lane_block_height)
        else {
            return false;
        };
        let descriptor = &receipt.proposal.descriptor;
        descriptor.lane_id == ownership.lane_id
            && descriptor.dataspace_id == ownership.dataspace_id
            && descriptor.lane_block_height == ownership.lane_block_height
            && descriptor.lane_block_view == ownership.lane_block_view
            && Some(descriptor.descriptor_hash) == ownership.lane_block_descriptor_hash
            && receipt.artifact.ownership == *ownership
    }

    fn shared_lane_block_authority_for_ingress(&self, target_height: u64) -> Vec<PeerId> {
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

    pub(super) fn schedule_lane_block_message_to_validator_set(
        &mut self,
        msg: BlockMessage,
        validator_set: &[PeerId],
    ) -> usize {
        let local_peer = self.common_config.peer.id().clone();
        let mut scheduled = 0_usize;
        let mut seen = BTreeSet::new();
        for peer in validator_set {
            if peer == &local_peer || !seen.insert(peer.clone()) {
                continue;
            }
            self.schedule_background(BackgroundRequest::Post {
                peer: peer.clone(),
                msg: BlockMessageWire::new(msg.clone()),
            });
            scheduled = scheduled.saturating_add(1);
        }
        scheduled
    }

    fn local_lane_block_prepare_vote_for_proposal(
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
                "skipping local lane-block prepare vote for invalid proposal"
            );
            return None;
        }
        if !self.lane_block_route_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
        ) || !self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
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
                "skipping local lane-block prepare vote for non-authoritative route or committee"
            );
            return None;
        }
        match self.common_config.key_pair.public_key().try_algorithm() {
            Ok(iroha_crypto::Algorithm::BlsNormal) => {}
            Ok(algorithm) => {
                warn!(
                    ?algorithm,
                    "skipping local lane-block prepare vote broadcast with non-BLS consensus key"
                );
                return None;
            }
            Err(err) => {
                warn!(
                    ?err,
                    "skipping local lane-block prepare vote broadcast with unrecognized consensus key"
                );
                return None;
            }
        }
        if local_peer.public_key() != self.common_config.key_pair.public_key() {
            warn!("skipping local lane-block prepare vote for mismatched local peer key");
            return None;
        }
        if !self.lane_block_replay_prerequisites_available_for_vote(proposal, "prepare") {
            return None;
        }

        let body = proposal.vote_body(crate::sumeragi::consensus::Phase::Prepare);
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
                    "skipping local lane-block prepare vote after signing failure"
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

    fn broadcast_local_prepare_vote_for_proposal(
        &mut self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> bool {
        let local_peer = self.common_config.peer.id().clone();
        if !self
            .subsystems
            .lane_blocks
            .local_prepare_vote_needed_for(proposal, &local_peer)
        {
            return false;
        }
        let Some(vote) = self.local_lane_block_prepare_vote_for_proposal(proposal) else {
            return false;
        };
        if !self.lane_block_vote_accepts_local_broadcast(&vote, &local_peer) {
            return false;
        }
        let scheduled = self.schedule_lane_block_message_to_validator_set(
            BlockMessage::LaneBlockVote(vote.clone()),
            proposal.descriptor.validator_set.as_slice(),
        );
        if scheduled > 0 {
            debug!(
                scheduled,
                lane_id = ?vote.body.lane_id,
                dataspace_id = ?vote.body.dataspace_id,
                lane_block_height = vote.body.lane_block_height,
                lane_block_view = vote.body.lane_block_view,
                "posting local lane-block prepare vote to lane committee after inbound proposal"
            );
        }
        self.schedule_background(BackgroundRequest::Broadcast {
            msg: BlockMessageWire::new(BlockMessage::LaneBlockVote(vote.clone())),
        });
        let mut handled = true;
        if let Err(err) = self.handle_lane_block_vote(vote, Some(&local_peer)) {
            warn!(
                ?err,
                "failed to cache local lane-block prepare vote after inbound proposal"
            );
            handled = false;
        }
        handled
    }

    fn broadcast_local_prepare_vote_for_incoming_lane_block_proposal(
        &mut self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    ) {
        let _ = self.broadcast_local_prepare_vote_for_proposal(proposal);
    }

    pub(super) fn broadcast_ready_local_lane_block_votes(&mut self) -> bool {
        let recovered_proposals = self.cache_unapplied_lane_block_artifact_proposals();
        let rebroadcasted_proposals =
            self.rebroadcast_cached_lane_block_proposals_without_commit_qc();
        let local_peer = self.common_config.peer.id().clone();
        let proposals = self
            .subsystems
            .lane_blocks
            .local_prepare_vote_proposals_for(&local_peer);
        let mut progress = false;
        for proposal in proposals {
            progress |= self.broadcast_local_prepare_vote_for_proposal(&proposal);
        }
        if progress {
            self.broadcast_newly_sealed_lane_block_qcs();
        }
        let commit_votes = self.broadcast_lane_block_commit_votes_for_prepared_sessions();
        let rebroadcasted_votes = self.rebroadcast_cached_local_lane_block_votes_without_qc();
        let rebroadcasted_qcs = self.rebroadcast_cached_lane_block_qcs_for_incomplete_sessions();
        self.publish_lane_block_session_status();
        commit_votes > 0
            || progress
            || rebroadcasted_qcs > 0
            || rebroadcasted_votes > 0
            || rebroadcasted_proposals > 0
            || recovered_proposals > 0
    }

    fn rebroadcast_cached_lane_block_proposals_without_commit_qc(&mut self) -> usize {
        let mut scheduled = 0_usize;
        for proposal in self.subsystems.lane_blocks.proposals_without_commit_qc() {
            if !self.lane_block_route_accepts_ingress(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
            ) || !self.lane_block_authority_accepts_ingress(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.proposal_height,
                proposal.descriptor.validator_count,
                proposal.descriptor.min_quorum,
                proposal.descriptor.validator_set_hash,
                Some(proposal.descriptor.validator_set.as_slice()),
                None,
            ) {
                continue;
            }
            scheduled =
                scheduled.saturating_add(self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    proposal.descriptor.validator_set.as_slice(),
                ));
        }
        scheduled
    }

    fn rebroadcast_cached_local_lane_block_votes_without_qc(&mut self) -> usize {
        let local_peer = self.common_config.peer.id().clone();
        let mut scheduled = 0_usize;
        for (proposal, vote) in self
            .subsystems
            .lane_blocks
            .local_vote_rebroadcast_artifacts_for(&local_peer)
        {
            if self.lane_block_route_accepts_ingress(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
            ) && self.lane_block_authority_accepts_ingress(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.proposal_height,
                proposal.descriptor.validator_count,
                proposal.descriptor.min_quorum,
                proposal.descriptor.validator_set_hash,
                Some(proposal.descriptor.validator_set.as_slice()),
                None,
            ) {
                scheduled =
                    scheduled.saturating_add(self.schedule_lane_block_message_to_validator_set(
                        BlockMessage::LaneBlockProposal(proposal.clone()),
                        proposal.descriptor.validator_set.as_slice(),
                    ));
            }
            if !self.lane_block_vote_body_targets_authorized_local_signer(&vote.body, &local_peer) {
                continue;
            }
            scheduled =
                scheduled.saturating_add(self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockVote(vote.clone()),
                    proposal.descriptor.validator_set.as_slice(),
                ));
            self.schedule_background(BackgroundRequest::Broadcast {
                msg: BlockMessageWire::new(BlockMessage::LaneBlockVote(vote)),
            });
            scheduled = scheduled.saturating_add(1);
        }
        scheduled
    }

    fn rebroadcast_cached_lane_block_qcs_for_incomplete_sessions(&mut self) -> usize {
        let mut scheduled = 0_usize;
        for qc in self.subsystems.lane_blocks.qcs_for_incomplete_sessions() {
            if !self.lane_block_route_accepts_ingress(
                qc.body.lane_id,
                qc.body.dataspace_id,
                qc.body.proposal_height,
                qc.body.lane_block_height,
            ) || !self.lane_block_authority_accepts_ingress(
                qc.body.lane_id,
                qc.body.dataspace_id,
                qc.body.proposal_height,
                qc.body.validator_count,
                qc.body.min_quorum,
                qc.body.validator_set_hash,
                Some(qc.validator_set.as_slice()),
                None,
            ) {
                continue;
            }
            scheduled =
                scheduled.saturating_add(self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockQc(qc.clone()),
                    qc.validator_set.as_slice(),
                ));
            self.schedule_background(BackgroundRequest::Broadcast {
                msg: BlockMessageWire::new(BlockMessage::LaneBlockQc(qc)),
            });
            scheduled = scheduled.saturating_add(1);
        }
        scheduled
    }

    pub(super) fn lane_block_validator_set_for_vote(
        &self,
        vote: &crate::lane_consensus::LaneBlockVoteV1,
    ) -> Option<Vec<PeerId>> {
        let key = crate::lane_consensus::LaneBlockSessionKey {
            lane_id: vote.body.lane_id,
            dataspace_id: vote.body.dataspace_id,
            lane_block_height: vote.body.lane_block_height,
            lane_block_view: vote.body.lane_block_view,
            proposal_hash: vote.body.proposal_hash,
        };
        self.subsystems.lane_blocks.proposal_validator_set(&key)
    }

    pub(super) fn schedule_lane_block_vote_to_known_validator_set(
        &mut self,
        vote: &crate::lane_consensus::LaneBlockVoteV1,
    ) -> usize {
        let Some(validator_set) = self.lane_block_validator_set_for_vote(vote) else {
            warn!(
                lane_id = vote.body.lane_id.as_u32(),
                dataspace_id = vote.body.dataspace_id.as_u64(),
                lane_block_height = vote.body.lane_block_height,
                lane_block_view = vote.body.lane_block_view,
                "skipping lane-block vote fanout because proposal validator set is not cached"
            );
            return 0;
        };
        let scheduled = self.schedule_lane_block_message_to_validator_set(
            BlockMessage::LaneBlockVote(vote.clone()),
            validator_set.as_slice(),
        );
        if scheduled > 0 {
            debug!(
                scheduled,
                phase = ?vote.body.phase,
                lane_id = ?vote.body.lane_id,
                dataspace_id = ?vote.body.dataspace_id,
                lane_block_height = vote.body.lane_block_height,
                lane_block_view = vote.body.lane_block_view,
                "posting lane-block vote to lane committee"
            );
        }
        scheduled
    }

    fn log_lane_block_commit_vote_post(
        scheduled: usize,
        vote: &crate::lane_consensus::LaneBlockVoteV1,
        prepare_qc: &crate::sumeragi::consensus::LaneBlockQcV1,
    ) {
        if scheduled == 0 {
            return;
        }
        debug!(
            scheduled,
            lane_id = ?vote.body.lane_id,
            dataspace_id = ?vote.body.dataspace_id,
            lane_block_height = vote.body.lane_block_height,
            lane_block_view = vote.body.lane_block_view,
            prepare_qc_phase = ?prepare_qc.body.phase,
            "posting local lane-block commit vote to lane committee after prepare QC"
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
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
        ) || !self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
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
        if !self.lane_block_replay_prerequisites_available_for_vote(proposal, "commit") {
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

    fn lane_block_replay_prerequisites_available_for_vote(
        &self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
        phase: &'static str,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        if !self
            .state
            .kura()
            .lane_block_predecessor_application_receipt_available(proposal)
        {
            debug!(
                lane_id = ?descriptor.lane_id,
                dataspace_id = ?descriptor.dataspace_id,
                lane_block_height = descriptor.lane_block_height,
                lane_block_view = descriptor.lane_block_view,
                phase,
                previous_lane_block_height = descriptor.previous_lane_block_height,
                "deferring local lane-block vote until predecessor application receipt is available"
            );
            return false;
        }
        true
    }

    pub(super) fn broadcast_lane_block_commit_votes_for_prepared_sessions(&mut self) -> usize {
        let local_peer = self.common_config.peer.id().clone();
        let requests = self
            .subsystems
            .lane_blocks
            .local_commit_vote_requests_for(&local_peer);

        let mut produced = 0_usize;
        for request in requests {
            let Some(vote) = self.local_lane_block_commit_vote(&request.proposal) else {
                continue;
            };
            if self.lane_block_vote_accepts_local_broadcast(&vote, &local_peer) {
                let scheduled = self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockVote(vote.clone()),
                    request.proposal.descriptor.validator_set.as_slice(),
                );
                Self::log_lane_block_commit_vote_post(scheduled, &vote, &request.prepare_qc);
                self.schedule_background(BackgroundRequest::Broadcast {
                    msg: BlockMessageWire::new(BlockMessage::LaneBlockVote(vote.clone())),
                });
            }
            if let Err(err) = self.handle_lane_block_vote(vote, Some(&local_peer)) {
                warn!(
                    ?err,
                    "failed to cache local lane-block commit vote after prepare QC"
                );
            } else {
                produced = produced.saturating_add(1);
            }
        }
        if produced > 0 {
            self.broadcast_newly_sealed_lane_block_qcs();
        }
        produced
    }

    pub(super) fn broadcast_newly_sealed_lane_block_qcs(&mut self) {
        for qc in self.subsystems.lane_blocks.drain_newly_sealed_qcs() {
            if !self.lane_block_route_accepts_ingress(
                qc.body.lane_id,
                qc.body.dataspace_id,
                qc.body.proposal_height,
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
                qc.body.proposal_height,
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
                "posting sealed lane-block QC to lane committee"
            );
            let validator_set = qc.validator_set.clone();
            self.schedule_lane_block_message_to_validator_set(
                BlockMessage::LaneBlockQc(qc.clone()),
                validator_set.as_slice(),
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
        let reset_heights = self.state.da_shard_reset_heights_snapshot_cached();
        let admissible_lane = |lane_id: LaneId,
                               dataspace_id: DataSpaceId,
                               lane_block_height: u64,
                               proposal_height: u64| {
            crate::state::nexus_active_lane_dataspace_at_height(lane_id, &nexus, proposal_height)
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
        let payload_hint_repair_candidates = self
            .subsystems
            .committed_lane_blocks
            .payload_hint_repair_candidates(self.state.kura());
        let payload_hint_repair_requests = payload_hint_repair_candidates
            .iter()
            .map(|proposal| {
                self.request_lane_block_payload_hint_repair(
                    proposal,
                    "committed_lane_block_payload_hint",
                )
            })
            .sum::<usize>();
        let canonical_application_receipts = self
            .subsystems
            .committed_lane_blocks
            .record_available_payload_application_receipts_into_kura(self.state.kura());
        let canonical_receipted_status = if canonical_application_receipts > 0 {
            self.subsystems
                .committed_lane_blocks
                .status_snapshot_with_payload_availability(
                    self.state.kura(),
                    u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX),
                    Some(self.state.lane_execution_state_hash()),
                )
        } else {
            Vec::new()
        };
        let pruned_canonical_receipted = self
            .subsystems
            .committed_lane_blocks
            .prune_application_receipted_sessions(self.state.kura());
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
            .record_available_payload_application_receipts_into_kura(self.state.kura())
            .saturating_add(canonical_application_receipts);
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
            .prune_application_receipted_sessions(self.state.kura())
            .saturating_add(pruned_canonical_receipted);
        let queued_after_prune = if pruned_applied_after > 0 {
            self.enqueue_ready_committed_lane_block_sessions()
        } else {
            0
        };
        let final_committed_status = self
            .subsystems
            .committed_lane_blocks
            .status_snapshot_for_state(&self.state);
        let committed_status = super::merge_committed_lane_block_statuses(
            canonical_receipted_status,
            committed_status_before_prune,
        );
        let committed_status =
            super::merge_committed_lane_block_statuses(committed_status, final_committed_status);
        let pending = self.subsystems.committed_lane_blocks.len();
        super::status::set_committed_lane_blocks(committed_status);
        self.publish_lane_block_session_status();
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
            payload_hint_repair_requests,
            pending,
            "queued committed lane-block sessions for execution"
        );
        queued > 0
            || recovered_inputs > 0
            || payload_hint_repair_requests > 0
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LaneBlockProposalIngressOutcome {
    Inserted,
    Duplicate,
    Dropped,
}

#[cfg(test)]
mod lane_block_artifact_recovery_tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};

    fn peer(seed: u8) -> PeerId {
        let keypair =
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture keypair");
        PeerId::new(keypair.public_key().clone())
    }

    #[test]
    fn lane_block_proposal_from_payload_ownership_reconstructs_canonical_proposal() {
        let mut validator_set = vec![peer(3), peer(1), peer(2)];
        validator_set.sort();
        let mut descriptor = crate::sumeragi::consensus::LaneBlockDescriptorV1 {
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            proposal_height: 5,
            previous_lane_block_height: 2,
            previous_lane_block_descriptor_hash: Some(Hash::prehashed([0x20; Hash::LENGTH])),
            lane_block_height: 3,
            lane_block_view: 1,
            subject_hash: Hash::prehashed([0x21; Hash::LENGTH]),
            payload_ownership_hash: Hash::prehashed([0x22; Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([0x23; Hash::LENGTH]),
            accepted_candidate_indices: vec![4, 1],
            accepted_transaction_hashes: vec![
                Hash::prehashed([0x24; Hash::LENGTH]),
                Hash::prehashed([0x25; Hash::LENGTH]),
            ],
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count: u32::try_from(validator_set.len()).expect("validator count fits"),
            min_quorum: 2,
            qc_mode_tag: "fixture:lane:7:dataspace:11".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let ownership = SumeragiLanePayloadOwnership {
            proposal_height: 5,
            proposal_view: 0,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            subject_hash: descriptor.subject_hash,
            qc_mode_tag: descriptor.qc_mode_tag.clone(),
            accepted_candidate_indices: descriptor.accepted_candidate_indices.clone(),
            accepted_transaction_hashes: descriptor.accepted_transaction_hashes.clone(),
            previous_lane_block_height: descriptor.previous_lane_block_height,
            previous_lane_block_descriptor_hash: descriptor.previous_lane_block_descriptor_hash,
            lane_block_descriptor_hash: Some(descriptor.descriptor_hash),
            lane_block_descriptor_validator_set: descriptor.validator_set.clone(),
            lane_block_descriptor_validator_count: descriptor.validator_count,
            lane_block_descriptor_min_quorum: descriptor.min_quorum,
            payload_ownership_hash: descriptor.payload_ownership_hash,
            rbc_instance_hash: descriptor.rbc_instance_hash,
        };

        let proposal_block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x31; Hash::LENGTH]));
        let proposal = Actor::lane_block_proposal_from_payload_ownership(
            &ownership,
            Some(proposal_block_hash),
        )
        .expect("ownership should reconstruct a canonical lane-block proposal");

        assert_eq!(proposal.descriptor, descriptor);
        assert_eq!(proposal.proposal_hash, proposal.computed_proposal_hash());
        let hint = proposal
            .payload_block_hint
            .expect("proposal reconstructed from a sidecar should carry the block hint");
        assert_eq!(hint.proposal_height, ownership.proposal_height);
        assert_eq!(hint.proposal_view, ownership.proposal_view);
        assert_eq!(hint.proposal_block_hash, proposal_block_hash);
    }

    #[test]
    fn lane_block_proposal_from_payload_ownership_rejects_descriptor_hash_drift() {
        let mut validator_set = vec![peer(1), peer(2), peer(3)];
        validator_set.sort();
        let ownership = SumeragiLanePayloadOwnership {
            proposal_height: 5,
            proposal_view: 0,
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_block_height: 3,
            lane_block_view: 1,
            subject_hash: Hash::prehashed([0x21; Hash::LENGTH]),
            qc_mode_tag: "fixture:lane:7:dataspace:11".to_owned(),
            accepted_candidate_indices: vec![4, 1],
            accepted_transaction_hashes: vec![
                Hash::prehashed([0x24; Hash::LENGTH]),
                Hash::prehashed([0x25; Hash::LENGTH]),
            ],
            previous_lane_block_height: 2,
            previous_lane_block_descriptor_hash: Some(Hash::prehashed([0x20; Hash::LENGTH])),
            lane_block_descriptor_hash: Some(Hash::prehashed([0xFF; Hash::LENGTH])),
            lane_block_descriptor_validator_set: validator_set.clone(),
            lane_block_descriptor_validator_count: u32::try_from(validator_set.len())
                .expect("validator count fits"),
            lane_block_descriptor_min_quorum: 2,
            payload_ownership_hash: Hash::prehashed([0x22; Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([0x23; Hash::LENGTH]),
        };

        assert!(Actor::lane_block_proposal_from_payload_ownership(&ownership, None).is_none());
    }
}

fn lane_block_session_error_reason(
    err: crate::lane_consensus::LaneBlockSessionError,
) -> super::status::ConsensusMessageReason {
    match err {
        crate::lane_consensus::LaneBlockSessionError::InvalidVote(
            crate::lane_consensus::LaneBlockVoteIngressError::InvalidSignature
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
