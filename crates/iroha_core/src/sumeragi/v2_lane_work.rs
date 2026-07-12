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
    num::{NonZeroU64, NonZeroUsize},
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey, Signature};
use iroha_data_model::{
    block::{
        BlockHeader, CertifiedMergeLedgerReference, SignedBlock,
        consensus::{
            CertPhase, LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockProposalPayloadHintV1,
            LaneBlockProposalV1, LaneBlockQcV1, LaneSettlementReceipt, NativeAmxAttestationBodyV2,
            NativeAmxAttestationQcV2, NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt,
            SumeragiLanePayloadOwnership,
        },
        consensus_v2 as wire,
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    merge::{
        LaneDrainCertificateV1, MAX_MERGE_LEDGER_ENTRY_BYTES, MergeCommitteeSignature,
        MergeLedgerEntry, MergeQuorumCertificate, MergeSignerProof,
    },
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    peer::PeerId,
};
use norito::codec::Encode as _;
use thiserror::Error;

use super::{
    InboundBlockMessage, LaneRelayMessage,
    main_loop::lane_scheduler::{
        lane_block_redrive_leader, prepare_v2_lane_payload_plan, proposal_lookahead_enabled,
        v2_known_lane_tip_for_route,
    },
    message::BlockMessage,
    v2_candidate::{
        CandidateDescriptor, CandidateWorkProvider, CandidateWorkUnavailable, PreparedCandidateWork,
    },
};
use crate::{
    kura::Kura,
    lane_consensus::{
        CommittedLaneBlockSession, DurableLaneBlockNewViewCertificateV1,
        DurableLanePayloadAvailabilityCertificateV1, LaneBlockNewViewCacheOutcome,
        LaneBlockNewViewCertificateCache, LaneBlockNewViewCertificateV1, LaneBlockNewViewVoteCache,
        LaneBlockNewViewVoteV1, LaneBlockSessionCache, LaneBlockSessionInsertOutcome,
        LaneBlockVoteV1, LaneDrainVoteState, LaneDrainVoteV1, LaneExecutablePayloadHandoffCache,
        LaneExecutablePayloadHandoffV1, LaneExecutablePayloadV1, LanePayloadAvailabilityVoteV1,
        aggregate_lane_drain_votes, lane_drain_vote_recipients,
    },
    lane_drain::{LaneDrainSigningGuard, LaneDrainSigningGuardError},
    merge_sidecar::{
        CERTIFIED_MERGE_SIDECAR_VERSION_V1, CandidateChunkOutcome, CertifiedMergeSidecarMessage,
        ChunkIngestOutcome, MergeCandidateAdvertV1, MergeCandidateMessage, MergeCandidatePost,
        MergeCandidateTransport, MergeSidecarError, MergeSidecarPost, MergeSidecarTransport,
        MergeSigningContextV1, MergeSigningGuard, certified_merge_reference_digest,
        certified_merge_sidecar_holders, decode_certified_merge_sidecar,
        decode_merge_candidate_body,
    },
    native_amx::{
        NativeAmxAttestationRequestV2, NativeAmxCommitRequestV2, NativeAmxMessage,
        NativeAmxSessionCache, NativeAmxSessionError, NativeAmxSessionKey, NativeAmxSigningGuard,
        NativeAmxSigningGuardError, NativeAmxVoteV2, aggregate_votes_to_qc, validate_native_amx_qc,
    },
    queue::{RoutingDecision, RoutingPlan},
    state::State,
};

// Keep compact-QC preflight at least as strict as State's full-entry admission
// before allocating transport. These are first-release protocol caps, not
// runtime tuning knobs.
const MAX_FETCH_MERGE_SIGNER_PROOFS: usize = 4_096;
const MAX_FETCH_MERGE_VALIDATORS: usize = 4_096;
const MAX_FETCH_MERGE_QC_BYTES: usize = 4 * 1024 * 1024;
const MERGE_QC_PROOF_BYTES: usize = 96;
const MAX_AUTHENTICATED_MERGE_QCS: usize = 64;
const MERGE_QC_AUTH_CACHE_DOMAIN: &[u8] = b"iroha:sumeragi:v2:merge-qc-auth-cache:v1\0";

fn validate_merge_sidecar_reference_bounds(
    context: &wire::HeightContext,
    reference: &CertifiedMergeLedgerReference,
) -> Result<(), String> {
    let qc = &reference.merge_qc;
    if reference.version != CERTIFIED_MERGE_SIDECAR_VERSION_V1
        || reference.encoded_len == 0
        || reference.encoded_len > u64::try_from(MAX_MERGE_LEDGER_ENTRY_BYTES).unwrap_or(u64::MAX)
        || reference.epoch_id != qc.epoch_id
    {
        return Err("certified merge reference has invalid length or epoch metadata".to_owned());
    }
    let execution_fields = [
        reference.execution_batch_hash.is_some(),
        reference.entrypoint_count.is_some(),
        reference.entrypoint_merkle_root.is_some(),
        reference.result_merkle_root.is_some(),
        reference.base_state_height.is_some(),
        reference.base_state_hash.is_some(),
    ];
    if execution_fields.iter().any(|present| *present)
        && !execution_fields.iter().all(|present| *present)
    {
        return Err("certified merge reference has a partial execution projection".to_owned());
    }
    if qc.chain_id_digest != crate::merge::merge_chain_id_digest(&context.chain_id) {
        return Err("certified merge reference is bound to another chain".to_owned());
    }
    let expected_bitmap_len = qc.validator_set.len().div_ceil(8);
    if qc.validator_set.len() > MAX_FETCH_MERGE_VALIDATORS
        || qc.signer_proofs.len() > MAX_FETCH_MERGE_SIGNER_PROOFS
        || qc.signers_bitmap.len() != expected_bitmap_len
        || qc.aggregate_signature.len() != MERGE_QC_PROOF_BYTES
        || qc
            .signer_proofs
            .iter()
            .any(|proof| proof.proof_of_possession.len() != MERGE_QC_PROOF_BYTES)
    {
        return Err("certified merge QC exceeds a hard count or byte limit".to_owned());
    }
    if qc.validator_set.len() != context.roster.len()
        || qc
            .validator_set
            .iter()
            .zip(&context.roster)
            .any(|(actual, expected)| actual != &expected.validator)
    {
        return Err("certified merge QC does not use the frozen height roster".to_owned());
    }
    Ok(())
}

fn authenticate_bounded_merge_sidecar_holders(
    context: &wire::HeightContext,
    reference: &CertifiedMergeLedgerReference,
) -> Result<Vec<PeerId>, String> {
    let qc = &reference.merge_qc;
    let holders = certified_merge_sidecar_holders(reference).map_err(|error| error.to_string())?;
    let mut signer_indices = Vec::with_capacity(holders.len());
    for (byte_index, byte) in qc.signers_bitmap.iter().copied().enumerate() {
        for bit in 0_u8..8 {
            if byte & (1_u8 << bit) != 0 {
                signer_indices.push(byte_index * 8 + usize::from(bit));
            }
        }
    }
    let min_signers = usize::try_from(context.quorum.min_signers).unwrap_or(usize::MAX);
    let signed_power = signer_indices.iter().try_fold(0_u64, |total, index| {
        total.checked_add(context.roster.get(*index)?.power)
    });
    if signer_indices.len() < min_signers
        || signed_power
            .is_none_or(|power| u128::from(power) * 3 <= u128::from(context.quorum.total_power) * 2)
    {
        return Err("certified merge QC does not meet the frozen dual quorum".to_owned());
    }
    if qc.signer_proofs.len() != signer_indices.len() {
        return Err("certified merge QC signer proofs do not match its bitmap".to_owned());
    }
    let mut public_keys = Vec::with_capacity(signer_indices.len());
    let mut proof_refs = Vec::with_capacity(signer_indices.len());
    for (index, proof) in signer_indices.iter().copied().zip(&qc.signer_proofs) {
        let expected_signer = u32::try_from(index)
            .map_err(|_| "certified merge QC signer index exceeds u32".to_owned())?;
        if proof.signer != expected_signer {
            return Err("certified merge QC signer proofs are not canonical".to_owned());
        }
        let public_key = qc
            .validator_set
            .get(index)
            .expect("validated signer index is in the exact frozen roster")
            .public_key();
        iroha_crypto::bls_normal_pop_verify(public_key, &proof.proof_of_possession)
            .map_err(|_| "certified merge QC contains an invalid proof of possession".to_owned())?;
        public_keys.push(public_key);
        proof_refs.push(proof.proof_of_possession.as_slice());
    }
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        qc.message_digest.as_ref(),
        &qc.aggregate_signature,
        &public_keys,
        &proof_refs,
    )
    .map_err(|_| "certified merge QC aggregate signature is invalid".to_owned())?;
    Ok(holders)
}

fn bounded_merge_qc_authentication_key(
    reference: &CertifiedMergeLedgerReference,
) -> Result<Hash, String> {
    let bytes = reference.merge_qc.encode();
    if bytes.len() > MAX_FETCH_MERGE_QC_BYTES {
        return Err("certified merge QC exceeds a hard count or byte limit".to_owned());
    }
    Ok(Hash::new_from_chunks(&[
        MERGE_QC_AUTH_CACHE_DOMAIN,
        bytes.as_slice(),
    ]))
}

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
    /// Send one authoritative standalone lane message to one committee member.
    PostLaneBlock {
        /// Destination committee member.
        peer: PeerId,
        /// Lane-local message; global legacy variants are never emitted.
        message: BlockMessage,
    },
    /// Send one crash-safe lane-drain vote to a lane/global committee member.
    PostLaneDrainVote {
        /// Destination in the canonical union of lane and global committees.
        peer: PeerId,
        /// Exact intent/frontier-bound vote.
        vote: LaneDrainVoteV1,
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
    /// Send one authenticated certified merge-sidecar request or response.
    PostCertifiedMergeSidecar {
        /// Exact destination selected by the sidecar transport.
        peer: PeerId,
        /// Bounded request or fixed-boundary response chunk.
        message: CertifiedMergeSidecarMessage,
    },
    /// Send one bounded authenticated pre-certificate merge-candidate message.
    PostMergeCandidate {
        /// Exact round participant selected by the candidate transport.
        peer: PeerId,
        /// Leader advert, follower request, or fixed-boundary response chunk.
        message: MergeCandidateMessage,
    },
}

/// Result of registering an otherwise-valid body whose certified merge
/// sidecar is not yet present locally.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MergeSidecarDeferralDisposition {
    /// A bounded authenticated fetch is active (or already active).
    Fetching,
    /// The exact entry was already durable and validation can retry now.
    Available,
    /// A transient bounded transport cap prevented registration; the caller
    /// must retain and retry the exact deferral.
    RetryLater,
    /// The compact reference cannot describe this body's exact carrier.
    Rejected(String),
}

/// Terminal validation result for an exact fetched merge sidecar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RejectedMergeSidecar {
    entry_hash: HashOf<MergeLedgerEntry>,
    reason: String,
}

impl RejectedMergeSidecar {
    /// Hash shared by every deferred body waiting for this exact entry.
    pub(crate) const fn entry_hash(&self) -> HashOf<MergeLedgerEntry> {
        self.entry_hash
    }

    /// Deterministic full-entry validation diagnostic.
    pub(crate) fn reason(&self) -> &str {
        &self.reason
    }
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
#[derive(Debug, Error, PartialEq, Eq)]
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
    /// Canonical Kura recovery conflicts with quorum-certified in-memory evidence.
    #[error("canonical lane-session rollover conflict: {0}")]
    RolloverConflict(String),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct NativeVoteSlotKey {
    chain_id_hash: Hash,
    context_id: wire::HeightContextId,
    epoch: u64,
    authority_context_height: u64,
    participant_lane: LaneId,
    participant_dataspace: DataSpaceId,
    participant_lane_incarnation: Hash,
    participant_lane_block_height: u64,
    participant_lane_block_view: u64,
    phase: NativeAmxPhase,
    signer: HashOf<PeerId>,
}

impl NativeVoteSlotKey {
    fn from_body(body: &NativeAmxAttestationBodyV2, signer: &PeerId) -> Self {
        Self {
            chain_id_hash: body.chain_id_hash,
            context_id: body.round.context_id,
            epoch: body.epoch,
            authority_context_height: body.authority_context_height,
            participant_lane: body.participant_lane_id,
            participant_dataspace: body.participant_dataspace_id,
            participant_lane_incarnation: body.participant_lane_incarnation,
            participant_lane_block_height: body.participant_lane_block_height,
            participant_lane_block_view: body.participant_lane_block_view,
            phase: body.phase,
            signer: HashOf::new(signer),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeVoteSlotClaim {
    proposal_hash: Hash,
    settlement_commitment: Hash,
}

#[derive(Clone, Debug)]
struct NativeParticipantControl {
    proposal: LaneBlockProposalV1,
    settlement: LaneBlockCommitment,
}

type NativeParticipantControlMap = BTreeMap<(LaneId, DataSpaceId), NativeParticipantControl>;

impl NativeVoteSlotClaim {
    fn from_body(body: &NativeAmxAttestationBodyV2) -> Self {
        Self {
            proposal_hash: body.participant_proposal_hash,
            settlement_commitment: body.participant_settlement_commitment,
        }
    }
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
    stage: PendingMergeStage,
    signatures: BTreeMap<wire::ValidatorIndex, Vec<u8>>,
}

#[derive(Clone, Debug)]
enum PendingMergeStage {
    Collecting(crate::merge::MergeLedgerCandidate),
    Certified(MergeLedgerEntry),
}

/// Bounded lane-local evidence transferred across one global-height boundary.
///
/// Height-local Native AMX, merge, retransmission, and effect state is excluded.
/// The contained session cache is canonicalized and pruned both when it leaves
/// the old adapter and when it enters the successor so lifecycle
/// reconfiguration cannot retain stale incarnations. Ephemeral NewView votes
/// are deliberately reconstructed from the durable cursor chain instead.
#[derive(Debug)]
pub(crate) struct V2LaneWorkRollover {
    lane_sessions: LaneBlockSessionCache,
    lane_drain_votes: LaneDrainVoteState,
}

/// Authoritative bounded adapter retained for exactly one global height.
pub(crate) struct V2LaneWorkAdapter {
    context: wire::HeightContext,
    local_peer: PeerId,
    key_pair: KeyPair,
    voting_enabled: bool,
    state: Arc<State>,
    kura: Arc<Kura>,
    lane_drain_signing_guard: LaneDrainSigningGuard,
    merge_signing_guard: MergeSigningGuard,
    native_signing_guard: Option<NativeAmxSigningGuard>,
    signing_guard_failure: Option<String>,
    native_signing_capacity_exhausted: bool,
    limits: V2LaneWorkLimits,
    lane_sessions: LaneBlockSessionCache,
    lane_payload_handoffs: LaneExecutablePayloadHandoffCache,
    outbound_lane_handoffs: BTreeMap<Hash, LaneExecutablePayloadHandoffV1>,
    lane_new_view_votes: LaneBlockNewViewVoteCache,
    lane_new_view_certificates: LaneBlockNewViewCertificateCache,
    lane_new_view_waiting: BTreeSet<(Hash, u64)>,
    lane_drain_votes: LaneDrainVoteState,
    native_sessions: NativeAmxSessionCache,
    native_claims: BTreeMap<NativeVoteClaimKey, NativeAmxAttestationBodyV2>,
    native_claim_signatures: BTreeMap<NativeVoteClaimKey, Vec<u8>>,
    /// Authenticated signer decisions retained for the whole global height so
    /// global view changes cannot hide a participant lane-slot equivocation.
    native_slot_claims: BTreeMap<NativeVoteSlotKey, NativeVoteSlotClaim>,
    local_native_claims: BTreeMap<NativeVoteClaimKey, NativeAmxAttestationBodyV2>,
    native_requests: BTreeMap<NativeRequestKey, NativeAmxMessage>,
    authenticated_native_requests: BTreeMap<Hash, (PeerId, NativeAmxMessage)>,
    native_active_view: wire::View,
    planned_lane_proposals: BTreeMap<wire::ConsensusRound, Vec<LaneBlockProposalV1>>,
    pending_local_lane_proposals: BTreeMap<HashOf<BlockHeader>, Vec<LaneBlockProposalV1>>,
    pending_local_global_bodies: BTreeMap<HashOf<BlockHeader>, SignedBlock>,
    globally_locked_body_hash: Option<HashOf<BlockHeader>>,
    globally_locked_body: Option<SignedBlock>,
    retained_merge_carrier_state: Option<(
        wire::View,
        Option<wire::BlockSubject>,
        Option<wire::BlockSubject>,
    )>,
    #[cfg(test)]
    merge_retention_scans: usize,
    locally_bound_lane_proposals: BTreeSet<Hash>,
    pending_committed_lanes: VecDeque<CommittedLaneBlockSession>,
    admitted_relays: BTreeSet<(LaneId, DataSpaceId, u64, Hash)>,
    merge_entries: BTreeMap<MergeKey, PendingMerge>,
    merge_claims: BTreeMap<(u64, u64, wire::ValidatorIndex), Hash>,
    durably_staged_merge_entries: BTreeSet<MergeKey>,
    validated_merge_candidate_digests: BTreeMap<(u64, wire::View), (Hash, Hash)>,
    merge_candidates: MergeCandidateTransport,
    merge_sidecars: MergeSidecarTransport,
    authenticated_merge_qcs: BTreeSet<Hash>,
    authenticated_merge_qc_order: VecDeque<Hash>,
    #[cfg(test)]
    merge_qc_preflight_checks: usize,
    completed_merge_sidecars: BTreeSet<HashOf<MergeLedgerEntry>>,
    rejected_merge_sidecars: BTreeMap<HashOf<MergeLedgerEntry>, String>,
    sidecar_effects: VecDeque<V2LaneWorkEffect>,
    sidecar_effect_keys: BTreeSet<Hash>,
    effects: VecDeque<V2LaneWorkEffect>,
    effect_keys: BTreeSet<Hash>,
    drain_sidecar_next: bool,
    lane_fanout_cursor: usize,
    lane_artifact_cursor: usize,
    native_retransmit_cursor: usize,
    merge_candidate_fanout_cursor: usize,
}

impl V2LaneWorkAdapter {
    /// Open one adapter after verifying the frozen Nexus/AMX commitment and
    /// canonicalizing any predecessor lane-session rollover.
    ///
    /// # Errors
    ///
    /// Returns [`V2LaneWorkError`] for malformed context, local-key drift, or
    /// committed-state/context drift. `recovered_applied_height` is accepted
    /// only when it identifies this exact context and canonical post-apply tip.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        context: wire::HeightContext,
        local_peer: PeerId,
        key_pair: KeyPair,
        voting_enabled: bool,
        state: Arc<State>,
        kura: Arc<Kura>,
        limits: V2LaneWorkLimits,
        recovered_applied_height: Option<super::v2_recovery::PendingKuraApply>,
        rollover: Option<V2LaneWorkRollover>,
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
        if let Some(height) = usize::try_from(state_height)
            .ok()
            .and_then(NonZeroUsize::new)
            && let Some(block) = kura.get_block(height)
        {
            // Repair the post-commit Native AMX effect sidecar before any new
            // participant proposal can extend its lane-local frontier. This
            // is idempotent and runs only after durable v2 finality exists.
            kura.persist_native_amx_participant_application_receipts(&block)
                .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        }
        let chain_id = context.chain_id.clone().into_inner();
        let committed_merge_epoch = state
            .merge_ledger()
            .latest()
            .map_or(0, |entry| entry.epoch_id);
        let committed_carrier_height = u64::try_from(state.committed_height())
            .map_err(|_| V2LaneWorkError::StateHeightMismatch)?;
        let merge_signing_guard = MergeSigningGuard::open_with_committed_frontier(
            &kura.store_root(),
            committed_merge_epoch,
            committed_carrier_height,
        )
        .map_err(|error| V2LaneWorkError::SigningGuard(error.to_string()))?;
        let active_lane_incarnations = state
            .lane_incarnations_snapshot()
            .into_iter()
            .collect::<BTreeSet<_>>();
        let lane_drain_signing_guard =
            LaneDrainSigningGuard::open(&kura.store_root(), &active_lane_incarnations)
                .map_err(|error| V2LaneWorkError::SigningGuard(error.to_string()))?;
        let native_signing_guard = if native_signing_guard_required(voting_enabled, &local_peer) {
            let native_signing_capacity = limits.native_signing_capacity()?;
            Some(
                NativeAmxSigningGuard::open(
                    &kura.store_root(),
                    context.height,
                    context.id(),
                    context.epoch,
                    Hash::new(chain_id.as_bytes()),
                    local_peer.clone(),
                    native_signing_capacity,
                )
                .map_err(|error| V2LaneWorkError::SigningGuard(error.to_string()))?,
            )
        } else {
            None
        };
        let mut adapter = Self {
            context,
            local_peer,
            key_pair,
            voting_enabled,
            state,
            kura,
            lane_drain_signing_guard,
            merge_signing_guard,
            native_signing_guard,
            signing_guard_failure: None,
            native_signing_capacity_exhausted: false,
            limits,
            lane_sessions: LaneBlockSessionCache::new(limits.session_capacity.get()),
            lane_payload_handoffs: LaneExecutablePayloadHandoffCache::new(
                limits.session_capacity.get(),
            ),
            outbound_lane_handoffs: BTreeMap::new(),
            lane_new_view_votes: LaneBlockNewViewVoteCache::new(limits.session_capacity.get()),
            lane_new_view_certificates: LaneBlockNewViewCertificateCache::new(
                limits.session_capacity.get(),
            ),
            lane_new_view_waiting: BTreeSet::new(),
            lane_drain_votes: LaneDrainVoteState::new(),
            native_sessions: NativeAmxSessionCache::with_limits(
                limits.session_capacity,
                limits.body_buckets_per_session,
            ),
            native_claims: BTreeMap::new(),
            native_claim_signatures: BTreeMap::new(),
            native_slot_claims: BTreeMap::new(),
            local_native_claims: BTreeMap::new(),
            native_requests: BTreeMap::new(),
            authenticated_native_requests: BTreeMap::new(),
            native_active_view: 0,
            planned_lane_proposals: BTreeMap::new(),
            pending_local_lane_proposals: BTreeMap::new(),
            pending_local_global_bodies: BTreeMap::new(),
            globally_locked_body_hash: None,
            globally_locked_body: None,
            retained_merge_carrier_state: None,
            #[cfg(test)]
            merge_retention_scans: 0,
            locally_bound_lane_proposals: BTreeSet::new(),
            pending_committed_lanes: VecDeque::new(),
            admitted_relays: BTreeSet::new(),
            merge_entries: BTreeMap::new(),
            merge_claims: BTreeMap::new(),
            durably_staged_merge_entries: BTreeSet::new(),
            validated_merge_candidate_digests: BTreeMap::new(),
            merge_candidates: MergeCandidateTransport::new(),
            merge_sidecars: MergeSidecarTransport::new(),
            authenticated_merge_qcs: BTreeSet::new(),
            authenticated_merge_qc_order: VecDeque::new(),
            #[cfg(test)]
            merge_qc_preflight_checks: 0,
            completed_merge_sidecars: BTreeSet::new(),
            rejected_merge_sidecars: BTreeMap::new(),
            sidecar_effects: VecDeque::new(),
            sidecar_effect_keys: BTreeSet::new(),
            effects: VecDeque::new(),
            effect_keys: BTreeSet::new(),
            drain_sidecar_next: true,
            lane_fanout_cursor: 0,
            lane_artifact_cursor: 0,
            native_retransmit_cursor: 0,
            merge_candidate_fanout_cursor: 0,
        };
        let finalized_cleanup_height = if is_post_apply {
            adapter.context.height
        } else {
            adapter.context.height.saturating_sub(1)
        };
        adapter
            .kura
            .prune_finalized_pending_certified_merge_entries(finalized_cleanup_height)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        adapter.repair_globally_applied_lane_receipts()?;
        if let Some(rollover) = rollover {
            adapter.install_rollover(rollover)?;
        }
        super::status::clear_lane_payload_ownerships();
        super::status::set_lane_payload_ownerships(adapter.canonical_ownership_status());
        adapter.hydrate_canonical_lane_artifacts()?;
        adapter.recover_autonomous_payloads_from_committed_anchors()?;
        adapter.drive_lane_sessions();
        adapter.publish_operator_status();
        Ok(adapter)
    }

    fn canonical_rollover_proposal(
        kura: &Kura,
        lane_id: LaneId,
        lane_block_height: u64,
    ) -> Option<LaneBlockProposalV1> {
        let artifact = kura.read_lane_block_artifact(lane_id, lane_block_height)?;
        proposal_from_ownership(&artifact.ownership, artifact.proposal_block_hash)
    }

    fn rollover_slot_is_active(
        state: &State,
        context_height: u64,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
    ) -> bool {
        state.lane_route_and_incarnation_active_at_height(
            lane_id,
            dataspace_id,
            lane_incarnation,
            context_height,
        )
    }

    fn rollover_slot_is_unfinalized(
        state: &State,
        kura: &Kura,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        lane_block_height: u64,
    ) -> bool {
        let Some(artifact) = kura.read_lane_block_artifact(lane_id, lane_block_height) else {
            return false;
        };
        let ownership = &artifact.ownership;
        ownership.lane_id == lane_id
            && ownership.dataspace_id == dataspace_id
            && ownership.lane_incarnation == lane_incarnation
            && ownership.lane_block_height == lane_block_height
            && !state.lane_block_artifact_is_applied_or_snapshot_anchored_cached(&artifact)
    }

    fn sanitize_rollover_cache(
        state: &State,
        kura: &Kura,
        context_height: u64,
        capacity: usize,
        cache: &mut LaneBlockSessionCache,
    ) -> Result<(), V2LaneWorkError> {
        cache
            .retain_canonical_rollover_evidence(
                capacity,
                |lane_id, lane_block_height| {
                    Self::canonical_rollover_proposal(kura, lane_id, lane_block_height)
                },
                |lane_id, dataspace_id, lane_incarnation, _lane_block_height| {
                    Self::rollover_slot_is_active(
                        state,
                        context_height,
                        lane_id,
                        dataspace_id,
                        lane_incarnation,
                    )
                },
                |lane_id, dataspace_id, lane_incarnation, lane_block_height| {
                    Self::rollover_slot_is_unfinalized(
                        state,
                        kura,
                        lane_id,
                        dataspace_id,
                        lane_incarnation,
                        lane_block_height,
                    )
                },
            )
            .map(|_| ())
            .map_err(|error| V2LaneWorkError::RolloverConflict(error.to_string()))
    }

    fn install_rollover(
        &mut self,
        mut rollover: V2LaneWorkRollover,
    ) -> Result<(), V2LaneWorkError> {
        Self::sanitize_rollover_cache(
            self.state.as_ref(),
            self.kura.as_ref(),
            self.context.height,
            self.limits.session_capacity.get(),
            &mut rollover.lane_sessions,
        )?;
        self.lane_sessions = rollover.lane_sessions;
        self.lane_drain_votes = rollover.lane_drain_votes;
        Ok(())
    }

    /// Extract exact active, unfinalized lane evidence for the successor height.
    ///
    /// Callers persist completed anchored sessions first. Any remaining
    /// unanchored completed queue entries are losing global-proposal evidence and
    /// deliberately do not cross the boundary.
    ///
    /// # Errors
    ///
    /// Returns [`V2LaneWorkError::RolloverConflict`] if quorum-certified cache
    /// evidence conflicts with the exact canonical Kura proposal.
    pub(crate) fn take_rollover(&mut self) -> Result<V2LaneWorkRollover, V2LaneWorkError> {
        let mut lane_sessions = std::mem::replace(
            &mut self.lane_sessions,
            LaneBlockSessionCache::new(self.limits.session_capacity.get()),
        );
        if let Err(error) = Self::sanitize_rollover_cache(
            self.state.as_ref(),
            self.kura.as_ref(),
            self.context.height,
            self.limits.session_capacity.get(),
            &mut lane_sessions,
        ) {
            self.lane_sessions = lane_sessions;
            return Err(error);
        }
        self.lane_payload_handoffs =
            LaneExecutablePayloadHandoffCache::new(self.limits.session_capacity.get());
        self.outbound_lane_handoffs.clear();
        self.lane_new_view_votes =
            LaneBlockNewViewVoteCache::new(self.limits.session_capacity.get());
        self.lane_new_view_certificates =
            LaneBlockNewViewCertificateCache::new(self.limits.session_capacity.get());
        self.lane_new_view_waiting.clear();
        Ok(V2LaneWorkRollover {
            lane_sessions,
            lane_drain_votes: std::mem::replace(
                &mut self.lane_drain_votes,
                LaneDrainVoteState::new(),
            ),
        })
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

    fn recover_autonomous_payloads_from_committed_anchors(
        &mut self,
    ) -> Result<usize, V2LaneWorkError> {
        let nexus = self.state.nexus_snapshot();
        if !nexus.enabled {
            return Ok(0);
        }
        let mut recovered = self.hydrate_durable_autonomous_lane_sessions()?;
        let committed_height = u64::try_from(self.state.committed_height())
            .map_err(|_| V2LaneWorkError::StateHeightMismatch)?;
        let remaining = self.limits.session_capacity.get().saturating_sub(recovered);
        if remaining == 0 {
            return Ok(recovered);
        }
        let mut candidates = Vec::with_capacity(remaining);
        // Apply every actionability guard before consuming the global bound.
        // Otherwise stale routes or non-committee history from low-sorted lanes
        // can starve an exact recoverable payload indefinitely.
        for artifact in self.kura.lane_block_artifacts_snapshot() {
            let ownership = &artifact.ownership;
            if ownership.proposal_height > committed_height
                || !proposal_lookahead_enabled(&nexus, ownership.proposal_height)
                || self
                    .state
                    .lane_block_artifact_is_applied_or_snapshot_anchored_cached(&artifact)
                || !ownership
                    .lane_block_descriptor_validator_set
                    .contains(&self.local_peer)
                || !self.lane_route_active(
                    ownership.lane_id,
                    ownership.dataspace_id,
                    ownership.lane_incarnation,
                    ownership.proposal_height,
                )
                || self
                    .expected_lane_validators(ownership.lane_id, ownership.proposal_height)
                    .as_deref()
                    != Some(ownership.lane_block_descriptor_validator_set.as_slice())
            {
                continue;
            }
            let Some(proposal) = proposal_from_ownership(ownership, artifact.proposal_block_hash)
            else {
                return Err(V2LaneWorkError::Persistence(
                    "committed lane ownership cannot reconstruct its exact proposal".to_owned(),
                ));
            };
            let epoch = self.epoch_for_proposal_height(ownership.proposal_height);
            if self
                .kura
                .read_autonomous_lane_block_artifact(
                    ownership.lane_id,
                    ownership.lane_block_height,
                    self.chain_id_hash(),
                    epoch,
                )
                .is_some()
            {
                continue;
            }
            let hint = proposal.payload_block_hint.ok_or_else(|| {
                V2LaneWorkError::Persistence(
                    "committed multilane ownership omitted its global proposal hint".to_owned(),
                )
            })?;
            let block = self.observed_global_anchor(hint).ok_or_else(|| {
                V2LaneWorkError::Persistence(
                    "committed multilane ownership has no canonical global block".to_owned(),
                )
            })?;
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
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    V2LaneWorkError::Persistence(
                        "committed lane ownership indexes outside its canonical global body"
                            .to_owned(),
                    )
                })?;
            if !Self::global_block_matches_payload(&block, hint, &proposal, &selected) {
                return Err(V2LaneWorkError::Persistence(
                    "committed lane payload does not match its protected global block".to_owned(),
                ));
            }
            candidates.push((proposal, selected, epoch));
            if candidates.len() == remaining {
                break;
            }
        }
        for (proposal, selected, epoch) in candidates {
            let payload = LaneExecutablePayloadV1::new_signed(
                self.chain_id_hash(),
                epoch,
                proposal,
                selected,
                self.local_peer.clone(),
                self.key_pair.private_key(),
            )
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
            let local_peer = self.local_peer.clone();
            if self.accept_lane_executable_payload(payload, Some(&local_peer), 0)
                == V2LaneIngressOutcome::Rejected
            {
                return Err(V2LaneWorkError::Persistence(
                    "reconstructed committed lane payload failed deterministic admission"
                        .to_owned(),
                ));
            }
            recovered = recovered.saturating_add(1);
        }
        Ok(recovered)
    }

    fn hydrate_durable_autonomous_lane_sessions(&mut self) -> Result<usize, V2LaneWorkError> {
        let capacity = self.limits.session_capacity.get();
        // Kura bounds its scan per lane. Filter the resulting cross-lane set
        // before applying our global session bound so inert history cannot
        // crowd out work this validator can actually vote for.
        let mut recovered = self
            .kura
            .latest_autonomous_lane_block_artifacts_snapshot(
                self.chain_id_hash(),
                capacity,
                |proposal_height| self.epoch_for_proposal_height(proposal_height),
            )
            .into_iter()
            .filter(|(artifact, _cursor)| {
                let payload = &artifact.executable_payload;
                let proposal = &payload.origin_proposal;
                let descriptor = &proposal.descriptor;
                let Some(hint) = proposal.payload_block_hint else {
                    return false;
                };
                let certified = self
                    .kura
                    .read_certified_lane_block_artifact(
                        descriptor.lane_id,
                        descriptor.lane_block_height,
                    )
                    .is_some_and(|certified| certified.proposal == *proposal);
                descriptor.proposal_height <= self.context.height
                    && !self.autonomous_origin_is_applied_or_snapshot_anchored(proposal)
                    && descriptor.validator_set.contains(&self.local_peer)
                    && self.lane_route_active(
                        descriptor.lane_id,
                        descriptor.dataspace_id,
                        descriptor.lane_incarnation,
                        descriptor.proposal_height,
                    )
                    && self
                        .expected_lane_validators(descriptor.lane_id, descriptor.proposal_height)
                        .as_deref()
                        == Some(descriptor.validator_set.as_slice())
                    && self.global_anchor_matches_payload(hint, proposal, &payload.entrypoints)
                    && (certified
                        || self.lane_executable_payload_passes_stateful_preflight(payload))
            })
            .collect::<Vec<_>>();
        if recovered.len() > capacity {
            let excess = recovered.len().saturating_sub(capacity);
            recovered.drain(..excess);
        }

        let mut hydrated = 0_usize;
        for (artifact, _cursor) in recovered {
            let payload = artifact.executable_payload.clone();
            let proposal = payload.origin_proposal.clone();
            let descriptor = &proposal.descriptor;
            let Some(hint) = proposal.payload_block_hint else {
                continue;
            };
            debug_assert!(descriptor.validator_set.contains(&self.local_peer));
            debug_assert!(self.global_anchor_matches_payload(
                hint,
                &proposal,
                &payload.entrypoints
            ));
            let Some(block) = self.observed_global_anchor(hint) else {
                continue;
            };
            let mut next_sessions = self.lane_sessions.clone();
            if !self.hydrate_exact_durable_autonomous_origin_for_protected_block(
                &proposal,
                &block,
                &artifact,
                &mut next_sessions,
            )? {
                continue;
            }
            self.lane_sessions = next_sessions;

            if let Some(durable) = artifact.new_view_certificates.last().or_else(|| {
                artifact
                    .view_checkpoint
                    .as_ref()
                    .map(|checkpoint| &checkpoint.certificate)
            }) {
                self.lane_new_view_certificates
                    .insert(durable.certificate.clone(), &durable.signer_pops)
                    .map_err(|error| V2LaneWorkError::RolloverConflict(error.to_string()))?;
            }
            hydrated = hydrated.saturating_add(1);
        }
        Ok(hydrated)
    }

    fn hydrate_exact_durable_autonomous_origin_for_protected_block(
        &self,
        proposal: &LaneBlockProposalV1,
        block: &SignedBlock,
        artifact: &crate::kura::AutonomousLaneBlockArtifact,
        sessions: &mut LaneBlockSessionCache,
    ) -> Result<bool, V2LaneWorkError> {
        let descriptor = &proposal.descriptor;
        let epoch = self.epoch_for_proposal_height(descriptor.proposal_height);
        let Some((payload, durable_origin)) = self.autonomous_certification_payload(
            descriptor.lane_id,
            descriptor.lane_block_height,
            epoch,
        ) else {
            return Err(V2LaneWorkError::Persistence(
                "durable autonomous payload failed origin/cursor validation".to_owned(),
            ));
        };
        let Some(hint) = proposal.payload_block_hint else {
            return Err(V2LaneWorkError::Persistence(
                "durable V2 autonomous origin omitted its global proposal hint".to_owned(),
            ));
        };
        if artifact.executable_payload != payload
            || durable_origin != *proposal
            || payload.origin_proposal != *proposal
            || !descriptor.validator_set.contains(&self.local_peer)
            || !self.lane_route_active(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.proposal_height,
            )
            || self
                .expected_lane_validators(descriptor.lane_id, descriptor.proposal_height)
                .as_deref()
                != Some(descriptor.validator_set.as_slice())
            || !Self::global_block_matches_payload(block, hint, proposal, &payload.entrypoints)
            || self.autonomous_origin_is_applied_or_snapshot_anchored(proposal)
        {
            return Ok(false);
        }

        let certified = self
            .kura
            .read_certified_lane_block_artifact(descriptor.lane_id, descriptor.lane_block_height)
            .filter(|certified| certified.proposal == *proposal);
        if certified.is_none() && !self.lane_executable_payload_passes_stateful_preflight(&payload)
        {
            return Ok(false);
        }

        sessions
            .insert_recovered_proposal_replacing_uncommitted_conflict(proposal.clone())
            .map_err(|error| V2LaneWorkError::RolloverConflict(error.to_string()))?;
        let availability_body = crate::lane_consensus::lane_payload_availability_body(
            &payload,
            proposal,
            self.chain_id_hash(),
            epoch,
        )
        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        sessions
            .authorize_payload_availability(proposal, availability_body)
            .map_err(|error| V2LaneWorkError::RolloverConflict(error.to_string()))?;

        if let Some(availability) = artifact.availability_certificate.as_ref() {
            let qc = availability.certificate.clone();
            if qc.body != proposal.vote_body(CertPhase::Prepare) {
                return Err(V2LaneWorkError::Persistence(
                    "durable autonomous availability certificate changed its origin subject"
                        .to_owned(),
                ));
            }
            let pops = qc
                .payload_availability_qc
                .as_ref()
                .map(|availability_qc| {
                    availability_qc
                        .validator_set
                        .iter()
                        .zip(&availability_qc.validator_set_pops)
                        .map(|(peer, pop)| (peer.public_key().clone(), pop.clone()))
                        .collect::<BTreeMap<_, _>>()
                })
                .ok_or_else(|| {
                    V2LaneWorkError::Persistence(
                        "durable autonomous availability certificate omitted READY proof"
                            .to_owned(),
                    )
                })?;
            sessions
                .insert_qc_with_pops(qc, &pops)
                .map_err(|error| V2LaneWorkError::RolloverConflict(error.to_string()))?;
        }

        if let Some(certified) = certified {
            let crate::kura::CertifiedLaneBlockArtifact {
                prepare_qc,
                commit_qc,
                signer_pops,
                ..
            } = certified;
            sessions
                .insert_qc_with_pops(prepare_qc, &signer_pops)
                .map_err(|error| V2LaneWorkError::RolloverConflict(error.to_string()))?;
            sessions
                .insert_qc_with_pops(commit_qc, &signer_pops)
                .map_err(|error| V2LaneWorkError::RolloverConflict(error.to_string()))?;
        }
        Ok(true)
    }

    /// Bind locally planned lane proposals to the exact global block body.
    pub(crate) fn bind_local_candidate(
        &mut self,
        round: wire::ConsensusRound,
        block: &SignedBlock,
    ) -> V2LaneIngressOutcome {
        if !self.round_is_current(round) {
            return V2LaneIngressOutcome::Rejected;
        }
        let block_hash = block.hash();
        if block.header().height().get() != round.height
            || block.header().view_change_index() != round.view
        {
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
            match next_sessions
                .insert_replanned_proposal_replacing_uncommitted_conflict(proposal.clone())
            {
                Ok(LaneBlockSessionInsertOutcome::Inserted) => inserted = true,
                Ok(LaneBlockSessionInsertOutcome::Duplicate) => {}
                Err(_) => return V2LaneIngressOutcome::Rejected,
            }
        }
        self.lane_sessions = next_sessions;
        self.locally_bound_lane_proposals.clear();
        self.pending_local_lane_proposals.clear();
        self.pending_local_lane_proposals
            .insert(block_hash, proposals.clone());
        self.pending_local_global_bodies.clear();
        self.pending_local_global_bodies
            .insert(block_hash, block.clone());
        self.outbound_lane_handoffs.clear();
        if !self.emit_local_candidate_lane_payloads(block, &proposals, round.view) {
            return V2LaneIngressOutcome::Rejected;
        }
        self.publish_lane_session_status();
        if inserted {
            V2LaneIngressOutcome::Inserted
        } else {
            V2LaneIngressOutcome::Duplicate
        }
    }

    fn emit_local_candidate_lane_payloads(
        &mut self,
        block: &SignedBlock,
        proposals: &[LaneBlockProposalV1],
        active_view: wire::View,
    ) -> bool {
        let local_is_global_proposer = usize::try_from(self.context.leader(active_view))
            .ok()
            .and_then(|index| self.context.roster.get(index))
            .is_some_and(|entry| entry.validator == self.local_peer);
        if !local_is_global_proposer {
            return false;
        }
        let entrypoints = block.external_entrypoints_cloned().collect::<Vec<_>>();
        for proposal in proposals {
            let selected = proposal
                .descriptor
                .accepted_candidate_indices
                .iter()
                .copied()
                .map(|raw_index| {
                    usize::try_from(raw_index)
                        .ok()
                        .and_then(|index| entrypoints.get(index).cloned())
                })
                .collect::<Option<Vec<_>>>();
            let Some(selected) = selected else {
                return false;
            };
            let Ok(handoff) = LaneExecutablePayloadHandoffV1::new_signed(
                self.chain_id_hash(),
                self.context.epoch,
                proposal.clone(),
                selected,
                self.local_peer.clone(),
                self.key_pair.private_key(),
            ) else {
                return false;
            };
            if self.outbound_lane_handoffs.len() >= self.limits.session_capacity.get()
                && !self
                    .outbound_lane_handoffs
                    .contains_key(&proposal.proposal_hash)
            {
                return false;
            }
            self.outbound_lane_handoffs
                .insert(proposal.proposal_hash, handoff);
        }
        true
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
        self.globally_locked_body = None;
        self.lane_payload_handoffs.retain_global_anchor(block_hash);
        self.outbound_lane_handoffs.retain(|_, handoff| {
            handoff
                .origin_proposal
                .payload_block_hint
                .is_some_and(|hint| hint.proposal_block_hash == block_hash)
        });
        self.locally_bound_lane_proposals.clear();
        self.clear_merge_candidate_round_state();
        true
    }

    fn clear_merge_candidate_round_state(&mut self) {
        self.merge_entries.clear();
        self.merge_claims.clear();
        self.durably_staged_merge_entries.clear();
        self.validated_merge_candidate_digests.clear();
        self.merge_candidates = MergeCandidateTransport::new();
        self.merge_candidate_fanout_cursor = 0;
        self.effects
            .retain(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)));
        self.effect_keys = self.effects.iter().map(lane_work_effect_key).collect();
        self.sidecar_effects
            .retain(|effect| !matches!(effect, V2LaneWorkEffect::PostMergeCandidate { .. }));
        self.sidecar_effect_keys = self
            .sidecar_effects
            .iter()
            .map(lane_work_effect_key)
            .collect();
    }

    /// Prune losing pending merge entries after a certified view transition
    /// only when neither a safety lock nor a durable Decision protects an
    /// earlier immutable body.
    pub(crate) fn retain_merge_sidecars_for_global_view(
        &mut self,
        view: wire::View,
        locked_subject: Option<wire::BlockSubject>,
        decided_subject: Option<wire::BlockSubject>,
    ) -> Result<(), V2LaneWorkError> {
        let carrier_state = (view, locked_subject, decided_subject);
        if self.retained_merge_carrier_state == Some(carrier_state) {
            return Ok(());
        }
        self.clear_merge_candidate_round_state();
        if locked_subject.is_some() || decided_subject.is_some() {
            self.retained_merge_carrier_state = Some(carrier_state);
            return Ok(());
        }
        #[cfg(test)]
        {
            self.merge_retention_scans = self.merge_retention_scans.saturating_add(1);
        }
        let Some(parent) = self
            .context
            .parent_commit_qc
            .as_ref()
            .map(|certificate| certificate.subject.block_hash)
        else {
            self.kura
                .retain_pending_certified_merge_entry_for_locked_carrier(self.context.height, None)
                .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
            self.retained_merge_carrier_state = Some(carrier_state);
            self.refresh_merge_candidates(view);
            return Ok(());
        };
        self.kura
            .prune_pending_certified_merge_entries_not_bound_to(self.context.height, parent, view)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        self.retained_merge_carrier_state = Some(carrier_state);
        self.refresh_merge_candidates(view);
        Ok(())
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
                || descriptor.lane_block_view != 0
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
                || self.expected_lane_author(&proposal).is_none()
            {
                return V2LaneIngressOutcome::Rejected;
            }
            proposals.push(proposal);
        }

        let local = self.pending_local_lane_proposals.get(&block_hash).cloned();
        if local.as_ref().is_some_and(|planned| planned != &proposals) {
            return V2LaneIngressOutcome::Rejected;
        }
        let mut next_sessions = self.lane_sessions.clone();
        let mut inserted = false;
        for proposal in &proposals {
            match next_sessions
                .insert_recovered_proposal_replacing_uncommitted_conflict(proposal.clone())
            {
                Ok(LaneBlockSessionInsertOutcome::Inserted) => inserted = true,
                Ok(LaneBlockSessionInsertOutcome::Duplicate) => {}
                Err(_) => return V2LaneIngressOutcome::Rejected,
            }
            let descriptor = &proposal.descriptor;
            let epoch = self.epoch_for_proposal_height(descriptor.proposal_height);
            // The payload write can precede the global lock/body handoff. On
            // restart it remains deliberately dormant until this exact body is
            // protected; authorize it in the cloned cache before publishing
            // any in-memory binding or releasing the vote driver.
            if let Some(artifact) = self.kura.read_autonomous_lane_block_artifact(
                descriptor.lane_id,
                descriptor.lane_block_height,
                self.chain_id_hash(),
                epoch,
            ) && !self
                .hydrate_exact_durable_autonomous_origin_for_protected_block(
                    proposal,
                    block,
                    &artifact,
                    &mut next_sessions,
                )
                .is_ok_and(|hydrated| hydrated)
            {
                return V2LaneIngressOutcome::Rejected;
            }
        }
        // Do not delete any losing durable sidecar until every in-memory
        // locked-body check and session insertion has succeeded. Once this
        // exact retention succeeds, the remaining operations are infallible.
        if self
            .kura
            .retain_pending_certified_merge_entry_for_locked_carrier(
                self.context.height,
                bundle.and_then(|bundle| bundle.merge_entry.as_ref()),
            )
            .is_err()
        {
            return V2LaneIngressOutcome::Rejected;
        }
        self.pending_local_lane_proposals.remove(&block_hash);
        self.pending_local_lane_proposals.clear();
        self.pending_local_global_bodies.remove(&block_hash);
        self.pending_local_global_bodies.clear();
        self.globally_locked_body = Some(block.clone());
        self.lane_sessions = next_sessions;
        self.locally_bound_lane_proposals = proposals
            .iter()
            .map(|proposal| proposal.proposal_hash)
            .collect();
        let _ = self.process_deferred_lane_payload_handoffs(global_view);
        for proposal in local.into_iter().flatten() {
            if let Some(handoff) = self
                .outbound_lane_handoffs
                .get(&proposal.proposal_hash)
                .cloned()
            {
                if proposal.descriptor.validator_set.contains(&self.local_peer) {
                    let proposer = handoff.proposer.clone();
                    if self.accept_lane_executable_payload_handoff(
                        handoff.clone(),
                        Some(&proposer),
                        global_view,
                    ) == V2LaneIngressOutcome::Rejected
                    {
                        return V2LaneIngressOutcome::Rejected;
                    }
                }
                self.fanout_lane_message(
                    BlockMessage::LaneExecutablePayloadHandoff(handoff),
                    &proposal.descriptor.validator_set,
                );
            }
            if let Some(payload) = self.autonomous_payload_for_proposal(&proposal) {
                self.fanout_lane_message(
                    BlockMessage::LaneExecutablePayload(payload),
                    &proposal.descriptor.validator_set,
                );
            }
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

    /// Persist completed lane sessions and their canonical application evidence.
    ///
    /// A session leaves the retry queue only after its exact Kura anchor is
    /// still canonical, both QCs are durable, and the canonical transaction
    /// results have produced a receipt that verifies against the stored block.
    ///
    /// # Errors
    ///
    /// Returns [`V2LaneWorkError::Persistence`] if an anchored certificate or
    /// its canonical globally-applied receipt cannot be written and verified
    /// durably.
    pub(crate) fn persist_anchored_sessions(&mut self) -> Result<usize, V2LaneWorkError> {
        self.collect_committed_lane_sessions();
        let mut sessions = std::mem::take(&mut self.pending_committed_lanes)
            .into_iter()
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
        let mut sessions = sessions.into_iter();
        let mut retained = VecDeque::new();
        let mut persisted = 0usize;
        while let Some(session) = sessions.next() {
            if !self.session_has_canonical_anchor(&session) {
                retained.push_back(session);
                continue;
            }
            if !self.proposal_anchor_is_committed_in_state(&session.proposal) {
                let committed_height =
                    u64::try_from(self.state.committed_height()).unwrap_or(u64::MAX);
                if session.proposal.descriptor.proposal_height > committed_height {
                    // The exact canonical block is durable in Kura but its WSV
                    // transaction has not committed yet. This is an ordinary
                    // asynchronous apply boundary, not a certificate conflict.
                    retained.push_back(session);
                    continue;
                }
                retained.push_back(session);
                retained.extend(sessions);
                self.pending_committed_lanes = retained;
                self.publish_operator_status();
                return Err(V2LaneWorkError::Persistence(
                    "lane certificate anchor conflicts with committed State".to_owned(),
                ));
            }
            let persisted_result = (|| {
                let pops = self.pops_for_lane_session(&session);
                self.kura
                    .persist_committed_lane_block_session(&session, &pops)
                    .map_err(|error| {
                        V2LaneWorkError::Persistence(format!(
                            "certified lane-block sidecar: {error}"
                        ))
                    })?;
                if self
                    .state
                    .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(&session)
                {
                    return Ok(());
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
                    .map_err(|error| {
                        V2LaneWorkError::Persistence(format!(
                            "canonical lane-block application receipt: {error}"
                        ))
                    })?;
                if !receipt_persisted
                    || !self
                        .kura
                        .lane_block_application_receipt_available(&session.proposal)
                {
                    return Err(V2LaneWorkError::Persistence(
                        "globally applied lane block has no recoverable canonical results"
                            .to_owned(),
                    ));
                }
                Ok(())
            })();
            if let Err(error) = persisted_result {
                // A certified sidecar may already be durable while its receipt
                // write failed. Preserve this exact session and every later
                // item so the next runner pass retries the idempotent boundary
                // instead of silently losing application evidence.
                retained.push_back(session);
                retained.extend(sessions);
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

    /// Retire losing certified merge sidecars once another carrier is durably
    /// finalized at this height.
    pub(crate) fn prune_finalized_merge_sidecars(&mut self) -> Result<(), V2LaneWorkError> {
        self.merge_sidecars
            .retain_pending_blocks(&BTreeSet::new(), self.context.height);
        self.kura
            .prune_finalized_pending_certified_merge_entries(self.context.height)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        Ok(())
    }

    fn proposal_anchor_is_committed_in_state(&self, proposal: &LaneBlockProposalV1) -> bool {
        let Some(hint) = proposal.payload_block_hint else {
            return false;
        };
        hint.proposal_height == proposal.descriptor.proposal_height
            && self
                .state
                .committed_block_hash_at_height(hint.proposal_height)
                == Some(hint.proposal_block_hash)
            && self.observed_global_anchor(hint).is_some()
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
            BlockMessage::LaneExecutablePayload(payload) => {
                self.accept_lane_executable_payload(payload, sender.as_ref(), active_view)
            }
            BlockMessage::LaneExecutablePayloadHandoff(handoff) => {
                self.accept_lane_executable_payload_handoff(handoff, sender.as_ref(), active_view)
            }
            BlockMessage::LaneBlockNewViewVote(vote) => {
                self.accept_lane_new_view_vote(vote, sender.as_ref(), active_view)
            }
            BlockMessage::LaneBlockNewViewCertificate(certificate) => {
                self.accept_lane_new_view_certificate(certificate, sender.as_ref(), active_view)
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

    fn accept_lane_executable_payload(
        &mut self,
        payload: LaneExecutablePayloadV1,
        sender: Option<&PeerId>,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let descriptor = &payload.origin_proposal.descriptor;
        let validators = descriptor.validator_set.clone();
        let proposal_epoch = self.epoch_for_proposal_height(descriptor.proposal_height);
        let expected_validators =
            self.expected_lane_validators(descriptor.lane_id, descriptor.proposal_height);
        let sender_authorized = sender.is_some_and(|peer| {
            validators.contains(peer)
                || self
                    .context
                    .roster
                    .iter()
                    .any(|entry| &entry.validator == peer)
        });
        let anchor_authorized = payload
            .origin_proposal
            .payload_block_hint
            .is_some_and(|hint| {
                self.global_anchor_matches_payload(
                    hint,
                    &payload.origin_proposal,
                    &payload.entrypoints,
                )
            });
        if descriptor.proposal_height == 0
            || descriptor.proposal_height > self.context.height
            || payload.epoch != proposal_epoch
            || !sender_authorized
            || !anchor_authorized
            || !validators.contains(&self.local_peer)
            || payload
                .validate(self.chain_id_hash(), proposal_epoch)
                .is_err()
            // A globally hinted V2 payload is reconstructed solely from the
            // globally committed entrypoints. Reservations, routing plans, and
            // Native AMX receipts are not committed by the ownership record;
            // accepting producer-supplied values would let one committee member
            // win Kura's first-write slot with a different execution policy.
            || !payload.reservation_keys.is_empty()
            || !payload.routing_plans.is_empty()
            || !payload.native_amx_receipts.is_empty()
            || (descriptor.proposal_height == self.context.height
                && !self.qc_mode_tag_matches_context(
                    &descriptor.qc_mode_tag,
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                ))
            || !self.lane_route_active(
                descriptor.lane_id,
                descriptor.dataspace_id,
                descriptor.lane_incarnation,
                descriptor.proposal_height,
            )
            || expected_validators.as_deref() != Some(validators.as_slice())
            || self.autonomous_origin_is_applied_or_snapshot_anchored(
                &payload.origin_proposal,
            )
        {
            return V2LaneIngressOutcome::Rejected;
        }

        let already_present = self
            .kura
            .read_autonomous_lane_block_artifact(
                descriptor.lane_id,
                descriptor.lane_block_height,
                self.chain_id_hash(),
                proposal_epoch,
            )
            .is_some();
        if !already_present && !self.lane_executable_payload_passes_stateful_preflight(&payload) {
            return V2LaneIngressOutcome::Rejected;
        }
        let proposal = payload.origin_proposal.clone();
        let mut next_sessions = self.lane_sessions.clone();
        let proposal_outcome = match next_sessions
            .insert_recovered_proposal_replacing_uncommitted_conflict(proposal.clone())
        {
            Ok(LaneBlockSessionInsertOutcome::Inserted) => V2LaneIngressOutcome::Inserted,
            Ok(LaneBlockSessionInsertOutcome::Duplicate) => V2LaneIngressOutcome::Duplicate,
            Err(_) => return V2LaneIngressOutcome::Rejected,
        };
        let Ok(availability_body) = crate::lane_consensus::lane_payload_availability_body(
            &payload,
            &proposal,
            self.chain_id_hash(),
            proposal_epoch,
        ) else {
            return V2LaneIngressOutcome::Rejected;
        };
        if next_sessions
            .authorize_payload_availability(&proposal, availability_body)
            .is_err()
        {
            return V2LaneIngressOutcome::Rejected;
        }
        if self
            .kura
            .persist_lane_executable_payload(&payload, self.chain_id_hash(), proposal_epoch)
            .is_err()
        {
            return V2LaneIngressOutcome::Rejected;
        }
        self.lane_sessions = next_sessions;
        if !already_present {
            self.fanout_lane_message(BlockMessage::LaneExecutablePayload(payload), &validators);
            self.fanout_lane_message(BlockMessage::LaneBlockProposal(proposal), &validators);
        }
        let _ = active_view;
        if already_present {
            V2LaneIngressOutcome::Duplicate
        } else {
            proposal_outcome
        }
    }

    fn lane_executable_payload_passes_stateful_preflight(
        &self,
        payload: &LaneExecutablePayloadV1,
    ) -> bool {
        let Ok(input) = Kura::autonomous_lane_block_execution_input_candidate(
            payload,
            self.chain_id_hash(),
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

    fn accept_lane_executable_payload_handoff(
        &mut self,
        handoff: LaneExecutablePayloadHandoffV1,
        sender: Option<&PeerId>,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let descriptor = &handoff.origin_proposal.descriptor;
        let validators = descriptor.validator_set.clone();
        let Some(hint) = handoff.origin_proposal.payload_block_hint else {
            return V2LaneIngressOutcome::Rejected;
        };
        let frozen_global_authority = self.frozen_validator_set();
        let expected_global_proposer = usize::try_from(self.context.leader(hint.proposal_view))
            .ok()
            .and_then(|index| self.context.roster.get(index))
            .map(|entry| &entry.validator);
        if descriptor.proposal_height != self.context.height
            || hint.proposal_height != self.context.height
            || handoff.epoch != self.context.epoch
            || handoff
                .validate(self.chain_id_hash(), self.context.epoch)
                .is_err()
            || handoff
                .validate_sender_authority(sender, &frozen_global_authority)
                .is_err()
            || expected_global_proposer != Some(&handoff.proposer)
            || !validators.contains(&self.local_peer)
            || self.local_peer.public_key() != self.key_pair.public_key()
            || self.local_peer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
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
            || self
                .expected_lane_validators(descriptor.lane_id, descriptor.proposal_height)
                .as_deref()
                != Some(validators.as_slice())
        {
            return V2LaneIngressOutcome::Rejected;
        }

        if self
            .globally_locked_body_hash
            .is_some_and(|block_hash| block_hash != hint.proposal_block_hash)
        {
            return V2LaneIngressOutcome::Rejected;
        }

        if self.observed_global_anchor(hint).is_none() {
            return match self.lane_payload_handoffs.insert(handoff) {
                Ok(LaneBlockNewViewCacheOutcome::Inserted) => V2LaneIngressOutcome::Inserted,
                Ok(LaneBlockNewViewCacheOutcome::Duplicate) => V2LaneIngressOutcome::Duplicate,
                Err(_) => V2LaneIngressOutcome::Rejected,
            };
        }
        if !self.global_anchor_matches_payload(hint, &handoff.origin_proposal, &handoff.entrypoints)
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let expected_payload_hash = handoff.payload_hash;
        let payload = match LaneExecutablePayloadV1::new_signed(
            handoff.chain_id_hash,
            handoff.epoch,
            handoff.origin_proposal,
            handoff.entrypoints,
            self.local_peer.clone(),
            self.key_pair.private_key(),
        ) {
            Ok(payload) if payload.payload_hash == expected_payload_hash => payload,
            _ => return V2LaneIngressOutcome::Rejected,
        };
        let local_peer = self.local_peer.clone();
        self.accept_lane_executable_payload(payload, Some(&local_peer), active_view)
    }

    fn process_deferred_lane_payload_handoffs(&mut self, active_view: wire::View) -> usize {
        let deferred = self.lane_payload_handoffs.snapshot();
        let mut processed = 0_usize;
        for handoff in deferred {
            let Some(hint) = handoff.origin_proposal.payload_block_hint else {
                self.lane_payload_handoffs.remove(&handoff);
                continue;
            };
            if self.observed_global_anchor(hint).is_none() {
                continue;
            }
            self.lane_payload_handoffs.remove(&handoff);
            let proposer = handoff.proposer.clone();
            if self.accept_lane_executable_payload_handoff(handoff, Some(&proposer), active_view)
                != V2LaneIngressOutcome::Rejected
            {
                processed = processed.saturating_add(1);
            }
        }
        processed
    }

    fn accept_lane_new_view_vote(
        &mut self,
        vote: LaneBlockNewViewVoteV1,
        sender: Option<&PeerId>,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let body = &vote.body;
        if sender != Some(&vote.signer)
            || body.chain_id_hash != self.chain_id_hash()
            || body.epoch != self.epoch_for_proposal_height(body.proposal_height)
            || body.proposal_height == 0
            || body.proposal_height > self.context.height
            || vote.validate_ingress().is_err()
            || !self.lane_route_active(
                body.lane_id,
                body.dataspace_id,
                body.lane_incarnation,
                body.proposal_height,
            )
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let Some((payload, current)) = self.kura.current_autonomous_lane_payload(
            body.lane_id,
            body.lane_block_height,
            self.chain_id_hash(),
            body.epoch,
        ) else {
            return V2LaneIngressOutcome::Rejected;
        };
        let Some(origin_hint) = payload.origin_proposal.payload_block_hint else {
            return V2LaneIngressOutcome::Rejected;
        };
        if !self.global_anchor_matches_payload(
            origin_hint,
            &payload.origin_proposal,
            &payload.entrypoints,
        ) {
            return V2LaneIngressOutcome::Rejected;
        }
        let expected = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
            &current,
            &payload,
            body.target_view,
            self.chain_id_hash(),
            body.epoch,
        );
        let validator_set = current.descriptor.validator_set.clone();
        if expected.as_ref() != Ok(body)
            || !validator_set.contains(&vote.signer)
            || self
                .expected_lane_validators(body.lane_id, body.proposal_height)
                .as_deref()
                != Some(validator_set.as_slice())
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let (cache_outcome, sealed) = match self
            .lane_new_view_votes
            .insert_and_maybe_seal(vote, &validator_set)
        {
            Ok(outcome) => outcome,
            Err(_) => return V2LaneIngressOutcome::Rejected,
        };
        if let Some(certificate) = sealed
            && self.install_lane_new_view_certificate(certificate, None, active_view)
                == V2LaneIngressOutcome::Rejected
        {
            return V2LaneIngressOutcome::Rejected;
        }
        match cache_outcome {
            LaneBlockNewViewCacheOutcome::Inserted => V2LaneIngressOutcome::Inserted,
            LaneBlockNewViewCacheOutcome::Duplicate => V2LaneIngressOutcome::Duplicate,
        }
    }

    fn accept_lane_new_view_certificate(
        &mut self,
        certificate: LaneBlockNewViewCertificateV1,
        sender: Option<&PeerId>,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let sender_authorized = sender.is_some_and(|peer| {
            certificate.validator_set.contains(peer)
                || self
                    .context
                    .roster
                    .iter()
                    .any(|entry| &entry.validator == peer)
        });
        if !sender_authorized {
            return V2LaneIngressOutcome::Rejected;
        }
        self.install_lane_new_view_certificate(certificate, sender, active_view)
    }

    fn install_lane_new_view_certificate(
        &mut self,
        certificate: LaneBlockNewViewCertificateV1,
        _sender: Option<&PeerId>,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let body = &certificate.body;
        let epoch = body.epoch;
        if body.chain_id_hash != self.chain_id_hash()
            || body.epoch != self.epoch_for_proposal_height(body.proposal_height)
            || body.proposal_height == 0
            || body.proposal_height > self.context.height
            || !self.lane_route_active(
                body.lane_id,
                body.dataspace_id,
                body.lane_incarnation,
                body.proposal_height,
            )
            || self
                .expected_lane_validators(body.lane_id, body.proposal_height)
                .as_deref()
                != Some(certificate.validator_set.as_slice())
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let Some(signer_pops) = self.lane_new_view_certificate_signer_pops(&certificate) else {
            return V2LaneIngressOutcome::Rejected;
        };
        if crate::lane_consensus::validate_lane_block_new_view_certificate(
            &certificate,
            &signer_pops,
        )
        .is_err()
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let Some((payload, certification_proposal)) =
            self.autonomous_certification_payload(body.lane_id, body.lane_block_height, epoch)
        else {
            return V2LaneIngressOutcome::Rejected;
        };
        let Some(origin_hint) = certification_proposal.payload_block_hint else {
            return V2LaneIngressOutcome::Rejected;
        };
        if !self.global_anchor_matches_payload(
            origin_hint,
            &certification_proposal,
            &payload.entrypoints,
        ) {
            return V2LaneIngressOutcome::Rejected;
        }
        let already_durable =
            self.kura
                .read_autonomous_lane_block_artifact(
                    body.lane_id,
                    body.lane_block_height,
                    self.chain_id_hash(),
                    body.epoch,
                )
                .is_some_and(|existing| {
                    existing
                        .new_view_certificates
                        .iter()
                        .any(|durable| durable.certificate == certificate)
                        || existing.view_checkpoint.as_ref().is_some_and(|checkpoint| {
                            checkpoint.certificate.certificate == certificate
                        })
                });
        let mut next_certificates = self.lane_new_view_certificates.clone();
        let cache_outcome = match next_certificates.insert(certificate.clone(), &signer_pops) {
            Ok(outcome) => outcome,
            Err(_) => return V2LaneIngressOutcome::Rejected,
        };
        let durable = DurableLaneBlockNewViewCertificateV1 {
            certificate: certificate.clone(),
            signer_pops: signer_pops.clone(),
        };
        let target_cursor = if already_durable {
            let Some((_payload, cursor)) = self.kura.current_autonomous_lane_payload(
                body.lane_id,
                body.lane_block_height,
                self.chain_id_hash(),
                body.epoch,
            ) else {
                return V2LaneIngressOutcome::Rejected;
            };
            if cursor.descriptor.lane_block_view < body.target_view {
                return V2LaneIngressOutcome::Rejected;
            }
            cursor
        } else {
            match self.kura.persist_lane_new_view_certificate(
                body.lane_id,
                body.lane_block_height,
                durable,
                self.chain_id_hash(),
                body.epoch,
            ) {
                Ok(target) => target,
                Err(_) => return V2LaneIngressOutcome::Rejected,
            }
        };
        self.lane_new_view_certificates = next_certificates;
        self.fanout_lane_message(
            BlockMessage::LaneBlockNewViewCertificate(certificate),
            &target_cursor.descriptor.validator_set,
        );
        if target_cursor.descriptor.validator_set != certification_proposal.descriptor.validator_set
        {
            return V2LaneIngressOutcome::Rejected;
        }
        self.fanout_lane_message(
            BlockMessage::LaneExecutablePayload(payload),
            &certification_proposal.descriptor.validator_set,
        );
        self.fanout_lane_message(
            BlockMessage::LaneBlockProposal(certification_proposal.clone()),
            &certification_proposal.descriptor.validator_set,
        );
        let _ = active_view;
        match cache_outcome {
            LaneBlockNewViewCacheOutcome::Inserted => V2LaneIngressOutcome::Inserted,
            LaneBlockNewViewCacheOutcome::Duplicate => V2LaneIngressOutcome::Duplicate,
        }
    }

    fn lane_new_view_certificate_signer_pops(
        &self,
        certificate: &LaneBlockNewViewCertificateV1,
    ) -> Option<BTreeMap<PublicKey, Vec<u8>>> {
        let aligned = self.lane_validator_set_pops(
            certificate.body.lane_id,
            certificate.body.proposal_height,
            &certificate.validator_set,
        )?;
        let mut pops = BTreeMap::new();
        for (byte_index, byte) in certificate.signers_bitmap.iter().copied().enumerate() {
            for bit in 0..8 {
                if byte & (1_u8 << bit) == 0 {
                    continue;
                }
                let index = byte_index * 8 + bit;
                let signer = certificate.validator_set.get(index)?;
                pops.insert(signer.public_key().clone(), aligned.get(index)?.clone());
            }
        }
        Some(pops)
    }

    fn lane_validator_set_pops(
        &self,
        lane_id: LaneId,
        authority_height: u64,
        validator_set: &[PeerId],
    ) -> Option<Vec<Vec<u8>>> {
        let nexus = self.state.nexus_snapshot();
        if authority_height < self.context.height
            && !(nexus.enabled && proposal_lookahead_enabled(&nexus, authority_height))
        {
            let store = super::v2_context_store::V2ContextStore::open_existing(
                self.kura.sumeragi_v2_storage_root(),
            )
            .ok()
            .flatten()?;
            let record = store.load(authority_height).ok().flatten()?;
            let historical = record
                .context()
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .zip(record.proofs_of_possession().iter().cloned())
                .collect::<BTreeMap<_, _>>();
            if historical.len() != validator_set.len() {
                return None;
            }
            return validator_set
                .iter()
                .map(|validator| historical.get(validator).cloned())
                .collect();
        }
        let pinned = super::main_loop::pinned_autoscale_validator_pops_for_set(
            &self.state,
            lane_id,
            validator_set,
        )?;
        let pops = if let Some(pops) = pinned {
            pops
        } else {
            let world = self.state.world_view();
            validator_set
                .iter()
                .map(|peer| {
                    crate::state::live_consensus_key_pop_for_peer(&world, peer, authority_height)
                })
                .collect::<Option<Vec<_>>>()?
        };
        (pops.len() == validator_set.len()
            && validator_set.iter().zip(&pops).all(|(peer, pop)| {
                pop.len() == crate::lane_consensus::LANE_BLS_PROOF_BYTES
                    && iroha_crypto::bls_normal_pop_verify(peer.public_key(), pop).is_ok()
            }))
        .then_some(pops)
    }

    fn drive_lane_new_views(&mut self, active_view: wire::View) {
        if !self.voting_enabled
            || self.local_peer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
        {
            return;
        }
        let proposals = self.lane_sessions.proposals_without_commit_qc();
        let mut active_cursors = BTreeSet::new();
        for proposal in proposals {
            if !proposal.descriptor.validator_set.contains(&self.local_peer) {
                continue;
            }
            let proposal_epoch =
                self.epoch_for_proposal_height(proposal.descriptor.proposal_height);
            let Some((payload, current_cursor)) =
                self.autonomous_view_cursor_for_proposal(&proposal)
            else {
                continue;
            };
            let cursor_key = (
                proposal.proposal_hash,
                current_cursor.descriptor.lane_block_view,
            );
            active_cursors.insert(cursor_key);
            if self.lane_new_view_waiting.insert(cursor_key) {
                continue;
            }
            let Some(target_view) = current_cursor.descriptor.lane_block_view.checked_add(1) else {
                continue;
            };
            let Ok(body) = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
                &current_cursor,
                &payload,
                target_view,
                self.chain_id_hash(),
                proposal_epoch,
            ) else {
                continue;
            };
            let Ok(vote) = LaneBlockNewViewVoteV1::new_signed(
                body,
                self.local_peer.clone(),
                self.key_pair.private_key(),
            ) else {
                continue;
            };
            if !self.lane_new_view_votes.contains(&vote) {
                let local_peer = self.local_peer.clone();
                let _ = self.accept_lane_new_view_vote(vote, Some(&local_peer), active_view);
            }
        }
        self.lane_new_view_waiting
            .retain(|cursor| active_cursors.contains(cursor));
        let local_votes = self.lane_new_view_votes.votes_for_signer(&self.local_peer);
        for vote in local_votes {
            let body = &vote.body;
            let proposal_epoch = self.epoch_for_proposal_height(body.proposal_height);
            let Some((payload, current)) = self.kura.current_autonomous_lane_payload(
                body.lane_id,
                body.lane_block_height,
                self.chain_id_hash(),
                proposal_epoch,
            ) else {
                continue;
            };
            let Ok(expected_body) = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
                &current,
                &payload,
                body.target_view,
                self.chain_id_hash(),
                proposal_epoch,
            ) else {
                continue;
            };
            let Some(validators) =
                self.expected_lane_validators(body.lane_id, body.proposal_height)
            else {
                continue;
            };
            if expected_body != *body
                || current.descriptor.validator_set != validators
                || vote.validate_ingress().is_err()
            {
                continue;
            }
            self.fanout_lane_message(BlockMessage::LaneBlockNewViewVote(vote), &validators);
        }
        for (body, votes) in self.lane_new_view_votes.quorum_vote_sets() {
            let Some(validators) =
                self.expected_lane_validators(body.lane_id, body.proposal_height)
            else {
                continue;
            };
            let Ok(certificate) = crate::lane_consensus::aggregate_lane_block_new_view_votes(
                body, validators, &votes,
            ) else {
                continue;
            };
            let _ = self.install_lane_new_view_certificate(certificate, None, active_view);
        }
    }

    /// Register a deterministic validation blocked only on one exact certified
    /// merge sidecar. Locked bodies may be re-proposed in a later consensus
    /// round, so the immutable carrier view is taken from the merge QC and may
    /// be earlier than `round.view`; it is never rebound to the later view.
    pub(crate) fn defer_missing_merge_sidecar(
        &mut self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        reference: CertifiedMergeLedgerReference,
    ) -> Result<MergeSidecarDeferralDisposition, V2LaneWorkError> {
        self.defer_missing_merge_sidecar_with_priority(round, subject, reference, false)
    }

    /// Register a decided Apply dependency using transport capacity reserved
    /// from speculative validation work.
    pub(crate) fn defer_missing_decided_merge_sidecar(
        &mut self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        reference: CertifiedMergeLedgerReference,
    ) -> Result<MergeSidecarDeferralDisposition, V2LaneWorkError> {
        self.defer_missing_merge_sidecar_with_priority(round, subject, reference, true)
    }

    fn defer_missing_merge_sidecar_with_priority(
        &mut self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        reference: CertifiedMergeLedgerReference,
        decided: bool,
    ) -> Result<MergeSidecarDeferralDisposition, V2LaneWorkError> {
        let Some(parent_hash) = subject.parent_block_hash else {
            return Ok(MergeSidecarDeferralDisposition::Rejected(
                "height-one body cannot carry a certified merge entry".to_owned(),
            ));
        };
        if round.context_id != self.context.id()
            || round.height != self.context.height
            || reference.merge_qc.carrier_height != round.height
            || reference.merge_qc.carrier_parent_hash != parent_hash
            || reference.merge_qc.view > round.view
        {
            return Ok(MergeSidecarDeferralDisposition::Rejected(
                "certified merge reference is not bound to the body's exact carrier height, parent, and immutable origin view"
                    .to_owned(),
            ));
        }
        // No transport progress is possible until the reserved outbound queue
        // drains. Avoid repeating a protocol-sized QC/PoP verification twice
        // per runner iteration while the exact deferral remains queued.
        if self.sidecar_effect_slots() == 0 {
            return Ok(MergeSidecarDeferralDisposition::RetryLater);
        }

        match self.kura.merge_entry_by_hash(reference.entry_hash) {
            Ok(Some(entry)) => {
                if !reference.matches_entry(&entry) {
                    return Ok(MergeSidecarDeferralDisposition::Rejected(
                        "durable merge sidecar differs from the block's compact reference"
                            .to_owned(),
                    ));
                }
                if let Err(error) = self
                    .state
                    .validate_certified_merge_entry_for_global_order(&entry)
                {
                    return Ok(MergeSidecarDeferralDisposition::Rejected(error.to_string()));
                }
                self.completed_merge_sidecars.insert(reference.entry_hash);
                return Ok(MergeSidecarDeferralDisposition::Available);
            }
            Ok(None) => {}
            Err(error) => {
                return Err(V2LaneWorkError::Persistence(error.to_string()));
            }
        }

        if let Err(error) = self.authenticate_merge_sidecar_reference(&reference) {
            return Ok(MergeSidecarDeferralDisposition::Rejected(error));
        }
        let committed_height = u64::try_from(self.state.committed_height())
            .map_err(|_| V2LaneWorkError::StateHeightMismatch)?;
        let deferred = if decided {
            self.merge_sidecars.defer_decided_block(
                subject.block_hash,
                round.height,
                reference.merge_qc.view,
                reference,
                &self.local_peer,
                committed_height,
                Instant::now(),
            )
        } else {
            self.merge_sidecars.defer_block(
                subject.block_hash,
                round.height,
                reference.merge_qc.view,
                reference,
                &self.local_peer,
                committed_height,
                Instant::now(),
            )
        };
        match deferred {
            Ok(Some(post)) => {
                debug_assert!(self.push_merge_sidecar_post(post));
                Ok(MergeSidecarDeferralDisposition::Fetching)
            }
            Ok(None) => Ok(MergeSidecarDeferralDisposition::Fetching),
            Err(MergeSidecarError::Capacity(_)) => Ok(MergeSidecarDeferralDisposition::RetryLater),
            Err(error) => Ok(MergeSidecarDeferralDisposition::Rejected(error.to_string())),
        }
    }

    fn authenticate_merge_sidecar_reference(
        &mut self,
        reference: &CertifiedMergeLedgerReference,
    ) -> Result<(), String> {
        validate_merge_sidecar_reference_bounds(&self.context, reference)?;
        let qc_key = bounded_merge_qc_authentication_key(reference)?;
        if self.authenticated_merge_qcs.contains(&qc_key) {
            return Ok(());
        }
        #[cfg(test)]
        {
            self.merge_qc_preflight_checks = self.merge_qc_preflight_checks.saturating_add(1);
        }
        authenticate_bounded_merge_sidecar_holders(&self.context, reference)?;
        if self.authenticated_merge_qcs.insert(qc_key) {
            self.authenticated_merge_qc_order.push_back(qc_key);
        }
        while self.authenticated_merge_qc_order.len() > MAX_AUTHENTICATED_MERGE_QCS {
            let oldest = self
                .authenticated_merge_qc_order
                .pop_front()
                .expect("non-empty authenticated-QC order");
            self.authenticated_merge_qcs.remove(&oldest);
        }
        Ok(())
    }

    /// Take one exact entry hash whose durable installation permits validation
    /// of all retained bodies referencing it to retry.
    pub(crate) fn take_completed_merge_sidecar(&mut self) -> Option<HashOf<MergeLedgerEntry>> {
        let hash = self.completed_merge_sidecars.iter().next().copied()?;
        self.completed_merge_sidecars.remove(&hash);
        Some(hash)
    }

    /// Take one exact full-entry rejection to apply to every retained body
    /// referencing the same hash.
    pub(crate) fn take_rejected_merge_sidecar(&mut self) -> Option<RejectedMergeSidecar> {
        let entry_hash = self.rejected_merge_sidecars.keys().next().copied()?;
        let reason = self
            .rejected_merge_sidecars
            .remove(&entry_hash)
            .expect("selected rejection exists");
        Some(RejectedMergeSidecar { entry_hash, reason })
    }

    /// Release transport reservations for validation tasks no longer owned by
    /// the executor after a certified view transition or terminal completion.
    pub(crate) fn retain_deferred_merge_sidecars(
        &mut self,
        pending_blocks: &BTreeSet<HashOf<BlockHeader>>,
    ) -> Result<(), V2LaneWorkError> {
        let committed_height = u64::try_from(self.state.committed_height())
            .map_err(|_| V2LaneWorkError::StateHeightMismatch)?;
        self.merge_sidecars
            .retain_pending_blocks(pending_blocks, committed_height);
        Ok(())
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
            LaneRelayMessage::LaneDrainVote { sender, vote } => {
                self.accept_lane_drain_vote(sender, vote, active_view)
            }
            LaneRelayMessage::MergeCandidate { sender, message } => {
                self.accept_merge_candidate(sender, message, active_view)
            }
            LaneRelayMessage::CertifiedMergeSidecar { sender, message } => {
                self.accept_certified_merge_sidecar(sender, message)
            }
            LaneRelayMessage::NativeAmx { sender, message } => {
                self.accept_native_amx(sender, message, active_view)
            }
        }
    }

    fn accept_lane_drain_vote(
        &mut self,
        sender: PeerId,
        vote: LaneDrainVoteV1,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let Some((expected_body, validator_set)) = self.state.pending_autoscale_lane_drain_body()
        else {
            self.lane_drain_votes.retain_body(None);
            return V2LaneIngressOutcome::Rejected;
        };
        self.lane_drain_votes
            .retain_body(Some(expected_body.clone()));
        if sender != vote.signer
            || vote.body != expected_body
            || !validator_set.contains(&vote.signer)
            || vote.validate_ingress().is_err()
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let changed = match self.lane_drain_votes.insert_vote(vote, Instant::now()) {
            Ok(changed) => changed,
            Err(_) => return V2LaneIngressOutcome::Rejected,
        };
        let _ = self.aggregate_active_lane_drain_certificate(&validator_set);
        self.refresh_merge_candidates(active_view);
        if changed {
            V2LaneIngressOutcome::Inserted
        } else {
            V2LaneIngressOutcome::Duplicate
        }
    }

    fn aggregate_active_lane_drain_certificate(&mut self, validator_set: &[PeerId]) -> bool {
        if self.lane_drain_votes.certificate().is_some() {
            return false;
        }
        let Some(body) = self.lane_drain_votes.active_body().cloned() else {
            return false;
        };
        let Ok(required) = usize::try_from(body.intent.min_quorum) else {
            return false;
        };
        if self.lane_drain_votes.votes().len() < required {
            return false;
        }
        let votes = self
            .lane_drain_votes
            .votes()
            .values()
            .cloned()
            .collect::<Vec<_>>();
        let Ok(certificate) = aggregate_lane_drain_votes(body, validator_set.to_vec(), &votes)
        else {
            return false;
        };
        self.lane_drain_votes.set_certificate(certificate);
        true
    }

    fn drive_lane_drain(
        &mut self,
        active_view: wire::View,
        rebroadcast: bool,
    ) -> Option<LaneDrainCertificateV1> {
        let Some((body, validator_set)) = self.state.pending_autoscale_lane_drain_body() else {
            self.lane_drain_votes.retain_body(None);
            return None;
        };
        self.lane_drain_votes.retain_body(Some(body.clone()));
        if validator_set.contains(&self.local_peer) {
            let local_vote = self.lane_drain_votes.votes().get(&self.local_peer).cloned();
            let vote = if let Some(vote) = local_vote {
                rebroadcast.then_some(vote)
            } else {
                match self.lane_drain_signing_guard.authorize_drain(&body) {
                    Ok(()) => match LaneDrainVoteV1::new_signed(
                        body.clone(),
                        self.local_peer.clone(),
                        self.key_pair.private_key(),
                    ) {
                        Ok(vote) => {
                            if self
                                .lane_drain_votes
                                .insert_vote(vote.clone(), Instant::now())
                                .is_err()
                            {
                                None
                            } else {
                                Some(vote)
                            }
                        }
                        Err(error) => {
                            self.latch_signing_guard_failure(format!(
                                "lane-drain vote signing failed after authorization: {error}"
                            ));
                            None
                        }
                    },
                    Err(
                        error @ (LaneDrainSigningGuardError::DrainEquivocation
                        | LaneDrainSigningGuardError::DrainFrontierBelowSignedCommit),
                    ) => {
                        iroha_logger::debug!(
                            ?error,
                            lane = body.intent.lane_id.as_u32(),
                            "v2 lane-drain vote blocked by durable frontier decision"
                        );
                        None
                    }
                    Err(error) => {
                        self.latch_signing_guard_failure(format!(
                            "lane-drain vote authorization failed: {error}"
                        ));
                        None
                    }
                }
            };
            if let Some(vote) = vote {
                let recipients = lane_drain_vote_recipients(
                    &validator_set,
                    &self.frozen_validator_set(),
                    &self.local_peer,
                );
                for peer in recipients {
                    if !self.push_effect(V2LaneWorkEffect::PostLaneDrainVote {
                        peer,
                        vote: vote.clone(),
                    }) {
                        break;
                    }
                }
                self.lane_drain_votes
                    .mark_local_vote_broadcast(Instant::now());
            }
        }
        let _ = active_view;
        let _ = self.aggregate_active_lane_drain_certificate(&validator_set);
        self.lane_drain_votes.certificate().cloned()
    }

    /// Drain at most `limit` explicit transport effects.
    pub(crate) fn drain_effects(&mut self, limit: usize) -> Vec<V2LaneWorkEffect> {
        let mut drained = Vec::with_capacity(
            limit.min(
                self.effects
                    .len()
                    .saturating_add(self.sidecar_effects.len()),
            ),
        );
        while drained.len() < limit {
            let both_ready = !self.sidecar_effects.is_empty() && !self.effects.is_empty();
            let take_sidecar = if both_ready {
                self.drain_sidecar_next
            } else {
                !self.sidecar_effects.is_empty()
            };
            let effect = if take_sidecar {
                let effect = self
                    .sidecar_effects
                    .pop_front()
                    .expect("sidecar effect selected only when present");
                self.sidecar_effect_keys
                    .remove(&lane_work_effect_key(&effect));
                effect
            } else if let Some(effect) = self.effects.pop_front() {
                self.effect_keys.remove(&lane_work_effect_key(&effect));
                effect
            } else {
                break;
            };
            if both_ready {
                self.drain_sidecar_next = !self.drain_sidecar_next;
            }
            drained.push(effect);
        }
        drained
    }

    /// Fail the serialized height runner after an unexpected durable signing
    /// journal error. Expected equivocation attempts are rejected locally and
    /// never populate this latch.
    pub(crate) fn ensure_healthy(&self) -> Result<(), V2LaneWorkError> {
        match self.signing_guard_failure.as_ref() {
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
        let _ = self.process_deferred_lane_payload_handoffs(active_view);
        self.drive_lane_sessions();
        let _ = self.drive_lane_drain(active_view, true);
        self.refresh_merge_candidates(active_view);
        let sidecar_posts = self.merge_sidecars.tick_bounded(
            &self.local_peer,
            Instant::now(),
            self.sidecar_effect_slots(),
        );
        for post in sidecar_posts {
            debug_assert!(self.push_merge_sidecar_post(post));
        }
        let candidate_slots = self.sidecar_effect_slots();
        let candidate_posts = self
            .merge_candidates
            .tick_bounded(Instant::now(), candidate_slots);
        for post in candidate_posts {
            if !self.push_merge_candidate_post(post) {
                break;
            }
        }
        self.drive_lane_new_views(active_view);
        let mut lane_artifacts = self
            .outbound_lane_handoffs
            .values()
            .filter(|handoff| self.lane_handoff_has_protected_global_anchor(handoff))
            .map(|handoff| {
                (
                    BlockMessage::LaneExecutablePayloadHandoff(handoff.clone()),
                    handoff.origin_proposal.descriptor.validator_set.clone(),
                )
            })
            .collect::<Vec<_>>();
        for proposal in self.lane_sessions.proposals_without_commit_qc() {
            if !self.proposal_body_available(&proposal) {
                continue;
            }
            if let Some(payload) = self.autonomous_payload_for_proposal(&proposal) {
                lane_artifacts.push((
                    BlockMessage::LaneExecutablePayload(payload),
                    proposal.descriptor.validator_set.clone(),
                ));
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
            if !self.persist_autonomous_lane_payload_availability_deliver(&qc) {
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
            for (key, pending) in &self.merge_entries {
                let Some(signature) = pending.signatures.get(&local_index) else {
                    continue;
                };
                merge_effects.push(V2LaneWorkEffect::BroadcastMerge(MergeCommitteeSignature {
                    epoch_id: key.epoch_id,
                    view: key.view,
                    signer: local_index,
                    message_digest: key.digest,
                    bls_sig: signature.clone(),
                }));
            }
        }
        for effect in merge_effects {
            self.push_effect(effect);
        }
        // Quorum formation and Kura publication are separate durability
        // boundaries. Retry already-quorate candidates on the normal bounded
        // retransmission cadence so a transient disk/capacity failure cannot
        // strand a certificate when no additional distinct signature arrives.
        let merge_keys = self.merge_entries.keys().copied().collect::<Vec<_>>();
        for key in merge_keys {
            self.try_commit_merge(key);
        }
    }

    fn sidecar_effect_slots(&self) -> usize {
        self.limits
            .relay_capacity
            .get()
            .saturating_sub(self.sidecar_effects.len())
    }

    fn push_merge_sidecar_post(&mut self, post: MergeSidecarPost) -> bool {
        let effect = V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: post.peer,
            message: post.message,
        };
        let key = lane_work_effect_key(&effect);
        if self.sidecar_effect_keys.contains(&key) {
            return true;
        }
        if self.sidecar_effect_slots() == 0 {
            return false;
        }
        self.sidecar_effect_keys.insert(key);
        self.sidecar_effects.push_back(effect);
        true
    }

    fn push_merge_candidate_post(&mut self, post: MergeCandidatePost) -> bool {
        let effect = V2LaneWorkEffect::PostMergeCandidate {
            peer: post.peer,
            message: post.message,
        };
        let key = lane_work_effect_key(&effect);
        if self.sidecar_effect_keys.contains(&key) {
            return true;
        }
        if self.sidecar_effect_slots() == 0 {
            return false;
        }
        self.sidecar_effect_keys.insert(key);
        self.sidecar_effects.push_back(effect);
        true
    }

    fn accept_certified_merge_sidecar(
        &mut self,
        sender: PeerId,
        message: CertifiedMergeSidecarMessage,
    ) -> V2LaneIngressOutcome {
        match message {
            CertifiedMergeSidecarMessage::Request(request) => {
                self.accept_certified_merge_sidecar_request(sender, request)
            }
            CertifiedMergeSidecarMessage::Chunk(chunk) => {
                self.accept_certified_merge_sidecar_chunk(sender, chunk)
            }
        }
    }

    fn accept_certified_merge_sidecar_request(
        &mut self,
        sender: PeerId,
        request: crate::merge_sidecar::CertifiedMergeSidecarRequestV1,
    ) -> V2LaneIngressOutcome {
        let now = Instant::now();
        if let Err(error) =
            self.merge_sidecars
                .admit_server_request(&sender, &request, &self.local_peer, now)
        {
            iroha_logger::debug!(%sender, ?error, "dropping v2 certified merge-sidecar request");
            return V2LaneIngressOutcome::Rejected;
        }
        let entry = match self.kura.merge_entry_by_hash(request.entry_hash) {
            Ok(Some(entry)) => entry,
            Ok(None) => return V2LaneIngressOutcome::Rejected,
            Err(error) => {
                iroha_logger::warn!(
                    %sender,
                    entry_hash = %request.entry_hash,
                    ?error,
                    "failed to read a requested v2 certified merge sidecar"
                );
                return V2LaneIngressOutcome::Rejected;
            }
        };
        let reference = CertifiedMergeLedgerReference::new(&entry);
        let metadata_matches = request.encoded_len == reference.encoded_len
            && request.epoch_id == reference.epoch_id
            && request.reference_digest == certified_merge_reference_digest(&reference);
        let local_is_holder = certified_merge_sidecar_holders(&reference)
            .is_ok_and(|holders| holders.contains(&self.local_peer));
        if !metadata_matches || !local_is_holder {
            return V2LaneIngressOutcome::Rejected;
        }
        if let Err(error) =
            self.merge_sidecars
                .enqueue_response(request, entry.canonical_bytes(), now)
        {
            iroha_logger::debug!(%sender, ?error, "v2 merge-sidecar response budget rejected request");
            return V2LaneIngressOutcome::Rejected;
        }
        let posts = self
            .merge_sidecars
            .drain_outbound_chunks(self.sidecar_effect_slots().min(8), now);
        let inserted = !posts.is_empty();
        for post in posts {
            debug_assert!(self.push_merge_sidecar_post(post));
        }
        if inserted {
            V2LaneIngressOutcome::Inserted
        } else {
            V2LaneIngressOutcome::Duplicate
        }
    }

    fn accept_certified_merge_sidecar_chunk(
        &mut self,
        sender: PeerId,
        chunk: crate::merge_sidecar::CertifiedMergeSidecarChunkV1,
    ) -> V2LaneIngressOutcome {
        let entry_hash = chunk.entry_hash;
        let now = Instant::now();
        let outcome = match self.merge_sidecars.ingest_chunk(&sender, chunk, now) {
            Ok(outcome) => outcome,
            Err(error) => {
                iroha_logger::debug!(%sender, %entry_hash, ?error, "dropping invalid v2 merge-sidecar chunk");
                return V2LaneIngressOutcome::Rejected;
            }
        };
        let ChunkIngestOutcome::Complete(completed) = outcome else {
            return V2LaneIngressOutcome::Inserted;
        };
        let reference_digest = certified_merge_reference_digest(&completed.reference);
        let entry = match decode_certified_merge_sidecar(&completed.reference, &completed.bytes) {
            Ok(entry) => entry,
            Err(error) => {
                iroha_logger::warn!(
                    %sender,
                    %entry_hash,
                    ?error,
                    "reassembled v2 certified merge sidecar is corrupt; rotating holder"
                );
                self.retry_completed_merge_sidecar(entry_hash, reference_digest, now);
                return V2LaneIngressOutcome::Rejected;
            }
        };
        if let Err(error) = self
            .state
            .validate_certified_merge_entry_for_global_order(&entry)
        {
            let affected = self.merge_sidecars.discard_invalid(entry_hash);
            if !affected.is_empty() {
                self.rejected_merge_sidecars
                    .entry(entry_hash)
                    .or_insert_with(|| error.to_string());
            }
            return V2LaneIngressOutcome::Rejected;
        }
        match self.kura.persist_pending_certified_merge_entry(&entry) {
            Ok(persisted_hash) if persisted_hash == entry_hash => {
                let (affected, _) = self.merge_sidecars.finish_completed(
                    entry_hash,
                    reference_digest,
                    true,
                    &self.local_peer,
                    now,
                );
                if !affected.is_empty() {
                    self.completed_merge_sidecars.insert(entry_hash);
                }
                V2LaneIngressOutcome::Inserted
            }
            Ok(other_hash) => {
                let affected = self.merge_sidecars.discard_invalid(entry_hash);
                if !affected.is_empty() {
                    self.rejected_merge_sidecars.entry(entry_hash).or_insert_with(|| {
                        format!(
                            "Kura persisted conflicting certified merge sidecar hash {other_hash}"
                        )
                    });
                }
                V2LaneIngressOutcome::Rejected
            }
            Err(error) => {
                iroha_logger::warn!(
                    %entry_hash,
                    ?error,
                    "failed to persist a validated v2 merge sidecar; rotating holder"
                );
                self.retry_completed_merge_sidecar(entry_hash, reference_digest, now);
                V2LaneIngressOutcome::Rejected
            }
        }
    }

    fn retry_completed_merge_sidecar(
        &mut self,
        entry_hash: HashOf<MergeLedgerEntry>,
        reference_digest: Hash,
        now: Instant,
    ) {
        let (_, retry) = self.merge_sidecars.finish_completed(
            entry_hash,
            reference_digest,
            false,
            &self.local_peer,
            now,
        );
        if let Some(post) = retry {
            if !self.push_merge_sidecar_post(post.clone())
                && let CertifiedMergeSidecarMessage::Request(request) = &post.message
            {
                self.merge_sidecars.release_unsent_request(request);
            }
        }
    }

    fn round_is_current(&self, round: wire::ConsensusRound) -> bool {
        round.context_id == self.context.id() && round.height == self.context.height
    }

    /// Advance the Native AMX round namespace and discard retransmittable work
    /// from the superseded global view. Authenticated participant lane-slot
    /// claims deliberately survive for this adapter's whole global height so a
    /// signer cannot hide proposal/settlement equivocation behind a view change.
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
        let mut next_sessions = self.lane_sessions.clone();
        let outcome = match next_sessions.insert_qc_with_pops(qc.clone(), &pops) {
            Ok(outcome) => outcome,
            Err(_) => return V2LaneIngressOutcome::Rejected,
        };
        if !self.persist_autonomous_lane_payload_availability_deliver(&qc) {
            return V2LaneIngressOutcome::Rejected;
        }
        self.lane_sessions = next_sessions;
        match outcome {
            LaneBlockSessionInsertOutcome::Inserted => V2LaneIngressOutcome::Inserted,
            LaneBlockSessionInsertOutcome::Duplicate => V2LaneIngressOutcome::Duplicate,
        }
    }

    fn persist_autonomous_lane_payload_availability_deliver(&self, qc: &LaneBlockQcV1) -> bool {
        if qc.body.phase != CertPhase::Prepare {
            return true;
        }
        let epoch = self.epoch_for_proposal_height(qc.body.proposal_height);
        let Some(artifact) = self.kura.read_autonomous_lane_block_artifact(
            qc.body.lane_id,
            qc.body.lane_block_height,
            self.chain_id_hash(),
            epoch,
        ) else {
            return true;
        };
        if qc.payload_availability_qc.is_none() {
            return false;
        }
        let durable = DurableLanePayloadAvailabilityCertificateV1 {
            certificate: qc.clone(),
        };
        if crate::lane_consensus::validate_lane_payload_availability_certificate(
            &durable,
            &artifact.executable_payload,
            self.chain_id_hash(),
            epoch,
        )
        .is_err()
        {
            return false;
        }
        self.kura
            .persist_lane_payload_availability_certificate(
                qc.body.lane_id,
                qc.body.lane_block_height,
                durable,
                self.chain_id_hash(),
                epoch,
            )
            .is_ok()
    }

    fn lane_proposal_authorized(
        &self,
        proposal: &LaneBlockProposalV1,
        sender: Option<&PeerId>,
        local: bool,
        active_view: wire::View,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        if self.autonomous_payload_for_proposal(proposal).is_some() {
            let sender_authorized = sender.is_some_and(|peer| {
                descriptor.validator_set.contains(peer)
                    || self
                        .context
                        .roster
                        .iter()
                        .any(|entry| &entry.validator == peer)
            });
            return descriptor.proposal_height > 0
                && descriptor.proposal_height <= self.context.height
                && (descriptor.proposal_height != self.context.height
                    || self.qc_mode_tag_matches_context(
                        &descriptor.qc_mode_tag,
                        descriptor.lane_id,
                        descriptor.dataspace_id,
                    ))
                && self.lane_route_active(
                    descriptor.lane_id,
                    descriptor.dataspace_id,
                    descriptor.lane_incarnation,
                    descriptor.proposal_height,
                )
                && self.expected_lane_validators(descriptor.lane_id, descriptor.proposal_height)
                    == Some(descriptor.validator_set.clone())
                && ((local && descriptor.validator_set.contains(&self.local_peer))
                    || sender_authorized);
        }
        if let Some(anchor) = self.canonical_anchor_for_proposal(proposal) {
            return proposal.payload_block_hint.as_ref().is_some_and(|hint| {
                hint.proposal_block_hash == anchor.proposal_block_hash
                    && hint.proposal_height == descriptor.proposal_height
                    && hint.proposal_view == anchor.ownership.proposal_view
            });
        }
        if descriptor.proposal_height != self.context.height
            || descriptor.lane_block_view > active_view
            || proposal.payload_block_hint.as_ref().is_none_or(|hint| {
                hint.proposal_height != descriptor.proposal_height
                    || hint.proposal_view > active_view
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
        let local_is_global_proposer = usize::try_from(self.context.leader(active_view))
            .ok()
            .and_then(|index| self.context.roster.get(index))
            .is_some_and(|entry| entry.validator == self.local_peer);
        (local && (&self.local_peer == author || local_is_global_proposer))
            || sender == Some(author)
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
        if let Some(proposal) = self.autonomous_proposal_for_vote_body(body) {
            return proposal.descriptor.validator_set.contains(&vote.signer)
                && proposal.vote_body(body.phase) == *body;
        }
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
        if let Some(proposal) = self.autonomous_proposal_for_vote_body(body) {
            return proposal.descriptor.validator_set == qc.validator_set
                && proposal.vote_body(body.phase) == *body;
        }
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

        for qc in self.lane_sessions.drain_newly_sealed_qcs() {
            if !self.persist_autonomous_lane_payload_availability_deliver(&qc) {
                // Do not drain a commit-vote request until the exact READY QC
                // is crash-safe. The cached QC remains available to the
                // retransmission path, which retries this persistence boundary.
                return;
            }
            let validators = qc.validator_set.clone();
            self.fanout_lane_message(BlockMessage::LaneBlockQc(qc), &validators);
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
            if !self.persist_autonomous_lane_payload_availability_deliver(&qc) {
                continue;
            }
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
        let mut sessions = self
            .state
            .certified_lane_block_sessions_snapshot_cached(self.limits.session_capacity.get());
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
        &mut self,
        body: iroha_data_model::block::consensus::LaneBlockVoteBodyV1,
    ) -> Option<LaneBlockVoteV1> {
        if !self.voting_enabled
            || self.local_peer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
        {
            return None;
        }
        if body.phase == CertPhase::Commit {
            if let Some(proposal) = self.autonomous_proposal_for_vote_body(&body)
                && !self.autonomous_payload_availability_deliver_is_durable(&proposal)
            {
                return None;
            }
            match self.lane_drain_signing_guard.authorize_commit_vote(&body) {
                Ok(()) => {}
                Err(
                    error @ (LaneDrainSigningGuardError::CommitVoteEquivocation
                    | LaneDrainSigningGuardError::LaneClosed),
                ) => {
                    iroha_logger::debug!(
                        ?error,
                        lane = body.lane_id.as_u32(),
                        lane_block_height = body.lane_block_height,
                        "v2 lane Commit vote blocked by durable drain safety decision"
                    );
                    return None;
                }
                Err(error) => {
                    self.latch_signing_guard_failure(format!(
                        "lane Commit-vote authorization failed: {error}"
                    ));
                    return None;
                }
            }
        }
        let payload_availability_vote = if body.phase == CertPhase::Prepare {
            let key = crate::lane_consensus::LaneBlockSessionKey {
                lane_id: body.lane_id,
                dataspace_id: body.dataspace_id,
                lane_incarnation: body.lane_incarnation,
                lane_block_height: body.lane_block_height,
                lane_block_view: body.lane_block_view,
                proposal_hash: body.proposal_hash,
            };
            let proposal = self.lane_sessions.proposal_for_key(&key)?;
            if let Some(payload) = self.autonomous_payload_for_proposal(&proposal) {
                let proposal_epoch =
                    self.epoch_for_proposal_height(proposal.descriptor.proposal_height);
                let availability_body = crate::lane_consensus::lane_payload_availability_body(
                    &payload,
                    &proposal,
                    self.chain_id_hash(),
                    proposal_epoch,
                )
                .ok()?;
                let validator_set_pops = self.lane_validator_set_pops(
                    body.lane_id,
                    body.proposal_height,
                    &proposal.descriptor.validator_set,
                )?;
                Some(
                    LanePayloadAvailabilityVoteV1::new_signed(
                        availability_body,
                        self.local_peer.clone(),
                        validator_set_pops,
                        self.key_pair.private_key(),
                    )
                    .ok()?,
                )
            } else {
                None
            }
        } else {
            None
        };
        let signature =
            Signature::try_new(self.key_pair.private_key(), &body.signature_preimage()).ok()?;
        Some(LaneBlockVoteV1 {
            body,
            payload_availability_vote,
            signer: self.local_peer.clone(),
            bls_signature: signature.payload().to_vec(),
        })
    }

    fn autonomous_payload_availability_deliver_is_durable(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> bool {
        let descriptor = &proposal.descriptor;
        let epoch = self.epoch_for_proposal_height(descriptor.proposal_height);
        self.kura
            .read_autonomous_lane_block_artifact(
                descriptor.lane_id,
                descriptor.lane_block_height,
                self.chain_id_hash(),
                epoch,
            )
            .and_then(|artifact| artifact.availability_certificate)
            .is_some_and(|durable| {
                durable.certificate.body == proposal.vote_body(CertPhase::Prepare)
                    && durable.certificate.payload_availability_qc.is_some()
            })
    }

    fn latch_signing_guard_failure(&mut self, error: String) {
        if self.signing_guard_failure.is_none() {
            iroha_logger::error!(%error, "v2 lane work signing safety failed closed");
            self.signing_guard_failure = Some(error);
        }
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
        let nexus = self.state.nexus_snapshot();
        if nexus.enabled && proposal_lookahead_enabled(&nexus, proposal.descriptor.proposal_height)
        {
            return self.autonomous_payload_for_proposal(proposal).is_some()
                && (self.canonical_anchor_for_proposal(proposal).is_some()
                    || self
                        .locally_bound_lane_proposals
                        .contains(&proposal.proposal_hash));
        }
        self.canonical_anchor_for_proposal(proposal).is_some()
            || self
                .locally_bound_lane_proposals
                .contains(&proposal.proposal_hash)
            || self.autonomous_payload_for_proposal(proposal).is_some()
    }

    fn chain_id_hash(&self) -> Hash {
        Hash::new(self.context.chain_id.clone().into_inner().as_bytes())
    }

    fn epoch_for_proposal_height(&self, height: u64) -> u64 {
        if height == self.context.height {
            self.context.epoch
        } else {
            let world = self.state.world_view();
            super::epoch_for_height_from_world(&world, height)
        }
    }

    fn autonomous_payload_for_proposal(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> Option<LaneExecutablePayloadV1> {
        let descriptor = &proposal.descriptor;
        let epoch = self.epoch_for_proposal_height(descriptor.proposal_height);
        let (payload, certification_proposal) = self.autonomous_certification_payload(
            descriptor.lane_id,
            descriptor.lane_block_height,
            epoch,
        )?;
        let hint = certification_proposal.payload_block_hint?;
        (certification_proposal.same_consensus_identity(proposal)
            && self.global_anchor_matches_payload(
                hint,
                &certification_proposal,
                &payload.entrypoints,
            ))
        .then_some(payload)
    }

    fn autonomous_certification_payload(
        &self,
        lane_id: LaneId,
        lane_block_height: u64,
        epoch: u64,
    ) -> Option<(LaneExecutablePayloadV1, LaneBlockProposalV1)> {
        self.kura.autonomous_lane_certification_payload(
            lane_id,
            lane_block_height,
            self.chain_id_hash(),
            epoch,
        )
    }

    fn autonomous_view_cursor_for_proposal(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> Option<(LaneExecutablePayloadV1, LaneBlockProposalV1)> {
        let descriptor = &proposal.descriptor;
        let epoch = self.epoch_for_proposal_height(descriptor.proposal_height);
        let (payload, cursor) = self.kura.current_autonomous_lane_payload(
            descriptor.lane_id,
            descriptor.lane_block_height,
            self.chain_id_hash(),
            epoch,
        )?;
        let origin_hint = payload.origin_proposal.payload_block_hint?;
        (payload.origin_proposal.same_consensus_identity(proposal)
            && self.global_anchor_matches_payload(
                origin_hint,
                &payload.origin_proposal,
                &payload.entrypoints,
            ))
        .then_some((payload, cursor))
    }

    fn observed_global_anchor(&self, hint: LaneBlockProposalPayloadHintV1) -> Option<SignedBlock> {
        if let Some(block) = self.globally_locked_body.as_ref()
            && block.hash() == hint.proposal_block_hash
            && block.header().height().get() == hint.proposal_height
            && block.header().view_change_index() == hint.proposal_view
        {
            return Some(block.clone());
        }
        let height = usize::try_from(hint.proposal_height)
            .ok()
            .and_then(NonZeroUsize::new)?;
        self.kura.get_block(height).and_then(|block| {
            (block.hash() == hint.proposal_block_hash
                && block.header().view_change_index() == hint.proposal_view)
                .then(|| block.as_ref().clone())
        })
    }

    fn global_anchor_matches_payload(
        &self,
        hint: LaneBlockProposalPayloadHintV1,
        proposal: &LaneBlockProposalV1,
        entrypoints: &[iroha_data_model::transaction::TransactionEntrypoint],
    ) -> bool {
        let Some(block) = self.observed_global_anchor(hint) else {
            return false;
        };
        Self::global_block_matches_payload(&block, hint, proposal, entrypoints)
    }

    fn global_block_matches_payload(
        block: &SignedBlock,
        hint: LaneBlockProposalPayloadHintV1,
        proposal: &LaneBlockProposalV1,
        entrypoints: &[iroha_data_model::transaction::TransactionEntrypoint],
    ) -> bool {
        if block.hash() != hint.proposal_block_hash
            || block.header().height().get() != hint.proposal_height
            || block.header().view_change_index() != hint.proposal_view
        {
            return false;
        }
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
        selected.as_deref() == Some(entrypoints)
            && block.execution_context().is_some_and(|context| {
                context.lane_payload_ownerships.iter().any(|ownership| {
                    proposal_from_ownership(ownership, hint.proposal_block_hash)
                        .is_some_and(|anchored| anchored.same_consensus_identity(proposal))
                })
            })
    }

    fn autonomous_origin_is_applied_or_snapshot_anchored(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> bool {
        self.kura.lane_block_application_receipt_available(proposal)
            || self
                .canonical_anchor_for_proposal(proposal)
                .is_some_and(|artifact| {
                    self.state
                        .lane_block_artifact_is_applied_or_snapshot_anchored_cached(&artifact)
                })
    }

    fn lane_handoff_has_protected_global_anchor(
        &self,
        handoff: &LaneExecutablePayloadHandoffV1,
    ) -> bool {
        self.locally_bound_lane_proposals
            .contains(&handoff.origin_proposal.proposal_hash)
            || handoff
                .origin_proposal
                .payload_block_hint
                .is_some_and(|hint| {
                    self.global_anchor_matches_payload(
                        hint,
                        &handoff.origin_proposal,
                        &handoff.entrypoints,
                    )
                })
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
            || self.autonomous_proposal_for_vote_body(body).is_some()
    }

    fn expected_lane_validators(
        &self,
        lane_id: LaneId,
        proposal_height: u64,
    ) -> Option<Vec<PeerId>> {
        if proposal_height == 0 || proposal_height > self.context.height {
            return None;
        }
        let nexus = self.state.nexus_snapshot();
        let mut validators = if nexus.enabled && proposal_lookahead_enabled(&nexus, proposal_height)
        {
            self.state
                .authoritative_lane_peer_ids_at_height(lane_id, proposal_height)
        } else if proposal_height == self.context.height {
            self.context
                .roster
                .iter()
                .map(|entry| entry.validator.clone())
                .collect()
        } else {
            let store = super::v2_context_store::V2ContextStore::open_existing(
                self.kura.sumeragi_v2_storage_root(),
            )
            .ok()
            .flatten()?;
            let record = store.load(proposal_height).ok().flatten()?;
            record
                .context()
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
        tag == self.canonical_qc_mode_tag(lane_id, dataspace_id)
    }

    fn canonical_qc_mode_tag(&self, lane_id: LaneId, dataspace_id: DataSpaceId) -> String {
        let base = match self.context.mode {
            wire::ConsensusMode::Permissioned => wire::PERMISSIONED_TAG,
            wire::ConsensusMode::Npos => wire::NPOS_TAG,
        };
        let context_tag = format!(
            "{base}::height-context:{}::epoch:{}",
            hex::encode(self.context.id().0.as_ref()),
            self.context.epoch
        );
        LaneRelayEnvelope::lane_qc_mode_tag_for(lane_id, dataspace_id, &context_tag)
    }

    fn hydrate_canonical_lane_artifacts(&mut self) -> Result<(), V2LaneWorkError> {
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

        let mut certified_proposals = BTreeSet::new();
        let certified_sessions = self
            .state
            .certified_lane_block_sessions_snapshot_cached(self.limits.session_capacity.get())
            .into_iter()
            .filter(|session| {
                !self
                    .state
                    .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(session)
                    && self.session_has_canonical_anchor(session)
            })
            .take(self.limits.session_capacity.get())
            .collect::<Vec<_>>();
        for session in certified_sessions {
            certified_proposals.insert(session.proposal.proposal_hash);
            self.pending_committed_lanes.push_back(session);
        }

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
            if certified_proposals.contains(&proposal.proposal_hash) {
                continue;
            }
            self.lane_sessions
                .insert_recovered_proposal_replacing_uncommitted_conflict(proposal)
                .map_err(|error| V2LaneWorkError::RolloverConflict(error.to_string()))?;
        }
        Ok(())
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

    fn autonomous_proposal_for_vote_body(
        &self,
        body: &iroha_data_model::block::consensus::LaneBlockVoteBodyV1,
    ) -> Option<LaneBlockProposalV1> {
        let (payload, proposal) = self.autonomous_certification_payload(
            body.lane_id,
            body.lane_block_height,
            self.epoch_for_proposal_height(body.proposal_height),
        )?;
        let hint = proposal.payload_block_hint?;
        (proposal.vote_body(body.phase) == *body
            && self.global_anchor_matches_payload(hint, &proposal, &payload.entrypoints))
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
        let slot_key = NativeVoteSlotKey::from_body(&vote.body, &vote.signer);
        let slot_claim = NativeVoteSlotClaim::from_body(&vote.body);
        if self.native_vote_slot_conflicts(slot_key, slot_claim) {
            return V2LaneIngressOutcome::Rejected;
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
        if self.native_claims.len() >= claim_capacity
            || (!self.native_slot_claims.contains_key(&slot_key)
                && self.native_slot_claims.len() >= claim_capacity)
        {
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
                self.native_slot_claims
                    .entry(slot_key)
                    .or_insert(slot_claim);
                V2LaneIngressOutcome::Inserted
            }
            Err(NativeAmxSessionError::DuplicateSigner) => {
                self.native_claims.insert(key, body);
                self.native_claim_signatures.insert(key, signature);
                self.native_slot_claims
                    .entry(slot_key)
                    .or_insert(slot_claim);
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

    fn native_vote_slot_conflicts(
        &self,
        key: NativeVoteSlotKey,
        claim: NativeVoteSlotClaim,
    ) -> bool {
        self.native_slot_claims
            .get(&key)
            .is_some_and(|accepted| *accepted != claim)
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
        ) && self.native_participant_request_is_authoritative(request)
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
        ) && self.native_participant_request_is_authoritative(request)
    }

    fn native_participant_request_is_authoritative(
        &self,
        request: &NativeAmxAttestationRequestV2,
    ) -> bool {
        let body = &request.body;
        let Some((validators, min_signers)) = self.native_committee_shape_for_route(
            body.participant_lane_id,
            body.participant_dataspace_id,
            body.authority_context_height,
        ) else {
            return false;
        };
        let Some((previous_height, previous_hash)) = v2_known_lane_tip_for_route(
            self.state.as_ref(),
            self.kura.as_ref(),
            body.authority_context_height,
            body.participant_lane_id,
            body.participant_dataspace_id,
            body.participant_lane_incarnation,
        ) else {
            return false;
        };
        let descriptor = &request.participant_proposal.descriptor;
        request.validate_plan_binding().is_ok()
            && body.participant_previous_block_height == previous_height
            && body.participant_previous_block_descriptor_hash == previous_hash
            && body.participant_lane_block_height == previous_height.saturating_add(1)
            && descriptor.validator_set == validators
            && usize::try_from(descriptor.min_quorum).ok() == Some(min_signers)
            && self.qc_mode_tag_matches_context(
                &descriptor.qc_mode_tag,
                body.participant_lane_id,
                body.participant_dataspace_id,
            )
    }

    fn native_coordinator_predecessor(
        &self,
        body: &NativeAmxAttestationBodyV2,
    ) -> Option<(u64, Option<Hash>)> {
        let Some(artifact) =
            self.kura
                .latest_lane_block_artifact_matching(body.coordinator_lane_id, |artifact| {
                    let ownership = &artifact.ownership;
                    ownership.dataspace_id == body.coordinator_dataspace_id
                        && ownership.lane_incarnation == body.coordinator_lane_incarnation
                        && ownership.proposal_height < body.authority_context_height
                        && self.lane_route_active(
                            ownership.lane_id,
                            ownership.dataspace_id,
                            ownership.lane_incarnation,
                            ownership.proposal_height,
                        )
                })
        else {
            return Some((0, None));
        };
        let ownership = artifact.ownership;
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
        if self.signing_guard_failure.is_some()
            || !native_signing_guard_required(self.voting_enabled, &self.local_peer)
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
        let Some(native_signing_guard) = self.native_signing_guard.as_ref() else {
            let message =
                "Native AMX signing guard is unavailable for a voting BLS peer".to_owned();
            iroha_logger::error!(
                height = body.round.height,
                view = body.round.view,
                "Native AMX signing guard failed closed"
            );
            self.signing_guard_failure = Some(message);
            return None;
        };
        match native_signing_guard.record(&body) {
            Ok(()) => {}
            Err(
                NativeAmxSigningGuardError::Equivocation
                | NativeAmxSigningGuardError::PlanEquivocation
                | NativeAmxSigningGuardError::SlotEquivocation,
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
                if self.signing_guard_failure.is_none() {
                    iroha_logger::error!(
                        %error,
                        height = body.round.height,
                        view = body.round.view,
                        "Native AMX signing guard failed closed"
                    );
                    self.signing_guard_failure = Some(message);
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

    fn prepare_native_participant_controls(
        &self,
        candidates: &[CandidateDescriptor<'_>],
        coordinator_proposals: &[LaneBlockProposalV1],
    ) -> Result<NativeParticipantControlMap, BTreeSet<usize>> {
        let mut grouped =
            BTreeMap::<(LaneId, DataSpaceId), Vec<(usize, Hash, [u8; Hash::LENGTH])>>::new();
        for (candidate_index, candidate) in candidates.iter().copied().enumerate() {
            let RoutingPlan::NativeAmx(plan) = candidate.routing_plan() else {
                continue;
            };
            let entrypoint_hash = Hash::from(candidate.entrypoint_hash());
            let mut source_id = [0_u8; Hash::LENGTH];
            source_id.copy_from_slice(candidate.transaction().hash().as_ref());
            for participant in &plan.participants {
                grouped
                    .entry((participant.route.lane_id, participant.route.dataspace_id))
                    .or_default()
                    .push((candidate_index, entrypoint_hash, source_id));
            }
        }

        let mut unavailable = BTreeSet::new();
        let mut controls = BTreeMap::new();
        for ((lane_id, dataspace_id), mut members) in grouped {
            let route = RoutingDecision::new(lane_id, dataspace_id);
            members.sort_by_key(|(index, _, _)| *index);
            if members.len() > crate::native_amx::MAX_NATIVE_AMX_PARTICIPANT_CONTROL_SOURCES {
                unavailable.extend(members.iter().map(|(index, _, _)| *index));
                continue;
            }
            let coordinator_proposal = coordinator_proposals.iter().find(|proposal| {
                proposal.descriptor.lane_id == route.lane_id
                    && proposal.descriptor.dataspace_id == route.dataspace_id
            });
            let Some(participant_lane_incarnation) = self
                .state
                .lane_incarnation_at_height(route.lane_id, self.context.height)
            else {
                unavailable.extend(members.iter().map(|(index, _, _)| *index));
                continue;
            };
            let Some((validators, min_signers)) = self.native_committee_shape_for_route(
                route.lane_id,
                route.dataspace_id,
                self.context.height,
            ) else {
                unavailable.extend(members.iter().map(|(index, _, _)| *index));
                continue;
            };
            let proposal = if let Some(proposal) = coordinator_proposal {
                proposal.clone()
            } else {
                let Some((previous_height, previous_hash)) = v2_known_lane_tip_for_route(
                    self.state.as_ref(),
                    self.kura.as_ref(),
                    self.context.height,
                    route.lane_id,
                    route.dataspace_id,
                    participant_lane_incarnation,
                ) else {
                    unavailable.extend(members.iter().map(|(index, _, _)| *index));
                    continue;
                };
                let Some(lane_block_height) = previous_height.checked_add(1) else {
                    unavailable.extend(members.iter().map(|(index, _, _)| *index));
                    continue;
                };
                let lane_block_view = 0;
                let accepted_candidate_indices = members
                    .iter()
                    .map(|(index, _, _)| u64::try_from(*index))
                    .collect::<Result<Vec<_>, _>>();
                let Ok(accepted_candidate_indices) = accepted_candidate_indices else {
                    unavailable.extend(members.iter().map(|(index, _, _)| *index));
                    continue;
                };
                let accepted_transaction_hashes =
                    members.iter().map(|(_, hash, _)| *hash).collect::<Vec<_>>();
                let qc_mode_tag = self.canonical_qc_mode_tag(route.lane_id, route.dataspace_id);
                let mut ownership = SumeragiLanePayloadOwnership {
                    proposal_height: self.context.height,
                    // This is an independent participant control view, not the
                    // coordinator's global consensus view.
                    proposal_view: lane_block_view,
                    lane_id: route.lane_id,
                    dataspace_id: route.dataspace_id,
                    lane_incarnation: participant_lane_incarnation,
                    lane_block_height,
                    lane_block_view,
                    subject_hash: Hash::prehashed([0; Hash::LENGTH]),
                    qc_mode_tag: qc_mode_tag.clone(),
                    accepted_candidate_indices: accepted_candidate_indices.clone(),
                    accepted_transaction_hashes: accepted_transaction_hashes.clone(),
                    previous_lane_block_height: previous_height,
                    previous_lane_block_descriptor_hash: previous_hash,
                    lane_block_descriptor_hash: Some(Hash::prehashed([1; Hash::LENGTH])),
                    lane_block_descriptor_validator_set: validators.clone(),
                    lane_block_descriptor_validator_count: u32::try_from(validators.len())
                        .unwrap_or(u32::MAX),
                    lane_block_descriptor_min_quorum: u32::try_from(min_signers)
                        .unwrap_or(u32::MAX),
                    payload_ownership_hash: Hash::prehashed([0; Hash::LENGTH]),
                    rbc_instance_hash: Hash::prehashed([0; Hash::LENGTH]),
                };
                let Ok(replay) = ownership.compute_replay_hashes() else {
                    unavailable.extend(members.iter().map(|(index, _, _)| *index));
                    continue;
                };
                ownership.subject_hash = replay.subject_hash;
                ownership.payload_ownership_hash = replay.payload_ownership_hash;
                ownership.rbc_instance_hash = replay.rbc_instance_hash;
                ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
                let descriptor = LaneBlockDescriptorV1 {
                    lane_id: ownership.lane_id,
                    dataspace_id: ownership.dataspace_id,
                    lane_incarnation: ownership.lane_incarnation,
                    proposal_height: ownership.proposal_height,
                    previous_lane_block_height: ownership.previous_lane_block_height,
                    previous_lane_block_descriptor_hash: ownership
                        .previous_lane_block_descriptor_hash,
                    lane_block_height: ownership.lane_block_height,
                    lane_block_view: ownership.lane_block_view,
                    subject_hash: ownership.subject_hash,
                    payload_ownership_hash: ownership.payload_ownership_hash,
                    rbc_instance_hash: ownership.rbc_instance_hash,
                    accepted_candidate_indices,
                    accepted_transaction_hashes,
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&validators),
                    validator_set: validators.clone(),
                    validator_count: ownership.lane_block_descriptor_validator_count,
                    min_quorum: ownership.lane_block_descriptor_min_quorum,
                    qc_mode_tag,
                    descriptor_hash: replay.lane_block_descriptor_hash,
                };
                let mut proposal = LaneBlockProposalV1 {
                    descriptor,
                    proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
                    payload_block_hint: None,
                };
                proposal.proposal_hash = proposal.computed_proposal_hash();
                proposal
            };
            if crate::lane_consensus::validate_lane_block_proposal(&proposal).is_err()
                || proposal.descriptor.lane_incarnation != participant_lane_incarnation
                || proposal.descriptor.validator_set != validators
                || usize::try_from(proposal.descriptor.min_quorum).ok() != Some(min_signers)
            {
                unavailable.extend(members.iter().map(|(index, _, _)| *index));
                continue;
            }
            let receipts = members
                .iter()
                .map(|(_, _, source_id)| LaneSettlementReceipt {
                    source_id: *source_id,
                    local_amount_micro: 0,
                    xor_due_micro: 0,
                    xor_after_haircut_micro: 0,
                    xor_variance_micro: 0,
                    timestamp_ms: self.context.height,
                })
                .collect::<Vec<_>>();
            let settlement = LaneBlockCommitment {
                block_height: proposal.descriptor.lane_block_height,
                lane_id: route.lane_id,
                lane_incarnation: participant_lane_incarnation,
                dataspace_id: route.dataspace_id,
                tx_count: u64::try_from(receipts.len()).unwrap_or(u64::MAX),
                total_local_micro: 0,
                total_xor_due_micro: 0,
                total_xor_after_haircut_micro: 0,
                total_xor_variance_micro: 0,
                swap_metadata: None,
                receipts,
                nexus_fee_receipts: Vec::new(),
                native_amx_receipts: Vec::new(),
            };
            controls.insert(
                (route.lane_id, route.dataspace_id),
                NativeParticipantControl {
                    proposal,
                    settlement,
                },
            );
        }
        if unavailable.is_empty() {
            Ok(controls)
        } else {
            Err(unavailable)
        }
    }

    fn prepare_native_receipt(
        &mut self,
        view: wire::View,
        candidate: CandidateDescriptor<'_>,
        coordinator_proposals: &[LaneBlockProposalV1],
        participant_controls: &NativeParticipantControlMap,
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
            let participant_control = participant_controls
                .get(&(participant.route.lane_id, participant.route.dataspace_id))?;
            let participant_proposal = &participant_control.proposal;
            let participant_descriptor = &participant_proposal.descriptor;
            let participant_is_coordinator = participant.route == plan.coordinator.route;
            let participant_entrypoint_count = participant_descriptor
                .accepted_transaction_hashes
                .iter()
                .filter(|hash| **hash == entrypoint_hash)
                .count();
            if participant_descriptor.lane_id != participant.route.lane_id
                || participant_descriptor.dataspace_id != participant.route.dataspace_id
                || participant_descriptor.lane_incarnation != participant_lane_incarnation
                || participant_descriptor.proposal_height != self.context.height
                || participant_descriptor.validator_set != validators
                || participant_descriptor.validator_count != u32::try_from(validators.len()).ok()?
                || participant_descriptor.min_quorum != u32::try_from(min_signers).ok()?
                || participant_entrypoint_count > 1
                || (participant_is_coordinator && participant_entrypoint_count != 1)
            {
                return None;
            }
            let participant_settlement = participant_control.settlement.clone();
            let participant_settlement_hash =
                iroha_data_model::nexus::compute_settlement_hash(&participant_settlement).ok()?;
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
                participant_previous_block_height: participant_descriptor
                    .previous_lane_block_height,
                participant_previous_block_descriptor_hash: participant_descriptor
                    .previous_lane_block_descriptor_hash,
                participant_lane_block_height: participant_descriptor.lane_block_height,
                participant_lane_block_view: participant_descriptor.lane_block_view,
                participant_proposal_hash: participant_proposal.proposal_hash,
                participant_settlement_commitment: Hash::from(participant_settlement_hash),
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
                participant_proposal: participant_proposal.clone(),
                participant_settlement,
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
                participant_proposal: commit_request.participant_proposal,
                participant_settlement: commit_request.participant_settlement,
                participant_settlement_hash: HashOf::from_untyped_unchecked(
                    commit_body.participant_settlement_commitment,
                ),
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

    fn exact_merge_candidate_round(
        &self,
        active_view: wire::View,
    ) -> Option<(
        u64,
        BlockHeader,
        HashOf<BlockHeader>,
        PeerId,
        HashOf<Vec<PeerId>>,
    )> {
        let parent_header = self.state.latest_block_header_fast()?;
        let parent_hash = self.state.latest_block_hash_fast()?;
        let committed_height = u64::try_from(self.state.committed_height()).ok()?;
        if parent_header.height().get() != committed_height
            || parent_header.hash() != parent_hash
            || committed_height.checked_add(1) != Some(self.context.height)
        {
            return None;
        }
        let leader = usize::try_from(self.context.leader(active_view))
            .ok()
            .and_then(|index| self.context.roster.get(index))?
            .validator
            .clone();
        let epoch_id = self
            .state
            .merge_ledger()
            .latest()
            .map_or(1, |entry| entry.epoch_id.saturating_add(1));
        Some((
            epoch_id,
            parent_header,
            parent_hash,
            leader,
            self.frozen_validator_set_hash(),
        ))
    }

    fn merge_candidate_advert_matches_round(
        advert: &MergeCandidateAdvertV1,
        sender: &PeerId,
        leader: &PeerId,
        epoch_id: u64,
        view: wire::View,
        height: u64,
        parent_hash: HashOf<BlockHeader>,
        validator_set_hash: HashOf<Vec<PeerId>>,
    ) -> bool {
        sender == leader
            && &advert.proposer == leader
            && advert.epoch_id == epoch_id
            && advert.view == view
            && advert.carrier_height == height
            && advert.parent_hash == parent_hash
            && advert.validator_set_hash == validator_set_hash
    }

    fn accept_merge_candidate(
        &mut self,
        sender: PeerId,
        message: MergeCandidateMessage,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let Some((epoch_id, parent_header, parent_hash, leader, validator_set_hash)) =
            self.exact_merge_candidate_round(active_view)
        else {
            return V2LaneIngressOutcome::Rejected;
        };
        self.merge_candidates.retain_exact_round(
            epoch_id,
            active_view,
            self.context.height,
            parent_hash,
            validator_set_hash,
        );
        let now = Instant::now();
        match message {
            MergeCandidateMessage::Advert(advert) => {
                if self.local_peer == leader {
                    return V2LaneIngressOutcome::Rejected;
                }
                if let Some((validated_digest, validated_transfer)) = self
                    .validated_merge_candidate_digests
                    .get(&(epoch_id, active_view))
                {
                    return if *validated_digest == advert.message_digest
                        && *validated_transfer == advert.transfer_id
                    {
                        V2LaneIngressOutcome::Duplicate
                    } else {
                        V2LaneIngressOutcome::Rejected
                    };
                }
                match self.merge_candidates.accept_advert(
                    &sender,
                    advert,
                    &leader,
                    &self.local_peer,
                    epoch_id,
                    active_view,
                    self.context.height,
                    parent_hash,
                    validator_set_hash,
                    now,
                ) {
                    Ok(Some(post)) => {
                        if self.push_merge_candidate_post(post) {
                            V2LaneIngressOutcome::Inserted
                        } else {
                            V2LaneIngressOutcome::Rejected
                        }
                    }
                    Ok(None) => V2LaneIngressOutcome::Duplicate,
                    Err(_) => V2LaneIngressOutcome::Rejected,
                }
            }
            MergeCandidateMessage::Request(request) => {
                if self.local_peer != leader
                    || !self.frozen_validator_set().contains(&sender)
                    || !Self::merge_candidate_advert_matches_round(
                        &request.advert,
                        &leader,
                        &leader,
                        epoch_id,
                        active_view,
                        self.context.height,
                        parent_hash,
                        validator_set_hash,
                    )
                {
                    return V2LaneIngressOutcome::Rejected;
                }
                if self
                    .merge_candidates
                    .accept_request(&sender, request, &leader, now)
                    .is_err()
                {
                    return V2LaneIngressOutcome::Rejected;
                }
                let response_slots = self.sidecar_effect_slots().min(8);
                let posts = self.merge_candidates.drain_outbound(response_slots, now);
                for post in posts {
                    if !self.push_merge_candidate_post(post) {
                        break;
                    }
                }
                V2LaneIngressOutcome::Inserted
            }
            MergeCandidateMessage::Chunk(chunk) => {
                let transfer_id = chunk.advert.transfer_id;
                let outcome = match self.merge_candidates.ingest_chunk(&sender, chunk, now) {
                    Ok(outcome) => outcome,
                    Err(_) => return V2LaneIngressOutcome::Rejected,
                };
                let CandidateChunkOutcome::Complete(completed) = outcome else {
                    return V2LaneIngressOutcome::Inserted;
                };
                let candidate =
                    match decode_merge_candidate_body(&completed.advert, &completed.bytes) {
                        Ok(candidate) => candidate,
                        Err(_) => {
                            let _ = self
                                .merge_candidates
                                .finish_completed(transfer_id, true, now);
                            return V2LaneIngressOutcome::Rejected;
                        }
                    };
                let expected_digest = crate::merge::merge_qc_message_digest(
                    &self.context.chain_id,
                    &candidate,
                    VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash,
                );
                if !Self::merge_candidate_advert_matches_round(
                    &completed.advert,
                    &sender,
                    &leader,
                    epoch_id,
                    active_view,
                    self.context.height,
                    parent_hash,
                    validator_set_hash,
                ) || completed.advert.message_digest != expected_digest
                    || candidate.epoch_id != epoch_id
                    || candidate.view != active_view
                    || candidate.carrier_height != self.context.height
                    || candidate.carrier_parent_hash != parent_hash
                    || self
                        .state
                        .validate_merge_candidate_for_global_round(
                            &candidate,
                            &parent_header,
                            active_view,
                        )
                        .is_err()
                {
                    let _ = self
                        .merge_candidates
                        .finish_completed(transfer_id, true, now);
                    return V2LaneIngressOutcome::Rejected;
                }
                let signing_context = MergeSigningContextV1 {
                    epoch_id,
                    view: active_view,
                    carrier_height: self.context.height,
                    parent_hash,
                    validator_set_hash,
                };
                match self.merge_signing_guard.authorized_digest(&signing_context) {
                    Ok(Some(authorized)) if authorized != expected_digest => {
                        let _ = self
                            .merge_candidates
                            .finish_completed(transfer_id, true, now);
                        return V2LaneIngressOutcome::Rejected;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        self.latch_signing_guard_failure(format!(
                            "merge signing decision lookup failed: {error}"
                        ));
                        return V2LaneIngressOutcome::Rejected;
                    }
                }
                let key = MergeKey {
                    epoch_id,
                    view: active_view,
                    digest: expected_digest,
                };
                if self.merge_entries.get(&key).is_some_and(|pending| {
                    !matches!(
                        &pending.stage,
                        PendingMergeStage::Collecting(existing) if existing == &candidate
                    )
                }) || (!self.merge_entries.contains_key(&key)
                    && self.merge_entries.len() >= self.limits.merge_capacity.get())
                {
                    let _ = self
                        .merge_candidates
                        .finish_completed(transfer_id, true, now);
                    return V2LaneIngressOutcome::Rejected;
                }
                let duplicate = self.merge_entries.contains_key(&key);
                self.merge_entries.entry(key).or_insert(PendingMerge {
                    stage: PendingMergeStage::Collecting(candidate),
                    signatures: BTreeMap::new(),
                });
                self.validated_merge_candidate_digests.insert(
                    (epoch_id, active_view),
                    (expected_digest, completed.advert.transfer_id),
                );
                let _ = self
                    .merge_candidates
                    .finish_completed(transfer_id, true, now);
                self.refresh_merge_candidates(active_view);
                if duplicate {
                    V2LaneIngressOutcome::Duplicate
                } else {
                    V2LaneIngressOutcome::Inserted
                }
            }
        }
    }

    fn refresh_merge_candidates(&mut self, active_view: wire::View) {
        let drain_certificate = self.drive_lane_drain(active_view, false);
        let carrier_protected = self
            .retained_merge_carrier_state
            .is_some_and(|(_, locked, decided)| locked.is_some() || decided.is_some());
        if self.globally_locked_body_hash.is_some() || carrier_protected {
            self.clear_merge_candidate_round_state();
            return;
        }
        self.merge_entries.retain(|key, _| key.view == active_view);
        self.merge_claims
            .retain(|(_, view, _), _| *view == active_view);
        self.durably_staged_merge_entries
            .retain(|key| key.view == active_view);
        self.validated_merge_candidate_digests
            .retain(|(_, view), _| *view == active_view);
        let Some((expected_epoch, parent_header, parent_hash, leader, validator_set_hash)) =
            self.exact_merge_candidate_round(active_view)
        else {
            return;
        };
        self.merge_candidates.retain_exact_round(
            expected_epoch,
            active_view,
            self.context.height,
            parent_hash,
            validator_set_hash,
        );
        let Some(local_index) = self.local_validator_index() else {
            return;
        };
        let signing_context = MergeSigningContextV1 {
            epoch_id: expected_epoch,
            view: active_view,
            carrier_height: self.context.height,
            parent_hash,
            validator_set_hash,
        };
        let authorized_digest = match self.merge_signing_guard.authorized_digest(&signing_context) {
            Ok(digest) => digest,
            Err(error) => {
                self.latch_signing_guard_failure(format!(
                    "merge signing decision lookup failed: {error}"
                ));
                return;
            }
        };
        let local_is_leader = self.local_peer == leader;
        let mut candidates_by_digest = self
            .merge_entries
            .iter()
            .filter_map(|(key, pending)| {
                let PendingMergeStage::Collecting(candidate) = &pending.stage else {
                    return None;
                };
                (candidate.epoch_id == expected_epoch
                    && candidate.view == active_view
                    && candidate.carrier_height == self.context.height
                    && candidate.carrier_parent_hash == parent_hash)
                    .then(|| (key.digest, candidate.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        let installed_preference = (candidates_by_digest.len() == 1)
            .then(|| candidates_by_digest.keys().next().copied())
            .flatten();
        let authorized_is_installed =
            authorized_digest.is_some_and(|digest| candidates_by_digest.contains_key(&digest));
        let mut leader_preference = None;
        let reusable_unguarded_candidate = authorized_digest.is_none()
            && drain_certificate.is_none()
            && installed_preference.is_some();
        if local_is_leader && !authorized_is_installed && !reusable_unguarded_candidate {
            let drain_candidate = drain_certificate.and_then(|certificate| {
                self.state
                    .merge_drain_candidate_for_next_carrier(
                        &parent_header,
                        active_view,
                        certificate,
                    )
                    .ok()
            });
            let desired_digest = authorized_digest;
            let mut needs_candidate = true;
            let mut install_candidate = |candidate: crate::merge::MergeLedgerCandidate| {
                if candidate.epoch_id != expected_epoch
                    || candidate.view != active_view
                    || candidate.carrier_height != self.context.height
                    || candidate.carrier_parent_hash != parent_hash
                    || self
                        .state
                        .validate_merge_candidate_for_global_round(
                            &candidate,
                            &parent_header,
                            active_view,
                        )
                        .is_err()
                {
                    return None;
                }
                let digest = crate::merge::merge_qc_message_digest(
                    &self.context.chain_id,
                    &candidate,
                    VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash,
                );
                candidates_by_digest.entry(digest).or_insert(candidate);
                Some(digest)
            };
            if let Some(digest) = drain_candidate.and_then(&mut install_candidate)
                && desired_digest.is_none_or(|desired| desired == digest)
            {
                leader_preference = Some(digest);
                needs_candidate = false;
            }
            if needs_candidate
                && let Some(digest) = self
                    .state
                    .merge_execution_candidate_for_next_carrier(&parent_header, active_view)
                    .and_then(&mut install_candidate)
                && desired_digest.is_none_or(|desired| desired == digest)
            {
                leader_preference = Some(digest);
                needs_candidate = false;
            }
            if needs_candidate {
                for candidate in self
                    .state
                    .merge_entry_candidates_from_lane_relays_for_view(active_view)
                {
                    let Some(digest) = install_candidate(candidate) else {
                        continue;
                    };
                    if desired_digest.is_none_or(|desired| desired == digest) {
                        leader_preference = Some(digest);
                        break;
                    }
                }
            }
        }
        let selected_digest = authorized_digest.or_else(|| {
            if local_is_leader {
                leader_preference.or(installed_preference)
            } else {
                installed_preference
            }
        });
        let Some(selected_digest) = selected_digest else {
            return;
        };
        let Some(candidate) = candidates_by_digest.get(&selected_digest).cloned() else {
            // A durable decision without its exact candidate bytes is safe but
            // unsignable. The leader retransmits the body, or the next global
            // view creates a fresh signing context after a crash.
            return;
        };
        let key = MergeKey {
            epoch_id: expected_epoch,
            view: active_view,
            digest: selected_digest,
        };
        if authorized_digest.is_none() {
            self.merge_entries
                .retain(|existing, pending| *existing == key || !pending.signatures.is_empty());
        }
        if !self.merge_entries.contains_key(&key) {
            if self.merge_entries.len() >= self.limits.merge_capacity.get() {
                return;
            }
            self.merge_entries.insert(
                key,
                PendingMerge {
                    stage: PendingMergeStage::Collecting(candidate.clone()),
                    signatures: BTreeMap::new(),
                },
            );
        }
        if candidate.execution_batch.as_ref().is_some_and(|batch| {
            !self.state.merge_application_time_is_locally_ready(
                batch.application_block_header.creation_time_ms,
                wall_clock_ms(),
            )
        }) {
            return;
        }
        if let Err(error) = self
            .merge_signing_guard
            .authorize(signing_context, key.digest)
        {
            match error {
                MergeSidecarError::LocalSigningEquivocation => {
                    iroha_logger::warn!(
                        epoch = key.epoch_id,
                        view = key.view,
                        "v2 merge signature blocked by durable anti-equivocation decision"
                    );
                }
                other => self.latch_signing_guard_failure(format!(
                    "merge signing authorization failed: {other}"
                )),
            }
            return;
        }
        if local_is_leader {
            let advert = match self.merge_candidates.publish(
                &candidate,
                self.context.height,
                parent_hash,
                validator_set_hash,
                key.digest,
                self.local_peer.clone(),
                Instant::now(),
            ) {
                Ok(advert) => advert,
                Err(error) => {
                    iroha_logger::warn!(
                        ?error,
                        epoch = key.epoch_id,
                        view = key.view,
                        "unable to publish the durably authorized v2 merge candidate"
                    );
                    return;
                }
            };
            let recipients = self
                .frozen_validator_set()
                .into_iter()
                .filter(|peer| peer != &self.local_peer)
                .collect::<Vec<_>>();
            let start = if recipients.is_empty() {
                0
            } else {
                self.merge_candidate_fanout_cursor % recipients.len()
            };
            let mut advanced = 0usize;
            for offset in 0..recipients.len() {
                let peer = recipients[(start + offset) % recipients.len()].clone();
                if !self.push_merge_candidate_post(MergeCandidatePost {
                    peer,
                    message: MergeCandidateMessage::Advert(advert.clone()),
                }) {
                    break;
                }
                advanced = advanced.saturating_add(1);
            }
            if !recipients.is_empty() {
                self.merge_candidate_fanout_cursor = (start + advanced.max(1)) % recipients.len();
            }
        }
        if self.merge_entries[&key]
            .signatures
            .contains_key(&local_index)
        {
            self.try_commit_merge(key);
            return;
        }
        let Ok(signature) = Signature::try_new(self.key_pair.private_key(), key.digest.as_ref())
        else {
            return;
        };
        let payload = signature.payload().to_vec();
        self.merge_entries
            .get_mut(&key)
            .expect("selected merge entry exists")
            .signatures
            .insert(local_index, payload.clone());
        self.merge_claims
            .insert((key.epoch_id, key.view, local_index), key.digest);
        self.push_effect(V2LaneWorkEffect::BroadcastMerge(MergeCommitteeSignature {
            epoch_id: key.epoch_id,
            view: key.view,
            signer: local_index,
            message_digest: key.digest,
            bls_sig: payload,
        }));
        self.try_commit_merge(key);
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
        let expected_digest = match &pending.stage {
            PendingMergeStage::Collecting(candidate) => crate::merge::merge_qc_message_digest(
                &self.context.chain_id,
                candidate,
                VALIDATOR_SET_HASH_VERSION_V1,
                self.frozen_validator_set_hash(),
            ),
            PendingMergeStage::Certified(entry) => entry.merge_qc.message_digest,
        };
        if expected_digest != signature.message_digest {
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
            if self.merge_entries[&key].signatures.get(&signature.signer)
                != Some(&signature.bls_sig)
            {
                return V2LaneIngressOutcome::Rejected;
            }
            // A previous quorum may have failed only because stale durable
            // sidecars occupied Kura's bounded pending store. Exact duplicate
            // delivery is a safe opportunity to retry the already-certified
            // candidate without accepting another signer claim.
            self.try_commit_merge(key);
            return V2LaneIngressOutcome::Duplicate;
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
        let cached_entry = match &pending.stage {
            PendingMergeStage::Certified(entry) => Some(entry.clone()),
            PendingMergeStage::Collecting(_) => None,
        };
        if let Some(entry) = cached_entry {
            self.persist_certified_merge_entry(key, &entry);
            return;
        }
        let Some(PendingMerge {
            stage: PendingMergeStage::Collecting(candidate),
            ..
        }) = self.merge_entries.get(&key)
        else {
            return;
        };
        let validator_set = self.frozen_validator_set();
        let validator_set_hash = HashOf::new(&validator_set);
        if crate::merge::merge_qc_message_digest(
            &self.context.chain_id,
            candidate,
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
            candidate.carrier_height,
            candidate.carrier_parent_hash,
            crate::merge::merge_chain_id_digest(&self.context.chain_id),
            VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash,
            validator_set,
            bitmap,
            signer_proofs,
            aggregate_signature,
            key.digest,
        );
        let entry = candidate.clone().into_entry(qc);
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
        let Some(pending) = self.merge_entries.get_mut(&key) else {
            return;
        };
        pending.stage = PendingMergeStage::Certified(entry.clone());
        self.persist_certified_merge_entry(key, &entry);
    }

    fn persist_certified_merge_entry(&mut self, key: MergeKey, entry: &MergeLedgerEntry) {
        if self.durably_staged_merge_entries.contains(&key) {
            return;
        }
        match self.kura.persist_pending_certified_merge_entry(entry) {
            Ok(_) => {
                // Keep the certified body and every authenticated share until
                // the carrier locks or the view retires. A peer that reaches
                // quorum before the global proposer must continue relaying its
                // local share so asymmetric loss cannot strand certification.
                self.durably_staged_merge_entries.insert(key);
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
        if !self.advance_native_view(view) {
            return Err(all_unavailable(candidates.len(), "stale Native AMX view"));
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
        let participant_controls = self
            .prepare_native_participant_controls(candidates, &lane_plan.proposals)
            .map_err(|indices| {
                CandidateWorkUnavailable::new(
                    indices,
                    "Native AMX participant lane conflicts with coordinator ownership or authority",
                )
            })?;
        if participant_controls.len() > self.limits.session_capacity.get() {
            return Err(all_unavailable(
                candidates.len(),
                "Native AMX participant proposal count exceeds the bounded session capacity",
            ));
        }

        let mut receipts = Vec::with_capacity(candidates.len());
        let mut unavailable = BTreeSet::new();
        for (index, candidate) in candidates.iter().copied().enumerate() {
            match candidate.routing_plan() {
                RoutingPlan::Single(_) => receipts.push(None),
                RoutingPlan::NativeAmx(_) => {
                    match self.prepare_native_receipt(
                        view,
                        candidate,
                        &lane_plan.proposals,
                        &participant_controls,
                    ) {
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
        V2LaneWorkEffect::PostLaneDrainVote { peer, vote } => {
            encoded.push(4);
            encoded.extend(peer.encode());
            encoded.extend(vote.encode());
        }
        V2LaneWorkEffect::BroadcastMerge(signature) => {
            encoded.push(2);
            encoded.extend(signature.encode());
        }
        V2LaneWorkEffect::PostCertifiedMergeSidecar { peer, message } => {
            encoded.push(3);
            encoded.extend(peer.encode());
            encoded.extend(message.encode());
        }
        V2LaneWorkEffect::PostMergeCandidate { peer, message } => {
            encoded.push(5);
            encoded.extend(peer.encode());
            encoded.extend(message.encode());
        }
    }
    Hash::new(encoded)
}

#[cfg(test)]
fn select_guarded_merge_key(
    authorized_digest: Option<Hash>,
    preferred_digest: Option<Hash>,
    candidates_by_digest: &BTreeMap<Hash, MergeKey>,
) -> Option<MergeKey> {
    match authorized_digest {
        Some(digest) => candidates_by_digest.get(&digest).copied(),
        None => preferred_digest.and_then(|digest| candidates_by_digest.get(&digest).copied()),
    }
}

fn wall_clock_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
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
            || ownership.lane_block_view != 0
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

fn native_signing_guard_required(voting_enabled: bool, local_peer: &PeerId) -> bool {
    voting_enabled && local_peer.public_key().try_algorithm().ok() == Some(Algorithm::BlsNormal)
}

#[cfg(test)]
mod native_signing_guard_policy_tests {
    use super::*;

    #[test]
    fn native_signing_guard_is_required_only_for_voting_bls_peers() {
        let bls = KeyPair::try_from_seed(vec![0xA1; 32], Algorithm::BlsNormal)
            .expect("derive deterministic BLS key");
        let ed25519 = KeyPair::try_from_seed(vec![0xA2; 32], Algorithm::Ed25519)
            .expect("derive deterministic Ed25519 key");
        let bls_peer = PeerId::new(bls.public_key().clone());
        let ed25519_peer = PeerId::new(ed25519.public_key().clone());

        assert!(native_signing_guard_required(true, &bls_peer));
        assert!(!native_signing_guard_required(false, &bls_peer));
        assert!(!native_signing_guard_required(true, &ed25519_peer));
        assert!(!native_signing_guard_required(false, &ed25519_peer));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        borrow::Cow,
        collections::{BTreeMap, BTreeSet},
        num::{NonZeroU64, NonZeroUsize},
        sync::Arc,
    };

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        ChainId, Level,
        account::AccountId,
        block::{
            BlockExecutionContextBundle, BlockHeader, BlockSignature, ExternalExecutionContext,
            SignedBlock,
            builder::BlockBuilder,
            consensus::{NativeAmxAttestationBodyV2, NativeAmxPhase, SumeragiLanePayloadOwnership},
            consensus_v2 as wire,
        },
        consensus::{ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus},
        isi::Log,
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
        transaction::{TransactionBuilder, TransactionEntrypoint, signed::TransactionResultInner},
        trigger::DataTriggerSequence,
    };
    use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};

    use super::*;
    use crate::{
        block::{BlockBuilder as CoreBlockBuilder, CommittedBlock, ValidBlock},
        query::store::LiveQueryStore,
        state::World,
        sumeragi::network_topology::Topology,
        tx::AcceptedTransaction,
    };

    fn fixture_with_limits(
        mode: wire::ConsensusMode,
        limits: V2LaneWorkLimits,
    ) -> Result<(V2LaneWorkAdapter, Vec<KeyPair>), V2LaneWorkError> {
        fixture_with_height_limits_and_voting(mode, 9, limits, true)
    }

    fn fixture_with_limits_and_voting(
        mode: wire::ConsensusMode,
        limits: V2LaneWorkLimits,
        voting_enabled: bool,
    ) -> Result<(V2LaneWorkAdapter, Vec<KeyPair>), V2LaneWorkError> {
        fixture_with_height_limits_and_voting(mode, 9, limits, voting_enabled)
    }

    fn fixture(mode: wire::ConsensusMode) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
        fixture_at_height(mode, 9)
    }

    fn fixture_at_height(
        mode: wire::ConsensusMode,
        height: u64,
    ) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
        let nonzero = NonZeroUsize::new(8).expect("nonzero");
        fixture_with_height_limits_and_voting(
            mode,
            height,
            V2LaneWorkLimits::new(nonzero, nonzero, nonzero, nonzero, nonzero, nonzero),
            true,
        )
        .expect("open lane adapter")
    }

    fn fixture_with_height_limits_and_voting(
        mode: wire::ConsensusMode,
        height: u64,
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
        let nexus = state.nexus_snapshot();
        let incarnations = state.lane_incarnations_snapshot();
        for entry in nexus.lane_config.entries() {
            let incarnation = incarnations.get(&entry.lane_id).copied().ok_or_else(|| {
                V2LaneWorkError::Persistence(format!(
                    "test fixture lane {} has no active incarnation",
                    entry.lane_id.as_u32(),
                ))
            })?;
            kura.install_lane_incarnation_marker_for_test(entry, incarnation, 0)
                .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        }
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
            next_epoch_snapshot: None,
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
                execution_commitment: wire::ExecutionCommitment::without_topups(
                    Hash::new(b"lane-work parent state"),
                    Hash::new(b"lane-work post state"),
                    Hash::new(b"lane-work ordinary writes"),
                ),
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
        let adapter = V2LaneWorkAdapter::new(
            context,
            local_peer,
            local_key,
            voting_enabled,
            state,
            kura,
            limits,
            None,
            None,
        )?;
        Ok((adapter, keys))
    }

    fn missing_sidecar_reference(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
        carrier_view: wire::View,
    ) -> CertifiedMergeLedgerReference {
        let validator_set = adapter
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        let mut signers_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
        let message_digest = Hash::new(b"missing v2 merge sidecar QC");
        let mut signatures = Vec::with_capacity(keys.len());
        let mut signer_proofs = Vec::with_capacity(keys.len());
        for (index, key) in keys.iter().enumerate() {
            signers_bitmap[index / 8] |= 1_u8 << (index % 8);
            signatures.push(
                Signature::try_new(key.private_key(), message_digest.as_ref())
                    .expect("sign missing-sidecar fixture digest")
                    .payload()
                    .to_vec(),
            );
            signer_proofs.push(MergeSignerProof {
                signer: u32::try_from(index).expect("fixture signer index fits u32"),
                proof_of_possession: iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture BLS proof of possession"),
            });
        }
        let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("aggregate missing-sidecar fixture signatures");
        let parent = adapter
            .context
            .parent_commit_qc
            .as_ref()
            .expect("non-genesis fixture context")
            .subject
            .block_hash;
        CertifiedMergeLedgerReference {
            version: 1,
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"missing v2 merge sidecar")),
            encoded_len: 64,
            epoch_id: 1,
            execution_batch_hash: None,
            entrypoint_count: None,
            entrypoint_merkle_root: None,
            result_merkle_root: None,
            base_state_height: None,
            base_state_hash: None,
            merge_qc: MergeQuorumCertificate {
                view: carrier_view,
                epoch_id: 1,
                carrier_height: adapter.context.height,
                carrier_parent_hash: parent,
                chain_id_digest: crate::merge::merge_chain_id_digest(&adapter.context.chain_id),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                signers_bitmap,
                signer_proofs,
                aggregate_signature,
                message_digest,
            },
        }
    }

    fn pending_sidecar_entry(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
        carrier_view: wire::View,
    ) -> MergeLedgerEntry {
        let reference = missing_sidecar_reference(adapter, keys, carrier_view);
        MergeLedgerEntry {
            epoch_id: reference.epoch_id,
            lane_catalog_hash: Hash::new(b"v2 direct-decision sidecar catalog"),
            active_lanes: Vec::new(),
            incarnation_root: Hash::new(b"v2 direct-decision sidecar incarnations"),
            activation_root: Hash::new(b"v2 direct-decision sidecar activations"),
            lane_snapshots: Vec::new(),
            execution_batch: None,
            lane_drain_certificates: Vec::new(),
            global_state_root: Hash::new(b"v2 direct-decision sidecar state"),
            merge_qc: reference.merge_qc,
        }
    }

    #[test]
    fn direct_later_view_decision_retains_earlier_view_merge_sidecar_until_finalization() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let origin_view = 1;
        let decision_view = 3;
        let entry = pending_sidecar_entry(&adapter, &keys, origin_view);
        let entry_hash = adapter
            .kura
            .persist_pending_certified_merge_entry(&entry)
            .expect("persist earlier-view merge sidecar before direct Decision recovery");
        let decided_subject = wire::BlockSubject {
            parent_block_hash: adapter
                .context
                .parent_commit_qc
                .as_ref()
                .map(|certificate| certificate.subject.block_hash),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"direct WAL Decision carrier")),
            payload_hash: Hash::new(b"direct WAL Decision carrier payload"),
        };

        // Model replay of a later-view WAL Decision received directly from a
        // CommitQC. There is deliberately no local LockAndCommit/PrepareQC.
        adapter
            .retain_merge_sidecars_for_global_view(decision_view, None, Some(decided_subject))
            .expect("a direct Decision protects its immutable carrier sidecar");
        assert_eq!(
            adapter
                .kura
                .merge_entry_by_hash(entry_hash)
                .expect("read retained direct-Decision sidecar"),
            Some(entry),
            "view pruning must retain an earlier-view sidecar needed by the decided body"
        );

        adapter
            .prune_finalized_merge_sidecars()
            .expect("finalized carrier cleanup succeeds");
        assert!(
            adapter
                .kura
                .merge_entry_by_hash(entry_hash)
                .expect("read finalized direct-Decision sidecar state")
                .is_none(),
            "terminal finalization, not the later Decision view, retires the sidecar"
        );
    }

    #[test]
    fn repeated_carrier_state_retention_scans_kura_only_on_transition() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        adapter
            .retain_merge_sidecars_for_global_view(0, None, None)
            .expect("install initial unprotected carrier state");
        assert_eq!(adapter.merge_retention_scans, 1);
        adapter
            .retain_merge_sidecars_for_global_view(0, None, None)
            .expect("repeat exact carrier state");
        assert_eq!(
            adapter.merge_retention_scans, 1,
            "an unchanged actor-loop snapshot must not rescan the bounded Kura store"
        );
        adapter
            .retain_merge_sidecars_for_global_view(1, None, None)
            .expect("install next certified view");
        assert_eq!(adapter.merge_retention_scans, 2);
    }

    #[test]
    fn direct_decision_retires_and_suppresses_merge_candidate_production() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let candidate = merge_candidate_for_persistence_retry(&adapter, 0);
        let digest = crate::merge::merge_qc_message_digest(
            &adapter.context.chain_id,
            &candidate,
            VALIDATOR_SET_HASH_VERSION_V1,
            adapter.frozen_validator_set_hash(),
        );
        let key = MergeKey {
            epoch_id: candidate.epoch_id,
            view: candidate.view,
            digest,
        };
        let decided = wire::BlockSubject {
            parent_block_hash: adapter
                .context
                .parent_commit_qc
                .as_ref()
                .map(|qc| qc.subject.block_hash),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"decided carrier")),
            payload_hash: Hash::new(b"decided carrier payload"),
        };
        adapter.merge_entries.insert(
            key,
            PendingMerge {
                stage: PendingMergeStage::Collecting(candidate.clone()),
                signatures: BTreeMap::new(),
            },
        );
        adapter
            .merge_claims
            .insert((key.epoch_id, key.view, 0), digest);
        adapter
            .retain_merge_sidecars_for_global_view(0, None, Some(decided))
            .expect("install direct Decision carrier state");
        assert!(adapter.merge_entries.is_empty());
        assert!(adapter.merge_claims.is_empty());

        adapter.merge_entries.insert(
            key,
            PendingMerge {
                stage: PendingMergeStage::Collecting(candidate),
                signatures: BTreeMap::new(),
            },
        );
        adapter.refresh_merge_candidates(0);
        assert!(
            adapter.merge_entries.is_empty(),
            "no new merge candidate may survive after a durable Decision"
        );
    }

    #[test]
    fn missing_sidecar_deferral_preserves_origin_view_and_rejects_carrier_drift() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: adapter
                .context
                .parent_commit_qc
                .as_ref()
                .map(|qc| qc.subject.block_hash),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"deferred v2 carrier")),
            payload_hash: Hash::new(b"deferred v2 carrier payload"),
        };
        let reference = missing_sidecar_reference(&adapter, &keys, 1);

        let mut wrong_height = reference.clone();
        wrong_height.merge_qc.carrier_height = round.height + 1;
        let mut wrong_parent = reference.clone();
        wrong_parent.merge_qc.carrier_parent_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"wrong carrier parent"));
        let mut future_view = reference.clone();
        future_view.merge_qc.view = round.view + 1;
        for invalid in [wrong_height, wrong_parent, future_view] {
            assert!(matches!(
                adapter
                    .defer_missing_merge_sidecar(round, subject, invalid)
                    .expect("carrier drift is a deterministic rejection"),
                MergeSidecarDeferralDisposition::Rejected(_)
            ));
        }
        assert!(adapter.drain_effects(usize::MAX).is_empty());

        assert_eq!(
            adapter
                .defer_missing_merge_sidecar(round, subject, reference)
                .expect("earlier immutable carrier view remains fetchable"),
            MergeSidecarDeferralDisposition::Fetching
        );
        assert!(matches!(
            adapter.drain_effects(1).as_slice(),
            [V2LaneWorkEffect::PostCertifiedMergeSidecar { .. }]
        ));
    }

    #[test]
    fn missing_sidecar_fetch_rejects_untrusted_rosters_caps_and_authentication() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: 3,
        };
        let subject = wire::BlockSubject {
            parent_block_hash: adapter
                .context
                .parent_commit_qc
                .as_ref()
                .map(|qc| qc.subject.block_hash),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"preflight carrier")),
            payload_hash: Hash::new(b"preflight carrier payload"),
        };
        let reference = missing_sidecar_reference(&adapter, &keys, 1);

        let attacker = KeyPair::try_from_seed(vec![0xE1; 32], Algorithm::BlsNormal)
            .expect("deterministic attacker key");
        let mut foreign_roster = reference.clone();
        foreign_roster.merge_qc.validator_set[0] = PeerId::new(attacker.public_key().clone());
        foreign_roster.merge_qc.validator_set_hash =
            HashOf::new(&foreign_roster.merge_qc.validator_set);

        let mut oversized_roster = reference.clone();
        oversized_roster.merge_qc.validator_set =
            vec![adapter.context.roster[0].validator.clone(); MAX_FETCH_MERGE_VALIDATORS + 1];

        let mut bad_signature = reference.clone();
        bad_signature.merge_qc.aggregate_signature[0] ^= 0x80;

        let mut insufficient_quorum = reference;
        insufficient_quorum.merge_qc.signers_bitmap.fill(0);
        insufficient_quorum.merge_qc.signers_bitmap[0] = 1;

        for invalid in [
            foreign_roster,
            oversized_roster,
            bad_signature,
            insufficient_quorum,
        ] {
            assert!(matches!(
                adapter
                    .defer_missing_merge_sidecar(round, subject, invalid)
                    .expect("invalid compact QC is a deterministic rejection"),
                MergeSidecarDeferralDisposition::Rejected(_)
            ));
        }
        assert!(
            adapter.drain_effects(usize::MAX).is_empty(),
            "an unauthenticated compact QC must allocate no network work"
        );
    }

    #[test]
    fn attacker_first_reference_metadata_cannot_poison_honest_fetch_registration() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: 3,
        };
        let parent_block_hash = adapter
            .context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash);
        let honest = missing_sidecar_reference(&adapter, &keys, 1);
        let mut attacker = honest.clone();
        attacker.encoded_len += 1;
        let attacker_subject = wire::BlockSubject {
            parent_block_hash,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"attacker-first carrier")),
            payload_hash: Hash::new(b"attacker-first payload"),
        };
        let honest_subject = wire::BlockSubject {
            parent_block_hash,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"honest decided carrier")),
            payload_hash: Hash::new(b"honest decided payload"),
        };

        assert_eq!(
            adapter
                .defer_missing_merge_sidecar(round, attacker_subject, attacker)
                .expect("attacker reference remains bounded and isolated"),
            MergeSidecarDeferralDisposition::Fetching
        );
        assert_eq!(
            adapter
                .defer_missing_merge_sidecar(round, honest_subject, honest)
                .expect("honest same-hash reference gets an independent session"),
            MergeSidecarDeferralDisposition::Fetching
        );
        assert_eq!(
            adapter.sidecar_effects.len(),
            2,
            "both exact reference digests must own independent bounded requests"
        );
    }

    #[test]
    fn merge_sidecar_posts_use_reserved_capacity_without_dropping_new_work() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: 3,
        };
        let parent = adapter
            .context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash);
        for index in 0..adapter.limits.relay_capacity.get() {
            let index_byte = u8::try_from(index).expect("fixture index fits u8");
            let mut reference = missing_sidecar_reference(&adapter, &keys, 1);
            reference.entry_hash = HashOf::from_untyped_unchecked(Hash::new(index.to_le_bytes()));
            let subject = wire::BlockSubject {
                parent_block_hash: parent,
                block_hash: HashOf::from_untyped_unchecked(Hash::new([0xA5, index_byte])),
                payload_hash: Hash::new([index_byte, 0x5A]),
            };
            assert_eq!(
                adapter
                    .defer_missing_merge_sidecar(round, subject, reference)
                    .expect("register within reserved sidecar effect capacity"),
                MergeSidecarDeferralDisposition::Fetching
            );
        }
        assert_eq!(
            adapter.sidecar_effects.len(),
            adapter.limits.relay_capacity.get()
        );
        let checks_before_backpressure = adapter.merge_qc_preflight_checks;

        let mut overflow = missing_sidecar_reference(&adapter, &keys, 1);
        overflow.entry_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"reserved sidecar overflow"));
        let overflow_subject = wire::BlockSubject {
            parent_block_hash: parent,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"overflow carrier")),
            payload_hash: Hash::new(b"overflow carrier payload"),
        };
        assert_eq!(
            adapter
                .defer_missing_merge_sidecar(round, overflow_subject, overflow.clone())
                .expect("capacity pressure stays retryable"),
            MergeSidecarDeferralDisposition::RetryLater
        );
        assert_eq!(
            adapter
                .defer_missing_merge_sidecar(round, overflow_subject, overflow.clone())
                .expect("repeated capacity pressure stays retryable"),
            MergeSidecarDeferralDisposition::RetryLater
        );
        assert_eq!(
            adapter.merge_qc_preflight_checks, checks_before_backpressure,
            "a full outbound queue must not repeat compact-QC cryptography"
        );

        assert_eq!(adapter.drain_effects(1).len(), 1);
        assert_eq!(
            adapter
                .defer_missing_merge_sidecar(round, overflow_subject, overflow)
                .expect("released reserved slot accepts exact retry"),
            MergeSidecarDeferralDisposition::Fetching
        );
        assert_eq!(
            adapter.sidecar_effects.len(),
            adapter.limits.relay_capacity.get()
        );
        assert_eq!(
            adapter.merge_qc_preflight_checks,
            checks_before_backpressure + 1,
            "the deferred exact reference is authenticated once progress is possible"
        );
    }

    #[test]
    fn same_qc_reference_variants_reuse_bounded_positive_authentication() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: 3,
        };
        let parent_block_hash = adapter
            .context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash);
        let reference = missing_sidecar_reference(&adapter, &keys, 1);
        let mut same_qc_variant = reference.clone();
        same_qc_variant.encoded_len += 1;
        let mut malformed_variant = reference.clone();
        malformed_variant.execution_batch_hash = Some(Hash::new(b"partial projection"));
        let first = wire::BlockSubject {
            parent_block_hash,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"cached QC first carrier")),
            payload_hash: Hash::new(b"cached QC first payload"),
        };
        let second = wire::BlockSubject {
            parent_block_hash,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"cached QC second carrier")),
            payload_hash: Hash::new(b"cached QC second payload"),
        };
        let malformed = wire::BlockSubject {
            parent_block_hash,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"cached QC malformed carrier")),
            payload_hash: Hash::new(b"cached QC malformed payload"),
        };

        assert_eq!(
            adapter
                .defer_missing_merge_sidecar(round, first, reference.clone())
                .expect("register first exact deferral"),
            MergeSidecarDeferralDisposition::Fetching
        );
        assert_eq!(adapter.merge_qc_preflight_checks, 1);
        assert_eq!(adapter.drain_effects(1).len(), 1);
        assert_eq!(
            adapter
                .defer_missing_merge_sidecar(round, second, same_qc_variant)
                .expect("register a distinct reference around the authenticated QC"),
            MergeSidecarDeferralDisposition::Fetching
        );
        assert_eq!(
            adapter.merge_qc_preflight_checks, 1,
            "unsigned reference variants around one QC must not repeat BLS verification"
        );
        assert!(matches!(
            adapter
                .defer_missing_merge_sidecar(round, malformed, malformed_variant)
                .expect("cheap reference-shape checks remain active on a cached QC"),
            MergeSidecarDeferralDisposition::Rejected(_)
        ));
        assert_eq!(adapter.merge_qc_preflight_checks, 1);
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

    fn successor_context_after_block(
        context: &wire::HeightContext,
        state: &State,
        block: &SignedBlock,
    ) -> wire::HeightContext {
        let mut successor = context.clone();
        successor.height = context.height.saturating_add(1);
        successor.parent_commit_qc = Some(wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: block.header().view_change_index(),
            },
            phase: wire::GlobalPhase::Commit,
            subject: wire::BlockSubject {
                parent_block_hash: block.header().prev_block_hash(),
                block_hash: block.hash(),
                payload_hash: Hash::new(b"lane rollover committed parent payload"),
            },
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"lane rollover parent state"),
                Hash::new(b"lane rollover post state"),
                Hash::new(b"lane rollover ordinary writes"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xA6; 48],
        });
        successor.nexus_amx_context_hash =
            super::super::v2_recovery::committed_nexus_amx_context_hash(state);
        successor.validate().expect("valid successor context");
        successor
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
                None,
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
                None,
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
            None,
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
            context, local_peer, local_key, true, state, kura, limits, None, None,
        )
        .expect("open successor-height adapter");
        assert!(
            recovered.lane_sessions.get(&session_key).is_some(),
            "successor height must retain unfinished lane consensus anchored by the prior block"
        );
    }

    #[test]
    fn canonical_lane_session_rollover_survives_fast_global_finality_and_makes_progress() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist canonical rollover anchor");
        adapter
            .lane_sessions
            .insert_proposal(proposal.clone())
            .expect("insert canonical rollover proposal");
        let min_quorum = usize::try_from(proposal.descriptor.min_quorum)
            .expect("fixture lane quorum fits usize");
        let prepare_votes = keys
            .iter()
            .take(min_quorum)
            .map(|key| signed_lane_vote(&proposal, CertPhase::Prepare, key))
            .collect::<Vec<_>>();
        for vote in &prepare_votes {
            adapter
                .lane_sessions
                .insert_vote(vote.clone(), Some(&vote.signer))
                .expect("retain remote prepare vote before rollover");
        }
        let descriptor = &proposal.descriptor;
        let session_key = crate::lane_consensus::LaneBlockSessionKey {
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            proposal_hash: proposal.proposal_hash,
        };
        let prepare_qc = adapter
            .lane_sessions
            .get(&session_key)
            .and_then(|session| session.prepare_qc.clone())
            .expect("fixture seals PrepareQC before global finality");
        let remote_vote = prepare_votes
            .iter()
            .find(|vote| vote.signer != adapter.local_peer)
            .expect("fixture includes a remote prepare voter")
            .clone();

        let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
        commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
        let successor_context =
            successor_context_after_block(&adapter.context, adapter.state.as_ref(), &block);
        let rollover = adapter.take_rollover().expect("extract canonical rollover");
        let local_peer = adapter.local_peer.clone();
        let local_key = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        let mut successor = V2LaneWorkAdapter::new(
            successor_context,
            local_peer,
            local_key,
            true,
            state,
            kura,
            limits,
            None,
            Some(rollover),
        )
        .expect("open successor with canonical lane evidence");
        let carried = successor
            .lane_sessions
            .get(&session_key)
            .expect("unfinalized canonical session survives rollover");
        assert_eq!(carried.prepare_qc.as_ref(), Some(&prepare_qc));
        assert_eq!(
            carried.prepare_votes.get(&remote_vote.signer),
            Some(&remote_vote)
        );

        for key in &keys {
            let vote = signed_lane_vote(&proposal, CertPhase::Commit, key);
            match successor
                .lane_sessions
                .insert_vote(vote.clone(), Some(&vote.signer))
            {
                Ok(
                    LaneBlockSessionInsertOutcome::Inserted
                    | LaneBlockSessionInsertOutcome::Duplicate,
                ) => {}
                Err(error) => panic!("successor commit vote must remain admissible: {error}"),
            }
        }
        successor.drive_lane_sessions();
        assert_eq!(
            successor
                .persist_anchored_sessions()
                .expect("persist carried anchored lane session"),
            1,
            "carried PrepareQC must finish and persist at the consecutive global height"
        );
        assert!(
            successor
                .kura
                .lane_block_application_receipt_available(&proposal)
        );
        let applied = successor
            .take_rollover()
            .expect("applied lane evidence is safely prunable");
        assert!(applied.lane_sessions.is_empty());
        assert_eq!(applied.lane_sessions.commit_vote_lock_len(), 0);
    }

    #[test]
    fn lane_rollover_prunes_unanchored_loser_and_inactive_incarnation_locks() {
        let (mut unanchored, keys) = fixture(wire::ConsensusMode::Permissioned);
        let losing = coordinator_proposal(&unanchored, &keys);
        unanchored
            .lane_sessions
            .insert_proposal(losing.clone())
            .expect("insert unanchored losing proposal");
        let losing_vote = signed_lane_vote(&losing, CertPhase::Prepare, &keys[0]);
        unanchored
            .lane_sessions
            .insert_vote(losing_vote.clone(), Some(&losing_vote.signer))
            .expect("insert unanchored losing vote");
        let rollover = unanchored
            .take_rollover()
            .expect("unanchored speculative evidence is prunable");
        assert!(rollover.lane_sessions.is_empty());

        let (mut inactive, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 2);
        let incarnation = inactive
            .state
            .lane_incarnation_at_height(LaneId::SINGLE, 1)
            .expect("fixture lane incarnation");
        let proposal = proposal_for_route(
            &inactive,
            &keys,
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            incarnation,
            1,
            1,
        );
        let canonical = store_canonical_anchor(&inactive, &proposal, &keys[0]);
        inactive
            .lane_sessions
            .insert_proposal(canonical.clone())
            .expect("insert canonical inactive fixture");
        inactive
            .lane_sessions
            .insert_qc(lane_qc(&canonical, &keys))
            .expect("insert canonical PrepareQC");
        let commit_vote = signed_lane_vote(&canonical, CertPhase::Commit, &keys[0]);
        inactive
            .lane_sessions
            .insert_vote(commit_vote.clone(), Some(&commit_vote.signer))
            .expect("record inactive incarnation commit lock");
        assert!(inactive.lane_sessions.commit_vote_lock_len() > 0);
        mark_lane_reset(&inactive, LaneId::SINGLE, inactive.context.height);

        let rollover = inactive
            .take_rollover()
            .expect("inactive incarnation evidence is prunable");
        assert!(rollover.lane_sessions.is_empty());
        assert_eq!(rollover.lane_sessions.commit_vote_lock_len(), 0);
    }

    #[test]
    fn lane_rollover_fails_closed_on_kura_certified_proposal_conflict() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 2);
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(LaneId::SINGLE, 1)
            .expect("fixture lane incarnation");
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            incarnation,
            1,
            1,
        );
        let canonical = store_canonical_anchor(&adapter, &proposal, &keys[0]);
        let mut conflicting = canonical.clone();
        conflicting.descriptor.subject_hash = Hash::new(b"certified rollover conflict");
        conflicting.descriptor.descriptor_hash = conflicting.descriptor.computed_descriptor_hash();
        conflicting.proposal_hash = conflicting.computed_proposal_hash();
        adapter
            .lane_sessions
            .insert_proposal(conflicting.clone())
            .expect("insert conflicting in-memory proposal");
        adapter
            .lane_sessions
            .insert_qc(lane_qc(&conflicting, &keys))
            .expect("insert conflicting certified PrepareQC");
        let before = adapter.lane_sessions.clone();

        assert!(matches!(
            adapter.take_rollover(),
            Err(V2LaneWorkError::RolloverConflict(_))
        ));
        assert_eq!(adapter.lane_sessions, before);
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
        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("defer Kura-ahead anchored lane session"),
            0,
            "a complete Kura-anchored session must defer until its global WSV commit"
        );
        assert_eq!(adapter.pending_committed_lanes.len(), 1);
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
            None,
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

    fn autonomous_prepare_qc(
        adapter: &V2LaneWorkAdapter,
        payload: &LaneExecutablePayloadV1,
        proposal: &LaneBlockProposalV1,
        keys: &[KeyPair],
    ) -> LaneBlockQcV1 {
        let validator_set = proposal.descriptor.validator_set.clone();
        let pops_by_key = lane_signer_pops(keys);
        let aligned_pops = validator_set
            .iter()
            .map(|peer| {
                pops_by_key
                    .get(peer.public_key())
                    .cloned()
                    .expect("fixture validator PoP")
            })
            .collect::<Vec<_>>();
        let availability_body = crate::lane_consensus::lane_payload_availability_body(
            payload,
            proposal,
            adapter.chain_id_hash(),
            adapter.context.epoch,
        )
        .expect("fixture availability body");
        let vote_body = proposal.vote_body(CertPhase::Prepare);
        let quorum = usize::try_from(proposal.descriptor.min_quorum).expect("fixture quorum");
        let votes = validator_set
            .iter()
            .take(quorum)
            .map(|peer| {
                let key = keys
                    .iter()
                    .find(|key| key.public_key() == peer.public_key())
                    .expect("fixture validator key");
                let availability_vote = LanePayloadAvailabilityVoteV1::new_signed(
                    availability_body.clone(),
                    peer.clone(),
                    aligned_pops.clone(),
                    key.private_key(),
                )
                .expect("fixture READY vote");
                let signature =
                    Signature::try_new(key.private_key(), &vote_body.signature_preimage())
                        .expect("fixture Prepare signature");
                LaneBlockVoteV1 {
                    body: vote_body.clone(),
                    payload_availability_vote: Some(availability_vote),
                    signer: peer.clone(),
                    bls_signature: signature.payload().to_vec(),
                }
            })
            .collect::<Vec<_>>();
        crate::lane_consensus::aggregate_lane_block_votes_to_qc(vote_body, validator_set, &votes)
            .expect("fixture autonomous Prepare QC")
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
        let entrypoint_hash = Hash::new(b"entrypoint");
        let mut source_id = [0_u8; Hash::LENGTH];
        source_id.copy_from_slice(entrypoint_hash.as_ref());
        let validators = adapter.frozen_validator_set();
        let mut body = NativeAmxAttestationBodyV2 {
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
            coordinator_lane_id: LaneId::SINGLE,
            coordinator_dataspace_id: DataSpaceId::UNIVERSAL,
            coordinator_lane_incarnation: adapter
                .state
                .lane_incarnation_at_height(LaneId::SINGLE, adapter.context.height)
                .expect("fixture single-lane incarnation"),
            participant_lane_id: LaneId::SINGLE,
            participant_dataspace_id: DataSpaceId::UNIVERSAL,
            participant_lane_incarnation: adapter
                .state
                .lane_incarnation_at_height(LaneId::SINGLE, adapter.context.height)
                .expect("fixture single-lane incarnation"),
            participant_previous_block_height: 0,
            participant_previous_block_descriptor_hash: None,
            participant_lane_block_height: 1,
            participant_lane_block_view: 0,
            participant_proposal_hash: Hash::new(b"v2-lane-work-participant-proposal"),
            participant_settlement_commitment: Hash::prehashed([0; Hash::LENGTH]),
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
        };
        body.participant_settlement_commitment = body.computed_participant_settlement_commitment();
        body
    }

    fn coordinator_proposal(adapter: &V2LaneWorkAdapter, keys: &[KeyPair]) -> LaneBlockProposalV1 {
        let validator_set = keys
            .iter()
            .map(|key| PeerId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            lane_incarnation: adapter
                .state
                .lane_incarnation_at_height(LaneId::SINGLE, adapter.context.height)
                .expect("fixture single-lane incarnation"),
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
        let mut block: SignedBlock = CoreBlockBuilder::new(vec![accepted])
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

        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("persist anchored lane session"),
            1
        );
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
            None,
            None,
        )
        .expect("restart lane adapter");
        assert!(
            restarted.lane_sessions.status_snapshot().is_empty(),
            "durably applied canonical work must not rehydrate after restart"
        );

        restarted.pending_committed_lanes.push_back(session);
        assert_eq!(
            restarted
                .persist_anchored_sessions()
                .expect("persist idempotent anchored lane session"),
            1
        );
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
            None,
            None,
        )
        .expect("restart lane adapter after partial receipt persistence");
        assert_eq!(
            restarted.pending_committed_lanes.len(),
            1,
            "restart must re-queue the exact certified session whose receipt is absent"
        );
        assert_eq!(
            restarted
                .persist_anchored_sessions()
                .expect("retry canonical receipt persistence after restart"),
            1
        );
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
        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("defer sessions without a canonical anchor"),
            0
        );
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
        body.participant_previous_block_height =
            coordinator_proposal.descriptor.previous_lane_block_height;
        body.participant_previous_block_descriptor_hash = coordinator_proposal
            .descriptor
            .previous_lane_block_descriptor_hash;
        body.participant_lane_block_height = coordinator_proposal.descriptor.lane_block_height;
        body.participant_lane_block_view = coordinator_proposal.descriptor.lane_block_view;
        body.participant_proposal_hash = coordinator_proposal.proposal_hash;
        let participant_proposal = coordinator_proposal.clone();
        let participant_settlement = body.computed_participant_settlement();
        body.participant_settlement_commitment = body.computed_participant_settlement_commitment();
        NativeAmxAttestationRequestV2 {
            body,
            plan_legs: plan.legs(),
            coordinator_proposal,
            participant_proposal,
            participant_settlement,
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
        if request.body.participant_lane_id == request.body.coordinator_lane_id
            && request.body.participant_dataspace_id == request.body.coordinator_dataspace_id
        {
            request.participant_proposal = request.coordinator_proposal.clone();
            let descriptor = &request.participant_proposal.descriptor;
            request.body.participant_lane_incarnation = descriptor.lane_incarnation;
            request.body.participant_previous_block_height = descriptor.previous_lane_block_height;
            request.body.participant_previous_block_descriptor_hash =
                descriptor.previous_lane_block_descriptor_hash;
            request.body.participant_lane_block_height = descriptor.lane_block_height;
            request.body.participant_lane_block_view = descriptor.lane_block_view;
            request.body.participant_proposal_hash = request.participant_proposal.proposal_hash;
            request.participant_settlement = request.body.computed_participant_settlement();
            request.body.participant_settlement_commitment =
                request.body.computed_participant_settlement_commitment();
        }
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
        assert_eq!(
            at_limit
                .native_signing_capacity()
                .expect("exact hard limit is valid"),
            hard
        );
        let (adapter, _) = fixture_with_limits(wire::ConsensusMode::Permissioned, at_limit)
            .expect("the exact durable journal hard limit is accepted");
        drop(adapter);

        let over =
            NonZeroUsize::new(crate::native_amx::MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD + 1)
                .expect("hard limit successor is nonzero");
        let over_limit = V2LaneWorkLimits::new(over, one, one, one, one, over);
        assert_eq!(
            over_limit
                .native_signing_capacity()
                .expect("logical capacity is clamped to the hard limit"),
            hard
        );
        let (adapter, _) = fixture_with_limits(wire::ConsensusMode::Permissioned, over_limit)
            .expect("a valid logical capacity is clamped at the durable hard limit");
        drop(adapter);

        let request_bound = NonZeroUsize::new(17).expect("nonzero request bound");
        let request_limited = V2LaneWorkLimits::new(hard, one, one, one, one, request_bound);
        assert_eq!(
            request_limited
                .native_signing_capacity()
                .expect("authenticated request bound is valid"),
            request_bound,
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
        assert_eq!(
            limits
                .native_signing_capacity()
                .expect("production native signing capacity is valid"),
            control
        );

        let (validator, _) = fixture_with_limits(wire::ConsensusMode::Permissioned, limits)
            .expect("production defaults construct a voting adapter");
        assert!(validator.voting_enabled);
        assert!(validator.native_signing_guard.is_some());
        drop(validator);

        let (observer, _) =
            fixture_with_limits_and_voting(wire::ConsensusMode::Permissioned, limits, false)
                .expect("production defaults construct an observer adapter");
        assert!(!observer.voting_enabled);
        assert!(observer.native_signing_guard.is_none());
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
        let durable_slot_key = NativeVoteSlotKey::from_body(&request.body, &vote.signer);
        let durable_slot_claim = NativeVoteSlotClaim::from_body(&request.body);
        adapter
            .native_slot_claims
            .insert(durable_slot_key, durable_slot_claim);
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
        assert_eq!(adapter.native_slot_claims.len(), 1);
        let mut conflicting_slot = request.body;
        conflicting_slot.round.view = 1;
        conflicting_slot.participant_proposal_hash =
            Hash::new(b"conflicting participant proposal after global view change");
        assert!(adapter.native_vote_slot_conflicts(
            NativeVoteSlotKey::from_body(&conflicting_slot, &vote.signer),
            NativeVoteSlotClaim::from_body(&conflicting_slot),
        ));
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
        let mut restarted = V2LaneWorkAdapter::new(
            context, local_peer, key_pair, true, state, kura, limits, None, None,
        )
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

        let mut restarted = V2LaneWorkAdapter::new(
            context, local_peer, key_pair, true, state, kura, limits, None, None,
        )
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
    fn v2_commit_signing_guard_survives_restart_and_drain_closes_lane() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height)
            .expect("active lane incarnation");
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            lane_id,
            dataspace_id,
            incarnation,
            adapter.context.height,
            1,
        );
        let commit_body = proposal.vote_body(CertPhase::Commit);
        assert!(adapter.sign_lane_vote(commit_body.clone()).is_some());
        assert!(
            adapter.sign_lane_vote(commit_body.clone()).is_some(),
            "the exact durable Commit decision remains idempotently signable"
        );

        let validator_set = proposal.descriptor.validator_set.clone();
        let drain_body = iroha_data_model::merge::LaneDrainCertificateBodyV1 {
            version: 1,
            intent: iroha_data_model::merge::LaneDrainIntentV1 {
                version: 1,
                chain_id_digest: crate::merge::merge_chain_id_digest(&adapter.context.chain_id),
                lane_id,
                dataspace_id,
                lane_incarnation: incarnation,
                close_global_height: adapter.context.height,
                initial_merged_lane_height: 0,
                initial_merged_descriptor_hash: None,
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set,
                validator_count: proposal.descriptor.validator_count,
                min_quorum: proposal.descriptor.min_quorum,
            },
            final_lane_block_height: proposal.descriptor.lane_block_height,
            final_lane_block_descriptor_hash: Some(proposal.descriptor.descriptor_hash),
        };
        adapter
            .lane_drain_signing_guard
            .authorize_drain(&drain_body)
            .expect("the drain covers the durable Commit high-water");
        assert!(
            adapter.sign_lane_vote(commit_body.clone()).is_none(),
            "a durable drain decision permanently closes later Commit signing"
        );
        assert!(adapter.ensure_healthy().is_ok());

        let context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let key_pair = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        let mut restarted = V2LaneWorkAdapter::new(
            context, local_peer, key_pair, true, state, kura, limits, None, None,
        )
        .expect("reopen V2 lane adapter with the durable drain journal");
        assert!(restarted.sign_lane_vote(commit_body).is_none());
        assert!(restarted.ensure_healthy().is_ok());
    }

    #[test]
    fn durable_merge_choice_overrides_later_drain_priority() {
        let drain_digest = Hash::new(b"preferred drain candidate");
        let relay_digest = Hash::new(b"previously authorized relay candidate");
        let drain = MergeKey {
            epoch_id: 7,
            view: 2,
            digest: drain_digest,
        };
        let relay = MergeKey {
            epoch_id: 7,
            view: 2,
            digest: relay_digest,
        };
        let candidates = BTreeMap::from([(drain_digest, drain), (relay_digest, relay)]);

        assert_eq!(
            select_guarded_merge_key(None, Some(drain_digest), &candidates),
            Some(drain),
            "an uncommitted carrier deterministically prioritizes the drain"
        );
        assert_eq!(
            select_guarded_merge_key(Some(relay_digest), Some(drain_digest), &candidates),
            Some(relay),
            "a durable prior decision must win over a newly available drain"
        );
        assert_eq!(
            select_guarded_merge_key(
                Some(Hash::new(b"authorized body not locally reconstructed")),
                Some(drain_digest),
                &candidates,
            ),
            None,
            "missing authorized bytes cause abstention, never a conflicting signature"
        );
    }

    #[test]
    fn completed_merge_candidate_round_rejects_sequential_leader_equivocation_and_replay() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let view = 0;
        let (epoch_id, _, parent_hash, leader, validator_set_hash) = adapter
            .exact_merge_candidate_round(view)
            .expect("fixture has an exact merge round");
        let follower_key = keys
            .iter()
            .find(|key| key.public_key() != leader.public_key())
            .expect("fixture has a follower")
            .clone();
        adapter.local_peer = PeerId::new(follower_key.public_key().clone());
        adapter.key_pair = follower_key;

        let accepted_digest = Hash::new(b"first validated merge candidate");
        let accepted = MergeCandidateAdvertV1::new(
            epoch_id,
            view,
            adapter.context.height,
            parent_hash,
            validator_set_hash,
            accepted_digest,
            Hash::new(b"first validated merge candidate bytes"),
            128,
            leader.clone(),
        );
        adapter
            .validated_merge_candidate_digests
            .insert((epoch_id, view), (accepted_digest, accepted.transfer_id));

        assert_eq!(
            adapter.accept_merge_candidate(
                leader.clone(),
                MergeCandidateMessage::Advert(accepted),
                view,
            ),
            V2LaneIngressOutcome::Duplicate,
            "an exact completed advert replay must not trigger another body request"
        );
        let conflicting = MergeCandidateAdvertV1::new(
            epoch_id,
            view,
            adapter.context.height,
            parent_hash,
            validator_set_hash,
            Hash::new(b"second merge candidate digest"),
            Hash::new(b"second merge candidate bytes"),
            128,
            leader.clone(),
        );
        assert_eq!(
            adapter.accept_merge_candidate(
                leader,
                MergeCandidateMessage::Advert(conflicting),
                view,
            ),
            V2LaneIngressOutcome::Rejected,
            "a leader cannot reopen a completed exact round with different candidate bytes"
        );
        assert!(adapter.sidecar_effects.is_empty());
    }

    #[test]
    fn certified_view_transition_immediately_purges_queued_merge_traffic() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let (_, _, parent_hash, leader, validator_set_hash) = adapter
            .exact_merge_candidate_round(0)
            .expect("fixture has an exact merge round");
        let signature = MergeCommitteeSignature {
            epoch_id: 1,
            view: 0,
            signer: 0,
            message_digest: Hash::new(b"retired merge signature"),
            bls_sig: vec![0xA5; 96],
        };
        assert!(adapter.push_effect(V2LaneWorkEffect::BroadcastMerge(signature)));
        let advert = MergeCandidateAdvertV1::new(
            1,
            0,
            adapter.context.height,
            parent_hash,
            validator_set_hash,
            Hash::new(b"retired candidate digest"),
            Hash::new(b"retired candidate bytes"),
            128,
            leader.clone(),
        );
        let destination = adapter
            .frozen_validator_set()
            .into_iter()
            .find(|peer| peer != &adapter.local_peer)
            .expect("fixture has a merge follower");
        assert!(adapter.push_merge_candidate_post(MergeCandidatePost {
            peer: destination,
            message: MergeCandidateMessage::Advert(advert),
        }));

        adapter
            .retain_merge_sidecars_for_global_view(1, None, None)
            .expect("install certified next view");
        let effects = adapter.drain_effects(usize::MAX);
        assert!(effects.iter().all(|effect| !matches!(
            effect,
            V2LaneWorkEffect::BroadcastMerge(_) | V2LaneWorkEffect::PostMergeCandidate { .. }
        )));
        assert!(adapter.validated_merge_candidate_digests.is_empty());
    }

    #[test]
    fn autonomous_prepare_qc_write_failure_blocks_commit_and_retries_exactly() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter.globally_locked_body_hash = Some(block.hash());
        adapter.globally_locked_body = Some(block.clone());
        let producer_key = keys
            .iter()
            .find(|key| {
                proposal
                    .descriptor
                    .validator_set
                    .iter()
                    .any(|peer| peer.public_key() == key.public_key())
            })
            .expect("fixture lane producer key");
        let producer = PeerId::new(producer_key.public_key().clone());
        let payload = LaneExecutablePayloadV1::new_signed(
            adapter.chain_id_hash(),
            adapter.context.epoch,
            proposal.clone(),
            block.external_entrypoints_cloned().collect(),
            producer.clone(),
            producer_key.private_key(),
        )
        .expect("construct exact executable payload");
        assert_ne!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneExecutablePayload(payload.clone()),
                    Some(producer),
                ),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
        );
        adapter.drain_effects(usize::MAX);
        let prepare_qc = autonomous_prepare_qc(&adapter, &payload, &proposal, &keys);

        adapter
            .kura
            .fail_next_autonomous_lane_view_state_write_for_tests();
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(BlockMessage::LaneBlockQc(prepare_qc.clone()), None),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
            "PrepareQC ingress must fail closed when READY durability fails",
        );
        assert!(
            adapter
                .kura
                .read_autonomous_lane_block_artifact(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                    adapter.chain_id_hash(),
                    adapter.context.epoch,
                )
                .is_some_and(|artifact| artifact.availability_certificate.is_none()),
        );
        assert!(adapter.lane_sessions.commit_vote_lock_slots().is_empty());
        assert!(adapter.drain_effects(usize::MAX).iter().all(|effect| {
            !matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockVote(vote),
                    ..
                } if vote.body.phase == CertPhase::Commit
            )
        }));

        assert_ne!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(BlockMessage::LaneBlockQc(prepare_qc), None),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
            "the exact QC must remain retryable after the injected disk fault",
        );
        assert!(adapter.kura.autonomous_lane_payload_availability_delivered(
            &proposal,
            adapter.chain_id_hash(),
            adapter.context.epoch,
        ),);
        assert_eq!(adapter.lane_sessions.commit_vote_lock_slots().len(), 1);
        assert!(adapter.drain_effects(usize::MAX).iter().any(|effect| {
            matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockVote(vote),
                    ..
                } if vote.body.phase == CertPhase::Commit
            )
        }));
    }

    #[test]
    fn restart_hydrates_durable_autonomous_origin_without_network_replay() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist canonical global anchor before simulated crash");
        adapter.globally_locked_body_hash = Some(block.hash());
        adapter.globally_locked_body = Some(block.clone());
        let producer_key = keys
            .iter()
            .find(|key| {
                proposal
                    .descriptor
                    .validator_set
                    .iter()
                    .any(|peer| peer.public_key() == key.public_key())
            })
            .expect("fixture lane producer key");
        let producer = PeerId::new(producer_key.public_key().clone());
        let payload = LaneExecutablePayloadV1::new_signed(
            adapter.chain_id_hash(),
            adapter.context.epoch,
            proposal.clone(),
            block.external_entrypoints_cloned().collect(),
            producer.clone(),
            producer_key.private_key(),
        )
        .expect("construct exact executable payload");
        assert_ne!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneExecutablePayload(payload),
                    Some(producer),
                ),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
        );
        assert!(
            adapter
                .kura
                .autonomous_lane_certification_payload(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                    adapter.chain_id_hash(),
                    adapter.context.epoch,
                )
                .is_some(),
        );

        let context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let local_key = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        let restarted = V2LaneWorkAdapter::new(
            context,
            local_peer.clone(),
            local_key,
            true,
            state,
            kura,
            limits,
            None,
            None,
        )
        .expect("restart hydrates durable autonomous payload without inbound messages");
        assert!(
            restarted
                .lane_sessions
                .proposals_without_commit_qc()
                .iter()
                .any(|cached| cached == &proposal),
        );
        assert!(
            restarted
                .lane_sessions
                .local_vote_rebroadcast_artifacts_for(&local_peer)
                .iter()
                .any(|(cached, vote)| {
                    cached == &proposal && vote.body.phase == CertPhase::Prepare
                }),
            "constructor redrive must recreate the missing local Prepare vote from durable bytes",
        );
    }

    #[test]
    fn raw_autonomous_payload_never_persists_on_a_non_committee_node() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter.globally_locked_body_hash = Some(block.hash());
        adapter.globally_locked_body = Some(block.clone());
        let producer_key = keys
            .iter()
            .find(|key| {
                proposal
                    .descriptor
                    .validator_set
                    .iter()
                    .any(|peer| peer.public_key() == key.public_key())
            })
            .expect("fixture lane producer key");
        let producer = PeerId::new(producer_key.public_key().clone());
        let payload = LaneExecutablePayloadV1::new_signed(
            adapter.chain_id_hash(),
            adapter.context.epoch,
            proposal.clone(),
            block.external_entrypoints_cloned().collect(),
            producer.clone(),
            producer_key.private_key(),
        )
        .expect("construct committee-authenticated executable payload");
        let outsider_key =
            KeyPair::try_from_seed(vec![0xD9; 32], Algorithm::BlsNormal).expect("outsider key");
        adapter.local_peer = PeerId::new(outsider_key.public_key().clone());
        adapter.key_pair = outsider_key;

        assert_eq!(
            adapter.accept_lane_executable_payload(payload, Some(&producer), 0),
            V2LaneIngressOutcome::Rejected,
            "a valid remote signature must not allocate raw Kura state outside the committee",
        );
        assert!(
            adapter
                .kura
                .read_autonomous_lane_block_artifact(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                    adapter.chain_id_hash(),
                    adapter.context.epoch,
                )
                .is_none(),
        );
        assert!(!adapter.lane_sessions.contains_proposal(&proposal));
    }

    #[test]
    fn applied_autonomous_origin_replay_is_rejected_without_redriving_votes() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist canonical global block");
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
            .expect("persist exact certified lane origin");
        adapter
            .kura
            .persist_lane_block_application_receipt(&proposal)
            .expect("publish exact applied receipt");
        assert!(adapter.autonomous_origin_is_applied_or_snapshot_anchored(&proposal),);

        adapter.globally_locked_body_hash = Some(block.hash());
        adapter.globally_locked_body = Some(block.clone());
        let producer_key = keys
            .iter()
            .find(|key| {
                proposal
                    .descriptor
                    .validator_set
                    .iter()
                    .any(|peer| peer.public_key() == key.public_key())
            })
            .expect("fixture lane producer key");
        let producer = PeerId::new(producer_key.public_key().clone());
        let payload = LaneExecutablePayloadV1::new_signed(
            adapter.chain_id_hash(),
            adapter.context.epoch,
            proposal.clone(),
            block.external_entrypoints_cloned().collect(),
            producer.clone(),
            producer_key.private_key(),
        )
        .expect("construct replayed executable payload");
        adapter.drain_effects(usize::MAX);

        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneExecutablePayload(payload),
                    Some(producer),
                ),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
            "an applied origin must not return a drive-triggering ingress outcome",
        );
        assert!(adapter.drain_effects(usize::MAX).is_empty());
        assert!(!adapter.lane_sessions.contains_proposal(&proposal));
        assert!(
            adapter
                .kura
                .read_autonomous_lane_block_artifact(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                    adapter.chain_id_hash(),
                    adapter.context.epoch,
                )
                .is_none(),
            "a finalized proposal must be rejected before raw payload persistence",
        );
    }

    #[test]
    fn locked_body_rehydrates_payload_written_before_restart_without_network_replay() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        adapter.globally_locked_body_hash = Some(block.hash());
        adapter.globally_locked_body = Some(block.clone());
        let payload = LaneExecutablePayloadV1::new_signed(
            adapter.chain_id_hash(),
            adapter.context.epoch,
            proposal.clone(),
            block.external_entrypoints_cloned().collect(),
            adapter.local_peer.clone(),
            adapter.key_pair.private_key(),
        )
        .expect("construct exact payload before simulated crash");
        let local_peer = adapter.local_peer.clone();
        assert_ne!(
            adapter.accept_lane_executable_payload(payload, Some(&local_peer), 0),
            V2LaneIngressOutcome::Rejected,
        );

        let context = adapter.context.clone();
        let local_key = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        let mut restarted = V2LaneWorkAdapter::new(
            context,
            local_peer.clone(),
            local_key,
            true,
            state,
            kura,
            limits,
            None,
            None,
        )
        .expect("restart before the global body is locally rebound");
        assert!(
            !restarted.lane_sessions.contains_proposal(&proposal),
            "an unprotected Kura payload must remain dormant until the exact body locks",
        );
        assert!(restarted.mark_global_body_locked(block.hash()));
        assert_ne!(
            restarted.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected,
            "binding the exact protected body must authorize the already-durable payload",
        );
        assert!(restarted.lane_sessions.contains_proposal(&proposal));
        assert!(
            restarted
                .lane_sessions
                .local_vote_rebroadcast_artifacts_for(&local_peer)
                .iter()
                .any(|(cached, vote)| {
                    cached == &proposal && vote.body.phase == CertPhase::Prepare
                }),
            "locked-body binding must redrive a local Prepare vote without inbound payload replay",
        );
    }

    #[test]
    fn v2_payload_and_new_view_ingress_advance_only_the_durable_transport_cursor() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter.globally_locked_body_hash = Some(block.hash());
        adapter.globally_locked_body = Some(block.clone());
        let entrypoints = block.external_entrypoints_cloned().collect::<Vec<_>>();
        let lane_leader = lane_block_redrive_leader(&proposal, 0)
            .expect("fixture lane leader")
            .clone();
        let lane_key = keys
            .iter()
            .find(|key| key.public_key() == lane_leader.public_key())
            .expect("fixture contains lane leader key");
        let payload = LaneExecutablePayloadV1::new_signed(
            adapter.chain_id_hash(),
            adapter.context.epoch,
            proposal.clone(),
            entrypoints,
            lane_leader.clone(),
            lane_key.private_key(),
        )
        .expect("construct exact executable payload");
        let outsider_key =
            KeyPair::try_from_seed(vec![0xEF; 32], Algorithm::BlsNormal).expect("outsider key");
        let outsider = PeerId::new(outsider_key.public_key().clone());

        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneExecutablePayload(payload.clone()),
                    Some(outsider.clone()),
                ),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
            "an unauthenticated payload sender must not allocate durable state"
        );
        assert!(
            adapter
                .kura
                .read_autonomous_lane_block_artifact(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                    adapter.chain_id_hash(),
                    adapter.context.epoch,
                )
                .is_none()
        );
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneExecutablePayload(payload.clone()),
                    Some(lane_leader.clone()),
                ),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneExecutablePayload(payload.clone()),
                    Some(lane_leader),
                ),
                0,
            ),
            V2LaneIngressOutcome::Duplicate,
            "exact payload replay is idempotent"
        );
        let alternate_key = keys
            .iter()
            .find(|key| key.public_key() != payload.producer.public_key())
            .expect("fixture has another lane committee member");
        let alternate_producer = PeerId::new(alternate_key.public_key().clone());
        let alternate_payload = LaneExecutablePayloadV1::new_signed(
            adapter.chain_id_hash(),
            adapter.context.epoch,
            proposal.clone(),
            payload.entrypoints.clone(),
            alternate_producer.clone(),
            alternate_key.private_key(),
        )
        .expect("another exact committee member can authenticate the same payload");
        assert_eq!(alternate_payload.payload_hash, payload.payload_hash);
        assert_eq!(
            Kura::autonomous_lane_block_execution_input_candidate(
                &payload,
                adapter.chain_id_hash(),
                adapter.context.epoch,
            )
            .expect("canonical producer execution input"),
            Kura::autonomous_lane_block_execution_input_candidate(
                &alternate_payload,
                adapter.chain_id_hash(),
                adapter.context.epoch,
            )
            .expect("failover producer execution input"),
            "producer signatures must not change deterministic execution bytes",
        );
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneExecutablePayload(alternate_payload),
                    Some(alternate_producer),
                ),
                0,
            ),
            V2LaneIngressOutcome::Duplicate,
            "committee failover may change only the producer signature, never the payload body"
        );

        let (_, current) = adapter
            .kura
            .current_autonomous_lane_payload(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
                adapter.chain_id_hash(),
                adapter.context.epoch,
            )
            .expect("payload is durable before NewView voting");
        let body = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
            &current,
            &payload,
            1,
            adapter.chain_id_hash(),
            adapter.context.epoch,
        )
        .expect("one-step NewView body");
        let first_vote = LaneBlockNewViewVoteV1::new_signed(
            body.clone(),
            PeerId::new(keys[0].public_key().clone()),
            keys[0].private_key(),
        )
        .expect("valid first NewView vote");
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockNewViewVote(first_vote.clone()),
                    Some(outsider),
                ),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
            "the authenticated outer sender must equal the declared signer"
        );
        let quorum = usize::try_from(body.min_quorum).expect("quorum fits usize");
        for (index, key) in keys.iter().take(quorum).enumerate() {
            let signer = PeerId::new(key.public_key().clone());
            let vote =
                LaneBlockNewViewVoteV1::new_signed(body.clone(), signer.clone(), key.private_key())
                    .expect("valid quorum NewView vote");
            if index + 1 == quorum {
                adapter
                    .kura
                    .fail_next_autonomous_lane_view_state_write_for_tests();
            }
            let outcome = adapter.accept_lane_message(
                InboundBlockMessage::new(BlockMessage::LaneBlockNewViewVote(vote), Some(signer)),
                0,
            );
            if index + 1 == quorum {
                assert_eq!(
                    outcome,
                    V2LaneIngressOutcome::Rejected,
                    "a failed durable cursor write must fail the sealing ingress closed",
                );
            } else {
                assert_ne!(outcome, V2LaneIngressOutcome::Rejected);
            }
        }
        let (_, not_advanced) = adapter
            .kura
            .current_autonomous_lane_payload(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
                adapter.chain_id_hash(),
                adapter.context.epoch,
            )
            .expect("origin cursor remains readable after failed write");
        assert_eq!(not_advanced.descriptor.lane_block_view, 0);
        adapter.drive_lane_new_views(0);
        let (_, advanced) = adapter
            .kura
            .current_autonomous_lane_payload(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
                adapter.chain_id_hash(),
                adapter.context.epoch,
            )
            .expect("NewView certificate remains restart-verifiable in Kura");
        assert_eq!(advanced.descriptor.lane_block_view, 1);
        assert_eq!(
            advanced.descriptor.accepted_transaction_hashes,
            proposal.descriptor.accepted_transaction_hashes
        );
        let (_, certification_proposal) = adapter
            .kura
            .autonomous_lane_certification_payload(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
                adapter.chain_id_hash(),
                adapter.context.epoch,
            )
            .expect("immutable certification subject remains durable");
        assert_eq!(certification_proposal, proposal);
        adapter
            .kura
            .recover_autonomous_lane_block_payload(
                &proposal,
                adapter.chain_id_hash(),
                adapter.context.epoch,
            )
            .expect("cursor advancement must not retarget canonical execution recovery");
        assert!(
            adapter
                .lane_sessions
                .proposals_without_commit_qc()
                .iter()
                .any(|cached| cached == &proposal),
            "NewView must redrive the origin session instead of creating a second QC subject",
        );
        assert!(
            adapter
                .lane_sessions
                .proposals_without_commit_qc()
                .iter()
                .all(|cached| cached.descriptor.lane_block_view == 0),
        );
    }

    #[test]
    fn hinted_payload_rejects_uncommitted_routing_metadata_before_first_write() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter.globally_locked_body_hash = Some(block.hash());
        adapter.globally_locked_body = Some(block.clone());
        let entrypoints = block.external_entrypoints_cloned().collect::<Vec<_>>();
        let [entrypoint] = entrypoints.as_slice() else {
            panic!("fixture must carry one externally executed transaction")
        };
        let producer_key = keys
            .iter()
            .find(|key| {
                proposal
                    .descriptor
                    .validator_set
                    .contains(&PeerId::new(key.public_key().clone()))
            })
            .expect("fixture lane committee has a signing key");
        let producer = PeerId::new(producer_key.public_key().clone());
        let routing_plan = RoutingPlan::single(RoutingDecision::new(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
        ));
        let accepted =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(entrypoint.clone()));
        let reservation = crate::queue::LaneQueueReservationKeyV1 {
            signed_transaction_hash: accepted.hash(),
            entrypoint_hash: entrypoint.hash(),
            routing_plan_digest: routing_plan.digest(),
            coordinator_leg: routing_plan.coordinator_leg(),
            lane_id: proposal.descriptor.lane_id,
            dataspace_id: proposal.descriptor.dataspace_id,
            lane_incarnation: proposal.descriptor.lane_incarnation,
            proposal_height: proposal.descriptor.proposal_height,
            lane_block_height: proposal.descriptor.lane_block_height,
            lane_block_view: proposal.descriptor.lane_block_view,
            reservation_owner_hash: Hash::new(b"adversarial-hinted-reservation-owner"),
            proposal_identity_hash: proposal.proposal_hash,
        };
        let adversarial = LaneExecutablePayloadV1::new_signed_with_reservations(
            adapter.chain_id_hash(),
            adapter.context.epoch,
            proposal.clone(),
            entrypoints.clone(),
            vec![reservation],
            vec![routing_plan],
            vec![None],
            producer.clone(),
            producer_key.private_key(),
        )
        .expect("producer-signed hinted metadata is structurally valid");
        assert_eq!(
            adapter.accept_lane_executable_payload(adversarial, Some(&producer), 0),
            V2LaneIngressOutcome::Rejected,
            "uncommitted producer routing metadata must not own the durable lane slot"
        );
        assert!(
            adapter
                .kura
                .read_autonomous_lane_block_artifact(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                    adapter.chain_id_hash(),
                    adapter.context.epoch,
                )
                .is_none()
        );

        let canonical = LaneExecutablePayloadV1::new_signed(
            adapter.chain_id_hash(),
            adapter.context.epoch,
            proposal.clone(),
            entrypoints,
            producer.clone(),
            producer_key.private_key(),
        )
        .expect("canonical hinted payload");
        assert_ne!(
            adapter.accept_lane_executable_payload(canonical, Some(&producer), 0),
            V2LaneIngressOutcome::Rejected,
            "the exact globally reconstructible payload must remain admissible"
        );
        assert!(adapter.autonomous_payload_for_proposal(&proposal).is_some());
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
    fn certified_merge_sidecar_effect_dedup_is_destination_and_payload_bound() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let responder = adapter.context.roster[0].validator.clone();
        let alternate_destination = adapter.context.roster[1].validator.clone();
        let request = crate::merge_sidecar::CertifiedMergeSidecarRequestV1 {
            version: crate::merge_sidecar::CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            request_id: Hash::new(b"v2-lane-work-sidecar-request"),
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"v2-lane-work-sidecar-entry")),
            encoded_len: 128,
            epoch_id: 4,
            reference_digest: Hash::new(b"v2-lane-work-sidecar-reference"),
            requester: adapter.local_peer.clone(),
            responder: responder.clone(),
        };
        let effect = V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: responder,
            message: CertifiedMergeSidecarMessage::Request(request.clone()),
        };

        assert!(adapter.push_effect(effect.clone()));
        assert!(adapter.push_effect(effect.clone()));
        assert_eq!(adapter.effects.len(), 1, "an exact retry is deduplicated");

        assert!(
            adapter.push_effect(V2LaneWorkEffect::PostCertifiedMergeSidecar {
                peer: alternate_destination,
                message: CertifiedMergeSidecarMessage::Request(request.clone()),
            })
        );
        assert_eq!(
            adapter.effects.len(),
            2,
            "the authenticated destination is part of the effect identity"
        );

        let mut distinct_request = request;
        distinct_request.request_id = Hash::new(b"v2-lane-work-sidecar-request-2");
        assert!(
            adapter.push_effect(V2LaneWorkEffect::PostCertifiedMergeSidecar {
                peer: distinct_request.responder.clone(),
                message: CertifiedMergeSidecarMessage::Request(distinct_request),
            })
        );
        assert_eq!(
            adapter.effects.len(),
            3,
            "the bounded sidecar payload is part of the effect identity"
        );

        assert_eq!(adapter.drain_effects(usize::MAX).len(), 3);
        assert!(adapter.push_effect(effect));
        assert_eq!(
            adapter.effects.len(),
            1,
            "a drained sidecar transport may be retried"
        );
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
            adapter.bind_local_candidate(round_zero, &block_zero),
            V2LaneIngressOutcome::Inserted
        );
        adapter.schedule_retransmission(0);
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
            adapter.bind_local_candidate(later_round, &later_block),
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
        adapter.schedule_retransmission(later_view);
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
    fn global_lock_prunes_losing_deferred_handoff_without_poisoning_the_lane_slot() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let later_view =
            u64::try_from(adapter.context.roster.len()).expect("fixture roster length fits u64");
        assert_eq!(
            adapter.context.leader(0),
            adapter.context.leader(later_view),
            "the same local proposer must authenticate both competing handoffs"
        );

        let (losing_block, losing_proposal) =
            planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let losing_round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: 0,
        };
        adapter
            .planned_lane_proposals
            .insert(losing_round, vec![losing_proposal.clone()]);
        assert_eq!(
            adapter.bind_local_candidate(losing_round, &losing_block),
            V2LaneIngressOutcome::Inserted
        );
        let losing_handoff = adapter
            .outbound_lane_handoffs
            .get(&losing_proposal.proposal_hash)
            .cloned()
            .expect("losing global proposer handoff is retained before lock");
        let losing_sender = losing_handoff.proposer.clone();
        assert_eq!(
            adapter.accept_lane_executable_payload_handoff(
                losing_handoff.clone(),
                Some(&losing_sender),
                0,
            ),
            V2LaneIngressOutcome::Inserted,
            "a valid handoff may arrive before its global proposal is selected"
        );
        assert_eq!(adapter.lane_payload_handoffs.snapshot().len(), 1);

        let (winning_block, winning_proposal) =
            planned_lane_candidate_block_at_view(&adapter, &keys, later_view);
        let winning_round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: later_view,
        };
        adapter
            .planned_lane_proposals
            .insert(winning_round, vec![winning_proposal.clone()]);
        assert_eq!(
            adapter.bind_local_candidate(winning_round, &winning_block),
            V2LaneIngressOutcome::Inserted
        );
        let winning_handoff = adapter
            .outbound_lane_handoffs
            .get(&winning_proposal.proposal_hash)
            .cloned()
            .expect("winning global proposer handoff is retained before lock");

        assert!(adapter.mark_global_body_locked(winning_block.hash()));
        assert!(
            adapter.lane_payload_handoffs.snapshot().is_empty(),
            "locking the winning global body must evict a losing first-wins cache entry"
        );
        assert_eq!(
            adapter.accept_lane_executable_payload_handoff(
                losing_handoff,
                Some(&losing_sender),
                later_view,
            ),
            V2LaneIngressOutcome::Rejected,
            "a losing handoff arriving after the lock must not refill the slot"
        );

        let winning_sender = winning_handoff.proposer.clone();
        assert_eq!(
            adapter.accept_lane_executable_payload_handoff(
                winning_handoff,
                Some(&winning_sender),
                later_view,
            ),
            V2LaneIngressOutcome::Inserted,
            "the exact locked handoff must remain admissible for deferred body binding"
        );
        assert_ne!(
            adapter.bind_locked_global_body(&winning_block),
            V2LaneIngressOutcome::Rejected
        );
        assert!(adapter.lane_payload_handoffs.snapshot().is_empty());
        assert!(
            adapter
                .autonomous_payload_for_proposal(&winning_proposal)
                .is_some()
        );
    }

    #[test]
    fn locked_body_protected_session_conflict_keeps_kura_sidecars_state_inert() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let entry = pending_sidecar_entry(&adapter, &keys, 1);
        let entry_hash = adapter
            .kura
            .persist_pending_certified_merge_entry(&entry)
            .expect("persist losing sidecar before locked-body failure");
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height)
            .expect("fixture lane is active");
        let (block, locked_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let conflict = proposal_for_route_at_view(
            &adapter,
            &keys,
            lane_id,
            dataspace_id,
            incarnation,
            adapter.context.height + 1,
            locked_proposal.descriptor.lane_block_height,
            locked_proposal.descriptor.lane_block_view,
        );
        assert_ne!(conflict.proposal_hash, locked_proposal.proposal_hash);
        assert_eq!(
            adapter.lane_sessions.insert_proposal(conflict.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            adapter
                .lane_sessions
                .insert_qc_with_pops(lane_qc(&conflict, &keys), &lane_signer_pops(&keys)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(adapter.mark_global_body_locked(block.hash()));
        assert_eq!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected,
            "a lane-local QC for a conflicting payload must remain safety-protected"
        );
        assert_eq!(
            adapter
                .kura
                .merge_entry_by_hash(entry_hash)
                .expect("read sidecar after rejected locked body"),
            Some(entry),
            "rejected in-memory binding must not destructively prune Kura"
        );
    }

    #[test]
    fn locked_body_replaces_uncommitted_same_slot_conflict() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, locked_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let descriptor = &locked_proposal.descriptor;
        let conflict = proposal_for_route_at_view(
            &adapter,
            &keys,
            descriptor.lane_id,
            descriptor.dataspace_id,
            descriptor.lane_incarnation,
            adapter.context.height + 1,
            descriptor.lane_block_height,
            descriptor.lane_block_view,
        );
        assert_ne!(conflict.proposal_hash, locked_proposal.proposal_hash);
        assert_eq!(
            adapter.lane_sessions.insert_proposal(conflict.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        assert!(adapter.mark_global_body_locked(block.hash()));
        assert_eq!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Inserted,
            "the globally locked payload must displace an uncertified attacker shell"
        );
        assert!(adapter.lane_sessions.contains_proposal(&locked_proposal));
        assert!(!adapter.lane_sessions.contains_proposal(&conflict));
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

    fn merge_candidate_for_persistence_retry(
        adapter: &V2LaneWorkAdapter,
        view: wire::View,
    ) -> crate::merge::MergeLedgerCandidate {
        let nexus = adapter.state.nexus_snapshot();
        let active_lanes = nexus
            .lane_catalog
            .lanes()
            .iter()
            .map(|lane| iroha_data_model::merge::MergeLaneBinding {
                lane_id: lane.id,
                dataspace_id: lane.dataspace_id,
                lane_config_hash: crate::merge::merge_lane_config_hash(lane),
                incarnation: adapter
                    .state
                    .lane_incarnation_at_height(lane.id, adapter.context.height)
                    .expect("fixture lane incarnation is active"),
                activation_height: 1,
            })
            .collect::<Vec<_>>();
        let incarnation_entries = active_lanes
            .iter()
            .map(
                |binding| iroha_data_model::nexus::LaneLifecycleIncarnationEntry {
                    lane_id: binding.lane_id,
                    incarnation: binding.incarnation,
                },
            )
            .collect::<Vec<_>>();
        crate::merge::MergeLedgerCandidate {
            epoch_id: 1,
            view,
            carrier_height: adapter.context.height,
            carrier_parent_hash: adapter
                .context
                .parent_commit_qc
                .as_ref()
                .expect("non-genesis fixture parent")
                .subject
                .block_hash,
            lane_catalog_hash: iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(
                &nexus.lane_catalog,
            ),
            active_lanes: active_lanes.clone(),
            incarnation_root: iroha_data_model::nexus::LaneLifecycleParameterV1::incarnation_root(
                &incarnation_entries,
            ),
            activation_root: crate::merge::merge_activation_root(&active_lanes),
            lane_snapshots: Vec::new(),
            execution_batch: None,
            lane_drain_certificates: Vec::new(),
            global_state_root: crate::merge::reduce_merge_hint_roots(&[]),
        }
    }

    #[test]
    fn quorate_merge_persistence_retries_after_transient_kura_failure() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let candidate = merge_candidate_for_persistence_retry(&adapter, 0);
        let digest = crate::merge::merge_qc_message_digest(
            &adapter.context.chain_id,
            &candidate,
            VALIDATOR_SET_HASH_VERSION_V1,
            adapter.frozen_validator_set_hash(),
        );
        let key = MergeKey {
            epoch_id: candidate.epoch_id,
            view: candidate.view,
            digest,
        };
        let signatures = keys
            .iter()
            .enumerate()
            .map(|(index, key_pair)| {
                (
                    u32::try_from(index).expect("fixture signer index fits u32"),
                    Signature::try_new(key_pair.private_key(), digest.as_ref())
                        .expect("sign retry candidate")
                        .payload()
                        .to_vec(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let certified_entry = pending_sidecar_entry(&adapter, &keys, 0);
        adapter.merge_entries.insert(
            key,
            PendingMerge {
                // Exercise the boundary after quorum authentication and full
                // State validation: production caches this exact entry before
                // its first fallible Kura publication attempt.
                stage: PendingMergeStage::Certified(certified_entry),
                signatures,
            },
        );

        let pending_dir = adapter.kura.store_root().join("pending_merge_entries");
        std::fs::write(&pending_dir, b"temporarily block pending sidecar directory")
            .expect("install transient Kura obstruction");
        adapter.try_commit_merge(key);
        assert!(
            adapter.merge_entries.contains_key(&key),
            "failed Kura publication must retain the complete quorum"
        );
        assert!(
            matches!(
                adapter.merge_entries[&key].stage,
                PendingMergeStage::Certified(_)
            ),
            "a disk failure must retain the already-validated exact entry"
        );
        std::fs::remove_file(&pending_dir).expect("remove transient Kura obstruction");

        adapter.schedule_retransmission(0);
        assert!(
            adapter.merge_entries.contains_key(&key),
            "a durably staged quorum must remain available for asymmetric share retransmission"
        );
        assert!(adapter.durably_staged_merge_entries.contains(&key));
        assert_eq!(
            std::fs::read_dir(&pending_dir)
                .expect("read recovered pending sidecar directory")
                .count(),
            1
        );
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
