//! Native AMX control-plane messages and deterministic vote-session cache.

use std::{
    collections::{BTreeMap, VecDeque},
    num::NonZeroUsize,
};

use iroha_crypto::{Algorithm, Hash, HashOf, Signature};
use iroha_data_model::{
    block::consensus::{
        LaneBlockProposalV1, NativeAmxAttestationBodyV1, NativeAmxAttestationQcV1, NativeAmxPhase,
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    peer::PeerId,
};
use norito::codec::{Decode, Encode};
use thiserror::Error;

const DEFAULT_SESSION_BODY_BUCKET_MAX: usize = MAX_NATIVE_AMX_PLAN_LEGS * 2;
/// Hard protocol cap for a coordinator plus all native AMX participant legs.
pub(crate) const MAX_NATIVE_AMX_PLAN_LEGS: usize = 256;
/// Hard protocol cap for one native AMX participant committee.
pub(crate) const MAX_NATIVE_AMX_VALIDATORS: usize = 128;
/// Canonical compressed BLS-normal signature/proof size.
pub(crate) const NATIVE_AMX_BLS_PROOF_BYTES: usize = 96;

use crate::queue::{RouteLeg, RouteLegRole, RoutingDecision, RoutingPlan};

/// Native AMX session key scoped to one source transaction and routing plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode)]
pub struct NativeAmxSessionKey {
    /// Source transaction hash/id.
    pub source_id: [u8; iroha_crypto::Hash::LENGTH],
    /// Full routing-plan digest.
    pub plan_digest: Hash,
}

impl NativeAmxSessionKey {
    /// Construct a session key from an attestation body.
    #[must_use]
    pub fn from_body(body: &NativeAmxAttestationBodyV1) -> Self {
        Self {
            source_id: body.source_id,
            plan_digest: body.plan_digest,
        }
    }
}

/// Individual native AMX vote before participant committee aggregation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NativeAmxVoteV1 {
    /// Body signed by the participant validator.
    pub body: NativeAmxAttestationBodyV1,
    /// Validator that produced the vote.
    pub signer: PeerId,
    /// BLS signature over [`NativeAmxAttestationBodyV1::signature_preimage`].
    pub bls_signature: Vec<u8>,
}

/// Full-plan request presented to a native AMX participant committee.
///
/// The signed attestation body carries the stable plan digest and the exact
/// participant leg. The complete canonical leg list is included so a signer
/// can independently recompute that digest and reject omitted, extra,
/// duplicated, or role-swapped routes before producing a vote.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct NativeAmxAttestationRequestV1 {
    /// Participant attestation body that will be signed after validation.
    pub body: NativeAmxAttestationBodyV1,
    /// Complete plan in coordinator-first canonical order.
    pub plan_legs: Vec<RouteLeg>,
    /// Exact coordinator lane proposal whose transaction membership is being attested.
    ///
    /// This proposal is a non-circular pre-commitment: its hash binds lane
    /// coordinates, committee, predecessor, and transaction hashes, but does
    /// not include the native AMX receipt assembled from the resulting votes.
    pub coordinator_proposal: LaneBlockProposalV1,
    /// Prepare certificate authorizing a commit-phase request.
    ///
    /// Prepare requests must carry `None`; commit requests must carry the
    /// exact participant prepare QC for the same session and committee.
    pub prepare_qc: Option<NativeAmxAttestationQcV1>,
}

/// Failure while validating a full-plan native AMX attestation request.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxRequestError {
    /// Request omitted a coordinator or every participant.
    #[error("native AMX request has an incomplete route plan")]
    IncompletePlan,
    /// Coordinator/participant roles or canonical ordering are invalid.
    #[error("native AMX request route roles or ordering are invalid")]
    InvalidRolesOrOrder,
    /// The same lane/dataspace route occurs more than once.
    #[error("native AMX request contains a duplicate route")]
    DuplicateRoute,
    /// The body names a coordinator or participant different from the plan.
    #[error("native AMX request body route does not match the full plan")]
    BodyRouteMismatch,
    /// The advertised digest does not commit to the supplied full plan.
    #[error("native AMX request plan digest mismatch")]
    PlanDigestMismatch,
    /// A request exceeds a protocol resource cap.
    #[error("native AMX request exceeds a protocol resource cap")]
    ResourceLimitExceeded,
    /// The supplied coordinator proposal is malformed.
    #[error("native AMX request coordinator proposal is malformed")]
    InvalidCoordinatorProposal,
    /// The attestation body does not bind the supplied coordinator proposal.
    #[error("native AMX request coordinator proposal binding mismatch")]
    CoordinatorProposalMismatch,
    /// Source id and transaction entrypoint hash do not identify the same transaction.
    #[error("native AMX request source and entrypoint hashes differ")]
    SourceEntrypointMismatch,
    /// Prepare/commit phase evidence is missing, unexpected, or mismatched.
    #[error("native AMX request phase evidence is invalid")]
    InvalidPhaseEvidence,
}

impl NativeAmxAttestationRequestV1 {
    /// Validate complete plan membership, canonical roles/order, and digest.
    ///
    /// # Errors
    /// Returns an error for malformed or replay-substituted plan evidence.
    pub fn validate_plan_binding(&self) -> Result<(), NativeAmxRequestError> {
        if self.plan_legs.len() > MAX_NATIVE_AMX_PLAN_LEGS
            || self.coordinator_proposal.descriptor.validator_set.len() > MAX_NATIVE_AMX_VALIDATORS
            || self
                .coordinator_proposal
                .descriptor
                .accepted_transaction_hashes
                .len()
                > crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS
            || self
                .coordinator_proposal
                .descriptor
                .accepted_candidate_indices
                .len()
                > crate::lane_consensus::MAX_LANE_EXECUTABLE_ENTRYPOINTS
        {
            return Err(NativeAmxRequestError::ResourceLimitExceeded);
        }
        let Some(coordinator) = self.plan_legs.first().copied() else {
            return Err(NativeAmxRequestError::IncompletePlan);
        };
        if coordinator.role != RouteLegRole::Coordinator || self.plan_legs.len() < 2 {
            return Err(NativeAmxRequestError::IncompletePlan);
        }
        let participants = &self.plan_legs[1..];
        let mut previous = None;
        let mut seen = std::collections::BTreeSet::new();
        if !seen.insert((coordinator.route.dataspace_id, coordinator.route.lane_id)) {
            return Err(NativeAmxRequestError::DuplicateRoute);
        }
        for participant in participants {
            if participant.role != RouteLegRole::Participant {
                return Err(NativeAmxRequestError::InvalidRolesOrOrder);
            }
            let key = (participant.route.dataspace_id, participant.route.lane_id);
            if previous.is_some_and(|previous| previous >= key) {
                return Err(if previous == Some(key) {
                    NativeAmxRequestError::DuplicateRoute
                } else {
                    NativeAmxRequestError::InvalidRolesOrOrder
                });
            }
            if !seen.insert(key) {
                return Err(NativeAmxRequestError::DuplicateRoute);
            }
            previous = Some(key);
        }
        let body = &self.body;
        if coordinator.route
            != RoutingDecision::new(body.coordinator_lane_id, body.coordinator_dataspace_id)
            || !participants.iter().any(|participant| {
                participant.route
                    == RoutingDecision::new(body.participant_lane_id, body.participant_dataspace_id)
            })
        {
            return Err(NativeAmxRequestError::BodyRouteMismatch);
        }
        let expected = RoutingPlan::native_amx(coordinator.route, participants.to_vec());
        if expected.digest() != body.plan_digest {
            return Err(NativeAmxRequestError::PlanDigestMismatch);
        }
        if body.tx_entrypoint_hash.as_ref() != body.source_id.as_slice() {
            return Err(NativeAmxRequestError::SourceEntrypointMismatch);
        }
        crate::lane_consensus::validate_lane_block_proposal(&self.coordinator_proposal)
            .map_err(|_| NativeAmxRequestError::InvalidCoordinatorProposal)?;
        let descriptor = &self.coordinator_proposal.descriptor;
        let entrypoint_hash = Hash::from(body.tx_entrypoint_hash);
        if self.coordinator_proposal.proposal_hash != body.coordinator_proposal_hash
            || descriptor.lane_id != body.coordinator_lane_id
            || descriptor.dataspace_id != body.coordinator_dataspace_id
            || descriptor.lane_incarnation != body.coordinator_lane_incarnation
            || descriptor.proposal_height != body.authority_context_height
            || descriptor.lane_block_height != body.coordinator_lane_block_height
            || descriptor.lane_block_view != body.coordinator_lane_block_view
            || descriptor
                .accepted_transaction_hashes
                .iter()
                .filter(|hash| **hash == entrypoint_hash)
                .count()
                != 1
        {
            return Err(NativeAmxRequestError::CoordinatorProposalMismatch);
        }
        match (body.phase, self.prepare_qc.as_ref()) {
            (NativeAmxPhase::Prepare, None) => {}
            (NativeAmxPhase::Commit, Some(prepare_qc)) => {
                let mut expected_prepare_body = body.clone();
                expected_prepare_body.phase = NativeAmxPhase::Prepare;
                if prepare_qc.body != expected_prepare_body
                    || prepare_qc.validator_set_hash != body.participant_validator_set_hash
                    || prepare_qc.validator_set.len()
                        != usize::try_from(body.participant_validator_count).unwrap_or(usize::MAX)
                    || prepare_qc.validator_set.len() > MAX_NATIVE_AMX_VALIDATORS
                    || prepare_qc.validator_set_pops.len() != prepare_qc.validator_set.len()
                    || prepare_qc.signers_bitmap.len() != prepare_qc.validator_set.len().div_ceil(8)
                    || prepare_qc.bls_aggregate_signature.len() != NATIVE_AMX_BLS_PROOF_BYTES
                {
                    return Err(NativeAmxRequestError::InvalidPhaseEvidence);
                }
            }
            _ => return Err(NativeAmxRequestError::InvalidPhaseEvidence),
        }
        Ok(())
    }
}

fn peer_uses_bls_normal(peer: &PeerId) -> bool {
    peer.public_key()
        .try_algorithm()
        .is_ok_and(|algorithm| algorithm == Algorithm::BlsNormal)
}

impl NativeAmxVoteV1 {
    /// Validate only bounded shape, phase, sender binding, and key algorithm.
    ///
    /// Call this before any committee lookup or cryptographic work. Stateful
    /// ingress must reject non-committee signers after this check and before
    /// calling [`Self::verify_signature`].
    pub fn validate_ingress_shape(
        &self,
        expected_phase: NativeAmxPhase,
        sender: Option<&PeerId>,
    ) -> Result<(), NativeAmxVoteIngressError> {
        if self.body.phase != expected_phase {
            return Err(NativeAmxVoteIngressError::PhaseMismatch {
                expected: expected_phase,
                actual: self.body.phase,
            });
        }
        if let Some(sender) = sender
            && sender != &self.signer
        {
            return Err(NativeAmxVoteIngressError::SenderMismatch);
        }
        if self.body.authority_context_height == 0
            || self.body.coordinator_lane_block_height == 0
            || self.body.participant_validator_count == 0
            || !usize::try_from(self.body.participant_validator_count)
                .is_ok_and(|count| count <= MAX_NATIVE_AMX_VALIDATORS)
            || self.body.participant_min_quorum == 0
            || self.body.participant_min_quorum > self.body.participant_validator_count
            || self.body.tx_entrypoint_hash.as_ref() != self.body.source_id.as_slice()
            || self
                .body
                .chain_id_hash
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || self.body.plan_digest.as_ref().iter().all(|byte| *byte == 0)
            || self
                .body
                .coordinator_lane_incarnation
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || self
                .body
                .participant_lane_incarnation
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
            || self
                .body
                .coordinator_proposal_hash
                .as_ref()
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(NativeAmxVoteIngressError::InvalidBody);
        }
        if !peer_uses_bls_normal(&self.signer) {
            return Err(NativeAmxVoteIngressError::SignerNotBlsNormal);
        }
        if self.bls_signature.len() != NATIVE_AMX_BLS_PROOF_BYTES {
            return Err(NativeAmxVoteIngressError::InvalidSignature);
        }
        Ok(())
    }

    /// Verify the already shape-checked BLS signature.
    pub fn verify_signature(&self) -> Result<(), NativeAmxVoteIngressError> {
        Signature::try_from_bytes(&self.bls_signature)
            .map_err(|_| NativeAmxVoteIngressError::InvalidSignature)?
            .verify(self.signer.public_key(), &self.body.signature_preimage())
            .map_err(|_| NativeAmxVoteIngressError::InvalidSignature)
    }

    /// Validate phase, transport signer binding, BLS-normal identity, and vote signature.
    ///
    /// This is the stateless ingress prefilter. Callers that know the current world state must
    /// still verify that the signer has a live proof of possession at the planned block height.
    ///
    /// # Errors
    /// Returns an error when the vote is carried by the wrong phase message, the authenticated
    /// sender does not match the signer, the signer is not BLS-normal, or the BLS signature does
    /// not verify against the canonical attestation preimage.
    pub fn validate_ingress(
        &self,
        expected_phase: NativeAmxPhase,
        sender: Option<&PeerId>,
    ) -> Result<(), NativeAmxVoteIngressError> {
        self.validate_ingress_shape(expected_phase, sender)?;
        self.verify_signature()
    }
}

/// Native AMX control-plane request or vote.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub enum NativeAmxMessage {
    /// Coordinator asks a participant dataspace committee to prepare a leg.
    PrepareRequest(NativeAmxAttestationRequestV1),
    /// Participant validator prepare vote.
    PrepareVote(NativeAmxVoteV1),
    /// Coordinator asks a participant dataspace committee to commit a prepared leg.
    CommitRequest(NativeAmxAttestationRequestV1),
    /// Participant validator commit vote.
    CommitVote(NativeAmxVoteV1),
}

/// Failure while validating a native AMX vote before session-cache insertion.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxVoteIngressError {
    /// native AMX vote message phase does not match the embedded body phase
    #[error("native AMX vote phase mismatch: expected {expected:?}, got {actual:?}")]
    PhaseMismatch {
        /// Phase implied by the received message variant.
        expected: NativeAmxPhase,
        /// Phase embedded in the signed attestation body.
        actual: NativeAmxPhase,
    },
    /// native AMX vote was transported by a peer other than the signer
    #[error("native AMX vote sender does not match signer")]
    SenderMismatch,
    /// native AMX vote body has invalid or oversized session coordinates
    #[error("native AMX vote body is malformed")]
    InvalidBody,
    /// native AMX vote signer is not a BLS-normal consensus identity
    #[error("native AMX vote signer is not BLS-normal")]
    SignerNotBlsNormal,
    /// native AMX vote signature is missing, malformed, or invalid
    #[error("native AMX vote signature is invalid")]
    InvalidSignature,
}

/// Failure while adding a native AMX vote to the session cache.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxSessionError {
    /// native AMX vote phase does not match the target cache bucket
    #[error("native AMX vote phase does not match the target cache bucket")]
    PhaseMismatch,
    /// native AMX vote signer already exists in this session
    #[error("native AMX vote signer already exists in this session")]
    DuplicateSigner,
    /// one source transaction attempted to occupy two live routing plans
    #[error("native AMX source transaction attempted routing-plan equivocation")]
    PlanEquivocation,
    /// vote does not match a request explicitly authorized by this coordinator
    #[error("native AMX vote does not match an authorized coordinator request")]
    UnauthorizedBody,
}

/// Failure while building a native AMX attestation QC from participant votes.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum NativeAmxQcBuildError {
    /// no votes were supplied for the requested native AMX phase
    #[error("no votes were supplied for the requested native AMX phase")]
    EmptyVotes,
    /// participant committee is empty, oversized, non-canonical, or duplicated
    #[error("native AMX participant validator set is malformed")]
    InvalidValidatorSet,
    /// signed participant committee hash/count/quorum does not match assembly inputs
    #[error("native AMX signed participant committee context mismatch")]
    CommitteeContextMismatch,
    /// aligned historical proof-of-possession material is malformed or invalid
    #[error("native AMX participant validator proof-of-possession is invalid")]
    InvalidProofOfPossession,
    /// a vote signed a different native AMX attestation body
    #[error("a vote signed a different native AMX attestation body")]
    BodyMismatch,
    /// a vote signer is not in the participant validator set
    #[error("a vote signer is not in the participant validator set")]
    SignerNotInValidatorSet,
    /// a vote signer appears more than once
    #[error("a vote signer appears more than once")]
    DuplicateSigner,
    /// a vote signer is not a BLS-normal consensus identity
    #[error("a vote signer is not BLS-normal")]
    SignerNotBlsNormal,
    /// an individual vote signature is missing, malformed, or invalid
    #[error("an individual native AMX vote signature is invalid")]
    InvalidSignature,
    /// the vote set does not satisfy the participant quorum
    #[error("native AMX vote quorum is not met")]
    QuorumNotMet,
    /// BLS signature aggregation failed
    #[error("failed to aggregate native AMX BLS signatures")]
    SignatureAggregate,
}

/// Failure while independently validating an embedded native AMX QC.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(crate) enum NativeAmxQcValidationError {
    /// QC vectors, committee context, bitmap, or proof sizes are malformed.
    #[error("native AMX QC shape is invalid")]
    InvalidShape,
    /// A committee key or aligned proof of possession is invalid.
    #[error("native AMX QC proof of possession is invalid")]
    InvalidProofOfPossession,
    /// The aggregate signature does not verify for the selected quorum.
    #[error("native AMX QC aggregate signature is invalid")]
    InvalidAggregateSignature,
}

/// Validate a body-bound native AMX committee and aggregate signature without live state.
///
/// Exact route/committee authority must have been checked when voters signed
/// the request. The signed committee hash/count/quorum plus aligned PoPs make
/// this proof restart-verifiable after key rotation or lane retirement.
pub(crate) fn validate_self_contained_qc(
    qc: &NativeAmxAttestationQcV1,
) -> Result<(), NativeAmxQcValidationError> {
    let Ok(validator_count) = usize::try_from(qc.body.participant_validator_count) else {
        return Err(NativeAmxQcValidationError::InvalidShape);
    };
    let Ok(min_quorum) = usize::try_from(qc.body.participant_min_quorum) else {
        return Err(NativeAmxQcValidationError::InvalidShape);
    };
    let expected_quorum =
        crate::sumeragi::network_topology::commit_quorum_from_len(validator_count).max(1);
    if validator_count == 0
        || validator_count > MAX_NATIVE_AMX_VALIDATORS
        || min_quorum != expected_quorum
        || qc.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
        || qc.validator_set.len() != validator_count
        || qc.validator_set.windows(2).any(|pair| pair[0] >= pair[1])
        || qc.validator_set_hash != qc.body.participant_validator_set_hash
        || qc.validator_set_hash != HashOf::new(&qc.validator_set)
        || qc.validator_set_pops.len() != validator_count
        || qc.signers_bitmap.len() != validator_count.div_ceil(8)
        || qc.bls_aggregate_signature.len() != NATIVE_AMX_BLS_PROOF_BYTES
    {
        return Err(NativeAmxQcValidationError::InvalidShape);
    }
    for (validator, pop) in qc.validator_set.iter().zip(&qc.validator_set_pops) {
        if !peer_uses_bls_normal(validator)
            || pop.len() != NATIVE_AMX_BLS_PROOF_BYTES
            || iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop).is_err()
        {
            return Err(NativeAmxQcValidationError::InvalidProofOfPossession);
        }
    }
    let mut signer_keys = Vec::with_capacity(validator_count);
    let mut signer_pops = Vec::with_capacity(validator_count);
    for (byte_index, byte) in qc.signers_bitmap.iter().copied().enumerate() {
        for bit in 0..8 {
            if byte & (1_u8 << bit) == 0 {
                continue;
            }
            let signer_index = byte_index * 8 + bit;
            if signer_index >= validator_count {
                return Err(NativeAmxQcValidationError::InvalidShape);
            }
            signer_keys.push(qc.validator_set[signer_index].public_key());
            signer_pops.push(qc.validator_set_pops[signer_index].as_slice());
        }
    }
    if signer_keys.len() < min_quorum {
        return Err(NativeAmxQcValidationError::InvalidShape);
    }
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &qc.body.signature_preimage(),
        &qc.bls_aggregate_signature,
        &signer_keys,
        &signer_pops,
    )
    .map_err(|_| NativeAmxQcValidationError::InvalidAggregateSignature)
}

/// Validate the bounded, producer-hashable shape of an aligned native AMX receipt.
///
/// This deliberately performs no aggregate cryptography or state lookup; block
/// and merge pre-execution must additionally validate the exact historical
/// route, committee authority, proofs of possession, and aggregate signatures.
#[must_use]
pub(crate) fn receipt_shape_matches_coordinator_payload(
    receipt: Option<&iroha_data_model::block::consensus::NativeAmxReceipt>,
    routing_plan: &RoutingPlan,
    expected_source_id: &[u8],
    expected_entrypoint_hash: Hash,
    expected_chain_id_hash: Hash,
    coordinator_proposal: &LaneBlockProposalV1,
) -> bool {
    let RoutingPlan::NativeAmx(native_plan) = routing_plan else {
        return receipt.is_none();
    };
    if native_plan.participants.is_empty()
        || native_plan.participants.len() >= MAX_NATIVE_AMX_PLAN_LEGS
    {
        return false;
    }
    let Some(receipt) = receipt else {
        return false;
    };
    let descriptor = &coordinator_proposal.descriptor;
    if receipt.version != 1
        || receipt.source_id.as_slice() != expected_source_id
        || receipt.source_id.as_slice() != expected_entrypoint_hash.as_ref()
        || receipt.chain_id_hash != expected_chain_id_hash
        || receipt.plan_digest != routing_plan.digest()
        || receipt.lane_id != descriptor.lane_id
        || receipt.dataspace_id != descriptor.dataspace_id
        || receipt.lane_incarnation != descriptor.lane_incarnation
        || receipt.authority_context_height != descriptor.proposal_height
        || receipt.lane_block_height != descriptor.lane_block_height
        || receipt.lane_block_view != descriptor.lane_block_view
        || receipt.coordinator_proposal_hash != coordinator_proposal.proposal_hash
        || receipt.legs.len() != native_plan.participants.len()
        || receipt.legs.len() > MAX_NATIVE_AMX_PLAN_LEGS
    {
        return false;
    }

    receipt
        .legs
        .iter()
        .zip(&native_plan.participants)
        .all(|(leg, planned)| {
            if leg.lane_id != planned.route.lane_id
                || leg.dataspace_id != planned.route.dataspace_id
                || leg.lane_incarnation.as_ref().iter().all(|byte| *byte == 0)
            {
                return false;
            }
            let prepare = &leg.prepare_qc;
            let commit = &leg.commit_qc;
            let common_qc_shape = |qc: &NativeAmxAttestationQcV1, phase: NativeAmxPhase| {
                let body = &qc.body;
                let Ok(validator_count) = usize::try_from(body.participant_validator_count) else {
                    return false;
                };
                let Ok(min_quorum) = usize::try_from(body.participant_min_quorum) else {
                    return false;
                };
                let expected_quorum =
                    crate::sumeragi::network_topology::commit_quorum_from_len(validator_count)
                        .max(1);
                let signer_count = qc
                    .signers_bitmap
                    .iter()
                    .map(|byte| byte.count_ones() as usize)
                    .sum::<usize>();
                let trailing_bits_clear = qc.signers_bitmap.last().is_none_or(|last| {
                    let used = validator_count % 8;
                    used == 0 || *last & !((1_u8 << used) - 1) == 0
                });
                body.chain_id_hash == expected_chain_id_hash
                    && body.source_id == receipt.source_id
                    && Hash::from(body.tx_entrypoint_hash) == expected_entrypoint_hash
                    && body.plan_digest == receipt.plan_digest
                    && body.phase == phase
                    && body.coordinator_lane_id == descriptor.lane_id
                    && body.coordinator_dataspace_id == descriptor.dataspace_id
                    && body.coordinator_lane_incarnation == descriptor.lane_incarnation
                    && body.participant_lane_id == leg.lane_id
                    && body.participant_dataspace_id == leg.dataspace_id
                    && body.participant_lane_incarnation == leg.lane_incarnation
                    && body.authority_context_height == descriptor.proposal_height
                    && body.coordinator_lane_block_height == descriptor.lane_block_height
                    && body.coordinator_lane_block_view == descriptor.lane_block_view
                    && body.coordinator_proposal_hash == coordinator_proposal.proposal_hash
                    && validator_count == qc.validator_set.len()
                    && validator_count > 0
                    && validator_count <= MAX_NATIVE_AMX_VALIDATORS
                    && min_quorum > 0
                    && min_quorum <= validator_count
                    && min_quorum == expected_quorum
                    && qc.validator_set_hash_version == VALIDATOR_SET_HASH_VERSION_V1
                    && qc.validator_set_hash == body.participant_validator_set_hash
                    && qc.validator_set_hash == HashOf::new(&qc.validator_set)
                    && qc.validator_set.windows(2).all(|pair| pair[0] < pair[1])
                    && qc.validator_set.iter().all(peer_uses_bls_normal)
                    && qc.validator_set_pops.len() == qc.validator_set.len()
                    && qc
                        .validator_set_pops
                        .iter()
                        .all(|pop| pop.len() == NATIVE_AMX_BLS_PROOF_BYTES)
                    && qc.signers_bitmap.len() == validator_count.div_ceil(8)
                    && trailing_bits_clear
                    && signer_count >= min_quorum
                    && qc.bls_aggregate_signature.len() == NATIVE_AMX_BLS_PROOF_BYTES
            };
            if !common_qc_shape(prepare, NativeAmxPhase::Prepare)
                || !common_qc_shape(commit, NativeAmxPhase::Commit)
                || prepare.validator_set != commit.validator_set
                || prepare.validator_set_pops != commit.validator_set_pops
                || prepare.validator_set_hash != commit.validator_set_hash
            {
                return false;
            }
            let mut expected_commit_body = prepare.body;
            expected_commit_body.phase = NativeAmxPhase::Commit;
            commit.body == expected_commit_body
        })
}

/// Build a native AMX attestation QC from sorted or unsorted participant votes.
///
/// The resulting bitmap and aggregate signature are deterministic because votes are projected into
/// the supplied validator-set order before aggregation.
///
/// # Errors
/// Returns an error when votes do not match `body`, include duplicate or unknown signers, fail to
/// meet `min_signers`, or cannot be aggregated as BLS-normal signatures.
pub fn aggregate_votes_to_qc(
    body: NativeAmxAttestationBodyV1,
    validator_set: Vec<PeerId>,
    validator_set_pops: Vec<Vec<u8>>,
    votes: &[NativeAmxVoteV1],
    min_signers: usize,
) -> Result<NativeAmxAttestationQcV1, NativeAmxQcBuildError> {
    if votes.is_empty() {
        return Err(NativeAmxQcBuildError::EmptyVotes);
    }
    if validator_set.is_empty()
        || validator_set.len() > MAX_NATIVE_AMX_VALIDATORS
        || votes.len() > validator_set.len()
        || validator_set.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(NativeAmxQcBuildError::InvalidValidatorSet);
    }
    let Ok(validator_count) = u32::try_from(validator_set.len()) else {
        return Err(NativeAmxQcBuildError::InvalidValidatorSet);
    };
    let Ok(min_quorum) = u32::try_from(min_signers) else {
        return Err(NativeAmxQcBuildError::CommitteeContextMismatch);
    };
    if min_signers == 0
        || min_signers > validator_set.len()
        || body.participant_validator_set_hash != HashOf::new(&validator_set)
        || body.participant_validator_count != validator_count
        || body.participant_min_quorum != min_quorum
    {
        return Err(NativeAmxQcBuildError::CommitteeContextMismatch);
    }
    if validator_set_pops.len() != validator_set.len()
        || validator_set_pops
            .iter()
            .any(|pop| pop.len() != NATIVE_AMX_BLS_PROOF_BYTES)
    {
        return Err(NativeAmxQcBuildError::InvalidProofOfPossession);
    }
    for (validator, pop) in validator_set.iter().zip(&validator_set_pops) {
        if !peer_uses_bls_normal(validator)
            || iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop).is_err()
        {
            return Err(NativeAmxQcBuildError::InvalidProofOfPossession);
        }
    }

    let mut indexed_signatures: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    for vote in votes {
        if vote.body != body {
            return Err(NativeAmxQcBuildError::BodyMismatch);
        }
        let Some(index) = validator_set
            .iter()
            .position(|validator| validator == &vote.signer)
        else {
            return Err(NativeAmxQcBuildError::SignerNotInValidatorSet);
        };
        if indexed_signatures
            .insert(index, vote.bls_signature.clone())
            .is_some()
        {
            return Err(NativeAmxQcBuildError::DuplicateSigner);
        }
        if !peer_uses_bls_normal(&vote.signer) {
            return Err(NativeAmxQcBuildError::SignerNotBlsNormal);
        }
        if vote.bls_signature.len() != NATIVE_AMX_BLS_PROOF_BYTES {
            return Err(NativeAmxQcBuildError::InvalidSignature);
        }
        let signature = Signature::try_from_bytes(&vote.bls_signature)
            .map_err(|_| NativeAmxQcBuildError::InvalidSignature)?;
        if signature
            .verify(vote.signer.public_key(), &body.signature_preimage())
            .is_err()
        {
            return Err(NativeAmxQcBuildError::InvalidSignature);
        }
    }

    if indexed_signatures.len() < min_signers {
        return Err(NativeAmxQcBuildError::QuorumNotMet);
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
        .map_err(|_| NativeAmxQcBuildError::SignatureAggregate)?;

    Ok(NativeAmxAttestationQcV1 {
        body,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set,
        validator_set_pops,
        signers_bitmap,
        bls_aggregate_signature,
    })
}

#[derive(Default)]
struct NativeAmxSession {
    order: VecDeque<NativeAmxVoteBucket>,
    votes: BTreeMap<NativeAmxVoteBucket, BTreeMap<PeerId, NativeAmxVoteV1>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NativeAmxVoteBucket {
    body: NativeAmxAttestationBodyV1,
}

impl NativeAmxVoteBucket {
    const fn from_body(body: &NativeAmxAttestationBodyV1) -> Self {
        Self { body: *body }
    }
}

impl NativeAmxSession {
    fn authorize_body(
        &mut self,
        body: &NativeAmxAttestationBodyV1,
        max_body_buckets: NonZeroUsize,
    ) {
        let bucket = NativeAmxVoteBucket::from_body(body);
        if self.votes.contains_key(&bucket) {
            return;
        }
        while self.votes.len() >= max_body_buckets.get() {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            self.votes.remove(&oldest);
        }
        self.order.push_back(bucket);
        self.votes.insert(bucket, BTreeMap::new());
    }

    fn insert_vote(&mut self, vote: NativeAmxVoteV1) -> Result<(), NativeAmxSessionError> {
        let bucket = NativeAmxVoteBucket::from_body(&vote.body);
        let Some(target) = self.votes.get_mut(&bucket) else {
            return Err(NativeAmxSessionError::UnauthorizedBody);
        };
        if target.contains_key(&vote.signer) {
            return Err(NativeAmxSessionError::DuplicateSigner);
        }
        target.insert(vote.signer.clone(), vote);
        Ok(())
    }

    fn votes_for_body(&self, body: &NativeAmxAttestationBodyV1) -> Vec<NativeAmxVoteV1> {
        self.votes
            .get(&NativeAmxVoteBucket::from_body(body))
            .map(|source| source.values().cloned().collect())
            .unwrap_or_default()
    }
}

/// Bounded cache of native AMX vote sessions keyed by source transaction and plan digest.
pub struct NativeAmxSessionCache {
    max_sessions: NonZeroUsize,
    max_body_buckets_per_session: NonZeroUsize,
    order: VecDeque<NativeAmxSessionKey>,
    sessions: BTreeMap<NativeAmxSessionKey, NativeAmxSession>,
    /// One live plan claim per source transaction, removed only when the
    /// corresponding bounded session is evicted.
    source_plan_claims: BTreeMap<[u8; iroha_crypto::Hash::LENGTH], Hash>,
}

impl NativeAmxSessionCache {
    /// Create a bounded native AMX session cache.
    #[must_use]
    pub fn new(max_sessions: NonZeroUsize) -> Self {
        Self::with_limits(
            max_sessions,
            NonZeroUsize::new(DEFAULT_SESSION_BODY_BUCKET_MAX).expect("default is non-zero"),
        )
    }

    /// Create a bounded native AMX session cache with an exact-body cap per session.
    #[must_use]
    pub fn with_limits(
        max_sessions: NonZeroUsize,
        max_body_buckets_per_session: NonZeroUsize,
    ) -> Self {
        Self {
            max_sessions,
            max_body_buckets_per_session,
            order: VecDeque::new(),
            sessions: BTreeMap::new(),
            source_plan_claims: BTreeMap::new(),
        }
    }

    fn ensure_session(
        &mut self,
        key: NativeAmxSessionKey,
    ) -> Result<&mut NativeAmxSession, NativeAmxSessionError> {
        if self
            .source_plan_claims
            .get(&key.source_id)
            .is_some_and(|claimed| *claimed != key.plan_digest)
        {
            return Err(NativeAmxSessionError::PlanEquivocation);
        }
        if !self.sessions.contains_key(&key) {
            while self.sessions.len() >= self.max_sessions.get() {
                let Some(oldest) = self.order.pop_front() else {
                    break;
                };
                self.sessions.remove(&oldest);
                if !self
                    .sessions
                    .keys()
                    .any(|candidate| candidate.source_id == oldest.source_id)
                {
                    self.source_plan_claims.remove(&oldest.source_id);
                }
            }
            self.order.push_back(key);
            self.source_plan_claims
                .insert(key.source_id, key.plan_digest);
        }
        Ok(self.sessions.entry(key).or_default())
    }

    /// Authorize one exact statefully validated request body for later vote ingress.
    ///
    /// Authorization is idempotent and consumes the same bounded FIFO body
    /// buckets as votes, so unsolicited valid committee signatures cannot
    /// create cache entries or evict an active coordinator session.
    pub fn authorize_request(
        &mut self,
        request: &NativeAmxAttestationRequestV1,
    ) -> Result<(), NativeAmxSessionError> {
        let key = NativeAmxSessionKey::from_body(&request.body);
        let max_body_buckets = self.max_body_buckets_per_session;
        self.ensure_session(key)?
            .authorize_body(&request.body, max_body_buckets);
        Ok(())
    }

    /// Return whether an exact body has been authorized by a coordinator request.
    #[must_use]
    pub fn is_authorized_body(&self, body: &NativeAmxAttestationBodyV1) -> bool {
        let key = NativeAmxSessionKey::from_body(body);
        self.sessions.get(&key).is_some_and(|session| {
            session
                .votes
                .contains_key(&NativeAmxVoteBucket::from_body(body))
        })
    }

    /// Insert a vote, rejecting duplicate signers for the same exact attestation body.
    ///
    /// Eviction is deterministic FIFO by session key insertion order.
    ///
    /// # Errors
    /// Returns [`NativeAmxSessionError::DuplicateSigner`] when a signer votes twice for one body.
    pub fn insert_vote(&mut self, vote: NativeAmxVoteV1) -> Result<(), NativeAmxSessionError> {
        let key = NativeAmxSessionKey::from_body(&vote.body);
        if self
            .source_plan_claims
            .get(&key.source_id)
            .is_some_and(|claimed| *claimed != key.plan_digest)
        {
            return Err(NativeAmxSessionError::PlanEquivocation);
        }
        let Some(session) = self.sessions.get_mut(&key) else {
            return Err(NativeAmxSessionError::UnauthorizedBody);
        };
        session.insert_vote(vote)
    }

    /// Return votes sorted deterministically by signer id for a session phase.
    #[must_use]
    pub fn sorted_votes(
        &self,
        key: NativeAmxSessionKey,
        phase: NativeAmxPhase,
    ) -> Vec<NativeAmxVoteV1> {
        self.sessions
            .get(&key)
            .map(|session| {
                session
                    .votes
                    .iter()
                    .filter(|(bucket, _)| bucket.body.phase == phase)
                    .flat_map(|(_, votes)| votes.values().cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return votes sorted deterministically by signer id for an exact participant body.
    #[must_use]
    pub fn sorted_votes_for_body(
        &self,
        key: NativeAmxSessionKey,
        body: &NativeAmxAttestationBodyV1,
    ) -> Vec<NativeAmxVoteV1> {
        self.sessions
            .get(&key)
            .map(|session| session.votes_for_body(body))
            .unwrap_or_default()
    }

    /// Return exact-body votes restricted to the validator set used for QC assembly.
    #[must_use]
    pub fn sorted_votes_for_body_from(
        &self,
        key: NativeAmxSessionKey,
        body: &NativeAmxAttestationBodyV1,
        validator_set: &[PeerId],
    ) -> Vec<NativeAmxVoteV1> {
        self.sorted_votes_for_body(key, body)
            .into_iter()
            .filter(|vote| {
                validator_set
                    .iter()
                    .any(|validator| validator == &vote.signer)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        block::consensus::{LaneBlockDescriptorV1, NativeAmxAttestationBodyV1},
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
        transaction::TransactionEntrypoint,
    };

    use super::*;

    fn checked_random_ed25519_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .expect("generate checked native AMX fixture keypair")
    }

    fn checked_bls_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
            .expect("generate checked native AMX BLS fixture keypair")
    }

    fn checked_bls_signature_payload(keypair: &KeyPair, message: &[u8]) -> Vec<u8> {
        let signature = Signature::try_new(keypair.private_key(), message)
            .expect("checked native AMX vote fixture signature");
        signature
            .verify(keypair.public_key(), message)
            .expect("checked native AMX vote fixture signature verifies");
        signature.payload().to_vec()
    }

    fn body(phase: NativeAmxPhase) -> NativeAmxAttestationBodyV1 {
        let participant_validator_set =
            vec![PeerId::new(checked_bls_keypair(0xD0).public_key().clone())];
        NativeAmxAttestationBodyV1 {
            chain_id_hash: Hash::new(b"native-amx-test-chain"),
            source_id: [0xCD; iroha_crypto::Hash::LENGTH],
            tx_entrypoint_hash:
                iroha_crypto::HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
                    Hash::prehashed([0xCD; iroha_crypto::Hash::LENGTH]),
                ),
            plan_digest: Hash::new(b"native-amx-plan"),
            phase,
            coordinator_lane_id: LaneId::new(1),
            coordinator_dataspace_id: DataSpaceId::new(7),
            coordinator_lane_incarnation: Hash::new(b"native-amx-test-coordinator"),
            participant_lane_id: LaneId::new(2),
            participant_dataspace_id: DataSpaceId::new(8),
            participant_lane_incarnation: Hash::new(b"native-amx-test-participant"),
            participant_validator_set_hash: HashOf::new(&participant_validator_set),
            participant_validator_count: 1,
            participant_min_quorum: 1,
            authority_context_height: 42,
            coordinator_lane_block_height: 9,
            coordinator_lane_block_view: 3,
            coordinator_proposal_hash: Hash::new(b"native-amx-test-proposal"),
        }
    }

    fn request(phase: NativeAmxPhase) -> NativeAmxAttestationRequestV1 {
        let coordinator = RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7));
        let participants = vec![
            RouteLeg::new(
                RoutingDecision::new(LaneId::new(2), DataSpaceId::new(8)),
                RouteLegRole::Participant,
            ),
            RouteLeg::new(
                RoutingDecision::new(LaneId::new(3), DataSpaceId::new(9)),
                RouteLegRole::Participant,
            ),
        ];
        let plan = RoutingPlan::native_amx(coordinator, participants);
        let mut body = body(phase);
        body.plan_digest = plan.digest();
        let validator_set = vec![PeerId::new(checked_bls_keypair(0xD1).public_key().clone())];
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: body.coordinator_lane_id,
            dataspace_id: body.coordinator_dataspace_id,
            lane_incarnation: body.coordinator_lane_incarnation,
            proposal_height: body.authority_context_height,
            previous_lane_block_height: body.coordinator_lane_block_height - 1,
            previous_lane_block_descriptor_hash: Some(Hash::new(b"native-amx-test-previous")),
            lane_block_height: body.coordinator_lane_block_height,
            lane_block_view: body.coordinator_lane_block_view,
            subject_hash: Hash::new(b"native-amx-test-subject"),
            payload_ownership_hash: Hash::new(b"native-amx-test-ownership"),
            rbc_instance_hash: Hash::new(b"native-amx-test-rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::from(body.tx_entrypoint_hash)],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set,
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: "native-amx:test-lane".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut coordinator_proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        coordinator_proposal.proposal_hash = coordinator_proposal.computed_proposal_hash();
        body.coordinator_proposal_hash = coordinator_proposal.proposal_hash;
        let prepare_qc = (phase == NativeAmxPhase::Commit).then(|| {
            let mut prepare_body = body;
            prepare_body.phase = NativeAmxPhase::Prepare;
            NativeAmxAttestationQcV1 {
                body: prepare_body,
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: prepare_body.participant_validator_set_hash,
                validator_set: participant_validator_set_for_body(&prepare_body),
                validator_set_pops: vec![vec![0_u8; NATIVE_AMX_BLS_PROOF_BYTES]],
                signers_bitmap: vec![1],
                bls_aggregate_signature: vec![0_u8; NATIVE_AMX_BLS_PROOF_BYTES],
            }
        });
        NativeAmxAttestationRequestV1 {
            body,
            plan_legs: plan.legs(),
            coordinator_proposal,
            prepare_qc,
        }
    }

    fn participant_validator_set_for_body(body: &NativeAmxAttestationBodyV1) -> Vec<PeerId> {
        let validator_set = vec![PeerId::new(checked_bls_keypair(0xD0).public_key().clone())];
        assert_eq!(
            HashOf::new(&validator_set),
            body.participant_validator_set_hash
        );
        validator_set
    }

    #[test]
    fn attestation_request_binds_complete_canonical_plan_and_roles() {
        request(NativeAmxPhase::Prepare)
            .validate_plan_binding()
            .expect("canonical request");
    }

    #[test]
    fn attestation_request_rejects_omitted_extra_duplicate_and_role_swapped_legs() {
        let canonical = request(NativeAmxPhase::Prepare);

        let mut omitted = canonical.clone();
        omitted.plan_legs.pop();
        assert_eq!(
            omitted.validate_plan_binding(),
            Err(NativeAmxRequestError::PlanDigestMismatch)
        );

        let mut extra = canonical.clone();
        extra.plan_legs.push(RouteLeg::new(
            RoutingDecision::new(LaneId::new(4), DataSpaceId::new(10)),
            RouteLegRole::Participant,
        ));
        assert_eq!(
            extra.validate_plan_binding(),
            Err(NativeAmxRequestError::PlanDigestMismatch)
        );

        let mut duplicate = canonical.clone();
        duplicate
            .plan_legs
            .push(*duplicate.plan_legs.last().expect("participant"));
        assert_eq!(
            duplicate.validate_plan_binding(),
            Err(NativeAmxRequestError::DuplicateRoute)
        );

        let mut role_swapped = canonical;
        role_swapped.plan_legs[0].role = RouteLegRole::Participant;
        role_swapped.plan_legs[1].role = RouteLegRole::Coordinator;
        assert_eq!(
            role_swapped.validate_plan_binding(),
            Err(NativeAmxRequestError::IncompletePlan)
        );
    }

    #[test]
    fn attestation_request_rejects_wrong_participant_and_cross_plan_replay() {
        let canonical = request(NativeAmxPhase::Commit);
        let mut wrong_participant = canonical.clone();
        wrong_participant.body.participant_dataspace_id = DataSpaceId::new(99);
        assert_eq!(
            wrong_participant.validate_plan_binding(),
            Err(NativeAmxRequestError::BodyRouteMismatch)
        );

        let mut cross_plan = canonical;
        let other_plan = RoutingPlan::native_amx(
            RoutingDecision::new(LaneId::new(1), DataSpaceId::new(7)),
            vec![RouteLeg::new(
                RoutingDecision::new(LaneId::new(5), DataSpaceId::new(12)),
                RouteLegRole::Participant,
            )],
        );
        cross_plan.body.plan_digest = other_plan.digest();
        assert_eq!(
            cross_plan.validate_plan_binding(),
            Err(NativeAmxRequestError::PlanDigestMismatch)
        );
    }

    #[test]
    fn commit_request_requires_exact_cryptographic_prepare_evidence() {
        let mut missing = request(NativeAmxPhase::Commit);
        missing.prepare_qc = None;
        assert_eq!(
            missing.validate_plan_binding(),
            Err(NativeAmxRequestError::InvalidPhaseEvidence)
        );

        let forged = request(NativeAmxPhase::Commit);
        forged
            .validate_plan_binding()
            .expect("bounded relational shape is checked before cryptography");
        assert_eq!(
            validate_self_contained_qc(forged.prepare_qc.as_ref().expect("fixture prepare QC")),
            Err(NativeAmxQcValidationError::InvalidProofOfPossession),
            "zero-filled PoP/signature evidence must fail closed"
        );

        let mut unexpected = request(NativeAmxPhase::Prepare);
        unexpected.prepare_qc = forged.prepare_qc;
        assert_eq!(
            unexpected.validate_plan_binding(),
            Err(NativeAmxRequestError::InvalidPhaseEvidence)
        );
    }

    fn vote(phase: NativeAmxPhase) -> NativeAmxVoteV1 {
        let keypair = checked_random_ed25519_keypair();
        NativeAmxVoteV1 {
            body: body(phase),
            signer: PeerId::new(keypair.public_key().clone()),
            bls_signature: vec![0xA5; 96],
        }
    }

    fn authorize_body(cache: &mut NativeAmxSessionCache, body: &NativeAmxAttestationBodyV1) {
        let mut authorized = request(body.phase);
        authorized.body = *body;
        cache
            .authorize_request(&authorized)
            .expect("authorize exact native AMX test body");
    }

    #[test]
    fn session_cache_rejects_duplicate_signer() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        authorize_body(&mut cache, &vote.body);
        cache
            .insert_vote(vote.clone())
            .expect("first vote should insert");
        assert!(matches!(
            cache.insert_vote(vote),
            Err(NativeAmxSessionError::DuplicateSigner)
        ));
    }

    #[test]
    fn session_cache_rejects_live_source_plan_equivocation() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let first = vote(NativeAmxPhase::Prepare);
        let mut conflicting = first.clone();
        conflicting.body.plan_digest = Hash::new(b"conflicting-native-amx-plan");

        authorize_body(&mut cache, &first.body);
        cache.insert_vote(first).expect("first source plan claim");
        assert_eq!(
            cache.insert_vote(conflicting),
            Err(NativeAmxSessionError::PlanEquivocation),
            "one live source must not collect votes under two routing plans"
        );
    }

    #[test]
    fn session_cache_plan_claim_lifetime_matches_session_eviction() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(1).expect("nonzero"));
        let first = vote(NativeAmxPhase::Prepare);
        let mut replacement_source = first.clone();
        replacement_source.body.source_id = [0xBC; iroha_crypto::Hash::LENGTH];
        let mut recycled = first.clone();
        recycled.body.plan_digest = Hash::new(b"recycled-source-new-plan");

        authorize_body(&mut cache, &first.body);
        cache.insert_vote(first).expect("initial claim");
        authorize_body(&mut cache, &replacement_source.body);
        cache
            .insert_vote(replacement_source)
            .expect("different source evicts initial claim");
        // Rebuild a distinct body/signer so the assertion specifically proves
        // source-plan claim eviction rather than duplicate-vote behavior.
        recycled.signer = vote(NativeAmxPhase::Commit).signer;
        authorize_body(&mut cache, &recycled.body);
        cache
            .insert_vote(recycled)
            .expect("evicted source may later establish a new bounded claim");
    }

    #[test]
    fn session_cache_allows_same_signer_for_retried_body() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&vote.body);
        let mut retried_vote = vote.clone();
        retried_vote.body.coordinator_lane_block_view = retried_vote
            .body
            .coordinator_lane_block_view
            .saturating_add(1);

        authorize_body(&mut cache, &vote.body);
        authorize_body(&mut cache, &retried_vote.body);
        cache.insert_vote(vote.clone()).expect("first body vote");
        cache
            .insert_vote(retried_vote.clone())
            .expect("same signer may vote on a retried body");

        assert_eq!(cache.sorted_votes_for_body(key, &vote.body), vec![vote]);
        assert_eq!(
            cache.sorted_votes_for_body(key, &retried_vote.body),
            vec![retried_vote]
        );
        assert_eq!(cache.sorted_votes(key, NativeAmxPhase::Prepare).len(), 2);
    }

    #[test]
    fn session_cache_allows_same_signer_for_different_participant_legs() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&vote.body);
        let mut other_leg = vote.clone();
        other_leg.body.participant_lane_id = LaneId::new(9);
        other_leg.body.participant_dataspace_id = DataSpaceId::new(10);

        authorize_body(&mut cache, &vote.body);
        authorize_body(&mut cache, &other_leg.body);
        cache.insert_vote(vote.clone()).expect("first leg vote");
        cache
            .insert_vote(other_leg.clone())
            .expect("same signer may vote on another participant leg");

        assert_eq!(cache.sorted_votes_for_body(key, &vote.body), vec![vote]);
        assert_eq!(
            cache.sorted_votes_for_body(key, &other_leg.body),
            vec![other_leg]
        );
    }

    #[test]
    fn session_cache_filters_exact_body_votes_to_validator_set() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let allowed_keypair = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("generate checked allowed native AMX BLS fixture keypair");
        let unknown_keypair = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("generate checked unknown native AMX BLS fixture keypair");
        let allowed = PeerId::new(allowed_keypair.public_key().clone());
        let unknown = PeerId::new(unknown_keypair.public_key().clone());
        let body = body(NativeAmxPhase::Prepare);
        let allowed_vote = NativeAmxVoteV1 {
            body,
            signer: allowed.clone(),
            bls_signature: vec![1],
        };
        let unknown_vote = NativeAmxVoteV1 {
            body,
            signer: unknown,
            bls_signature: vec![2],
        };
        let key = NativeAmxSessionKey::from_body(&body);

        authorize_body(&mut cache, &body);
        cache
            .insert_vote(allowed_vote.clone())
            .expect("allowed signer vote");
        cache
            .insert_vote(unknown_vote)
            .expect("unknown signer vote");

        assert_eq!(
            cache.sorted_votes_for_body_from(key, &body, &[allowed]),
            vec![allowed_vote]
        );
    }

    #[test]
    fn session_cache_eviction_is_fifo() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(1).expect("nonzero"));
        let first = vote(NativeAmxPhase::Prepare);
        let first_key = NativeAmxSessionKey::from_body(&first.body);
        authorize_body(&mut cache, &first.body);
        cache.insert_vote(first).expect("first vote");

        let mut second = vote(NativeAmxPhase::Prepare);
        second.body.source_id = [0xAC; iroha_crypto::Hash::LENGTH];
        let second_key = NativeAmxSessionKey::from_body(&second.body);
        authorize_body(&mut cache, &second.body);
        cache.insert_vote(second).expect("second vote");

        assert!(
            cache
                .sorted_votes(first_key, NativeAmxPhase::Prepare)
                .is_empty()
        );
        assert_eq!(
            cache
                .sorted_votes(second_key, NativeAmxPhase::Prepare)
                .len(),
            1
        );
    }

    #[test]
    fn session_cache_evicts_oldest_body_bucket_within_session() {
        let mut cache = NativeAmxSessionCache::with_limits(
            NonZeroUsize::new(4).expect("nonzero sessions"),
            NonZeroUsize::new(2).expect("nonzero body buckets"),
        );
        let first = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&first.body);
        let mut second = first.clone();
        second.body.coordinator_lane_block_view = 43;
        let mut third = first.clone();
        third.body.coordinator_lane_block_view = 44;

        authorize_body(&mut cache, &first.body);
        authorize_body(&mut cache, &second.body);
        authorize_body(&mut cache, &third.body);
        cache.insert_vote(first.clone()).expect("first vote");
        cache.insert_vote(second.clone()).expect("second vote");
        cache.insert_vote(third.clone()).expect("third vote");

        assert!(
            cache.sorted_votes_for_body(key, &first.body).is_empty(),
            "oldest exact-body bucket should be evicted"
        );
        assert_eq!(cache.sorted_votes_for_body(key, &second.body), vec![second]);
        assert_eq!(cache.sorted_votes_for_body(key, &third.body), vec![third]);
        assert_eq!(cache.sorted_votes(key, NativeAmxPhase::Prepare).len(), 2);
    }

    #[test]
    fn unsolicited_vote_flood_cannot_allocate_or_evict_authorized_session() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(1).expect("nonzero"));
        let authorized = vote(NativeAmxPhase::Prepare);
        authorize_body(&mut cache, &authorized.body);
        cache
            .insert_vote(authorized.clone())
            .expect("authorized vote inserts");
        let authorized_key = NativeAmxSessionKey::from_body(&authorized.body);

        for tag in 0_u16..1_000 {
            let mut unsolicited = vote(NativeAmxPhase::Prepare);
            unsolicited.body.source_id[..2].copy_from_slice(&tag.to_le_bytes());
            unsolicited.body.tx_entrypoint_hash =
                HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed({
                    let mut bytes = [0_u8; Hash::LENGTH];
                    bytes[..2].copy_from_slice(&tag.to_le_bytes());
                    bytes
                }));
            assert_eq!(
                cache.insert_vote(unsolicited),
                Err(NativeAmxSessionError::UnauthorizedBody)
            );
        }

        assert_eq!(cache.sessions.len(), 1);
        assert_eq!(
            cache.sorted_votes_for_body(authorized_key, &authorized.body),
            vec![authorized]
        );
    }

    #[test]
    fn request_rejects_oversized_plan_before_hashing_or_signature_work() {
        let mut oversized = request(NativeAmxPhase::Prepare);
        let participant = *oversized.plan_legs.last().expect("participant");
        oversized.plan_legs = vec![participant; MAX_NATIVE_AMX_PLAN_LEGS + 1];
        assert_eq!(
            oversized.validate_plan_binding(),
            Err(NativeAmxRequestError::ResourceLimitExceeded)
        );
    }

    #[test]
    fn new_view_requires_exact_coordinator_proposal_rebinding() {
        let original = request(NativeAmxPhase::Prepare);
        let mut stale_body = original.clone();
        stale_body.body.coordinator_lane_block_view += 1;
        assert_eq!(
            stale_body.validate_plan_binding(),
            Err(NativeAmxRequestError::CoordinatorProposalMismatch)
        );

        let mut transitioned = original;
        transitioned.coordinator_proposal.descriptor.lane_block_view += 1;
        transitioned.coordinator_proposal.descriptor.descriptor_hash = transitioned
            .coordinator_proposal
            .descriptor
            .computed_descriptor_hash();
        transitioned.coordinator_proposal.proposal_hash =
            transitioned.coordinator_proposal.computed_proposal_hash();
        transitioned.body.coordinator_lane_block_view =
            transitioned.coordinator_proposal.descriptor.lane_block_view;
        transitioned.body.coordinator_proposal_hash =
            transitioned.coordinator_proposal.proposal_hash;
        transitioned
            .validate_plan_binding()
            .expect("exact next-view proposal may collect a distinct attestation");
        assert_ne!(
            request(NativeAmxPhase::Prepare).body.signature_preimage(),
            transitioned.body.signature_preimage(),
            "a prior-view signature must not authorize the transitioned proposal"
        );
    }

    #[test]
    fn lane_incarnation_aba_changes_attestation_preimage_and_breaks_proposal_binding() {
        let canonical = request(NativeAmxPhase::Commit);
        let mut replay = canonical.clone();
        replay.body.coordinator_lane_incarnation = Hash::new(b"recreated-coordinator-lane");
        assert_ne!(
            canonical.body.signature_preimage(),
            replay.body.signature_preimage()
        );
        assert_eq!(
            replay.validate_plan_binding(),
            Err(NativeAmxRequestError::CoordinatorProposalMismatch)
        );
    }

    fn signed_vote(body: &NativeAmxAttestationBodyV1, keypair: &KeyPair) -> NativeAmxVoteV1 {
        NativeAmxVoteV1 {
            body: body.clone(),
            signer: PeerId::new(keypair.public_key().clone()),
            bls_signature: checked_bls_signature_payload(keypair, &body.signature_preimage()),
        }
    }

    fn sorted_committee<'a>(keypairs: &'a [KeyPair]) -> Vec<(PeerId, &'a KeyPair)> {
        let mut members = keypairs
            .iter()
            .map(|keypair| (PeerId::new(keypair.public_key().clone()), keypair))
            .collect::<Vec<_>>();
        members.sort_by(|left, right| left.0.cmp(&right.0));
        members
    }

    fn committee_pops(members: &[(PeerId, &KeyPair)]) -> Vec<Vec<u8>> {
        members
            .iter()
            .map(|(_, keypair)| {
                iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                    .expect("native AMX committee fixture PoP")
            })
            .collect()
    }

    fn bind_body_committee(
        mut body: NativeAmxAttestationBodyV1,
        validator_set: &[PeerId],
        min_quorum: usize,
    ) -> NativeAmxAttestationBodyV1 {
        body.participant_validator_set_hash = HashOf::new(&validator_set.to_vec());
        body.participant_validator_count =
            u32::try_from(validator_set.len()).expect("fixture committee count");
        body.participant_min_quorum = u32::try_from(min_quorum).expect("fixture quorum");
        body
    }

    #[test]
    fn vote_ingress_validation_accepts_matching_signed_bls_vote() {
        let keypair = checked_bls_keypair(0xE1);
        let body = body(NativeAmxPhase::Prepare);
        let vote = signed_vote(&body, &keypair);
        let sender = vote.signer.clone();

        assert_eq!(
            vote.validate_ingress(NativeAmxPhase::Prepare, Some(&sender)),
            Ok(())
        );
    }

    #[test]
    fn vote_ingress_validation_rejects_phase_and_sender_mismatches() {
        let keypair = checked_bls_keypair(0xE2);
        let other_keypair = checked_bls_keypair(0xE3);
        let body = body(NativeAmxPhase::Prepare);
        let vote = signed_vote(&body, &keypair);
        let sender = vote.signer.clone();
        let other_sender = PeerId::new(other_keypair.public_key().clone());

        assert_eq!(
            vote.validate_ingress(NativeAmxPhase::Commit, Some(&sender)),
            Err(NativeAmxVoteIngressError::PhaseMismatch {
                expected: NativeAmxPhase::Commit,
                actual: NativeAmxPhase::Prepare
            })
        );
        assert_eq!(
            vote.validate_ingress(NativeAmxPhase::Prepare, Some(&other_sender)),
            Err(NativeAmxVoteIngressError::SenderMismatch)
        );
    }

    #[test]
    fn vote_ingress_validation_rejects_non_bls_and_bad_signatures() {
        let ed25519_keypair = checked_random_ed25519_keypair();
        let body = body(NativeAmxPhase::Commit);
        let ed25519_signature =
            Signature::try_new(ed25519_keypair.private_key(), &body.signature_preimage())
                .expect("checked Ed25519 fixture signature")
                .payload()
                .to_vec();
        let ed25519_vote = NativeAmxVoteV1 {
            body,
            signer: PeerId::new(ed25519_keypair.public_key().clone()),
            bls_signature: ed25519_signature,
        };

        assert_eq!(
            ed25519_vote.validate_ingress(NativeAmxPhase::Commit, None),
            Err(NativeAmxVoteIngressError::SignerNotBlsNormal)
        );

        let bls_keypair = checked_bls_keypair(0xE4);
        let mut bad_signature_vote = signed_vote(&body, &bls_keypair);
        bad_signature_vote.bls_signature = vec![0_u8; 96];

        assert_eq!(
            bad_signature_vote.validate_ingress(NativeAmxPhase::Commit, None),
            Err(NativeAmxVoteIngressError::InvalidSignature)
        );
    }

    #[test]
    fn aggregate_votes_to_qc_orders_votes_by_validator_set() {
        let keypairs = [
            checked_bls_keypair(0xA1),
            checked_bls_keypair(0xB2),
            checked_bls_keypair(0xC3),
        ];
        let members = sorted_committee(&keypairs);
        let validator_set = members
            .iter()
            .map(|(peer, _)| peer.clone())
            .collect::<Vec<_>>();
        let validator_set_pops = committee_pops(&members);
        let body = bind_body_committee(body(NativeAmxPhase::Commit), &validator_set, 2);
        let votes = vec![
            signed_vote(&body, &keypairs[2]),
            signed_vote(&body, &keypairs[0]),
        ];

        let qc = aggregate_votes_to_qc(
            body,
            validator_set.clone(),
            validator_set_pops.clone(),
            &votes,
            2,
        )
        .expect("valid quorum should aggregate");

        assert_eq!(qc.body, body);
        assert_eq!(qc.validator_set, validator_set);
        assert_eq!(qc.validator_set_pops, validator_set_pops);
        let expected_bitmap = [&keypairs[0], &keypairs[2]]
            .iter()
            .fold(0_u8, |bitmap, keypair| {
                let signer = PeerId::new(keypair.public_key().clone());
                let index = qc
                    .validator_set
                    .iter()
                    .position(|peer| peer == &signer)
                    .expect("fixture signer in committee");
                bitmap | (1_u8 << index)
            });
        assert_eq!(qc.signers_bitmap, vec![expected_bitmap]);
        let individual_signatures = members
            .iter()
            .filter(|(_, keypair)| {
                keypair.public_key() == keypairs[0].public_key()
                    || keypair.public_key() == keypairs[2].public_key()
            })
            .map(|(_, keypair)| signed_vote(&body, keypair).bls_signature)
            .collect::<Vec<_>>();
        let signature_refs = individual_signatures
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let expected_aggregate = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("aggregate reference signatures");
        assert_eq!(qc.bls_aggregate_signature, expected_aggregate);
    }

    #[test]
    fn authorized_remote_votes_converge_to_three_of_four_qc() {
        let keypairs = [
            checked_bls_keypair(0x91),
            checked_bls_keypair(0x92),
            checked_bls_keypair(0x93),
            checked_bls_keypair(0x94),
        ];
        let members = sorted_committee(&keypairs);
        let validator_set = members
            .iter()
            .map(|(peer, _)| peer.clone())
            .collect::<Vec<_>>();
        let validator_set_pops = committee_pops(&members);
        let mut request = request(NativeAmxPhase::Prepare);
        request.body = bind_body_committee(request.body, &validator_set, 3);
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        cache
            .authorize_request(&request)
            .expect("coordinator authorizes exact body");

        for (_, keypair) in members.iter().take(3) {
            let vote = signed_vote(&request.body, keypair);
            vote.validate_ingress(NativeAmxPhase::Prepare, Some(&vote.signer))
                .expect("remote vote authenticates");
            cache.insert_vote(vote).expect("remote vote converges");
        }
        let key = NativeAmxSessionKey::from_body(&request.body);
        let votes = cache.sorted_votes_for_body(key, &request.body);
        let qc = aggregate_votes_to_qc(request.body, validator_set, validator_set_pops, &votes, 3)
            .expect("three remote votes form the participant QC");
        assert_eq!(
            qc.signers_bitmap
                .iter()
                .map(|byte| byte.count_ones())
                .sum::<u32>(),
            3
        );
    }

    #[test]
    fn aggregate_votes_to_qc_rejects_bad_vote_sets() {
        let keypairs = [checked_bls_keypair(0xD1), checked_bls_keypair(0xD2)];
        let members = sorted_committee(&keypairs);
        let validator_set = members
            .iter()
            .map(|(peer, _)| peer.clone())
            .collect::<Vec<_>>();
        let validator_set_pops = committee_pops(&members);
        let body_quorum_one = bind_body_committee(body(NativeAmxPhase::Prepare), &validator_set, 1);
        let body_quorum_two = bind_body_committee(body(NativeAmxPhase::Prepare), &validator_set, 2);
        let vote_quorum_one = signed_vote(&body_quorum_one, &keypairs[0]);
        let vote_quorum_two = signed_vote(&body_quorum_two, &keypairs[0]);

        assert_eq!(
            aggregate_votes_to_qc(
                body_quorum_one,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[],
                1,
            ),
            Err(NativeAmxQcBuildError::EmptyVotes)
        );
        assert_eq!(
            aggregate_votes_to_qc(
                body_quorum_two,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[vote_quorum_two],
                2,
            ),
            Err(NativeAmxQcBuildError::QuorumNotMet)
        );
        assert_eq!(
            aggregate_votes_to_qc(
                body_quorum_one,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[vote_quorum_one.clone(), vote_quorum_one.clone()],
                1
            ),
            Err(NativeAmxQcBuildError::DuplicateSigner)
        );

        let outsider = checked_bls_keypair(0xD3);
        assert_eq!(
            aggregate_votes_to_qc(
                body_quorum_one,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[signed_vote(&body_quorum_one, &outsider)],
                1
            ),
            Err(NativeAmxQcBuildError::SignerNotInValidatorSet)
        );

        let ed25519_keypair = checked_random_ed25519_keypair();
        let ed25519_signer = PeerId::new(ed25519_keypair.public_key().clone());
        let ed25519_vote = NativeAmxVoteV1 {
            body: body_quorum_one,
            signer: ed25519_signer.clone(),
            bls_signature: Signature::try_new(
                ed25519_keypair.private_key(),
                &body_quorum_one.signature_preimage(),
            )
            .expect("checked Ed25519 fixture signature")
            .payload()
            .to_vec(),
        };
        assert_eq!(
            aggregate_votes_to_qc(
                bind_body_committee(body_quorum_one, &[ed25519_signer.clone()], 1),
                vec![ed25519_signer],
                vec![vec![0; NATIVE_AMX_BLS_PROOF_BYTES]],
                &[ed25519_vote],
                1,
            ),
            Err(NativeAmxQcBuildError::InvalidProofOfPossession)
        );

        let mut bad_signature_vote = vote_quorum_one.clone();
        bad_signature_vote.bls_signature = vec![0_u8; 96];
        assert_eq!(
            aggregate_votes_to_qc(
                body_quorum_one,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[bad_signature_vote],
                1
            ),
            Err(NativeAmxQcBuildError::InvalidSignature)
        );

        let mut wrong_body_vote = vote_quorum_one;
        wrong_body_vote.body.phase = NativeAmxPhase::Commit;
        assert_eq!(
            aggregate_votes_to_qc(
                body_quorum_one,
                validator_set,
                validator_set_pops,
                &[wrong_body_vote],
                1,
            ),
            Err(NativeAmxQcBuildError::BodyMismatch)
        );
    }
}
