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
        BlockHeader, SignedBlock,
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
    /// Committed State is neither immediately before nor exactly at this context height.
    #[error("committed State height is incompatible with the frozen height context")]
    StateHeightMismatch,
    /// Interrupted post-application recovery token does not match both durable tips.
    #[error("recovered Sumeragi v2 applied tip does not match State and Kura")]
    RecoveredAppliedTipMismatch,
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
    globally_locked_body_hash: Option<HashOf<BlockHeader>>,
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
    /// committed-state/context drift. `recovered_applied_height` is accepted
    /// only when it identifies this exact context and canonical post-apply tip.
    pub(crate) fn new(
        context: wire::HeightContext,
        local_peer: PeerId,
        key_pair: KeyPair,
        voting_enabled: bool,
        state: Arc<State>,
        kura: Arc<Kura>,
        limits: V2LaneWorkLimits,
        recovered_applied_height: Option<super::v2_recovery::PendingKuraApply>,
    ) -> Result<Self, V2LaneWorkError> {
        context
            .validate()
            .map_err(|error| V2LaneWorkError::InvalidContext(error.to_string()))?;
        if local_peer.public_key() != key_pair.public_key() {
            return Err(V2LaneWorkError::LocalKeyMismatch);
        }
        let committed_context_matches =
            super::v2_recovery::committed_nexus_amx_context_hash(state.as_ref())
                == context.nexus_amx_context_hash;
        let state_height = u64::try_from(state.committed_height())
            .map_err(|_| V2LaneWorkError::StateHeightMismatch)?;
        let is_pre_apply = state_height.checked_add(1) == Some(context.height);
        let is_post_apply = state_height == context.height;
        if !is_pre_apply && !is_post_apply {
            return Err(V2LaneWorkError::StateHeightMismatch);
        }
        let recovered_applied_tip_matches = recovered_applied_height.is_some_and(|pending| {
            let Ok(height) = usize::try_from(context.height) else {
                return false;
            };
            let Some(nonzero_height) = NonZeroUsize::new(height) else {
                return false;
            };
            pending.context_id() == context.id()
                && pending.height() == context.height
                && state.committed_height() == height
                && state.latest_block_hash_fast() == Some(pending.block_hash())
                && kura.durable_blocks_count() == height
                && kura.get_durable_block_hash(nonzero_height) == Some(pending.block_hash())
        });
        if (is_post_apply || recovered_applied_height.is_some()) && !recovered_applied_tip_matches {
            return Err(V2LaneWorkError::RecoveredAppliedTipMismatch);
        }
        if is_pre_apply && !committed_context_matches {
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
            globally_locked_body_hash: None,
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
        adapter.repair_globally_applied_lane_receipts()?;
        adapter.hydrate_canonical_lane_artifacts();
        adapter.refresh_merge_candidates(0);
        adapter.drive_lane_sessions();
        Ok(adapter)
    }

    fn repair_globally_applied_lane_receipts(&self) -> Result<usize, V2LaneWorkError> {
        let pending = self
            .state
            .unapplied_lane_block_artifact_heights_snapshot_cached();
        let mut repaired = 0_usize;
        for ((lane_id, dataspace_id), lane_block_height) in
            pending.into_iter().take(self.limits.session_capacity.get())
        {
            let Some(artifact) = self
                .kura
                .read_lane_block_artifact(lane_id, lane_block_height)
            else {
                continue;
            };
            if artifact.ownership.dataspace_id != dataspace_id {
                continue;
            }
            let Some(proposal) =
                proposal_from_ownership(&artifact.ownership, artifact.proposal_block_hash)
            else {
                continue;
            };
            let Some(certified) = self
                .kura
                .read_certified_lane_block_artifact(lane_id, lane_block_height)
            else {
                continue;
            };
            if certified.proposal != proposal {
                continue;
            }
            if !self.proposal_anchor_is_committed_in_state(&proposal) {
                if proposal.descriptor.proposal_height
                    > u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX)
                {
                    continue;
                }
                return Err(V2LaneWorkError::Persistence(
                    "certified lane block anchor conflicts with committed State".to_owned(),
                ));
            }
            if !self
                .state
                .certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached(&proposal)
            {
                return Err(V2LaneWorkError::Persistence(
                    "certified lane block has no applied predecessor during recovery".to_owned(),
                ));
            }
            let persisted = self
                .kura
                .persist_lane_block_application_receipt_if_ready(&proposal)
                .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
            if !persisted {
                return Err(V2LaneWorkError::Persistence(
                    "certified globally applied lane block has no canonical results".to_owned(),
                ));
            }
            repaired = repaired.saturating_add(1);
        }
        Ok(repaired)
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

    /// Record the one global subject protected by the reducer's durable
    /// PrepareQC lock. The exact durable body must still be bound with
    /// [`Self::bind_locked_global_body`] before any lane proposal becomes
    /// signable.
    #[must_use]
    pub(crate) fn mark_global_body_locked(&mut self, block_hash: HashOf<BlockHeader>) -> bool {
        if self.globally_locked_body_hash.is_some() {
            return false;
        }
        self.globally_locked_body_hash = Some(block_hash);
        self.locally_bound_lane_proposals.clear();
        true
    }

    /// Bind lane proposals reconstructed from the exact durable globally
    /// locked body, then release their bounded lane-local consensus sessions.
    pub(crate) fn bind_locked_global_body(&mut self, block: &SignedBlock) -> V2LaneIngressOutcome {
        let block_hash = block.hash();
        if self.globally_locked_body_hash != Some(block_hash)
            || block.header().height().get() != self.context.height
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let bundle = block.execution_context();
        let ownerships = bundle.map_or(&[][..], |bundle| bundle.lane_payload_ownerships.as_slice());
        if ownerships.len() > self.limits.session_capacity.get() {
            return V2LaneIngressOutcome::Rejected;
        }
        let routes = bundle
            .into_iter()
            .flat_map(|bundle| &bundle.external)
            .map(|entry| RoutingDecision::new(entry.lane_id, entry.dataspace_id))
            .collect::<Vec<_>>();
        let hashes = bundle
            .into_iter()
            .flat_map(|bundle| &bundle.external)
            .map(|entry| Hash::from(entry.entrypoint_hash))
            .collect::<Vec<_>>();
        let global_view = block.header().view_change_index();
        let Some(global_leader) = usize::try_from(self.context.leader(global_view))
            .ok()
            .and_then(|index| self.context.roster.get(index))
            .map(|entry| &entry.validator)
        else {
            return V2LaneIngressOutcome::Rejected;
        };
        let canonical_recovery = canonical_v2_lane_payload_matches_kura(
            self.state.as_ref(),
            self.kura.as_ref(),
            &self.context,
            block,
        );
        if !canonical_recovery {
            let Ok(expected) = prepare_v2_lane_payload_plan(
                self.state.as_ref(),
                &self.context,
                global_view,
                global_leader,
                &routes,
                &hashes,
            ) else {
                return V2LaneIngressOutcome::Rejected;
            };
            if !expected.unavailable_indices.is_empty() || expected.ownerships != ownerships {
                return V2LaneIngressOutcome::Rejected;
            }
        }
        let mut proposals = Vec::with_capacity(ownerships.len());
        for ownership in ownerships {
            let Some(proposal) = proposal_from_ownership(ownership, block_hash) else {
                return V2LaneIngressOutcome::Rejected;
            };
            let descriptor = &proposal.descriptor;
            if descriptor.proposal_height != self.context.height
                || ownership.proposal_view != global_view
                || descriptor.lane_block_view != global_view
                || !self.qc_mode_tag_matches_context(
                    &descriptor.qc_mode_tag,
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                )
                || !self.lane_route_active(
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                    descriptor.lane_incarnation,
                    descriptor.proposal_height,
                )
                || self.expected_lane_validators(descriptor.lane_id, descriptor.proposal_height)
                    != Some(descriptor.validator_set.clone())
                || self.expected_lane_author(&proposal) != Some(global_leader)
            {
                return V2LaneIngressOutcome::Rejected;
            }
            proposals.push(proposal);
        }

        let local = self.pending_local_lane_proposals.remove(&block_hash);
        self.pending_local_lane_proposals.clear();
        if local.as_ref().is_some_and(|planned| planned != &proposals) {
            return V2LaneIngressOutcome::Rejected;
        }
        let mut next_sessions = self.lane_sessions.clone();
        let mut inserted = false;
        for proposal in &proposals {
            match next_sessions.insert_proposal(proposal.clone()) {
                Ok(LaneBlockSessionInsertOutcome::Inserted) => inserted = true,
                Ok(LaneBlockSessionInsertOutcome::Duplicate) => {}
                Err(_) => return V2LaneIngressOutcome::Rejected,
            }
        }
        self.lane_sessions = next_sessions;
        self.locally_bound_lane_proposals = proposals
            .iter()
            .map(|proposal| proposal.proposal_hash)
            .collect();
        for proposal in local.into_iter().flatten() {
            self.fanout_lane_message(
                BlockMessage::LaneBlockProposal(proposal.clone()),
                &proposal.descriptor.validator_set,
            );
        }
        self.drive_lane_sessions();
        if inserted {
            V2LaneIngressOutcome::Inserted
        } else {
            V2LaneIngressOutcome::Duplicate
        }
    }

    /// Persist only completed lane sessions anchored by canonical Kura blocks.
    ///
    /// # Errors
    ///
    /// Returns [`V2LaneWorkError::Persistence`] if an anchored certificate or
    /// its canonical globally-applied receipt cannot be written durably.
    pub(crate) fn persist_anchored_sessions(&mut self) -> Result<usize, V2LaneWorkError> {
        self.collect_committed_lane_sessions();
        let mut sessions = self
            .pending_committed_lanes
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by_key(|session| {
            let descriptor = &session.proposal.descriptor;
            (
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.lane_block_height,
                descriptor.lane_block_view,
            )
        });
        let mut retained = VecDeque::new();
        let mut persisted = 0usize;
        for session in sessions {
            if !self.session_has_canonical_anchor(&session) {
                retained.push_back(session);
                continue;
            }
            if !self.proposal_anchor_is_committed_in_state(&session.proposal) {
                return Err(V2LaneWorkError::Persistence(
                    "lane certificate anchor is not committed in State".to_owned(),
                ));
            }
            let pops = self.pops_for_lane_session(&session);
            self.kura
                .persist_committed_lane_block_session(&session, &pops)
                .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
            if self
                .state
                .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(&session)
            {
                persisted = persisted.saturating_add(1);
                continue;
            }
            if !self
                .state
                .certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached(
                    &session.proposal,
                )
            {
                return Err(V2LaneWorkError::Persistence(
                    "globally applied lane block has no applied predecessor".to_owned(),
                ));
            }
            let receipt_persisted = self
                .kura
                .persist_lane_block_application_receipt_if_ready(&session.proposal)
                .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
            if !receipt_persisted {
                return Err(V2LaneWorkError::Persistence(
                    "globally applied lane block has no recoverable canonical results".to_owned(),
                ));
            }
            persisted = persisted.saturating_add(1);
        }
        self.pending_committed_lanes = retained;
        Ok(persisted)
    }

    fn proposal_anchor_is_committed_in_state(&self, proposal: &LaneBlockProposalV1) -> bool {
        let Some(hint) = proposal.payload_block_hint else {
            return false;
        };
        hint.proposal_height == proposal.descriptor.proposal_height
            && hint.proposal_view == proposal.descriptor.lane_block_view
            && self
                .state
                .committed_block_hash_at_height(hint.proposal_height)
                == Some(hint.proposal_block_hash)
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
            LaneRelayMessage::Envelope(envelope) => self.accept_lane_relay(envelope, active_view),
            LaneRelayMessage::MergeSignature(signature) => {
                self.accept_merge_signature(signature, active_view)
            }
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
            if !self.proposal_body_available(&proposal) {
                continue;
            }
            lane_artifacts.push((
                BlockMessage::LaneBlockProposal(proposal.clone()),
                proposal.descriptor.validator_set,
            ));
        }
        for (proposal, vote) in self
            .lane_sessions
            .local_vote_rebroadcast_artifacts_for(&self.local_peer)
        {
            if !self.proposal_body_available(&proposal) {
                continue;
            }
            lane_artifacts.push((
                BlockMessage::LaneBlockVote(vote),
                proposal.descriptor.validator_set,
            ));
        }
        for qc in self.lane_sessions.qcs_for_incomplete_sessions() {
            if !self.lane_vote_body_available(&qc.body) {
                continue;
            }
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

    fn accept_lane_relay(
        &mut self,
        envelope: LaneRelayEnvelope,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
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
                self.refresh_merge_candidates(active_view);
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
        if !self.proposal_body_available(&proposal)
            || !self.lane_proposal_authorized(&proposal, sender, local, active_view)
        {
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
        if sender != Some(&vote.signer)
            || !self.lane_vote_body_available(&vote.body)
            || !self.lane_vote_authorized(&vote, active_view)
        {
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
        if !self.lane_vote_body_available(&qc.body) || !self.lane_qc_authorized(&qc, active_view) {
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
                descriptor.lane_incarnation,
                descriptor.proposal_height,
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
                    body.lane_incarnation,
                    body.proposal_height,
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
                    body.lane_incarnation,
                    body.proposal_height,
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

    fn lane_vote_body_available(
        &self,
        body: &iroha_data_model::block::consensus::LaneBlockVoteBodyV1,
    ) -> bool {
        let key = crate::lane_consensus::LaneBlockSessionKey {
            lane_id: body.lane_id,
            dataspace_id: body.dataspace_id,
            lane_incarnation: body.lane_incarnation,
            lane_block_height: body.lane_block_height,
            lane_block_view: body.lane_block_view,
            proposal_hash: body.proposal_hash,
        };
        self.lane_sessions
            .proposal_for_key(&key)
            .as_ref()
            .is_some_and(|proposal| self.proposal_body_available(proposal))
            || self.canonical_proposal_for_vote_body(body).is_some()
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
        lane_incarnation: Hash,
        proposal_height: u64,
    ) -> bool {
        self.state.lane_route_and_incarnation_active_at_height(
            lane_id,
            dataspace_id,
            lane_incarnation,
            proposal_height,
        )
    }

    fn nexus_route_active(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        global_height: u64,
    ) -> bool {
        let nexus = self.state.nexus_snapshot();
        crate::state::consensus_lane_dataspace_at_height(lane_id, &nexus, global_height)
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
        let active_routes = self
            .state
            .consensus_lane_routes_at_height(self.context.height);
        // Reconstruct a sidecar if this is the post-application crash window.
        // Normal block persistence writes ownership sidecars transactionally;
        // the exact canonical lookup covers the one interrupted current-height
        // boundary without walking historical global blocks.
        let _ = self
            .kura
            .canonical_lane_block_artifacts_at_proposal_height_matching(
                self.context.height,
                self.limits.session_capacity.get(),
                |ownership| {
                    active_routes.get(&(ownership.lane_id, ownership.dataspace_id))
                        == Some(&ownership.lane_incarnation)
                },
            );

        let pending = self
            .state
            .unapplied_lane_block_artifact_heights_snapshot_cached();
        for ((lane_id, dataspace_id), lane_block_height) in
            pending.into_iter().take(self.limits.session_capacity.get())
        {
            let Some(artifact) = self
                .kura
                .read_lane_block_artifact(lane_id, lane_block_height)
            else {
                continue;
            };
            let ownership = &artifact.ownership;
            if ownership.lane_id != lane_id
                || ownership.dataspace_id != dataspace_id
                || self
                    .state
                    .lane_block_artifact_is_applied_or_snapshot_anchored_cached(&artifact)
                || !self.lane_route_active(
                    ownership.lane_id,
                    ownership.dataspace_id,
                    ownership.lane_incarnation,
                    ownership.proposal_height,
                )
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
                let ownership = &artifact.ownership;
                self.lane_route_active(
                    ownership.lane_id,
                    ownership.dataspace_id,
                    ownership.lane_incarnation,
                    ownership.proposal_height,
                ) && proposal_from_ownership(ownership, artifact.proposal_block_hash).as_ref()
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
        (self.canonical_anchor_for_proposal(&proposal).is_some()
            && proposal.vote_body(body.phase) == *body)
            .then_some(proposal)
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
            .latest_lane_block_artifact_matching(body.coordinator_lane_id, |artifact| {
                let ownership = &artifact.ownership;
                ownership.dataspace_id == body.coordinator_dataspace_id
                    && self.lane_route_active(
                        ownership.lane_id,
                        ownership.dataspace_id,
                        ownership.lane_incarnation,
                        ownership.proposal_height,
                    )
            })
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

    fn refresh_merge_candidates(&mut self, active_view: wire::View) {
        self.merge_entries.retain(|key, _| key.view == active_view);
        self.merge_claims
            .retain(|(_, view, _), _| *view == active_view);
        let candidates = self
            .state
            .merge_entry_candidates_from_lane_relays_for_view(active_view);
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
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        if signature.view != active_view {
            return V2LaneIngressOutcome::Rejected;
        }
        self.refresh_merge_candidates(active_view);
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
        let entry = pending.candidate.clone().into_entry(qc);
        if let Err(error) = self
            .state
            .validate_certified_merge_entry_for_global_order(&entry)
        {
            iroha_logger::warn!(
                ?error,
                epoch = entry.epoch_id,
                view = key.view,
                "rejecting locally certified merge entry before durable carrier staging"
            );
            return;
        }
        match self.kura.persist_pending_certified_merge_entry(&entry) {
            Ok(_) => {
                self.merge_entries.remove(&key);
                self.merge_claims
                    .retain(|(epoch, view, _), _| *epoch != key.epoch_id || *view != key.view);
            }
            Err(error) => {
                iroha_logger::warn!(
                    ?error,
                    epoch = entry.epoch_id,
                    view = key.view,
                    "failed to durably stage certified merge entry for global V2 consensus"
                );
            }
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
        self.refresh_merge_candidates(view);
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

/// Verify that a Kura-durable current-height body carries the exact lane plan
/// which was validated before its interrupted State application.
///
/// Re-running ordinary planning after `Kura::store_block` is incorrect because
/// the just-persisted lane artifacts intentionally block the next lane slot.
/// Recovery therefore authenticates the canonical block hash, exact sidecars,
/// frozen lifecycle/committee/tag bindings, proposer authority, and applied
/// predecessor instead of consulting the post-persistence frontier.
pub(crate) fn canonical_v2_lane_payload_matches_kura(
    state: &State,
    kura: &Kura,
    context: &wire::HeightContext,
    block: &SignedBlock,
) -> bool {
    let block_hash = block.hash();
    let Some(height) = usize::try_from(context.height)
        .ok()
        .and_then(NonZeroUsize::new)
    else {
        return false;
    };
    if block.header().height().get() != context.height
        || kura.block_hash_at_height(height) != Some(block_hash)
    {
        return false;
    }
    let Some(bundle) = block.execution_context() else {
        return block.external_entrypoint_count() == 0;
    };
    let ownerships = &bundle.lane_payload_ownerships;
    if ownerships.is_empty() {
        return bundle.external.is_empty();
    }

    let view = block.header().view_change_index();
    let Some(global_leader) = usize::try_from(context.leader(view))
        .ok()
        .and_then(|index| context.roster.get(index))
        .map(|entry| &entry.validator)
    else {
        return false;
    };
    let nexus = state.nexus_snapshot();
    let shared_committee = !nexus.enabled || !proposal_lookahead_enabled(&nexus, context.height);
    let base_mode_tag = match context.mode {
        wire::ConsensusMode::Permissioned => wire::PERMISSIONED_TAG,
        wire::ConsensusMode::Npos => wire::NPOS_TAG,
    };
    let context_mode_tag = format!(
        "{base_mode_tag}::height-context:{}::epoch:{}",
        hex::encode(context.id().0.as_ref()),
        context.epoch
    );

    let ownership_is_valid = |ownership: &SumeragiLanePayloadOwnership| {
        if ownership.proposal_height != context.height
            || ownership.proposal_view != view
            || ownership.lane_block_view != view
            || ownership.validate_replay_material().is_err()
            || !state.lane_route_and_incarnation_active_at_height(
                ownership.lane_id,
                ownership.dataspace_id,
                ownership.lane_incarnation,
                ownership.proposal_height,
            )
            || ownership.qc_mode_tag
                != LaneRelayEnvelope::lane_qc_mode_tag_for(
                    ownership.lane_id,
                    ownership.dataspace_id,
                    &context_mode_tag,
                )
        {
            return false;
        }
        let mut expected_validators = if shared_committee {
            context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect::<Vec<_>>()
        } else {
            state.authoritative_lane_peer_ids_at_height(ownership.lane_id, context.height)
        };
        expected_validators.sort();
        expected_validators.dedup();
        if expected_validators.is_empty()
            || ownership.lane_block_descriptor_validator_set != expected_validators
        {
            return false;
        }
        let Some(proposal) = proposal_from_ownership(ownership, block_hash) else {
            return false;
        };
        let expected_author = if shared_committee {
            Some(global_leader)
        } else {
            lane_proposal_author(&proposal)
        };
        expected_author == Some(global_leader)
            && state
                .certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached(&proposal)
    };

    let artifacts = kura.canonical_lane_block_artifacts_at_proposal_height_matching(
        context.height,
        ownerships.len(),
        ownership_is_valid,
    );
    artifacts.len() == ownerships.len()
        && artifacts
            .iter()
            .zip(ownerships)
            .all(|(artifact, ownership)| {
                artifact.proposal_block_hash == block_hash && artifact.ownership == *ownership
            })
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
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::{NonZeroU64, NonZeroUsize},
        sync::Arc,
    };

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::{
            BlockExecutionContextBundle, BlockHeader, BlockSignature, ExternalExecutionContext,
            SignedBlock,
            builder::BlockBuilder,
            consensus::{NativeAmxAttestationBodyV2, NativeAmxPhase, SumeragiLanePayloadOwnership},
            consensus_v2 as wire,
        },
        consensus::{ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus},
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
        transaction::{TransactionBuilder, TransactionEntrypoint, signed::TransactionResultInner},
        trigger::DataTriggerSequence,
    };

    use super::*;
    use crate::{
        block::{CommittedBlock, ValidBlock},
        query::store::LiveQueryStore,
        state::World,
        sumeragi::network_topology::Topology,
    };

    fn fixture(mode: wire::ConsensusMode) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
        fixture_at_height(mode, 9)
    }

    fn fixture_at_height(
        mode: wire::ConsensusMode,
        height: u64,
    ) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
        let chain_id: ChainId = "v2-lane-work-test".into();
        let kura = Kura::blank_kura_for_testing();
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let mut world = World::new();
        for (index, key) in keys.iter().enumerate() {
            let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, format!("validator{index}"));
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: key.public_key().clone(),
                pop: Some(
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("BLS proof of possession"),
                ),
                activation_height: 0,
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            world.consensus_keys.insert(id.clone(), record.clone());
            world
                .consensus_keys_by_pk
                .insert(record.public_key.to_string(), vec![id]);
        }
        let state = Arc::new(State::new_with_chain_for_testing(
            world,
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            chain_id.clone(),
        ));
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
            height,
            epoch: 4,
            epoch_end_height: height.saturating_add(11),
            mode,
            parent_commit_qc: (height > 1).then(|| wire::QuorumCertificate {
                round: wire::ConsensusRound {
                    context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                        format!("v2-lane-work-parent-context:{}", height - 1).as_bytes(),
                    ))),
                    height: height - 1,
                    view: 0,
                },
                phase: wire::GlobalPhase::Commit,
                subject: wire::BlockSubject {
                    parent_block_hash: None,
                    block_hash: HashOf::from_untyped_unchecked(Hash::new(
                        format!("v2-lane-work-parent-block:{}", height - 1).as_bytes(),
                    )),
                    payload_hash: Hash::new(
                        format!("v2-lane-work-parent-payload:{}", height - 1).as_bytes(),
                    ),
                },
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xA5; 48],
            }),
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
        let mut parent = None;
        for block_height in 1..height {
            let block = ValidBlock::new_dummy_and_modify_header(
                keys[0].private_key(),
                |header: &mut BlockHeader| {
                    header.set_height(
                        NonZeroU64::new(block_height).expect("non-zero fixture height"),
                    );
                    header.set_prev_block_hash(parent);
                    header.merkle_root = None;
                },
            )
            .commit_unchecked()
            .unpack(|_| {});
            parent = Some(block.as_ref().hash());
            commit_test_block_to_state(state.as_ref(), &block, &context);
        }
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
            None,
        )
        .expect("open lane adapter");
        (adapter, keys)
    }

    fn commit_test_block_to_state(
        state: &State,
        block: &CommittedBlock,
        context: &wire::HeightContext,
    ) {
        let topology = Topology::new(context.roster.iter().map(|entry| entry.validator.clone()));
        let mut state_block = state.block(block.as_ref().header());
        let _events = state_block.apply_without_execution(block, topology.as_ref().to_owned());
        state_block.commit().expect("commit synthetic state block");
    }

    #[test]
    fn post_apply_recovery_requires_exact_state_and_kura_tip_binding() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let mut context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let local_key = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        let pre_apply_token = super::super::v2_recovery::PendingKuraApply::for_test(
            context.id(),
            context.height,
            HashOf::from_untyped_unchecked(Hash::new(b"executor-owned pre-apply token")),
        );
        let pre_apply = V2LaneWorkAdapter::new(
            context.clone(),
            local_peer.clone(),
            local_key.clone(),
            true,
            Arc::clone(&state),
            Arc::clone(&kura),
            limits,
            None,
        )
        .expect("pre-apply recovery continues to use the committed lifecycle projection");
        drop(pre_apply);
        assert!(matches!(
            V2LaneWorkAdapter::new(
                context.clone(),
                local_peer.clone(),
                local_key.clone(),
                true,
                Arc::clone(&state),
                Arc::clone(&kura),
                limits,
                Some(pre_apply_token),
            ),
            Err(V2LaneWorkError::RecoveredAppliedTipMismatch)
        ));

        let block = ValidBlock::new_dummy_and_modify_header(
            keys[0].private_key(),
            |header: &mut BlockHeader| {
                header.set_height(NonZeroU64::new(1).expect("non-zero fixture height"));
                header.set_prev_block_hash(None);
                header.merkle_root = None;
            },
        )
        .commit_unchecked()
        .unpack(|_| {});
        kura.store_block(block.clone())
            .expect("persist canonical recovery tip");
        commit_test_block_to_state(state.as_ref(), &block, &context);

        context.nexus_amx_context_hash = Hash::new(b"frozen pre-application lifecycle");
        assert_ne!(
            context.nexus_amx_context_hash,
            super::super::v2_recovery::committed_nexus_amx_context_hash(state.as_ref()),
            "fixture must exercise the post-application context-hash exception"
        );
        let block_hash = block.as_ref().hash();
        let wrong = super::super::v2_recovery::PendingKuraApply::for_test(
            context.id(),
            context.height,
            HashOf::from_untyped_unchecked(Hash::new(b"wrong recovery block")),
        );
        assert!(matches!(
            V2LaneWorkAdapter::new(
                context.clone(),
                local_peer.clone(),
                local_key.clone(),
                true,
                Arc::clone(&state),
                Arc::clone(&kura),
                limits,
                Some(wrong),
            ),
            Err(V2LaneWorkError::RecoveredAppliedTipMismatch)
        ));

        let exact = super::super::v2_recovery::PendingKuraApply::for_test(
            context.id(),
            context.height,
            block_hash,
        );
        V2LaneWorkAdapter::new(
            context,
            local_peer,
            local_key,
            true,
            state,
            kura,
            limits,
            Some(exact),
        )
        .expect("exact post-application recovery tip bypasses mutable lifecycle drift");
    }

    #[test]
    fn canonical_kura_lane_recovery_rejects_body_lifecycle_and_qc_tag_drift() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(LaneId::SINGLE, 1)
            .expect("canonical lane incarnation");
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            incarnation,
            1,
            1,
        );
        let ownership = ownership_from_proposal(&proposal);
        let leader = usize::try_from(adapter.context.leader(0)).expect("leader index");
        let block = test_block(1, None, Some(ownership), &keys[leader]);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist exact canonical recovery body");
        assert!(canonical_v2_lane_payload_matches_kura(
            adapter.state.as_ref(),
            adapter.kura.as_ref(),
            &adapter.context,
            &block,
        ));

        let drifted_body = test_block(1, None, None, &keys[leader]);
        assert_ne!(drifted_body.hash(), block.hash());
        assert!(!canonical_v2_lane_payload_matches_kura(
            adapter.state.as_ref(),
            adapter.kura.as_ref(),
            &adapter.context,
            &drifted_body,
        ));

        for (incarnation, tag_suffix) in [
            (Hash::new(b"retired lane incarnation"), None),
            (incarnation, Some("::wrong-height-context")),
        ] {
            let (drifted, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
            let proposal = proposal_for_route(
                &drifted,
                &keys,
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
                incarnation,
                1,
                1,
            );
            let mut ownership = ownership_from_proposal(&proposal);
            if let Some(suffix) = tag_suffix {
                ownership.qc_mode_tag.push_str(suffix);
                let replay = ownership
                    .compute_replay_hashes()
                    .expect("recompute adversarial ownership hashes");
                ownership.subject_hash = replay.subject_hash;
                ownership.payload_ownership_hash = replay.payload_ownership_hash;
                ownership.rbc_instance_hash = replay.rbc_instance_hash;
                ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
            }
            let leader = usize::try_from(drifted.context.leader(0)).expect("leader index");
            let block = test_block(1, None, Some(ownership), &keys[leader]);
            drifted
                .kura
                .store_block(block.clone())
                .expect("persist adversarial canonical body");
            assert!(
                !canonical_v2_lane_payload_matches_kura(
                    drifted.state.as_ref(),
                    drifted.kura.as_ref(),
                    &drifted.context,
                    &block,
                ),
                "canonical Kura placement must not authorize lifecycle or QC-tag drift"
            );
        }
    }

    #[test]
    fn adapter_hydrates_unapplied_canonical_frontier_from_prior_global_height() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 2);
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, 1)
            .expect("canonical lane incarnation is active at the prior height");
        let proposal =
            proposal_for_route(&adapter, &keys, lane_id, dataspace_id, incarnation, 1, 1);
        let canonical = store_canonical_anchor(&adapter, &proposal, &keys[0]);
        let descriptor = &canonical.descriptor;
        let session_key = crate::lane_consensus::LaneBlockSessionKey {
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            proposal_hash: canonical.proposal_hash,
        };

        let context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let local_key = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        let recovered = V2LaneWorkAdapter::new(
            context, local_peer, local_key, true, state, kura, limits, None,
        )
        .expect("open successor-height adapter");
        assert!(
            recovered.lane_sessions.get(&session_key).is_some(),
            "successor height must retain unfinished lane consensus anchored by the prior block"
        );
    }

    #[test]
    fn persisted_v2_lane_qc_records_globally_applied_receipt_and_unblocks_next_height() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, 1)
            .expect("canonical lane incarnation is active");
        let transaction_key =
            KeyPair::try_from_seed(vec![0xD1; 32], Algorithm::Ed25519).expect("transaction key");
        let transaction = TransactionBuilder::new(
            adapter.context.chain_id.clone(),
            AccountId::new(transaction_key.public_key().clone()),
        )
        .sign(transaction_key.private_key());
        let entrypoint_hash = transaction.hash_as_entrypoint();

        let base = proposal_for_route(&adapter, &keys, lane_id, dataspace_id, incarnation, 1, 1);
        let mut ownership = ownership_from_proposal(&base);
        ownership.accepted_transaction_hashes = vec![Hash::from(entrypoint_hash)];
        let replay = ownership
            .compute_replay_hashes()
            .expect("receipt fixture replay material");
        ownership.subject_hash = replay.subject_hash;
        ownership.payload_ownership_hash = replay.payload_ownership_hash;
        ownership.rbc_instance_hash = replay.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);

        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero fixture height"),
            None,
            None,
            None,
            1,
            0,
        );
        let signature = SignatureOf::try_from_hash(keys[0].private_key(), header.hash())
            .expect("sign receipt fixture block");
        let mut block =
            SignedBlock::presigned(BlockSignature::new(0, signature), header, vec![transaction]);
        block.set_execution_context(Some(
            BlockExecutionContextBundle::new(Vec::new())
                .with_lane_payload_ownerships(vec![ownership.clone()]),
        ));
        block
            .set_transaction_results(
                Vec::new(),
                &[entrypoint_hash],
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("attach canonical transaction result");
        let proposal = proposal_from_ownership(&ownership, block.hash())
            .expect("reconstruct globally anchored proposal");
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist globally applied canonical block");
        assert!(adapter.mark_global_body_locked(block.hash()));
        assert_eq!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Inserted
        );

        assert_eq!(
            adapter.insert_lane_qc(lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare), 0,),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(
            adapter.insert_lane_qc(lane_qc_for_phase(&proposal, &keys, CertPhase::Commit), 0,),
            V2LaneIngressOutcome::Inserted
        );
        assert!(matches!(
            adapter.persist_anchored_sessions(),
            Err(V2LaneWorkError::Persistence(_))
        ));
        assert!(
            !adapter
                .kura
                .lane_block_application_receipt_available(&proposal),
            "Kura-ahead recovery must not manufacture a WSV application receipt"
        );

        let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
        commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("persist v2 certificate and application receipt"),
            1
        );
        assert!(
            adapter
                .kura
                .lane_block_application_receipt_available(&proposal)
        );
        assert!(
            adapter
                .state
                .unapplied_lane_block_artifact_heights_snapshot_cached()
                .is_empty(),
            "canonical results receipt must unblock the next lane-local height"
        );
        assert_eq!(
            adapter.state.lane_block_artifact_tips_snapshot_cached(),
            vec![(
                lane_id,
                dataspace_id,
                incarnation,
                1,
                Some(proposal.descriptor.descriptor_hash),
            )]
        );
    }

    #[test]
    fn restart_repairs_certified_lane_sidecar_missing_only_application_receipt() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist globally anchored lane block");
        let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
        commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);

        let certified = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        adapter
            .kura
            .persist_committed_lane_block_session(&certified, &lane_signer_pops(&keys))
            .expect("persist certificate before simulated crash");
        assert!(
            !adapter
                .kura
                .lane_block_application_receipt_available(&proposal),
            "fixture must stop after certificate durability but before receipt publication"
        );

        let context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let local_key = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        let recovery = super::super::v2_recovery::PendingKuraApply::for_test(
            context.id(),
            context.height,
            block.hash(),
        );
        drop(adapter);

        let reopened = V2LaneWorkAdapter::new(
            context,
            local_peer,
            local_key,
            true,
            Arc::clone(&state),
            Arc::clone(&kura),
            limits,
            Some(recovery),
        )
        .expect("restart repairs the exact certificate/receipt crash boundary");
        assert!(
            kura.lane_block_application_receipt_available(&proposal),
            "restart must publish the missing canonical application receipt"
        );
        assert!(
            state
                .unapplied_lane_block_artifact_heights_snapshot_cached()
                .is_empty(),
            "the repaired receipt must unblock the next lane-local height"
        );
        drop(reopened);
    }

    fn canonical_qc_mode_tag(
        adapter: &V2LaneWorkAdapter,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
    ) -> String {
        let base = match adapter.context.mode {
            wire::ConsensusMode::Permissioned => wire::PERMISSIONED_TAG,
            wire::ConsensusMode::Npos => wire::NPOS_TAG,
        };
        let context_tag = format!(
            "{base}::height-context:{}::epoch:{}",
            hex::encode(adapter.context.id().0.as_ref()),
            adapter.context.epoch
        );
        LaneRelayEnvelope::lane_qc_mode_tag_for(lane_id, dataspace_id, &context_tag)
    }

    fn proposal_for_route(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        proposal_height: u64,
        lane_block_height: u64,
    ) -> LaneBlockProposalV1 {
        proposal_for_route_at_view(
            adapter,
            keys,
            lane_id,
            dataspace_id,
            lane_incarnation,
            proposal_height,
            lane_block_height,
            0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn proposal_for_route_at_view(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        proposal_height: u64,
        lane_block_height: u64,
        lane_block_view: u64,
    ) -> LaneBlockProposalV1 {
        let validator_set = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let validator_count =
            u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
        let min_quorum = u32::try_from(
            crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()).max(1),
        )
        .expect("fixture quorum fits u32");
        let previous_lane_block_height = lane_block_height.saturating_sub(1);
        let mut ownership = SumeragiLanePayloadOwnership {
            proposal_height,
            proposal_view: lane_block_view,
            lane_id,
            dataspace_id,
            lane_incarnation,
            lane_block_height,
            lane_block_view,
            subject_hash: Hash::prehashed([0; Hash::LENGTH]),
            qc_mode_tag: canonical_qc_mode_tag(adapter, lane_id, dataspace_id),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::new(
                format!(
                    "v2-lane-work-candidate:{proposal_height}:{lane_block_height}:{}:{}",
                    lane_id.as_u32(),
                    dataspace_id.as_u64()
                )
                .as_bytes(),
            )],
            previous_lane_block_height,
            previous_lane_block_descriptor_hash: (previous_lane_block_height > 0).then(|| {
                Hash::new(
                    format!("v2-lane-work-previous:{proposal_height}:{previous_lane_block_height}")
                        .as_bytes(),
                )
            }),
            lane_block_descriptor_hash: Some(Hash::prehashed([0; Hash::LENGTH])),
            lane_block_descriptor_validator_set: validator_set,
            lane_block_descriptor_validator_count: validator_count,
            lane_block_descriptor_min_quorum: min_quorum,
            payload_ownership_hash: Hash::prehashed([0; Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        let replay = ownership
            .compute_replay_hashes()
            .expect("fixture ownership replay material is well-formed");
        ownership.subject_hash = replay.subject_hash;
        ownership.payload_ownership_hash = replay.payload_ownership_hash;
        ownership.rbc_instance_hash = replay.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
        proposal_from_ownership(
            &ownership,
            HashOf::from_untyped_unchecked(Hash::new(
                format!("v2-lane-work-hint:{proposal_height}:{lane_block_height}").as_bytes(),
            )),
        )
        .expect("canonical fixture ownership reconstructs a proposal")
    }

    fn mark_lane_reset(adapter: &V2LaneWorkAdapter, lane_id: LaneId, reset_height: u64) {
        adapter
            .state
            .da_shard_cursors
            .write()
            .mark_lanes_canonically_reset(&BTreeSet::from([lane_id]), reset_height);
    }

    fn signed_lane_vote(
        proposal: &LaneBlockProposalV1,
        phase: CertPhase,
        key_pair: &KeyPair,
    ) -> LaneBlockVoteV1 {
        let body = proposal.vote_body(phase);
        let signature = Signature::try_new(key_pair.private_key(), &body.signature_preimage())
            .expect("fixture lane vote signature");
        LaneBlockVoteV1 {
            body,
            payload_availability_vote: None,
            signer: PeerId::new(key_pair.public_key().clone()),
            bls_signature: signature.payload().to_vec(),
        }
    }

    fn lane_qc(proposal: &LaneBlockProposalV1, keys: &[KeyPair]) -> LaneBlockQcV1 {
        lane_qc_for_phase(proposal, keys, CertPhase::Prepare)
    }

    fn lane_qc_for_phase(
        proposal: &LaneBlockProposalV1,
        keys: &[KeyPair],
        phase: CertPhase,
    ) -> LaneBlockQcV1 {
        let votes = keys
            .iter()
            .map(|key_pair| signed_lane_vote(proposal, phase, key_pair))
            .collect::<Vec<_>>();
        crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            proposal.vote_body(phase),
            proposal.descriptor.validator_set.clone(),
            &votes,
        )
        .expect("fixture lane votes form a valid QC")
    }

    fn ownership_from_proposal(proposal: &LaneBlockProposalV1) -> SumeragiLanePayloadOwnership {
        let descriptor = &proposal.descriptor;
        SumeragiLanePayloadOwnership {
            proposal_height: descriptor.proposal_height,
            proposal_view: descriptor.lane_block_view,
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
        }
    }

    fn globally_anchored_lane_block_fixture(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
    ) -> (SignedBlock, LaneBlockProposalV1) {
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height)
            .expect("canonical lane incarnation is active");
        let transaction_key =
            KeyPair::try_from_seed(vec![0xD2; 32], Algorithm::Ed25519).expect("transaction key");
        let transaction = TransactionBuilder::new(
            adapter.context.chain_id.clone(),
            AccountId::new(transaction_key.public_key().clone()),
        )
        .sign(transaction_key.private_key());
        let entrypoint_hash = transaction.hash_as_entrypoint();
        let base = proposal_for_route(
            adapter,
            keys,
            lane_id,
            dataspace_id,
            incarnation,
            adapter.context.height,
            1,
        );
        let mut ownership = ownership_from_proposal(&base);
        ownership.accepted_transaction_hashes = vec![Hash::from(entrypoint_hash)];
        let replay = ownership
            .compute_replay_hashes()
            .expect("restart receipt replay material");
        ownership.subject_hash = replay.subject_hash;
        ownership.payload_ownership_hash = replay.payload_ownership_hash;
        ownership.rbc_instance_hash = replay.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);

        let header = BlockHeader::new(
            NonZeroU64::new(adapter.context.height).expect("non-zero fixture height"),
            None,
            None,
            None,
            1,
            0,
        );
        let leader = usize::try_from(adapter.context.leader(0)).expect("leader index");
        let signature = SignatureOf::try_from_hash(keys[leader].private_key(), header.hash())
            .expect("sign restart receipt fixture block");
        let mut block = SignedBlock::presigned(
            BlockSignature::new(
                u64::try_from(leader).expect("leader index fits u64"),
                signature,
            ),
            header,
            vec![transaction],
        );
        block.set_execution_context(Some(
            BlockExecutionContextBundle::new(Vec::new())
                .with_lane_payload_ownerships(vec![ownership.clone()]),
        ));
        block
            .set_transaction_results(
                Vec::new(),
                &[entrypoint_hash],
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("attach canonical restart transaction result");
        let proposal = proposal_from_ownership(&ownership, block.hash())
            .expect("reconstruct globally anchored restart proposal");
        (block, proposal)
    }

    fn lane_signer_pops(keys: &[KeyPair]) -> BTreeMap<PublicKey, Vec<u8>> {
        keys.iter()
            .map(|key| {
                (
                    key.public_key().clone(),
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("lane validator proof of possession"),
                )
            })
            .collect()
    }

    fn test_block(
        height: u64,
        parent: Option<HashOf<BlockHeader>>,
        ownership: Option<SumeragiLanePayloadOwnership>,
        signer: &KeyPair,
    ) -> SignedBlock {
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("fixture block height is non-zero"),
            parent,
            None,
            None,
            height,
            0,
        );
        let mut builder = BlockBuilder::new(header);
        if let Some(ownership) = ownership {
            builder.set_execution_context(Some(
                BlockExecutionContextBundle::new(Vec::new())
                    .with_lane_payload_ownerships(vec![ownership]),
            ));
        }
        builder.build_with_signature(0, signer.private_key())
    }

    fn planned_lane_candidate_block_at_view(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
        view: u64,
    ) -> (SignedBlock, LaneBlockProposalV1) {
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let transaction_key = KeyPair::try_from_seed(
            vec![u8::try_from(view).unwrap_or(u8::MAX).wrapping_add(0x40); 32],
            Algorithm::Ed25519,
        )
        .expect("deterministic candidate transaction key");
        let transaction = TransactionBuilder::new(
            adapter.context.chain_id.clone(),
            AccountId::new(transaction_key.public_key().clone()),
        )
        .sign(transaction_key.private_key());
        let entrypoint_hash = transaction.hash_as_entrypoint();
        let leader_index =
            usize::try_from(adapter.context.leader(view)).expect("global leader index fits usize");
        let leader = &adapter.context.roster[leader_index].validator;
        let plan = prepare_v2_lane_payload_plan(
            adapter.state.as_ref(),
            &adapter.context,
            view,
            leader,
            &[RoutingDecision::new(lane_id, dataspace_id)],
            &[Hash::from(entrypoint_hash)],
        )
        .expect("coherent lane candidate plan");
        assert!(plan.unavailable_indices.is_empty());
        assert_eq!(plan.ownerships.len(), 1);
        assert_eq!(plan.proposals.len(), 1);

        let header = BlockHeader::new(
            NonZeroU64::new(adapter.context.height).expect("non-zero fixture height"),
            None,
            None,
            None,
            adapter.context.height,
            view,
        );
        let mut builder = BlockBuilder::new(header);
        builder.push_transaction(transaction);
        builder.set_execution_context(Some(
            BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                entrypoint_hash,
                lane_id,
                dataspace_id,
            )])
            .with_lane_payload_ownerships(plan.ownerships.clone()),
        ));
        let block = builder.build_with_signature(
            u64::try_from(leader_index).expect("leader index fits u64"),
            keys[leader_index].private_key(),
        );
        let proposal = proposal_from_ownership(&plan.ownerships[0], block.hash())
            .expect("planned ownership reconstructs a proposal");
        assert_eq!(proposal.proposal_hash, plan.proposals[0].proposal_hash);
        (block, proposal)
    }

    fn store_canonical_anchor(
        adapter: &V2LaneWorkAdapter,
        proposal: &LaneBlockProposalV1,
        signer: &KeyPair,
    ) -> LaneBlockProposalV1 {
        let target_height = proposal.descriptor.proposal_height;
        assert!(target_height > 0, "canonical fixture height is non-zero");
        assert_eq!(
            adapter.kura.blocks_count(),
            0,
            "canonical fixture expects a blank Kura"
        );
        let mut parent = None;
        for height in 1..target_height {
            let block = test_block(height, parent, None, signer);
            parent = Some(block.hash());
            adapter
                .kura
                .store_block(block)
                .expect("store canonical fixture ancestor");
        }
        let ownership = ownership_from_proposal(proposal);
        ownership
            .validate_replay_material()
            .expect("canonical fixture ownership replay material validates");
        let block = test_block(target_height, parent, Some(ownership.clone()), signer);
        let block_hash = block.hash();
        adapter
            .kura
            .store_block(block)
            .expect("store canonical lane anchor block");
        proposal_from_ownership(&ownership, block_hash)
            .expect("stored ownership reconstructs its canonical proposal")
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
            coordinator_lane_id: LaneId::SINGLE,
            coordinator_dataspace_id: DataSpaceId::UNIVERSAL,
            participant_lane_id: LaneId::SINGLE,
            participant_dataspace_id: DataSpaceId::UNIVERSAL,
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
    fn native_coordinator_height_ignores_retired_incarnation_artifacts() {
        let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let stale = proposal_for_route(
            &adapter,
            &keys,
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            Hash::new(b"retired-native-coordinator-incarnation"),
            adapter.context.height,
            100,
        );
        let _ = store_canonical_anchor(&adapter, &stale, &keys[0]);
        assert!(
            adapter
                .kura
                .latest_lane_block_artifact(LaneId::SINGLE)
                .is_some_and(|artifact| artifact.ownership.lane_block_height == 100),
            "fixture must install a stale high lane-local artifact"
        );

        let body = native_body(&adapter);
        assert!(
            adapter.native_coordinator_height_is_current(&body),
            "retired-incarnation history must not advance the active coordinator height"
        );
        assert!(adapter.native_body_matches_context(&body, 0));
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
    fn lane_work_stays_quiescent_until_the_exact_global_prepare_lock() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let later_view =
            u64::try_from(adapter.context.roster.len()).expect("fixture roster length fits u64");
        assert_eq!(
            adapter.context.leader(0),
            adapter.context.leader(later_view)
        );

        let (block_zero, proposal_at_view_zero) =
            planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let round_zero = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: 0,
        };
        adapter
            .planned_lane_proposals
            .insert(round_zero, vec![proposal_at_view_zero.clone()]);
        assert_eq!(
            adapter.bind_local_candidate(round_zero, block_zero.hash()),
            V2LaneIngressOutcome::Inserted
        );
        adapter.schedule_retransmission();
        assert!(
            adapter.drain_effects(usize::MAX).is_empty(),
            "local Prepare intent must not leak lane proposals or votes before PrepareQC"
        );
        assert!(adapter.lane_sessions.commit_vote_lock_slots().is_empty());

        let (later_block, proposal_at_later_view) =
            planned_lane_candidate_block_at_view(&adapter, &keys, later_view);
        assert_ne!(
            proposal_at_view_zero.proposal_hash,
            proposal_at_later_view.proposal_hash
        );
        let later_round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: later_view,
        };
        adapter
            .planned_lane_proposals
            .insert(later_round, vec![proposal_at_later_view.clone()]);
        assert_eq!(
            adapter.bind_local_candidate(later_round, later_block.hash()),
            V2LaneIngressOutcome::Inserted,
            "a later global view must remain free to replan before any PrepareQC lock"
        );

        assert_eq!(
            adapter.bind_locked_global_body(&block_zero),
            V2LaneIngressOutcome::Rejected,
            "a validated body alone is insufficient without the reducer lock"
        );
        assert!(adapter.mark_global_body_locked(later_block.hash()));
        assert_eq!(
            adapter.bind_locked_global_body(&block_zero),
            V2LaneIngressOutcome::Rejected,
            "a stale body must not satisfy the exact locked subject"
        );
        adapter.schedule_retransmission();
        assert!(
            adapter.drain_effects(usize::MAX).is_empty(),
            "the lock without its exact durable body must not release lane work"
        );

        assert_ne!(
            adapter.bind_locked_global_body(&later_block),
            V2LaneIngressOutcome::Rejected
        );
        let effects = adapter.drain_effects(usize::MAX);
        assert!(effects.iter().any(|effect| matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockProposal(proposal),
                ..
            } if proposal.proposal_hash == proposal_at_later_view.proposal_hash
        )));
        assert!(effects.iter().any(|effect| matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockVote(vote),
                ..
            } if vote.body.proposal_hash == proposal_at_later_view.proposal_hash
        )));
        assert!(!effects.iter().any(|effect| matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockProposal(proposal),
                ..
            } if proposal.proposal_hash == proposal_at_view_zero.proposal_hash
        )));
    }

    #[test]
    fn lane_route_reset_watermark_is_global_proposal_height_not_lane_local_height() {
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let reset_height = 8;

        let (fresh_adapter, fresh_keys) =
            fixture_at_height(wire::ConsensusMode::Permissioned, reset_height + 1);
        mark_lane_reset(&fresh_adapter, lane_id, reset_height);
        let fresh_incarnation = fresh_adapter
            .state
            .lane_incarnation_at_height(lane_id, reset_height + 1)
            .expect("canonical lane incarnation is active after the reset height");
        let fresh_lane_one = proposal_for_route(
            &fresh_adapter,
            &fresh_keys,
            lane_id,
            dataspace_id,
            fresh_incarnation,
            reset_height + 1,
            1,
        );
        assert!(
            fresh_adapter.lane_route_active(
                lane_id,
                dataspace_id,
                fresh_incarnation,
                fresh_lane_one.descriptor.proposal_height,
            ),
            "a newly recreated lane-local height 1 must become active at global reset + 1"
        );
        assert!(
            fresh_adapter.lane_proposal_authorized(&fresh_lane_one, None, true, 0),
            "the fresh lane-local height 1 proposal must pass the complete proposal guard"
        );

        let (stale_adapter, stale_keys) =
            fixture_at_height(wire::ConsensusMode::Permissioned, reset_height);
        mark_lane_reset(&stale_adapter, lane_id, reset_height);
        let stale_incarnation = stale_adapter
            .state
            .lane_incarnation(lane_id)
            .expect("canonical lane incarnation remains identifiable at the reset boundary");
        assert_eq!(
            stale_adapter
                .state
                .lane_incarnation_at_height(lane_id, reset_height),
            None,
            "the reset carrier height must fail closed before proposal construction"
        );
        let stale_high_lane_height = proposal_for_route(
            &stale_adapter,
            &stale_keys,
            lane_id,
            dataspace_id,
            stale_incarnation,
            reset_height,
            100,
        );
        assert!(
            !stale_adapter.lane_route_active(
                lane_id,
                dataspace_id,
                stale_incarnation,
                stale_high_lane_height.descriptor.proposal_height,
            ),
            "a high lane-local height must not outrun the global reset watermark"
        );
        assert!(
            !stale_adapter.lane_proposal_authorized(&stale_high_lane_height, None, true, 0),
            "the complete proposal guard must reject evidence at the reset boundary"
        );
    }

    #[test]
    fn lane_proposal_vote_and_qc_reject_non_authoritative_incarnation() {
        let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let proposal_height = adapter.context.height;
        let active_incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, proposal_height)
            .expect("canonical lane incarnation is active");
        let active = proposal_for_route(
            &adapter,
            &keys,
            lane_id,
            dataspace_id,
            active_incarnation,
            proposal_height,
            1,
        );
        let active_vote = signed_lane_vote(&active, CertPhase::Prepare, &keys[0]);
        let active_qc = lane_qc(&active, &keys);
        assert!(adapter.lane_proposal_authorized(&active, None, true, 0));
        assert!(adapter.lane_vote_authorized(&active_vote, 0));
        assert!(adapter.lane_qc_authorized(&active_qc, 0));

        let stale_incarnation = Hash::new(b"retired-v2-lane-work-incarnation");
        assert_ne!(stale_incarnation, active_incarnation);
        let stale = proposal_for_route(
            &adapter,
            &keys,
            lane_id,
            dataspace_id,
            stale_incarnation,
            proposal_height,
            1,
        );
        let stale_vote = signed_lane_vote(&stale, CertPhase::Prepare, &keys[0]);
        let stale_qc = lane_qc(&stale, &keys);
        assert!(
            !adapter.lane_route_active(lane_id, dataspace_id, stale_incarnation, proposal_height,),
            "route admission must bind the exact active incarnation"
        );
        assert!(
            !adapter.lane_proposal_authorized(&stale, None, true, 0),
            "a well-formed, correctly authored proposal from a retired incarnation must fail"
        );
        assert!(
            !adapter.lane_vote_authorized(&stale_vote, 0),
            "a validly signed vote cannot revive a retired incarnation"
        );
        assert!(
            !adapter.lane_qc_authorized(&stale_qc, 0),
            "a cryptographically valid QC cannot revive a retired incarnation"
        );
    }

    #[test]
    fn canonical_kura_anchor_cannot_bypass_route_reset_or_incarnation_guards() {
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;

        {
            let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 3);
            let incarnation = adapter
                .state
                .lane_incarnation_at_height(lane_id, adapter.context.height)
                .expect("canonical lane incarnation is active");
            let proposal = proposal_for_route(
                &adapter,
                &keys,
                lane_id,
                dataspace_id,
                incarnation,
                adapter.context.height,
                1,
            );
            let canonical = store_canonical_anchor(&adapter, &proposal, &keys[0]);
            assert!(
                adapter.kura.read_lane_block_artifact(lane_id, 1).is_some(),
                "fixture must retain a raw canonical Kura anchor"
            );
            assert!(adapter.canonical_anchor_for_proposal(&canonical).is_some());
            assert!(
                adapter
                    .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                    .is_some()
            );

            mark_lane_reset(&adapter, lane_id, adapter.context.height);
            assert!(
                adapter.kura.read_lane_block_artifact(lane_id, 1).is_some(),
                "reset validation must be tested with the canonical file still present"
            );
            assert!(
                adapter.canonical_anchor_for_proposal(&canonical).is_none(),
                "a canonical file at the reset watermark is not an admissible anchor"
            );
            assert!(
                adapter
                    .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                    .is_none(),
                "historical vote recovery must apply the reset guard too"
            );
            assert!(
                !adapter.lane_proposal_authorized(&canonical, None, true, 0),
                "canonical-anchor fast path must not bypass the reset guard"
            );
        }

        {
            let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 2);
            let incarnation = adapter
                .state
                .lane_incarnation_at_height(lane_id, adapter.context.height)
                .expect("canonical lane incarnation is active");
            let wrong_dataspace = DataSpaceId::new(91);
            let proposal = proposal_for_route(
                &adapter,
                &keys,
                lane_id,
                wrong_dataspace,
                incarnation,
                adapter.context.height,
                1,
            );
            let canonical = store_canonical_anchor(&adapter, &proposal, &keys[0]);
            assert!(
                adapter.kura.read_lane_block_artifact(lane_id, 1).is_some(),
                "wrong-route fixture must still be canonical Kura data"
            );
            assert!(
                adapter.canonical_anchor_for_proposal(&canonical).is_none(),
                "canonical storage must not make an inactive dataspace route authoritative"
            );
            assert!(
                adapter
                    .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                    .is_none()
            );
            assert!(!adapter.lane_proposal_authorized(&canonical, None, true, 0));
        }

        {
            let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 2);
            let active_incarnation = adapter
                .state
                .lane_incarnation_at_height(lane_id, adapter.context.height)
                .expect("canonical lane incarnation is active");
            let stale_incarnation = Hash::new(b"canonical-but-retired-lane-incarnation");
            assert_ne!(stale_incarnation, active_incarnation);
            let proposal = proposal_for_route(
                &adapter,
                &keys,
                lane_id,
                dataspace_id,
                stale_incarnation,
                adapter.context.height,
                1,
            );
            let canonical = store_canonical_anchor(&adapter, &proposal, &keys[0]);
            assert!(
                adapter.kura.read_lane_block_artifact(lane_id, 1).is_some(),
                "stale-incarnation fixture must still be canonical Kura data"
            );
            assert!(
                adapter.canonical_anchor_for_proposal(&canonical).is_none(),
                "canonical storage must not authorize a retired incarnation"
            );
            assert!(
                adapter
                    .canonical_proposal_for_vote_body(&canonical.vote_body(CertPhase::Prepare))
                    .is_none()
            );
            assert!(!adapter.lane_proposal_authorized(&canonical, None, true, 0));
        }
    }

    #[test]
    fn merge_signers_must_meet_both_count_and_power_quorums() {
        let (adapter, _) = fixture(wire::ConsensusMode::Npos);
        assert!(!adapter.frozen_dual_quorum_met(&[0, 1]));
        assert!(!adapter.frozen_dual_quorum_met(&[1, 2, 3]));
        assert!(adapter.frozen_dual_quorum_met(&[0, 1, 3]));
        assert!(adapter.frozen_dual_quorum_met(&[0, 1, 2, 3]));
    }

    #[test]
    fn merge_signature_state_is_bound_to_the_active_global_view() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let stale_digest = Hash::new(b"stale merge claim");
        adapter.merge_claims.insert((7, 0, 0), stale_digest);
        adapter.refresh_merge_candidates(1);
        assert!(
            adapter.merge_claims.is_empty(),
            "advancing the reducer view must retire old-view signing claims"
        );

        let stale = MergeCommitteeSignature {
            epoch_id: 7,
            view: 0,
            signer: 0,
            message_digest: stale_digest,
            bls_sig: vec![0xA5; 96],
        };
        assert_eq!(
            adapter.accept_merge_signature(stale, 1),
            V2LaneIngressOutcome::Rejected
        );
        assert!(adapter.merge_claims.is_empty());
        assert!(adapter.merge_entries.is_empty());
    }
}
