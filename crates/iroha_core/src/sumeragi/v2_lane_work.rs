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
    main_loop::lane_scheduler::{
        lane_block_redrive_leader, prepare_v2_lane_payload_plan, proposal_lookahead_enabled,
    },
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
        NativeAmxAttestationRequestV2, NativeAmxCommitRequestV2, NativeAmxMessage,
        NativeAmxSessionCache, NativeAmxSessionError, NativeAmxSessionKey, NativeAmxSigningGuard,
        NativeAmxSigningGuardError, NativeAmxVoteV2, aggregate_votes_to_qc, validate_native_amx_qc,
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

    fn native_signing_capacity(self) -> Result<NonZeroUsize, V2LaneWorkError> {
        let requested = self
            .session_capacity
            .get()
            .checked_mul(self.body_buckets_per_session.get())
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                V2LaneWorkError::InvalidContext(
                    "Native AMX signing capacity overflows the local address space".to_owned(),
                )
            })?;
        // The session/body product is an upper bound on concurrent logical
        // work, not permission to grow the crash-safety journal without bound.
        // In particular, the production defaults intentionally provision more
        // queue headroom than the durable anti-equivocation protocol ceiling.
        // Exhausting this clamped journal makes the validator abstain safely
        // until the next height; it must not make an otherwise valid node fail
        // during height construction.
        //
        // Every local signature must correspond to one authenticated request,
        // whose distinct retention table has its own explicit per-height
        // bound.  Using that bound avoids turning the much larger theoretical
        // session×leg product into gigabytes of durable journal allowance.
        NonZeroUsize::new(
            requested
                .get()
                .min(self.native_request_capacity.get())
                .min(crate::native_amx::MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD),
        )
        .ok_or_else(|| {
            V2LaneWorkError::InvalidContext(
                "Native AMX signing capacity resolved to zero".to_owned(),
            )
        })
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
    /// Durable Native AMX anti-equivocation state failed open or at runtime.
    #[error("Native AMX signing guard failed closed: {0}")]
    SigningGuard(String),
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
    native_signing_guard: NativeAmxSigningGuard,
    native_signing_guard_failure: Option<String>,
    native_signing_capacity_exhausted: bool,
    limits: V2LaneWorkLimits,
    lane_sessions: LaneBlockSessionCache,
    native_sessions: NativeAmxSessionCache,
    native_claims: BTreeMap<NativeVoteClaimKey, NativeAmxAttestationBodyV2>,
    native_claim_signatures: BTreeMap<NativeVoteClaimKey, Vec<u8>>,
    local_native_claims: BTreeMap<NativeVoteClaimKey, NativeAmxAttestationBodyV2>,
    native_requests: BTreeMap<NativeRequestKey, NativeAmxMessage>,
    authenticated_native_requests: BTreeMap<Hash, (PeerId, NativeAmxMessage)>,
    native_active_view: wire::View,
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
        let chain_id = context.chain_id.clone().into_inner();
        let native_signing_capacity = limits.native_signing_capacity()?;
        let native_signing_guard = NativeAmxSigningGuard::open(
            &kura.store_root(),
            context.height,
            context.id(),
            context.epoch,
            Hash::new(chain_id.as_bytes()),
            local_peer.clone(),
            native_signing_capacity,
        )
        .map_err(|error| V2LaneWorkError::SigningGuard(error.to_string()))?;
        let mut adapter = Self {
            context,
            local_peer,
            key_pair,
            voting_enabled,
            state,
            kura,
            native_signing_guard,
            native_signing_guard_failure: None,
            native_signing_capacity_exhausted: false,
            limits,
            lane_sessions: LaneBlockSessionCache::new(limits.session_capacity.get()),
            native_sessions: NativeAmxSessionCache::with_limits(
                limits.session_capacity,
                limits.body_buckets_per_session,
            ),
            native_claims: BTreeMap::new(),
            native_claim_signatures: BTreeMap::new(),
            local_native_claims: BTreeMap::new(),
            native_requests: BTreeMap::new(),
            authenticated_native_requests: BTreeMap::new(),
            native_active_view: 0,
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
        super::status::clear_lane_payload_ownerships();
        super::status::set_lane_payload_ownerships(adapter.canonical_ownership_status());
        adapter.hydrate_canonical_lane_artifacts();
        adapter.refresh_merge_candidates(0);
        adapter.drive_lane_sessions();
        adapter.publish_operator_status();
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
        self.publish_lane_session_status();
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

    /// Persist completed lane sessions and their canonical application evidence.
    ///
    /// A session leaves the retry queue only after its exact Kura anchor is
    /// still canonical, both QCs are durable, and the canonical transaction
    /// results have produced a receipt that verifies against the stored block.
    ///
    /// # Errors
    ///
    /// Returns [`V2LaneWorkError::Persistence`] if an anchored certificate or
    /// its application receipt cannot be written and verified durably.
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
            let persisted_result = self
                .kura
                .persist_committed_lane_block_session(&session, &pops)
                .map_err(|error| {
                    V2LaneWorkError::Persistence(format!(
                        "certified lane-block sidecar: {error}"
                    ))
                })
                .and_then(|()| {
                    self.kura
                        .persist_lane_block_application_receipt(&session.proposal)
                        .map_err(|error| {
                            V2LaneWorkError::Persistence(format!(
                                "canonical lane-block application receipt: {error}"
                            ))
                        })
                })
                .and_then(|()| {
                    self.kura
                        .lane_block_application_receipt_available(&session.proposal)
                        .then_some(())
                        .ok_or_else(|| {
                            V2LaneWorkError::Persistence(
                                "canonical lane-block application receipt failed post-write verification"
                                    .to_owned(),
                            )
                        })
                });
            if let Err(error) = persisted_result {
                // A certified sidecar may already be durable while its receipt
                // write failed. Preserve this exact session and every later
                // item so the next runner pass retries the idempotent boundary
                // instead of silently losing application evidence.
                retained.push_back(session);
                retained.append(&mut self.pending_committed_lanes);
                self.pending_committed_lanes = retained;
                self.publish_operator_status();
                return Err(error);
            }
            persisted = persisted.saturating_add(1);
        }
        self.pending_committed_lanes = retained;
        self.publish_operator_status();
        Ok(persisted)
    }

    /// Accept a lane proposal/vote/QC from the existing bounded ingress lanes.
    pub(crate) fn accept_lane_message(
        &mut self,
        inbound: InboundBlockMessage,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        if !self.advance_native_view(active_view) {
            return V2LaneIngressOutcome::Rejected;
        }
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
        if !self.advance_native_view(active_view) {
            return V2LaneIngressOutcome::Rejected;
        }
        match message {
            LaneRelayMessage::Envelope(envelope) => self.accept_lane_relay(envelope, active_view),
            LaneRelayMessage::MergeSignature(signature) => {
                self.accept_merge_signature(signature, active_view)
            }
            LaneRelayMessage::LaneDrainVote { .. }
            | LaneRelayMessage::CertifiedMergeSidecar { .. }
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

    /// Fail the serialized height runner after an unexpected durable signing
    /// journal error. Expected equivocation attempts are rejected locally and
    /// never populate this latch.
    pub(crate) fn ensure_healthy(&self) -> Result<(), V2LaneWorkError> {
        match self.native_signing_guard_failure.as_ref() {
            Some(error) => Err(V2LaneWorkError::SigningGuard(error.clone())),
            None => Ok(()),
        }
    }

    /// Publish the exact active lane-session and canonical committed-lane
    /// snapshots used by operator APIs and localnet lifecycle probes.
    pub(crate) fn publish_operator_status(&self) {
        self.publish_lane_session_status();
        super::status::set_committed_lane_blocks(self.committed_lane_status());
    }

    /// Re-enqueue bounded lane votes, QCs, and Native AMX requests for reliable
    /// point-to-point retransmission.
    pub(crate) fn schedule_retransmission(&mut self, active_view: wire::View) {
        if !self.advance_native_view(active_view) {
            return;
        }
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

    /// Advance the Native AMX round namespace and discard every artifact from
    /// the superseded view before it can consume bounded capacity or be
    /// retransmitted. Native AMX certificates bind the exact global view, so
    /// no request, vote bucket, or anti-equivocation claim is reusable after a
    /// view change.
    fn advance_native_view(&mut self, active_view: wire::View) -> bool {
        if active_view < self.native_active_view {
            return false;
        }
        if active_view == self.native_active_view {
            return true;
        }

        self.native_active_view = active_view;
        self.native_requests.clear();
        self.authenticated_native_requests.clear();
        self.native_claims.clear();
        self.native_claim_signatures.clear();
        self.local_native_claims.clear();
        self.native_sessions = NativeAmxSessionCache::with_limits(
            self.limits.session_capacity,
            self.limits.body_buckets_per_session,
        );
        self.native_retransmit_cursor = 0;

        self.effects
            .retain(|effect| !matches!(effect, V2LaneWorkEffect::PostNativeAmx { .. }));
        self.effect_keys = self.effects.iter().map(lane_work_effect_key).collect();
        true
    }

    fn accept_lane_relay(
        &mut self,
        envelope: LaneRelayEnvelope,
        global_view: wire::View,
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
                self.refresh_merge_candidates(global_view);
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
        self.publish_lane_session_status();
    }

    fn publish_lane_session_status(&self) {
        super::status::set_lane_block_sessions(self.lane_session_status());
    }

    fn lane_session_status(
        &self,
    ) -> Vec<iroha_data_model::block::consensus::SumeragiLaneBlockSessionStatus> {
        let nexus = self.state.nexus_snapshot();
        self.lane_sessions
            .status_snapshot()
            .into_iter()
            .filter(|entry| {
                self.state.lane_incarnation(entry.lane_id) == Some(entry.lane_incarnation)
                    || !nexus.enabled
            })
            .filter(|entry| {
                !nexus.enabled
                    || crate::state::nexus_active_lane_dataspace_at_height(
                        entry.lane_id,
                        &nexus,
                        self.context.height,
                    ) == Some(entry.dataspace_id)
            })
            .collect()
    }

    fn canonical_ownership_status(&self) -> Vec<SumeragiLanePayloadOwnership> {
        let nexus = self.state.nexus_snapshot();
        self.kura
            .lane_block_artifacts_snapshot()
            .into_iter()
            .map(|artifact| artifact.ownership)
            .filter(|ownership| {
                (!nexus.enabled
                    || self.state.lane_incarnation(ownership.lane_id)
                        == Some(ownership.lane_incarnation))
                    && (!nexus.enabled
                        || crate::state::nexus_active_lane_dataspace_at_height(
                            ownership.lane_id,
                            &nexus,
                            self.context.height,
                        ) == Some(ownership.dataspace_id))
            })
            .collect()
    }

    fn committed_lane_status(&self) -> Vec<super::status::CommittedLaneBlockSnapshot> {
        let mut sessions = self.state.certified_lane_block_sessions_snapshot_cached();
        let pending = self
            .pending_committed_lanes
            .iter()
            .filter(|session| self.session_has_canonical_anchor(session))
            .filter(|session| {
                !sessions
                    .iter()
                    .any(|existing| committed_lane_sessions_same_identity(existing, session))
            })
            .cloned()
            .collect::<Vec<_>>();
        sessions.extend(pending);
        sessions.sort_by_key(|session| {
            let descriptor = &session.proposal.descriptor;
            (
                descriptor.lane_block_height,
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_block_view,
                session.proposal.proposal_hash,
            )
        });
        sessions
            .iter()
            .map(|session| {
                super::status::CommittedLaneBlockSnapshot::from_committed_session_with_execution_status(
                    session,
                    committed_lane_execution_status(self.state.as_ref(), self.kura.as_ref(), session),
                )
            })
            .collect()
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
        let mut certified_proposals = BTreeSet::new();
        let certified_sessions = self
            .state
            .certified_lane_block_sessions_snapshot_cached()
            .into_iter()
            .filter(|session| {
                !self
                    .kura
                    .lane_block_application_receipt_available(&session.proposal)
                    && self.session_has_canonical_anchor(session)
            })
            .take(self.limits.session_capacity.get())
            .collect::<Vec<_>>();
        for session in certified_sessions {
            certified_proposals.insert(session.proposal.proposal_hash);
            self.pending_committed_lanes.push_back(session);
        }
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
            if certified_proposals.contains(&proposal.proposal_hash) {
                continue;
            }
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
        if !self.advance_native_view(active_view) {
            return V2LaneIngressOutcome::Rejected;
        }
        match message {
            NativeAmxMessage::PrepareRequest(request) => {
                self.accept_native_request(sender, request, None, active_view)
            }
            NativeAmxMessage::CommitRequest(request) => self.accept_native_request(
                sender,
                request.request,
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
        request: NativeAmxAttestationRequestV2,
        prepare_qc: Option<NativeAmxAttestationQcV2>,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let body = request.body;
        let expected_leader = usize::try_from(self.context.leader(body.round.view))
            .ok()
            .and_then(|index| self.context.roster.get(index))
            .map(|entry| &entry.validator);
        if !self.native_body_matches_context(&body, active_view)
            || expected_leader != Some(&sender)
            || request.validate_plan_binding().is_err()
            || !self.native_coordinator_request_matches_authority_shape(&request, &sender)
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let replay_message = match (body.phase, prepare_qc.as_ref()) {
            (NativeAmxPhase::Prepare, None) => NativeAmxMessage::PrepareRequest(request.clone()),
            (NativeAmxPhase::Commit, Some(prepare_qc)) => {
                NativeAmxMessage::CommitRequest(NativeAmxCommitRequestV2 {
                    request: request.clone(),
                    prepare_qc: prepare_qc.clone(),
                })
            }
            (NativeAmxPhase::Prepare, Some(_)) | (NativeAmxPhase::Commit, None) => {
                return V2LaneIngressOutcome::Rejected;
            }
        };
        let replay_key = native_authenticated_request_key(&sender, &replay_message);
        if let Some(outcome) =
            self.authenticated_native_request_replay(replay_key, &sender, &replay_message)
        {
            return outcome;
        }
        if self.authenticated_native_requests.len() >= self.limits.native_request_capacity.get() {
            return V2LaneIngressOutcome::Rejected;
        }
        let Some((validators, min_signers)) = self.native_committee_shape(&body) else {
            return V2LaneIngressOutcome::Rejected;
        };
        if !validators.contains(&self.local_peer) {
            return V2LaneIngressOutcome::Rejected;
        }

        // Every sender/context/request/committee gate above is deliberately
        // cheaper than PoP, vote-signature, or aggregate-QC verification. An
        // exact request replay is recognized only from this view's previously
        // authenticated full envelope and sender.
        if !self.native_coordinator_request_is_authoritative(&request, &sender) {
            return V2LaneIngressOutcome::Rejected;
        }
        let Some((verified_validators, verified_min_signers, pops, _)) =
            self.native_committee(&body)
        else {
            return V2LaneIngressOutcome::Rejected;
        };
        if verified_validators != validators || verified_min_signers != min_signers {
            return V2LaneIngressOutcome::Rejected;
        }
        match body.phase {
            NativeAmxPhase::Commit => {
                let prepare_qc = prepare_qc.expect("Commit replay shape checked above");
                let request = NativeAmxCommitRequestV2 {
                    request: request.clone(),
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
            peer: sender.clone(),
            message: match body.phase {
                NativeAmxPhase::Prepare => NativeAmxMessage::PrepareVote(vote),
                NativeAmxPhase::Commit => NativeAmxMessage::CommitVote(vote),
            },
        }) {
            return V2LaneIngressOutcome::Rejected;
        }
        self.authenticated_native_requests
            .insert(replay_key, (sender, replay_message));
        V2LaneIngressOutcome::Inserted
    }

    fn accept_native_vote(
        &mut self,
        sender: PeerId,
        vote: NativeAmxVoteV2,
        expected_phase: NativeAmxPhase,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        // Reject unauthenticated transport/context/request/committee drift
        // before parsing or verifying an attacker-controlled BLS signature.
        if vote
            .validate_ingress_shape(expected_phase, Some(&sender))
            .is_err()
            || !self.native_body_matches_context(&vote.body, active_view)
            || !self.native_request_was_sent_to_vote_signer(&vote, expected_phase)
        {
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
        if let Some(outcome) = self.authenticated_native_vote_replay(key, &vote) {
            return outcome;
        }
        let Some((validators, _)) = self.native_committee_shape(&vote.body) else {
            return V2LaneIngressOutcome::Rejected;
        };
        if !validators.contains(&vote.signer) {
            return V2LaneIngressOutcome::Rejected;
        }
        let claim_capacity = self
            .limits
            .session_capacity
            .get()
            .saturating_mul(self.limits.body_buckets_per_session.get())
            .saturating_mul(crate::native_amx::MAX_NATIVE_AMX_VALIDATORS);
        if self.native_claims.len() >= claim_capacity {
            return V2LaneIngressOutcome::Rejected;
        }
        // Only an exact replay of a previously authenticated full vote can
        // bypass BLS/PoP verification. A same-claim envelope with any changed
        // body or signature is rejected without replacing the retained proof.
        if vote.verify_signature().is_err() || self.native_committee(&vote.body).is_none() {
            return V2LaneIngressOutcome::Rejected;
        }
        let body = vote.body;
        let signature = vote.bls_signature.clone();
        match self.native_sessions.insert_vote(vote) {
            Ok(()) => {
                self.native_claims.insert(key, body);
                self.native_claim_signatures.insert(key, signature);
                V2LaneIngressOutcome::Inserted
            }
            Err(NativeAmxSessionError::DuplicateSigner) => {
                self.native_claims.insert(key, body);
                self.native_claim_signatures.insert(key, signature);
                V2LaneIngressOutcome::Duplicate
            }
            Err(
                NativeAmxSessionError::PhaseMismatch
                | NativeAmxSessionError::PlanEquivocation
                | NativeAmxSessionError::Capacity,
            ) => V2LaneIngressOutcome::Rejected,
        }
    }

    fn authenticated_native_request_replay(
        &self,
        key: Hash,
        sender: &PeerId,
        message: &NativeAmxMessage,
    ) -> Option<V2LaneIngressOutcome> {
        self.authenticated_native_requests
            .get(&key)
            .map(|(accepted_sender, accepted_message)| {
                if accepted_sender == sender && accepted_message == message {
                    V2LaneIngressOutcome::Duplicate
                } else {
                    V2LaneIngressOutcome::Rejected
                }
            })
    }

    fn authenticated_native_vote_replay(
        &self,
        key: NativeVoteClaimKey,
        vote: &NativeAmxVoteV2,
    ) -> Option<V2LaneIngressOutcome> {
        self.native_claims.get(&key).map(|existing| {
            if existing == &vote.body
                && self
                    .native_claim_signatures
                    .get(&key)
                    .is_some_and(|signature| signature == &vote.bls_signature)
            {
                V2LaneIngressOutcome::Duplicate
            } else {
                V2LaneIngressOutcome::Rejected
            }
        })
    }

    fn native_body_matches_context(
        &self,
        body: &NativeAmxAttestationBodyV2,
        active_view: wire::View,
    ) -> bool {
        let nexus_enabled = self.state.nexus_snapshot().enabled;
        body.round.context_id == self.context.id()
            && body.round.height == self.context.height
            && body.round.view == active_view
            && body.epoch == self.context.epoch
            && body.chain_id_hash == self.native_chain_id_hash()
            && body.authority_context_height == self.context.height
            && body.coordinator_lane_block_view == body.round.view
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
            && (!nexus_enabled
                || self.state.lane_incarnation_at_height(
                    body.coordinator_lane_id,
                    body.authority_context_height,
                ) == Some(body.coordinator_lane_incarnation))
            && (!nexus_enabled
                || self.state.lane_incarnation_at_height(
                    body.participant_lane_id,
                    body.authority_context_height,
                ) == Some(body.participant_lane_incarnation))
    }

    fn native_request_was_sent_to_vote_signer(
        &self,
        vote: &NativeAmxVoteV2,
        expected_phase: NativeAmxPhase,
    ) -> bool {
        let key = NativeRequestKey {
            body: vote.body,
            peer: vote.signer.clone(),
        };
        self.native_requests
            .get(&key)
            .is_some_and(|message| match (expected_phase, message) {
                (NativeAmxPhase::Prepare, NativeAmxMessage::PrepareRequest(request)) => {
                    request.body == vote.body
                }
                (NativeAmxPhase::Commit, NativeAmxMessage::CommitRequest(request)) => {
                    request.request.body == vote.body
                }
                _ => false,
            })
    }

    fn native_chain_id_hash(&self) -> Hash {
        let chain_id = self.context.chain_id.clone().into_inner();
        Hash::new(chain_id.as_bytes())
    }

    fn native_coordinator_request_is_authoritative(
        &self,
        request: &NativeAmxAttestationRequestV2,
        sender: &PeerId,
    ) -> bool {
        let body = &request.body;
        let Some((validators, min_signers, _, _)) = self.native_committee_for_route(
            body.coordinator_lane_id,
            body.coordinator_dataspace_id,
            body.authority_context_height,
        ) else {
            return false;
        };
        let Some((previous_height, previous_hash)) = self.native_coordinator_predecessor(body)
        else {
            return false;
        };
        native_coordinator_proposal_matches_authority(
            request,
            sender,
            &validators,
            min_signers,
            previous_height,
            previous_hash,
        )
    }

    fn native_coordinator_request_matches_authority_shape(
        &self,
        request: &NativeAmxAttestationRequestV2,
        sender: &PeerId,
    ) -> bool {
        let body = &request.body;
        let Some((validators, min_signers)) = self.native_committee_shape_for_route(
            body.coordinator_lane_id,
            body.coordinator_dataspace_id,
            body.authority_context_height,
        ) else {
            return false;
        };
        let Some((previous_height, previous_hash)) = self.native_coordinator_predecessor(body)
        else {
            return false;
        };
        native_coordinator_proposal_matches_authority(
            request,
            sender,
            &validators,
            min_signers,
            previous_height,
            previous_hash,
        )
    }

    fn native_coordinator_predecessor(
        &self,
        body: &NativeAmxAttestationBodyV2,
    ) -> Option<(u64, Option<Hash>)> {
        let Some(artifact) = self.kura.latest_lane_block_artifact_for_dataspace(
            body.coordinator_lane_id,
            body.coordinator_dataspace_id,
        ) else {
            return Some((0, None));
        };
        let ownership = artifact.ownership;
        if ownership.lane_incarnation != body.coordinator_lane_incarnation {
            // A recreated lane starts a fresh height-zero predecessor
            // namespace; retired-incarnation Kura entries are never inherited.
            return Some((0, None));
        }
        if ownership.proposal_height >= body.authority_context_height {
            return None;
        }
        Some((
            ownership.lane_block_height,
            Some(ownership.lane_block_descriptor_hash?),
        ))
    }

    fn native_coordinator_height_is_current(&self, body: &NativeAmxAttestationBodyV2) -> bool {
        self.native_coordinator_predecessor(body)
            .and_then(|(height, _)| height.checked_add(1))
            == Some(body.planned_coordinator_block_height)
    }

    fn native_committee(
        &self,
        body: &NativeAmxAttestationBodyV2,
    ) -> Option<(
        Vec<PeerId>,
        usize,
        BTreeMap<PublicKey, Vec<u8>>,
        Vec<Vec<u8>>,
    )> {
        let (validators, min_signers, pops, aligned_pops) = self.native_committee_for_route(
            body.participant_lane_id,
            body.participant_dataspace_id,
            body.authority_context_height,
        )?;
        if body.participant_validator_set_hash != HashOf::new(&validators)
            || usize::try_from(body.participant_validator_count).ok() != Some(validators.len())
            || usize::try_from(body.participant_min_quorum).ok() != Some(min_signers)
        {
            return None;
        }
        Some((validators, min_signers, pops, aligned_pops))
    }

    fn native_committee_shape(
        &self,
        body: &NativeAmxAttestationBodyV2,
    ) -> Option<(Vec<PeerId>, usize)> {
        let (validators, min_signers) = self.native_committee_shape_for_route(
            body.participant_lane_id,
            body.participant_dataspace_id,
            body.authority_context_height,
        )?;
        if body.participant_validator_set_hash != HashOf::new(&validators)
            || usize::try_from(body.participant_validator_count).ok() != Some(validators.len())
            || usize::try_from(body.participant_min_quorum).ok() != Some(min_signers)
        {
            return None;
        }
        Some((validators, min_signers))
    }

    fn native_committee_shape_for_route(
        &self,
        participant_lane: LaneId,
        participant_dataspace: DataSpaceId,
        authority_height: u64,
    ) -> Option<(Vec<PeerId>, usize)> {
        let mut validators = self
            .state
            .authoritative_lane_peer_ids_at_height(participant_lane, authority_height);
        validators.sort();
        if validators.is_empty()
            || validators.len() > crate::native_amx::MAX_NATIVE_AMX_VALIDATORS
            || validators.windows(2).any(|pair| pair[0] >= pair[1])
            || validators
                .iter()
                .any(|peer| peer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal))
        {
            return None;
        }
        let nexus = self.state.nexus_snapshot();
        let fault_tolerance = nexus
            .dataspace_catalog
            .entries()
            .iter()
            .find(|entry| entry.id == participant_dataspace)?
            .fault_tolerance;
        let minimum_committee =
            usize::try_from(fault_tolerance.checked_mul(3)?.checked_add(1)?).ok()?;
        if validators.len() < minimum_committee {
            return None;
        }
        let min_signers = super::network_topology::commit_quorum_from_len(validators.len()).max(1);
        Some((validators, min_signers))
    }

    fn native_committee_for_route(
        &self,
        participant_lane: LaneId,
        participant_dataspace: DataSpaceId,
        authority_height: u64,
    ) -> Option<(
        Vec<PeerId>,
        usize,
        BTreeMap<PublicKey, Vec<u8>>,
        Vec<Vec<u8>>,
    )> {
        let (validators, min_signers) = self.native_committee_shape_for_route(
            participant_lane,
            participant_dataspace,
            authority_height,
        )?;
        let pinned = super::main_loop::pinned_autoscale_validator_pops_for_set(
            &self.state,
            participant_lane,
            &validators,
        )?;
        let aligned_pops = if let Some(pops) = pinned {
            pops
        } else {
            let world = self.state.world_view();
            validators
                .iter()
                .map(|peer| {
                    let pop = crate::state::live_consensus_key_pop_for_peer(
                        &world,
                        peer,
                        authority_height,
                    )?;
                    iroha_crypto::bls_normal_pop_verify(peer.public_key(), &pop).ok()?;
                    Some(pop)
                })
                .collect::<Option<Vec<_>>>()?
        };
        let pops = verified_native_committee_pops(&validators, &aligned_pops)?;
        Some((validators, min_signers, pops, aligned_pops))
    }

    fn sign_native_vote_once(
        &mut self,
        body: NativeAmxAttestationBodyV2,
    ) -> Option<NativeAmxVoteV2> {
        if self.native_signing_guard_failure.is_some()
            || !self.voting_enabled
            || self.local_peer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
            || !self.native_body_matches_context(&body, self.native_active_view)
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
            let capacity = self.limits.native_signing_capacity().ok()?.get();
            if self.local_native_claims.len() >= capacity {
                return None;
            }
        }
        match self.native_signing_guard.record(&body) {
            Ok(()) => {}
            Err(
                NativeAmxSigningGuardError::Equivocation
                | NativeAmxSigningGuardError::PlanEquivocation,
            ) => return None,
            Err(NativeAmxSigningGuardError::Capacity) => {
                if !self.native_signing_capacity_exhausted {
                    iroha_logger::warn!(
                        height = body.round.height,
                        view = body.round.view,
                        "Native AMX signing work exhausted its bounded durable journal"
                    );
                    self.native_signing_capacity_exhausted = true;
                }
                return None;
            }
            Err(error) => {
                let message = error.to_string();
                if self.native_signing_guard_failure.is_none() {
                    iroha_logger::error!(
                        %error,
                        height = body.round.height,
                        view = body.round.view,
                        "Native AMX signing guard failed closed"
                    );
                    self.native_signing_guard_failure = Some(message);
                }
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
        let plan_legs = candidate.routing_plan().legs();
        let mut prepared = Vec::with_capacity(plan.participants.len());
        for participant in &plan.participants {
            let (validators, min_signers, pops, aligned_pops) = self.native_committee_for_route(
                participant.route.lane_id,
                participant.route.dataspace_id,
                self.context.height,
            )?;
            let participant_lane_incarnation = self
                .state
                .lane_incarnation_at_height(participant.route.lane_id, self.context.height)?;
            let prepare_body = NativeAmxAttestationBodyV2 {
                round,
                epoch: self.context.epoch,
                chain_id_hash: self.native_chain_id_hash(),
                source_id,
                tx_entrypoint_hash: candidate.entrypoint_hash(),
                plan_digest: plan.plan_digest,
                phase: NativeAmxPhase::Prepare,
                coordinator_lane_id: plan.coordinator.route.lane_id,
                coordinator_dataspace_id: plan.coordinator.route.dataspace_id,
                coordinator_lane_incarnation: coordinator_descriptor.lane_incarnation,
                participant_lane_id: participant.route.lane_id,
                participant_dataspace_id: participant.route.dataspace_id,
                participant_lane_incarnation,
                participant_validator_set_hash: HashOf::new(&validators),
                participant_validator_count: u32::try_from(validators.len()).ok()?,
                participant_min_quorum: u32::try_from(min_signers).ok()?,
                authority_context_height: self.context.height,
                planned_coordinator_block_height: coordinator_descriptor.lane_block_height,
                coordinator_lane_block_view: coordinator_descriptor.lane_block_view,
                coordinator_proposal_hash: coordinator_proposal.proposal_hash,
            };
            let request = NativeAmxAttestationRequestV2 {
                body: prepare_body,
                plan_legs: plan_legs.clone(),
                coordinator_proposal: coordinator_proposal.clone(),
            };
            if request.validate_plan_binding().is_err()
                || !self.native_coordinator_request_is_authoritative(&request, &self.local_peer)
            {
                return None;
            }
            self.ensure_native_prepare_requests(&request, &validators);
            prepared.push((
                *participant,
                request,
                validators,
                min_signers,
                pops,
                aligned_pops,
            ));
        }

        let mut certified_prepares = Vec::with_capacity(prepared.len());
        for (participant, prepare_request, validators, min_signers, pops, aligned_pops) in prepared
        {
            let prepare_body = prepare_request.body;
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
                aligned_pops.clone(),
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
            let mut commit_request = prepare_request;
            commit_request.body.phase = NativeAmxPhase::Commit;
            self.ensure_native_commit_requests(&commit_request, &prepare_qc, &validators);
            certified_prepares.push((
                participant,
                commit_request,
                validators,
                min_signers,
                pops,
                aligned_pops,
                prepare_qc,
            ));
        }

        let mut legs = Vec::with_capacity(certified_prepares.len());
        for (
            participant,
            commit_request,
            validators,
            min_signers,
            pops,
            aligned_pops,
            prepare_qc,
        ) in certified_prepares
        {
            let commit_body = commit_request.body;
            let commit_votes =
                self.native_sessions
                    .sorted_votes_for_body_from(session, &commit_body, &validators);
            if commit_votes.len() < min_signers {
                return None;
            }
            let commit_qc = aggregate_votes_to_qc(
                commit_body,
                validators.clone(),
                aligned_pops,
                &commit_votes,
                min_signers,
            )
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
        request: &NativeAmxAttestationRequestV2,
        validators: &[PeerId],
    ) {
        let body = request.body;
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
                NativeAmxMessage::PrepareRequest(request.clone()),
            );
        }
    }

    fn ensure_native_commit_requests(
        &mut self,
        request: &NativeAmxAttestationRequestV2,
        prepare_qc: &NativeAmxAttestationQcV2,
        validators: &[PeerId],
    ) {
        let body = request.body;
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
                    request: request.clone(),
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
        let message_body = match &message {
            NativeAmxMessage::PrepareRequest(request) => Some(request.body),
            NativeAmxMessage::CommitRequest(request) => Some(request.request.body),
            NativeAmxMessage::PrepareVote(_) | NativeAmxMessage::CommitVote(_) => None,
        };
        if body.round.view != self.native_active_view || message_body != Some(body) {
            return;
        }
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

    fn refresh_merge_candidates(&mut self, global_view: wire::View) {
        let candidates = self
            .state
            .merge_entry_candidates_from_lane_relays_for_view(global_view);
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
        global_view: wire::View,
    ) -> V2LaneIngressOutcome {
        self.refresh_merge_candidates(global_view);
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
        if !self.advance_native_view(view) {
            return Err(all_unavailable(candidates.len(), "stale Native AMX view"));
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
        super::status::set_lane_payload_ownerships(lane_plan.ownerships.clone());
        Ok(PreparedCandidateWork {
            native_amx_receipts: receipts,
            lane_payload_ownerships: lane_plan.ownerships,
        })
    }
}

fn committed_lane_sessions_same_identity(
    left: &CommittedLaneBlockSession,
    right: &CommittedLaneBlockSession,
) -> bool {
    left.proposal == right.proposal
        && left.prepare_qc == right.prepare_qc
        && left.commit_qc == right.commit_qc
}

fn committed_lane_execution_status(
    state: &State,
    kura: &Kura,
    session: &CommittedLaneBlockSession,
) -> super::status::CommittedLaneBlockExecutionStatus {
    use super::status::CommittedLaneBlockExecutionStatus as Status;

    let proposal = &session.proposal;
    if kura.lane_block_application_receipt_available(proposal) {
        let descriptor = &proposal.descriptor;
        return match kura
            .read_lane_block_application_receipt(descriptor.lane_id, descriptor.lane_block_height)
        {
            Some(receipt)
                if receipt.format
                    == crate::kura::LaneBlockApplicationReceiptArtifactFormat::DirectExecution =>
            {
                Status::StateAppliedByDirectExecution
            }
            _ => Status::StateAppliedByCanonicalBlock,
        };
    }
    if kura.lane_block_application_receipt_conflicts_with_preflight(proposal) {
        return Status::ApplicationReceiptConflictsWithPreflight;
    }
    if !kura.lane_block_predecessor_application_receipt_available(proposal) {
        return Status::AwaitingPredecessorApplication;
    }
    let current_height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
    let current_hash = Some(state.lane_execution_state_hash());
    if kura
        .read_preflighted_lane_block_execution_input_for_application(
            proposal,
            current_height,
            current_hash,
        )
        .is_some()
    {
        return Status::PayloadPreflightedAwaitingStateApplication;
    }
    if kura.lane_block_execution_preflight_has_rejections(proposal, current_height, current_hash)
        == Some(true)
    {
        return Status::PayloadPreflightRejectedAwaitingStateApplication;
    }
    if kura.lane_block_execution_input_available(proposal) {
        return Status::PayloadRecoveredAwaitingStateApplication;
    }
    if kura
        .lane_block_payload_availability(proposal)
        .is_available()
    {
        return Status::PayloadAvailableAwaitingExecutor;
    }
    Status::AwaitingExecutablePayload
}

fn all_unavailable(count: usize, reason: impl Into<String>) -> CandidateWorkUnavailable {
    CandidateWorkUnavailable::new((0..count).collect(), reason)
}

fn verified_native_committee_pops(
    validators: &[PeerId],
    aligned_pops: &[Vec<u8>],
) -> Option<BTreeMap<PublicKey, Vec<u8>>> {
    if aligned_pops.len() != validators.len()
        || validators.iter().zip(aligned_pops).any(|(peer, pop)| {
            pop.len() != crate::native_amx::NATIVE_AMX_BLS_PROOF_BYTES
                || iroha_crypto::bls_normal_pop_verify(peer.public_key(), pop.as_slice()).is_err()
        })
    {
        return None;
    }
    Some(
        validators
            .iter()
            .zip(aligned_pops)
            .map(|(peer, pop)| (peer.public_key().clone(), pop.clone()))
            .collect(),
    )
}

#[allow(clippy::too_many_arguments)]
fn native_coordinator_proposal_matches_authority(
    request: &NativeAmxAttestationRequestV2,
    sender: &PeerId,
    expected_validators: &[PeerId],
    expected_min_signers: usize,
    expected_previous_height: u64,
    expected_previous_hash: Option<Hash>,
) -> bool {
    if request.validate_plan_binding().is_err()
        || expected_validators.is_empty()
        || expected_validators.len() > crate::native_amx::MAX_NATIVE_AMX_VALIDATORS
        || expected_validators
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || expected_validators
            .iter()
            .any(|peer| peer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal))
    {
        return false;
    }
    let exact_commit_quorum =
        super::network_topology::commit_quorum_from_len(expected_validators.len()).max(1);
    let Ok(expected_count) = u32::try_from(expected_validators.len()) else {
        return false;
    };
    let Ok(expected_quorum) = u32::try_from(exact_commit_quorum) else {
        return false;
    };
    let Some(expected_lane_height) = expected_previous_height.checked_add(1) else {
        return false;
    };
    let descriptor = &request.coordinator_proposal.descriptor;
    expected_min_signers == exact_commit_quorum
        && descriptor.validator_set_hash_version == VALIDATOR_SET_HASH_VERSION_V1
        && descriptor.validator_set == expected_validators
        && descriptor.validator_set_hash == HashOf::new(&expected_validators.to_vec())
        && descriptor.validator_count == expected_count
        && descriptor.min_quorum == expected_quorum
        && descriptor.previous_lane_block_height == expected_previous_height
        && descriptor.previous_lane_block_descriptor_hash == expected_previous_hash
        && descriptor.lane_block_height == expected_lane_height
        && lane_block_redrive_leader(&request.coordinator_proposal, 0) == Some(sender)
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

fn native_authenticated_request_key(sender: &PeerId, message: &NativeAmxMessage) -> Hash {
    let mut encoded = b"iroha:sumeragi:v2:native-amx:authenticated-request:v1\0".to_vec();
    encoded.extend(sender.encode());
    encoded.extend(message.encode());
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

#[cfg(all(test, unix))]
mod tests {
    use std::{borrow::Cow, num::NonZeroUsize, sync::Arc};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, bls_normal_pop_prove};
    use iroha_data_model::{
        ChainId, Level,
        block::{
            BlockExecutionContextBundle, ExternalExecutionContext, SignedBlock,
            consensus::{NativeAmxAttestationBodyV2, NativeAmxPhase},
            consensus_v2 as wire,
        },
        isi::Log,
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
        transaction::{TransactionBuilder, TransactionEntrypoint, signed::TransactionResultInner},
        trigger::DataTriggerSequence,
    };
    use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};

    use super::*;
    use crate::{
        block::BlockBuilder, query::store::LiveQueryStore, state::World, tx::AcceptedTransaction,
    };

    fn fixture_with_limits(
        mode: wire::ConsensusMode,
        limits: V2LaneWorkLimits,
    ) -> Result<(V2LaneWorkAdapter, Vec<KeyPair>), V2LaneWorkError> {
        fixture_with_limits_and_voting(mode, limits, true)
    }

    fn fixture_with_limits_and_voting(
        mode: wire::ConsensusMode,
        limits: V2LaneWorkLimits,
        voting_enabled: bool,
    ) -> Result<(V2LaneWorkAdapter, Vec<KeyPair>), V2LaneWorkError> {
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
        for key in &keys {
            let pop = bls_normal_pop_prove(key.private_key())
                .expect("deterministic v2 lane-work validator PoP");
            world.register_validator_pop_for_testing(key.public_key().clone(), pop);
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
            height: 9,
            epoch: 4,
            epoch_end_height: 20,
            mode,
            parent_commit_qc: Some(wire::QuorumCertificate {
                round: wire::ConsensusRound {
                    context_id: wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
                        b"v2-lane-work-parent-context",
                    ))),
                    height: 8,
                    view: 0,
                },
                phase: wire::GlobalPhase::Commit,
                subject: wire::BlockSubject {
                    parent_block_hash: None,
                    block_hash: HashOf::from_untyped_unchecked(Hash::new(
                        b"v2-lane-work-parent-block",
                    )),
                    payload_hash: Hash::new(b"v2-lane-work-parent-payload"),
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
        let local_index = usize::try_from(context.leader(0)).expect("leader index");
        let local_key = keys[local_index].clone();
        let local_peer = PeerId::new(local_key.public_key().clone());
        let adapter = V2LaneWorkAdapter::new(
            context,
            local_peer,
            local_key,
            voting_enabled,
            state,
            kura,
            limits,
        )?;
        Ok((adapter, keys))
    }

    fn fixture(mode: wire::ConsensusMode) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
        let nonzero = NonZeroUsize::new(8).expect("nonzero");
        fixture_with_limits(
            mode,
            V2LaneWorkLimits::new(nonzero, nonzero, nonzero, nonzero, nonzero, nonzero),
        )
        .expect("open lane adapter")
    }

    fn native_body(adapter: &V2LaneWorkAdapter) -> NativeAmxAttestationBodyV2 {
        let entrypoint_hash = Hash::new(b"entrypoint");
        let mut source_id = [0_u8; Hash::LENGTH];
        source_id.copy_from_slice(entrypoint_hash.as_ref());
        let validators = adapter.frozen_validator_set();
        NativeAmxAttestationBodyV2 {
            round: wire::ConsensusRound {
                context_id: adapter.context.id(),
                height: adapter.context.height,
                view: 0,
            },
            epoch: adapter.context.epoch,
            chain_id_hash: adapter.native_chain_id_hash(),
            source_id,
            tx_entrypoint_hash: HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
                entrypoint_hash,
            ),
            plan_digest: Hash::new(b"plan"),
            phase: NativeAmxPhase::Prepare,
            coordinator_lane_id: LaneId::new(1),
            coordinator_dataspace_id: DataSpaceId::new(7),
            coordinator_lane_incarnation: Hash::new(b"v2-lane-work-coordinator-incarnation"),
            participant_lane_id: LaneId::new(2),
            participant_dataspace_id: DataSpaceId::new(8),
            participant_lane_incarnation: Hash::new(b"v2-lane-work-participant-incarnation"),
            participant_validator_set_hash: HashOf::new(&validators),
            participant_validator_count: u32::try_from(validators.len())
                .expect("fixture validator count"),
            participant_min_quorum: u32::try_from(
                crate::sumeragi::network_topology::commit_quorum_from_len(validators.len()).max(1),
            )
            .expect("fixture validator quorum"),
            authority_context_height: adapter.context.height,
            planned_coordinator_block_height: 1,
            coordinator_lane_block_view: 0,
            coordinator_proposal_hash: Hash::new(b"v2-lane-work-coordinator-proposal"),
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

    fn signed_lane_vote(
        proposal: &LaneBlockProposalV1,
        phase: CertPhase,
        key: &KeyPair,
    ) -> LaneBlockVoteV1 {
        let body = proposal.vote_body(phase);
        let signature = Signature::try_new(key.private_key(), &body.signature_preimage())
            .expect("lane status fixture signature");
        LaneBlockVoteV1 {
            body,
            payload_availability_vote: None,
            signer: PeerId::new(key.public_key().clone()),
            bls_signature: signature.payload().to_vec(),
        }
    }

    fn committed_lane_session(
        proposal: LaneBlockProposalV1,
        keys: &[KeyPair],
    ) -> CommittedLaneBlockSession {
        let validator_set = proposal.descriptor.validator_set.clone();
        let min_quorum = usize::try_from(proposal.descriptor.min_quorum)
            .expect("lane fixture quorum fits usize");
        let prepare_votes = keys
            .iter()
            .take(min_quorum)
            .map(|key| signed_lane_vote(&proposal, CertPhase::Prepare, key))
            .collect::<Vec<_>>();
        let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            proposal.vote_body(CertPhase::Prepare),
            validator_set.clone(),
            &prepare_votes,
        )
        .expect("aggregate canonical prepare QC");
        let commit_votes = keys
            .iter()
            .take(min_quorum)
            .map(|key| signed_lane_vote(&proposal, CertPhase::Commit, key))
            .collect::<Vec<_>>();
        let commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            proposal.vote_body(CertPhase::Commit),
            validator_set,
            &commit_votes,
        )
        .expect("aggregate canonical commit QC");
        CommittedLaneBlockSession {
            proposal,
            prepare_qc,
            commit_qc,
        }
    }

    fn anchored_committed_lane_session(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
    ) -> CommittedLaneBlockSession {
        let transaction = TransactionBuilder::new(
            ChainId::from("v2-lane-work-canonical"),
            SAMPLE_GENESIS_ACCOUNT_ID.to_owned(),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "v2 lane application receipt".to_owned(),
        )])
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(transaction));
        let mut block: SignedBlock = BlockBuilder::new(vec![accepted])
            .chain(0, None)
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .unpack(|_| {})
            .into();
        let entrypoint_hash = block
            .external_entrypoints_cloned()
            .next()
            .expect("canonical lane fixture entrypoint")
            .hash();
        let validator_set = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let validator_count =
            u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
        let min_quorum = u32::try_from(
            crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()).max(1),
        )
        .expect("fixture lane quorum fits u32");
        let mut ownership = SumeragiLanePayloadOwnership {
            proposal_height: block.header().height().get(),
            proposal_view: block.header().view_change_index(),
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_incarnation: Hash::new(b"v2-lane-work-canonical-incarnation"),
            lane_block_height: 1,
            lane_block_view: block.header().view_change_index(),
            subject_hash: Hash::new(b"v2-lane-work-subject-placeholder"),
            qc_mode_tag: "permissioned:v2-lane-work-canonical".to_owned(),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::from(entrypoint_hash)],
            previous_lane_block_height: 0,
            previous_lane_block_descriptor_hash: None,
            lane_block_descriptor_hash: Some(Hash::new(b"v2-lane-work-descriptor-placeholder")),
            lane_block_descriptor_validator_set: validator_set,
            lane_block_descriptor_validator_count: validator_count,
            lane_block_descriptor_min_quorum: min_quorum,
            payload_ownership_hash: Hash::new(b"v2-lane-work-ownership-placeholder"),
            rbc_instance_hash: Hash::new(b"v2-lane-work-rbc-placeholder"),
        };
        let replay_hashes = ownership
            .compute_replay_hashes()
            .expect("compute canonical lane replay hashes");
        ownership.subject_hash = replay_hashes.subject_hash;
        ownership.payload_ownership_hash = replay_hashes.payload_ownership_hash;
        ownership.rbc_instance_hash = replay_hashes.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay_hashes.lane_block_descriptor_hash);

        let execution_context =
            BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                entrypoint_hash,
                ownership.lane_id,
                ownership.dataspace_id,
            )])
            .with_lane_payload_ownerships(vec![ownership.clone()]);
        block.set_execution_context(Some(execution_context));
        block
            .set_transaction_results(
                Vec::new(),
                &[entrypoint_hash],
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("attach deterministic canonical lane result");
        let proposal = proposal_from_ownership(&ownership, block.hash())
            .expect("reconstruct canonical lane proposal");
        adapter
            .kura
            .store_block(Arc::new(block))
            .expect("store canonical lane carrier block");
        committed_lane_session(proposal, keys)
    }

    #[test]
    fn anchored_lane_session_persists_verified_receipt_idempotently_across_adapter_restart() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let session = anchored_committed_lane_session(&adapter, &keys);
        let proposal = session.proposal.clone();
        let lane_id = proposal.descriptor.lane_id;
        let lane_block_height = proposal.descriptor.lane_block_height;
        adapter.pending_committed_lanes.push_back(session.clone());

        assert_eq!(adapter.persist_anchored_sessions(), Ok(1));
        assert!(adapter.pending_committed_lanes.is_empty());
        assert!(
            adapter
                .kura
                .read_certified_lane_block_artifact(lane_id, lane_block_height)
                .is_some()
        );
        assert!(
            adapter
                .kura
                .lane_block_application_receipt_available(&proposal)
        );
        assert_eq!(
            committed_lane_execution_status(
                adapter.state.as_ref(),
                adapter.kura.as_ref(),
                &session,
            ),
            super::super::status::CommittedLaneBlockExecutionStatus::StateAppliedByCanonicalBlock
        );
        let receipt = adapter
            .kura
            .read_lane_block_application_receipt(lane_id, lane_block_height)
            .expect("canonical application receipt");

        let context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let key_pair = adapter.key_pair.clone();
        let voting_enabled = adapter.voting_enabled;
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);
        let mut restarted = V2LaneWorkAdapter::new(
            context,
            local_peer,
            key_pair,
            voting_enabled,
            state,
            kura,
            limits,
        )
        .expect("restart lane adapter");
        assert!(
            restarted.lane_sessions.status_snapshot().is_empty(),
            "durably applied canonical work must not rehydrate after restart"
        );

        restarted.pending_committed_lanes.push_back(session);
        assert_eq!(restarted.persist_anchored_sessions(), Ok(1));
        assert!(restarted.pending_committed_lanes.is_empty());
        assert_eq!(
            restarted
                .kura
                .read_lane_block_application_receipt(lane_id, lane_block_height),
            Some(receipt),
            "the exact receipt boundary must be idempotent"
        );
    }

    #[test]
    fn canonical_receipt_write_failure_is_fail_closed_and_restart_retryable() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let session = anchored_committed_lane_session(&adapter, &keys);
        let proposal = session.proposal.clone();
        let lane_id = proposal.descriptor.lane_id;
        let lane_block_height = proposal.descriptor.lane_block_height;
        adapter.pending_committed_lanes.push_back(session);
        adapter
            .kura
            .fail_next_lane_block_application_receipt_write_for_tests();

        assert!(matches!(
            adapter.persist_anchored_sessions(),
            Err(V2LaneWorkError::Persistence(message))
                if message.contains("canonical lane-block application receipt")
        ));
        assert_eq!(adapter.pending_committed_lanes.len(), 1);
        assert!(
            adapter
                .kura
                .read_certified_lane_block_artifact(lane_id, lane_block_height)
                .is_some(),
            "the certified sidecar may commit before the receipt failure"
        );
        assert!(
            !adapter
                .kura
                .lane_block_application_receipt_available(&proposal),
            "a failed receipt write must never advertise applied state"
        );

        let context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let key_pair = adapter.key_pair.clone();
        let voting_enabled = adapter.voting_enabled;
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        let mut restarted = V2LaneWorkAdapter::new(
            context,
            local_peer,
            key_pair,
            voting_enabled,
            state,
            kura,
            limits,
        )
        .expect("restart lane adapter after partial receipt persistence");
        assert_eq!(
            restarted.pending_committed_lanes.len(),
            1,
            "restart must re-queue the exact certified session whose receipt is absent"
        );
        assert_eq!(restarted.persist_anchored_sessions(), Ok(1));
        assert!(restarted.pending_committed_lanes.is_empty());
        assert!(
            restarted
                .kura
                .lane_block_application_receipt_available(&proposal)
        );
    }

    #[test]
    fn missing_or_mismatched_canonical_anchor_never_writes_durable_evidence() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let canonical = anchored_committed_lane_session(&adapter, &keys);
        let lane_id = canonical.proposal.descriptor.lane_id;
        let lane_block_height = canonical.proposal.descriptor.lane_block_height;

        let missing_session = committed_lane_session(coordinator_proposal(&adapter, &keys), &keys);
        let missing_lane_id = missing_session.proposal.descriptor.lane_id;
        let missing_lane_block_height = missing_session.proposal.descriptor.lane_block_height;

        let mut mismatched_hint = canonical.proposal.clone();
        mismatched_hint
            .payload_block_hint
            .as_mut()
            .expect("canonical proposal has a payload hint")
            .proposal_block_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"v2-lane-work-wrong-canonical-anchor"));
        mismatched_hint.proposal_hash = mismatched_hint.computed_proposal_hash();
        let mismatched_session = committed_lane_session(mismatched_hint, &keys);

        adapter
            .pending_committed_lanes
            .extend([missing_session, mismatched_session]);
        assert_eq!(adapter.persist_anchored_sessions(), Ok(0));
        assert_eq!(adapter.pending_committed_lanes.len(), 2);
        assert!(
            adapter
                .kura
                .read_certified_lane_block_artifact(missing_lane_id, missing_lane_block_height)
                .is_none()
        );
        assert!(
            adapter
                .kura
                .read_lane_block_application_receipt(missing_lane_id, missing_lane_block_height)
                .is_none()
        );
        assert!(
            adapter
                .kura
                .read_certified_lane_block_artifact(lane_id, lane_block_height)
                .is_none()
        );
        assert!(
            adapter
                .kura
                .read_lane_block_application_receipt(lane_id, lane_block_height)
                .is_none()
        );
    }

    #[test]
    fn operator_session_projection_reports_bounded_inflight_lane_work() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let proposal = coordinator_proposal(&adapter, &keys);
        adapter
            .lane_sessions
            .insert_proposal(proposal.clone())
            .expect("insert status proposal");

        let status = adapter.lane_session_status();
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].lane_id, proposal.descriptor.lane_id);
        assert_eq!(status[0].dataspace_id, proposal.descriptor.dataspace_id);
        assert_eq!(status[0].proposal_hash, proposal.proposal_hash);
        assert!(status[0].has_proposal);
        assert!(!status[0].has_commit_qc);
    }

    #[test]
    fn unanchored_lane_certificate_never_appears_committed_in_operator_status() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let proposal = coordinator_proposal(&adapter, &keys);
        adapter
            .lane_sessions
            .insert_proposal(proposal.clone())
            .expect("insert status proposal");
        for key in keys.iter().take(3) {
            adapter
                .lane_sessions
                .insert_vote(
                    signed_lane_vote(&proposal, CertPhase::Prepare, key),
                    Some(&PeerId::new(key.public_key().clone())),
                )
                .expect("insert prepare vote");
        }
        let _ = adapter.lane_sessions.drain_newly_sealed_qcs();
        for key in keys.iter().take(3) {
            adapter
                .lane_sessions
                .insert_vote(
                    signed_lane_vote(&proposal, CertPhase::Commit, key),
                    Some(&PeerId::new(key.public_key().clone())),
                )
                .expect("insert commit vote");
        }
        adapter.collect_committed_lane_sessions();

        assert_eq!(adapter.pending_committed_lanes.len(), 1);
        assert!(adapter.committed_lane_status().is_empty());
    }

    fn native_request(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
    ) -> NativeAmxAttestationRequestV2 {
        let coordinator_proposal = coordinator_proposal(adapter, keys);
        let mut body = native_body(adapter);
        body.coordinator_lane_incarnation = coordinator_proposal.descriptor.lane_incarnation;
        body.authority_context_height = coordinator_proposal.descriptor.proposal_height;
        body.planned_coordinator_block_height = coordinator_proposal.descriptor.lane_block_height;
        body.coordinator_lane_block_view = coordinator_proposal.descriptor.lane_block_view;
        body.coordinator_proposal_hash = coordinator_proposal.proposal_hash;
        let plan = RoutingPlan::native_amx(
            RoutingDecision::new(body.coordinator_lane_id, body.coordinator_dataspace_id),
            vec![crate::queue::RouteLeg::new(
                RoutingDecision::new(body.participant_lane_id, body.participant_dataspace_id),
                crate::queue::RouteLegRole::Participant,
            )],
        );
        body.plan_digest = plan.digest();
        NativeAmxAttestationRequestV2 {
            body,
            plan_legs: plan.legs(),
            coordinator_proposal,
        }
    }

    fn refresh_coordinator_request_proposal(request: &mut NativeAmxAttestationRequestV2) {
        request.coordinator_proposal.descriptor.descriptor_hash = request
            .coordinator_proposal
            .descriptor
            .computed_descriptor_hash();
        request.coordinator_proposal.proposal_hash =
            request.coordinator_proposal.computed_proposal_hash();
        request.body.coordinator_proposal_hash = request.coordinator_proposal.proposal_hash;
    }

    fn signed_native_vote(body: NativeAmxAttestationBodyV2, key: &KeyPair) -> NativeAmxVoteV2 {
        let signature = Signature::try_new(key.private_key(), &body.signature_preimage())
            .expect("sign native AMX vote fixture");
        NativeAmxVoteV2 {
            body,
            signer: PeerId::new(key.public_key().clone()),
            bls_signature: signature.payload().to_vec(),
        }
    }

    #[test]
    fn adapter_construction_clamps_native_signing_journal_at_hard_capacity_boundary() {
        let one = NonZeroUsize::new(1).expect("nonzero");
        let hard = NonZeroUsize::new(crate::native_amx::MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD)
            .expect("hard limit is nonzero");
        let at_limit = V2LaneWorkLimits::new(hard, one, one, one, one, hard);
        assert_eq!(at_limit.native_signing_capacity(), Ok(hard));
        let (adapter, _) = fixture_with_limits(wire::ConsensusMode::Permissioned, at_limit)
            .expect("the exact durable journal hard limit is accepted");
        drop(adapter);

        let over =
            NonZeroUsize::new(crate::native_amx::MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD + 1)
                .expect("hard limit successor is nonzero");
        let over_limit = V2LaneWorkLimits::new(over, one, one, one, one, over);
        assert_eq!(over_limit.native_signing_capacity(), Ok(hard));
        let (adapter, _) = fixture_with_limits(wire::ConsensusMode::Permissioned, over_limit)
            .expect("a valid logical capacity is clamped at the durable hard limit");
        drop(adapter);

        let request_bound = NonZeroUsize::new(17).expect("nonzero request bound");
        let request_limited = V2LaneWorkLimits::new(hard, one, one, one, one, request_bound);
        assert_eq!(
            request_limited.native_signing_capacity(),
            Ok(request_bound),
            "the authenticated request budget is the durable per-height signing budget"
        );

        let overflow = V2LaneWorkLimits::new(
            NonZeroUsize::new(usize::MAX).expect("usize max is nonzero"),
            NonZeroUsize::new(2).expect("nonzero"),
            one,
            one,
            one,
            one,
        );
        assert!(matches!(
            fixture_with_limits(wire::ConsensusMode::Permissioned, overflow),
            Err(V2LaneWorkError::InvalidContext(message))
                if message.contains("overflows the local address space")
        ));
    }

    #[test]
    fn production_default_native_signing_capacity_constructs_for_validator_and_observer() {
        use iroha_config::parameters::defaults::sumeragi as defaults;

        let control = NonZeroUsize::new(defaults::MSG_CHANNEL_CAP_VOTES)
            .expect("production control queue default is nonzero");
        let max_transactions = defaults::V2_BLOCK_MAX_TRANSACTIONS;
        let one = NonZeroUsize::new(1).expect("nonzero");
        let limits = V2LaneWorkLimits::new(control, max_transactions, one, one, one, control);
        assert_eq!(
            control.get().checked_mul(max_transactions.get()),
            Some(4_194_304),
            "test must exercise the exact production-default product"
        );
        assert_eq!(limits.native_signing_capacity(), Ok(control));

        let (validator, _) = fixture_with_limits(wire::ConsensusMode::Permissioned, limits)
            .expect("production defaults construct a voting adapter");
        assert!(validator.voting_enabled);
        drop(validator);

        let (observer, _) =
            fixture_with_limits_and_voting(wire::ConsensusMode::Permissioned, limits, false)
                .expect("production defaults construct an observer adapter");
        assert!(!observer.voting_enabled);
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
        future_view.coordinator_lane_block_view = 1;
        assert!(!adapter.native_body_matches_context(&future_view, 0));
        assert!(adapter.native_body_matches_context(&future_view, 1));
        assert!(
            !adapter.native_body_matches_context(&body, 1),
            "a past-view request or vote must not remain admissible"
        );

        let mut wrong_lane_height = body;
        wrong_lane_height.planned_coordinator_block_height = 2;
        assert!(!adapter.native_body_matches_context(&wrong_lane_height, 0));
    }

    #[test]
    fn native_vote_requires_the_exact_request_sent_to_its_signer_before_crypto() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let request = native_request(&adapter, &keys);
        let signer_index = keys
            .iter()
            .position(|key| key.public_key() != adapter.local_peer.public_key())
            .expect("fixture has a remote signer");
        let signer = PeerId::new(keys[signer_index].public_key().clone());
        adapter.register_native_request(
            request.body,
            signer.clone(),
            NativeAmxMessage::PrepareRequest(request.clone()),
        );
        let vote = signed_native_vote(request.body, &keys[signer_index]);
        assert!(adapter.native_request_was_sent_to_vote_signer(&vote, NativeAmxPhase::Prepare));

        adapter.native_requests.clear();
        let other = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .find(|peer| peer != &signer)
            .expect("fixture has another peer");
        adapter.register_native_request(
            request.body,
            other.clone(),
            NativeAmxMessage::PrepareRequest(request.clone()),
        );
        assert!(
            !adapter.native_request_was_sent_to_vote_signer(&vote, NativeAmxPhase::Prepare),
            "a request sent to another validator must not authorize this signer"
        );
        assert_eq!(
            adapter.accept_native_vote(other, vote, NativeAmxPhase::Prepare, 0),
            V2LaneIngressOutcome::Rejected,
            "authenticated transport sender drift must fail before signature or PoP work"
        );

        let wrong_request_sender = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .find(|peer| peer != &adapter.local_peer)
            .expect("fixture has a non-leader peer");
        assert_eq!(
            adapter.accept_native_request(wrong_request_sender, request, None, 0),
            V2LaneIngressOutcome::Rejected,
            "only the exact current global/lane coordinator may issue a request"
        );
    }

    #[test]
    fn authenticated_native_replay_gates_require_exact_sender_body_and_signature() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let request = native_request(&adapter, &keys);
        let request_sender = adapter.local_peer.clone();
        let request_message = NativeAmxMessage::PrepareRequest(request.clone());
        let request_key = native_authenticated_request_key(&request_sender, &request_message);
        adapter.authenticated_native_requests.insert(
            request_key,
            (request_sender.clone(), request_message.clone()),
        );
        assert_eq!(
            adapter.authenticated_native_request_replay(
                request_key,
                &request_sender,
                &request_message,
            ),
            Some(V2LaneIngressOutcome::Duplicate)
        );
        let other_sender = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .find(|peer| peer != &request_sender)
            .expect("fixture has another sender");
        assert_eq!(
            adapter.authenticated_native_request_replay(
                request_key,
                &other_sender,
                &request_message,
            ),
            Some(V2LaneIngressOutcome::Rejected),
            "a hash-bucket hit never authenticates another transport sender"
        );

        let signer_index = keys
            .iter()
            .position(|key| key.public_key() != adapter.local_peer.public_key())
            .expect("fixture has a remote signer");
        let signer = PeerId::new(keys[signer_index].public_key().clone());
        adapter.register_native_request(
            request.body,
            signer.clone(),
            NativeAmxMessage::PrepareRequest(request.clone()),
        );
        let vote = signed_native_vote(request.body, &keys[signer_index]);
        let claim = NativeVoteClaimKey {
            session: NativeAmxSessionKey::from_body(&vote.body),
            round: vote.body.round,
            epoch: vote.body.epoch,
            participant_lane: vote.body.participant_lane_id,
            participant_dataspace: vote.body.participant_dataspace_id,
            phase: vote.body.phase,
            signer: HashOf::new(&vote.signer),
        };
        adapter.native_claims.insert(claim, vote.body);
        adapter
            .native_claim_signatures
            .insert(claim, vote.bls_signature.clone());
        assert_eq!(
            adapter.accept_native_vote(signer.clone(), vote.clone(), NativeAmxPhase::Prepare, 0,),
            V2LaneIngressOutcome::Duplicate,
            "an exact previously authenticated vote replay bypasses repeated BLS/PoP work"
        );
        let mut changed_signature = vote;
        changed_signature.bls_signature[0] ^= 1;
        assert_eq!(
            adapter.accept_native_vote(signer, changed_signature, NativeAmxPhase::Prepare, 0,),
            V2LaneIngressOutcome::Rejected,
            "same-claim unauthenticated signature substitution must not use the replay fast path"
        );
    }

    #[test]
    fn native_view_advance_prunes_stale_capacity_claims_sessions_requests_and_effects() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let request = native_request(&adapter, &keys);
        let remote_index = keys
            .iter()
            .position(|key| key.public_key() != adapter.local_peer.public_key())
            .expect("fixture has a remote validator");
        let remote = PeerId::new(keys[remote_index].public_key().clone());
        adapter.register_native_request(
            request.body,
            remote,
            NativeAmxMessage::PrepareRequest(request.clone()),
        );
        let vote = signed_native_vote(request.body, &keys[remote_index]);
        adapter
            .native_sessions
            .insert_vote(vote.clone())
            .expect("seed old-view session");
        let remote_claim = NativeVoteClaimKey {
            session: NativeAmxSessionKey::from_body(&request.body),
            round: request.body.round,
            epoch: request.body.epoch,
            participant_lane: request.body.participant_lane_id,
            participant_dataspace: request.body.participant_dataspace_id,
            phase: request.body.phase,
            signer: HashOf::new(&vote.signer),
        };
        adapter.native_claims.insert(remote_claim, request.body);
        adapter
            .native_claim_signatures
            .insert(remote_claim, vote.bls_signature.clone());
        let authenticated_request = NativeAmxMessage::PrepareRequest(request.clone());
        let authenticated_request_key =
            native_authenticated_request_key(&adapter.local_peer, &authenticated_request);
        adapter.authenticated_native_requests.insert(
            authenticated_request_key,
            (adapter.local_peer.clone(), authenticated_request),
        );

        let local_capacity = adapter
            .limits
            .session_capacity
            .get()
            .saturating_mul(adapter.limits.body_buckets_per_session.get());
        for index in 0..local_capacity {
            let mut claimed_body = request.body;
            let source_hash = Hash::new(index.to_be_bytes());
            claimed_body.source_id.copy_from_slice(source_hash.as_ref());
            claimed_body.plan_digest =
                Hash::new(index.saturating_add(local_capacity).to_be_bytes());
            let claim = NativeVoteClaimKey {
                session: NativeAmxSessionKey::from_body(&claimed_body),
                round: claimed_body.round,
                epoch: claimed_body.epoch,
                participant_lane: claimed_body.participant_lane_id,
                participant_dataspace: claimed_body.participant_dataspace_id,
                phase: claimed_body.phase,
                signer: HashOf::new(&adapter.local_peer),
            };
            adapter.local_native_claims.insert(claim, claimed_body);
        }
        assert!(
            adapter.sign_native_vote_once(request.body).is_none(),
            "the exact-view anti-equivocation cap must fail closed"
        );
        assert!(!adapter.native_requests.is_empty());
        assert!(!adapter.effects.is_empty());

        adapter.schedule_retransmission(1);
        assert_eq!(adapter.native_active_view, 1);
        assert!(adapter.native_requests.is_empty());
        assert!(adapter.authenticated_native_requests.is_empty());
        assert!(adapter.native_claims.is_empty());
        assert!(adapter.native_claim_signatures.is_empty());
        assert!(adapter.local_native_claims.is_empty());
        assert!(
            adapter
                .native_sessions
                .sorted_votes_for_body(
                    NativeAmxSessionKey::from_body(&request.body),
                    &request.body,
                )
                .is_empty()
        );
        assert!(
            adapter
                .effects
                .iter()
                .all(|effect| !matches!(effect, V2LaneWorkEffect::PostNativeAmx { .. }))
        );
        assert_eq!(adapter.effect_keys.len(), adapter.effects.len());

        let mut fresh = request.body;
        fresh.round.view = 1;
        fresh.coordinator_lane_block_view = 1;
        assert!(
            adapter.sign_native_vote_once(fresh).is_some(),
            "fresh-view work must make progress immediately after stale capacity is pruned"
        );
        assert!(!adapter.advance_native_view(0));
        assert_eq!(adapter.native_active_view, 1);
    }

    #[test]
    fn durable_multiview_signing_capacity_exhaustion_is_nonfatal_and_still_fail_closed() {
        let one = NonZeroUsize::new(1).expect("nonzero");
        let eight = NonZeroUsize::new(8).expect("nonzero");
        let limits = V2LaneWorkLimits::new(one, one, eight, eight, eight, eight);
        let (mut adapter, _) = fixture_with_limits(wire::ConsensusMode::Permissioned, limits)
            .expect("open one-record signing adapter");
        let first = native_body(&adapter);
        adapter
            .sign_native_vote_once(first)
            .expect("first view consumes the one durable record");

        adapter.schedule_retransmission(1);
        let mut next_view = first;
        next_view.round.view = 1;
        next_view.coordinator_lane_block_view = 1;
        assert!(adapter.sign_native_vote_once(next_view).is_none());
        assert!(adapter.native_signing_capacity_exhausted);
        assert!(
            adapter.ensure_healthy().is_ok(),
            "bounded work exhaustion must not abort the serialized height runner"
        );

        let mut conflicting_plan = next_view;
        conflicting_plan.plan_digest = Hash::new(b"capacity-conflicting-plan");
        assert!(adapter.sign_native_vote_once(conflicting_plan).is_none());
        assert!(
            adapter.ensure_healthy().is_ok(),
            "durable source-plan rejection remains hostile input, not journal corruption"
        );

        let context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let key_pair = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        drop(adapter);
        let mut restarted =
            V2LaneWorkAdapter::new(context, local_peer, key_pair, true, state, kura, limits)
                .expect("restart capacity-exhausted signing adapter");
        restarted.schedule_retransmission(1);
        assert!(restarted.sign_native_vote_once(next_view).is_none());
        assert!(restarted.ensure_healthy().is_ok());
        assert!(restarted.sign_native_vote_once(conflicting_plan).is_none());
        assert!(restarted.ensure_healthy().is_ok());
    }

    #[test]
    fn native_coordinator_authority_rejects_leader_quorum_committee_and_predecessor_drift() {
        let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let request = native_request(&adapter, &keys);
        let validators = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let min_signers =
            super::super::network_topology::commit_quorum_from_len(validators.len()).max(1);
        let leader = lane_block_redrive_leader(&request.coordinator_proposal, 0)
            .expect("canonical lane leader")
            .clone();
        assert!(native_coordinator_proposal_matches_authority(
            &request,
            &leader,
            &validators,
            min_signers,
            0,
            None,
        ));

        let wrong_leader = validators
            .iter()
            .find(|peer| *peer != &leader)
            .expect("fixture has another validator");
        assert!(!native_coordinator_proposal_matches_authority(
            &request,
            wrong_leader,
            &validators,
            min_signers,
            0,
            None,
        ));
        assert!(!native_coordinator_proposal_matches_authority(
            &request,
            &leader,
            &validators,
            min_signers.saturating_sub(1),
            0,
            None,
        ));

        let mut wrong_quorum = request.clone();
        wrong_quorum.coordinator_proposal.descriptor.min_quorum = wrong_quorum
            .coordinator_proposal
            .descriptor
            .min_quorum
            .saturating_sub(1);
        refresh_coordinator_request_proposal(&mut wrong_quorum);
        assert!(!native_coordinator_proposal_matches_authority(
            &wrong_quorum,
            &leader,
            &validators,
            min_signers,
            0,
            None,
        ));

        let replacement = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::BlsNormal)
            .expect("replacement BLS key");
        let mut substituted_validators = validators.clone();
        substituted_validators[0] = PeerId::new(replacement.public_key().clone());
        substituted_validators.sort();
        let mut wrong_committee = request.clone();
        let descriptor = &mut wrong_committee.coordinator_proposal.descriptor;
        descriptor.validator_set = substituted_validators;
        descriptor.validator_set_hash = HashOf::new(&descriptor.validator_set);
        descriptor.validator_count =
            u32::try_from(descriptor.validator_set.len()).expect("validator count");
        descriptor.min_quorum = u32::try_from(
            super::super::network_topology::commit_quorum_from_len(descriptor.validator_set.len())
                .max(1),
        )
        .expect("validator quorum");
        refresh_coordinator_request_proposal(&mut wrong_committee);
        let substituted_leader =
            lane_block_redrive_leader(&wrong_committee.coordinator_proposal, 0)
                .expect("substituted proposal has a deterministic leader");
        assert!(!native_coordinator_proposal_matches_authority(
            &wrong_committee,
            substituted_leader,
            &validators,
            min_signers,
            0,
            None,
        ));

        let mut wrong_predecessor = request.clone();
        wrong_predecessor
            .coordinator_proposal
            .descriptor
            .previous_lane_block_height = 1;
        wrong_predecessor
            .coordinator_proposal
            .descriptor
            .previous_lane_block_descriptor_hash = Some(Hash::new(b"wrong predecessor"));
        wrong_predecessor
            .coordinator_proposal
            .descriptor
            .lane_block_height = 2;
        wrong_predecessor.body.planned_coordinator_block_height = 2;
        refresh_coordinator_request_proposal(&mut wrong_predecessor);
        let predecessor_leader =
            lane_block_redrive_leader(&wrong_predecessor.coordinator_proposal, 0)
                .expect("predecessor-bound proposal has a deterministic leader");
        assert!(!native_coordinator_proposal_matches_authority(
            &wrong_predecessor,
            predecessor_leader,
            &validators,
            min_signers,
            0,
            None,
        ));
        assert!(native_coordinator_proposal_matches_authority(
            &wrong_predecessor,
            predecessor_leader,
            &validators,
            min_signers,
            1,
            Some(Hash::new(b"wrong predecessor")),
        ));
        assert!(!native_coordinator_proposal_matches_authority(
            &wrong_predecessor,
            predecessor_leader,
            &validators,
            min_signers,
            1,
            Some(Hash::new(b"other predecessor")),
        ));
    }

    #[test]
    fn native_coordinator_authority_rejects_incarnation_and_pop_substitution() {
        let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let request = native_request(&adapter, &keys);
        let validators = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let min_signers =
            super::super::network_topology::commit_quorum_from_len(validators.len()).max(1);
        let leader = lane_block_redrive_leader(&request.coordinator_proposal, 0)
            .expect("canonical lane leader")
            .clone();

        let mut wrong_incarnation = request.clone();
        wrong_incarnation
            .coordinator_proposal
            .descriptor
            .lane_incarnation = Hash::new(b"substituted incarnation");
        refresh_coordinator_request_proposal(&mut wrong_incarnation);
        assert!(!native_coordinator_proposal_matches_authority(
            &wrong_incarnation,
            &leader,
            &validators,
            min_signers,
            0,
            None,
        ));

        let pops = keys
            .iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("fixture PoP"))
            .collect::<Vec<_>>();
        assert!(verified_native_committee_pops(&validators, &pops).is_some());
        let mut substituted_pops = pops.clone();
        substituted_pops.swap(0, 1);
        assert!(verified_native_committee_pops(&validators, &substituted_pops).is_none());
        let mut truncated_pops = pops;
        truncated_pops.pop();
        assert!(verified_native_committee_pops(&validators, &truncated_pops).is_none());

        let ed25519 = KeyPair::try_from_seed(vec![0xE1; 32], Algorithm::Ed25519)
            .expect("Ed25519 adversarial key");
        let mut mixed_committee = validators;
        mixed_committee[0] = PeerId::new(ed25519.public_key().clone());
        mixed_committee.sort();
        assert!(!native_coordinator_proposal_matches_authority(
            &request,
            &leader,
            &mixed_committee,
            min_signers,
            0,
            None,
        ));
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

        let mut conflicting_plan = body;
        conflicting_plan.plan_digest = Hash::new(b"conflicting-routing-plan");
        assert!(
            adapter.sign_native_vote_once(conflicting_plan).is_none(),
            "one source transaction must retain one durable plan across all local claim keys"
        );
        assert!(adapter.ensure_healthy().is_ok());
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
    fn local_native_amx_signing_decision_survives_adapter_restart() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let body = native_body(&adapter);
        let context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let key_pair = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;

        adapter
            .sign_native_vote_once(body)
            .expect("first exact body may be signed");
        drop(adapter);

        let mut restarted =
            V2LaneWorkAdapter::new(context, local_peer, key_pair, true, state, kura, limits)
                .expect("reopen lane adapter at the same canonical height");
        let mut conflicting = body;
        conflicting.coordinator_proposal_hash = Hash::new(b"restart-conflicting-proposal");
        assert!(
            restarted.sign_native_vote_once(conflicting).is_none(),
            "the durable journal must reject an equivocation before the empty memory cache can"
        );
        assert!(
            restarted.ensure_healthy().is_ok(),
            "a rejected equivocation is hostile input, not journal corruption"
        );
        assert!(restarted.local_native_claims.is_empty());

        restarted
            .sign_native_vote_once(body)
            .expect("the exact durable decision remains idempotently signable");
        assert_eq!(restarted.local_native_claims.len(), 1);
    }

    #[test]
    fn unexpected_native_signing_guard_failure_latches_adapter_health() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let valid = native_body(&adapter);
        let mut malformed = valid;
        malformed.participant_validator_count = 0;

        assert!(adapter.sign_native_vote_once(malformed).is_none());
        assert!(matches!(
            adapter.ensure_healthy(),
            Err(V2LaneWorkError::SigningGuard(_))
        ));
        assert!(
            adapter.sign_native_vote_once(valid).is_none(),
            "a latched journal failure must block unrelated future signatures"
        );
        assert!(adapter.local_native_claims.is_empty());
    }

    #[test]
    fn effect_queue_is_bounded_and_deduplicates_until_drain() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let message = NativeAmxMessage::PrepareRequest(native_request(&adapter, &keys));
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
