//! Bounded lane-local consensus, merge, and Native AMX adapter for Sumeragi v2.
//!
//! Global consensus is owned exclusively by the v2 reducer. This module keeps
//! the independent lane-local Prepare/Commit sessions, deterministic RBC
//! ownership identities, merge signatures, and context-bound Native AMX
//! receipts as bounded transport/validity inputs. A certified lane session is
//! persisted only after a canonical global block anchors the exact ownership;
//! a losing global proposal can therefore never advance the durable lane tip.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    num::NonZeroUsize,
    sync::Arc,
};

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey, Signature};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{
            CertPhase, LaneBlockDescriptorV1, LaneBlockProposalPayloadHintV1, LaneBlockProposalV1,
            LaneBlockQcV1, NativeAmxAttestationBodyV2, NativeAmxAttestationQcV2,
            NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt, SumeragiLanePayloadOwnership,
        },
        consensus_v2 as wire,
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    merge::{MergeCommitteeSignature, MergeQuorumCertificate, MergeSignerProof},
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    peer::PeerId,
};
use norito::codec::Encode as _;
use thiserror::Error;

use super::{
    InboundBlockMessage, LaneRelayMessage,
    main_loop::lane_scheduler::{prepare_v2_lane_payload_plan, proposal_lookahead_enabled},
    message::BlockMessage,
    v2_candidate::{
        CandidateDescriptor, CandidateWorkProvider, CandidateWorkUnavailable, PreparedCandidateWork,
    },
};
use crate::{
    kura::Kura,
    lane_consensus::{
        CommittedLaneBlockSession, LaneBlockSessionCache, LaneBlockSessionInsertOutcome,
        LaneBlockVoteV1,
    },
    native_amx::{
        NativeAmxCommitRequestV2, NativeAmxMessage, NativeAmxSessionCache, NativeAmxSessionError,
        NativeAmxSessionKey, NativeAmxVoteV2, aggregate_votes_to_qc, validate_native_amx_qc,
    },
    queue::{RoutingDecision, RoutingPlan},
    state::State,
};

/// Exact local bounds for one height-local lane/AMX adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct V2LaneWorkLimits {
    session_capacity: NonZeroUsize,
    body_buckets_per_session: NonZeroUsize,
    effect_capacity: NonZeroUsize,
    relay_capacity: NonZeroUsize,
    merge_capacity: NonZeroUsize,
    native_request_capacity: NonZeroUsize,
}

impl V2LaneWorkLimits {
    /// Construct non-zero bounds for every retained collection.
    pub(crate) fn new(
        session_capacity: NonZeroUsize,
        body_buckets_per_session: NonZeroUsize,
        effect_capacity: NonZeroUsize,
        relay_capacity: NonZeroUsize,
        merge_capacity: NonZeroUsize,
        native_request_capacity: NonZeroUsize,
    ) -> Self {
        Self {
            session_capacity,
            body_buckets_per_session,
            effect_capacity,
            relay_capacity,
            merge_capacity,
            native_request_capacity,
        }
    }
}

/// One authenticated lane-local transport action emitted by the adapter.
#[derive(Clone, Debug)]
pub(crate) enum V2LaneWorkEffect {
    /// Send a standalone lane proposal/vote/QC to one committee member.
    PostLaneBlock {
        /// Destination committee member.
        peer: PeerId,
        /// Lane-local message; global legacy variants are never emitted.
        message: BlockMessage,
    },
    /// Send a context-bound Native AMX request or vote to one peer.
    PostNativeAmx {
        /// Destination participant/coordinator.
        peer: PeerId,
        /// Context-bound Native AMX v2 message.
        message: NativeAmxMessage,
    },
    /// Broadcast a merge signature share to the frozen voting roster.
    BroadcastMerge(MergeCommitteeSignature),
}

/// Outcome of one bounded lane/AMX ingress operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum V2LaneIngressOutcome {
    /// New authenticated state was retained.
    Inserted,
    /// The exact artifact was already retained.
    Duplicate,
    /// The artifact was malformed, stale, unauthorized, conflicting, or over capacity.
    Rejected,
}

/// Fail-closed adapter construction or durable-retention error.
#[derive(Debug, Error)]
pub(crate) enum V2LaneWorkError {
    /// Frozen context is malformed.
    #[error("invalid Sumeragi v2 height context: {0}")]
    InvalidContext(String),
    /// Committed Nexus/AMX projection differs from the frozen context.
    #[error("committed Nexus/AMX context does not match the frozen height context")]
    NexusContextMismatch,
    /// Local consensus key does not match the supplied peer identity.
    #[error("local lane/AMX consensus key does not match the local peer")]
    LocalKeyMismatch,
    /// Durable lane certificate persistence failed.
    #[error("failed to persist anchored lane-local certificate: {0}")]
    Persistence(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NativeVoteClaimKey {
    session: NativeAmxSessionKey,
    round: wire::ConsensusRound,
    epoch: u64,
    participant_lane: LaneId,
    participant_dataspace: DataSpaceId,
    phase: NativeAmxPhase,
    signer: HashOf<PeerId>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NativeRequestKey {
    body: NativeAmxAttestationBodyV2,
    peer: PeerId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MergeKey {
    epoch_id: u64,
    view: u64,
    digest: Hash,
}

#[derive(Clone, Debug)]
struct PendingMerge {
    candidate: crate::merge::MergeLedgerCandidate,
    signatures: BTreeMap<wire::ValidatorIndex, Vec<u8>>,
}

/// Authoritative bounded adapter retained for exactly one global height.
pub(crate) struct V2LaneWorkAdapter {
    context: wire::HeightContext,
    local_peer: PeerId,
    key_pair: KeyPair,
    voting_enabled: bool,
    state: Arc<State>,
    kura: Arc<Kura>,
    limits: V2LaneWorkLimits,
    lane_sessions: LaneBlockSessionCache,
    native_sessions: NativeAmxSessionCache,
    native_claims: BTreeMap<NativeVoteClaimKey, NativeAmxAttestationBodyV2>,
    native_claim_order: VecDeque<NativeVoteClaimKey>,
    local_native_claims: BTreeMap<NativeVoteClaimKey, NativeAmxAttestationBodyV2>,
    native_requests: BTreeMap<NativeRequestKey, NativeAmxMessage>,
    planned_lane_proposals: BTreeMap<wire::ConsensusRound, Vec<LaneBlockProposalV1>>,
    pending_local_lane_proposals: BTreeMap<HashOf<BlockHeader>, Vec<LaneBlockProposalV1>>,
    locally_bound_lane_proposals: BTreeSet<Hash>,
    pending_committed_lanes: VecDeque<CommittedLaneBlockSession>,
    admitted_relays: BTreeSet<(LaneId, DataSpaceId, u64, Hash)>,
    merge_entries: BTreeMap<MergeKey, PendingMerge>,
    merge_claims: BTreeMap<(u64, u64, wire::ValidatorIndex), Hash>,
    effects: VecDeque<V2LaneWorkEffect>,
    effect_keys: BTreeSet<Hash>,
    lane_fanout_cursor: usize,
    lane_artifact_cursor: usize,
    native_retransmit_cursor: usize,
}

impl V2LaneWorkAdapter {
    /// Open one adapter after verifying the frozen Nexus/AMX commitment.
    ///
    /// # Errors
    ///
    /// Returns [`V2LaneWorkError`] for malformed context, local-key drift, or
    /// committed-state/context drift.
    pub(crate) fn new(
        context: wire::HeightContext,
        local_peer: PeerId,
        key_pair: KeyPair,
        voting_enabled: bool,
        state: Arc<State>,
        kura: Arc<Kura>,
        limits: V2LaneWorkLimits,
    ) -> Result<Self, V2LaneWorkError> {
        context
            .validate()
            .map_err(|error| V2LaneWorkError::InvalidContext(error.to_string()))?;
        if local_peer.public_key() != key_pair.public_key() {
            return Err(V2LaneWorkError::LocalKeyMismatch);
        }
        if super::v2_recovery::committed_nexus_amx_context_hash(state.as_ref())
            != context.nexus_amx_context_hash
        {
            return Err(V2LaneWorkError::NexusContextMismatch);
        }
        let mut adapter = Self {
            context,
            local_peer,
            key_pair,
            voting_enabled,
            state,
            kura,
            limits,
            lane_sessions: LaneBlockSessionCache::new(limits.session_capacity.get()),
            native_sessions: NativeAmxSessionCache::with_limits(
                limits.session_capacity,
                limits.body_buckets_per_session,
            ),
            native_claims: BTreeMap::new(),
            native_claim_order: VecDeque::new(),
            local_native_claims: BTreeMap::new(),
            native_requests: BTreeMap::new(),
            planned_lane_proposals: BTreeMap::new(),
            pending_local_lane_proposals: BTreeMap::new(),
            locally_bound_lane_proposals: BTreeSet::new(),
            pending_committed_lanes: VecDeque::new(),
            admitted_relays: BTreeSet::new(),
            merge_entries: BTreeMap::new(),
            merge_claims: BTreeMap::new(),
            effects: VecDeque::new(),
            effect_keys: BTreeSet::new(),
            lane_fanout_cursor: 0,
            lane_artifact_cursor: 0,
            native_retransmit_cursor: 0,
        };
        adapter.hydrate_canonical_lane_artifacts();
        adapter.refresh_merge_candidates();
        adapter.drive_lane_sessions();
        Ok(adapter)
    }

    /// Bind locally planned lane proposals to the exact global block body.
    pub(crate) fn bind_local_candidate(
        &mut self,
        round: wire::ConsensusRound,
        block_hash: HashOf<BlockHeader>,
    ) -> V2LaneIngressOutcome {
        if !self.round_is_current(round) {
            return V2LaneIngressOutcome::Rejected;
        }
        let Some(proposals) = self.planned_lane_proposals.remove(&round) else {
            return V2LaneIngressOutcome::Duplicate;
        };
        let proposals = proposals
            .into_iter()
            .map(|proposal| {
                proposal.with_payload_block_hint(LaneBlockProposalPayloadHintV1 {
                    proposal_height: round.height,
                    proposal_view: round.view,
                    proposal_block_hash: block_hash,
                })
            })
            .collect::<Vec<_>>();
        let mut next_sessions = self.lane_sessions.clone();
        let mut inserted = false;
        for proposal in &proposals {
            if !self.lane_proposal_authorized(proposal, None, true, round.view) {
                return V2LaneIngressOutcome::Rejected;
            }
            match next_sessions.insert_proposal(proposal.clone()) {
                Ok(LaneBlockSessionInsertOutcome::Inserted) => inserted = true,
                Ok(LaneBlockSessionInsertOutcome::Duplicate) => {}
                Err(_) => return V2LaneIngressOutcome::Rejected,
            }
        }
        self.lane_sessions = next_sessions;
        self.locally_bound_lane_proposals.clear();
        self.pending_local_lane_proposals.clear();
        self.pending_local_lane_proposals
            .insert(block_hash, proposals);
        if inserted {
            V2LaneIngressOutcome::Inserted
        } else {
            V2LaneIngressOutcome::Duplicate
        }
    }

    /// Release only the exact locally planned lane artifacts whose global body
    /// has completed deterministic production validation and durable Prepare
    /// intent. A remote proposal sharing the same block hash cannot populate
    /// this map and therefore cannot trick an honest node into signing an
    /// unrelated lane descriptor.
    pub(crate) fn mark_global_body_validated(&mut self, block_hash: HashOf<BlockHeader>) {
        let Some(proposals) = self.pending_local_lane_proposals.remove(&block_hash) else {
            return;
        };
        self.locally_bound_lane_proposals.clear();
        self.locally_bound_lane_proposals
            .extend(proposals.iter().map(|proposal| proposal.proposal_hash));
        for proposal in proposals {
            self.fanout_lane_message(
                BlockMessage::LaneBlockProposal(proposal.clone()),
                &proposal.descriptor.validator_set,
            );
        }
        self.drive_lane_sessions();
    }

    /// Persist only completed lane sessions anchored by canonical Kura blocks.
    ///
    /// # Errors
    ///
    /// Returns [`V2LaneWorkError::Persistence`] if an anchored certificate
    /// cannot be written durably.
    pub(crate) fn persist_anchored_sessions(&mut self) -> Result<usize, V2LaneWorkError> {
        self.collect_committed_lane_sessions();
        let mut retained = VecDeque::new();
        let mut persisted = 0usize;
        while let Some(session) = self.pending_committed_lanes.pop_front() {
            if !self.session_has_canonical_anchor(&session) {
                retained.push_back(session);
                continue;
            }
            let pops = self.pops_for_lane_session(&session);
            self.kura
                .persist_committed_lane_block_session(&session, &pops)
                .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
            persisted = persisted.saturating_add(1);
        }
        self.pending_committed_lanes = retained;
        Ok(persisted)
    }

    /// Accept a lane proposal/vote/QC from the existing bounded ingress lanes.
    pub(crate) fn accept_lane_message(
        &mut self,
        inbound: InboundBlockMessage,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let (message, sender) = inbound.into_message_and_sender();
        let outcome = match message {
            BlockMessage::LaneBlockProposal(proposal) => {
                self.insert_lane_proposal(proposal, sender.as_ref(), false, active_view)
            }
            BlockMessage::LaneBlockVote(vote) => {
                self.insert_lane_vote(vote, sender.as_ref(), active_view)
            }
            BlockMessage::LaneBlockQc(qc) => self.insert_lane_qc(qc, active_view),
            _ => V2LaneIngressOutcome::Rejected,
        };
        if outcome != V2LaneIngressOutcome::Rejected {
            self.drive_lane_sessions();
        }
        outcome
    }

    /// Accept one lane relay, merge signature, or context-bound Native AMX message.
    pub(super) fn accept_relay_message(
        &mut self,
        message: LaneRelayMessage,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        match message {
            LaneRelayMessage::Envelope(envelope) => self.accept_lane_relay(envelope),
            LaneRelayMessage::MergeSignature(signature) => self.accept_merge_signature(signature),
            LaneRelayMessage::CertifiedMergeSidecar { .. }
            | LaneRelayMessage::MergeCandidate { .. } => V2LaneIngressOutcome::Rejected,
            LaneRelayMessage::NativeAmx { sender, message } => {
                self.accept_native_amx(sender, message, active_view)
            }
        }
    }

    /// Drain at most `limit` explicit transport effects.
    pub(crate) fn drain_effects(&mut self, limit: usize) -> Vec<V2LaneWorkEffect> {
        let mut drained = Vec::with_capacity(limit.min(self.effects.len()));
        for _ in 0..limit {
            let Some(effect) = self.effects.pop_front() else {
                break;
            };
            self.effect_keys.remove(&lane_work_effect_key(&effect));
            drained.push(effect);
        }
        drained
    }

    /// Re-enqueue bounded lane votes, QCs, and Native AMX requests for reliable
    /// point-to-point retransmission.
    pub(crate) fn schedule_retransmission(&mut self) {
        let mut lane_artifacts = Vec::new();
        for proposal in self.lane_sessions.proposals_without_commit_qc() {
            lane_artifacts.push((
                BlockMessage::LaneBlockProposal(proposal.clone()),
                proposal.descriptor.validator_set,
            ));
        }
        for (proposal, vote) in self
            .lane_sessions
            .local_vote_rebroadcast_artifacts_for(&self.local_peer)
        {
            lane_artifacts.push((
                BlockMessage::LaneBlockVote(vote),
                proposal.descriptor.validator_set,
            ));
        }
        for qc in self.lane_sessions.qcs_for_incomplete_sessions() {
            let validators = qc.validator_set.clone();
            lane_artifacts.push((BlockMessage::LaneBlockQc(qc), validators));
        }
        if !lane_artifacts.is_empty() {
            let start = self.lane_artifact_cursor % lane_artifacts.len();
            let mut advanced = 0usize;
            for offset in 0..lane_artifacts.len() {
                let (message, validators) =
                    &lane_artifacts[(start + offset) % lane_artifacts.len()];
                self.fanout_lane_message(message.clone(), validators);
                advanced = advanced.saturating_add(1);
                if self.effects.len() >= self.limits.effect_capacity.get() {
                    break;
                }
            }
            self.lane_artifact_cursor = (start + advanced.max(1)) % lane_artifacts.len();
        }
        let requests = self
            .native_requests
            .iter()
            .map(|(key, message)| (key.peer.clone(), message.clone()))
            .collect::<Vec<_>>();
        if !requests.is_empty() {
            let start = self.native_retransmit_cursor % requests.len();
            let mut advanced = 0usize;
            for offset in 0..requests.len() {
                let (peer, message) = requests[(start + offset) % requests.len()].clone();
                if !self.push_effect(V2LaneWorkEffect::PostNativeAmx { peer, message }) {
                    break;
                }
                advanced = advanced.saturating_add(1);
            }
            self.native_retransmit_cursor = (start + advanced.max(1)) % requests.len();
        }
        let mut merge_effects = Vec::new();
        if let Some(local_index) = self.local_validator_index() {
            for pending in self.merge_entries.values() {
                let Some(signature) = pending.signatures.get(&local_index) else {
                    continue;
                };
                merge_effects.push(V2LaneWorkEffect::BroadcastMerge(MergeCommitteeSignature {
                    epoch_id: pending.candidate.epoch_id,
                    view: pending.candidate.view,
                    signer: local_index,
                    message_digest: crate::merge::merge_qc_message_digest(
                        &self.context.chain_id,
                        &pending.candidate,
                        VALIDATOR_SET_HASH_VERSION_V1,
                        self.frozen_validator_set_hash(),
                    ),
                    bls_sig: signature.clone(),
                }));
            }
        }
        for effect in merge_effects {
            self.push_effect(effect);
        }
    }

    fn round_is_current(&self, round: wire::ConsensusRound) -> bool {
        round.context_id == self.context.id() && round.height == self.context.height
    }

    fn accept_lane_relay(&mut self, envelope: LaneRelayEnvelope) -> V2LaneIngressOutcome {
        let key = (
            envelope.lane_id,
            envelope.dataspace_id,
            envelope.block_height,
            Hash::from(envelope.settlement_hash),
        );
        if self.admitted_relays.contains(&key) {
            return V2LaneIngressOutcome::Duplicate;
        }
        if self.admitted_relays.len() >= self.limits.relay_capacity.get() {
            return V2LaneIngressOutcome::Rejected;
        }
        match self.state.record_lane_relay(&envelope) {
            Ok(crate::state::LaneRelayInsert::Duplicate) => V2LaneIngressOutcome::Duplicate,
            Ok(
                crate::state::LaneRelayInsert::Inserted | crate::state::LaneRelayInsert::Replaced,
            ) => {
                self.admitted_relays.insert(key);
                self.refresh_merge_candidates();
                V2LaneIngressOutcome::Inserted
            }
            Err(_) => V2LaneIngressOutcome::Rejected,
        }
    }

    fn insert_lane_proposal(
        &mut self,
        proposal: LaneBlockProposalV1,
        sender: Option<&PeerId>,
        local: bool,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        if !self.lane_proposal_authorized(&proposal, sender, local, active_view) {
            return V2LaneIngressOutcome::Rejected;
        }
        match self.lane_sessions.insert_proposal(proposal) {
            Ok(LaneBlockSessionInsertOutcome::Inserted) => V2LaneIngressOutcome::Inserted,
            Ok(LaneBlockSessionInsertOutcome::Duplicate) => V2LaneIngressOutcome::Duplicate,
            Err(_) => V2LaneIngressOutcome::Rejected,
        }
    }

    fn insert_lane_vote(
        &mut self,
        vote: LaneBlockVoteV1,
        sender: Option<&PeerId>,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        if sender != Some(&vote.signer) || !self.lane_vote_authorized(&vote, active_view) {
            return V2LaneIngressOutcome::Rejected;
        }
        match self.lane_sessions.insert_vote(vote, sender) {
            Ok(LaneBlockSessionInsertOutcome::Inserted) => V2LaneIngressOutcome::Inserted,
            Ok(LaneBlockSessionInsertOutcome::Duplicate) => V2LaneIngressOutcome::Duplicate,
            Err(_) => V2LaneIngressOutcome::Rejected,
        }
    }

    fn insert_lane_qc(
        &mut self,
        qc: LaneBlockQcV1,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        if !self.lane_qc_authorized(&qc, active_view) {
            return V2LaneIngressOutcome::Rejected;
        }
        let pops = self.pops_for_lane_qc(&qc);
        match self.lane_sessions.insert_qc_with_pops(qc, &pops) {
            Ok(LaneBlockSessionInsertOutcome::Inserted) => V2LaneIngressOutcome::Inserted,
            Ok(LaneBlockSessionInsertOutcome::Duplicate) => V2LaneIngressOutcome::Duplicate,
            Err(_) => V2LaneIngressOutcome::Rejected,
        }
    }

    fn lane_proposal_authorized(
        &self,
        proposal: &LaneBlockProposalV1,
        sender: Option<&PeerId>,
        local: bool,
        active_view: wire::View,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        if let Some(anchor) = self.canonical_anchor_for_proposal(proposal) {
            return proposal.payload_block_hint.as_ref().is_some_and(|hint| {
                hint.proposal_block_hash == anchor.proposal_block_hash
                    && hint.proposal_height == descriptor.proposal_height
                    && hint.proposal_view == descriptor.lane_block_view
            });
        }
        if descriptor.proposal_height != self.context.height
            || descriptor.lane_block_view > active_view
            || proposal.payload_block_hint.as_ref().is_none_or(|hint| {
                hint.proposal_height != descriptor.proposal_height
                    || hint.proposal_view != descriptor.lane_block_view
            })
            || !self.qc_mode_tag_matches_context(
                &descriptor.qc_mode_tag,
                descriptor.lane_id,
                descriptor.dataspace_id,
            )
            || !self.lane_route_active(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.proposal_height,
                descriptor.lane_block_height,
            )
            || self.expected_lane_validators(descriptor.lane_id, descriptor.proposal_height)
                != Some(descriptor.validator_set.clone())
        {
            return false;
        }
        let Some(author) = self.expected_lane_author(proposal) else {
            return false;
        };
        (local && &self.local_peer == author) || sender == Some(author)
    }

    fn expected_lane_author<'a>(&'a self, proposal: &'a LaneBlockProposalV1) -> Option<&'a PeerId> {
        let nexus = self.state.nexus_snapshot();
        if !nexus.enabled
            || !proposal_lookahead_enabled(&nexus, proposal.descriptor.proposal_height)
        {
            let index =
                usize::try_from(self.context.leader(proposal.descriptor.lane_block_view)).ok()?;
            return self.context.roster.get(index).map(|entry| &entry.validator);
        }
        lane_proposal_author(proposal)
    }

    fn lane_vote_authorized(&self, vote: &LaneBlockVoteV1, active_view: wire::View) -> bool {
        let body = &vote.body;
        if body.proposal_height == self.context.height {
            body.lane_block_view <= active_view
                && self.qc_mode_tag_matches_context(
                    &body.qc_mode_tag,
                    body.lane_id,
                    body.dataspace_id,
                )
                && self.lane_route_active(
                    body.lane_id,
                    body.dataspace_id,
                    body.proposal_height,
                    body.lane_block_height,
                )
                && self
                    .expected_lane_validators(body.lane_id, body.proposal_height)
                    .is_some_and(|validators| {
                        HashOf::new(&validators) == body.validator_set_hash
                            && validators.contains(&vote.signer)
                    })
        } else {
            self.canonical_proposal_for_vote_body(body)
                .is_some_and(|proposal| proposal.descriptor.validator_set.contains(&vote.signer))
        }
    }

    fn lane_qc_authorized(&self, qc: &LaneBlockQcV1, active_view: wire::View) -> bool {
        let body = &qc.body;
        if body.proposal_height == self.context.height {
            body.lane_block_view <= active_view
                && self.qc_mode_tag_matches_context(
                    &body.qc_mode_tag,
                    body.lane_id,
                    body.dataspace_id,
                )
                && self.lane_route_active(
                    body.lane_id,
                    body.dataspace_id,
                    body.proposal_height,
                    body.lane_block_height,
                )
                && self
                    .expected_lane_validators(body.lane_id, body.proposal_height)
                    .is_some_and(|validators| validators == qc.validator_set)
        } else {
            self.canonical_proposal_for_vote_body(body)
                .is_some_and(|proposal| proposal.descriptor.validator_set == qc.validator_set)
        }
    }

    fn drive_lane_sessions(&mut self) {
        let proposals = self
            .lane_sessions
            .local_prepare_vote_proposals_for(&self.local_peer);
        for proposal in proposals {
            if !self.proposal_body_available(&proposal) {
                continue;
            }
            let body = proposal.vote_body(CertPhase::Prepare);
            let Some(vote) = self.sign_lane_vote(body) else {
                continue;
            };
            if self
                .lane_sessions
                .insert_vote(vote.clone(), Some(&self.local_peer))
                .is_ok()
            {
                self.fanout_lane_message(
                    BlockMessage::LaneBlockVote(vote),
                    &proposal.descriptor.validator_set,
                );
            }
        }

        let commit_requests = self
            .lane_sessions
            .local_commit_vote_requests_for(&self.local_peer);
        for request in commit_requests {
            if !self.proposal_body_available(&request.proposal) {
                continue;
            }
            let body = request.proposal.vote_body(CertPhase::Commit);
            let Some(vote) = self.sign_lane_vote(body) else {
                continue;
            };
            if self
                .lane_sessions
                .insert_vote(vote.clone(), Some(&self.local_peer))
                .is_ok()
            {
                self.fanout_lane_message(
                    BlockMessage::LaneBlockVote(vote),
                    &request.proposal.descriptor.validator_set,
                );
            }
        }

        for qc in self.lane_sessions.drain_newly_sealed_qcs() {
            let validators = qc.validator_set.clone();
            self.fanout_lane_message(BlockMessage::LaneBlockQc(qc), &validators);
        }
        self.collect_committed_lane_sessions();
    }

    fn sign_lane_vote(
        &self,
        body: iroha_data_model::block::consensus::LaneBlockVoteBodyV1,
    ) -> Option<LaneBlockVoteV1> {
        if !self.voting_enabled
            || self.local_peer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
        {
            return None;
        }
        let signature =
            Signature::try_new(self.key_pair.private_key(), &body.signature_preimage()).ok()?;
        Some(LaneBlockVoteV1 {
            body,
            payload_availability_vote: None,
            signer: self.local_peer.clone(),
            bls_signature: signature.payload().to_vec(),
        })
    }

    fn fanout_lane_message(&mut self, message: BlockMessage, validators: &[PeerId]) {
        let mut seen = BTreeSet::new();
        let peers = validators
            .iter()
            .filter(|peer| *peer != &self.local_peer && seen.insert((*peer).clone()))
            .cloned()
            .collect::<Vec<_>>();
        if peers.is_empty() {
            return;
        }
        let start = self.lane_fanout_cursor % peers.len();
        let mut advanced = 0usize;
        for offset in 0..peers.len() {
            let peer = peers[(start + offset) % peers.len()].clone();
            if !self.push_effect(V2LaneWorkEffect::PostLaneBlock {
                peer,
                message: message.clone(),
            }) {
                break;
            }
            advanced = advanced.saturating_add(1);
        }
        self.lane_fanout_cursor = (start + advanced.max(1)) % peers.len();
    }

    fn push_effect(&mut self, effect: V2LaneWorkEffect) -> bool {
        let key = lane_work_effect_key(&effect);
        if self.effect_keys.contains(&key) {
            return true;
        }
        if self.effects.len() >= self.limits.effect_capacity.get() {
            return false;
        }
        self.effect_keys.insert(key);
        self.effects.push_back(effect);
        true
    }

    fn collect_committed_lane_sessions(&mut self) {
        let remaining = self
            .limits
            .session_capacity
            .get()
            .saturating_sub(self.pending_committed_lanes.len());
        self.pending_committed_lanes
            .extend(self.lane_sessions.drain_committed_sessions_up_to(remaining));
    }

    fn proposal_body_available(&self, proposal: &LaneBlockProposalV1) -> bool {
        self.canonical_anchor_for_proposal(proposal).is_some()
            || self
                .locally_bound_lane_proposals
                .contains(&proposal.proposal_hash)
    }

    fn expected_lane_validators(
        &self,
        lane_id: LaneId,
        proposal_height: u64,
    ) -> Option<Vec<PeerId>> {
        if proposal_height != self.context.height {
            return None;
        }
        let nexus = self.state.nexus_snapshot();
        let mut validators = if nexus.enabled && proposal_lookahead_enabled(&nexus, proposal_height)
        {
            self.state
                .authoritative_lane_peer_ids_at_height(lane_id, proposal_height)
        } else {
            self.context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect()
        };
        validators.sort();
        validators.dedup();
        (!validators.is_empty()).then_some(validators)
    }

    fn lane_route_active(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        proposal_height: u64,
        lane_block_height: u64,
    ) -> bool {
        self.nexus_route_active(lane_id, dataspace_id, proposal_height)
            && self
                .state
                .da_shard_canonical_reset_heights_snapshot_cached()
                .get(&lane_id)
                .is_none_or(|reset| lane_block_height > *reset)
    }

    fn nexus_route_active(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        global_height: u64,
    ) -> bool {
        let nexus = self.state.nexus_snapshot();
        !nexus.enabled
            || crate::state::nexus_active_lane_dataspace_at_height(lane_id, &nexus, global_height)
                == Some(dataspace_id)
    }

    fn qc_mode_tag_matches_context(
        &self,
        tag: &str,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
    ) -> bool {
        let base = match self.context.mode {
            wire::ConsensusMode::Permissioned => wire::PERMISSIONED_TAG,
            wire::ConsensusMode::Npos => wire::NPOS_TAG,
        };
        let context_tag = format!(
            "{base}::height-context:{}::epoch:{}",
            hex::encode(self.context.id().0.as_ref()),
            self.context.epoch
        );
        tag == LaneRelayEnvelope::lane_qc_mode_tag_for(lane_id, dataspace_id, &context_tag)
    }

    fn hydrate_canonical_lane_artifacts(&mut self) {
        for artifact in self.kura.lane_block_artifacts_snapshot() {
            if self
                .kura
                .read_lane_block_application_receipt(
                    artifact.ownership.lane_id,
                    artifact.ownership.lane_block_height,
                )
                .is_some()
            {
                continue;
            }
            let Some(proposal) =
                proposal_from_ownership(&artifact.ownership, artifact.proposal_block_hash)
            else {
                continue;
            };
            let _ = self
                .lane_sessions
                .insert_recovered_proposal_replacing_uncommitted_conflict(proposal);
        }
    }

    fn canonical_anchor_for_proposal(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> Option<crate::kura::LaneBlockArtifact> {
        self.kura
            .read_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .filter(|artifact| {
                proposal_from_ownership(&artifact.ownership, artifact.proposal_block_hash).as_ref()
                    == Some(proposal)
            })
    }

    fn canonical_proposal_for_vote_body(
        &self,
        body: &iroha_data_model::block::consensus::LaneBlockVoteBodyV1,
    ) -> Option<LaneBlockProposalV1> {
        let artifact = self
            .kura
            .read_lane_block_artifact(body.lane_id, body.lane_block_height)?;
        let proposal = proposal_from_ownership(&artifact.ownership, artifact.proposal_block_hash)?;
        (proposal.vote_body(body.phase) == *body).then_some(proposal)
    }

    fn session_has_canonical_anchor(&self, session: &CommittedLaneBlockSession) -> bool {
        self.canonical_anchor_for_proposal(&session.proposal)
            .is_some()
    }

    fn pops_for_lane_qc(&self, qc: &LaneBlockQcV1) -> BTreeMap<PublicKey, Vec<u8>> {
        let world = self.state.world_view();
        qc.validator_set
            .iter()
            .filter_map(|peer| {
                crate::state::live_consensus_key_pop_for_peer(&world, peer, qc.body.proposal_height)
                    .map(|pop| (peer.public_key().clone(), pop))
            })
            .collect()
    }

    fn pops_for_lane_session(
        &self,
        session: &CommittedLaneBlockSession,
    ) -> BTreeMap<PublicKey, Vec<u8>> {
        let mut pops = self.pops_for_lane_qc(&session.prepare_qc);
        pops.extend(self.pops_for_lane_qc(&session.commit_qc));
        pops
    }

    fn accept_native_amx(
        &mut self,
        sender: PeerId,
        message: NativeAmxMessage,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        match message {
            NativeAmxMessage::PrepareRequest(body) => {
                self.accept_native_request(sender, body, None, active_view)
            }
            NativeAmxMessage::CommitRequest(request) => self.accept_native_request(
                sender,
                request.body,
                Some(request.prepare_qc),
                active_view,
            ),
            NativeAmxMessage::PrepareVote(vote) => {
                self.accept_native_vote(sender, vote, NativeAmxPhase::Prepare, active_view)
            }
            NativeAmxMessage::CommitVote(vote) => {
                self.accept_native_vote(sender, vote, NativeAmxPhase::Commit, active_view)
            }
        }
    }

    fn accept_native_request(
        &mut self,
        sender: PeerId,
        body: NativeAmxAttestationBodyV2,
        prepare_qc: Option<NativeAmxAttestationQcV2>,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let expected_leader = usize::try_from(self.context.leader(body.round.view))
            .ok()
            .and_then(|index| self.context.roster.get(index))
            .map(|entry| &entry.validator);
        if !self.native_body_matches_context(&body, active_view) || expected_leader != Some(&sender)
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let Some((validators, min_signers, pops)) = self.native_committee(&body) else {
            return V2LaneIngressOutcome::Rejected;
        };
        if !validators.contains(&self.local_peer) {
            return V2LaneIngressOutcome::Rejected;
        }
        match body.phase {
            NativeAmxPhase::Prepare if prepare_qc.is_some() => {
                return V2LaneIngressOutcome::Rejected;
            }
            NativeAmxPhase::Commit => {
                let Some(prepare_qc) = prepare_qc else {
                    return V2LaneIngressOutcome::Rejected;
                };
                let request = NativeAmxCommitRequestV2 {
                    body,
                    prepare_qc: prepare_qc.clone(),
                };
                if request.validate_shape().is_err()
                    || validate_native_amx_qc(
                        &prepare_qc,
                        &prepare_qc.body,
                        &validators,
                        min_signers,
                        &pops,
                    )
                    .is_err()
                {
                    return V2LaneIngressOutcome::Rejected;
                }
            }
            NativeAmxPhase::Prepare => {}
        }
        let Some(vote) = self.sign_native_vote_once(body) else {
            return V2LaneIngressOutcome::Rejected;
        };
        if !self.push_effect(V2LaneWorkEffect::PostNativeAmx {
            peer: sender,
            message: match body.phase {
                NativeAmxPhase::Prepare => NativeAmxMessage::PrepareVote(vote),
                NativeAmxPhase::Commit => NativeAmxMessage::CommitVote(vote),
            },
        }) {
            return V2LaneIngressOutcome::Rejected;
        }
        V2LaneIngressOutcome::Inserted
    }

    fn accept_native_vote(
        &mut self,
        sender: PeerId,
        vote: NativeAmxVoteV2,
        expected_phase: NativeAmxPhase,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        if !self.native_body_matches_context(&vote.body, active_view)
            || vote
                .validate_ingress(expected_phase, Some(&sender))
                .is_err()
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let Some((validators, _, _)) = self.native_committee(&vote.body) else {
            return V2LaneIngressOutcome::Rejected;
        };
        if !validators.contains(&vote.signer) {
            return V2LaneIngressOutcome::Rejected;
        }
        let key = NativeVoteClaimKey {
            session: NativeAmxSessionKey::from_body(&vote.body),
            round: vote.body.round,
            epoch: vote.body.epoch,
            participant_lane: vote.body.participant_lane_id,
            participant_dataspace: vote.body.participant_dataspace_id,
            phase: vote.body.phase,
            signer: HashOf::new(&vote.signer),
        };
        if let Some(existing) = self.native_claims.get(&key) {
            return if existing == &vote.body {
                V2LaneIngressOutcome::Duplicate
            } else {
                V2LaneIngressOutcome::Rejected
            };
        }
        let claim_capacity = self
            .limits
            .session_capacity
            .get()
            .saturating_mul(self.limits.body_buckets_per_session.get());
        while self.native_claims.len() >= claim_capacity {
            let Some(oldest) = self.native_claim_order.pop_front() else {
                return V2LaneIngressOutcome::Rejected;
            };
            self.native_claims.remove(&oldest);
        }
        let body = vote.body;
        match self.native_sessions.insert_vote(vote) {
            Ok(()) => {
                self.native_claims.insert(key, body);
                self.native_claim_order.push_back(key);
                V2LaneIngressOutcome::Inserted
            }
            Err(NativeAmxSessionError::DuplicateSigner) => V2LaneIngressOutcome::Duplicate,
            Err(NativeAmxSessionError::PhaseMismatch) => V2LaneIngressOutcome::Rejected,
        }
    }

    fn native_body_matches_context(
        &self,
        body: &NativeAmxAttestationBodyV2,
        active_view: wire::View,
    ) -> bool {
        body.round.context_id == self.context.id()
            && body.round.height == self.context.height
            && body.round.view <= active_view
            && body.epoch == self.context.epoch
            && self.native_coordinator_height_is_current(body)
            && self.nexus_route_active(
                body.coordinator_lane_id,
                body.coordinator_dataspace_id,
                self.context.height,
            )
            && self.nexus_route_active(
                body.participant_lane_id,
                body.participant_dataspace_id,
                self.context.height,
            )
    }

    fn native_coordinator_height_is_current(&self, body: &NativeAmxAttestationBodyV2) -> bool {
        let expected = self
            .kura
            .latest_lane_block_artifact_for_dataspace(
                body.coordinator_lane_id,
                body.coordinator_dataspace_id,
            )
            .map_or(1, |artifact| {
                artifact.ownership.lane_block_height.saturating_add(1)
            });
        body.planned_coordinator_block_height == expected
    }

    fn native_committee(
        &self,
        body: &NativeAmxAttestationBodyV2,
    ) -> Option<(Vec<PeerId>, usize, BTreeMap<PublicKey, Vec<u8>>)> {
        let authority_height = body.round.height;
        let mut validators = self
            .state
            .authoritative_lane_peer_ids_at_height(body.participant_lane_id, authority_height);
        validators.sort();
        validators.dedup();
        validators
            .retain(|peer| peer.public_key().try_algorithm().ok() == Some(Algorithm::BlsNormal));
        let nexus = self.state.nexus_snapshot();
        let fault_tolerance = nexus
            .dataspace_catalog
            .entries()
            .iter()
            .find(|entry| entry.id == body.participant_dataspace_id)?
            .fault_tolerance;
        let minimum_committee =
            usize::try_from(fault_tolerance.checked_mul(3)?.checked_add(1)?).ok()?;
        if validators.len() < minimum_committee
            || validators.len() > crate::native_amx::MAX_NATIVE_AMX_VALIDATORS
        {
            return None;
        }
        let world = self.state.world_view();
        let pops = validators
            .iter()
            .map(|peer| {
                let pop =
                    crate::state::live_consensus_key_pop_for_peer(&world, peer, authority_height)?;
                iroha_crypto::bls_normal_pop_verify(peer.public_key(), &pop).ok()?;
                Some((peer.public_key().clone(), pop))
            })
            .collect::<Option<BTreeMap<_, _>>>()?;
        let min_signers = super::network_topology::commit_quorum_from_len(validators.len()).max(1);
        Some((validators, min_signers, pops))
    }

    fn sign_native_vote_once(
        &mut self,
        body: NativeAmxAttestationBodyV2,
    ) -> Option<NativeAmxVoteV2> {
        if !self.voting_enabled
            || self.local_peer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
        {
            return None;
        }
        let claim = NativeVoteClaimKey {
            session: NativeAmxSessionKey::from_body(&body),
            round: body.round,
            epoch: body.epoch,
            participant_lane: body.participant_lane_id,
            participant_dataspace: body.participant_dataspace_id,
            phase: body.phase,
            signer: HashOf::new(&self.local_peer),
        };
        if let Some(existing) = self.local_native_claims.get(&claim) {
            if existing != &body {
                return None;
            }
        } else {
            let capacity = self
                .limits
                .session_capacity
                .get()
                .saturating_mul(self.limits.body_buckets_per_session.get());
            if self.local_native_claims.len() >= capacity {
                return None;
            }
        }
        let signature =
            Signature::try_new(self.key_pair.private_key(), &body.signature_preimage()).ok()?;
        self.local_native_claims.entry(claim).or_insert(body);
        Some(NativeAmxVoteV2 {
            body,
            signer: self.local_peer.clone(),
            bls_signature: signature.payload().to_vec(),
        })
    }

    fn prepare_native_receipt(
        &mut self,
        view: wire::View,
        candidate: CandidateDescriptor<'_>,
        coordinator_proposals: &[LaneBlockProposalV1],
    ) -> Option<NativeAmxReceipt> {
        let RoutingPlan::NativeAmx(plan) = candidate.routing_plan() else {
            return None;
        };
        let entrypoint_hash = Hash::from(candidate.entrypoint_hash());
        let mut matching_proposals = coordinator_proposals.iter().filter(|proposal| {
            let descriptor = &proposal.descriptor;
            descriptor.lane_id == plan.coordinator.route.lane_id
                && descriptor.dataspace_id == plan.coordinator.route.dataspace_id
                && descriptor.proposal_height == self.context.height
                && descriptor
                    .accepted_transaction_hashes
                    .iter()
                    .filter(|hash| **hash == entrypoint_hash)
                    .count()
                    == 1
        });
        let coordinator_proposal = matching_proposals.next()?;
        if matching_proposals.next().is_some()
            || crate::lane_consensus::validate_lane_block_proposal(coordinator_proposal).is_err()
        {
            return None;
        }
        let coordinator_descriptor = &coordinator_proposal.descriptor;
        let round = wire::ConsensusRound {
            context_id: self.context.id(),
            height: self.context.height,
            view,
        };
        let mut source_id = [0_u8; Hash::LENGTH];
        source_id.copy_from_slice(candidate.transaction().hash().as_ref());
        let session = NativeAmxSessionKey {
            source_id,
            plan_digest: plan.plan_digest,
        };
        let mut prepared = Vec::with_capacity(plan.participants.len());
        for participant in &plan.participants {
            let prepare_body = NativeAmxAttestationBodyV2 {
                round,
                epoch: self.context.epoch,
                source_id,
                tx_entrypoint_hash: candidate.entrypoint_hash(),
                plan_digest: plan.plan_digest,
                phase: NativeAmxPhase::Prepare,
                coordinator_lane_id: plan.coordinator.route.lane_id,
                coordinator_dataspace_id: plan.coordinator.route.dataspace_id,
                participant_lane_id: participant.route.lane_id,
                participant_dataspace_id: participant.route.dataspace_id,
                planned_coordinator_block_height: coordinator_descriptor.lane_block_height,
            };
            let Some((validators, min_signers, pops)) = self.native_committee(&prepare_body) else {
                return None;
            };
            self.ensure_native_prepare_requests(prepare_body, &validators);
            prepared.push((participant, prepare_body, validators, min_signers, pops));
        }

        let mut certified_prepares = Vec::with_capacity(prepared.len());
        for (participant, prepare_body, validators, min_signers, pops) in prepared {
            let prepare_votes = self.native_sessions.sorted_votes_for_body_from(
                session,
                &prepare_body,
                &validators,
            );
            if prepare_votes.len() < min_signers {
                return None;
            }
            let prepare_qc = aggregate_votes_to_qc(
                prepare_body,
                validators.clone(),
                &prepare_votes,
                min_signers,
            )
            .ok()?;
            if validate_native_amx_qc(&prepare_qc, &prepare_body, &validators, min_signers, &pops)
                .is_err()
            {
                return None;
            }
            self.retire_native_requests(&prepare_body);
            let commit_body = NativeAmxAttestationBodyV2 {
                phase: NativeAmxPhase::Commit,
                ..prepare_body
            };
            self.ensure_native_commit_requests(commit_body, &prepare_qc, &validators);
            certified_prepares.push((
                participant,
                commit_body,
                validators,
                min_signers,
                pops,
                prepare_qc,
            ));
        }

        let mut legs = Vec::with_capacity(certified_prepares.len());
        for (participant, commit_body, validators, min_signers, pops, prepare_qc) in
            certified_prepares
        {
            let commit_votes =
                self.native_sessions
                    .sorted_votes_for_body_from(session, &commit_body, &validators);
            if commit_votes.len() < min_signers {
                return None;
            }
            let commit_qc =
                aggregate_votes_to_qc(commit_body, validators.clone(), &commit_votes, min_signers)
                    .ok()?;
            if validate_native_amx_qc(&commit_qc, &commit_body, &validators, min_signers, &pops)
                .is_err()
            {
                return None;
            }
            self.retire_native_requests(&commit_body);
            legs.push(NativeAmxLegRecordV2 {
                lane_id: participant.route.lane_id,
                dataspace_id: participant.route.dataspace_id,
                prepare_qc,
                commit_qc,
            });
        }
        self.assemble_native_receipt(
            source_id,
            plan.coordinator.route,
            plan.plan_digest,
            coordinator_proposal,
            legs,
        )
    }

    fn assemble_native_receipt(
        &self,
        source_id: [u8; Hash::LENGTH],
        coordinator: RoutingDecision,
        plan_digest: Hash,
        coordinator_proposal: &LaneBlockProposalV1,
        legs: Vec<NativeAmxLegRecordV2>,
    ) -> Option<NativeAmxReceipt> {
        let descriptor = &coordinator_proposal.descriptor;
        if crate::lane_consensus::validate_lane_block_proposal(coordinator_proposal).is_err()
            || descriptor.lane_id != coordinator.lane_id
            || descriptor.dataspace_id != coordinator.dataspace_id
            || descriptor.proposal_height != self.context.height
            || descriptor.lane_block_height == 0
            || descriptor
                .lane_incarnation
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || coordinator_proposal.proposal_hash != coordinator_proposal.computed_proposal_hash()
        {
            return None;
        }
        let chain_id = self.context.chain_id.clone().into_inner();
        Some(NativeAmxReceipt {
            version: 2,
            source_id,
            chain_id_hash: Hash::new(chain_id.as_bytes()),
            plan_digest,
            lane_id: coordinator.lane_id,
            dataspace_id: coordinator.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            authority_context_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            coordinator_proposal_hash: coordinator_proposal.proposal_hash,
            legs,
        })
    }

    fn ensure_native_prepare_requests(
        &mut self,
        body: NativeAmxAttestationBodyV2,
        validators: &[PeerId],
    ) {
        for peer in validators {
            if peer == &self.local_peer {
                if self
                    .native_sessions
                    .sorted_votes_for_body(NativeAmxSessionKey::from_body(&body), &body)
                    .iter()
                    .all(|vote| vote.signer != self.local_peer)
                    && let Some(vote) = self.sign_native_vote_once(body)
                {
                    let _ = self.native_sessions.insert_vote(vote);
                }
                continue;
            }
            self.register_native_request(
                body,
                peer.clone(),
                NativeAmxMessage::PrepareRequest(body),
            );
        }
    }

    fn ensure_native_commit_requests(
        &mut self,
        body: NativeAmxAttestationBodyV2,
        prepare_qc: &NativeAmxAttestationQcV2,
        validators: &[PeerId],
    ) {
        for peer in validators {
            if peer == &self.local_peer {
                if self
                    .native_sessions
                    .sorted_votes_for_body(NativeAmxSessionKey::from_body(&body), &body)
                    .iter()
                    .all(|vote| vote.signer != self.local_peer)
                    && let Some(vote) = self.sign_native_vote_once(body)
                {
                    let _ = self.native_sessions.insert_vote(vote);
                }
                continue;
            }
            self.register_native_request(
                body,
                peer.clone(),
                NativeAmxMessage::CommitRequest(NativeAmxCommitRequestV2 {
                    body,
                    prepare_qc: prepare_qc.clone(),
                }),
            );
        }
    }

    fn register_native_request(
        &mut self,
        body: NativeAmxAttestationBodyV2,
        peer: PeerId,
        message: NativeAmxMessage,
    ) {
        let key = NativeRequestKey {
            body,
            peer: peer.clone(),
        };
        let mut inserted = false;
        if !self.native_requests.contains_key(&key)
            && self.native_requests.len() < self.limits.native_request_capacity.get()
        {
            self.native_requests.insert(key.clone(), message.clone());
            inserted = true;
        }
        if inserted {
            if self.push_effect(V2LaneWorkEffect::PostNativeAmx { peer, message }) {
                self.native_retransmit_cursor = self.native_retransmit_cursor.saturating_add(1);
            }
        }
    }

    fn retire_native_requests(&mut self, body: &NativeAmxAttestationBodyV2) {
        self.native_requests.retain(|key, _| &key.body != body);
    }

    fn refresh_merge_candidates(&mut self) {
        let candidates = self.state.merge_entry_candidates_from_lane_relays();
        for candidate in candidates {
            let digest = crate::merge::merge_qc_message_digest(
                &self.context.chain_id,
                &candidate,
                VALIDATOR_SET_HASH_VERSION_V1,
                self.frozen_validator_set_hash(),
            );
            let key = MergeKey {
                epoch_id: candidate.epoch_id,
                view: candidate.view,
                digest,
            };
            if !self.merge_entries.contains_key(&key)
                && self.merge_entries.len() >= self.limits.merge_capacity.get()
            {
                continue;
            }
            self.merge_entries.entry(key).or_insert(PendingMerge {
                candidate: candidate.clone(),
                signatures: BTreeMap::new(),
            });
            let Some(local_index) = self.local_validator_index() else {
                continue;
            };
            if self.merge_entries[&key]
                .signatures
                .contains_key(&local_index)
            {
                continue;
            }
            let Ok(signature) = Signature::try_new(self.key_pair.private_key(), digest.as_ref())
            else {
                continue;
            };
            let payload = signature.payload().to_vec();
            self.merge_entries
                .get_mut(&key)
                .expect("entry inserted above")
                .signatures
                .insert(local_index, payload.clone());
            self.merge_claims
                .insert((key.epoch_id, key.view, local_index), digest);
            self.push_effect(V2LaneWorkEffect::BroadcastMerge(MergeCommitteeSignature {
                epoch_id: key.epoch_id,
                view: key.view,
                signer: local_index,
                message_digest: digest,
                bls_sig: payload,
            }));
            self.try_commit_merge(key);
        }
    }

    fn accept_merge_signature(
        &mut self,
        signature: MergeCommitteeSignature,
    ) -> V2LaneIngressOutcome {
        self.refresh_merge_candidates();
        let key = MergeKey {
            epoch_id: signature.epoch_id,
            view: signature.view,
            digest: signature.message_digest,
        };
        let Some(pending) = self.merge_entries.get(&key) else {
            return V2LaneIngressOutcome::Rejected;
        };
        if crate::merge::merge_qc_message_digest(
            &self.context.chain_id,
            &pending.candidate,
            VALIDATOR_SET_HASH_VERSION_V1,
            self.frozen_validator_set_hash(),
        ) != signature.message_digest
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let Some(peer) = self
            .context
            .roster
            .get(usize::try_from(signature.signer).unwrap_or(usize::MAX))
            .map(|entry| &entry.validator)
        else {
            return V2LaneIngressOutcome::Rejected;
        };
        let Ok(parsed) = Signature::try_from_bytes(&signature.bls_sig) else {
            return V2LaneIngressOutcome::Rejected;
        };
        if parsed
            .verify(peer.public_key(), signature.message_digest.as_ref())
            .is_err()
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let claim_key = (signature.epoch_id, signature.view, signature.signer);
        if let Some(existing) = self.merge_claims.get(&claim_key) {
            if existing != &signature.message_digest {
                return V2LaneIngressOutcome::Rejected;
            }
            return if self.merge_entries[&key].signatures.get(&signature.signer)
                == Some(&signature.bls_sig)
            {
                V2LaneIngressOutcome::Duplicate
            } else {
                V2LaneIngressOutcome::Rejected
            };
        }
        self.merge_claims
            .insert(claim_key, signature.message_digest);
        self.merge_entries
            .get_mut(&key)
            .expect("pending entry checked above")
            .signatures
            .insert(signature.signer, signature.bls_sig);
        self.try_commit_merge(key);
        V2LaneIngressOutcome::Inserted
    }

    fn try_commit_merge(&mut self, key: MergeKey) {
        let Some(pending) = self.merge_entries.get(&key) else {
            return;
        };
        let validator_set = self.frozen_validator_set();
        let validator_set_hash = HashOf::new(&validator_set);
        if crate::merge::merge_qc_message_digest(
            &self.context.chain_id,
            &pending.candidate,
            VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash,
        ) != key.digest
        {
            return;
        }
        let signers = pending.signatures.keys().copied().collect::<Vec<_>>();
        if !self.frozen_dual_quorum_met(&signers) {
            return;
        }
        let signatures = signers
            .iter()
            .filter_map(|signer| pending.signatures.get(signer))
            .collect::<Vec<_>>();
        let refs = signatures
            .iter()
            .map(|signature| signature.as_slice())
            .collect::<Vec<_>>();
        let Ok(aggregate_signature) = iroha_crypto::bls_normal_aggregate_signatures(&refs) else {
            return;
        };
        let mut bitmap = vec![0_u8; self.context.roster.len().div_ceil(8)];
        for signer in &signers {
            let Ok(index) = usize::try_from(*signer) else {
                return;
            };
            bitmap[index / 8] |= 1_u8 << (index % 8);
        }
        let signer_proofs = {
            let world = self.state.world_view();
            let mut proofs = Vec::with_capacity(signers.len());
            for signer in &signers {
                let Ok(index) = usize::try_from(*signer) else {
                    return;
                };
                let Some(peer) = validator_set.get(index) else {
                    return;
                };
                let Some(proof_of_possession) =
                    crate::state::consensus_key_pop_for_public_key(&world, peer.public_key())
                else {
                    return;
                };
                proofs.push(MergeSignerProof {
                    signer: *signer,
                    proof_of_possession,
                });
            }
            proofs
        };
        let qc = MergeQuorumCertificate::new(
            key.view,
            key.epoch_id,
            pending.candidate.carrier_height,
            pending.candidate.carrier_parent_hash,
            crate::merge::merge_chain_id_digest(&self.context.chain_id),
            VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash,
            validator_set,
            bitmap,
            signer_proofs,
            aggregate_signature,
            key.digest,
        );
        let candidate = pending.candidate.clone();
        if self
            .state
            .commit_merge_entry(candidate.into_entry(qc))
            .is_ok()
        {
            self.merge_entries.remove(&key);
            self.merge_claims
                .retain(|(epoch, view, _), _| *epoch != key.epoch_id || *view != key.view);
        }
    }

    fn frozen_dual_quorum_met(&self, signers: &[wire::ValidatorIndex]) -> bool {
        let distinct = signers.iter().copied().collect::<BTreeSet<_>>();
        if distinct.len() < usize::try_from(self.context.quorum.min_signers).unwrap_or(usize::MAX) {
            return false;
        }
        let power = distinct.iter().try_fold(0_u64, |total, signer| {
            let entry = self.context.roster.get(usize::try_from(*signer).ok()?)?;
            total.checked_add(entry.power)
        });
        power.is_some_and(|power| {
            u128::from(power) * 3 > u128::from(self.context.quorum.total_power) * 2
        })
    }

    fn local_validator_index(&self) -> Option<wire::ValidatorIndex> {
        if !self.voting_enabled {
            return None;
        }
        self.context
            .roster
            .iter()
            .position(|entry| entry.validator == self.local_peer)
            .and_then(|index| u32::try_from(index).ok())
    }

    fn frozen_validator_set(&self) -> Vec<PeerId> {
        self.context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect()
    }

    fn frozen_validator_set_hash(&self) -> HashOf<Vec<PeerId>> {
        HashOf::new(&self.frozen_validator_set())
    }
}

impl CandidateWorkProvider for &mut V2LaneWorkAdapter {
    fn prepare(
        &mut self,
        context: &wire::HeightContext,
        view: wire::View,
        candidates: &[CandidateDescriptor<'_>],
    ) -> Result<PreparedCandidateWork, CandidateWorkUnavailable> {
        if context != &self.context {
            return Err(all_unavailable(candidates.len(), "height context drift"));
        }
        self.planned_lane_proposals.clear();
        let routes = candidates
            .iter()
            .map(|candidate| candidate.routing_plan().coordinator_route())
            .collect::<Vec<_>>();
        let hashes = candidates
            .iter()
            .map(|candidate| Hash::from(candidate.entrypoint_hash()))
            .collect::<Vec<_>>();
        let lane_plan = prepare_v2_lane_payload_plan(
            self.state.as_ref(),
            context,
            view,
            &self.local_peer,
            &routes,
            &hashes,
        )
        .map_err(|error| all_unavailable(candidates.len(), error.to_string()))?;
        if !lane_plan.unavailable_indices.is_empty() {
            return Err(CandidateWorkUnavailable::new(
                lane_plan.unavailable_indices,
                "lane-local author, committee, or predecessor unavailable",
            ));
        }
        if lane_plan.proposals.len() > self.limits.session_capacity.get() {
            return Err(all_unavailable(
                candidates.len(),
                "lane-local proposal count exceeds the bounded session capacity",
            ));
        }

        let mut receipts = Vec::with_capacity(candidates.len());
        let mut unavailable = BTreeSet::new();
        for (index, candidate) in candidates.iter().copied().enumerate() {
            match candidate.routing_plan() {
                RoutingPlan::Single(_) => receipts.push(None),
                RoutingPlan::NativeAmx(_) => {
                    match self.prepare_native_receipt(view, candidate, &lane_plan.proposals) {
                        Some(receipt) => receipts.push(Some(receipt)),
                        None => {
                            receipts.push(None);
                            unavailable.insert(index);
                        }
                    }
                }
            }
        }
        if !unavailable.is_empty() {
            return Err(CandidateWorkUnavailable::new(
                unavailable,
                "context-bound Native AMX prepare/commit certificates unavailable",
            ));
        }
        self.planned_lane_proposals.insert(
            wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view,
            },
            lane_plan.proposals,
        );
        Ok(PreparedCandidateWork {
            native_amx_receipts: receipts,
            lane_payload_ownerships: lane_plan.ownerships,
        })
    }
}

fn all_unavailable(count: usize, reason: impl Into<String>) -> CandidateWorkUnavailable {
    CandidateWorkUnavailable::new((0..count).collect(), reason)
}

fn lane_work_effect_key(effect: &V2LaneWorkEffect) -> Hash {
    let mut encoded = Vec::new();
    match effect {
        V2LaneWorkEffect::PostLaneBlock { peer, message } => {
            encoded.push(0);
            encoded.extend(peer.encode());
            encoded.extend(message.encode());
        }
        V2LaneWorkEffect::PostNativeAmx { peer, message } => {
            encoded.push(1);
            encoded.extend(peer.encode());
            encoded.extend(message.encode());
        }
        V2LaneWorkEffect::BroadcastMerge(signature) => {
            encoded.push(2);
            encoded.extend(signature.encode());
        }
    }
    Hash::new(encoded)
}

fn lane_proposal_author(proposal: &LaneBlockProposalV1) -> Option<&PeerId> {
    let count = u64::try_from(proposal.descriptor.validator_set.len()).ok()?;
    if count == 0 {
        return None;
    }
    let index = proposal.descriptor.lane_block_height.saturating_sub(1) % count;
    proposal
        .descriptor
        .validator_set
        .get(usize::try_from(index).ok()?)
}

fn proposal_from_ownership(
    ownership: &SumeragiLanePayloadOwnership,
    block_hash: HashOf<BlockHeader>,
) -> Option<LaneBlockProposalV1> {
    let descriptor_hash = ownership.lane_block_descriptor_hash?;
    let descriptor = LaneBlockDescriptorV1 {
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
    if descriptor.computed_descriptor_hash() != descriptor_hash {
        return None;
    }
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: Some(LaneBlockProposalPayloadHintV1 {
            proposal_height: ownership.proposal_height,
            proposal_view: ownership.proposal_view,
            proposal_block_hash: block_hash,
        }),
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    Some(proposal)
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroUsize, sync::Arc};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{
        ChainId,
        block::{
            consensus::{NativeAmxAttestationBodyV2, NativeAmxPhase},
            consensus_v2 as wire,
        },
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
        transaction::TransactionEntrypoint,
    };

    use super::*;
    use crate::{query::store::LiveQueryStore, state::World};

    fn fixture(mode: wire::ConsensusMode) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
        let chain_id: ChainId = "v2-lane-work-test".into();
        let kura = Kura::blank_kura_for_testing();
        let state = Arc::new(State::new_with_chain_for_testing(
            World::new(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            chain_id.clone(),
        ));
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let powers = match mode {
            wire::ConsensusMode::Permissioned => [1, 1, 1, 1],
            wire::ConsensusMode::Npos => [4, 3, 2, 1],
        };
        let roster = keys
            .iter()
            .zip(powers)
            .map(|(key, power)| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            chain_id,
            protocol_version: wire::PROTOCOL_VERSION,
            height: 9,
            epoch: 4,
            epoch_end_height: 20,
            mode,
            parent_commit_qc: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("dual quorum"),
            roster,
            nexus_amx_context_hash: super::super::v2_recovery::committed_nexus_amx_context_hash(
                state.as_ref(),
            ),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 1024,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 4096,
                max_chunk_count: 4,
            },
            leader_seed: [0x42; 32],
        };
        let local_index = usize::try_from(context.leader(0)).expect("leader index");
        let local_key = keys[local_index].clone();
        let local_peer = PeerId::new(local_key.public_key().clone());
        let nonzero = NonZeroUsize::new(8).expect("nonzero");
        let adapter = V2LaneWorkAdapter::new(
            context,
            local_peer,
            local_key,
            true,
            state,
            kura,
            V2LaneWorkLimits::new(nonzero, nonzero, nonzero, nonzero, nonzero, nonzero),
        )
        .expect("open lane adapter");
        (adapter, keys)
    }

    fn native_body(adapter: &V2LaneWorkAdapter) -> NativeAmxAttestationBodyV2 {
        NativeAmxAttestationBodyV2 {
            round: wire::ConsensusRound {
                context_id: adapter.context.id(),
                height: adapter.context.height,
                view: 0,
            },
            epoch: adapter.context.epoch,
            source_id: [0xA5; Hash::LENGTH],
            tx_entrypoint_hash: HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
                b"entrypoint",
            )),
            plan_digest: Hash::new(b"plan"),
            phase: NativeAmxPhase::Prepare,
            coordinator_lane_id: LaneId::new(1),
            coordinator_dataspace_id: DataSpaceId::new(7),
            participant_lane_id: LaneId::new(2),
            participant_dataspace_id: DataSpaceId::new(8),
            planned_coordinator_block_height: 1,
        }
    }

    fn coordinator_proposal(adapter: &V2LaneWorkAdapter, keys: &[KeyPair]) -> LaneBlockProposalV1 {
        let validator_set = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(7),
            lane_incarnation: Hash::new(b"v2-lane-work-coordinator-incarnation"),
            proposal_height: adapter.context.height,
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_height: 1,
            lane_block_view: 0,
            subject_hash: Hash::new(b"v2-lane-work-subject"),
            payload_ownership_hash: Hash::new(b"v2-lane-work-ownership"),
            rbc_instance_hash: Hash::new(b"v2-lane-work-rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::new(b"entrypoint")],
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_count: u32::try_from(validator_set.len()).expect("fixture validator count"),
            min_quorum: u32::try_from(
                crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len())
                    .max(1),
            )
            .expect("fixture quorum"),
            validator_set,
            qc_mode_tag: "permissioned:v2-lane-work".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    #[test]
    fn native_amx_context_guard_rejects_replayed_round_epoch_and_future_view() {
        let (adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let body = native_body(&adapter);
        assert!(adapter.native_body_matches_context(&body, 0));

        let mut wrong_context = body;
        wrong_context.round.context_id =
            wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(b"other-context")));
        assert!(!adapter.native_body_matches_context(&wrong_context, 0));

        let mut wrong_epoch = body;
        wrong_epoch.epoch = wrong_epoch.epoch.saturating_add(1);
        assert!(!adapter.native_body_matches_context(&wrong_epoch, 0));

        let mut future_view = body;
        future_view.round.view = 1;
        assert!(!adapter.native_body_matches_context(&future_view, 0));
        assert!(adapter.native_body_matches_context(&future_view, 1));

        let mut wrong_lane_height = body;
        wrong_lane_height.planned_coordinator_block_height = 2;
        assert!(!adapter.native_body_matches_context(&wrong_lane_height, 0));
    }

    #[test]
    fn full_native_amx_receipt_metadata_is_derived_from_frozen_context_and_proposal() {
        let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let proposal = coordinator_proposal(&adapter, &keys);
        let coordinator = RoutingDecision::new(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
        );
        let source_id = [0x5A; Hash::LENGTH];
        let plan_digest = Hash::new(b"full-native-amx-plan");
        let receipt = adapter
            .assemble_native_receipt(source_id, coordinator, plan_digest, &proposal, Vec::new())
            .expect("canonical coordinator proposal builds a full receipt");
        let chain_id = adapter.context.chain_id.clone().into_inner();

        assert_eq!(receipt.version, 2);
        assert_eq!(receipt.source_id, source_id);
        assert_eq!(receipt.chain_id_hash, Hash::new(chain_id.as_bytes()));
        assert_eq!(receipt.plan_digest, plan_digest);
        assert_eq!(receipt.lane_id, proposal.descriptor.lane_id);
        assert_eq!(receipt.dataspace_id, proposal.descriptor.dataspace_id);
        assert_eq!(
            receipt.lane_incarnation,
            proposal.descriptor.lane_incarnation
        );
        assert_eq!(
            receipt.authority_context_height,
            proposal.descriptor.proposal_height
        );
        assert_eq!(
            receipt.lane_block_height,
            proposal.descriptor.lane_block_height
        );
        assert_eq!(receipt.lane_block_view, proposal.descriptor.lane_block_view);
        assert_eq!(receipt.coordinator_proposal_hash, proposal.proposal_hash);

        let mut wrong_height = proposal;
        wrong_height.descriptor.proposal_height = adapter.context.height.saturating_add(1);
        wrong_height.descriptor.descriptor_hash =
            wrong_height.descriptor.computed_descriptor_hash();
        wrong_height.proposal_hash = wrong_height.computed_proposal_hash();
        assert!(
            adapter
                .assemble_native_receipt(
                    source_id,
                    coordinator,
                    plan_digest,
                    &wrong_height,
                    Vec::new(),
                )
                .is_none(),
            "receipt assembly must reject a proposal outside the frozen authority height"
        );
    }

    #[test]
    fn observer_role_cannot_sign_lane_merge_or_native_amx_votes() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        adapter.voting_enabled = false;

        assert_eq!(adapter.local_validator_index(), None);
        assert!(
            adapter
                .sign_native_vote_once(native_body(&adapter))
                .is_none()
        );
        assert!(adapter.local_native_claims.is_empty());
    }

    #[test]
    fn local_native_amx_signer_rejects_conflicting_claim_for_one_leg_phase() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let body = native_body(&adapter);
        let first = adapter
            .sign_native_vote_once(body)
            .expect("first exact body may be signed");
        let retransmission = adapter
            .sign_native_vote_once(body)
            .expect("an exact retransmission is idempotently signable");
        assert_eq!(first, retransmission);

        let mut conflicting = body;
        conflicting.tx_entrypoint_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"conflicting entrypoint"));
        assert!(
            adapter.sign_native_vote_once(conflicting).is_none(),
            "an honest adapter must not sign a second body for one round/session/leg/phase"
        );
        assert_eq!(adapter.local_native_claims.len(), 1);

        let commit = NativeAmxAttestationBodyV2 {
            phase: NativeAmxPhase::Commit,
            ..body
        };
        assert!(
            adapter.sign_native_vote_once(commit).is_some(),
            "Prepare and Commit are distinct durable claims"
        );
    }

    #[test]
    fn effect_queue_is_bounded_and_deduplicates_until_drain() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let message = NativeAmxMessage::PrepareRequest(native_body(&adapter));
        let effect = V2LaneWorkEffect::PostNativeAmx {
            peer: adapter.local_peer.clone(),
            message,
        };
        assert!(adapter.push_effect(effect.clone()));
        assert!(adapter.push_effect(effect.clone()));
        assert_eq!(adapter.effects.len(), 1);
        assert_eq!(adapter.drain_effects(1).len(), 1);
        assert!(adapter.push_effect(effect));
        assert_eq!(adapter.effects.len(), 1);
    }

    #[test]
    fn merge_signers_must_meet_both_count_and_power_quorums() {
        let (adapter, _) = fixture(wire::ConsensusMode::Npos);
        assert!(!adapter.frozen_dual_quorum_met(&[0, 1]));
        assert!(!adapter.frozen_dual_quorum_met(&[1, 2, 3]));
        assert!(adapter.frozen_dual_quorum_met(&[0, 1, 3]));
        assert!(adapter.frozen_dual_quorum_met(&[0, 1, 2, 3]));
    }
}
