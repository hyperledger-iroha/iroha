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
        sender: Option<&PeerId>,
    ) -> Result<()> {
        if !self.lane_block_proposal_sender_accepts_ingress(&proposal, sender) {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidSignature,
            );
            warn!(
                lane_id = proposal.descriptor.lane_id.as_u32(),
                dataspace_id = proposal.descriptor.dataspace_id.as_u64(),
                proposal_height = proposal.descriptor.proposal_height,
                lane_block_height = proposal.descriptor.lane_block_height,
                ?sender,
                "dropping lane-block proposal without an authenticated consensus sender"
            );
            return Ok(());
        }
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

    pub(super) fn handle_lane_executable_payload(
        &mut self,
        payload: crate::lane_consensus::LaneExecutablePayloadV1,
        sender: Option<&PeerId>,
    ) -> Result<()> {
        let descriptor = &payload.origin_proposal.descriptor;
        let expected_epoch = self.epoch_for_height(descriptor.proposal_height);
        let global_proposal_hint = payload.origin_proposal.payload_block_hint;
        let producer_is_lane_leader =
            super::lane_scheduler::lane_block_redrive_leader(&payload.origin_proposal, 0)
                == Some(&payload.producer);
        let local_peer = self.common_config.peer.id();
        let locally_constructed_by_global_lane_leader = global_proposal_hint.is_some_and(|hint| {
            sender == Some(local_peer)
                && payload.producer == *local_peer
                && self.lane_payload_global_proposer(hint).as_ref() == Some(local_peer)
        });
        let anchored_payload_is_authorized = global_proposal_hint.is_none_or(|hint| {
            (self.lane_payload_global_anchor_observed(hint)
                && self.lane_payload_global_anchor_matches(
                    hint,
                    &payload.origin_proposal,
                    &payload.entrypoints,
                ))
                || locally_constructed_by_global_lane_leader
        });
        let sender_authorized = sender.is_some_and(|sender| {
            descriptor.validator_set.contains(sender)
                || self
                    .shared_lane_block_authority_for_ingress(descriptor.proposal_height)
                    .contains(sender)
        });
        if !sender_authorized
            || payload.validate(self.chain_hash, expected_epoch).is_err()
            || !producer_is_lane_leader
            || !anchored_payload_is_authorized
            || !self.lane_block_route_accepts_ingress(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.proposal_height,
                descriptor.lane_block_height,
            )
            || !self.lane_block_authority_accepts_ingress(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.proposal_height,
                descriptor.lane_block_height,
                payload.origin_proposal.proposal_hash,
                descriptor.validator_count,
                descriptor.min_quorum,
                descriptor.validator_set_hash,
                Some(descriptor.validator_set.as_slice()),
                Some(&payload.producer),
            )
        {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidSignature,
            );
            warn!(
                lane_id = descriptor.lane_id.as_u32(),
                lane_block_height = descriptor.lane_block_height,
                ?sender,
                "dropping unauthorized or invalid lane executable payload"
            );
            return Ok(());
        }

        let already_present = self
            .state
            .kura()
            .read_autonomous_lane_block_artifact(
                descriptor.lane_id,
                descriptor.lane_block_height,
                self.chain_hash,
                expected_epoch,
            )
            .is_some();
        if !already_present && !self.lane_executable_payload_passes_stateful_preflight(&payload) {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            warn!(
                lane_id = descriptor.lane_id.as_u32(),
                lane_block_height = descriptor.lane_block_height,
                "dropping lane executable payload that fails routing/admission preflight"
            );
            return Ok(());
        }
        if let Err(err) = self.state.kura().persist_lane_executable_payload(
            &payload,
            self.chain_hash,
            expected_epoch,
        ) {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            warn!(?err, "dropping conflicting lane executable payload");
            return Ok(());
        }

        if !already_present {
            self.schedule_lane_block_message_to_validator_set(
                BlockMessage::LaneExecutablePayload(payload.clone()),
                descriptor.validator_set.as_slice(),
            );
            if sender == Some(&payload.producer)
                && payload.producer == *self.common_config.peer.id()
            {
                self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockProposal(payload.origin_proposal.clone()),
                    descriptor.validator_set.as_slice(),
                );
            }
        }
        let proposal = payload.origin_proposal.clone();
        let outcome = self.cache_lane_block_proposal(proposal.clone());
        if matches!(
            outcome,
            LaneBlockProposalIngressOutcome::Inserted | LaneBlockProposalIngressOutcome::Duplicate
        ) {
            self.broadcast_local_prepare_vote_for_incoming_lane_block_proposal(&proposal);
            self.broadcast_lane_block_commit_votes_for_prepared_sessions();
        }
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::LaneBlockProposal,
            if already_present {
                super::status::ConsensusMessageOutcome::Dropped
            } else {
                super::status::ConsensusMessageOutcome::Deferred
            },
            if already_present {
                super::status::ConsensusMessageReason::Duplicate
            } else {
                super::status::ConsensusMessageReason::PayloadUnapplied
            },
        );
        Ok(())
    }

    fn lane_executable_payload_passes_stateful_preflight(
        &self,
        payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    ) -> bool {
        let Ok(input) = crate::kura::Kura::autonomous_lane_block_execution_input_candidate(
            payload,
            self.chain_hash,
            payload.epoch,
        ) else {
            return false;
        };
        let current_height = u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
        let next_height = current_height.saturating_add(1).max(1);
        let header = BlockHeader::new(
            NonZeroU64::new(next_height).expect("lane preflight height is non-zero"),
            Some(self.state.lane_execution_state_hash()),
            None,
            None,
            0,
            0,
        );
        let mut state_block = self.state.lane_application_block(header);
        let mut ivm_cache = crate::smartcontracts::ivm::cache::IvmCache::new();
        state_block
            .validate_lane_block_execution_input_with_routing_context(&input, &mut ivm_cache)
            .is_ok_and(|results| results.iter().all(|(_, _, result)| result.is_ok()))
    }

    pub(super) fn handle_lane_executable_payload_handoff(
        &mut self,
        handoff: crate::lane_consensus::LaneExecutablePayloadHandoffV1,
        sender: Option<&PeerId>,
    ) -> Result<()> {
        let descriptor = &handoff.origin_proposal.descriptor;
        let expected_epoch = self.epoch_for_height(descriptor.proposal_height);
        let global_authority =
            self.shared_lane_block_authority_for_ingress(descriptor.proposal_height);
        let local_peer = self.common_config.peer.id().clone();
        let global_proposal_hint = handoff.origin_proposal.payload_block_hint;
        let proposer_is_global_leader = global_proposal_hint.is_some_and(|hint| {
            self.lane_payload_global_proposer(hint).as_ref() == Some(&handoff.proposer)
        });
        let local_is_active_committee_member = descriptor.validator_set.contains(&local_peer)
            && local_peer.public_key() == self.common_config.key_pair.public_key()
            && matches!(
                self.common_config.key_pair.public_key().try_algorithm(),
                Ok(iroha_crypto::Algorithm::BlsNormal)
            )
            && super::lane_scheduler::lane_block_redrive_leader(&handoff.origin_proposal, 0)
                == Some(&local_peer);
        if handoff.validate(self.chain_hash, expected_epoch).is_err()
            || handoff
                .validate_sender_authority(sender, &global_authority)
                .is_err()
            || !proposer_is_global_leader
            || !local_is_active_committee_member
            || !self.lane_block_route_accepts_ingress(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.proposal_height,
                descriptor.lane_block_height,
            )
            || !self.lane_block_authority_accepts_ingress(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.proposal_height,
                descriptor.lane_block_height,
                handoff.origin_proposal.proposal_hash,
                descriptor.validator_count,
                descriptor.min_quorum,
                descriptor.validator_set_hash,
                Some(descriptor.validator_set.as_slice()),
                Some(&local_peer),
            )
        {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidSignature,
            );
            warn!(
                lane_id = descriptor.lane_id.as_u32(),
                lane_block_height = descriptor.lane_block_height,
                ?sender,
                "dropping unauthorized or invalid lane executable payload handoff"
            );
            return Ok(());
        }

        let global_proposal_hint = global_proposal_hint
            .expect("validated lane payload handoff carries a global proposal hint");
        if !self.lane_payload_global_anchor_observed(global_proposal_hint) {
            match self.subsystems.lane_payload_handoffs.insert(handoff) {
                Ok(_) => self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Deferred,
                    super::status::ConsensusMessageReason::PayloadUnapplied,
                ),
                Err(err) => {
                    warn!(?err, "dropping conflicting deferred lane payload handoff");
                    self.record_consensus_message_handling(
                        super::status::ConsensusMessageKind::LaneBlockProposal,
                        super::status::ConsensusMessageOutcome::Dropped,
                        super::status::ConsensusMessageReason::PayloadMismatch,
                    );
                }
            }
            return Ok(());
        }
        if !self.lane_payload_global_anchor_matches(
            global_proposal_hint,
            &handoff.origin_proposal,
            &handoff.entrypoints,
        ) {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PayloadMismatch,
            );
            return Ok(());
        }

        let payload = match crate::lane_consensus::LaneExecutablePayloadV1::new_signed(
            handoff.chain_id_hash,
            handoff.epoch,
            handoff.origin_proposal,
            handoff.entrypoints,
            local_peer.clone(),
            self.common_config.key_pair.private_key(),
        ) {
            Ok(payload) if payload.payload_hash == handoff.payload_hash => payload,
            Ok(_) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::PayloadMismatch,
                );
                return Ok(());
            }
            Err(err) => {
                warn!(?err, "failed to re-sign verified lane payload handoff");
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockProposal,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::InvalidPayload,
                );
                return Ok(());
            }
        };
        self.handle_lane_executable_payload(payload, Some(&local_peer))
    }

    fn process_deferred_lane_payload_handoffs(&mut self) -> usize {
        let deferred = self.subsystems.lane_payload_handoffs.snapshot();
        let mut processed = 0_usize;
        for handoff in deferred {
            let Some(global_proposal_hint) = handoff.origin_proposal.payload_block_hint else {
                self.subsystems.lane_payload_handoffs.remove(&handoff);
                continue;
            };
            if !self.lane_payload_global_anchor_observed(global_proposal_hint) {
                continue;
            }
            self.subsystems.lane_payload_handoffs.remove(&handoff);
            let proposer = handoff.proposer.clone();
            if let Err(err) = self.handle_lane_executable_payload_handoff(handoff, Some(&proposer))
            {
                warn!(?err, "failed to resume deferred lane payload handoff");
                continue;
            }
            processed = processed.saturating_add(1);
        }
        processed
    }

    pub(super) fn handle_lane_block_new_view_vote(
        &mut self,
        vote: crate::lane_consensus::LaneBlockNewViewVoteV1,
        sender: Option<&PeerId>,
    ) -> Result<()> {
        let body = &vote.body;
        if sender != Some(&vote.signer)
            || body.chain_id_hash != self.chain_hash
            || body.epoch != self.epoch_for_height(body.proposal_height)
            || vote.validate_ingress().is_err()
            || !self.lane_block_route_accepts_ingress(
                body.lane_id,
                body.dataspace_id,
                body.lane_incarnation,
                body.proposal_height,
                body.lane_block_height,
            )
            || !self.lane_block_authority_accepts_ingress(
                body.lane_id,
                body.dataspace_id,
                body.lane_incarnation,
                body.proposal_height,
                body.lane_block_height,
                body.locked_proposal_hash,
                body.validator_count,
                body.min_quorum,
                body.validator_set_hash,
                None,
                Some(&vote.signer),
            )
        {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidSignature,
            );
            return Ok(());
        }
        let Some((payload, current)) = self.state.kura().current_autonomous_lane_payload(
            body.lane_id,
            body.lane_block_height,
            self.chain_hash,
            body.epoch,
        ) else {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::PayloadUnapplied,
            );
            return Ok(());
        };
        let expected_body = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
            &current,
            &payload,
            body.target_view,
            self.chain_hash,
            body.epoch,
        );
        if expected_body.as_ref() != Ok(body) {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockVote,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return Ok(());
        }
        let validator_set = current.descriptor.validator_set.clone();
        let (_, sealed) = match self
            .subsystems
            .lane_new_view_votes
            .insert_and_maybe_seal(vote, &validator_set)
        {
            Ok(result) => result,
            Err(err) => {
                warn!(?err, "dropping conflicting lane NewView vote");
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::ConflictingVote,
                );
                return Ok(());
            }
        };
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::LaneBlockVote,
            super::status::ConsensusMessageOutcome::Deferred,
            super::status::ConsensusMessageReason::PayloadUnapplied,
        );
        if let Some(certificate) = sealed {
            self.install_lane_block_new_view_certificate(certificate, true)?;
        }
        Ok(())
    }

    pub(super) fn handle_lane_block_new_view_certificate(
        &mut self,
        certificate: crate::lane_consensus::LaneBlockNewViewCertificateV1,
        sender: Option<&PeerId>,
    ) -> Result<()> {
        let sender_authorized = sender.is_some_and(|sender| {
            certificate.validator_set.contains(sender)
                || self
                    .shared_lane_block_authority_for_ingress(certificate.body.proposal_height)
                    .contains(sender)
        });
        if !sender_authorized {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockQc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidSignature,
            );
            return Ok(());
        }
        self.install_lane_block_new_view_certificate(certificate, false)
    }

    fn install_lane_block_new_view_certificate(
        &mut self,
        certificate: crate::lane_consensus::LaneBlockNewViewCertificateV1,
        locally_sealed: bool,
    ) -> Result<()> {
        let body = &certificate.body;
        if body.chain_id_hash != self.chain_hash
            || body.epoch != self.epoch_for_height(body.proposal_height)
            || !self.lane_block_route_accepts_ingress(
                body.lane_id,
                body.dataspace_id,
                body.lane_incarnation,
                body.proposal_height,
                body.lane_block_height,
            )
            || !self.lane_block_authority_accepts_ingress(
                body.lane_id,
                body.dataspace_id,
                body.lane_incarnation,
                body.proposal_height,
                body.lane_block_height,
                body.locked_proposal_hash,
                body.validator_count,
                body.min_quorum,
                body.validator_set_hash,
                Some(certificate.validator_set.as_slice()),
                None,
            )
        {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockQc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidPayload,
            );
            return Ok(());
        }
        let signer_pops = self.lane_new_view_certificate_signer_pops(&certificate);
        if crate::lane_consensus::validate_lane_block_new_view_certificate(
            &certificate,
            &signer_pops,
        )
        .is_err()
        {
            self.record_consensus_message_handling(
                super::status::ConsensusMessageKind::LaneBlockQc,
                super::status::ConsensusMessageOutcome::Dropped,
                super::status::ConsensusMessageReason::InvalidSignature,
            );
            return Ok(());
        }
        if let Some(existing) = self.state.kura().read_autonomous_lane_block_artifact(
            body.lane_id,
            body.lane_block_height,
            self.chain_hash,
            body.epoch,
        ) && (existing
            .new_view_certificates
            .iter()
            .any(|durable| durable.certificate == certificate)
            || existing
                .view_checkpoint
                .as_ref()
                .is_some_and(|checkpoint| checkpoint.certificate.certificate == certificate))
        {
            return Ok(());
        }
        let durable = crate::lane_consensus::DurableLaneBlockNewViewCertificateV1 {
            certificate: certificate.clone(),
            signer_pops: signer_pops.clone(),
        };
        let target = match self.state.kura().persist_lane_new_view_certificate(
            body.lane_id,
            body.lane_block_height,
            durable,
            self.chain_hash,
            body.epoch,
        ) {
            Ok(target) => target,
            Err(err) => {
                warn!(?err, "dropping conflicting lane NewView certificate");
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockQc,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::InvalidPayload,
                );
                return Ok(());
            }
        };
        let outcome = self
            .subsystems
            .lane_new_view_certificates
            .insert(certificate.clone(), &signer_pops);
        if outcome.is_err() {
            return Ok(());
        }
        let _ = locally_sealed;
        self.schedule_lane_block_message_to_validator_set(
            BlockMessage::LaneBlockNewViewCertificate(certificate),
            target.descriptor.validator_set.as_slice(),
        );
        let outcome = self.cache_lane_block_proposal(target.clone());
        if matches!(
            outcome,
            LaneBlockProposalIngressOutcome::Inserted | LaneBlockProposalIngressOutcome::Duplicate
        ) {
            self.schedule_lane_block_message_to_validator_set(
                BlockMessage::LaneBlockProposal(target.clone()),
                target.descriptor.validator_set.as_slice(),
            );
            self.broadcast_local_prepare_vote_for_incoming_lane_block_proposal(&target);
        }
        self.record_consensus_message_handling(
            super::status::ConsensusMessageKind::LaneBlockQc,
            super::status::ConsensusMessageOutcome::Deferred,
            super::status::ConsensusMessageReason::PayloadUnapplied,
        );
        Ok(())
    }

    pub(super) fn publish_lane_block_session_status(&self) {
        let nexus = self.state.nexus_snapshot();
        let entries = self
            .subsystems
            .lane_blocks
            .status_snapshot()
            .into_iter()
            .filter(|entry| {
                self.state.lane_incarnation(entry.lane_id) == Some(entry.lane_incarnation)
                    && crate::state::consensus_lane_dataspace_at_height(
                        entry.lane_id,
                        &nexus,
                        u64::try_from(self.state.committed_height())
                            .unwrap_or(u64::MAX)
                            .saturating_add(1),
                    ) == Some(entry.dataspace_id)
            })
            .collect();
        super::status::set_lane_block_sessions(entries);
    }

    fn authorize_cached_autonomous_lane_payload(
        &mut self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> bool {
        let epoch = self.epoch_for_height(proposal.descriptor.proposal_height);
        let Some(artifact) = self.state.kura().read_autonomous_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
            self.chain_hash,
            epoch,
        ) else {
            return true;
        };
        let body = match crate::lane_consensus::lane_payload_availability_body(
            &artifact.executable_payload,
            proposal,
            self.chain_hash,
            epoch,
        ) {
            Ok(body) => body,
            Err(err) => {
                warn!(
                    ?err,
                    lane_id = proposal.descriptor.lane_id.as_u32(),
                    lane_block_height = proposal.descriptor.lane_block_height,
                    lane_block_view = proposal.descriptor.lane_block_view,
                    "refusing to authorize mismatched autonomous payload session"
                );
                return false;
            }
        };
        match self
            .subsystems
            .lane_blocks
            .authorize_payload_availability(proposal, body)
        {
            Ok(()) => true,
            Err(err) => {
                warn!(
                    ?err,
                    lane_id = proposal.descriptor.lane_id.as_u32(),
                    lane_block_height = proposal.descriptor.lane_block_height,
                    lane_block_view = proposal.descriptor.lane_block_view,
                    "refusing to authorize autonomous payload session"
                );
                false
            }
        }
    }

    fn cache_lane_block_proposal(
        &mut self,
        proposal: crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> LaneBlockProposalIngressOutcome {
        if !self.lane_block_route_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_incarnation,
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
        if !self.lane_block_slot_within_ingress_horizon(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
            proposal.proposal_hash,
        ) {
            self.record_lane_block_horizon_drop(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
            );
            return LaneBlockProposalIngressOutcome::Dropped;
        }
        if !self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_incarnation,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
            proposal.proposal_hash,
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
                if !self.authorize_cached_autonomous_lane_payload(&proposal_for_repair) {
                    return LaneBlockProposalIngressOutcome::Dropped;
                }
                self.observe_lane_block_proposal_redrive(&proposal_for_repair);
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
                if !self.authorize_cached_autonomous_lane_payload(&proposal_for_repair) {
                    return LaneBlockProposalIngressOutcome::Dropped;
                }
                self.observe_lane_block_proposal_redrive(&proposal_for_repair);
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
            proposal.descriptor.lane_incarnation,
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
        if !self.lane_block_slot_within_ingress_horizon(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
            proposal.proposal_hash,
        ) {
            self.record_lane_block_horizon_drop(
                super::status::ConsensusMessageKind::LaneBlockProposal,
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
            );
            return LaneBlockProposalIngressOutcome::Dropped;
        }
        if !self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_incarnation,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
            proposal.proposal_hash,
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
                if !self.authorize_cached_autonomous_lane_payload(&proposal_for_repair) {
                    return LaneBlockProposalIngressOutcome::Dropped;
                }
                self.observe_lane_block_proposal_redrive(&proposal_for_repair);
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
                if !self.authorize_cached_autonomous_lane_payload(&proposal_for_repair) {
                    return LaneBlockProposalIngressOutcome::Dropped;
                }
                self.observe_lane_block_proposal_redrive(&proposal_for_repair);
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
        if hint.proposal_height == 0 || hint.proposal_height != proposal.descriptor.proposal_height
        {
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
            vote.body.lane_incarnation,
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
            vote.body.lane_incarnation,
            vote.body.proposal_height,
            vote.body.lane_block_height,
            vote.body.proposal_hash,
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
        if !self.lane_block_slot_within_ingress_horizon(
            vote.body.lane_id,
            vote.body.dataspace_id,
            vote.body.proposal_height,
            vote.body.lane_block_height,
            vote.body.proposal_hash,
        ) {
            self.record_lane_block_horizon_drop(
                super::status::ConsensusMessageKind::LaneBlockVote,
                vote.body.lane_id,
                vote.body.dataspace_id,
                vote.body.proposal_height,
                vote.body.lane_block_height,
            );
            return Ok(());
        }

        if vote.payload_availability_vote.is_some() {
            let key = crate::lane_consensus::LaneBlockSessionKey {
                lane_id: vote.body.lane_id,
                dataspace_id: vote.body.dataspace_id,
                lane_incarnation: vote.body.lane_incarnation,
                lane_block_height: vote.body.lane_block_height,
                lane_block_view: vote.body.lane_block_view,
                proposal_hash: vote.body.proposal_hash,
            };
            let Some(proposal) = self.subsystems.lane_blocks.proposal_for_key(&key) else {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::PayloadUnapplied,
                );
                return Ok(());
            };
            if !self.authorize_cached_autonomous_lane_payload(&proposal) {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockVote,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::InvalidPayload,
                );
                return Ok(());
            }
        }

        match self.subsystems.lane_blocks.insert_vote(vote, sender) {
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Inserted) => {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockVote,
                    super::status::ConsensusMessageOutcome::Deferred,
                    super::status::ConsensusMessageReason::PayloadUnapplied,
                );
                debug!("cached validated lane-block vote for live lane session");
                self.broadcast_missing_local_lane_block_prepare_votes();
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
            qc.body.lane_incarnation,
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
            qc.body.lane_incarnation,
            qc.body.proposal_height,
            qc.body.lane_block_height,
            qc.body.proposal_hash,
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
        if !self.lane_block_slot_within_ingress_horizon(
            qc.body.lane_id,
            qc.body.dataspace_id,
            qc.body.proposal_height,
            qc.body.lane_block_height,
            qc.body.proposal_hash,
        ) {
            self.record_lane_block_horizon_drop(
                super::status::ConsensusMessageKind::LaneBlockQc,
                qc.body.lane_id,
                qc.body.dataspace_id,
                qc.body.proposal_height,
                qc.body.lane_block_height,
            );
            return Ok(());
        }

        if qc.payload_availability_qc.is_some() {
            let key = crate::lane_consensus::LaneBlockSessionKey {
                lane_id: qc.body.lane_id,
                dataspace_id: qc.body.dataspace_id,
                lane_incarnation: qc.body.lane_incarnation,
                lane_block_height: qc.body.lane_block_height,
                lane_block_view: qc.body.lane_block_view,
                proposal_hash: qc.body.proposal_hash,
            };
            let Some(proposal) = self.subsystems.lane_blocks.proposal_for_key(&key) else {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockQc,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::PayloadUnapplied,
                );
                return Ok(());
            };
            if !self.authorize_cached_autonomous_lane_payload(&proposal) {
                self.record_consensus_message_handling(
                    super::status::ConsensusMessageKind::LaneBlockQc,
                    super::status::ConsensusMessageOutcome::Dropped,
                    super::status::ConsensusMessageReason::InvalidPayload,
                );
                return Ok(());
            }
        }

        let pops = self.lane_block_qc_signer_pops(&qc);
        match self
            .subsystems
            .lane_blocks
            .insert_qc_with_pops(qc.clone(), &pops)
        {
            Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Inserted) => {
                self.persist_autonomous_lane_payload_availability_deliver(&qc);
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
                // A crash may have persisted the session cache but not the
                // independently replaceable availability state. Replaying an
                // exact prepare QC repairs that bounded durable certificate.
                self.persist_autonomous_lane_payload_availability_deliver(&qc);
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

    /// Persist a validated prepare QC as the restart-verifiable availability
    /// DELIVER certificate for an autonomous payload, when one exists.
    fn persist_autonomous_lane_payload_availability_deliver(
        &self,
        qc: &crate::sumeragi::consensus::LaneBlockQcV1,
    ) -> bool {
        if qc.body.phase != crate::sumeragi::consensus::Phase::Prepare {
            return true;
        }
        let epoch = self.epoch_for_height(qc.body.proposal_height);
        let Some(artifact) = self.state.kura().read_autonomous_lane_block_artifact(
            qc.body.lane_id,
            qc.body.lane_block_height,
            self.chain_hash,
            epoch,
        ) else {
            // Global-block-backed lane proposals use the existing canonical
            // block availability path and do not need this sidecar.
            return true;
        };
        if qc.payload_availability_qc.is_none() {
            warn!(
                lane_id = qc.body.lane_id.as_u32(),
                lane_block_height = qc.body.lane_block_height,
                lane_block_view = qc.body.lane_block_view,
                "refusing autonomous prepare QC without exact signed payload availability proof"
            );
            return false;
        }
        let durable = crate::lane_consensus::DurableLanePayloadAvailabilityCertificateV1 {
            certificate: qc.clone(),
        };
        if let Err(err) = crate::lane_consensus::validate_lane_payload_availability_certificate(
            &durable,
            &artifact.executable_payload,
            self.chain_hash,
            epoch,
        ) {
            warn!(
                ?err,
                "refusing invalid autonomous payload availability certificate"
            );
            return false;
        }
        match self
            .state
            .kura()
            .persist_lane_payload_availability_certificate(
                qc.body.lane_id,
                qc.body.lane_block_height,
                durable,
                self.chain_hash,
                epoch,
            ) {
            Ok(()) => true,
            Err(err) => {
                warn!(
                    ?err,
                    lane_id = qc.body.lane_id.as_u32(),
                    lane_block_height = qc.body.lane_block_height,
                    lane_block_view = qc.body.lane_block_view,
                    "failed to persist autonomous lane payload availability DELIVER certificate"
                );
                false
            }
        }
    }

    pub(super) fn lane_block_artifact_targets_active_route(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        proposal_height: u64,
    ) -> bool {
        let nexus = self.state.nexus_snapshot();
        crate::state::consensus_lane_dataspace_at_height(lane_id, &nexus, proposal_height)
            == Some(dataspace_id)
            && self
                .state
                .lane_incarnation_at_height(lane_id, proposal_height)
                == Some(lane_incarnation)
    }

    fn lane_block_route_accepts_ingress(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        proposal_height: u64,
        lane_block_height: u64,
    ) -> bool {
        let next_global_height = u64::try_from(self.state.committed_height())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        if proposal_height == 0 || proposal_height > next_global_height || lane_block_height == 0 {
            return false;
        }
        self.lane_block_artifact_targets_active_route(
            lane_id,
            dataspace_id,
            lane_incarnation,
            proposal_height,
        )
    }

    fn lane_block_slot_is_durably_finalized(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        lane_block_height: u64,
    ) -> bool {
        if self
            .state
            .kura()
            .read_lane_block_artifact(lane_id, lane_block_height)
            .filter(|artifact| {
                let ownership = &artifact.ownership;
                ownership.dataspace_id == dataspace_id
                    && ownership.lane_incarnation == lane_incarnation
                    && self
                        .state
                        .da_lane_visible_after_reset(ownership.proposal_height, lane_id)
                    && self.lane_block_artifact_targets_active_route(
                        lane_id,
                        dataspace_id,
                        lane_incarnation,
                        ownership.proposal_height,
                    )
            })
            .is_some_and(|artifact| {
                self.state
                    .lane_block_artifact_is_applied_or_snapshot_anchored_cached(&artifact)
            })
        {
            return true;
        }

        self.state
            .kura()
            .read_certified_lane_block_artifact(lane_id, lane_block_height)
            .filter(|artifact| {
                let descriptor = &artifact.proposal.descriptor;
                descriptor.dataspace_id == dataspace_id
                    && descriptor.lane_incarnation == lane_incarnation
                    && self
                        .state
                        .da_lane_visible_after_reset(descriptor.proposal_height, lane_id)
                    && self.lane_block_artifact_targets_active_route(
                        lane_id,
                        dataspace_id,
                        lane_incarnation,
                        descriptor.proposal_height,
                    )
            })
            .is_some_and(|artifact| {
                let session = crate::lane_consensus::CommittedLaneBlockSession {
                    proposal: artifact.proposal,
                    prepare_qc: artifact.prepare_qc,
                    commit_qc: artifact.commit_qc,
                };
                self.state
                    .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(&session)
            })
    }

    fn exact_kura_lane_block_proposal(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        proposal_height: u64,
        lane_block_height: u64,
        proposal_hash: Hash,
    ) -> Option<crate::sumeragi::consensus::LaneBlockProposalV1> {
        if let Some(artifact) = self
            .state
            .kura()
            .read_certified_lane_block_artifact(lane_id, lane_block_height)
        {
            let descriptor = &artifact.proposal.descriptor;
            if descriptor.dataspace_id == dataspace_id
                && descriptor.proposal_height == proposal_height
                && artifact.proposal.proposal_hash == proposal_hash
            {
                return Some(artifact.proposal);
            }
        }

        self.state
            .kura()
            .read_lane_block_artifact(lane_id, lane_block_height)
            .filter(|artifact| {
                artifact.ownership.dataspace_id == dataspace_id
                    && artifact.ownership.proposal_height == proposal_height
            })
            .and_then(|artifact| {
                Self::lane_block_proposal_from_payload_ownership(
                    &artifact.ownership,
                    Some(artifact.proposal_block_hash),
                )
            })
            .filter(|proposal| proposal.proposal_hash == proposal_hash)
    }

    fn lane_block_slot_within_ingress_horizon(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        proposal_height: u64,
        lane_block_height: u64,
        proposal_hash: Hash,
    ) -> bool {
        let Some(lane_incarnation) = self
            .state
            .lane_incarnation_at_height(lane_id, proposal_height)
        else {
            return false;
        };
        if !self
            .state
            .da_lane_visible_after_reset(proposal_height, lane_id)
            || !self.lane_block_artifact_targets_active_route(
                lane_id,
                dataspace_id,
                lane_incarnation,
                proposal_height,
            )
        {
            return false;
        }
        if self.lane_block_slot_is_durably_finalized(
            lane_id,
            dataspace_id,
            lane_incarnation,
            lane_block_height,
        ) {
            return false;
        }

        let global_next_height = self.committed_height_snapshot().saturating_add(1);
        if proposal_height > global_next_height {
            return false;
        }
        if proposal_height < global_next_height
            && self
                .exact_kura_lane_block_proposal(
                    lane_id,
                    dataspace_id,
                    proposal_height,
                    lane_block_height,
                    proposal_hash,
                )
                .is_none()
        {
            return false;
        }

        let mut finalized_height = 0_u64;
        let mut unresolved_heights = BTreeSet::new();
        if let Some(artifact) =
            self.state
                .kura()
                .latest_lane_block_artifact_matching(lane_id, |artifact| {
                    let ownership = &artifact.ownership;
                    ownership.dataspace_id == dataspace_id
                        && ownership.lane_incarnation == lane_incarnation
                        && self
                            .state
                            .da_lane_visible_after_reset(ownership.proposal_height, lane_id)
                        && self.lane_block_artifact_targets_active_route(
                            lane_id,
                            dataspace_id,
                            lane_incarnation,
                            ownership.proposal_height,
                        )
                })
        {
            let ownership = &artifact.ownership;
            let height = ownership.lane_block_height;
            if self
                .state
                .lane_block_artifact_is_applied_or_snapshot_anchored_cached(&artifact)
            {
                finalized_height = finalized_height.max(height);
            } else {
                unresolved_heights.insert(height);
            }
        }
        if let Some(artifact) = self
            .state
            .kura()
            .latest_certified_lane_block_artifact_matching(lane_id, |artifact| {
                let descriptor = &artifact.proposal.descriptor;
                descriptor.dataspace_id == dataspace_id
                    && descriptor.lane_incarnation == lane_incarnation
                    && self
                        .state
                        .da_lane_visible_after_reset(descriptor.proposal_height, lane_id)
                    && self.lane_block_artifact_targets_active_route(
                        lane_id,
                        dataspace_id,
                        lane_incarnation,
                        descriptor.proposal_height,
                    )
            })
        {
            let session = crate::lane_consensus::CommittedLaneBlockSession {
                proposal: artifact.proposal,
                prepare_qc: artifact.prepare_qc,
                commit_qc: artifact.commit_qc,
            };
            let height = session.proposal.descriptor.lane_block_height;
            if self
                .state
                .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(&session)
            {
                finalized_height = finalized_height.max(height);
            } else {
                unresolved_heights.insert(height);
            }
        }
        for relay in self.state.lane_relay_snapshot() {
            let relay_proposal_height = relay.block_header.height().get();
            if relay.lane_id == lane_id
                && relay.dataspace_id == dataspace_id
                && relay.lane_incarnation == lane_incarnation
                && relay.is_merge_admissible()
                && relay.lane_block_descriptor_hash.is_some()
                && self
                    .state
                    .da_lane_visible_after_reset(relay_proposal_height, lane_id)
                && self.lane_block_artifact_targets_active_route(
                    lane_id,
                    dataspace_id,
                    lane_incarnation,
                    relay_proposal_height,
                )
            {
                unresolved_heights.insert(relay.block_height);
            }
        }
        for tip in self
            .subsystems
            .committed_lane_blocks
            .lane_block_tips_snapshot_for_admissible_lanes(
                |tip_lane, tip_dataspace, tip_incarnation, _tip_height, tip_proposal_height| {
                    tip_lane == lane_id
                        && tip_dataspace == dataspace_id
                        && tip_incarnation == lane_incarnation
                        && self
                            .state
                            .da_lane_visible_after_reset(tip_proposal_height, tip_lane)
                        && self.lane_block_artifact_targets_active_route(
                            tip_lane,
                            tip_dataspace,
                            tip_incarnation,
                            tip_proposal_height,
                        )
                },
            )
        {
            unresolved_heights.insert(tip.latest_lane_block_height);
        }

        let admissible_height = unresolved_heights
            .into_iter()
            .filter(|height| *height > finalized_height)
            .min()
            .unwrap_or_else(|| finalized_height.saturating_add(1));
        lane_block_height == admissible_height
    }

    fn disabled_nexus_lane_block_authority_accepts(
        &self,
        proposal_height: u64,
        validator_count: u32,
        min_quorum: u32,
        validator_set_hash: HashOf<Vec<PeerId>>,
        validator_set: Option<&[PeerId]>,
        signer: Option<&PeerId>,
    ) -> bool {
        if self.state.nexus_snapshot().enabled {
            return true;
        }
        let mut expected = self.shared_lane_block_authority_for_ingress(proposal_height);
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
        validator_count == expected_count
            && min_quorum == expected_quorum
            && validator_set_hash == HashOf::new(&expected)
            && signer.is_none_or(|signer| expected.contains(signer))
            && validator_set.is_none_or(|validator_set| validator_set == expected.as_slice())
    }

    pub(super) fn lane_block_proposal_sender_accepts_ingress(
        &self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
        sender: Option<&PeerId>,
    ) -> bool {
        let Some(sender) = sender else {
            return false;
        };
        if crate::lane_consensus::validate_lane_block_proposal(proposal).is_err() {
            return false;
        }
        if !self.lane_block_route_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_incarnation,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
        ) || !self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_incarnation,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
            proposal.proposal_hash,
            proposal.descriptor.validator_count,
            proposal.descriptor.min_quorum,
            proposal.descriptor.validator_set_hash,
            Some(proposal.descriptor.validator_set.as_slice()),
            None,
        ) {
            return false;
        }
        let sender_is_shared_authority = self
            .shared_lane_block_authority_for_ingress(proposal.descriptor.proposal_height)
            .contains(sender);
        let proposal_is_cached = self
            .subsystems
            .lane_blocks
            .contains_proposal_identity(proposal);
        let canonical_payload_available = self
            .state
            .kura()
            .lane_block_payload_availability(proposal)
            .is_available();
        let authenticated_new_view_chain_available =
            self.state.kura().autonomous_lane_payload_available(
                proposal,
                self.chain_hash,
                self.epoch_for_height(proposal.descriptor.proposal_height),
            );
        lane_block_proposal_sender_is_admissible(
            proposal,
            sender,
            sender_is_shared_authority,
            proposal_is_cached,
            canonical_payload_available,
            authenticated_new_view_chain_available,
        )
    }

    fn observe_lane_block_proposal_redrive(
        &mut self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    ) {
        match self.lane_block_redrive.observe(proposal, Instant::now()) {
            super::lane_scheduler::LaneBlockRedriveObservation::Inserted
            | super::lane_scheduler::LaneBlockRedriveObservation::Duplicate => {}
            super::lane_scheduler::LaneBlockRedriveObservation::Superseded { previous_view } => {
                debug!(
                    lane_id = proposal.descriptor.lane_id.as_u32(),
                    dataspace_id = proposal.descriptor.dataspace_id.as_u64(),
                    lane_block_height = proposal.descriptor.lane_block_height,
                    previous_view,
                    lane_block_view = proposal.descriptor.lane_block_view,
                    "advanced canonical lane-block proposal redrive view"
                );
            }
            observation => {
                warn!(
                    ?observation,
                    lane_id = proposal.descriptor.lane_id.as_u32(),
                    dataspace_id = proposal.descriptor.dataspace_id.as_u64(),
                    lane_block_height = proposal.descriptor.lane_block_height,
                    lane_block_view = proposal.descriptor.lane_block_view,
                    "lane-block proposal was cached but rejected by redrive scheduler"
                );
            }
        }
    }

    fn lane_block_authority_accepts_ingress(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        proposal_height: u64,
        lane_block_height: u64,
        proposal_hash: Hash,
        validator_count: u32,
        min_quorum: u32,
        validator_set_hash: HashOf<Vec<PeerId>>,
        validator_set: Option<&[PeerId]>,
        signer: Option<&PeerId>,
    ) -> bool {
        let global_next_height = self.committed_height_snapshot().saturating_add(1);
        if proposal_height > global_next_height {
            return false;
        }
        if proposal_height < global_next_height {
            let Some(canonical) = self.exact_kura_lane_block_proposal(
                lane_id,
                dataspace_id,
                proposal_height,
                lane_block_height,
                proposal_hash,
            ) else {
                return false;
            };
            let descriptor = canonical.descriptor;
            return lane_incarnation == descriptor.lane_incarnation
                && validator_count == descriptor.validator_count
                && min_quorum == descriptor.min_quorum
                && validator_set_hash == descriptor.validator_set_hash
                && signer.is_none_or(|signer| descriptor.validator_set.contains(signer))
                && validator_set.is_none_or(|validator_set| {
                    validator_set == descriptor.validator_set.as_slice()
                });
        }

        let nexus = self.state.nexus_snapshot();
        if !nexus.enabled {
            return self.disabled_nexus_lane_block_authority_accepts(
                proposal_height,
                validator_count,
                min_quorum,
                validator_set_hash,
                validator_set,
                signer,
            );
        }
        if crate::state::nexus_active_lane_dataspace_at_height(lane_id, &nexus, proposal_height)
            != Some(dataspace_id)
        {
            return false;
        }
        if lane_incarnation.as_ref().iter().all(|byte| *byte == 0)
            || self
                .state
                .lane_incarnation_at_height(lane_id, proposal_height)
                != Some(lane_incarnation)
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
        let local_peer = self.common_config.peer.id();
        let local_is_shared_authority = self
            .shared_lane_block_authority_for_ingress(proposal.descriptor.proposal_height)
            .contains(local_peer);
        lane_block_initial_origin_is_admissible(proposal, local_peer, local_is_shared_authority)
            && self.lane_block_route_accepts_ingress(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.lane_incarnation,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
            )
            && self.lane_block_slot_within_ingress_horizon(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
                proposal.proposal_hash,
            )
            && self.lane_block_authority_accepts_ingress(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.lane_incarnation,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
                proposal.proposal_hash,
                proposal.descriptor.validator_count,
                proposal.descriptor.min_quorum,
                proposal.descriptor.validator_set_hash,
                Some(proposal.descriptor.validator_set.as_slice()),
                None,
            )
            && self
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
            body.lane_incarnation,
            body.proposal_height,
            body.lane_block_height,
        ) && self.lane_block_slot_within_ingress_horizon(
            body.lane_id,
            body.dataspace_id,
            body.proposal_height,
            body.lane_block_height,
            body.proposal_hash,
        ) && self.lane_block_authority_accepts_ingress(
            body.lane_id,
            body.dataspace_id,
            body.lane_incarnation,
            body.proposal_height,
            body.lane_block_height,
            body.proposal_hash,
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
            lane_incarnation: ownership.lane_incarnation,
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
        let pending_artifacts = self
            .state
            .unapplied_lane_block_artifact_heights_snapshot_cached();
        let mut cached = 0_usize;
        for ((expected_lane_id, expected_dataspace_id), lane_block_height) in pending_artifacts {
            let Some(artifact) = self
                .state
                .kura()
                .read_lane_block_artifact(expected_lane_id, lane_block_height)
            else {
                continue;
            };
            if self
                .state
                .lane_block_artifact_is_applied_or_snapshot_anchored_cached(&artifact)
            {
                continue;
            }
            let lane_id = artifact.ownership.lane_id;
            let dataspace_id = artifact.ownership.dataspace_id;
            let lane_block_height = artifact.ownership.lane_block_height;
            if lane_id != expected_lane_id
                || dataspace_id != expected_dataspace_id
                || !self.lane_block_artifact_targets_active_route(
                    lane_id,
                    dataspace_id,
                    artifact.ownership.lane_incarnation,
                    artifact.ownership.proposal_height,
                )
            {
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

    /// Rehydrate producer-authenticated autonomous lane sessions after restart.
    ///
    /// Kura has already checked the chain/epoch binding and complete signed view
    /// chain. This final admission step binds the artifact to the active lane
    /// incarnation and committee, skips finalized/applied work, restores the
    /// latest certificate to the bounded redrive cache, and lets the normal
    /// prepare-vote path generate any missing authorized local vote.
    fn cache_unapplied_autonomous_lane_block_proposals(&mut self) -> usize {
        if self.autonomous_lane_blocks_hydrated {
            return 0;
        }
        self.autonomous_lane_blocks_hydrated = true;
        let recovered = self
            .state
            .kura()
            .latest_autonomous_lane_block_artifacts_snapshot(
                self.chain_hash,
                self.recovery_pending_proposal_cap(),
                |proposal_height| self.epoch_for_height(proposal_height),
            );
        let mut cached = 0_usize;
        for (artifact, proposal) in recovered {
            let descriptor = &proposal.descriptor;
            if !self.lane_block_artifact_targets_active_route(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.proposal_height,
            ) || !self.lane_block_authority_accepts_ingress(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.proposal_height,
                descriptor.lane_block_height,
                proposal.proposal_hash,
                descriptor.validator_count,
                descriptor.min_quorum,
                descriptor.validator_set_hash,
                Some(descriptor.validator_set.as_slice()),
                None,
            ) {
                continue;
            }
            let kura = self.state.kura();
            if kura.lane_block_application_receipt_available(&proposal)
                || kura
                    .read_certified_lane_block_artifact(
                        descriptor.lane_id,
                        descriptor.lane_block_height,
                    )
                    .is_some_and(|certified| {
                        let certified = &certified.proposal.descriptor;
                        certified.lane_id == descriptor.lane_id
                            && certified.dataspace_id == descriptor.dataspace_id
                            && certified.lane_incarnation == descriptor.lane_incarnation
                            && certified.lane_block_height == descriptor.lane_block_height
                    })
                || self
                    .subsystems
                    .committed_lane_blocks
                    .contains_proposal(&proposal)
            {
                continue;
            }
            let session_key = lane_block_session_key_for_proposal(&proposal);
            if self.subsystems.lane_blocks.get(&session_key).is_some() {
                continue;
            }
            if !self.lane_executable_payload_passes_stateful_preflight(&artifact.executable_payload)
            {
                warn!(
                    lane_id = descriptor.lane_id.as_u32(),
                    lane_block_height = descriptor.lane_block_height,
                    lane_block_view = descriptor.lane_block_view,
                    "skipping recovered autonomous lane payload that no longer passes preflight"
                );
                continue;
            }

            let latest_certificate = artifact
                .new_view_certificates
                .last()
                .or_else(|| {
                    artifact
                        .view_checkpoint
                        .as_ref()
                        .map(|checkpoint| &checkpoint.certificate)
                })
                .cloned();
            if let Some(durable) = latest_certificate {
                let _ = self
                    .subsystems
                    .lane_new_view_certificates
                    .insert(durable.certificate, &durable.signer_pops);
            }
            if matches!(
                self.cache_recovered_lane_block_artifact_proposal(proposal),
                LaneBlockProposalIngressOutcome::Inserted
            ) {
                cached = cached.saturating_add(1);
            }
        }
        cached
    }

    pub(super) fn shared_lane_block_authority_for_ingress(
        &self,
        target_height: u64,
    ) -> Vec<PeerId> {
        let (consensus_mode, _mode_tag, _prf_seed) =
            self.consensus_context_for_height(target_height);
        let mut validators = self.roster_for_live_vote_with_mode(target_height, consensus_mode);
        if validators.is_empty() {
            validators = self.effective_commit_topology();
        }
        validators
    }

    fn lane_payload_global_proposer(&self, hint: LaneBlockProposalPayloadHintV1) -> Option<PeerId> {
        let roster = self.shared_lane_block_authority_for_ingress(hint.proposal_height);
        let mut topology = crate::sumeragi::network_topology::Topology::new(roster);
        let leader_index = self
            .leader_index_for(&mut topology, hint.proposal_height, hint.proposal_view)
            .ok()?;
        topology.as_ref().get(leader_index).cloned()
    }

    fn lane_payload_global_anchor_observed(&self, hint: LaneBlockProposalPayloadHintV1) -> bool {
        let committed = self
            .state
            .kura()
            .get_block_height_by_hash(hint.proposal_block_hash)
            .is_some_and(|height| u64::try_from(height.get()).ok() == Some(hint.proposal_height));
        if committed {
            return true;
        }
        let pending = self
            .pending
            .pending_blocks
            .get(&hint.proposal_block_hash)
            .is_some_and(|pending| {
                pending.height == hint.proposal_height
                    && pending.view == hint.proposal_view
                    && pending.validation_status == ValidationStatus::Valid
            });
        let cached_hint_matches = self
            .subsystems
            .propose
            .proposal_cache
            .get_hint(hint.proposal_height, hint.proposal_view)
            .is_some_and(|observed| observed.block_hash == hint.proposal_block_hash);
        pending && cached_hint_matches
    }

    fn lane_payload_global_anchor_matches(
        &self,
        hint: LaneBlockProposalPayloadHintV1,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
        entrypoints: &[iroha_data_model::transaction::TransactionEntrypoint],
    ) -> bool {
        let matches = |block: &SignedBlock| {
            let block_entrypoints = block.external_entrypoints_cloned().collect::<Vec<_>>();
            let selected = proposal
                .descriptor
                .accepted_candidate_indices
                .iter()
                .copied()
                .map(|raw_index| {
                    usize::try_from(raw_index)
                        .ok()
                        .and_then(|index| block_entrypoints.get(index).cloned())
                })
                .collect::<Option<Vec<_>>>();
            if selected.as_deref() != Some(entrypoints) {
                return false;
            }
            block.execution_context().is_some_and(|context| {
                context.lane_payload_ownerships.iter().any(|ownership| {
                    Self::lane_block_proposal_from_payload_ownership(
                        ownership,
                        Some(hint.proposal_block_hash),
                    )
                    .is_some_and(|anchored| anchored.same_consensus_identity(proposal))
                })
            })
        };
        if let Some(pending) = self.pending.pending_blocks.get(&hint.proposal_block_hash) {
            return pending.height == hint.proposal_height
                && pending.view == hint.proposal_view
                && pending.validation_status == ValidationStatus::Valid
                && matches(&pending.block);
        }
        let Some(height) =
            NonZeroUsize::new(usize::try_from(hint.proposal_height).unwrap_or(usize::MAX))
        else {
            return false;
        };
        self.state.kura().get_block(height).is_some_and(|block| {
            block.hash() == hint.proposal_block_hash && matches(block.as_ref())
        })
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

    fn record_lane_block_horizon_drop(
        &mut self,
        kind: super::status::ConsensusMessageKind,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        proposal_height: u64,
        lane_block_height: u64,
    ) {
        self.record_consensus_message_handling(
            kind,
            super::status::ConsensusMessageOutcome::Dropped,
            super::status::ConsensusMessageReason::InvalidPayload,
        );
        warn!(
            lane_id = lane_id.as_u32(),
            dataspace_id = dataspace_id.as_u64(),
            proposal_height,
            lane_block_height,
            committed_height = self.committed_height_snapshot(),
            "dropping lane-block evidence outside the durable-tip ingress horizon"
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
        if let Some((kind, key)) = lane_block_rebroadcast_identity(&msg) {
            let cooldown = self.lane_block_rebroadcast_cooldown();
            self.lane_block_rebroadcast_log
                .record(kind, key, Instant::now(), cooldown);
        }
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
        if self.consensus_participation_halted_now() {
            return None;
        }
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
            proposal.descriptor.lane_incarnation,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
        ) || !self.lane_block_slot_within_ingress_horizon(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
            proposal.proposal_hash,
        ) || !self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_incarnation,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
            proposal.proposal_hash,
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
        if !self.lane_block_payload_available_for_vote(proposal, "prepare") {
            return None;
        }
        let body = proposal.vote_body(crate::sumeragi::consensus::Phase::Prepare);
        let epoch = self.epoch_for_height(proposal.descriptor.proposal_height);
        let payload_availability_vote = if let Some(artifact) =
            self.state.kura().read_autonomous_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
                self.chain_hash,
                epoch,
            ) {
            let availability_body = match crate::lane_consensus::lane_payload_availability_body(
                &artifact.executable_payload,
                proposal,
                self.chain_hash,
                epoch,
            ) {
                Ok(body) => body,
                Err(err) => {
                    warn!(
                        ?err,
                        "skipping READY vote for mismatched autonomous payload"
                    );
                    return None;
                }
            };
            let Some(validator_set_pops) =
                self.lane_block_validator_set_pops(&proposal.descriptor.validator_set)
            else {
                warn!("skipping READY vote because historical committee PoPs are incomplete");
                return None;
            };
            match crate::lane_consensus::LanePayloadAvailabilityVoteV1::new_signed(
                availability_body,
                local_peer.clone(),
                validator_set_pops,
                self.common_config.key_pair.private_key(),
            ) {
                Ok(vote) => Some(vote),
                Err(err) => {
                    warn!(
                        ?err,
                        "skipping READY vote after availability signing failure"
                    );
                    return None;
                }
            }
        } else {
            None
        };
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
            payload_availability_vote,
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

    fn broadcast_missing_local_lane_block_prepare_votes(&mut self) -> bool {
        let local_peer = self.common_config.peer.id().clone();
        let proposals = self
            .subsystems
            .lane_blocks
            .local_prepare_vote_proposals_for(&local_peer);
        let mut progress = false;
        for proposal in proposals {
            progress |= self.broadcast_local_prepare_vote_for_proposal(&proposal);
        }
        progress
    }

    fn release_unpersisted_lane_reservations(
        &self,
        reservations: &[crate::queue::LaneReservedTransaction],
        reason: &'static str,
    ) {
        for reservation in reservations {
            if let Err(err) = self.queue.release_lane_reservation(reservation.key()) {
                warn!(
                    ?err,
                    reason,
                    lane_id = reservation.key().lane_id.as_u32(),
                    lane_block_height = reservation.key().lane_block_height,
                    "failed to release unpersisted lane queue reservation"
                );
            }
        }
    }

    /// Independently select, authenticate, persist, and fan out one payload per
    /// active lane for which this peer is the deterministic slot producer.
    fn produce_ready_autonomous_lane_payloads(&mut self) -> usize {
        let nexus = self.state.nexus_snapshot();
        if !nexus.enabled || !self.queue.lane_reservation_journal_installed() {
            return 0;
        }
        let proposal_height = u64::try_from(self.state.committed_height())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        if proposal_height == 0 {
            return 0;
        }
        let blocked_lanes =
            self.unapplied_lane_block_lanes_for_proposal(self.state.as_ref(), proposal_height);
        let known_tips = self.known_lane_block_tips_for_proposal(proposal_height);
        let (_, lane_mode_tag, _) = self.consensus_context_for_height(proposal_height);
        let local_peer = self.common_config.peer.id().clone();
        if local_peer.public_key() != self.common_config.key_pair.public_key()
            || !matches!(
                self.common_config.key_pair.public_key().try_algorithm(),
                Ok(iroha_crypto::Algorithm::BlsNormal)
            )
        {
            return 0;
        }
        let reservation_cap = self
            .frontier_proposal_grace_tx_count()
            .min(crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS);
        let Some(reservation_cap) = NonZeroUsize::new(reservation_cap) else {
            return 0;
        };
        let mut lanes = nexus
            .lane_catalog
            .lanes()
            .iter()
            .map(|lane| lane.id)
            .collect::<Vec<_>>();
        lanes.sort();
        lanes.dedup();

        let mut produced = 0_usize;
        for lane_id in lanes {
            let Some(dataspace_id) = crate::state::nexus_active_lane_dataspace_at_height(
                lane_id,
                &nexus,
                proposal_height,
            ) else {
                continue;
            };
            if blocked_lanes.contains(&lane_id) {
                continue;
            }
            let Some(lane_incarnation) = self
                .state
                .lane_incarnation_at_height(lane_id, proposal_height)
                .filter(|incarnation| !incarnation.as_ref().iter().all(|byte| *byte == 0))
            else {
                continue;
            };
            let mut validator_set = self
                .state
                .authoritative_lane_peer_ids_at_height(lane_id, proposal_height);
            validator_set.sort();
            validator_set.dedup();
            let Ok(validator_count) = u32::try_from(validator_set.len()) else {
                continue;
            };
            let Ok(min_quorum) = u32::try_from(
                crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len())
                    .max(1),
            ) else {
                continue;
            };
            let Ok(quorum) =
                iroha_data_model::nexus::LaneRelayQuorumContext::new(validator_count, min_quorum)
            else {
                continue;
            };
            let validator_set_hash = HashOf::new(&validator_set);
            let tip = known_tips
                .iter()
                .filter(|tip| {
                    tip.lane_id == lane_id
                        && tip.dataspace_id == dataspace_id
                        && tip.lane_incarnation == lane_incarnation
                })
                .max_by_key(|tip| tip.latest_lane_block_height)
                .copied()
                .unwrap_or(super::lane_scheduler::LaneBlockTip {
                    lane_id,
                    dataspace_id,
                    lane_incarnation,
                    latest_lane_block_height: 0,
                    latest_lane_block_descriptor_hash: None,
                });
            let Some(lane_block_height) = tip.latest_lane_block_height.checked_add(1) else {
                continue;
            };
            let lane_block_view = 0;
            let Some(leader) = super::lane_scheduler::lane_block_slot_leader(
                lane_id,
                dataspace_id,
                lane_incarnation,
                proposal_height,
                tip.latest_lane_block_height,
                lane_block_height,
                lane_block_view,
                validator_set_hash,
                &validator_set,
                0,
            ) else {
                continue;
            };
            if leader != &local_peer {
                continue;
            }
            let (reservation_owner_hash, proposal_identity_hash) =
                super::lane_scheduler::lane_block_reservation_identities(
                    lane_id,
                    dataspace_id,
                    lane_incarnation,
                    proposal_height,
                    lane_block_height,
                    lane_block_view,
                    validator_set_hash,
                    leader,
                );
            let scope = crate::queue::LaneQueueReservationScopeV1 {
                lane_id,
                dataspace_id,
                lane_incarnation,
                proposal_height,
                lane_block_height,
                lane_block_view,
                reservation_owner_hash,
                proposal_identity_hash,
            };
            let reservations = match self.queue.reserve_transactions_for_lane(
                self.state.as_ref(),
                scope,
                reservation_cap,
            ) {
                Ok(reservations) if !reservations.is_empty() => reservations,
                Ok(_) => continue,
                Err(err) => {
                    warn!(
                        ?err,
                        lane_id = lane_id.as_u32(),
                        proposal_height,
                        lane_block_height,
                        "failed to reserve autonomous lane payload work"
                    );
                    continue;
                }
            };
            let candidate_hashes = reservations
                .iter()
                .map(|reservation| Hash::from(reservation.key().entrypoint_hash))
                .collect::<Vec<_>>();
            let domain = super::lane_scheduler::LaneConsensusDomain {
                lane_id,
                dataspace_id,
                accepted_candidates: reservations.len(),
                accepted_candidate_indices: (0..reservations.len()).collect(),
                validator_set: validator_set.clone(),
                quorum,
                qc_mode_tag: iroha_data_model::nexus::LaneRelayEnvelope::lane_qc_mode_tag_for(
                    lane_id,
                    dataspace_id,
                    lane_mode_tag,
                ),
            };
            let lane_incarnations = BTreeMap::from([(lane_id, lane_incarnation)]);
            let payload_plan = super::lane_scheduler::plan_lane_payload_with_incarnations(
                &[domain],
                &known_tips,
                &candidate_hashes,
                proposal_height.saturating_sub(1),
                &BTreeMap::new(),
                &lane_incarnations,
                proposal_height,
                lane_block_view,
            );
            let Ok(mut entries) = payload_plan.map(|plan| plan.entries) else {
                self.release_unpersisted_lane_reservations(
                    &reservations,
                    "lane_payload_plan_failed",
                );
                continue;
            };
            let Some(entry) = entries.pop() else {
                self.release_unpersisted_lane_reservations(
                    &reservations,
                    "lane_payload_plan_empty",
                );
                continue;
            };
            let proposal = entry.lane_block_proposal.artifact;
            let entrypoints = reservations
                .iter()
                .map(|reservation| reservation.clone_accepted().into_entrypoint())
                .collect::<Vec<_>>();
            let reservation_keys = reservations
                .iter()
                .map(|reservation| *reservation.key())
                .collect::<Vec<_>>();
            let routing_plans = reservations
                .iter()
                .map(|reservation| reservation.routing_plan().clone())
                .collect::<Vec<_>>();
            let accepted_transactions = reservations
                .iter()
                .map(crate::queue::LaneReservedTransaction::clone_accepted)
                .collect::<Vec<_>>();
            let native_amx_receipts = match self.native_amx_receipts_for_batch(
                &accepted_transactions,
                &routing_plans,
                proposal_height,
            ) {
                Ok(receipts) => receipts,
                Err(reason) => {
                    debug!(
                        lane_id = lane_id.as_u32(),
                        lane_block_height,
                        reason,
                        "deferring autonomous lane payload while native AMX attestations converge"
                    );
                    self.release_unpersisted_lane_reservations(
                        &reservations,
                        "native_amx_attestations_pending",
                    );
                    continue;
                }
            };
            let payload =
                match crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
                    self.chain_hash,
                    self.epoch_for_height(proposal_height),
                    proposal.clone(),
                    entrypoints,
                    reservation_keys,
                    routing_plans,
                    native_amx_receipts,
                    local_peer.clone(),
                    self.common_config.key_pair.private_key(),
                ) {
                    Ok(payload) => payload,
                    Err(err) => {
                        warn!(
                            ?err,
                            lane_id = lane_id.as_u32(),
                            "failed to sign autonomous lane payload"
                        );
                        self.release_unpersisted_lane_reservations(
                            &reservations,
                            "lane_payload_signing_failed",
                        );
                        continue;
                    }
                };
            if !self.lane_executable_payload_passes_stateful_preflight(&payload) {
                self.release_unpersisted_lane_reservations(
                    &reservations,
                    "lane_payload_preflight_failed",
                );
                continue;
            }
            if let Err(err) = self.state.kura().persist_lane_executable_payload(
                &payload,
                self.chain_hash,
                payload.epoch,
            ) {
                let durable = reservations.iter().all(|reservation| {
                    self.state
                        .kura()
                        .autonomous_lane_payload_matches_reservation(
                            reservation.key(),
                            self.chain_hash,
                            payload.epoch,
                        )
                });
                if !durable {
                    warn!(
                        ?err,
                        lane_id = lane_id.as_u32(),
                        "failed to persist autonomous lane payload"
                    );
                    self.release_unpersisted_lane_reservations(
                        &reservations,
                        "lane_payload_persistence_failed",
                    );
                    continue;
                }
            }

            let outcome = self.cache_lane_block_proposal(proposal.clone());
            if matches!(
                outcome,
                LaneBlockProposalIngressOutcome::Inserted
                    | LaneBlockProposalIngressOutcome::Duplicate
            ) {
                self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneExecutablePayload(payload),
                    validator_set.as_slice(),
                );
                self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    validator_set.as_slice(),
                );
                self.broadcast_local_prepare_vote_for_incoming_lane_block_proposal(&proposal);
                produced = produced.saturating_add(1);
            }
        }
        produced
    }

    pub(super) fn broadcast_ready_local_lane_block_votes(&mut self) -> bool {
        let pruned_finalized = self.prune_durably_finalized_lane_block_state();
        let recovered_autonomous_proposals = self.cache_unapplied_autonomous_lane_block_proposals();
        let produced_autonomous_payloads = self.produce_ready_autonomous_lane_payloads();
        let resumed_payload_handoffs = self.process_deferred_lane_payload_handoffs();
        let new_view_votes = self.broadcast_due_lane_block_new_view_votes();
        let recovered_proposals = self.cache_unapplied_lane_block_artifact_proposals();
        let pruned_noncanonical =
            self.prune_lane_block_sessions_conflicting_with_canonical_payloads();
        let canonical_sessions = self.canonical_lane_block_session_keys();
        let pruned_excess = self
            .subsystems
            .lane_blocks
            .prune_excess_speculative_siblings(
                LANE_BLOCK_SPECULATIVE_SIBLINGS_PER_GROUP,
                &canonical_sessions,
            );
        let (rebroadcasted_proposals, rebroadcasted_proposal_keys) =
            self.rebroadcast_cached_lane_block_proposals_without_commit_qc();
        let progress = self.broadcast_missing_local_lane_block_prepare_votes();
        if progress {
            self.broadcast_newly_sealed_lane_block_qcs();
        }
        let commit_votes = self.broadcast_lane_block_commit_votes_for_prepared_sessions();
        let rebroadcasted_votes =
            self.rebroadcast_cached_local_lane_block_votes_without_qc(&rebroadcasted_proposal_keys);
        let rebroadcasted_qcs = self.rebroadcast_cached_lane_block_qcs_for_incomplete_sessions();
        self.publish_lane_block_session_status();
        commit_votes > 0
            || progress
            || rebroadcasted_qcs > 0
            || rebroadcasted_votes > 0
            || rebroadcasted_proposals > 0
            || recovered_proposals > 0
            || recovered_autonomous_proposals > 0
            || produced_autonomous_payloads > 0
            || resumed_payload_handoffs > 0
            || new_view_votes > 0
            || pruned_noncanonical > 0
            || pruned_excess > 0
            || pruned_finalized > 0
    }

    pub(super) fn canonical_lane_block_session_keys(
        &self,
    ) -> BTreeSet<crate::lane_consensus::LaneBlockSessionKey> {
        self.subsystems
            .lane_blocks
            .cached_proposals()
            .into_iter()
            .filter(|proposal| {
                self.state
                    .kura()
                    .lane_block_payload_availability(proposal)
                    .is_available()
            })
            .map(|proposal| crate::lane_consensus::LaneBlockSessionKey {
                lane_id: proposal.descriptor.lane_id,
                dataspace_id: proposal.descriptor.dataspace_id,
                lane_incarnation: proposal.descriptor.lane_incarnation,
                lane_block_height: proposal.descriptor.lane_block_height,
                lane_block_view: proposal.descriptor.lane_block_view,
                proposal_hash: proposal.proposal_hash,
            })
            .collect()
    }

    pub(super) fn prune_lane_block_sessions_conflicting_with_canonical_payloads(
        &mut self,
    ) -> usize {
        let canonical_proposals = self
            .subsystems
            .lane_blocks
            .cached_proposals()
            .into_iter()
            .filter(|proposal| {
                self.state
                    .kura()
                    .lane_block_payload_availability(proposal)
                    .is_available()
            })
            .collect::<Vec<_>>();
        let mut pruned = 0_usize;
        for proposal in canonical_proposals {
            pruned = pruned.saturating_add(
                self.subsystems
                    .lane_blocks
                    .prune_uncommitted_sessions_conflicting_with_canonical_proposal(&proposal),
            );
        }
        if pruned > 0 {
            debug!(
                pruned,
                "pruned noncanonical prepared lane-block sessions after canonical payload discovery"
            );
        }
        pruned
    }

    fn legacy_lane_block_rebroadcast_cooldown(&self) -> Duration {
        self.payload_rebroadcast_cooldown()
            .max(CACHED_PROPOSAL_REBROADCAST_COOLDOWN_FLOOR)
    }

    fn lane_block_proposal_is_admissible_for_rebroadcast(
        &self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    ) -> bool {
        self.lane_block_route_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_incarnation,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
        ) && self.lane_block_slot_within_ingress_horizon(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
            proposal.proposal_hash,
        ) && self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_incarnation,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
            proposal.proposal_hash,
            proposal.descriptor.validator_count,
            proposal.descriptor.min_quorum,
            proposal.descriptor.validator_set_hash,
            Some(proposal.descriptor.validator_set.as_slice()),
            None,
        )
    }

    pub(super) fn lane_block_rebroadcast_next_due(&self, now: Instant) -> Option<Instant> {
        let local_peer = self.common_config.peer.id().clone();
        if !self
            .subsystems
            .lane_blocks
            .has_periodic_rebroadcast_work(&local_peer, |proposal| {
                self.lane_block_proposal_is_admissible_for_rebroadcast(proposal)
            })
        {
            return None;
        }
        Some(self.last_lane_block_rebroadcast.map_or(now, |last| {
            last.checked_add(self.legacy_lane_block_rebroadcast_cooldown())
                .unwrap_or(now)
                .max(now)
        }))
    }

    pub(super) fn rebroadcast_cached_lane_block_bundles_if_due(&mut self, now: Instant) -> usize {
        let _ = self.prune_durably_finalized_lane_block_state();
        if self.last_lane_block_rebroadcast.is_some_and(|last| {
            now.saturating_duration_since(last) < self.legacy_lane_block_rebroadcast_cooldown()
        }) {
            return 0;
        }

        let local_peer = self.common_config.peer.id().clone();
        let bundles = self
            .subsystems
            .lane_blocks
            .periodic_rebroadcast_bundles_after(
                &local_peer,
                self.lane_block_rebroadcast_cursor,
                LANE_BLOCK_REBROADCAST_BUNDLES_PER_TICK,
                |proposal| self.lane_block_proposal_is_admissible_for_rebroadcast(proposal),
            );
        let Some(last_bundle) = bundles.last() else {
            return 0;
        };
        self.lane_block_rebroadcast_cursor = Some(last_bundle.key);
        self.last_lane_block_rebroadcast = Some(now);

        let mut scheduled = 0_usize;
        for bundle in bundles {
            let proposal = bundle.proposal;
            if !self.lane_block_route_accepts_ingress(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.lane_incarnation,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
            ) || !self.lane_block_slot_within_ingress_horizon(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
                proposal.proposal_hash,
            ) || !self.lane_block_authority_accepts_ingress(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.lane_incarnation,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
                proposal.proposal_hash,
                proposal.descriptor.validator_count,
                proposal.descriptor.min_quorum,
                proposal.descriptor.validator_set_hash,
                Some(proposal.descriptor.validator_set.as_slice()),
                None,
            ) {
                continue;
            }

            if bundle.rebroadcast_proposal {
                scheduled =
                    scheduled.saturating_add(self.schedule_lane_block_message_to_validator_set(
                        BlockMessage::LaneBlockProposal(proposal.clone()),
                        proposal.descriptor.validator_set.as_slice(),
                    ));
            }
            for vote in bundle.local_votes {
                if !self
                    .lane_block_vote_body_targets_authorized_local_signer(&vote.body, &local_peer)
                {
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
            for qc in bundle.qcs {
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
        }
        scheduled
    }

    fn broadcast_due_lane_block_new_view_votes(&mut self) -> usize {
        let local_peer = self.common_config.peer.id().clone();
        if local_peer.public_key() != self.common_config.key_pair.public_key()
            || !matches!(
                self.common_config.key_pair.public_key().try_algorithm(),
                Ok(iroha_crypto::Algorithm::BlsNormal)
            )
        {
            return 0;
        }

        let now = Instant::now();
        let timeout = self.lane_block_rebroadcast_cooldown();
        let proposals = self.subsystems.lane_blocks.proposals_without_commit_qc();
        for proposal in proposals {
            if !proposal.descriptor.validator_set.contains(&local_peer)
                || self
                    .lane_block_redrive
                    .redrive_round(&proposal, now, timeout)
                    .is_none_or(|round| round == 0)
                || !self.lane_block_route_accepts_ingress(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.dataspace_id,
                    proposal.descriptor.lane_incarnation,
                    proposal.descriptor.proposal_height,
                    proposal.descriptor.lane_block_height,
                )
            {
                continue;
            }
            let epoch = self.epoch_for_height(proposal.descriptor.proposal_height);
            let Some((payload, current)) = self.state.kura().current_autonomous_lane_payload(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
                self.chain_hash,
                epoch,
            ) else {
                continue;
            };
            if !current.same_consensus_identity(&proposal) {
                continue;
            }
            let Some(target_view) = proposal.descriptor.lane_block_view.checked_add(1) else {
                continue;
            };
            let Ok(body) = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
                &proposal,
                &payload,
                target_view,
                self.chain_hash,
                epoch,
            ) else {
                continue;
            };
            let Ok(vote) = crate::lane_consensus::LaneBlockNewViewVoteV1::new_signed(
                body,
                local_peer.clone(),
                self.common_config.key_pair.private_key(),
            ) else {
                continue;
            };
            if self.subsystems.lane_new_view_votes.contains(&vote) {
                continue;
            }
            if let Err(err) = self.handle_lane_block_new_view_vote(vote, Some(&local_peer)) {
                warn!(?err, "failed to cache local lane NewView vote");
            }
        }

        let mut scheduled = 0_usize;
        for vote in self
            .subsystems
            .lane_new_view_votes
            .votes_for_signer(&local_peer)
        {
            let key = lane_block_new_view_vote_rebroadcast_identity(&vote);
            if !self.lane_block_rebroadcast_due(LaneBlockRebroadcastKind::NewViewVote, key) {
                continue;
            }
            let Some(validator_set) = self.subsystems.lane_blocks.proposal_validator_set(&key)
            else {
                continue;
            };
            scheduled =
                scheduled.saturating_add(self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockNewViewVote(vote.clone()),
                    validator_set.as_slice(),
                ));
        }
        scheduled
    }

    fn rebroadcast_cached_lane_block_proposals_without_commit_qc(
        &mut self,
    ) -> (
        usize,
        std::collections::BTreeSet<crate::lane_consensus::LaneBlockSessionKey>,
    ) {
        let mut scheduled = 0_usize;
        let mut rebroadcasted = std::collections::BTreeSet::new();
        let local_peer = self.common_config.peer.id().clone();
        for proposal in self.subsystems.lane_blocks.proposals_without_commit_qc() {
            if !self.lane_block_payload_available_for_vote(&proposal, "proposal_rebroadcast") {
                continue;
            }
            if !self.lane_block_route_accepts_ingress(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.lane_incarnation,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
            ) || !self.lane_block_authority_accepts_ingress(
                proposal.descriptor.lane_id,
                proposal.descriptor.dataspace_id,
                proposal.descriptor.lane_incarnation,
                proposal.descriptor.proposal_height,
                proposal.descriptor.lane_block_height,
                proposal.proposal_hash,
                proposal.descriptor.validator_count,
                proposal.descriptor.min_quorum,
                proposal.descriptor.validator_set_hash,
                Some(proposal.descriptor.validator_set.as_slice()),
                None,
            ) {
                continue;
            }
            if !self.lane_block_redrive.peer_may_redrive(
                &proposal,
                &local_peer,
                Instant::now(),
                self.lane_block_rebroadcast_cooldown(),
            ) {
                continue;
            }
            let proposal_key = lane_block_session_key_for_proposal(&proposal);
            if !self.lane_block_rebroadcast_due(LaneBlockRebroadcastKind::Proposal, proposal_key) {
                continue;
            }
            let epoch = self.epoch_for_height(proposal.descriptor.proposal_height);
            if let Some(artifact) = self.state.kura().read_autonomous_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
                self.chain_hash,
                epoch,
            ) && artifact
                .executable_payload
                .matches_proposal_static(&proposal)
            {
                // The proposal is only a digest. Re-fanout the immutable,
                // producer-signed bytes on every selected redrive round so a
                // validator that lost local storage can rejoin READY/DELIVER
                // without trusting the redriver.
                scheduled =
                    scheduled.saturating_add(self.schedule_lane_block_message_to_validator_set(
                        BlockMessage::LaneExecutablePayload(artifact.executable_payload),
                        proposal.descriptor.validator_set.as_slice(),
                    ));
            }
            scheduled =
                scheduled.saturating_add(self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    proposal.descriptor.validator_set.as_slice(),
                ));
            rebroadcasted.insert(proposal_key);
        }
        (scheduled, rebroadcasted)
    }

    fn rebroadcast_cached_local_lane_block_votes_without_qc(
        &mut self,
        already_rebroadcasted_proposals: &std::collections::BTreeSet<
            crate::lane_consensus::LaneBlockSessionKey,
        >,
    ) -> usize {
        let local_peer = self.common_config.peer.id().clone();
        let mut scheduled = 0_usize;
        for (proposal, vote) in self
            .subsystems
            .lane_blocks
            .local_vote_rebroadcast_artifacts_for(&local_peer)
        {
            let proposal_key = lane_block_session_key_for_proposal(&proposal);
            if !already_rebroadcasted_proposals.contains(&proposal_key)
                && self.lane_block_payload_available_for_vote(&proposal, "proposal_rebroadcast")
                && self.lane_block_route_accepts_ingress(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.dataspace_id,
                    proposal.descriptor.lane_incarnation,
                    proposal.descriptor.proposal_height,
                    proposal.descriptor.lane_block_height,
                )
                && self.lane_block_authority_accepts_ingress(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.dataspace_id,
                    proposal.descriptor.lane_incarnation,
                    proposal.descriptor.proposal_height,
                    proposal.descriptor.lane_block_height,
                    proposal.proposal_hash,
                    proposal.descriptor.validator_count,
                    proposal.descriptor.min_quorum,
                    proposal.descriptor.validator_set_hash,
                    Some(proposal.descriptor.validator_set.as_slice()),
                    None,
                )
                && self.lane_block_redrive.peer_may_redrive(
                    &proposal,
                    &local_peer,
                    Instant::now(),
                    self.lane_block_rebroadcast_cooldown(),
                )
                && self.lane_block_rebroadcast_due(LaneBlockRebroadcastKind::Proposal, proposal_key)
            {
                scheduled =
                    scheduled.saturating_add(self.schedule_lane_block_message_to_validator_set(
                        BlockMessage::LaneBlockProposal(proposal.clone()),
                        proposal.descriptor.validator_set.as_slice(),
                    ));
            }
            if !self.lane_block_vote_body_targets_authorized_local_signer(&vote.body, &local_peer) {
                continue;
            }
            let Some((kind, vote_key)) = lane_block_vote_rebroadcast_identity(&vote) else {
                continue;
            };
            if !self.lane_block_rebroadcast_due(kind, vote_key) {
                continue;
            }
            scheduled =
                scheduled.saturating_add(self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockVote(vote.clone()),
                    proposal.descriptor.validator_set.as_slice(),
                ));
        }
        scheduled
    }

    fn rebroadcast_cached_lane_block_qcs_for_incomplete_sessions(&mut self) -> usize {
        let mut scheduled = 0_usize;
        for qc in self.subsystems.lane_blocks.qcs_for_incomplete_sessions() {
            if !self.lane_block_route_accepts_ingress(
                qc.body.lane_id,
                qc.body.dataspace_id,
                qc.body.lane_incarnation,
                qc.body.proposal_height,
                qc.body.lane_block_height,
            ) || !self.lane_block_authority_accepts_ingress(
                qc.body.lane_id,
                qc.body.dataspace_id,
                qc.body.lane_incarnation,
                qc.body.proposal_height,
                qc.body.lane_block_height,
                qc.body.proposal_hash,
                qc.body.validator_count,
                qc.body.min_quorum,
                qc.body.validator_set_hash,
                Some(qc.validator_set.as_slice()),
                None,
            ) {
                continue;
            }
            let Some((kind, qc_key)) = lane_block_qc_rebroadcast_identity(&qc) else {
                continue;
            };
            if !self.lane_block_rebroadcast_due(kind, qc_key) {
                continue;
            }
            scheduled =
                scheduled.saturating_add(self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockQc(qc.clone()),
                    qc.validator_set.as_slice(),
                ));
        }
        scheduled
    }

    fn lane_block_rebroadcast_due(
        &mut self,
        kind: LaneBlockRebroadcastKind,
        key: crate::lane_consensus::LaneBlockSessionKey,
    ) -> bool {
        let cooldown = self.lane_block_rebroadcast_cooldown();
        self.lane_block_rebroadcast_log
            .allow(kind, key, Instant::now(), cooldown)
    }

    fn lane_block_rebroadcast_cooldown(&self) -> Duration {
        self.control_plane_rebroadcast_cooldown()
            .max(LANE_BLOCK_REBROADCAST_COOLDOWN_FLOOR)
    }

    pub(super) fn lane_block_validator_set_for_vote(
        &self,
        vote: &crate::lane_consensus::LaneBlockVoteV1,
    ) -> Option<Vec<PeerId>> {
        let key = crate::lane_consensus::LaneBlockSessionKey {
            lane_id: vote.body.lane_id,
            dataspace_id: vote.body.dataspace_id,
            lane_incarnation: vote.body.lane_incarnation,
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
        if self.consensus_participation_halted_now() {
            return None;
        }
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
            proposal.descriptor.lane_incarnation,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
        ) || !self.lane_block_authority_accepts_ingress(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
            proposal.descriptor.lane_incarnation,
            proposal.descriptor.proposal_height,
            proposal.descriptor.lane_block_height,
            proposal.proposal_hash,
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
        if !self.lane_block_payload_available_for_vote(proposal, "commit") {
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
            payload_availability_vote: None,
            signer: local_peer.clone(),
            bls_signature: signature.payload().to_vec(),
        })
    }

    pub(super) fn lane_block_payload_available_for_vote(
        &self,
        proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
        phase: &'static str,
    ) -> bool {
        let availability = self.state.kura().lane_block_payload_availability(proposal);
        let autonomous_available = self.state.kura().autonomous_lane_payload_available(
            proposal,
            self.chain_hash,
            self.epoch_for_height(proposal.descriptor.proposal_height),
        );
        if !availability.is_available() && !autonomous_available {
            let descriptor = &proposal.descriptor;
            debug!(
                lane_id = ?descriptor.lane_id,
                dataspace_id = ?descriptor.dataspace_id,
                lane_block_height = descriptor.lane_block_height,
                lane_block_view = descriptor.lane_block_view,
                proposal_height = descriptor.proposal_height,
                phase,
                ?availability,
                "deferring local lane-block vote until canonical payload evidence is available"
            );
            return false;
        }
        if phase == "commit"
            && autonomous_available
            && !self
                .state
                .kura()
                .autonomous_lane_payload_availability_delivered(
                    proposal,
                    self.chain_hash,
                    self.epoch_for_height(proposal.descriptor.proposal_height),
                )
        {
            let descriptor = &proposal.descriptor;
            debug!(
                lane_id = ?descriptor.lane_id,
                dataspace_id = ?descriptor.dataspace_id,
                lane_block_height = descriptor.lane_block_height,
                lane_block_view = descriptor.lane_block_view,
                proposal_height = descriptor.proposal_height,
                "deferring local lane-block commit vote until READY quorum durably DELIVERs the payload"
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
            if !self.persist_autonomous_lane_payload_availability_deliver(&request.prepare_qc) {
                continue;
            }
            let Some(vote) = self.local_lane_block_commit_vote(&request.proposal) else {
                continue;
            };
            if self.lane_block_vote_accepts_local_broadcast(&vote, &local_peer) {
                let scheduled = self.schedule_lane_block_message_to_validator_set(
                    BlockMessage::LaneBlockVote(vote.clone()),
                    request.proposal.descriptor.validator_set.as_slice(),
                );
                Self::log_lane_block_commit_vote_post(scheduled, &vote, &request.prepare_qc);
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
                qc.body.lane_incarnation,
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
            if !self.lane_block_slot_within_ingress_horizon(
                qc.body.lane_id,
                qc.body.dataspace_id,
                qc.body.proposal_height,
                qc.body.lane_block_height,
                qc.body.proposal_hash,
            ) {
                self.record_lane_block_horizon_drop(
                    super::status::ConsensusMessageKind::LaneBlockQc,
                    qc.body.lane_id,
                    qc.body.dataspace_id,
                    qc.body.proposal_height,
                    qc.body.lane_block_height,
                );
                continue;
            }
            if !self.lane_block_authority_accepts_ingress(
                qc.body.lane_id,
                qc.body.dataspace_id,
                qc.body.lane_incarnation,
                qc.body.proposal_height,
                qc.body.lane_block_height,
                qc.body.proposal_hash,
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
            if !self.persist_autonomous_lane_payload_availability_deliver(&qc) {
                // Never advertise a locally sealed prepare QC until its exact
                // payload-retention certificate is durable. The QC remains in
                // the session cache and the commit-vote path retries repair.
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
        }
    }

    fn prune_lane_block_sessions_for_inactive_lanes(&mut self) -> usize {
        let nexus = self.state.nexus_snapshot();
        let admissible_lane = |lane_id: LaneId,
                               dataspace_id: DataSpaceId,
                               lane_incarnation: Hash,
                               _lane_block_height: u64,
                               proposal_height: u64| {
            self.state
                .da_lane_visible_after_reset(proposal_height, lane_id)
                && crate::state::consensus_lane_dataspace_at_height(
                    lane_id,
                    &nexus,
                    proposal_height,
                ) == Some(dataspace_id)
                && self
                    .state
                    .lane_incarnation_at_height(lane_id, proposal_height)
                    == Some(lane_incarnation)
        };
        let pruned_lane_sessions = self
            .subsystems
            .lane_blocks
            .retain_sessions_for_admissible_lanes(&admissible_lane);
        let pruned_committed_sessions = self
            .subsystems
            .committed_lane_blocks
            .retain_sessions_for_admissible_lanes(&admissible_lane);
        let next_proposal_height = self.committed_height_snapshot().saturating_add(1);
        let pruned_inactive_commit_locks = self
            .subsystems
            .lane_blocks
            .prune_commit_vote_locks_for_inactive_incarnations(
                |lane_id, dataspace_id, lane_incarnation| {
                    crate::state::consensus_lane_dataspace_at_height(
                        lane_id,
                        &nexus,
                        next_proposal_height,
                    ) == Some(dataspace_id)
                        && self
                            .state
                            .lane_incarnation_at_height(lane_id, next_proposal_height)
                            == Some(lane_incarnation)
                },
            );
        let pruned = pruned_lane_sessions
            .saturating_add(pruned_committed_sessions)
            .saturating_add(pruned_inactive_commit_locks);
        if pruned > 0 {
            debug!(
                pruned_lane_sessions,
                pruned_committed_sessions,
                pruned_inactive_commit_locks,
                "pruned cached lane-block sessions for inactive lane routes"
            );
        }
        pruned
    }

    fn prune_durably_finalized_lane_block_state(&mut self) -> usize {
        let candidate_slots = self
            .subsystems
            .lane_blocks
            .commit_vote_lock_slots()
            .into_iter()
            .chain(
                self.subsystems
                    .lane_blocks
                    .status_snapshot()
                    .into_iter()
                    .map(|session| {
                        (
                            session.lane_id,
                            session.dataspace_id,
                            session.lane_incarnation,
                            session.lane_block_height,
                        )
                    }),
            )
            .collect::<BTreeSet<_>>();
        let finalized_slots = candidate_slots
            .into_iter()
            .filter(
                |(lane_id, dataspace_id, lane_incarnation, lane_block_height)| {
                    self.lane_block_slot_is_durably_finalized(
                        *lane_id,
                        *dataspace_id,
                        *lane_incarnation,
                        *lane_block_height,
                    )
                },
            )
            .collect::<BTreeSet<_>>();
        self.subsystems
            .lane_blocks
            .prune_sessions_and_commit_vote_locks_for_finalized_slots(
                |lane_id, dataspace_id, lane_incarnation, lane_block_height| {
                    finalized_slots.contains(&(
                        lane_id,
                        dataspace_id,
                        lane_incarnation,
                        lane_block_height,
                    ))
                },
            )
    }

    pub(super) fn queue_committed_lane_block_sessions(&mut self) -> bool {
        let pruned_inactive_lane_sessions = self.prune_lane_block_sessions_for_inactive_lanes();
        let mut pruned_finalized_lane_state = self.prune_durably_finalized_lane_block_state();
        let pruned_applied_before = self
            .subsystems
            .committed_lane_blocks
            .prune_applied_or_snapshot_anchored_sessions_for_state(&self.state);
        let queued = self.enqueue_ready_committed_lane_block_sessions();
        let pending_before_processing = self.subsystems.committed_lane_blocks.len();
        if queued == 0
            && pruned_inactive_lane_sessions == 0
            && pruned_finalized_lane_state == 0
            && pruned_applied_before == 0
            && pending_before_processing == 0
        {
            return false;
        }
        // Autonomous lane certificates are not applied directly to the shared
        // WSV here. Cross-lane execution requires a consensus-certified merge
        // order; local QC arrival order is not a deterministic commit order.
        let recovered_inputs = self
            .subsystems
            .committed_lane_blocks
            .recover_certified_inputs_awaiting_merge(&self.state);
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
            .record_available_payload_application_receipts_into_kura_for_state(&self.state);
        let canonical_receipted_status = if canonical_application_receipts > 0 {
            self.subsystems
                .committed_lane_blocks
                .status_snapshot_for_state(&self.state)
        } else {
            Vec::new()
        };
        let pruned_canonical_receipted = self
            .subsystems
            .committed_lane_blocks
            .prune_application_receipted_sessions(self.state.kura());
        // Fail closed until the merge subsystem supplies one consensus-certified
        // total order. In particular, do not preflight/apply in local QC arrival
        // order and do not synthesize receipts from legacy direct-state markers.
        let application_receipts = self
            .subsystems
            .committed_lane_blocks
            .record_available_payload_application_receipts_into_kura_for_state(&self.state)
            .saturating_add(canonical_application_receipts);
        let committed_status_before_prune = self
            .subsystems
            .committed_lane_blocks
            .status_snapshot_for_state(&self.state);
        let pruned_applied_after = self
            .subsystems
            .committed_lane_blocks
            .prune_applied_or_snapshot_anchored_sessions_for_state(&self.state)
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
        pruned_finalized_lane_state = pruned_finalized_lane_state
            .saturating_add(self.prune_durably_finalized_lane_block_state());
        super::status::set_committed_lane_blocks(committed_status);
        self.publish_lane_block_session_status();
        debug!(
            queued,
            recovered_inputs,
            application_receipts,
            pruned_inactive_lane_sessions,
            pruned_finalized_lane_state,
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
            || application_receipts > 0
            || pruned_inactive_lane_sessions > 0
            || pruned_finalized_lane_state > 0
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

    fn lane_block_validator_set_pops(&self, validator_set: &[PeerId]) -> Option<Vec<Vec<u8>>> {
        if validator_set.is_empty()
            || validator_set.len() > crate::lane_consensus::MAX_LANE_BLOCK_VALIDATORS
        {
            return None;
        }
        let trusted = self.common_config.trusted_peers.value();
        validator_set
            .iter()
            .map(|validator| {
                if validator.public_key().try_algorithm().ok()
                    != Some(iroha_crypto::Algorithm::BlsNormal)
                {
                    return None;
                }
                let public_key = validator.public_key();
                let pop = if public_key == self.common_config.key_pair.public_key() {
                    iroha_crypto::bls_normal_pop_prove(self.common_config.key_pair.private_key())
                        .ok()?
                } else {
                    self.roster_validation_cache
                        .pops
                        .get(public_key)
                        .or_else(|| trusted.pops.get(public_key))?
                        .clone()
                };
                if pop.len() != crate::lane_consensus::LANE_BLS_PROOF_BYTES
                    || iroha_crypto::bls_normal_pop_verify(public_key, &pop).is_err()
                {
                    return None;
                }
                Some(pop)
            })
            .collect()
    }

    fn lane_new_view_certificate_signer_pops(
        &self,
        certificate: &crate::lane_consensus::LaneBlockNewViewCertificateV1,
    ) -> BTreeMap<PublicKey, Vec<u8>> {
        let trusted = self.common_config.trusted_peers.value();
        let mut pops = BTreeMap::new();
        for (byte_index, byte) in certificate.signers_bitmap.iter().copied().enumerate() {
            if byte == 0 {
                continue;
            }
            for bit in 0..8 {
                if byte & (1_u8 << bit) == 0 {
                    continue;
                }
                let signer_index = byte_index * 8 + bit;
                let Some(signer) = certificate.validator_set.get(signer_index) else {
                    continue;
                };
                let public_key = signer.public_key();
                if let Some(pop) = self
                    .roster_validation_cache
                    .pops
                    .get(public_key)
                    .or_else(|| trusted.pops.get(public_key))
                {
                    pops.insert(public_key.clone(), pop.clone());
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

fn lane_block_proposal_sender_is_admissible(
    proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    sender: &PeerId,
    sender_is_shared_authority: bool,
    proposal_is_cached: bool,
    canonical_payload_available: bool,
    authenticated_new_view_chain_available: bool,
) -> bool {
    if crate::lane_consensus::validate_lane_block_proposal(proposal).is_err() {
        return false;
    }
    let sender_is_lane_validator = proposal.descriptor.validator_set.contains(sender);
    if !sender_is_lane_validator && !sender_is_shared_authority {
        return false;
    }
    if proposal_is_cached {
        return true;
    }

    if proposal.descriptor.lane_block_view > 0 {
        return authenticated_new_view_chain_available;
    }
    if canonical_payload_available || authenticated_new_view_chain_available {
        return true;
    }

    // An uncached initial view is authored only by the deterministic lane
    // leader. Higher views enter through the authenticated NewView certificate
    // path above; a shared/global-authority boolean alone never claims a slot.
    lane_block_initial_origin_is_admissible(proposal, sender, sender_is_shared_authority)
}

fn lane_block_initial_origin_is_admissible(
    proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
    sender: &PeerId,
    _sender_is_shared_authority: bool,
) -> bool {
    crate::lane_consensus::validate_lane_block_proposal(proposal).is_ok()
        && proposal.descriptor.lane_block_view == 0
        && super::lane_scheduler::lane_block_redrive_leader(proposal, 0) == Some(sender)
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

    fn proposal_with_view(lane_block_view: u64) -> crate::sumeragi::consensus::LaneBlockProposalV1 {
        let mut validator_set = vec![peer(4), peer(1), peer(3), peer(2)];
        validator_set.sort();
        let mut descriptor = crate::sumeragi::consensus::LaneBlockDescriptorV1 {
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: Hash::new(b"lane-block-ingress-fixture-incarnation"),
            proposal_height: 5,
            previous_lane_block_height: 2,
            previous_lane_block_descriptor_hash: Some(Hash::prehashed([0x20; Hash::LENGTH])),
            lane_block_height: 3,
            lane_block_view,
            subject_hash: Hash::prehashed([0x21; Hash::LENGTH]),
            payload_ownership_hash: Hash::prehashed([0x22; Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([0x23; Hash::LENGTH]),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::prehashed([0x24; Hash::LENGTH])],
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set,
            validator_count: 4,
            min_quorum: 3,
            qc_mode_tag: "fixture:lane:7:dataspace:11".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = crate::sumeragi::consensus::LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    #[test]
    fn lane_block_sender_policy_requires_lane_leader_before_canonical_recovery() {
        let proposal = proposal_with_view(0);
        let leader = super::super::lane_scheduler::lane_block_redrive_leader(&proposal, 0)
            .expect("canonical proposal leader")
            .clone();
        let wrong_validator = proposal
            .descriptor
            .validator_set
            .iter()
            .find(|peer| *peer != &leader)
            .expect("backup validator")
            .clone();

        assert!(lane_block_proposal_sender_is_admissible(
            &proposal, &leader, false, false, false, false
        ));
        assert!(!lane_block_proposal_sender_is_admissible(
            &proposal,
            &wrong_validator,
            false,
            false,
            false,
            false
        ));
        assert!(!lane_block_initial_origin_is_admissible(
            &proposal,
            &wrong_validator,
            true
        ));
        assert!(!lane_block_proposal_sender_is_admissible(
            &proposal,
            &wrong_validator,
            true,
            false,
            false,
            false
        ));
        assert!(lane_block_proposal_sender_is_admissible(
            &proposal,
            &wrong_validator,
            false,
            true,
            false,
            false
        ));
        assert!(lane_block_proposal_sender_is_admissible(
            &proposal,
            &wrong_validator,
            false,
            false,
            true,
            false
        ));

        let outsider = peer(99);
        assert!(!lane_block_proposal_sender_is_admissible(
            &proposal, &outsider, false, false, true, false
        ));
        assert!(lane_block_proposal_sender_is_admissible(
            &proposal, &outsider, true, false, true, false
        ));
    }

    #[test]
    fn lane_block_sender_policy_rejects_unproven_higher_view_and_forgery() {
        let proposal = proposal_with_view(3);
        let leader = super::super::lane_scheduler::lane_block_redrive_leader(&proposal, 0)
            .expect("higher-view proposal leader")
            .clone();
        assert!(!lane_block_proposal_sender_is_admissible(
            &proposal, &leader, false, false, false, false
        ));
        assert!(!lane_block_proposal_sender_is_admissible(
            &proposal, &leader, true, false, false, false
        ));
        assert!(!lane_block_proposal_sender_is_admissible(
            &proposal, &leader, false, false, true, false
        ));
        assert!(lane_block_proposal_sender_is_admissible(
            &proposal, &leader, false, false, false, true
        ));

        let mut forged = proposal;
        forged.proposal_hash = Hash::prehashed([0xEE; Hash::LENGTH]);
        assert!(!lane_block_proposal_sender_is_admissible(
            &forged, &leader, false, true, true, true
        ));
    }

    #[test]
    fn lane_block_proposal_from_payload_ownership_reconstructs_canonical_proposal() {
        let mut validator_set = vec![peer(3), peer(1), peer(2)];
        validator_set.sort();
        let mut descriptor = crate::sumeragi::consensus::LaneBlockDescriptorV1 {
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: Hash::new(b"lane-block-ingress-fixture-incarnation"),
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
            lane_incarnation: descriptor.lane_incarnation,
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
            lane_incarnation: Hash::new(b"lane-block-ingress-fixture-incarnation"),
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

fn lane_block_session_key_for_proposal(
    proposal: &crate::sumeragi::consensus::LaneBlockProposalV1,
) -> crate::lane_consensus::LaneBlockSessionKey {
    crate::lane_consensus::LaneBlockSessionKey {
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        proposal_hash: proposal.proposal_hash,
    }
}

fn lane_block_vote_rebroadcast_identity(
    vote: &crate::lane_consensus::LaneBlockVoteV1,
) -> Option<(
    LaneBlockRebroadcastKind,
    crate::lane_consensus::LaneBlockSessionKey,
)> {
    let kind = match vote.body.phase {
        crate::sumeragi::consensus::Phase::Prepare => LaneBlockRebroadcastKind::PrepareVote,
        crate::sumeragi::consensus::Phase::Commit => LaneBlockRebroadcastKind::CommitVote,
        crate::sumeragi::consensus::Phase::NewView => return None,
    };
    Some((
        kind,
        crate::lane_consensus::LaneBlockSessionKey {
            lane_id: vote.body.lane_id,
            dataspace_id: vote.body.dataspace_id,
            lane_incarnation: vote.body.lane_incarnation,
            lane_block_height: vote.body.lane_block_height,
            lane_block_view: vote.body.lane_block_view,
            proposal_hash: vote.body.proposal_hash,
        },
    ))
}

fn lane_block_new_view_vote_rebroadcast_identity(
    vote: &crate::lane_consensus::LaneBlockNewViewVoteV1,
) -> crate::lane_consensus::LaneBlockSessionKey {
    crate::lane_consensus::LaneBlockSessionKey {
        lane_id: vote.body.lane_id,
        dataspace_id: vote.body.dataspace_id,
        lane_incarnation: vote.body.lane_incarnation,
        lane_block_height: vote.body.lane_block_height,
        lane_block_view: vote.body.from_view,
        proposal_hash: vote.body.locked_proposal_hash,
    }
}

fn lane_block_qc_rebroadcast_identity(
    qc: &crate::sumeragi::consensus::LaneBlockQcV1,
) -> Option<(
    LaneBlockRebroadcastKind,
    crate::lane_consensus::LaneBlockSessionKey,
)> {
    let kind = match qc.body.phase {
        crate::sumeragi::consensus::Phase::Prepare => LaneBlockRebroadcastKind::PrepareQc,
        crate::sumeragi::consensus::Phase::Commit => LaneBlockRebroadcastKind::CommitQc,
        crate::sumeragi::consensus::Phase::NewView => return None,
    };
    Some((
        kind,
        crate::lane_consensus::LaneBlockSessionKey {
            lane_id: qc.body.lane_id,
            dataspace_id: qc.body.dataspace_id,
            lane_incarnation: qc.body.lane_incarnation,
            lane_block_height: qc.body.lane_block_height,
            lane_block_view: qc.body.lane_block_view,
            proposal_hash: qc.body.proposal_hash,
        },
    ))
}

fn lane_block_rebroadcast_identity(
    msg: &BlockMessage,
) -> Option<(
    LaneBlockRebroadcastKind,
    crate::lane_consensus::LaneBlockSessionKey,
)> {
    match msg {
        BlockMessage::LaneBlockProposal(proposal) => Some((
            LaneBlockRebroadcastKind::Proposal,
            lane_block_session_key_for_proposal(proposal),
        )),
        BlockMessage::LaneBlockVote(vote) => lane_block_vote_rebroadcast_identity(vote),
        BlockMessage::LaneBlockQc(qc) => lane_block_qc_rebroadcast_identity(qc),
        _ => None,
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
        | crate::lane_consensus::LaneBlockSessionError::EntrypointAlreadyClaimed
        | crate::lane_consensus::LaneBlockSessionError::VoteProposalMismatch
        | crate::lane_consensus::LaneBlockSessionError::VoteSignerNotInValidatorSet
        | crate::lane_consensus::LaneBlockSessionError::AvailabilityNotAuthorized
        | crate::lane_consensus::LaneBlockSessionError::AvailabilityMismatch
        | crate::lane_consensus::LaneBlockSessionError::CommitBeforePrepareQc
        | crate::lane_consensus::LaneBlockSessionError::QcProposalMismatch
        | crate::lane_consensus::LaneBlockSessionError::ConflictingQc => {
            super::status::ConsensusMessageReason::InvalidPayload
        }
    }
}
