//! Lane-local block vote validation, session caching, and QC aggregation helpers.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey, Signature};
use iroha_data_model::{
    block::consensus::{CertPhase, LaneBlockProposalV1, LaneBlockQcV1, LaneBlockVoteBodyV1},
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    nexus::{DataSpaceId, LaneId},
    peer::PeerId,
};
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Individual lane-local block vote before committee aggregation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LaneBlockVoteV1 {
    /// Body signed by the lane validator.
    pub body: LaneBlockVoteBodyV1,
    /// Validator that produced the vote.
    pub signer: PeerId,
    /// BLS signature over [`LaneBlockVoteBodyV1::signature_preimage`].
    pub bls_signature: Vec<u8>,
}

/// Stable key for one lane-local proposal session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LaneBlockSessionKey {
    /// Lane whose local block is being certified.
    pub(crate) lane_id: LaneId,
    /// Dataspace bound to the lane-local block.
    pub(crate) dataspace_id: DataSpaceId,
    /// Lane-local block height.
    pub(crate) lane_block_height: u64,
    /// Lane-local view.
    pub(crate) lane_block_view: u64,
    /// Proposal hash certified by votes and QCs in this session.
    pub(crate) proposal_hash: Hash,
}

/// Stable key for detecting conflicting proposals for the same lane slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LaneBlockSlotKey {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
    lane_block_view: u64,
}

/// Cached lane-local proposal, votes, and QCs for one proposal hash.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LaneBlockSession {
    /// Proposal artifact, when it has arrived.
    pub(crate) proposal: Option<LaneBlockProposalV1>,
    /// Prepare votes keyed by signer.
    pub(crate) prepare_votes: BTreeMap<PeerId, LaneBlockVoteV1>,
    /// Commit votes keyed by signer.
    pub(crate) commit_votes: BTreeMap<PeerId, LaneBlockVoteV1>,
    /// Prepare QC, when one has arrived.
    pub(crate) prepare_qc: Option<LaneBlockQcV1>,
    /// Commit QC, when one has arrived.
    pub(crate) commit_qc: Option<LaneBlockQcV1>,
    /// Prepare QC was sealed locally and has not yet been handed to transport.
    pending_prepare_qc_broadcast: bool,
    /// Commit QC was sealed locally and has not yet been handed to transport.
    pending_commit_qc_broadcast: bool,
    /// Proposal plus prepare QC are ready for a local commit vote handoff.
    pending_commit_vote_request: bool,
    /// Local commit vote handoff was already drained for this session.
    commit_vote_request_drained: bool,
    /// Proposal plus prepare/commit QCs are ready and have not yet been drained.
    pending_committed_session_drain: bool,
    /// Fully committed session was already handed to the lane executor boundary.
    committed_session_drained: bool,
}

/// Lane-local block session that has enough certificates to execute.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommittedLaneBlockSession {
    /// Proposal artifact that defines the lane-local block subject and committee.
    pub(crate) proposal: LaneBlockProposalV1,
    /// Prepare certificate for the proposal.
    pub(crate) prepare_qc: LaneBlockQcV1,
    /// Commit certificate for the proposal.
    pub(crate) commit_qc: LaneBlockQcV1,
}

/// Cached lane-local block that is ready for a local commit vote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LaneBlockCommitVoteRequest {
    /// Proposal artifact that defines the lane-local block subject and committee.
    pub(crate) proposal: LaneBlockProposalV1,
    /// Prepare certificate that unlocks the commit vote phase.
    pub(crate) prepare_qc: LaneBlockQcV1,
}

/// Result of inserting a lane-block artifact into a session cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LaneBlockSessionInsertOutcome {
    /// The cache state changed.
    Inserted,
    /// The artifact was already present with identical contents.
    Duplicate,
}

/// Failure while inserting a lane-block artifact into a session cache.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(crate) enum LaneBlockSessionError {
    /// proposal failed stateless ingress validation
    #[error("lane block proposal is invalid: {0}")]
    InvalidProposal(LaneBlockProposalIngressError),
    /// vote failed stateless ingress validation
    #[error("lane block vote is invalid: {0}")]
    InvalidVote(LaneBlockVoteIngressError),
    /// QC failed stateless ingress validation
    #[error("lane block QC is invalid: {0}")]
    InvalidQc(LaneBlockQcIngressError),
    /// another proposal already owns the same lane slot
    #[error("conflicting lane block proposal for lane slot")]
    ConflictingProposal,
    /// vote body does not match the cached proposal artifact
    #[error("lane block vote body does not match proposal")]
    VoteProposalMismatch,
    /// vote signer is not in the cached proposal validator set
    #[error("lane block vote signer is not in validator set")]
    VoteSignerNotInValidatorSet,
    /// signer already submitted different vote bytes for the same proposal phase
    #[error("conflicting lane block vote for signer")]
    ConflictingVote,
    /// QC body or validator set does not match the cached proposal artifact
    #[error("lane block QC does not match proposal")]
    QcProposalMismatch,
    /// a different QC already exists for the same proposal phase
    #[error("conflicting lane block QC")]
    ConflictingQc,
}

/// Bounded in-memory cache for standalone lane-block consensus sessions.
///
/// The capacity bounds ordinary uncommitted session state. Sessions that
/// already carry a proposal plus prepare and commit QCs are protected from
/// eviction until the executor boundary drains them, because dropping certified
/// lane blocks under queue backpressure can strand lane-local progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LaneBlockSessionCache {
    capacity: usize,
    sessions: BTreeMap<LaneBlockSessionKey, LaneBlockSession>,
    slot_proposals: BTreeMap<LaneBlockSlotKey, Hash>,
    order: VecDeque<LaneBlockSessionKey>,
}

impl LaneBlockSessionCache {
    /// Build a cache that stores at most `capacity.max(1)` uncommitted sessions.
    #[must_use]
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            sessions: BTreeMap::new(),
            slot_proposals: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Number of cached sessions.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Return true when no sessions are cached.
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Get a cached session by key.
    #[cfg(test)]
    pub(crate) fn get(&self, key: &LaneBlockSessionKey) -> Option<&LaneBlockSession> {
        self.sessions.get(key)
    }

    /// Drain QCs sealed locally from cached votes and mark them as handed to transport.
    pub(crate) fn drain_newly_sealed_qcs(&mut self) -> Vec<LaneBlockQcV1> {
        let mut sealed = Vec::new();
        for session in self.sessions.values_mut() {
            if session.pending_prepare_qc_broadcast {
                if let Some(qc) = session.prepare_qc.clone() {
                    sealed.push(qc);
                }
                session.pending_prepare_qc_broadcast = false;
            }
            if session.pending_commit_qc_broadcast {
                if let Some(qc) = session.commit_qc.clone() {
                    sealed.push(qc);
                }
                session.pending_commit_qc_broadcast = false;
            }
        }
        sealed
    }

    /// Drain sessions that have a proposal and prepare QC and still need the
    /// supplied validator's local commit vote.
    pub(crate) fn drain_commit_vote_requests_for(
        &mut self,
        signer: &PeerId,
    ) -> Vec<LaneBlockCommitVoteRequest> {
        let mut requests = Vec::new();
        for session in self.sessions.values_mut() {
            if !session.pending_commit_vote_request {
                continue;
            }
            session.pending_commit_vote_request = false;
            session.commit_vote_request_drained = true;
            let (Some(proposal), Some(prepare_qc)) =
                (session.proposal.clone(), session.prepare_qc.clone())
            else {
                continue;
            };
            if session.commit_qc.is_some()
                || session.commit_votes.contains_key(signer)
                || !proposal.descriptor.validator_set.contains(signer)
            {
                continue;
            }
            requests.push(LaneBlockCommitVoteRequest {
                proposal,
                prepare_qc,
            });
        }
        requests
    }

    /// Drain up to `limit` sessions whose proposal, prepare QC, and commit QC are all cached.
    ///
    /// This is intentionally separate from [`Self::drain_newly_sealed_qcs`]:
    /// inbound QCs are not transport work, but they can still make a session
    /// executable once the matching proposal and opposite phase QC are present.
    pub(crate) fn drain_committed_sessions_up_to(
        &mut self,
        limit: usize,
    ) -> Vec<CommittedLaneBlockSession> {
        if limit == 0 {
            return Vec::new();
        }
        let mut committed = Vec::new();
        for session in self.sessions.values_mut() {
            if committed.len() >= limit {
                break;
            }
            if !session.pending_committed_session_drain {
                continue;
            }
            session.pending_committed_session_drain = false;
            let (Some(proposal), Some(prepare_qc), Some(commit_qc)) = (
                session.proposal.clone(),
                session.prepare_qc.clone(),
                session.commit_qc.clone(),
            ) else {
                continue;
            };
            session.committed_session_drained = true;
            committed.push(CommittedLaneBlockSession {
                proposal,
                prepare_qc,
                commit_qc,
            });
        }
        committed
    }

    /// Drain all sessions whose proposal, prepare QC, and commit QC are all cached.
    #[cfg(test)]
    pub(crate) fn drain_committed_sessions(&mut self) -> Vec<CommittedLaneBlockSession> {
        self.drain_committed_sessions_up_to(usize::MAX)
    }

    /// Remove cached sessions whose lane/dataspace pair no longer belongs to the active topology.
    pub(crate) fn retain_sessions_for_active_lanes(
        &mut self,
        active_lane: impl Fn(LaneId, DataSpaceId) -> bool,
    ) -> usize {
        let before = self.sessions.len();
        self.sessions
            .retain(|key, _| active_lane(key.lane_id, key.dataspace_id));
        self.slot_proposals
            .retain(|key, _| active_lane(key.lane_id, key.dataspace_id));
        let retained_keys = self.sessions.keys().copied().collect::<BTreeSet<_>>();
        self.order.retain(|key| retained_keys.contains(key));
        before.saturating_sub(self.sessions.len())
    }

    /// Insert a standalone lane-block proposal artifact.
    ///
    /// If votes or QCs for the same proposal hash arrived first, they are
    /// reconciled against the proposal. Orphan artifacts that do not match the
    /// now-known proposal are discarded instead of blocking the valid proposal.
    pub(crate) fn insert_proposal(
        &mut self,
        proposal: LaneBlockProposalV1,
    ) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
        validate_lane_block_proposal(&proposal).map_err(LaneBlockSessionError::InvalidProposal)?;
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let slot_key = LaneBlockSlotKey::from_session_key(key);
        if let Some(existing_hash) = self.slot_proposals.get(&slot_key).copied()
            && existing_hash != key.proposal_hash
        {
            return Err(LaneBlockSessionError::ConflictingProposal);
        }

        self.touch(key);
        let session = self.sessions.entry(key).or_default();
        if let Some(existing) = &session.proposal {
            if existing == &proposal {
                return Ok(LaneBlockSessionInsertOutcome::Duplicate);
            }
            return Err(LaneBlockSessionError::ConflictingProposal);
        }
        reconcile_session_with_proposal(session, &proposal);
        session.proposal = Some(proposal);
        try_seal_session_qcs(session);
        refresh_commit_vote_request_ready(session);
        refresh_committed_session_ready(session);
        self.slot_proposals.insert(slot_key, key.proposal_hash);
        self.evict();
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    }

    /// Insert a standalone lane-block vote.
    pub(crate) fn insert_vote(
        &mut self,
        vote: LaneBlockVoteV1,
        sender: Option<&PeerId>,
    ) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
        vote.validate_ingress(vote.body.phase, sender)
            .map_err(LaneBlockSessionError::InvalidVote)?;
        let phase = vote.body.phase;
        let key = LaneBlockSessionKey::from_vote_body(&vote.body);

        self.touch(key);
        let session = self.sessions.entry(key).or_default();
        if let Some(proposal) = &session.proposal {
            validate_vote_matches_proposal(&vote, proposal)?;
        }
        let votes = votes_for_phase_mut(session, phase).ok_or(
            LaneBlockSessionError::InvalidVote(LaneBlockVoteIngressError::InvalidBody),
        )?;
        if let Some(existing) = votes.get(&vote.signer) {
            if existing == &vote {
                return Ok(LaneBlockSessionInsertOutcome::Duplicate);
            }
            return Err(LaneBlockSessionError::ConflictingVote);
        }
        votes.insert(vote.signer.clone(), vote);
        try_seal_phase_qc(session, phase);
        refresh_commit_vote_request_ready(session);
        refresh_committed_session_ready(session);
        self.evict();
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    }

    /// Insert a standalone lane-block QC without aggregate verification.
    #[cfg(test)]
    pub(crate) fn insert_qc(
        &mut self,
        qc: LaneBlockQcV1,
    ) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
        validate_lane_block_qc(&qc).map_err(LaneBlockSessionError::InvalidQc)?;
        self.insert_validated_qc(qc)
    }

    /// Insert a standalone lane-block QC after verifying its aggregate
    /// signature against the provided proof-of-possession material.
    pub(crate) fn insert_qc_with_pops(
        &mut self,
        qc: LaneBlockQcV1,
        pops: &BTreeMap<PublicKey, Vec<u8>>,
    ) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
        validate_lane_block_qc_aggregate(&qc, pops).map_err(LaneBlockSessionError::InvalidQc)?;
        self.insert_validated_qc(qc)
    }

    fn insert_validated_qc(
        &mut self,
        qc: LaneBlockQcV1,
    ) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
        let key = LaneBlockSessionKey::from_vote_body(&qc.body);

        self.touch(key);
        let session = self.sessions.entry(key).or_default();
        if let Some(proposal) = &session.proposal {
            validate_qc_matches_proposal(&qc, proposal)?;
        }
        let slot = qc_for_phase_mut(session, qc.body.phase).ok_or(
            LaneBlockSessionError::InvalidQc(LaneBlockQcIngressError::InvalidBody),
        )?;
        if let Some(existing) = slot.as_ref() {
            if existing == &qc {
                return Ok(LaneBlockSessionInsertOutcome::Duplicate);
            }
            return Err(LaneBlockSessionError::ConflictingQc);
        }
        *slot = Some(qc);
        refresh_commit_vote_request_ready(session);
        refresh_committed_session_ready(session);
        self.evict();
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    }

    fn touch(&mut self, key: LaneBlockSessionKey) {
        self.order.retain(|existing| *existing != key);
        self.order.push_back(key);
    }

    fn evict(&mut self) {
        while self.unprotected_session_count() > self.capacity {
            if !self.evict_oldest_unprotected_session() {
                break;
            }
        }
    }

    fn unprotected_session_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|session| !session_has_undrained_committed_evidence(session))
            .count()
    }

    fn evict_oldest_unprotected_session(&mut self) -> bool {
        let scan_limit = self.order.len();
        for _ in 0..scan_limit {
            let Some(oldest) = self.order.pop_front() else {
                return false;
            };
            let Some(session) = self.sessions.get(&oldest) else {
                continue;
            };
            if session_has_undrained_committed_evidence(session) {
                self.order.push_back(oldest);
                continue;
            }
            let removed = self
                .sessions
                .remove(&oldest)
                .expect("session existed before removal");
            if removed.proposal.is_some() {
                let slot = LaneBlockSlotKey::from_session_key(oldest);
                if self.slot_proposals.get(&slot) == Some(&oldest.proposal_hash) {
                    self.slot_proposals.remove(&slot);
                }
            }
            return true;
        }
        false
    }
}

impl Default for LaneBlockSessionCache {
    fn default() -> Self {
        Self::new(128)
    }
}

impl LaneBlockSessionKey {
    fn from_proposal(proposal: &LaneBlockProposalV1) -> Self {
        let descriptor = &proposal.descriptor;
        Self {
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            proposal_hash: proposal.proposal_hash,
        }
    }

    fn from_vote_body(body: &LaneBlockVoteBodyV1) -> Self {
        Self {
            lane_id: body.lane_id,
            dataspace_id: body.dataspace_id,
            lane_block_height: body.lane_block_height,
            lane_block_view: body.lane_block_view,
            proposal_hash: body.proposal_hash,
        }
    }
}

impl LaneBlockSlotKey {
    fn from_session_key(key: LaneBlockSessionKey) -> Self {
        Self {
            lane_id: key.lane_id,
            dataspace_id: key.dataspace_id,
            lane_block_height: key.lane_block_height,
            lane_block_view: key.lane_block_view,
        }
    }
}

fn votes_for_phase_mut(
    session: &mut LaneBlockSession,
    phase: CertPhase,
) -> Option<&mut BTreeMap<PeerId, LaneBlockVoteV1>> {
    match phase {
        CertPhase::Prepare => Some(&mut session.prepare_votes),
        CertPhase::Commit => Some(&mut session.commit_votes),
        CertPhase::NewView => None,
    }
}

fn qc_for_phase_mut(
    session: &mut LaneBlockSession,
    phase: CertPhase,
) -> Option<&mut Option<LaneBlockQcV1>> {
    match phase {
        CertPhase::Prepare => Some(&mut session.prepare_qc),
        CertPhase::Commit => Some(&mut session.commit_qc),
        CertPhase::NewView => None,
    }
}

fn proposal_vote_body(proposal: &LaneBlockProposalV1, phase: CertPhase) -> LaneBlockVoteBodyV1 {
    proposal.vote_body(phase)
}

fn validate_vote_matches_proposal(
    vote: &LaneBlockVoteV1,
    proposal: &LaneBlockProposalV1,
) -> Result<(), LaneBlockSessionError> {
    if vote.body != proposal_vote_body(proposal, vote.body.phase) {
        return Err(LaneBlockSessionError::VoteProposalMismatch);
    }
    if !proposal.descriptor.validator_set.contains(&vote.signer) {
        return Err(LaneBlockSessionError::VoteSignerNotInValidatorSet);
    }
    Ok(())
}

fn validate_qc_matches_proposal(
    qc: &LaneBlockQcV1,
    proposal: &LaneBlockProposalV1,
) -> Result<(), LaneBlockSessionError> {
    if qc.body != proposal_vote_body(proposal, qc.body.phase)
        || qc.validator_set != proposal.descriptor.validator_set
        || qc.validator_set_hash != proposal.descriptor.validator_set_hash
        || qc.validator_set_hash_version != proposal.descriptor.validator_set_hash_version
    {
        return Err(LaneBlockSessionError::QcProposalMismatch);
    }
    Ok(())
}

fn reconcile_session_with_proposal(session: &mut LaneBlockSession, proposal: &LaneBlockProposalV1) {
    for phase in [CertPhase::Prepare, CertPhase::Commit] {
        if let Some(votes) = votes_for_phase_mut(session, phase) {
            votes.retain(|_, vote| validate_vote_matches_proposal(vote, proposal).is_ok());
        }
        if let Some(slot) = qc_for_phase_mut(session, phase) {
            let keep_qc = slot
                .as_ref()
                .is_none_or(|qc| validate_qc_matches_proposal(qc, proposal).is_ok());
            if !keep_qc {
                *slot = None;
            }
        }
    }
}

fn try_seal_session_qcs(session: &mut LaneBlockSession) {
    for phase in [CertPhase::Prepare, CertPhase::Commit] {
        try_seal_phase_qc(session, phase);
    }
}

fn refresh_committed_session_ready(session: &mut LaneBlockSession) {
    if session.committed_session_drained || session.pending_committed_session_drain {
        return;
    }
    if session.proposal.is_some() && session.prepare_qc.is_some() && session.commit_qc.is_some() {
        session.pending_committed_session_drain = true;
    }
}

fn session_has_undrained_committed_evidence(session: &LaneBlockSession) -> bool {
    session.pending_committed_session_drain
        && !session.committed_session_drained
        && session.proposal.is_some()
        && session.prepare_qc.is_some()
        && session.commit_qc.is_some()
}

fn refresh_commit_vote_request_ready(session: &mut LaneBlockSession) {
    if session.commit_qc.is_some() || session.proposal.is_none() || session.prepare_qc.is_none() {
        session.pending_commit_vote_request = false;
        return;
    }
    if session.commit_vote_request_drained || session.pending_commit_vote_request {
        return;
    }
    session.pending_commit_vote_request = true;
}

fn try_seal_phase_qc(session: &mut LaneBlockSession, phase: CertPhase) {
    let Some(proposal) = session.proposal.clone() else {
        return;
    };
    let qc_already_exists = match phase {
        CertPhase::Prepare => session.prepare_qc.is_some(),
        CertPhase::Commit => session.commit_qc.is_some(),
        CertPhase::NewView => return,
    };
    if qc_already_exists {
        return;
    }
    let min_quorum = usize::try_from(proposal.descriptor.min_quorum).unwrap_or(usize::MAX);
    let votes = match phase {
        CertPhase::Prepare => session.prepare_votes.values(),
        CertPhase::Commit => session.commit_votes.values(),
        CertPhase::NewView => return,
    }
    .cloned()
    .collect::<Vec<_>>();
    if votes.len() < min_quorum {
        return;
    }
    if let Ok(qc) = aggregate_lane_block_votes_to_qc(
        proposal_vote_body(&proposal, phase),
        proposal.descriptor.validator_set,
        &votes,
    ) {
        if let Some(slot) = qc_for_phase_mut(session, phase) {
            *slot = Some(qc);
        }
        match phase {
            CertPhase::Prepare => session.pending_prepare_qc_broadcast = true,
            CertPhase::Commit => session.pending_commit_qc_broadcast = true,
            CertPhase::NewView => {}
        }
    }
}

fn peer_uses_bls_normal(peer: &PeerId) -> bool {
    peer.public_key()
        .try_algorithm()
        .is_ok_and(|algorithm| algorithm == Algorithm::BlsNormal)
}

impl LaneBlockVoteV1 {
    /// Validate phase, transport signer binding, BLS-normal identity, and vote signature.
    ///
    /// This is the stateless ingress prefilter. Callers that know the current
    /// world state must still verify that the signer belongs to the live lane
    /// committee and has a live proof of possession at the lane block height.
    ///
    /// # Errors
    ///
    /// Returns an error when the vote is carried by the wrong phase message,
    /// the authenticated sender does not match the signer, the signer is not
    /// BLS-normal, or the BLS signature does not verify against the canonical
    /// lane-block vote preimage.
    pub fn validate_ingress(
        &self,
        expected_phase: CertPhase,
        sender: Option<&PeerId>,
    ) -> Result<(), LaneBlockVoteIngressError> {
        validate_lane_block_vote_body_shape(&self.body)?;
        if self.body.phase != expected_phase {
            return Err(LaneBlockVoteIngressError::PhaseMismatch {
                expected: expected_phase,
                actual: self.body.phase,
            });
        }
        if let Some(sender) = sender
            && sender != &self.signer
        {
            return Err(LaneBlockVoteIngressError::SenderMismatch);
        }
        if !peer_uses_bls_normal(&self.signer) {
            return Err(LaneBlockVoteIngressError::SignerNotBlsNormal);
        }
        Signature::try_from_bytes(&self.bls_signature)
            .map_err(|_| LaneBlockVoteIngressError::InvalidSignature)?
            .verify(self.signer.public_key(), &self.body.signature_preimage())
            .map_err(|_| LaneBlockVoteIngressError::InvalidSignature)
    }
}

/// Failure while validating a lane-local block vote before session-cache insertion.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum LaneBlockVoteIngressError {
    /// lane block vote body is malformed
    #[error("lane block vote body is malformed")]
    InvalidBody,
    /// lane block vote message phase does not match the embedded body phase
    #[error("lane block vote phase mismatch: expected {expected:?}, got {actual:?}")]
    PhaseMismatch {
        /// Phase implied by the received message variant.
        expected: CertPhase,
        /// Phase embedded in the signed lane-block vote body.
        actual: CertPhase,
    },
    /// lane block vote was transported by a peer other than the signer
    #[error("lane block vote sender does not match signer")]
    SenderMismatch,
    /// lane block vote signer is not a BLS-normal consensus identity
    #[error("lane block vote signer is not BLS-normal")]
    SignerNotBlsNormal,
    /// lane block vote signature is missing, malformed, or invalid
    #[error("lane block vote signature is invalid")]
    InvalidSignature,
}

/// Failure while validating a standalone lane-local block proposal before session insertion.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum LaneBlockProposalIngressError {
    /// lane block proposal body is malformed
    #[error("lane block proposal body is malformed")]
    InvalidBody,
    /// descriptor validator set is empty
    #[error("lane block validator set is empty")]
    EmptyValidatorSet,
    /// descriptor validator set is not in canonical sorted order
    #[error("lane block validator set is not canonical")]
    ValidatorSetNotCanonical,
    /// descriptor validator set contains a duplicate peer
    #[error("lane block validator set contains a duplicate peer")]
    DuplicateValidator,
    /// descriptor validator set length does not match the descriptor quorum fields
    #[error("lane block validator count mismatch")]
    ValidatorCountMismatch,
    /// descriptor validator-set hash or hash version does not match the embedded validator set
    #[error("lane block validator-set hash mismatch")]
    ValidatorSetHashMismatch,
    /// descriptor hash does not match the canonical descriptor fields
    #[error("lane block descriptor hash mismatch")]
    DescriptorHashMismatch,
    /// proposal hash does not match the canonical proposal fields
    #[error("lane block proposal hash mismatch")]
    ProposalHashMismatch,
}

/// Failure while validating a standalone lane-local block QC before session insertion.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum LaneBlockQcIngressError {
    /// lane block QC body is malformed
    #[error("lane block QC body is malformed")]
    InvalidBody,
    /// descriptor validator set is empty
    #[error("lane block validator set is empty")]
    EmptyValidatorSet,
    /// descriptor validator set is not in canonical sorted order
    #[error("lane block validator set is not canonical")]
    ValidatorSetNotCanonical,
    /// descriptor validator set contains a duplicate peer
    #[error("lane block validator set contains a duplicate peer")]
    DuplicateValidator,
    /// descriptor validator set length does not match the QC body
    #[error("lane block validator count mismatch")]
    ValidatorCountMismatch,
    /// descriptor validator-set hash or hash version does not match the QC body
    #[error("lane block validator-set hash mismatch")]
    ValidatorSetHashMismatch,
    /// signer bitmap length does not match the validator set
    #[error("lane block QC signer bitmap length mismatch")]
    SignerBitmapLengthMismatch,
    /// signer bitmap contains bits beyond the validator set
    #[error("lane block QC signer bitmap contains out-of-range signers")]
    SignerBitmapOutOfRange,
    /// signer bitmap is below quorum
    #[error("lane block QC signer bitmap quorum is not met")]
    QuorumNotMet,
    /// signer bitmap selects a non-BLS-normal validator
    #[error("lane block QC signer is not BLS-normal")]
    SignerNotBlsNormal,
    /// aggregate signature bytes are missing
    #[error("lane block QC aggregate signature is missing")]
    AggregateSignatureMissing,
    /// signer bitmap selects a validator without proof-of-possession material
    #[error("lane block QC signer proof-of-possession is missing")]
    SignerPopMissing,
    /// signer bitmap selects a validator with invalid proof-of-possession material
    #[error("lane block QC signer proof-of-possession is invalid")]
    SignerPopInvalid,
    /// aggregate signature does not verify for the selected signers
    #[error("lane block QC aggregate signature is invalid")]
    AggregateSignatureInvalid,
}

/// Failure while building a lane-local block QC from validator votes.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum LaneBlockQcBuildError {
    /// lane block vote body is malformed
    #[error("lane block vote body is malformed")]
    InvalidBody,
    /// descriptor validator set is empty
    #[error("lane block validator set is empty")]
    EmptyValidatorSet,
    /// descriptor validator set is not in canonical sorted order
    #[error("lane block validator set is not canonical")]
    ValidatorSetNotCanonical,
    /// descriptor validator set contains a duplicate peer
    #[error("lane block validator set contains a duplicate peer")]
    DuplicateValidator,
    /// descriptor validator set length does not match the body
    #[error("lane block validator count mismatch")]
    ValidatorCountMismatch,
    /// descriptor validator-set hash or hash version does not match the body
    #[error("lane block validator-set hash mismatch")]
    ValidatorSetHashMismatch,
    /// no votes were supplied for the requested lane block
    #[error("no votes were supplied for the requested lane block")]
    EmptyVotes,
    /// a vote signed a different lane block body
    #[error("a vote signed a different lane block body")]
    BodyMismatch,
    /// a vote signer is not in the lane validator set
    #[error("a vote signer is not in the lane validator set")]
    SignerNotInValidatorSet,
    /// a vote signer appears more than once
    #[error("a vote signer appears more than once")]
    DuplicateSigner,
    /// a vote signer is not a BLS-normal consensus identity
    #[error("a lane block vote signer is not BLS-normal")]
    SignerNotBlsNormal,
    /// an individual vote signature is missing, malformed, or invalid
    #[error("an individual lane block vote signature is invalid")]
    InvalidSignature,
    /// the vote set does not satisfy the lane quorum
    #[error("lane block vote quorum is not met")]
    QuorumNotMet,
    /// BLS signature aggregation failed
    #[error("failed to aggregate lane block BLS signatures")]
    SignatureAggregate,
}

/// Validate the signer-independent body of a standalone lane-local block proposal.
///
/// This check is intentionally stateless: it proves that the descriptor,
/// validator set, quorum fields, and proposal hash are internally coherent.
/// Callers must still check lane lifecycle state, live committee authority, and
/// replay/execution availability before accepting the proposal into a lane
/// session.
///
/// # Errors
///
/// Returns an error when the descriptor has malformed coordinates, accepted
/// work, validator set, quorum fields, validator-set hash, descriptor hash, or
/// proposal hash.
pub fn validate_lane_block_proposal(
    proposal: &LaneBlockProposalV1,
) -> Result<(), LaneBlockProposalIngressError> {
    let descriptor = &proposal.descriptor;
    if descriptor.lane_block_height == 0
        || descriptor.qc_mode_tag.trim().is_empty()
        || descriptor.accepted_candidate_indices.is_empty()
        || descriptor.accepted_candidate_indices.len()
            != descriptor.accepted_transaction_hashes.len()
        || descriptor.previous_lane_block_height == 0
            && descriptor.previous_lane_block_descriptor_hash.is_some()
        || descriptor.lane_block_height <= descriptor.previous_lane_block_height
        || descriptor.min_quorum == 0
        || descriptor.validator_count == 0
        || descriptor.min_quorum > descriptor.validator_count
    {
        return Err(LaneBlockProposalIngressError::InvalidBody);
    }
    validate_lane_block_validator_set_fields(
        descriptor.validator_set_hash_version,
        descriptor.validator_set_hash,
        descriptor.validator_count,
        &descriptor.validator_set,
    )
    .map_err(|err| match err {
        LaneBlockQcBuildError::EmptyValidatorSet => {
            LaneBlockProposalIngressError::EmptyValidatorSet
        }
        LaneBlockQcBuildError::ValidatorSetNotCanonical => {
            LaneBlockProposalIngressError::ValidatorSetNotCanonical
        }
        LaneBlockQcBuildError::DuplicateValidator => {
            LaneBlockProposalIngressError::DuplicateValidator
        }
        LaneBlockQcBuildError::ValidatorCountMismatch => {
            LaneBlockProposalIngressError::ValidatorCountMismatch
        }
        LaneBlockQcBuildError::ValidatorSetHashMismatch => {
            LaneBlockProposalIngressError::ValidatorSetHashMismatch
        }
        _ => LaneBlockProposalIngressError::InvalidBody,
    })?;
    if descriptor.computed_descriptor_hash() != descriptor.descriptor_hash {
        return Err(LaneBlockProposalIngressError::DescriptorHashMismatch);
    }
    if proposal.computed_proposal_hash() != proposal.proposal_hash {
        return Err(LaneBlockProposalIngressError::ProposalHashMismatch);
    }
    Ok(())
}

/// Validate signer-independent lane QC structure before live session insertion.
///
/// This check intentionally does not verify the aggregate signature because
/// rogue-key-safe BLS aggregate verification needs the live proof-of-possession
/// material for the selected lane committee. It does verify the body, committee
/// shape, validator-set hash, signer bitmap, quorum, signer key algorithm, and
/// aggregate presence.
///
/// # Errors
///
/// Returns an error when the QC is malformed, below quorum, carries a bad
/// signer bitmap, or references non-BLS-normal validators.
pub fn validate_lane_block_qc(qc: &LaneBlockQcV1) -> Result<(), LaneBlockQcIngressError> {
    validate_lane_block_vote_body_shape(&qc.body)
        .map_err(|_| LaneBlockQcIngressError::InvalidBody)?;
    if qc.validator_set_hash_version != qc.body.validator_set_hash_version
        || qc.validator_set_hash != qc.body.validator_set_hash
    {
        return Err(LaneBlockQcIngressError::ValidatorSetHashMismatch);
    }
    validate_lane_block_validator_set(&qc.body, &qc.validator_set).map_err(|err| match err {
        LaneBlockQcBuildError::EmptyValidatorSet => LaneBlockQcIngressError::EmptyValidatorSet,
        LaneBlockQcBuildError::ValidatorSetNotCanonical => {
            LaneBlockQcIngressError::ValidatorSetNotCanonical
        }
        LaneBlockQcBuildError::DuplicateValidator => LaneBlockQcIngressError::DuplicateValidator,
        LaneBlockQcBuildError::ValidatorCountMismatch => {
            LaneBlockQcIngressError::ValidatorCountMismatch
        }
        LaneBlockQcBuildError::ValidatorSetHashMismatch => {
            LaneBlockQcIngressError::ValidatorSetHashMismatch
        }
        _ => LaneBlockQcIngressError::InvalidBody,
    })?;

    let expected_bitmap_len = qc.validator_set.len().div_ceil(8);
    if qc.signers_bitmap.len() != expected_bitmap_len {
        return Err(LaneBlockQcIngressError::SignerBitmapLengthMismatch);
    }
    if qc.bls_aggregate_signature.is_empty() {
        return Err(LaneBlockQcIngressError::AggregateSignatureMissing);
    }

    let mut signer_count = 0_u32;
    for (byte_index, byte) in qc.signers_bitmap.iter().copied().enumerate() {
        for bit in 0..8 {
            if byte & (1_u8 << bit) == 0 {
                continue;
            }
            let signer_index = byte_index * 8 + bit;
            let Some(signer) = qc.validator_set.get(signer_index) else {
                return Err(LaneBlockQcIngressError::SignerBitmapOutOfRange);
            };
            if !peer_uses_bls_normal(signer) {
                return Err(LaneBlockQcIngressError::SignerNotBlsNormal);
            }
            signer_count = signer_count.saturating_add(1);
        }
    }
    if signer_count < qc.body.min_quorum {
        return Err(LaneBlockQcIngressError::QuorumNotMet);
    }
    Ok(())
}

/// Validate a lane QC structure and its pre-aggregated BLS signature.
///
/// The `pops` map must contain a valid BLS-normal proof-of-possession for each
/// signer selected by the QC bitmap. This keeps same-message aggregate
/// verification rogue-key-safe without consulting global state inside this
/// deterministic helper.
///
/// # Errors
///
/// Returns an error when the QC shape is invalid, a selected signer has missing
/// or invalid proof-of-possession material, or the aggregate signature does not
/// verify for the selected signer keys and canonical vote preimage.
pub fn validate_lane_block_qc_aggregate(
    qc: &LaneBlockQcV1,
    pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<(), LaneBlockQcIngressError> {
    validate_lane_block_qc(qc)?;

    let mut public_keys: Vec<&PublicKey> = Vec::new();
    let mut pop_refs: Vec<&[u8]> = Vec::new();
    for (byte_index, byte) in qc.signers_bitmap.iter().copied().enumerate() {
        if byte == 0 {
            continue;
        }
        for bit in 0..8 {
            if byte & (1_u8 << bit) == 0 {
                continue;
            }
            let signer_index = byte_index * 8 + bit;
            let signer = qc
                .validator_set
                .get(signer_index)
                .ok_or(LaneBlockQcIngressError::SignerBitmapOutOfRange)?;
            let pk = signer.public_key();
            let pop = pops
                .get(pk)
                .ok_or(LaneBlockQcIngressError::SignerPopMissing)?;
            iroha_crypto::bls_normal_pop_verify(pk, pop)
                .map_err(|_| LaneBlockQcIngressError::SignerPopInvalid)?;
            public_keys.push(pk);
            pop_refs.push(pop.as_slice());
        }
    }

    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &qc.body.signature_preimage(),
        &qc.bls_aggregate_signature,
        &public_keys,
        &pop_refs,
    )
    .map_err(|_| LaneBlockQcIngressError::AggregateSignatureInvalid)
}

/// Build a lane-local block QC from sorted or unsorted validator votes.
///
/// The resulting bitmap and aggregate signature are deterministic because
/// votes are projected into the supplied validator-set order before
/// aggregation.
///
/// # Errors
///
/// Returns an error when the body or validator set is malformed, votes do not
/// match `body`, include duplicate or unknown signers, fail to meet
/// `body.min_quorum`, or cannot be aggregated as BLS-normal signatures.
pub fn aggregate_lane_block_votes_to_qc(
    body: LaneBlockVoteBodyV1,
    validator_set: Vec<PeerId>,
    votes: &[LaneBlockVoteV1],
) -> Result<LaneBlockQcV1, LaneBlockQcBuildError> {
    validate_lane_block_vote_body_shape(&body).map_err(|_| LaneBlockQcBuildError::InvalidBody)?;
    validate_lane_block_validator_set(&body, &validator_set)?;
    if votes.is_empty() {
        return Err(LaneBlockQcBuildError::EmptyVotes);
    }

    let mut indexed_signatures: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    for vote in votes {
        if vote.body != body {
            return Err(LaneBlockQcBuildError::BodyMismatch);
        }
        let Some(index) = validator_set
            .iter()
            .position(|validator| validator == &vote.signer)
        else {
            return Err(LaneBlockQcBuildError::SignerNotInValidatorSet);
        };
        if indexed_signatures
            .insert(index, vote.bls_signature.clone())
            .is_some()
        {
            return Err(LaneBlockQcBuildError::DuplicateSigner);
        }
        if !peer_uses_bls_normal(&vote.signer) {
            return Err(LaneBlockQcBuildError::SignerNotBlsNormal);
        }
        let signature = Signature::try_from_bytes(&vote.bls_signature)
            .map_err(|_| LaneBlockQcBuildError::InvalidSignature)?;
        if signature
            .verify(vote.signer.public_key(), &body.signature_preimage())
            .is_err()
        {
            return Err(LaneBlockQcBuildError::InvalidSignature);
        }
    }

    if indexed_signatures.len()
        < usize::try_from(body.min_quorum).map_err(|_| LaneBlockQcBuildError::InvalidBody)?
    {
        return Err(LaneBlockQcBuildError::QuorumNotMet);
    }

    let mut signers_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
    let ordered_signatures = indexed_signatures
        .into_iter()
        .map(|(index, signature)| {
            signers_bitmap[index / 8] |= 1_u8 << (index % 8);
            signature
        })
        .collect::<Vec<_>>();
    let signature_refs = ordered_signatures
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let bls_aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .map_err(|_| LaneBlockQcBuildError::SignatureAggregate)?;

    Ok(LaneBlockQcV1 {
        body,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set,
        signers_bitmap,
        bls_aggregate_signature,
    })
}

fn validate_lane_block_vote_body_shape(
    body: &LaneBlockVoteBodyV1,
) -> Result<(), LaneBlockVoteIngressError> {
    if body.phase == CertPhase::NewView
        || body.lane_block_height == 0
        || body.qc_mode_tag.trim().is_empty()
        || body.accepted_candidate_indices.is_empty()
        || body.accepted_candidate_indices.len() != body.accepted_transaction_hashes.len()
        || body.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
        || body.validator_count == 0
        || body.min_quorum == 0
        || body.min_quorum > body.validator_count
    {
        return Err(LaneBlockVoteIngressError::InvalidBody);
    }
    Ok(())
}

fn validate_lane_block_validator_set(
    body: &LaneBlockVoteBodyV1,
    validator_set: &[PeerId],
) -> Result<(), LaneBlockQcBuildError> {
    validate_lane_block_validator_set_fields(
        body.validator_set_hash_version,
        body.validator_set_hash,
        body.validator_count,
        validator_set,
    )
}

fn validate_lane_block_validator_set_fields(
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_count: u32,
    validator_set: &[PeerId],
) -> Result<(), LaneBlockQcBuildError> {
    if validator_set.is_empty() {
        return Err(LaneBlockQcBuildError::EmptyValidatorSet);
    }
    let actual_validator_count = u32::try_from(validator_set.len())
        .map_err(|_| LaneBlockQcBuildError::ValidatorCountMismatch)?;
    if actual_validator_count != validator_count {
        return Err(LaneBlockQcBuildError::ValidatorCountMismatch);
    }
    let mut canonical = validator_set.to_vec();
    canonical.sort();
    if canonical != validator_set {
        return Err(LaneBlockQcBuildError::ValidatorSetNotCanonical);
    }
    for pair in canonical.windows(2) {
        if pair[0] == pair[1] {
            return Err(LaneBlockQcBuildError::DuplicateValidator);
        }
    }
    if validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
        || validator_set_hash != HashOf::new(&validator_set.to_vec())
    {
        return Err(LaneBlockQcBuildError::ValidatorSetHashMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use iroha_crypto::{Hash, KeyPair, PublicKey, bls_normal_pop_prove};
    use iroha_data_model::{
        block::consensus::{LaneBlockDescriptorV1, LaneBlockProposalV1},
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        nexus::{DataSpaceId, LaneId},
    };

    use super::*;

    fn checked_bls_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
            .expect("generate checked lane block BLS fixture keypair")
    }

    fn checked_ed25519_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("generate checked lane block Ed25519 fixture keypair")
    }

    fn peer(keypair: &KeyPair) -> PeerId {
        PeerId::new(keypair.public_key().clone())
    }

    fn signed_vote(body: &LaneBlockVoteBodyV1, keypair: &KeyPair) -> LaneBlockVoteV1 {
        let signature = Signature::try_new(keypair.private_key(), &body.signature_preimage())
            .expect("checked lane block fixture signature");
        signature
            .verify(keypair.public_key(), &body.signature_preimage())
            .expect("checked lane block fixture signature verifies");
        LaneBlockVoteV1 {
            body: body.clone(),
            signer: peer(keypair),
            bls_signature: signature.payload().to_vec(),
        }
    }

    fn signer_pops(keypairs: &[KeyPair]) -> BTreeMap<PublicKey, Vec<u8>> {
        keypairs
            .iter()
            .map(|keypair| {
                (
                    keypair.public_key().clone(),
                    bls_normal_pop_prove(keypair.private_key())
                        .expect("checked lane block fixture PoP"),
                )
            })
            .collect()
    }

    fn vote_body(validator_set: &[PeerId]) -> LaneBlockVoteBodyV1 {
        LaneBlockVoteBodyV1 {
            phase: CertPhase::Prepare,
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_block_height: 13,
            lane_block_view: 2,
            proposal_hash: Hash::prehashed([0x31; Hash::LENGTH]),
            descriptor_hash: Hash::prehashed([0x32; Hash::LENGTH]),
            subject_hash: Hash::prehashed([0x33; Hash::LENGTH]),
            payload_ownership_hash: Hash::prehashed([0x34; Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([0x35; Hash::LENGTH]),
            accepted_candidate_indices: vec![2, 0],
            accepted_transaction_hashes: vec![
                Hash::prehashed([0x36; Hash::LENGTH]),
                Hash::prehashed([0x37; Hash::LENGTH]),
            ],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set.to_vec()),
            validator_count: u32::try_from(validator_set.len()).expect("validator count fits"),
            min_quorum: 2,
            qc_mode_tag: "permissioned:lane:7:dataspace:11".to_string(),
        }
    }

    fn lane_block_proposal(validator_set: &[PeerId]) -> LaneBlockProposalV1 {
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            previous_lane_block_height: 12,
            previous_lane_block_descriptor_hash: Some(Hash::prehashed([0x20; Hash::LENGTH])),
            lane_block_height: 13,
            lane_block_view: 2,
            subject_hash: Hash::prehashed([0x23; Hash::LENGTH]),
            payload_ownership_hash: Hash::prehashed([0x24; Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([0x25; Hash::LENGTH]),
            accepted_candidate_indices: vec![3, 1],
            accepted_transaction_hashes: vec![
                Hash::prehashed([0x26; Hash::LENGTH]),
                Hash::prehashed([0x27; Hash::LENGTH]),
            ],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set.to_vec()),
            validator_set: validator_set.to_vec(),
            validator_count: u32::try_from(validator_set.len()).expect("fixture validator count"),
            min_quorum: 2,
            qc_mode_tag: "permissioned:lane:7:dataspace:11".to_string(),
            descriptor_hash: Hash::prehashed([0x00; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0x00; Hash::LENGTH]),
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    fn lane_block_proposal_at_height(
        validator_set: &[PeerId],
        lane_block_height: u64,
    ) -> LaneBlockProposalV1 {
        assert!(
            lane_block_height > 1,
            "fixture lane block height needs a predecessor"
        );
        let tag = u8::try_from(lane_block_height).unwrap_or(u8::MAX);
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            previous_lane_block_height: lane_block_height - 1,
            previous_lane_block_descriptor_hash: Some(Hash::prehashed([tag - 1; Hash::LENGTH])),
            lane_block_height,
            lane_block_view: 2,
            subject_hash: Hash::prehashed([tag; Hash::LENGTH]),
            payload_ownership_hash: Hash::prehashed([tag.saturating_add(1); Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([tag.saturating_add(2); Hash::LENGTH]),
            accepted_candidate_indices: vec![3, 1],
            accepted_transaction_hashes: vec![
                Hash::prehashed([tag.saturating_add(3); Hash::LENGTH]),
                Hash::prehashed([tag.saturating_add(4); Hash::LENGTH]),
            ],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set.to_vec()),
            validator_set: validator_set.to_vec(),
            validator_count: u32::try_from(validator_set.len()).expect("fixture validator count"),
            min_quorum: 2,
            qc_mode_tag: "permissioned:lane:7:dataspace:11".to_string(),
            descriptor_hash: Hash::prehashed([0x00; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0x00; Hash::LENGTH]),
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    fn rebind_lane_block_proposal_route(
        mut proposal: LaneBlockProposalV1,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
    ) -> LaneBlockProposalV1 {
        proposal.descriptor.lane_id = lane_id;
        proposal.descriptor.dataspace_id = dataspace_id;
        proposal.descriptor.qc_mode_tag = format!(
            "permissioned:lane:{}:dataspace:{}",
            lane_id.as_u32(),
            dataspace_id.as_u64()
        );
        proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    fn retag_lane_block_proposal_payload(
        mut proposal: LaneBlockProposalV1,
        tag: u8,
    ) -> LaneBlockProposalV1 {
        proposal.descriptor.subject_hash = Hash::prehashed([tag; Hash::LENGTH]);
        proposal.descriptor.payload_ownership_hash =
            Hash::prehashed([tag.saturating_add(1); Hash::LENGTH]);
        proposal.descriptor.rbc_instance_hash =
            Hash::prehashed([tag.saturating_add(2); Hash::LENGTH]);
        proposal.descriptor.accepted_transaction_hashes = vec![
            Hash::prehashed([tag.saturating_add(3); Hash::LENGTH]),
            Hash::prehashed([tag.saturating_add(4); Hash::LENGTH]),
        ];
        proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    #[test]
    fn lane_block_proposal_ingress_accepts_canonical_artifact() {
        let keypairs = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keypairs.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);

        validate_lane_block_proposal(&proposal).expect("canonical proposal is valid");
        let body = proposal.vote_body(CertPhase::Prepare);
        assert_eq!(body.proposal_hash, proposal.proposal_hash);
        assert_eq!(body.descriptor_hash, proposal.descriptor.descriptor_hash);
        assert_eq!(body.validator_set_hash, HashOf::new(&validator_set));
        assert_eq!(
            body.accepted_transaction_hashes,
            proposal.descriptor.accepted_transaction_hashes
        );
    }

    #[test]
    fn lane_block_proposal_ingress_rejects_shape_and_committee_drift() {
        let keypairs = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keypairs.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);

        let mut empty_work = proposal.clone();
        empty_work.descriptor.accepted_candidate_indices.clear();
        assert_eq!(
            validate_lane_block_proposal(&empty_work),
            Err(LaneBlockProposalIngressError::InvalidBody)
        );

        let mut predecessor_at_genesis = proposal.clone();
        predecessor_at_genesis.descriptor.previous_lane_block_height = 0;
        assert_eq!(
            validate_lane_block_proposal(&predecessor_at_genesis),
            Err(LaneBlockProposalIngressError::InvalidBody)
        );

        let mut noncanonical = proposal.clone();
        noncanonical.descriptor.validator_set.reverse();
        noncanonical.descriptor.validator_set_hash =
            HashOf::new(&noncanonical.descriptor.validator_set);
        assert_eq!(
            validate_lane_block_proposal(&noncanonical),
            Err(LaneBlockProposalIngressError::ValidatorSetNotCanonical)
        );

        let mut duplicate = proposal.clone();
        duplicate.descriptor.validator_set =
            vec![validator_set[0].clone(), validator_set[0].clone()];
        duplicate.descriptor.validator_count = 2;
        duplicate.descriptor.min_quorum = 1;
        duplicate.descriptor.validator_set_hash = HashOf::new(&duplicate.descriptor.validator_set);
        assert_eq!(
            validate_lane_block_proposal(&duplicate),
            Err(LaneBlockProposalIngressError::DuplicateValidator)
        );
    }

    #[test]
    fn lane_block_proposal_ingress_rejects_hash_drift() {
        let keypairs = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keypairs.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);

        let mut validator_hash_drift = proposal.clone();
        validator_hash_drift.descriptor.validator_set_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0x70; Hash::LENGTH]));
        assert_eq!(
            validate_lane_block_proposal(&validator_hash_drift),
            Err(LaneBlockProposalIngressError::ValidatorSetHashMismatch)
        );

        let mut descriptor_hash_drift = proposal.clone();
        descriptor_hash_drift.descriptor.descriptor_hash = Hash::prehashed([0x71; Hash::LENGTH]);
        assert_eq!(
            validate_lane_block_proposal(&descriptor_hash_drift),
            Err(LaneBlockProposalIngressError::DescriptorHashMismatch)
        );

        let mut proposal_hash_drift = proposal;
        proposal_hash_drift.proposal_hash = Hash::prehashed([0x72; Hash::LENGTH]);
        assert_eq!(
            validate_lane_block_proposal(&proposal_hash_drift),
            Err(LaneBlockProposalIngressError::ProposalHashMismatch)
        );
    }

    #[test]
    fn lane_block_vote_ingress_accepts_matching_signed_bls_vote() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let body = vote_body(&validator_set);
        let vote = signed_vote(&body, &keys[0]);

        vote.validate_ingress(CertPhase::Prepare, Some(&vote.signer))
            .expect("valid signed lane block vote");
    }

    #[test]
    fn lane_block_vote_ingress_rejects_sender_phase_algorithm_and_signature_drift() {
        let bls = checked_bls_keypair(1);
        let other = checked_bls_keypair(2);
        let ed25519 = checked_ed25519_keypair(3);
        let mut validator_set = [peer(&bls), peer(&other)].to_vec();
        validator_set.sort();
        let body = vote_body(&validator_set);
        let vote = signed_vote(&body, &bls);

        assert_eq!(
            vote.validate_ingress(CertPhase::Commit, Some(&vote.signer)),
            Err(LaneBlockVoteIngressError::PhaseMismatch {
                expected: CertPhase::Commit,
                actual: CertPhase::Prepare,
            })
        );
        assert_eq!(
            vote.validate_ingress(CertPhase::Prepare, Some(&peer(&other))),
            Err(LaneBlockVoteIngressError::SenderMismatch)
        );

        let mut non_bls = signed_vote(&body, &ed25519);
        non_bls.signer = peer(&ed25519);
        assert_eq!(
            non_bls.validate_ingress(CertPhase::Prepare, None),
            Err(LaneBlockVoteIngressError::SignerNotBlsNormal)
        );

        let mut bad_signature = vote;
        bad_signature.bls_signature = signed_vote(&body, &other).bls_signature;
        assert_eq!(
            bad_signature.validate_ingress(CertPhase::Prepare, None),
            Err(LaneBlockVoteIngressError::InvalidSignature)
        );
    }

    #[test]
    fn aggregate_lane_block_votes_builds_sorted_bitmap_and_signature() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let body = vote_body(&validator_set);
        let vote_a = signed_vote(&body, &keys[0]);
        let vote_c = signed_vote(&body, &keys[2]);

        let qc = aggregate_lane_block_votes_to_qc(
            body.clone(),
            validator_set.clone(),
            &[vote_c.clone(), vote_a.clone()],
        )
        .expect("lane block QC");

        let expected_signer_indices = [vote_a.signer, vote_c.signer]
            .into_iter()
            .map(|signer| {
                validator_set
                    .iter()
                    .position(|validator| validator == &signer)
                    .expect("signer in validator set")
            })
            .collect::<Vec<_>>();
        let mut expected_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
        for index in expected_signer_indices {
            expected_bitmap[index / 8] |= 1_u8 << (index % 8);
        }
        assert_eq!(qc.signers_bitmap, expected_bitmap);
        assert_eq!(qc.body, body);
        assert_eq!(qc.validator_set_hash, HashOf::new(&validator_set));
        assert!(!qc.bls_aggregate_signature.is_empty());
    }

    #[test]
    fn lane_block_qc_ingress_accepts_aggregate_shape() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let body = vote_body(&validator_set);
        let vote_a = signed_vote(&body, &keys[0]);
        let vote_c = signed_vote(&body, &keys[2]);
        let qc = aggregate_lane_block_votes_to_qc(body, validator_set, &[vote_a, vote_c])
            .expect("lane block QC");

        validate_lane_block_qc(&qc).expect("QC ingress shape is valid");
    }

    #[test]
    fn lane_block_qc_aggregate_verifier_requires_valid_pops_and_signature() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let body = vote_body(&validator_set);
        let vote_a = signed_vote(&body, &keys[0]);
        let vote_c = signed_vote(&body, &keys[2]);
        let qc = aggregate_lane_block_votes_to_qc(
            body,
            validator_set.clone(),
            &[vote_a.clone(), vote_c],
        )
        .expect("lane block QC");
        let pops = signer_pops(&keys);

        validate_lane_block_qc_aggregate(&qc, &pops)
            .expect("QC aggregate verifies with signer PoPs");

        let mut missing_pop = pops.clone();
        missing_pop.remove(vote_a.signer.public_key());
        assert_eq!(
            validate_lane_block_qc_aggregate(&qc, &missing_pop),
            Err(LaneBlockQcIngressError::SignerPopMissing)
        );

        let mut invalid_pop = pops.clone();
        invalid_pop.insert(vote_a.signer.public_key().clone(), vec![0xA5; 96]);
        assert_eq!(
            validate_lane_block_qc_aggregate(&qc, &invalid_pop),
            Err(LaneBlockQcIngressError::SignerPopInvalid)
        );

        let mut forged_signature = qc;
        forged_signature.bls_aggregate_signature[0] ^= 0x01;
        assert_eq!(
            validate_lane_block_qc_aggregate(&forged_signature, &pops),
            Err(LaneBlockQcIngressError::AggregateSignatureInvalid)
        );
    }

    #[test]
    fn lane_block_qc_ingress_rejects_adversarial_shapes() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let body = vote_body(&validator_set);
        let vote_a = signed_vote(&body, &keys[0]);
        let vote_c = signed_vote(&body, &keys[2]);
        let qc = aggregate_lane_block_votes_to_qc(
            body.clone(),
            validator_set.clone(),
            &[vote_a, vote_c],
        )
        .expect("lane block QC");

        let mut hash_drift = qc.clone();
        hash_drift.validator_set_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0x81; Hash::LENGTH]));
        assert_eq!(
            validate_lane_block_qc(&hash_drift),
            Err(LaneBlockQcIngressError::ValidatorSetHashMismatch)
        );

        let mut short_bitmap = qc.clone();
        short_bitmap.signers_bitmap.clear();
        assert_eq!(
            validate_lane_block_qc(&short_bitmap),
            Err(LaneBlockQcIngressError::SignerBitmapLengthMismatch)
        );

        let mut out_of_range = qc.clone();
        out_of_range.signers_bitmap = vec![0b0000_1111];
        assert_eq!(
            validate_lane_block_qc(&out_of_range),
            Err(LaneBlockQcIngressError::SignerBitmapOutOfRange)
        );

        let mut below_quorum = qc.clone();
        below_quorum.signers_bitmap = vec![0b0000_0001];
        assert_eq!(
            validate_lane_block_qc(&below_quorum),
            Err(LaneBlockQcIngressError::QuorumNotMet)
        );

        let mut missing_signature = qc;
        missing_signature.bls_aggregate_signature.clear();
        assert_eq!(
            validate_lane_block_qc(&missing_signature),
            Err(LaneBlockQcIngressError::AggregateSignatureMissing)
        );
    }

    #[test]
    fn lane_block_session_cache_accepts_out_of_order_artifacts() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let body = proposal.vote_body(CertPhase::Prepare);
        let vote_a = signed_vote(&body, &keys[0]);
        let vote_c = signed_vote(&body, &keys[2]);
        let qc = aggregate_lane_block_votes_to_qc(
            body,
            validator_set.clone(),
            &[vote_a.clone(), vote_c],
        )
        .expect("lane block QC");
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_vote(vote_a.clone(), Some(&vote_a.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc(qc),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        let session = cache.get(&key).expect("session cached");
        assert_eq!(session.proposal.as_ref(), Some(&proposal));
        assert_eq!(session.prepare_votes.len(), 1);
        assert!(session.prepare_qc.is_some());
        assert!(session.commit_votes.is_empty());
        assert!(session.commit_qc.is_none());
    }

    #[test]
    fn lane_block_session_cache_seals_qc_when_vote_quorum_arrives() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let prepare_body = proposal.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let commit_body = proposal.vote_body(CertPhase::Commit);
        let commit_vote_a = signed_vote(&commit_body, &keys[0]);
        let commit_vote_b = signed_vote(&commit_body, &keys[1]);
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(proposal),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(prepare_vote_a.clone(), Some(&prepare_vote_a.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache
                .get(&key)
                .expect("session cached")
                .prepare_qc
                .is_none(),
            "below-quorum prepare votes must not seal a QC"
        );
        assert_eq!(
            cache.insert_vote(prepare_vote_b.clone(), Some(&prepare_vote_b.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        let prepare_qc = cache
            .get(&key)
            .expect("session cached")
            .prepare_qc
            .as_ref()
            .expect("prepare QC sealed from quorum votes");
        assert_eq!(prepare_qc.body.phase, CertPhase::Prepare);
        validate_lane_block_qc_aggregate(prepare_qc, &pops)
            .expect("sealed prepare QC aggregate verifies");

        assert_eq!(
            cache.insert_vote(commit_vote_a.clone(), Some(&commit_vote_a.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.get(&key).expect("session cached").commit_qc.is_none(),
            "below-quorum commit votes must not seal a QC"
        );
        assert_eq!(
            cache.insert_vote(commit_vote_b.clone(), Some(&commit_vote_b.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        let commit_qc = cache
            .get(&key)
            .expect("session cached")
            .commit_qc
            .as_ref()
            .expect("commit QC sealed from quorum votes");
        assert_eq!(commit_qc.body.phase, CertPhase::Commit);
        validate_lane_block_qc_aggregate(commit_qc, &pops)
            .expect("sealed commit QC aggregate verifies");

        let sealed = cache.drain_newly_sealed_qcs();
        assert_eq!(
            sealed.len(),
            2,
            "sealed prepare and commit QCs should be drained once"
        );
        assert_eq!(sealed[0].body.phase, CertPhase::Prepare);
        assert_eq!(sealed[1].body.phase, CertPhase::Commit);
        assert!(
            cache.drain_newly_sealed_qcs().is_empty(),
            "drained sealed QCs must not be emitted again"
        );
    }

    #[test]
    fn lane_block_session_cache_drains_committed_session_once_from_sealed_qcs() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let prepare_body = proposal.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let commit_body = proposal.vote_body(CertPhase::Commit);
        let commit_vote_a = signed_vote(&commit_body, &keys[0]);
        let commit_vote_b = signed_vote(&commit_body, &keys[1]);
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(prepare_vote_a.clone(), Some(&prepare_vote_a.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(prepare_vote_b.clone(), Some(&prepare_vote_b.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.drain_committed_sessions().is_empty(),
            "prepare QC alone is not enough to execute a lane block"
        );
        assert_eq!(
            cache.insert_vote(commit_vote_a.clone(), Some(&commit_vote_a.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(commit_vote_b.clone(), Some(&commit_vote_b.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        let committed = cache.drain_committed_sessions();
        assert_eq!(committed.len(), 1);
        assert_eq!(committed[0].proposal, proposal);
        assert_eq!(committed[0].prepare_qc.body.phase, CertPhase::Prepare);
        assert_eq!(committed[0].commit_qc.body.phase, CertPhase::Commit);
        validate_lane_block_qc_aggregate(&committed[0].prepare_qc, &pops)
            .expect("drained prepare QC verifies");
        validate_lane_block_qc_aggregate(&committed[0].commit_qc, &pops)
            .expect("drained commit QC verifies");
        assert!(
            cache.drain_committed_sessions().is_empty(),
            "committed sessions must be drained once"
        );
    }

    #[test]
    fn lane_block_session_cache_drains_committed_session_from_inbound_qcs() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let prepare_body = proposal.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set.clone(),
            &[prepare_vote_a, prepare_vote_b],
        )
        .expect("prepare QC");
        let commit_body = proposal.vote_body(CertPhase::Commit);
        let commit_vote_a = signed_vote(&commit_body, &keys[0]);
        let commit_vote_b = signed_vote(&commit_body, &keys[1]);
        let commit_qc = aggregate_lane_block_votes_to_qc(
            commit_body,
            validator_set,
            &[commit_vote_a, commit_vote_b],
        )
        .expect("commit QC");
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc.clone(), &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(cache.drain_committed_sessions().is_empty());
        assert_eq!(
            cache.insert_qc_with_pops(commit_qc.clone(), &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.drain_newly_sealed_qcs().is_empty(),
            "inbound QCs must not become transport broadcast work"
        );

        let committed = cache.drain_committed_sessions();
        assert_eq!(committed.len(), 1);
        assert_eq!(committed[0].proposal, proposal);
        assert_eq!(committed[0].prepare_qc, prepare_qc);
        assert_eq!(committed[0].commit_qc, commit_qc);
        assert!(cache.drain_committed_sessions().is_empty());
    }

    #[test]
    fn lane_block_session_cache_drains_commit_vote_request_once_after_prepare_qc() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let prepare_body = proposal.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set.clone(),
            &[prepare_vote_a, prepare_vote_b],
        )
        .expect("prepare QC");
        let signer = peer(&keys[2]);
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc.clone(), &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.drain_commit_vote_requests_for(&signer).is_empty(),
            "prepare QC without proposal must not request a commit vote"
        );
        assert_eq!(
            cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        let requests = cache.drain_commit_vote_requests_for(&signer);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].proposal, proposal);
        assert_eq!(requests[0].prepare_qc, prepare_qc);
        assert!(
            cache.drain_commit_vote_requests_for(&signer).is_empty(),
            "commit vote requests must drain once"
        );
    }

    #[test]
    fn lane_block_session_cache_skips_commit_vote_request_for_nonmember_or_existing_vote() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let outsider = checked_bls_keypair(4);
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let prepare_body = proposal.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set,
            &[prepare_vote_a, prepare_vote_b],
        )
        .expect("prepare QC");
        let pops = signer_pops(&keys);
        let mut nonmember_cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            nonmember_cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            nonmember_cache.insert_qc_with_pops(prepare_qc.clone(), &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            nonmember_cache
                .drain_commit_vote_requests_for(&peer(&outsider))
                .is_empty(),
            "non-committee signer must not receive a commit vote request"
        );
        assert!(
            nonmember_cache
                .drain_commit_vote_requests_for(&peer(&outsider))
                .is_empty(),
            "skipped non-member requests must not repeat"
        );

        let commit_body = proposal.vote_body(CertPhase::Commit);
        let existing_commit_vote = signed_vote(&commit_body, &keys[2]);
        let mut existing_vote_cache = LaneBlockSessionCache::new(4);
        assert_eq!(
            existing_vote_cache.insert_proposal(proposal),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            existing_vote_cache.insert_qc_with_pops(prepare_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            existing_vote_cache.insert_vote(
                existing_commit_vote.clone(),
                Some(&existing_commit_vote.signer)
            ),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            existing_vote_cache
                .drain_commit_vote_requests_for(&existing_commit_vote.signer)
                .is_empty(),
            "an already cached local commit vote must not be requested again"
        );
    }

    #[test]
    fn lane_block_session_cache_does_not_drain_until_proposal_and_both_qcs() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let prepare_body = proposal.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set.clone(),
            &[prepare_vote_a, prepare_vote_b],
        )
        .expect("prepare QC");
        let commit_body = proposal.vote_body(CertPhase::Commit);
        let commit_vote_a = signed_vote(&commit_body, &keys[0]);
        let commit_vote_b = signed_vote(&commit_body, &keys[1]);
        let commit_qc = aggregate_lane_block_votes_to_qc(
            commit_body,
            validator_set,
            &[commit_vote_a, commit_vote_b],
        )
        .expect("commit QC");
        let pops = signer_pops(&keys);

        let mut proposal_first = LaneBlockSessionCache::new(4);
        assert_eq!(
            proposal_first.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            proposal_first.insert_qc_with_pops(prepare_qc.clone(), &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            proposal_first.drain_committed_sessions().is_empty(),
            "one QC plus proposal is still incomplete"
        );

        let mut qcs_first = LaneBlockSessionCache::new(4);
        assert_eq!(
            qcs_first.insert_qc_with_pops(prepare_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            qcs_first.insert_qc_with_pops(commit_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            qcs_first.drain_committed_sessions().is_empty(),
            "QCs without the proposal are not executable"
        );
        assert_eq!(
            qcs_first.insert_proposal(proposal),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(qcs_first.drain_committed_sessions().len(), 1);
    }

    #[test]
    fn lane_block_session_cache_reconciles_orphan_qc_drift_before_commit_drain() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let prepare_body = proposal.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set.clone(),
            &[prepare_vote_a, prepare_vote_b],
        )
        .expect("prepare QC");
        let mut drift_commit_body = proposal.vote_body(CertPhase::Commit);
        drift_commit_body.descriptor_hash = Hash::prehashed([0xD0; Hash::LENGTH]);
        let drift_commit_vote_a = signed_vote(&drift_commit_body, &keys[0]);
        let drift_commit_vote_b = signed_vote(&drift_commit_body, &keys[1]);
        let drift_commit_qc = aggregate_lane_block_votes_to_qc(
            drift_commit_body,
            validator_set.clone(),
            &[drift_commit_vote_a, drift_commit_vote_b],
        )
        .expect("drifted commit QC");
        let commit_body = proposal.vote_body(CertPhase::Commit);
        let commit_vote_a = signed_vote(&commit_body, &keys[0]);
        let commit_vote_b = signed_vote(&commit_body, &keys[1]);
        let commit_qc = aggregate_lane_block_votes_to_qc(
            commit_body,
            validator_set,
            &[commit_vote_a, commit_vote_b],
        )
        .expect("commit QC");
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(drift_commit_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(proposal),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.drain_committed_sessions().is_empty(),
            "proposal reconciliation must drop body-drifted orphan commit QCs"
        );
        assert!(
            cache
                .get(&key)
                .expect("proposal session remains cached")
                .commit_qc
                .is_none()
        );
        assert_eq!(
            cache.insert_qc_with_pops(commit_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(cache.drain_committed_sessions().len(), 1);
    }

    #[test]
    fn lane_block_session_cache_seals_reconciled_orphan_vote_quorum() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let body = proposal.vote_body(CertPhase::Prepare);
        let vote_a = signed_vote(&body, &keys[0]);
        let vote_b = signed_vote(&body, &keys[1]);
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_vote(vote_a.clone(), Some(&vote_a.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(vote_b.clone(), Some(&vote_b.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache
                .get(&key)
                .expect("orphan session cached")
                .prepare_qc
                .is_none(),
            "orphan votes cannot seal before the proposal binds the validator set"
        );

        assert_eq!(
            cache.insert_proposal(proposal),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        let prepare_qc = cache
            .get(&key)
            .expect("proposal session cached")
            .prepare_qc
            .as_ref()
            .expect("reconciled orphan quorum seals prepare QC");
        validate_lane_block_qc_aggregate(prepare_qc, &pops)
            .expect("sealed orphan-vote QC aggregate verifies");
        let sealed = cache.drain_newly_sealed_qcs();
        assert_eq!(sealed.len(), 1);
        assert_eq!(sealed[0].body.phase, CertPhase::Prepare);
    }

    #[test]
    fn lane_block_session_cache_does_not_drain_inbound_qc() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let body = proposal.vote_body(CertPhase::Prepare);
        let vote_a = signed_vote(&body, &keys[0]);
        let vote_b = signed_vote(&body, &keys[1]);
        let qc = aggregate_lane_block_votes_to_qc(body, validator_set, &[vote_a, vote_b])
            .expect("lane block QC");
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_qc_with_pops(qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.drain_newly_sealed_qcs().is_empty(),
            "inbound QCs should not be treated as locally sealed transport work"
        );
    }

    #[test]
    fn lane_block_session_cache_rejects_conflicts_and_duplicate_replays() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let outsider = checked_bls_keypair(9);
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let body = proposal.vote_body(CertPhase::Prepare);
        let vote = signed_vote(&body, &keys[0]);
        let outsider_vote = signed_vote(&body, &outsider);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Duplicate)
        );
        assert_eq!(
            cache.insert_vote(vote.clone(), Some(&vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(vote.clone(), Some(&vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Duplicate)
        );
        assert_eq!(
            cache.insert_vote(outsider_vote, None),
            Err(LaneBlockSessionError::VoteSignerNotInValidatorSet)
        );

        let mut conflicting = proposal;
        conflicting.descriptor.subject_hash = Hash::prehashed([0xB0; Hash::LENGTH]);
        conflicting.descriptor.descriptor_hash = conflicting.descriptor.computed_descriptor_hash();
        conflicting.proposal_hash = conflicting.computed_proposal_hash();
        assert_eq!(
            cache.insert_proposal(conflicting),
            Err(LaneBlockSessionError::ConflictingProposal)
        );
    }

    #[test]
    fn lane_block_session_cache_rejects_forged_aggregate_qc() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let body = vote_body(&validator_set);
        let vote_a = signed_vote(&body, &keys[0]);
        let vote_c = signed_vote(&body, &keys[2]);
        let mut qc = aggregate_lane_block_votes_to_qc(body, validator_set, &[vote_a, vote_c])
            .expect("lane block QC");
        qc.bls_aggregate_signature[0] ^= 0x01;
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_qc_with_pops(qc, &pops),
            Err(LaneBlockSessionError::InvalidQc(
                LaneBlockQcIngressError::AggregateSignatureInvalid
            ))
        );
        assert!(
            cache.is_empty(),
            "forged aggregate QC must not populate the lane-block cache"
        );
    }

    #[test]
    fn lane_block_session_cache_reconciles_orphan_vote_drift_on_proposal() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let mut drift_body = proposal.vote_body(CertPhase::Prepare);
        drift_body.descriptor_hash = Hash::prehashed([0xC0; Hash::LENGTH]);
        let drift_vote = signed_vote(&drift_body, &keys[0]);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_vote(drift_vote.clone(), Some(&drift_vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache
                .get(&key)
                .expect("orphan session exists")
                .prepare_votes
                .len(),
            1
        );
        assert_eq!(
            cache.insert_proposal(proposal),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache
                .get(&key)
                .expect("proposal session exists")
                .prepare_votes
                .is_empty(),
            "proposal reconciliation must drop orphan votes whose body drifted"
        );
    }

    #[test]
    fn lane_block_session_cache_enforces_capacity() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal_a = lane_block_proposal(&validator_set);
        let key_a = LaneBlockSessionKey::from_proposal(&proposal_a);
        let mut proposal_b = lane_block_proposal(&validator_set);
        proposal_b.descriptor.lane_block_height =
            proposal_b.descriptor.lane_block_height.saturating_add(1);
        proposal_b.descriptor.previous_lane_block_height = proposal_b
            .descriptor
            .previous_lane_block_height
            .saturating_add(1);
        proposal_b.descriptor.descriptor_hash = proposal_b.descriptor.computed_descriptor_hash();
        proposal_b.proposal_hash = proposal_b.computed_proposal_hash();
        let key_b = LaneBlockSessionKey::from_proposal(&proposal_b);
        let mut cache = LaneBlockSessionCache::new(1);

        assert!(cache.is_empty());
        assert_eq!(
            cache.insert_proposal(proposal_a),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(proposal_b),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        assert_eq!(cache.len(), 1);
        assert!(cache.get(&key_a).is_none());
        assert!(cache.get(&key_b).is_some());
    }

    #[test]
    fn lane_block_session_cache_prunes_inactive_lane_sessions_and_slot_claims() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let active_lane = LaneId::new(7);
        let active_dataspace = DataSpaceId::new(11);
        let inactive_lane = LaneId::new(8);
        let inactive_dataspace = DataSpaceId::new(12);
        let active_proposal = lane_block_proposal_at_height(&validator_set, 13);
        let active_key = LaneBlockSessionKey::from_proposal(&active_proposal);
        let inactive_proposal = rebind_lane_block_proposal_route(
            lane_block_proposal_at_height(&validator_set, 13),
            inactive_lane,
            inactive_dataspace,
        );
        let inactive_key = LaneBlockSessionKey::from_proposal(&inactive_proposal);
        let conflicting_inactive_proposal =
            retag_lane_block_proposal_payload(inactive_proposal.clone(), 0xE0);
        assert_eq!(inactive_key.lane_id, inactive_lane);
        assert_eq!(inactive_key.dataspace_id, inactive_dataspace);
        assert_eq!(
            inactive_key.lane_block_height,
            LaneBlockSessionKey::from_proposal(&conflicting_inactive_proposal).lane_block_height
        );
        assert_ne!(
            inactive_proposal.proposal_hash,
            conflicting_inactive_proposal.proposal_hash
        );
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(active_proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(inactive_proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(cache.len(), 2);

        assert_eq!(
            cache.retain_sessions_for_active_lanes(|lane_id, dataspace_id| {
                lane_id == active_lane && dataspace_id == active_dataspace
            }),
            1
        );

        assert_eq!(cache.len(), 1);
        assert!(cache.get(&active_key).is_some());
        assert!(cache.get(&inactive_key).is_none());
        assert_eq!(
            cache.insert_proposal(conflicting_inactive_proposal),
            Ok(LaneBlockSessionInsertOutcome::Inserted),
            "pruning an inactive session must also release its slot claim"
        );
    }

    #[test]
    fn lane_block_session_cache_preserves_undrained_committed_session_under_backpressure() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal_a = lane_block_proposal_at_height(&validator_set, 13);
        let key_a = LaneBlockSessionKey::from_proposal(&proposal_a);
        let prepare_body = proposal_a.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set.clone(),
            &[prepare_vote_a, prepare_vote_b],
        )
        .expect("prepare QC");
        let commit_body = proposal_a.vote_body(CertPhase::Commit);
        let commit_vote_a = signed_vote(&commit_body, &keys[0]);
        let commit_vote_b = signed_vote(&commit_body, &keys[1]);
        let commit_qc = aggregate_lane_block_votes_to_qc(
            commit_body,
            validator_set.clone(),
            &[commit_vote_a, commit_vote_b],
        )
        .expect("commit QC");
        let proposal_b = lane_block_proposal_at_height(&validator_set, 14);
        let key_b = LaneBlockSessionKey::from_proposal(&proposal_b);
        let proposal_c = lane_block_proposal_at_height(&validator_set, 15);
        let key_c = LaneBlockSessionKey::from_proposal(&proposal_c);
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(1);

        assert_eq!(
            cache.insert_proposal(proposal_a.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc.clone(), &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(commit_qc.clone(), &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(proposal_b.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.get(&key_a).is_some(),
            "undrained certified lane-block session must survive cache backpressure"
        );
        assert!(
            cache.get(&key_b).is_some(),
            "ordinary sessions may coexist while committed evidence is protected"
        );
        assert_eq!(
            cache.len(),
            2,
            "protected committed evidence may temporarily exceed the ordinary cache capacity"
        );

        let committed = cache.drain_committed_sessions();
        assert_eq!(committed.len(), 1);
        assert_eq!(committed[0].proposal, proposal_a);
        assert_eq!(committed[0].prepare_qc, prepare_qc);
        assert_eq!(committed[0].commit_qc, commit_qc);

        assert_eq!(
            cache.insert_proposal(proposal_c),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.get(&key_a).is_none(),
            "drained committed evidence should become evictable again"
        );
        assert!(
            cache.get(&key_b).is_none(),
            "old uncommitted sessions should remain bounded after protected evidence drains"
        );
        assert!(cache.get(&key_c).is_some());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn aggregate_lane_block_votes_rejects_adversarial_vote_sets() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let outsider = checked_bls_keypair(9);
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let body = vote_body(&validator_set);
        let vote_a = signed_vote(&body, &keys[0]);
        let vote_b = signed_vote(&body, &keys[1]);

        assert_eq!(
            aggregate_lane_block_votes_to_qc(body.clone(), validator_set.clone(), &[]),
            Err(LaneBlockQcBuildError::EmptyVotes)
        );
        assert_eq!(
            aggregate_lane_block_votes_to_qc(
                body.clone(),
                validator_set.clone(),
                &[vote_a.clone()]
            ),
            Err(LaneBlockQcBuildError::QuorumNotMet)
        );
        assert_eq!(
            aggregate_lane_block_votes_to_qc(
                body.clone(),
                validator_set.clone(),
                &[vote_a.clone(), vote_a.clone()],
            ),
            Err(LaneBlockQcBuildError::DuplicateSigner)
        );

        let outsider_vote = signed_vote(&body, &outsider);
        assert_eq!(
            aggregate_lane_block_votes_to_qc(
                body.clone(),
                validator_set.clone(),
                &[vote_a.clone(), outsider_vote],
            ),
            Err(LaneBlockQcBuildError::SignerNotInValidatorSet)
        );

        let mut body_drift = body.clone();
        body_drift.descriptor_hash = Hash::prehashed([0xFE; Hash::LENGTH]);
        let drift_vote = signed_vote(&body_drift, &keys[1]);
        assert_eq!(
            aggregate_lane_block_votes_to_qc(
                body.clone(),
                validator_set.clone(),
                &[vote_a.clone(), drift_vote],
            ),
            Err(LaneBlockQcBuildError::BodyMismatch)
        );

        let mut hash_drift = body.clone();
        hash_drift.validator_set_hash =
            HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; Hash::LENGTH]));
        assert_eq!(
            aggregate_lane_block_votes_to_qc(hash_drift, validator_set, &[vote_a, vote_b]),
            Err(LaneBlockQcBuildError::ValidatorSetHashMismatch)
        );
    }
}
