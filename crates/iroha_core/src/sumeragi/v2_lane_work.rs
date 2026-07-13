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
    time::Instant,
};

#[cfg(test)]
use std::sync::{Barrier, mpsc};

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
        MAX_MERGE_EXECUTION_ENTRYPOINTS, MAX_MERGE_LEDGER_ENTRY_BYTES, MergeCommitteeSignature,
        MergeLedgerEntry, MergeQuorumCertificate, MergeSignerProof,
    },
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    peer::PeerId,
};
use iroha_primitives::numeric::Quantity;
use norito::codec::Encode as _;
use thiserror::Error;

use super::{
    InboundBlockMessage, LaneRelayMessage,
    lane_planner::{
        pinned_autoscale_validator_pops_for_set, prepare_v2_lane_payload_plan,
        proposal_lookahead_enabled, v2_known_lane_tip_for_route,
    },
    message::BlockMessage,
    output_guard::ConsensusOutputGuard,
    v2_candidate::{
        CandidateDescriptor, CandidateWorkProvider, CandidateWorkUnavailable, PreparedCandidateWork,
    },
    v2_context::StagedGenesisNexusAmxContext,
    v2_effects::VerifiedPendingGenesisNexusAmxContext,
};
use crate::{
    kura::Kura,
    lane_consensus::{
        CommittedLaneBlockSession, LaneBlockSessionCache, LaneBlockSessionInsertOutcome,
        LaneBlockVoteV1,
    },
    merge_sidecar::{
        CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarMessage, ChunkIngestOutcome,
        MergeSidecarError, MergeSidecarPost, MergeSidecarTransport, MergeSigningContextV1,
        MergeSigningGuard, certified_merge_reference_digest, certified_merge_sidecar_holders,
        decode_certified_merge_sidecar,
    },
    native_amx::{
        NativeAmxAttestationRequestV2, NativeAmxCommitRequestV2, NativeAmxMessage,
        NativeAmxSessionCache, NativeAmxSessionError, NativeAmxSessionKey, NativeAmxSigningGuard,
        NativeAmxVoteV2, aggregate_votes_to_qc, validate_native_amx_qc,
    },
    queue::{RoutingDecision, RoutingPlan},
    state::State,
};

#[cfg(test)]
use crate::queue::{RouteLeg, RouteLegRole};

// Keep compact-QC preflight at least as strict as State's full-entry admission
// before allocating transport. These are first-release protocol caps, not
// runtime tuning knobs.
const MAX_FETCH_MERGE_SIGNER_PROOFS: usize = 4_096;
const MAX_FETCH_MERGE_VALIDATORS: usize = 4_096;
const MAX_FETCH_MERGE_QC_BYTES: usize = 4 * 1024 * 1024;
const MERGE_QC_PROOF_BYTES: usize = 96;
const MAX_AUTHENTICATED_MERGE_QCS: usize = 64;
const MERGE_QC_AUTH_CACHE_DOMAIN: &[u8] = b"iroha:sumeragi:v2:merge-qc-auth-cache:v1\0";

fn preferred_merge_candidates<T: Clone>(
    authorized_digest: Option<Hash>,
    relays: Vec<T>,
    installed: Vec<T>,
    digest: impl Fn(&T) -> Hash,
) -> Vec<T> {
    let mut available = relays.iter().chain(&installed).cloned().collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    available.retain(|candidate| seen.insert(digest(candidate)));

    if let Some(authorized_digest) = authorized_digest {
        return available
            .into_iter()
            .find(|candidate| digest(candidate) == authorized_digest)
            .into_iter()
            .collect();
    }
    if !relays.is_empty() {
        return relays;
    }
    installed
}

/// Authenticated source for the sole height-one projection which is not yet
/// available from committed state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AuthenticatedGenesisNexusAmxContext {
    /// Projection recomputed from the validated, uncommitted genesis overlay.
    Staged(StagedGenesisNexusAmxContext),
    /// Projection bound by exact pending-Decision/body/validation replay.
    ReplayedPending(VerifiedPendingGenesisNexusAmxContext),
}

impl AuthenticatedGenesisNexusAmxContext {
    const fn hash(self) -> Hash {
        match self {
            Self::Staged(staged) => staged.hash(),
            Self::ReplayedPending(replayed) => replayed.hash(),
        }
    }
}

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
    if let Some(entrypoint_count) = reference.entrypoint_count
        && !(1..=u64::try_from(MAX_MERGE_EXECUTION_ENTRYPOINTS).unwrap_or(u64::MAX))
            .contains(&entrypoint_count)
    {
        return Err(
            "certified merge reference has an invalid execution entrypoint count".to_owned(),
        );
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
    /// Send one authenticated certified merge-sidecar request or response.
    PostCertifiedMergeSidecar {
        /// Exact destination selected by the sidecar transport.
        peer: PeerId,
        /// Bounded request or fixed-boundary response chunk.
        message: CertifiedMergeSidecarMessage,
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
#[derive(Debug, Error)]
pub(crate) enum V2LaneWorkError {
    /// A fatal consensus effect requires process restart before more lane work.
    #[error("Sumeragi v2 lane work requires process restart")]
    RestartRequired,
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
    /// Durable local merge-signing anti-equivocation state could not be opened.
    #[error("failed to open durable merge-signing guard: {0}")]
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

#[derive(Clone, Debug)]
struct NativeParticipantControl {
    proposal: LaneBlockProposalV1,
    settlement: LaneBlockCommitment,
}

type NativeParticipantControlMap = BTreeMap<(LaneId, DataSpaceId), NativeParticipantControl>;

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

#[cfg(test)]
struct LanePersistencePause {
    entered: mpsc::SyncSender<()>,
    release: Arc<Barrier>,
}

/// Authoritative bounded adapter retained for exactly one global height.
pub(crate) struct V2LaneWorkAdapter {
    context: wire::HeightContext,
    local_peer: PeerId,
    key_pair: KeyPair,
    voting_enabled: bool,
    state: Arc<State>,
    kura: Arc<Kura>,
    output_guard: Arc<ConsensusOutputGuard>,
    limits: V2LaneWorkLimits,
    lane_sessions: LaneBlockSessionCache,
    native_sessions: NativeAmxSessionCache,
    native_claims: BTreeMap<NativeVoteClaimKey, NativeAmxAttestationBodyV2>,
    native_claim_order: VecDeque<NativeVoteClaimKey>,
    local_native_claims: BTreeMap<NativeVoteClaimKey, NativeAmxAttestationBodyV2>,
    native_signing_guard: Option<NativeAmxSigningGuard>,
    native_requests: BTreeMap<NativeRequestKey, NativeAmxMessage>,
    planned_lane_proposals: BTreeMap<wire::ConsensusRound, Vec<LaneBlockProposalV1>>,
    pending_local_lane_proposals: BTreeMap<HashOf<BlockHeader>, Vec<LaneBlockProposalV1>>,
    globally_locked_body_hash: Option<HashOf<BlockHeader>>,
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
    merge_signing_guard: MergeSigningGuard,
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
    regular_retransmit_class_cursor: usize,
    #[cfg(test)]
    persistence_pause: Option<LanePersistencePause>,
    #[cfg(test)]
    persistence_failure: Option<String>,
}

impl V2LaneWorkAdapter {
    /// Open one adapter after verifying the frozen Nexus/AMX commitment.
    ///
    /// # Errors
    ///
    /// Returns [`V2LaneWorkError`] for malformed context, local-key drift, or
    /// committed-state/context drift. `recovered_applied_height` is accepted
    /// only when it identifies this exact context and canonical post-apply tip.
    #[cfg(test)]
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
        Self::new_with_output_guard(
            context,
            local_peer,
            key_pair,
            voting_enabled,
            state,
            kura,
            limits,
            None,
            recovered_applied_height,
            ConsensusOutputGuard::isolated(),
        )
    }

    /// Open one production adapter under the process-lifetime consensus output guard.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_output_guard(
        context: wire::HeightContext,
        local_peer: PeerId,
        key_pair: KeyPair,
        voting_enabled: bool,
        state: Arc<State>,
        kura: Arc<Kura>,
        limits: V2LaneWorkLimits,
        authenticated_genesis_nexus_amx_context: Option<AuthenticatedGenesisNexusAmxContext>,
        recovered_applied_height: Option<super::v2_recovery::PendingKuraApply>,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<Self, V2LaneWorkError> {
        let construction_guard = Arc::clone(&output_guard);
        let construction = construction_guard
            .begin_fail_stop_operation()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        context
            .validate()
            .map_err(|error| V2LaneWorkError::InvalidContext(error.to_string()))?;
        if local_peer.public_key() != key_pair.public_key() {
            return Err(V2LaneWorkError::LocalKeyMismatch);
        }
        let state_height = u64::try_from(state.committed_height())
            .map_err(|_| V2LaneWorkError::StateHeightMismatch)?;
        let is_pre_apply = state_height.checked_add(1) == Some(context.height);
        let is_post_apply = state_height == context.height;
        if !is_pre_apply && !is_post_apply {
            return Err(V2LaneWorkError::StateHeightMismatch);
        }
        let is_fresh_genesis_pre_apply = is_pre_apply
            && state_height == 0
            && context.height == 1
            && context.parent_commit_qc.is_none()
            && context.snapshot_bootstrap.is_none();
        let pre_apply_context_matches = if is_fresh_genesis_pre_apply {
            authenticated_genesis_nexus_amx_context
                .is_some_and(|authenticated| authenticated.hash() == context.nexus_amx_context_hash)
        } else {
            authenticated_genesis_nexus_amx_context.is_none()
                && super::v2_recovery::committed_nexus_amx_context_hash(state.as_ref())
                    == context.nexus_amx_context_hash
        };
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
                && kura
                    .exact_durable_blocks_count()
                    .is_ok_and(|durable_height| durable_height == height)
                && kura.get_durable_block_hash(nonzero_height) == Some(pending.block_hash())
        });
        if (is_post_apply || recovered_applied_height.is_some()) && !recovered_applied_tip_matches {
            return Err(V2LaneWorkError::RecoveredAppliedTipMismatch);
        }
        if authenticated_genesis_nexus_amx_context.is_some() && !is_fresh_genesis_pre_apply {
            return Err(V2LaneWorkError::NexusContextMismatch);
        }
        if is_pre_apply && !pre_apply_context_matches {
            return Err(V2LaneWorkError::NexusContextMismatch);
        }
        let committed_merge_epoch = state
            .merge_ledger()
            .latest()
            .map_or(0, |entry| entry.epoch_id);
        let merge_signing_guard = MergeSigningGuard::open_with_committed_frontier(
            &kura.store_root(),
            committed_merge_epoch,
            state_height,
        )
        .map_err(|error| V2LaneWorkError::SigningGuard(error.to_string()))?;
        let native_signing_guard = if voting_enabled
            && local_peer.public_key().try_algorithm().ok() == Some(Algorithm::BlsNormal)
        {
            let max_records = limits
                .session_capacity
                .get()
                .checked_mul(limits.body_buckets_per_session.get())
                .and_then(NonZeroUsize::new)
                .ok_or_else(|| {
                    V2LaneWorkError::SigningGuard(
                        "native AMX signing-record capacity overflows usize".to_owned(),
                    )
                })?;
            let chain_id = context.chain_id.clone().into_inner();
            Some(
                NativeAmxSigningGuard::open(
                    &kura.store_root(),
                    context.height,
                    context.id(),
                    context.epoch,
                    Hash::new(chain_id.as_bytes()),
                    local_peer.clone(),
                    max_records,
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
            output_guard,
            limits,
            lane_sessions: LaneBlockSessionCache::new(limits.session_capacity.get()),
            native_sessions: NativeAmxSessionCache::with_limits(
                limits.session_capacity,
                limits.body_buckets_per_session,
            ),
            native_claims: BTreeMap::new(),
            native_claim_order: VecDeque::new(),
            local_native_claims: BTreeMap::new(),
            native_signing_guard,
            native_requests: BTreeMap::new(),
            planned_lane_proposals: BTreeMap::new(),
            pending_local_lane_proposals: BTreeMap::new(),
            globally_locked_body_hash: None,
            retained_merge_carrier_state: None,
            #[cfg(test)]
            merge_retention_scans: 0,
            locally_bound_lane_proposals: BTreeSet::new(),
            pending_committed_lanes: VecDeque::new(),
            admitted_relays: BTreeSet::new(),
            merge_entries: BTreeMap::new(),
            merge_claims: BTreeMap::new(),
            merge_signing_guard,
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
            regular_retransmit_class_cursor: 0,
            #[cfg(test)]
            persistence_pause: None,
            #[cfg(test)]
            persistence_failure: None,
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
        adapter.hydrate_canonical_lane_artifacts();
        adapter.drive_lane_sessions();
        construction.complete();
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
        let output_guard = Arc::clone(&self.output_guard);
        let Some(_permit) = output_guard.acquire() else {
            return V2LaneIngressOutcome::Rejected;
        };
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
        let output_guard = Arc::clone(&self.output_guard);
        let Some(_permit) = output_guard.acquire() else {
            return false;
        };
        if self.globally_locked_body_hash.is_some() {
            return false;
        }
        self.globally_locked_body_hash = Some(block_hash);
        self.locally_bound_lane_proposals.clear();
        self.merge_entries.clear();
        self.merge_claims.clear();
        self.purge_queued_merge_broadcasts();
        true
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
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        let result = self.retain_merge_sidecars_for_global_view_guarded(
            view,
            locked_subject,
            decided_subject,
        );
        if result.is_ok() {
            operation.complete();
        }
        result
    }

    fn retain_merge_sidecars_for_global_view_guarded(
        &mut self,
        view: wire::View,
        locked_subject: Option<wire::BlockSubject>,
        decided_subject: Option<wire::BlockSubject>,
    ) -> Result<(), V2LaneWorkError> {
        let carrier_state = (view, locked_subject, decided_subject);
        if self.retained_merge_carrier_state == Some(carrier_state) {
            return Ok(());
        }
        self.purge_queued_merge_broadcasts();
        if locked_subject.is_some() || decided_subject.is_some() {
            self.merge_entries.clear();
            self.merge_claims.clear();
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
            .or_else(|| {
                self.context
                    .snapshot_bootstrap
                    .as_ref()
                    .map(|anchor| anchor.snapshot_block_hash)
            })
        else {
            self.kura
                .retain_pending_certified_merge_entry_for_locked_carrier(self.context.height, None)
                .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
            self.retained_merge_carrier_state = Some(carrier_state);
            self.refresh_merge_candidates(view)?;
            return Ok(());
        };
        self.kura
            .prune_pending_certified_merge_entries_not_bound_to(self.context.height, parent, view)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        self.retained_merge_carrier_state = Some(carrier_state);
        self.refresh_merge_candidates(view)?;
        Ok(())
    }

    /// Bind lane proposals reconstructed from the exact durable globally
    /// locked body, then release their bounded lane-local consensus sessions.
    pub(crate) fn bind_locked_global_body(&mut self, block: &SignedBlock) -> V2LaneIngressOutcome {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(validation_permit) = output_guard.acquire() else {
            return V2LaneIngressOutcome::Rejected;
        };
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
        }
        // Do not delete any losing durable sidecar until every in-memory
        // locked-body check and session insertion has succeeded. Once this
        // exact retention succeeds, the remaining operations are infallible.
        drop(validation_permit);
        let Some(persistence) = output_guard.begin_fail_stop_operation() else {
            return V2LaneIngressOutcome::Rejected;
        };
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
        let outcome = if inserted {
            V2LaneIngressOutcome::Inserted
        } else {
            V2LaneIngressOutcome::Duplicate
        };
        persistence.complete();
        outcome
    }

    /// Persist only completed lane sessions anchored by canonical Kura blocks.
    ///
    /// # Errors
    ///
    /// Returns [`V2LaneWorkError::Persistence`] if an anchored certificate or
    /// its canonical globally-applied receipt cannot be written durably.
    pub(crate) fn persist_anchored_sessions(&mut self) -> Result<usize, V2LaneWorkError> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2LaneWorkError::RestartRequired)?;
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
        operation.complete();
        Ok(persisted)
    }

    /// Retire losing certified merge sidecars once another carrier is durably
    /// finalized at this height.
    pub(crate) fn prune_finalized_merge_sidecars(&mut self) -> Result<(), V2LaneWorkError> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        #[cfg(test)]
        if let Some(pause) = &self.persistence_pause {
            let _ = pause.entered.send(());
            pause.release.wait();
        }
        #[cfg(test)]
        if let Some(reason) = self.persistence_failure.take() {
            return Err(V2LaneWorkError::Persistence(reason));
        }
        self.merge_entries.clear();
        self.merge_claims.clear();
        self.purge_queued_merge_broadcasts();
        self.merge_sidecars
            .retain_pending_blocks(&BTreeSet::new(), self.context.height);
        self.kura
            .prune_finalized_pending_certified_merge_entries(self.context.height)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        operation.complete();
        Ok(())
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
        let output_guard = Arc::clone(&self.output_guard);
        let Some(_permit) = output_guard.acquire() else {
            return V2LaneIngressOutcome::Rejected;
        };
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
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        let result = self.defer_missing_merge_sidecar_guarded(round, subject, reference, decided);
        if result.is_ok() {
            operation.complete();
        }
        result
    }

    fn defer_missing_merge_sidecar_guarded(
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
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard.acquire()?;
        let hash = self.completed_merge_sidecars.iter().next().copied()?;
        self.completed_merge_sidecars.remove(&hash);
        Some(hash)
    }

    /// Take one exact full-entry rejection to apply to every retained body
    /// referencing the same hash.
    pub(crate) fn take_rejected_merge_sidecar(&mut self) -> Option<RejectedMergeSidecar> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard.acquire()?;
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
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        let committed_height = u64::try_from(self.state.committed_height())
            .map_err(|_| V2LaneWorkError::StateHeightMismatch)?;
        self.merge_sidecars
            .retain_pending_blocks(pending_blocks, committed_height);
        operation.complete();
        Ok(())
    }

    /// Accept one lane relay, merge signature, or context-bound Native AMX message.
    pub(super) fn accept_relay_message(
        &mut self,
        message: LaneRelayMessage,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            return V2LaneIngressOutcome::Rejected;
        };
        let result = match message {
            LaneRelayMessage::Envelope(envelope) => self.accept_lane_relay(envelope, active_view),
            LaneRelayMessage::MergeSignature(signature) => {
                self.accept_merge_signature(signature, active_view)
            }
            LaneRelayMessage::CertifiedMergeSidecar { sender, message } => {
                self.accept_certified_merge_sidecar(sender, message)
            }
            LaneRelayMessage::NativeAmx { sender, message } => {
                Ok(self.accept_native_amx(sender, message, active_view))
            }
        };
        match result {
            Ok(outcome) => {
                operation.complete();
                outcome
            }
            Err(error) => {
                iroha_logger::error!(
                    ?error,
                    "Sumeragi v2 lane relay hit a fatal local durability failure"
                );
                drop(operation);
                V2LaneIngressOutcome::Rejected
            }
        }
    }

    /// Drain at most `limit` explicit transport effects.
    pub(crate) fn drain_effects(&mut self, limit: usize) -> Vec<V2LaneWorkEffect> {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(_permit) = output_guard.acquire() else {
            return Vec::new();
        };
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

    /// Re-enqueue bounded lane artifacts, Native AMX requests, and merge shares
    /// with rotating class priority for reliable retransmission.
    ///
    /// # Errors
    ///
    /// Returns a restart-required or durable-persistence error before this
    /// process may publish any later consensus output.
    pub(crate) fn schedule_retransmission(&mut self) -> Result<(), V2LaneWorkError> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        let sidecar_posts = self.merge_sidecars.tick_bounded(
            &self.local_peer,
            Instant::now(),
            self.sidecar_effect_slots(),
        );
        for post in sidecar_posts {
            debug_assert!(self.push_merge_sidecar_post(post));
        }

        let active_merge_view = self
            .pre_apply_unlocked_merge_view()
            .filter(|_| self.merge_parent_frontier_is_exact());
        if let Some(view) = active_merge_view {
            // Revisit reachable relay-settlement candidates on the bounded
            // retransmission cadence. Execution-batch candidates are filtered
            // before authorization or private-key use.
            self.refresh_merge_candidates(view)?;
        } else {
            self.purge_queued_merge_broadcasts();
        }
        const REGULAR_RETRANSMIT_CLASS_COUNT: usize = 3;
        let start = self.regular_retransmit_class_cursor % REGULAR_RETRANSMIT_CLASS_COUNT;
        for offset in 0..REGULAR_RETRANSMIT_CLASS_COUNT {
            match (start + offset) % REGULAR_RETRANSMIT_CLASS_COUNT {
                0 => self.schedule_lane_artifact_retransmissions(),
                1 => self.schedule_native_retransmissions(),
                2 => {
                    if let Some(view) = active_merge_view {
                        self.schedule_merge_share_retransmissions(view);
                    }
                }
                _ => unreachable!("regular retransmission class is modulo three"),
            }
        }
        self.regular_retransmit_class_cursor = (start + 1) % REGULAR_RETRANSMIT_CLASS_COUNT;

        let Some(active_merge_view) = active_merge_view else {
            operation.complete();
            return Ok(());
        };
        // Quorum formation and Kura publication are separate durability
        // boundaries. Revisit already-quorate candidates on the bounded
        // cadence; any publication failure poisons the process-wide output
        // guard instead of permitting this process to continue signing.
        let merge_keys = self
            .merge_entries
            .keys()
            .filter(|key| key.view == active_merge_view)
            .copied()
            .collect::<Vec<_>>();
        for key in merge_keys {
            self.try_commit_merge(key)?;
        }
        operation.complete();
        Ok(())
    }

    fn schedule_lane_artifact_retransmissions(&mut self) {
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
    }

    fn schedule_native_retransmissions(&mut self) {
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
    }

    fn schedule_merge_share_retransmissions(&mut self, active_merge_view: wire::View) {
        let mut merge_effects = Vec::new();
        if let Some(local_index) = self.local_validator_index() {
            for (key, pending) in &self.merge_entries {
                if key.view != active_merge_view {
                    continue;
                }
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

    fn accept_certified_merge_sidecar(
        &mut self,
        sender: PeerId,
        message: CertifiedMergeSidecarMessage,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
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
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        let now = Instant::now();
        if let Err(error) =
            self.merge_sidecars
                .admit_server_request(&sender, &request, &self.local_peer, now)
        {
            iroha_logger::debug!(%sender, ?error, "dropping v2 certified merge-sidecar request");
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let entry = match self.kura.merge_entry_by_hash(request.entry_hash) {
            Ok(Some(entry)) => entry,
            Ok(None) => return Ok(V2LaneIngressOutcome::Rejected),
            Err(error) => return Err(V2LaneWorkError::Persistence(error.to_string())),
        };
        if entry.execution_batch.is_some() {
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let reference = CertifiedMergeLedgerReference::new(&entry);
        let metadata_matches = request.encoded_len == reference.encoded_len
            && request.epoch_id == reference.epoch_id
            && request.reference_digest == certified_merge_reference_digest(&reference);
        let local_is_holder = certified_merge_sidecar_holders(&reference)
            .is_ok_and(|holders| holders.contains(&self.local_peer));
        if !metadata_matches || !local_is_holder {
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        if let Err(error) =
            self.merge_sidecars
                .enqueue_response(request, entry.canonical_bytes(), now)
        {
            iroha_logger::debug!(%sender, ?error, "v2 merge-sidecar response budget rejected request");
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let posts = self
            .merge_sidecars
            .drain_outbound_chunks(self.sidecar_effect_slots().min(8), now);
        let inserted = !posts.is_empty();
        for post in posts {
            debug_assert!(self.push_merge_sidecar_post(post));
        }
        Ok(if inserted {
            V2LaneIngressOutcome::Inserted
        } else {
            V2LaneIngressOutcome::Duplicate
        })
    }

    fn accept_certified_merge_sidecar_chunk(
        &mut self,
        sender: PeerId,
        chunk: crate::merge_sidecar::CertifiedMergeSidecarChunkV1,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        let entry_hash = chunk.entry_hash;
        let now = Instant::now();
        let outcome = match self.merge_sidecars.ingest_chunk(&sender, chunk, now) {
            Ok(outcome) => outcome,
            Err(error) => {
                iroha_logger::debug!(%sender, %entry_hash, ?error, "dropping invalid v2 merge-sidecar chunk");
                return Ok(V2LaneIngressOutcome::Rejected);
            }
        };
        let ChunkIngestOutcome::Complete(completed) = outcome else {
            return Ok(V2LaneIngressOutcome::Inserted);
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
                return Ok(V2LaneIngressOutcome::Rejected);
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
            return Ok(V2LaneIngressOutcome::Rejected);
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
                Ok(V2LaneIngressOutcome::Inserted)
            }
            Ok(other_hash) => Err(V2LaneWorkError::Persistence(format!(
                "Kura persisted conflicting certified merge sidecar hash {other_hash}"
            ))),
            Err(error) => Err(V2LaneWorkError::Persistence(error.to_string())),
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

    fn accept_lane_relay(
        &mut self,
        envelope: LaneRelayEnvelope,
        active_view: wire::View,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        let key = (
            envelope.lane_id,
            envelope.dataspace_id,
            envelope.block_height,
            Hash::from(envelope.settlement_hash),
        );
        if self.admitted_relays.contains(&key) {
            return Ok(V2LaneIngressOutcome::Duplicate);
        }
        if self.admitted_relays.len() >= self.limits.relay_capacity.get() {
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        match self.state.record_lane_relay(&envelope) {
            Ok(crate::state::LaneRelayInsert::Duplicate) => Ok(V2LaneIngressOutcome::Duplicate),
            Ok(
                crate::state::LaneRelayInsert::Inserted | crate::state::LaneRelayInsert::Replaced,
            ) => {
                self.admitted_relays.insert(key);
                self.refresh_merge_candidates(active_view)?;
                Ok(V2LaneIngressOutcome::Inserted)
            }
            Err(_) => Ok(V2LaneIngressOutcome::Rejected),
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

    fn purge_queued_merge_broadcasts(&mut self) {
        self.effects
            .retain(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)));
        self.effect_keys = self.effects.iter().map(lane_work_effect_key).collect();
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
        {
            return V2LaneIngressOutcome::Rejected;
        }
        let Some((validators, min_signers, pops, _)) = self.native_committee(&body) else {
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
        let Some((validators, _, _, _)) = self.native_committee(&vote.body) else {
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
            Err(
                NativeAmxSessionError::PhaseMismatch
                | NativeAmxSessionError::PlanEquivocation
                | NativeAmxSessionError::Capacity,
            ) => V2LaneIngressOutcome::Rejected,
        }
    }

    fn native_body_matches_context(
        &self,
        body: &NativeAmxAttestationBodyV2,
        active_view: wire::View,
    ) -> bool {
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
            && self
                .state
                .lane_incarnation_at_height(body.coordinator_lane_id, body.authority_context_height)
                == Some(body.coordinator_lane_incarnation)
            && self
                .state
                .lane_incarnation_at_height(body.participant_lane_id, body.authority_context_height)
                == Some(body.participant_lane_incarnation)
    }

    fn native_chain_id_hash(&self) -> Hash {
        let chain_id = self.context.chain_id.clone().into_inner();
        Hash::new(chain_id.as_bytes())
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
        let pinned =
            pinned_autoscale_validator_pops_for_set(&self.state, participant_lane, &validators)?;
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
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard.begin_fail_stop_operation()?;
        let Some(signing_guard) = self.native_signing_guard.as_ref() else {
            iroha_logger::error!(
                height = self.context.height,
                "Native AMX validator has no durable signing guard"
            );
            return None;
        };
        if let Err(error) = signing_guard.record(&body) {
            if error.requires_restart_recovery() {
                iroha_logger::error!(
                    height = self.context.height,
                    ?error,
                    "durable Native AMX signing guard requires restart recovery"
                );
                return None;
            }
            iroha_logger::debug!(
                height = self.context.height,
                ?error,
                "durable Native AMX signing guard rejected a conflicting or stale body"
            );
            operation.complete();
            return None;
        }
        let signature =
            Signature::try_new(self.key_pair.private_key(), &body.signature_preimage()).ok()?;
        self.local_native_claims.entry(claim).or_insert(body);
        let vote = NativeAmxVoteV2 {
            body,
            signer: self.local_peer.clone(),
            bls_signature: signature.payload().to_vec(),
        };
        operation.complete();
        Some(vote)
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
                    local_amount: Quantity::zero(),
                    xor_due: Quantity::zero(),
                    xor_after_haircut: Quantity::zero(),
                    xor_variance: Quantity::zero(),
                    timestamp_ms: self.context.height,
                })
                .collect::<Vec<_>>();
            let settlement = LaneBlockCommitment {
                block_height: proposal.descriptor.lane_block_height,
                lane_id: route.lane_id,
                lane_incarnation: participant_lane_incarnation,
                dataspace_id: route.dataspace_id,
                tx_count: u64::try_from(receipts.len()).unwrap_or(u64::MAX),
                total_local_amount: Quantity::zero(),
                total_xor_due: Quantity::zero(),
                total_xor_after_haircut: Quantity::zero(),
                total_xor_variance: Quantity::zero(),
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
            if request.validate_plan_binding().is_err() {
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

    fn pre_apply_unlocked_merge_view(&self) -> Option<wire::View> {
        let committed_height = u64::try_from(self.state.committed_height()).ok()?;
        if committed_height.checked_add(1) != Some(self.context.height)
            || self.globally_locked_body_hash.is_some()
        {
            return None;
        }
        self.retained_merge_carrier_state
            .and_then(|(view, locked, decided)| {
                (locked.is_none() && decided.is_none()).then_some(view)
            })
    }

    fn merge_parent_frontier_is_exact(&self) -> bool {
        let committed_height = self.state.committed_height();
        let Some(parent_height) = NonZeroUsize::new(committed_height) else {
            return false;
        };
        let Some(expected_parent) = self
            .context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash)
            .or_else(|| {
                self.context
                    .snapshot_bootstrap
                    .as_ref()
                    .map(|anchor| anchor.snapshot_block_hash)
            })
        else {
            return false;
        };
        self.kura
            .exact_durable_blocks_count()
            .is_ok_and(|durable_height| durable_height == committed_height)
            && self.kura.get_durable_block_hash(parent_height) == Some(expected_parent)
            && self.state.latest_block_hash_fast() == Some(expected_parent)
    }

    /// Publish the exact local merge-signing decision durably before the
    /// private key is used. In-memory claims remain an immediate same-process
    /// equivocation check, while the Kura-root journal is authoritative across
    /// crashes and adapter reconstruction.
    fn authorize_local_merge_claim(
        &mut self,
        candidate: &crate::merge::MergeLedgerCandidate,
        active_view: wire::View,
        signer: wire::ValidatorIndex,
        message_digest: Hash,
    ) -> Result<(), MergeSidecarError> {
        let Some(expected_parent) = self
            .context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash)
            .or_else(|| {
                self.context
                    .snapshot_bootstrap
                    .as_ref()
                    .map(|anchor| anchor.snapshot_block_hash)
            })
        else {
            return Err(MergeSidecarError::SigningGuard(
                "merge signing requires a committed global parent".to_owned(),
            ));
        };
        if self.pre_apply_unlocked_merge_view() != Some(active_view)
            || candidate.view != active_view
            || candidate.carrier_height != self.context.height
            || candidate.carrier_parent_hash != expected_parent
        {
            return Err(MergeSidecarError::LocalSigningEquivocation);
        }
        let expected_digest = crate::merge::merge_qc_message_digest(
            &self.context.chain_id,
            candidate,
            VALIDATOR_SET_HASH_VERSION_V1,
            self.frozen_validator_set_hash(),
        );
        if expected_digest != message_digest {
            return Err(MergeSidecarError::LocalSigningEquivocation);
        }
        let claim_key = (candidate.epoch_id, candidate.view, signer);
        if self
            .merge_claims
            .get(&claim_key)
            .is_some_and(|existing| *existing != message_digest)
        {
            return Err(MergeSidecarError::LocalSigningEquivocation);
        }
        let signing_context = MergeSigningContextV1 {
            epoch_id: candidate.epoch_id,
            view: candidate.view,
            carrier_height: candidate.carrier_height,
            parent_hash: candidate.carrier_parent_hash,
            validator_set_hash: self.frozen_validator_set_hash(),
        };
        if self
            .merge_signing_guard
            .authorized_digest(&signing_context)?
            .is_some_and(|authorized| authorized != message_digest)
        {
            return Err(MergeSidecarError::LocalSigningEquivocation);
        }
        let committed_height_usize = self.state.committed_height();
        let committed_height = u64::try_from(committed_height_usize)
            .map_err(|_| MergeSidecarError::SigningGuard("State height overflow".to_owned()))?;
        let durable_height = self
            .kura
            .exact_durable_blocks_count()
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        if durable_height != committed_height_usize {
            return Err(MergeSidecarError::SigningGuard(
                "merge signing requires identical committed State and durable Kura frontiers"
                    .to_owned(),
            ));
        }
        let parent_height = usize::try_from(committed_height)
            .ok()
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| {
                MergeSidecarError::SigningGuard(
                    "merge signing parent height is not representable".to_owned(),
                )
            })?;
        if self.kura.get_durable_block_hash(parent_height) != Some(expected_parent) {
            return Err(MergeSidecarError::SigningGuard(
                "merge signing parent is not the exact durable Kura tip".to_owned(),
            ));
        }
        let parent_header = self.state.latest_block_header_fast().ok_or_else(|| {
            MergeSidecarError::SigningGuard(
                "merge signing parent header is absent from committed State".to_owned(),
            )
        })?;
        if parent_header.hash() != expected_parent
            || parent_header.height().get() != committed_height
        {
            return Err(MergeSidecarError::SigningGuard(
                "merge signing parent header differs from the frozen context".to_owned(),
            ));
        }
        self.state
            .validate_merge_candidate_for_global_round(candidate, &parent_header, active_view)
            .map_err(|error| MergeSidecarError::SigningGuard(error.to_string()))?;
        self.merge_signing_guard
            .authorize(signing_context, message_digest)?;
        self.merge_claims.entry(claim_key).or_insert(message_digest);
        Ok(())
    }

    fn refresh_merge_candidates(&mut self, active_view: wire::View) -> Result<(), V2LaneWorkError> {
        let carrier_protected = self
            .retained_merge_carrier_state
            .is_some_and(|(_, locked, decided)| locked.is_some() || decided.is_some());
        if self.globally_locked_body_hash.is_some() || carrier_protected {
            self.merge_entries.clear();
            self.merge_claims.clear();
            return Ok(());
        }
        self.merge_entries.retain(|key, _| key.view == active_view);
        self.merge_claims
            .retain(|(_, view, _), _| *view == active_view);
        if self.pre_apply_unlocked_merge_view() != Some(active_view)
            || !self.merge_parent_frontier_is_exact()
        {
            return Ok(());
        }

        let Some(expected_parent) = self
            .context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash)
            .or_else(|| {
                self.context
                    .snapshot_bootstrap
                    .as_ref()
                    .map(|anchor| anchor.snapshot_block_hash)
            })
        else {
            return Ok(());
        };
        let Some(parent_header) = self.state.latest_block_header_fast() else {
            return Ok(());
        };
        if parent_header.hash() != expected_parent
            || parent_header.height().get().checked_add(1) != Some(self.context.height)
        {
            return Ok(());
        }

        let expected_epoch = self
            .state
            .merge_ledger()
            .latest()
            .map_or(1, |entry| entry.epoch_id.saturating_add(1));
        let validator_set_hash = self.frozen_validator_set_hash();
        let signing_context = MergeSigningContextV1 {
            epoch_id: expected_epoch,
            view: active_view,
            carrier_height: self.context.height,
            parent_hash: expected_parent,
            validator_set_hash,
        };
        let authorized_digest = self
            .merge_signing_guard
            .authorized_digest(&signing_context)
            .map_err(|error| V2LaneWorkError::SigningGuard(error.to_string()))?;
        let installed_candidates = self
            .merge_entries
            .values()
            .filter_map(|pending| match &pending.stage {
                PendingMergeStage::Collecting(candidate)
                    if candidate.epoch_id == expected_epoch
                        && candidate.view == active_view
                        && candidate.carrier_height == self.context.height
                        && candidate.carrier_parent_hash == expected_parent
                        && candidate.execution_batch.is_none() =>
                {
                    Some(candidate.clone())
                }
                PendingMergeStage::Collecting(_) | PendingMergeStage::Certified(_) => None,
            })
            .collect::<Vec<_>>();
        // TODO: Re-enable autonomous execution candidate synthesis only with a
        // coordinated candidate/queue/wire/session redesign that carries one
        // durable reservation identity from selection through availability QC
        // and global handoff. The first-release live path signs settlement
        // candidates produced by lane relays only.
        let relay_candidates = self
            .state
            .merge_entry_candidates_from_lane_relays_for_view(active_view)
            .into_iter()
            .filter(|candidate| {
                candidate.epoch_id == expected_epoch && candidate.execution_batch.is_none()
            })
            .collect::<Vec<_>>();
        let candidates = preferred_merge_candidates(
            authorized_digest,
            relay_candidates,
            installed_candidates,
            |candidate| {
                crate::merge::merge_qc_message_digest(
                    &self.context.chain_id,
                    candidate,
                    VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash,
                )
            },
        );
        for candidate in candidates {
            let digest = crate::merge::merge_qc_message_digest(
                &self.context.chain_id,
                &candidate,
                VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash,
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
                stage: PendingMergeStage::Collecting(candidate.clone()),
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
            if let Err(error) =
                self.authorize_local_merge_claim(&candidate, active_view, local_index, digest)
            {
                if let MergeSidecarError::SigningGuard(reason) = &error {
                    return Err(V2LaneWorkError::SigningGuard(reason.clone()));
                }
                iroha_logger::warn!(
                    ?error,
                    epoch = candidate.epoch_id,
                    view = candidate.view,
                    "refusing local merge signature without durable exact-context authorization"
                );
                continue;
            }
            let signature = Signature::try_new(self.key_pair.private_key(), digest.as_ref())
                .map_err(|error| V2LaneWorkError::SigningGuard(error.to_string()))?;
            let payload = signature.payload().to_vec();
            self.merge_entries
                .get_mut(&key)
                .expect("entry inserted above")
                .signatures
                .insert(local_index, payload.clone());
            self.push_effect(V2LaneWorkEffect::BroadcastMerge(MergeCommitteeSignature {
                epoch_id: key.epoch_id,
                view: key.view,
                signer: local_index,
                message_digest: digest,
                bls_sig: payload,
            }));
            self.try_commit_merge(key)?;
        }
        Ok(())
    }

    fn accept_merge_signature(
        &mut self,
        signature: MergeCommitteeSignature,
        active_view: wire::View,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        if signature.view != active_view
            || self.pre_apply_unlocked_merge_view() != Some(active_view)
            || !self.merge_parent_frontier_is_exact()
        {
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        self.refresh_merge_candidates(active_view)?;
        let key = MergeKey {
            epoch_id: signature.epoch_id,
            view: signature.view,
            digest: signature.message_digest,
        };
        let Some(pending) = self.merge_entries.get(&key) else {
            return Ok(V2LaneIngressOutcome::Rejected);
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
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let Some(peer) = self
            .context
            .roster
            .get(usize::try_from(signature.signer).unwrap_or(usize::MAX))
            .map(|entry| &entry.validator)
        else {
            return Ok(V2LaneIngressOutcome::Rejected);
        };
        let Ok(parsed) = Signature::try_from_bytes(&signature.bls_sig) else {
            return Ok(V2LaneIngressOutcome::Rejected);
        };
        if parsed
            .verify(peer.public_key(), signature.message_digest.as_ref())
            .is_err()
        {
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let claim_key = (signature.epoch_id, signature.view, signature.signer);
        if let Some(existing) = self.merge_claims.get(&claim_key) {
            if existing != &signature.message_digest {
                return Ok(V2LaneIngressOutcome::Rejected);
            }
            if self.merge_entries[&key].signatures.get(&signature.signer)
                != Some(&signature.bls_sig)
            {
                return Ok(V2LaneIngressOutcome::Rejected);
            }
            // Re-evaluate the existing exact quorum without accepting another
            // signer claim. Any durable publication failure propagates to the
            // enclosing fail-stop relay operation.
            self.try_commit_merge(key)?;
            return Ok(V2LaneIngressOutcome::Duplicate);
        }
        self.merge_claims
            .insert(claim_key, signature.message_digest);
        self.merge_entries
            .get_mut(&key)
            .expect("pending entry checked above")
            .signatures
            .insert(signature.signer, signature.bls_sig);
        self.try_commit_merge(key)?;
        Ok(V2LaneIngressOutcome::Inserted)
    }

    fn try_commit_merge(&mut self, key: MergeKey) -> Result<(), V2LaneWorkError> {
        let Some(pending) = self.merge_entries.get(&key) else {
            return Ok(());
        };
        let cached_entry = match &pending.stage {
            PendingMergeStage::Certified(entry) => Some(entry.clone()),
            PendingMergeStage::Collecting(_) => None,
        };
        if let Some(entry) = cached_entry {
            return self.persist_certified_merge_entry(key, &entry);
        }
        let Some(PendingMerge {
            stage: PendingMergeStage::Collecting(candidate),
            ..
        }) = self.merge_entries.get(&key)
        else {
            return Ok(());
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
            return Ok(());
        }
        let signers = pending.signatures.keys().copied().collect::<Vec<_>>();
        if !self.frozen_dual_quorum_met(&signers) {
            return Ok(());
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
            return Ok(());
        };
        let mut bitmap = vec![0_u8; self.context.roster.len().div_ceil(8)];
        for signer in &signers {
            let Ok(index) = usize::try_from(*signer) else {
                return Ok(());
            };
            bitmap[index / 8] |= 1_u8 << (index % 8);
        }
        let signer_proofs = {
            let world = self.state.world_view();
            let mut proofs = Vec::with_capacity(signers.len());
            for signer in &signers {
                let Ok(index) = usize::try_from(*signer) else {
                    return Ok(());
                };
                let Some(peer) = validator_set.get(index) else {
                    return Ok(());
                };
                let Some(proof_of_possession) =
                    crate::state::consensus_key_pop_for_public_key(&world, peer.public_key())
                else {
                    return Ok(());
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
            return Ok(());
        }
        let Some(pending) = self.merge_entries.get_mut(&key) else {
            return Ok(());
        };
        pending.stage = PendingMergeStage::Certified(entry.clone());
        self.persist_certified_merge_entry(key, &entry)
    }

    fn persist_certified_merge_entry(
        &mut self,
        key: MergeKey,
        entry: &MergeLedgerEntry,
    ) -> Result<(), V2LaneWorkError> {
        self.kura
            .persist_pending_certified_merge_entry(entry)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        self.merge_entries.remove(&key);
        Ok(())
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
        let output_guard = Arc::clone(&self.output_guard);
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            return Err(all_unavailable(
                candidates.len(),
                "Sumeragi v2 consensus requires process restart",
            ));
        };
        if context != &self.context {
            operation.complete();
            return Err(all_unavailable(candidates.len(), "height context drift"));
        }
        if let Err(error) = self.refresh_merge_candidates(view) {
            drop(operation);
            return Err(all_unavailable(candidates.len(), error.to_string()));
        }
        let result = (|| {
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
                        "Native AMX participant control proposal is unavailable",
                    )
                })?;

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
            Ok(PreparedCandidateWork {
                native_amx_receipts: receipts,
                lane_payload_ownerships: lane_plan.ownerships,
            })
        })();
        operation.complete();
        result
    }
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
        V2LaneWorkEffect::PostCertifiedMergeSidecar { peer, message } => {
            encoded.push(3);
            encoded.extend(peer.encode());
            encoded.extend(message.encode());
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
        sync::{Arc, Barrier, mpsc},
        thread,
        time::{Duration, Instant},
    };

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        ChainId,
        account::AccountId,
        block::{
            BlockExecutionContextBundle, BlockHeader, BlockSignature, ExternalExecutionContext,
            SignedBlock,
            builder::BlockBuilder,
            consensus::{
                LaneBlockCommitment, NativeAmxAttestationBodyV2, NativeAmxPhase,
                SumeragiLanePayloadOwnership,
            },
            consensus_v2 as wire,
        },
        consensus::{ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus},
        nexus::{DataSpaceId, LaneFastpqProofMaterial, LaneId, LaneStorageProfile, LaneVisibility},
        peer::PeerId,
        transaction::{TransactionBuilder, TransactionEntrypoint, signed::TransactionResultInner},
        trigger::DataTriggerSequence,
    };

    use super::*;
    use crate::{
        block::{CommittedBlock, ValidBlock},
        governance::manifest::{
            GovernanceRules, LaneManifestRegistry, LaneManifestStatus, ManifestValidatorBinding,
        },
        query::store::LiveQueryStore,
        state::World,
        sumeragi::network_topology::Topology,
    };

    fn fixture(mode: wire::ConsensusMode) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
        fixture_at_height(mode, 9)
    }

    fn fixture_with_durable_parent(mode: wire::ConsensusMode) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
        fixture_at_height_inner(mode, 9, true)
    }

    fn fixture_at_height(
        mode: wire::ConsensusMode,
        height: u64,
    ) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
        fixture_at_height_inner(mode, height, false)
    }

    fn fixture_at_height_inner(
        mode: wire::ConsensusMode,
        height: u64,
        persist_parent_chain: bool,
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
        let mut context = wire::HeightContext {
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
                    Hash::new(b"lane-work executed block wire"),
                ),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xA5; 48],
            }),
            snapshot_bootstrap: None,
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
            if persist_parent_chain {
                kura.store_block(block.clone())
                    .expect("persist exact merge-signing parent fixture");
            }
            commit_test_block_to_state(state.as_ref(), &block, &context);
        }
        if let Some(parent_qc) = context.parent_commit_qc.as_mut() {
            parent_qc.subject.block_hash = parent.expect("non-genesis fixture has a parent");
        }
        let local_index = usize::try_from(context.leader(0)).expect("leader index");
        let local_key = keys[local_index].clone();
        let local_peer = PeerId::new(local_key.public_key().clone());
        let nonzero = NonZeroUsize::new(8).expect("nonzero");
        let authenticated_genesis_nexus_amx_context = (height == 1).then(|| {
            AuthenticatedGenesisNexusAmxContext::Staged(StagedGenesisNexusAmxContext::for_test(
                context.nexus_amx_context_hash,
            ))
        });
        let adapter = V2LaneWorkAdapter::new_with_output_guard(
            context,
            local_peer,
            local_key,
            true,
            state,
            kura,
            V2LaneWorkLimits::new(nonzero, nonzero, nonzero, nonzero, nonzero, nonzero),
            authenticated_genesis_nexus_amx_context,
            None,
            ConsensusOutputGuard::isolated(),
        )
        .expect("open lane adapter");
        (adapter, keys)
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
            lane_drain_certificates: Vec::new(),
            execution_batch: None,
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
    fn restart_required_lane_persistence_rejects_before_any_mutation() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let entry = pending_sidecar_entry(&adapter, &keys, 1);
        let entry_hash = adapter
            .kura
            .persist_pending_certified_merge_entry(&entry)
            .expect("seed a pending sidecar before activating restart recovery");
        adapter
            .merge_claims
            .insert((entry.epoch_id, 1, 0), Hash::new(b"retained merge claim"));
        let claims_before = adapter.merge_claims.clone();

        adapter.output_guard.activate_restart_required();

        assert!(matches!(
            adapter.prune_finalized_merge_sidecars(),
            Err(V2LaneWorkError::RestartRequired)
        ));
        assert_eq!(
            adapter.merge_claims, claims_before,
            "post-latch lane cleanup must fail before in-memory mutation"
        );
        assert_eq!(
            adapter
                .kura
                .merge_entry_by_hash(entry_hash)
                .expect("read pending sidecar after denied cleanup"),
            Some(entry),
            "post-latch lane cleanup must perform no Kura mutation"
        );
    }

    #[test]
    fn lane_persistence_error_latches_restart_required_before_return() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        adapter
            .merge_claims
            .insert((1, 1, 0), Hash::new(b"retained merge claim"));
        let claims_before = adapter.merge_claims.clone();
        adapter.persistence_failure = Some("injected lane persistence failure".to_owned());
        let output_guard = Arc::clone(&adapter.output_guard);

        assert!(matches!(
            adapter.prune_finalized_merge_sidecars(),
            Err(V2LaneWorkError::Persistence(reason))
                if reason == "injected lane persistence failure"
        ));
        assert!(output_guard.restart_required());
        assert!(output_guard.acquire().is_none());
        assert_eq!(adapter.merge_claims, claims_before);
    }

    #[test]
    fn restart_activation_drains_inflight_lane_kura_persistence() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let entry = pending_sidecar_entry(&adapter, &keys, 1);
        let entry_hash = adapter
            .kura
            .persist_pending_certified_merge_entry(&entry)
            .expect("seed pending sidecar for persistence race");
        let output_guard = Arc::clone(&adapter.output_guard);
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let release = Arc::new(Barrier::new(2));
        adapter.persistence_pause = Some(LanePersistencePause {
            entered: entered_tx,
            release: Arc::clone(&release),
        });
        let persistence = thread::spawn(move || {
            let result = adapter.prune_finalized_merge_sidecars();
            (adapter, result)
        });
        entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("lane persistence acquired its output permit");

        let activation_guard = Arc::clone(&output_guard);
        let (activated_tx, activated_rx) = mpsc::sync_channel(1);
        let activation = thread::spawn(move || {
            activation_guard.activate_restart_required();
            activated_tx
                .send(())
                .expect("publish completed restart activation");
        });
        let deadline = Instant::now() + Duration::from_secs(1);
        while !output_guard.restart_required() && Instant::now() < deadline {
            thread::yield_now();
        }
        assert!(output_guard.restart_required());
        assert!(
            activated_rx
                .recv_timeout(Duration::from_millis(25))
                .is_err(),
            "restart activation must drain in-flight lane Kura persistence"
        );

        release.wait();
        let (mut adapter, result) = persistence.join().expect("join lane persistence worker");
        result.expect("already-admitted lane persistence completes before activation");
        activated_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("activation completes after lane persistence releases its permit");
        activation.join().expect("join restart activation worker");
        assert!(
            adapter
                .kura
                .merge_entry_by_hash(entry_hash)
                .expect("read pruned sidecar after drained persistence")
                .is_none()
        );
        assert!(matches!(
            adapter.prune_finalized_merge_sidecars(),
            Err(V2LaneWorkError::RestartRequired)
        ));
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
        adapter
            .refresh_merge_candidates(0)
            .expect("Decision-protected refresh is durability-safe");
        assert!(
            adapter.merge_entries.is_empty(),
            "no new merge candidate may survive after a durable Decision"
        );
    }

    #[test]
    fn merge_sidecar_deferral_rejects_partial_and_fetches_full_execution_projection() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: 1,
        };
        let parent_block_hash = adapter
            .context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash);
        let settlement = missing_sidecar_reference(&adapter, &keys, 1);
        let mut partial = settlement.clone();
        partial.result_merkle_root = Some(HashOf::from_untyped_unchecked(Hash::new(
            b"partial result root",
        )));
        let mut full = settlement;
        full.execution_batch_hash = Some(Hash::new(b"full batch"));
        full.entrypoint_count = Some(1);
        full.entrypoint_merkle_root = Some(HashOf::from_untyped_unchecked(Hash::new(
            b"full entrypoint root",
        )));
        full.result_merkle_root = Some(HashOf::from_untyped_unchecked(Hash::new(
            b"full result root",
        )));
        full.base_state_height = Some(0);
        full.base_state_hash = Some(HashOf::from_untyped_unchecked(Hash::new(
            b"full base state",
        )));

        let partial_subject = wire::BlockSubject {
            parent_block_hash,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"partial execution carrier")),
            payload_hash: Hash::new(b"partial execution payload"),
        };
        assert!(matches!(
            adapter
                .defer_missing_merge_sidecar(round, partial_subject, partial)
                .expect("partial projection rejection is non-fatal"),
            MergeSidecarDeferralDisposition::Rejected(reason)
                if reason == "certified merge reference has a partial execution projection"
        ));
        assert_eq!(adapter.merge_qc_preflight_checks, 0);

        let mut invalid_count = full.clone();
        invalid_count.entrypoint_count = Some(0);
        let invalid_count_subject = wire::BlockSubject {
            parent_block_hash,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"invalid execution count carrier",
            )),
            payload_hash: Hash::new(b"invalid execution count payload"),
        };
        assert!(matches!(
            adapter
                .defer_missing_merge_sidecar(round, invalid_count_subject, invalid_count)
                .expect("invalid execution count rejection is non-fatal"),
            MergeSidecarDeferralDisposition::Rejected(reason)
                if reason == "certified merge reference has an invalid execution entrypoint count"
        ));
        assert_eq!(adapter.merge_qc_preflight_checks, 0);

        let full_subject = wire::BlockSubject {
            parent_block_hash,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"full execution carrier")),
            payload_hash: Hash::new(b"full execution payload"),
        };
        assert_eq!(
            adapter
                .defer_missing_merge_sidecar(round, full_subject, full)
                .expect("complete execution projection is fetchable"),
            MergeSidecarDeferralDisposition::Fetching
        );
        assert_eq!(adapter.merge_qc_preflight_checks, 1);
        assert_eq!(adapter.drain_effects(usize::MAX).len(), 1);
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
            adapter.merge_qc_preflight_checks, checks_before_backpressure,
            "the deferred exact reference reuses the bounded positive QC authentication cache"
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

    #[test]
    fn fresh_genesis_requires_the_exact_authenticated_staged_nexus_projection() {
        let (adapter, _keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let mut context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let local_key = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        assert_eq!(
            context.nexus_amx_context_hash,
            super::super::v2_recovery::committed_nexus_amx_context_hash(state.as_ref()),
            "the default fresh-genesis fixture starts with the empty committed projection"
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
                None,
            ),
            Err(V2LaneWorkError::NexusContextMismatch)
        ));

        context.nexus_amx_context_hash = Hash::new(b"staged post-genesis Nexus/AMX projection");
        assert_ne!(
            context.nexus_amx_context_hash,
            super::super::v2_recovery::committed_nexus_amx_context_hash(state.as_ref()),
            "fixture must distinguish the staged post-genesis projection from empty committed state"
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
                None,
            ),
            Err(V2LaneWorkError::NexusContextMismatch)
        ));
        assert!(matches!(
            V2LaneWorkAdapter::new_with_output_guard(
                context.clone(),
                local_peer.clone(),
                local_key.clone(),
                true,
                Arc::clone(&state),
                Arc::clone(&kura),
                limits,
                Some(AuthenticatedGenesisNexusAmxContext::Staged(
                    StagedGenesisNexusAmxContext::for_test(Hash::new(
                        b"different staged Nexus/AMX projection",
                    )),
                )),
                None,
                ConsensusOutputGuard::isolated(),
            ),
            Err(V2LaneWorkError::NexusContextMismatch)
        ));

        V2LaneWorkAdapter::new_with_output_guard(
            context.clone(),
            local_peer,
            local_key,
            true,
            state,
            kura,
            limits,
            Some(AuthenticatedGenesisNexusAmxContext::Staged(
                StagedGenesisNexusAmxContext::for_test(context.nexus_amx_context_hash),
            )),
            None,
            ConsensusOutputGuard::isolated(),
        )
        .expect("the exact staged-genesis capability opens height one");
    }

    #[test]
    fn staged_genesis_nexus_capability_cannot_open_a_successor_height() {
        let (adapter, _keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 2);
        let context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let local_key = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        assert!(matches!(
            V2LaneWorkAdapter::new_with_output_guard(
                context.clone(),
                local_peer.clone(),
                local_key.clone(),
                true,
                Arc::clone(&state),
                Arc::clone(&kura),
                limits,
                Some(AuthenticatedGenesisNexusAmxContext::Staged(
                    StagedGenesisNexusAmxContext::for_test(context.nexus_amx_context_hash),
                )),
                None,
                ConsensusOutputGuard::isolated(),
            ),
            Err(V2LaneWorkError::NexusContextMismatch)
        ));

        let mut drifted_context = context;
        drifted_context.nexus_amx_context_hash =
            Hash::new(b"drifted successor Nexus/AMX projection");
        assert!(matches!(
            V2LaneWorkAdapter::new(
                drifted_context,
                local_peer,
                local_key,
                true,
                state,
                kura,
                limits,
                None,
            ),
            Err(V2LaneWorkError::NexusContextMismatch)
        ));
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
        let lane_incarnation = adapter
            .state
            .lane_incarnation_at_height(LaneId::SINGLE, adapter.context.height)
            .expect("fixture lane incarnation");
        let (validator_set, min_signers) = adapter
            .native_committee_shape_for_route(
                LaneId::SINGLE,
                DataSpaceId::UNIVERSAL,
                adapter.context.height,
            )
            .expect("fixture native AMX committee");
        let mut body = NativeAmxAttestationBodyV2 {
            round: wire::ConsensusRound {
                context_id: adapter.context.id(),
                height: adapter.context.height,
                view: 0,
            },
            epoch: adapter.context.epoch,
            chain_id_hash: adapter.native_chain_id_hash(),
            source_id: [0xA5; Hash::LENGTH],
            tx_entrypoint_hash: HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
                b"entrypoint",
            )),
            plan_digest: Hash::new(b"plan"),
            phase: NativeAmxPhase::Prepare,
            coordinator_lane_id: LaneId::SINGLE,
            coordinator_dataspace_id: DataSpaceId::UNIVERSAL,
            coordinator_lane_incarnation: lane_incarnation,
            participant_lane_id: LaneId::SINGLE,
            participant_dataspace_id: DataSpaceId::UNIVERSAL,
            participant_lane_incarnation: lane_incarnation,
            participant_previous_block_height: 0,
            participant_previous_block_descriptor_hash: None,
            participant_lane_block_height: 1,
            participant_lane_block_view: 0,
            participant_proposal_hash: Hash::new(b"native-amx-test-participant-proposal"),
            participant_settlement_commitment: Hash::prehashed([0; Hash::LENGTH]),
            participant_validator_set_hash: HashOf::new(&validator_set),
            participant_validator_count: u32::try_from(validator_set.len())
                .expect("fixture validator count"),
            participant_min_quorum: u32::try_from(min_signers).expect("fixture quorum"),
            authority_context_height: adapter.context.height,
            planned_coordinator_block_height: 1,
            coordinator_lane_block_view: 0,
            coordinator_proposal_hash: Hash::new(b"native-amx-test-coordinator-proposal"),
        };
        body.participant_settlement_commitment = body.computed_participant_settlement_commitment();
        body
    }

    fn native_request(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
    ) -> NativeAmxAttestationRequestV2 {
        let mut body = native_body(adapter);
        let coordinator =
            RoutingDecision::new(body.coordinator_lane_id, body.coordinator_dataspace_id);
        let participant =
            RoutingDecision::new(body.participant_lane_id, body.participant_dataspace_id);
        let plan = RoutingPlan::native_amx(
            coordinator,
            vec![RouteLeg::new(participant, RouteLegRole::Participant)],
        );
        body.plan_digest = plan.digest();

        let mut ordered_keys = keys.to_vec();
        ordered_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let base = proposal_for_route(
            adapter,
            &ordered_keys,
            body.coordinator_lane_id,
            body.coordinator_dataspace_id,
            body.coordinator_lane_incarnation,
            body.authority_context_height,
            body.planned_coordinator_block_height,
        );
        let mut ownership = ownership_from_proposal(&base);
        ownership.accepted_transaction_hashes = vec![Hash::from(body.tx_entrypoint_hash)];
        let replay = ownership
            .compute_replay_hashes()
            .expect("fixture native AMX ownership replay material");
        ownership.subject_hash = replay.subject_hash;
        ownership.payload_ownership_hash = replay.payload_ownership_hash;
        ownership.rbc_instance_hash = replay.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
        let proposal = proposal_from_ownership(
            &ownership,
            HashOf::from_untyped_unchecked(Hash::new(b"native-amx-test-proposal-hint")),
        )
        .expect("fixture native AMX proposal");
        body.coordinator_proposal_hash = proposal.proposal_hash;
        body.participant_proposal_hash = proposal.proposal_hash;
        let participant_settlement = body.computed_participant_settlement();
        body.participant_settlement_commitment = Hash::from(
            iroha_data_model::nexus::compute_settlement_hash(&participant_settlement)
                .expect("fixture native AMX settlement hash"),
        );
        NativeAmxAttestationRequestV2 {
            body,
            plan_legs: plan.legs(),
            coordinator_proposal: proposal.clone(),
            participant_proposal: proposal,
            participant_settlement,
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
        assert_eq!(
            adapter
                .native_signing_guard
                .as_ref()
                .expect("validator has durable Native AMX guard")
                .record_count_for_test(),
            1,
            "the exact retransmission must reuse one durable signing decision"
        );

        adapter.local_native_claims.clear();

        let mut conflicting = body;
        conflicting.tx_entrypoint_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"conflicting entrypoint"));
        assert!(
            adapter.sign_native_vote_once(conflicting).is_none(),
            "an honest adapter must not sign a second body for one round/session/leg/phase"
        );
        assert!(
            adapter.local_native_claims.is_empty(),
            "durable rejection must survive loss of the volatile fast-path claim"
        );

        let commit = NativeAmxAttestationBodyV2 {
            phase: NativeAmxPhase::Commit,
            ..body
        };
        assert!(
            adapter.sign_native_vote_once(commit).is_some(),
            "Prepare and Commit are distinct durable claims"
        );
        assert_eq!(
            adapter
                .native_signing_guard
                .as_ref()
                .expect("validator has durable Native AMX guard")
                .record_count_for_test(),
            2
        );
    }

    #[test]
    fn native_amx_signing_guard_reopens_same_height_without_losing_claims() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let body = native_body(&adapter);
        let first = adapter
            .sign_native_vote_once(body)
            .expect("first body is durably signable");

        let context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let key_pair = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        let mut reopened = V2LaneWorkAdapter::new_with_output_guard(
            context,
            local_peer,
            key_pair,
            true,
            state,
            kura,
            limits,
            None,
            None,
            ConsensusOutputGuard::isolated(),
        )
        .expect("reopen adapter against the exact durable height context");
        assert_eq!(
            reopened
                .sign_native_vote_once(body)
                .expect("exact durable replay remains signable"),
            first
        );
        reopened.local_native_claims.clear();
        let mut conflicting = body;
        conflicting.tx_entrypoint_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"restart-conflicting-entrypoint"));
        assert!(reopened.sign_native_vote_once(conflicting).is_none());
        assert_eq!(
            reopened
                .native_signing_guard
                .as_ref()
                .expect("reopened validator has durable guard")
                .record_count_for_test(),
            1
        );
    }

    #[test]
    fn unsafe_native_amx_signing_journal_latches_consensus_fail_stop() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let body = native_body(&adapter);
        adapter
            .sign_native_vote_once(body)
            .expect("seed one durable signing decision");
        adapter.local_native_claims.clear();
        adapter
            .native_signing_guard
            .as_ref()
            .expect("validator has durable guard")
            .remove_one_record_for_test();

        assert!(adapter.sign_native_vote_once(body).is_none());
        assert!(adapter.output_guard.restart_required());
        assert!(
            adapter.sign_native_vote_once(body).is_none(),
            "a poisoned process must never sign again"
        );
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
    fn retransmission_classes_rotate_fairly_at_capacity_one() {
        let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
        let (_, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        assert_eq!(
            adapter.lane_sessions.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        adapter
            .locally_bound_lane_proposals
            .insert(proposal.proposal_hash);

        let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
        adapter
            .retain_merge_sidecars_for_global_view(candidate.view, None, None)
            .expect("install exact unlocked reducer directive");
        assert!(adapter.drain_effects(usize::MAX).iter().any(|effect| {
            matches!(effect, V2LaneWorkEffect::BroadcastMerge(signature) if signature.message_digest
            == crate::merge::merge_qc_message_digest(
                &adapter.context.chain_id,
                &candidate,
                VALIDATOR_SET_HASH_VERSION_V1,
                adapter.frozen_validator_set_hash(),
            ))
        }));

        let request = native_request(&adapter, &keys);
        let body = request.body;
        let peer = adapter
            .context
            .roster
            .iter()
            .map(|entry| &entry.validator)
            .find(|peer| *peer != &adapter.local_peer)
            .expect("fixture has a remote validator")
            .clone();
        adapter.native_requests.insert(
            NativeRequestKey {
                body,
                peer: peer.clone(),
            },
            NativeAmxMessage::PrepareRequest(request),
        );
        adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("non-zero capacity");

        adapter
            .schedule_retransmission()
            .expect("schedule lane retransmission");
        assert!(matches!(
            adapter.drain_effects(usize::MAX).as_slice(),
            [V2LaneWorkEffect::PostLaneBlock { .. }]
        ));
        adapter
            .schedule_retransmission()
            .expect("schedule Native AMX retransmission");
        assert!(matches!(
            adapter.drain_effects(usize::MAX).as_slice(),
            [V2LaneWorkEffect::PostNativeAmx { .. }]
        ));
        adapter
            .schedule_retransmission()
            .expect("schedule merge retransmission");
        assert!(matches!(
            adapter.drain_effects(usize::MAX).as_slice(),
            [V2LaneWorkEffect::BroadcastMerge(_)]
        ));
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
            adapter.bind_local_candidate(round_zero, block_zero.hash()),
            V2LaneIngressOutcome::Inserted
        );
        adapter
            .schedule_retransmission()
            .expect("schedule pre-lock retransmission");
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
        adapter
            .schedule_retransmission()
            .expect("schedule locked-body retransmission");
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
            lane_drain_certificates: Vec::new(),
            execution_batch: None,
            global_state_root: crate::merge::reduce_merge_hint_roots(&[]),
        }
    }

    #[test]
    fn merge_candidate_selection_preserves_authorized_digest_and_relay_priority() {
        let relay_digest = Hash::new(b"relay candidate");
        let installed_digest = Hash::new(b"installed candidate");
        let digest = |candidate: &(u8, Hash)| candidate.1;

        assert_eq!(
            preferred_merge_candidates(
                None,
                vec![(2, relay_digest)],
                vec![(3, installed_digest)],
                digest,
            ),
            vec![(2, relay_digest)],
            "first-release production signs only reachable relay-settlement candidates"
        );
        assert_eq!(
            preferred_merge_candidates(
                Some(relay_digest),
                vec![(2, relay_digest)],
                vec![(3, installed_digest)],
                digest,
            ),
            vec![(2, relay_digest)],
            "a durable signing decision must survive later candidate installation"
        );
        assert!(
            preferred_merge_candidates(
                Some(Hash::new(b"unavailable authorized candidate")),
                vec![(2, relay_digest)],
                vec![(3, installed_digest)],
                digest,
            )
            .is_empty(),
            "an unavailable durable decision must fail closed instead of selecting another digest"
        );
    }

    #[test]
    fn installed_execution_candidate_never_reaches_local_signing() {
        let (mut adapter, _) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
        adapter
            .retain_merge_sidecars_for_global_view(0, None, None)
            .expect("install exact unlocked reducer directive");
        adapter.drain_effects(usize::MAX);

        let mut candidate = merge_candidate_for_persistence_retry(&adapter, 0);
        candidate.execution_batch = Some(iroha_data_model::merge::MergeExecutionBatch {
            version: 1,
            base_state_height: adapter.context.height.saturating_sub(1),
            base_state_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"retired execution base state",
            )),
            application_block_header: BlockHeader::new(
                NonZeroU64::new(adapter.context.height).expect("non-zero carrier height"),
                Some(candidate.carrier_parent_hash),
                None,
                None,
                1,
                candidate.view,
            ),
            lanes: Vec::new(),
            entrypoint_count: 1,
            entrypoint_merkle_root: HashOf::from_untyped_unchecked(Hash::new(
                b"retired execution entrypoints",
            )),
            result_merkle_root: HashOf::from_untyped_unchecked(Hash::new(
                b"retired execution results",
            )),
            execution_root: Hash::new(b"retired execution root"),
            application_write_set_root: Hash::new(b"retired execution application writes"),
            write_set_root: Hash::new(b"retired execution writes"),
            expected_post_state_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"retired execution post state",
            )),
            batch_hash: Hash::new(b"retired execution batch"),
        });
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
        adapter.merge_entries.insert(
            key,
            PendingMerge {
                stage: PendingMergeStage::Collecting(candidate),
                signatures: BTreeMap::new(),
            },
        );

        adapter
            .refresh_merge_candidates(0)
            .expect("retired execution candidate fails closed without signing");
        assert!(adapter.merge_entries[&key].signatures.is_empty());
        assert!(
            adapter
                .drain_effects(usize::MAX)
                .iter()
                .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_))),
            "an installed execution-batch candidate must not reach the private key"
        );
    }

    fn merge_signing_context_for_test(
        adapter: &V2LaneWorkAdapter,
        candidate: &crate::merge::MergeLedgerCandidate,
    ) -> MergeSigningContextV1 {
        MergeSigningContextV1 {
            epoch_id: candidate.epoch_id,
            view: candidate.view,
            carrier_height: candidate.carrier_height,
            parent_hash: candidate.carrier_parent_hash,
            validator_set_hash: adapter.frozen_validator_set_hash(),
        }
    }

    #[test]
    fn durable_local_merge_claim_rejects_same_context_candidate_drift() {
        let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
        let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
        let signer = adapter
            .local_validator_index()
            .expect("fixture local validator is in the frozen roster");
        let first_digest = crate::merge::merge_qc_message_digest(
            &adapter.context.chain_id,
            &candidate,
            VALIDATOR_SET_HASH_VERSION_V1,
            adapter.frozen_validator_set_hash(),
        );
        adapter
            .retain_merge_sidecars_for_global_view(0, None, None)
            .expect("install exact unlocked reducer directive");
        assert_eq!(
            adapter
                .merge_claims
                .get(&(candidate.epoch_id, candidate.view, signer)),
            Some(&first_digest)
        );

        let mut drifted = candidate.clone();
        drifted.global_state_root = Hash::new(b"same-context conflicting merge payload");
        let drifted_digest = crate::merge::merge_qc_message_digest(
            &adapter.context.chain_id,
            &drifted,
            VALIDATOR_SET_HASH_VERSION_V1,
            adapter.frozen_validator_set_hash(),
        );
        assert_ne!(first_digest, drifted_digest);
        assert_eq!(
            adapter.authorize_local_merge_claim(&drifted, 0, signer, drifted_digest),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
        assert_eq!(
            adapter
                .merge_claims
                .get(&(candidate.epoch_id, candidate.view, signer)),
            Some(&first_digest),
            "a conflicting candidate must never overwrite the in-memory decision"
        );
        assert_eq!(
            adapter
                .merge_signing_guard
                .authorized_digest(&merge_signing_context_for_test(&adapter, &candidate))
                .expect("read durable exact-context decision"),
            Some(first_digest),
            "a conflicting candidate must never overwrite the durable decision"
        );
    }

    #[test]
    fn durable_local_merge_claim_rejects_conflict_after_adapter_reopen() {
        let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
        let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
        let signer = adapter
            .local_validator_index()
            .expect("fixture local validator is in the frozen roster");
        let first_digest = crate::merge::merge_qc_message_digest(
            &adapter.context.chain_id,
            &candidate,
            VALIDATOR_SET_HASH_VERSION_V1,
            adapter.frozen_validator_set_hash(),
        );
        adapter
            .retain_merge_sidecars_for_global_view(0, None, None)
            .expect("authorize pre-restart merge decision from the unlocked directive");

        let context = adapter.context.clone();
        let local_peer = adapter.local_peer.clone();
        let key_pair = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        let mut reopened = V2LaneWorkAdapter::new(
            context, local_peer, key_pair, true, state, kura, limits, None,
        )
        .expect("reopen adapter against the same committed frontier");
        assert!(
            reopened
                .drain_effects(usize::MAX)
                .iter()
                .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_))),
            "constructor must not emit a merge share before reducer recovery"
        );
        reopened
            .retain_merge_sidecars_for_global_view(0, None, None)
            .expect("install reopened exact unlocked directive");
        assert!(
            reopened
                .drain_effects(usize::MAX)
                .iter()
                .any(|effect| matches!(effect, V2LaneWorkEffect::BroadcastMerge(signature) if signature.message_digest == first_digest)),
            "the exact unlocked directive may release the recovered candidate share"
        );
        reopened.merge_claims.clear();
        reopened.merge_entries.clear();
        reopened.purge_queued_merge_broadcasts();
        let mut drifted = candidate.clone();
        drifted.global_state_root = Hash::new(b"post-restart conflicting merge payload");
        let drifted_digest = crate::merge::merge_qc_message_digest(
            &reopened.context.chain_id,
            &drifted,
            VALIDATOR_SET_HASH_VERSION_V1,
            reopened.frozen_validator_set_hash(),
        );
        assert_eq!(
            reopened.authorize_local_merge_claim(&drifted, 0, signer, drifted_digest),
            Err(MergeSidecarError::LocalSigningEquivocation)
        );
        assert!(
            reopened
                .merge_claims
                .get(&(candidate.epoch_id, candidate.view, signer))
                .is_none(),
            "restart rejection must not manufacture a conflicting in-memory claim"
        );
        assert_eq!(
            reopened
                .merge_signing_guard
                .authorized_digest(&merge_signing_context_for_test(&reopened, &candidate))
                .expect("read restarted durable decision"),
            Some(first_digest)
        );
    }

    #[test]
    fn locked_later_view_directive_purges_queued_merge_shares_and_disables_retry() {
        let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
        let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
        adapter
            .retain_merge_sidecars_for_global_view(0, None, None)
            .expect("install initial unlocked directive");
        assert!(adapter.effects.iter().any(
            |effect| matches!(effect, V2LaneWorkEffect::BroadcastMerge(signature) if signature.view == 0)
        ));

        let locked = wire::BlockSubject {
            parent_block_hash: Some(candidate.carrier_parent_hash),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"locked later-view carrier")),
            payload_hash: Hash::new(b"locked later-view payload"),
        };
        adapter
            .retain_merge_sidecars_for_global_view(1, Some(locked), None)
            .expect("install locked later-view directive");
        assert!(adapter.merge_entries.is_empty());
        assert!(adapter.merge_claims.is_empty());
        assert!(
            adapter
                .effects
                .iter()
                .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
        );
        adapter
            .schedule_retransmission()
            .expect("schedule locked-view retransmission");
        assert!(
            adapter
                .drain_effects(usize::MAX)
                .iter()
                .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
        );
    }

    fn record_production_merge_candidate_for_persistence_retry(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
        view: wire::View,
    ) -> crate::merge::MergeLedgerCandidate {
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let lane_height = 1;
        let header = BlockHeader::new(
            NonZeroU64::new(lane_height).expect("non-zero lane height"),
            None,
            None,
            None,
            1_700_000_000_000,
            0,
        );

        // Relay admission requires committee members to be present in both
        // the exact frozen commit topology and World. The v2 adapter fixture
        // seeds the key registry directly and commits synthetic parent blocks,
        // so complete that production authority tuple before constructing
        // authenticated relay evidence.
        {
            let mut topology = adapter.state.commit_topology.block();
            topology.clear();
            for entry in &adapter.context.roster {
                topology.push(entry.validator.clone());
            }
            topology.commit();
        }
        let mut world_block = adapter.state.world.block();
        {
            let mut peers = world_block.peers_mut_for_testing().transaction();
            for key in keys {
                let peer = PeerId::new(key.public_key().clone());
                if !peers.iter().any(|existing| existing == &peer) {
                    peers.push(peer);
                }
            }
            peers.apply();
        }
        world_block.commit();

        let validators = keys
            .iter()
            .map(|key| AccountId::new(key.public_key().clone()))
            .collect::<Vec<_>>();
        let validator_bindings = validators
            .iter()
            .zip(keys)
            .map(|(validator, key)| ManifestValidatorBinding {
                validator: validator.clone(),
                peer_id: PeerId::new(key.public_key().clone()),
                torii_url: None,
            })
            .collect::<Vec<_>>();
        let status = LaneManifestStatus {
            lane: lane_id,
            alias: "default".to_owned(),
            dataspace: dataspace_id,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("parliament".to_owned()),
            manifest_path: Some(std::path::PathBuf::from(
                "/tmp/v2-merge-persistence-retry-manifest.json",
            )),
            governance_rules: Some(GovernanceRules {
                validators,
                validator_bindings,
                ..GovernanceRules::default()
            }),
            privacy_commitments: Vec::new(),
        };
        adapter
            .state
            .install_lane_manifests(&Arc::new(LaneManifestRegistry::from_statuses(
                BTreeMap::from([(lane_id, status)]),
            )));

        // Mirror the production relay-committee ranking. The fixture has the
        // exact 3f+1 topology, so every live validator is selected while this
        // ordering remains consensus-significant in the embedded QC.
        let epoch_seed = crate::sumeragi::npos_seed_for_height_from_world(
            &adapter.state.world.view(),
            &adapter.context.chain_id,
            lane_height,
        );
        let mut seed_preimage = Vec::new();
        seed_preimage.extend_from_slice(b"iroha:lane-relay:committee-seed:v1");
        seed_preimage.extend_from_slice(&epoch_seed);
        seed_preimage.extend_from_slice(&dataspace_id.as_u64().to_le_bytes());
        seed_preimage.extend_from_slice(&lane_id.as_u32().to_le_bytes());
        let committee_seed: [u8; 32] = Hash::new(seed_preimage).into();
        let mut ranked = adapter
            .state
            .commit_topology_snapshot()
            .into_iter()
            .map(|peer| {
                let mut member_preimage = Vec::new();
                member_preimage.extend_from_slice(b"iroha:lane-relay:committee-member:v1");
                member_preimage.extend_from_slice(&committee_seed);
                member_preimage.extend(
                    norito::to_bytes(&peer).expect("encode relay committee member for ranking"),
                );
                (Hash::new(member_preimage), peer)
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
        let committee = ranked.into_iter().map(|(_, peer)| peer).collect::<Vec<_>>();
        assert_eq!(
            committee.len(),
            keys.len(),
            "fixture must provide exact 3f+1 relay committee"
        );

        let mode_tag = LaneRelayEnvelope::lane_qc_mode_tag_for(
            lane_id,
            dataspace_id,
            crate::sumeragi::consensus::PERMISSIONED_TAG,
        );
        let parent_state_root = Hash::new(b"v2 merge retry parent state");
        let post_state_root = Hash::new(b"v2 merge retry post state");
        let mut qc = crate::sumeragi::consensus::Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: header.hash(),
            parent_state_root,
            post_state_root,
            height: lane_height,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: mode_tag.clone(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&committee),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: committee,
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap: Vec::new(),
                bls_aggregate_signature: Vec::new(),
            },
        };
        let vote = crate::sumeragi::consensus::Vote {
            phase: qc.phase,
            block_hash: qc.subject_block_hash,
            parent_state_root: qc.parent_state_root,
            post_state_root: qc.post_state_root,
            height: qc.height,
            view: qc.view,
            epoch: qc.epoch,
            chain_order_hash: qc.chain_order_hash,
            rechain_seq: qc.rechain_seq,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let preimage =
            crate::sumeragi::consensus::vote_preimage(&adapter.context.chain_id, &mode_tag, &vote);
        let signatures = keys
            .iter()
            .map(|key| {
                Signature::try_new(key.private_key(), &preimage)
                    .expect("sign production-valid relay QC")
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
        qc.aggregate = crate::sumeragi::consensus::QcAggregate {
            signers_bitmap: vec![(1_u8 << keys.len()) - 1],
            bls_aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                .expect("aggregate production-valid relay QC"),
        };

        let settlement = LaneBlockCommitment {
            block_height: lane_height,
            lane_id,
            lane_incarnation: adapter
                .state
                .lane_incarnation_at_height(lane_id, lane_height)
                .expect("fixture lane incarnation is active"),
            dataspace_id,
            tx_count: 0,
            total_local_amount: "0".parse().expect("valid settlement quantity"),
            total_xor_due: "0".parse().expect("valid settlement quantity"),
            total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
            total_xor_variance: "0".parse().expect("valid settlement quantity"),
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };
        let envelope = LaneRelayEnvelope::new(header, Some(qc), None, settlement, 0)
            .expect("construct production-valid relay envelope")
            .with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
                proof_digest: Hash::new(b"v2 merge persistence retry FastPQ proof"),
                verified_at_height: lane_height,
            }));
        adapter
            .state
            .record_lane_relay(&envelope)
            .expect("production relay admission accepts retry fixture");
        let candidates = adapter
            .state
            .merge_entry_candidates_from_lane_relays_for_view(view);
        assert_eq!(
            candidates.len(),
            1,
            "one admitted relay yields one candidate"
        );
        candidates
            .into_iter()
            .next()
            .expect("relay merge candidate")
    }

    #[test]
    fn merge_signing_rejects_wrong_round_context_and_post_apply_state() {
        let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
        let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
        let signer = adapter
            .local_validator_index()
            .expect("fixture local validator is in the frozen roster");
        adapter
            .retain_merge_sidecars_for_global_view(0, None, None)
            .expect("install exact unlocked directive");
        adapter.drain_effects(usize::MAX);

        let mut wrong_view = candidate.clone();
        wrong_view.view = wrong_view.view.saturating_add(1);
        let mut wrong_height = candidate.clone();
        wrong_height.carrier_height = wrong_height.carrier_height.saturating_add(1);
        let mut wrong_parent = candidate.clone();
        wrong_parent.carrier_parent_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"wrong merge signing parent"));
        for (label, drifted) in [
            ("view", wrong_view),
            ("height", wrong_height),
            ("parent", wrong_parent),
        ] {
            let digest = crate::merge::merge_qc_message_digest(
                &adapter.context.chain_id,
                &drifted,
                VALIDATOR_SET_HASH_VERSION_V1,
                adapter.frozen_validator_set_hash(),
            );
            assert_eq!(
                adapter.authorize_local_merge_claim(&drifted, 0, signer, digest),
                Err(MergeSidecarError::LocalSigningEquivocation),
                "wrong {label} must fail before private-key use"
            );
        }
        assert!(
            adapter
                .effects
                .iter()
                .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
        );

        let applied = test_block(
            adapter.context.height,
            Some(candidate.carrier_parent_hash),
            None,
            &keys[0],
        );
        adapter
            .kura
            .store_block(applied.clone())
            .expect("persist exact post-apply carrier");
        let committed = ValidBlock::committed_from_replay_signed_block(applied);
        commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
        let digest = crate::merge::merge_qc_message_digest(
            &adapter.context.chain_id,
            &candidate,
            VALIDATOR_SET_HASH_VERSION_V1,
            adapter.frozen_validator_set_hash(),
        );
        assert_eq!(
            adapter.authorize_local_merge_claim(&candidate, 0, signer, digest),
            Err(MergeSidecarError::LocalSigningEquivocation),
            "post-apply recovery must never authorize another share"
        );
        adapter
            .refresh_merge_candidates(0)
            .expect("post-apply refresh remains signing-silent");
        adapter
            .schedule_retransmission()
            .expect("schedule post-apply retransmission");
        assert!(
            adapter
                .drain_effects(usize::MAX)
                .iter()
                .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_)))
        );
    }

    #[test]
    fn merge_signing_rejects_block_first_kura_ahead_crash_image() {
        let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
        let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
        let signer = adapter
            .local_validator_index()
            .expect("fixture local validator is in the frozen roster");
        let digest = crate::merge::merge_qc_message_digest(
            &adapter.context.chain_id,
            &candidate,
            VALIDATOR_SET_HASH_VERSION_V1,
            adapter.frozen_validator_set_hash(),
        );
        let durable_carrier = test_block(
            adapter.context.height,
            Some(candidate.carrier_parent_hash),
            None,
            &keys[0],
        );
        adapter
            .kura
            .store_block(durable_carrier)
            .expect("persist block-first carrier without advancing State");

        adapter
            .retain_merge_sidecars_for_global_view(candidate.view, None, None)
            .expect("install exact unlocked reducer directive");
        assert!(
            adapter
                .drain_effects(usize::MAX)
                .iter()
                .all(|effect| !matches!(effect, V2LaneWorkEffect::BroadcastMerge(_))),
            "a Kura-ahead crash image must not release a private-key operation"
        );
        assert!(matches!(
            adapter.authorize_local_merge_claim(&candidate, candidate.view, signer, digest),
            Err(MergeSidecarError::SigningGuard(message))
                if message.contains("identical committed State and durable Kura frontiers")
        ));
        assert_eq!(
            adapter
                .merge_signing_guard
                .authorized_digest(&merge_signing_context_for_test(&adapter, &candidate))
                .expect("read durable signing guard"),
            None
        );
    }

    #[test]
    fn same_round_merge_claims_survive_successful_kura_staging() {
        let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
        let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
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
        let local_index = adapter
            .local_validator_index()
            .expect("fixture local validator is in the frozen roster");
        adapter
            .retain_merge_sidecars_for_global_view(candidate.view, None, None)
            .expect("install exact unlocked reducer directive");
        assert_eq!(
            adapter
                .merge_claims
                .get(&(candidate.epoch_id, candidate.view, local_index)),
            Some(&digest),
            "local claim must be recorded before its signature is produced"
        );

        let mut accepted_remote_signers = Vec::new();
        for (index, key_pair) in keys.iter().enumerate() {
            let signer = u32::try_from(index).expect("fixture signer index fits u32");
            if signer == local_index {
                continue;
            }
            let signature = Signature::try_new(key_pair.private_key(), digest.as_ref())
                .expect("sign remote merge share")
                .payload()
                .to_vec();
            assert_eq!(
                adapter
                    .accept_merge_signature(
                        MergeCommitteeSignature {
                            epoch_id: candidate.epoch_id,
                            view: candidate.view,
                            signer,
                            message_digest: digest,
                            bls_sig: signature,
                        },
                        candidate.view,
                    )
                    .expect("persist remote merge signature"),
                V2LaneIngressOutcome::Inserted
            );
            accepted_remote_signers.push(signer);
            if !adapter.merge_entries.contains_key(&key) {
                break;
            }
        }
        assert!(
            !adapter.merge_entries.contains_key(&key),
            "fixture shares must form quorum and publish the certified entry"
        );
        for signer in std::iter::once(local_index).chain(accepted_remote_signers) {
            assert_eq!(
                adapter
                    .merge_claims
                    .get(&(candidate.epoch_id, candidate.view, signer)),
                Some(&digest),
                "Kura staging must not reopen any same-round signer decision"
            );
        }
        let (_, staged) = adapter
            .kura
            .select_pending_certified_merge_entry()
            .expect("read pending certified merge entry")
            .expect("quorum must stage one exact merge entry");
        assert_eq!(staged.merge_qc.message_digest, digest);
    }

    #[test]
    fn quorate_merge_persistence_failure_latches_restart_required() {
        let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
        let candidate = record_production_merge_candidate_for_persistence_retry(&adapter, &keys, 0);
        adapter
            .retain_merge_sidecars_for_global_view(candidate.view, None, None)
            .expect("install exact unlocked reducer directive");
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
        adapter.merge_entries.insert(
            key,
            PendingMerge {
                stage: PendingMergeStage::Collecting(candidate.clone()),
                signatures,
            },
        );

        let pending_dir = adapter.kura.store_root().join("pending_merge_entries");
        if pending_dir.is_dir() {
            std::fs::remove_dir(&pending_dir)
                .expect("remove empty pending sidecar directory before obstruction");
        }
        std::fs::write(&pending_dir, b"temporarily block pending sidecar directory")
            .expect("install transient Kura obstruction");
        assert!(matches!(
            adapter.schedule_retransmission(),
            Err(V2LaneWorkError::Persistence(_))
        ));
        assert!(
            adapter.merge_entries.contains_key(&key),
            "failed Kura publication must retain the complete quorum"
        );
        let certified_entry = match &adapter.merge_entries[&key].stage {
            PendingMergeStage::Certified(entry) => entry.clone(),
            PendingMergeStage::Collecting(_) => {
                panic!("production quorum must advance to Certified before Kura publication")
            }
        };
        assert_eq!(certified_entry.merge_qc.message_digest, key.digest);
        assert_eq!(certified_entry.epoch_id, candidate.epoch_id);
        assert_eq!(certified_entry.lane_snapshots, candidate.lane_snapshots);
        assert_eq!(certified_entry.active_lanes, candidate.active_lanes);
        let certified_hash = crate::merge::merge_ledger_entry_hash(&certified_entry);
        std::fs::remove_file(&pending_dir).expect("remove transient Kura obstruction");

        assert!(
            adapter.output_guard.restart_required(),
            "failed durable publication must poison this process before it can sign again"
        );
        assert!(matches!(
            adapter.schedule_retransmission(),
            Err(V2LaneWorkError::RestartRequired)
        ));
        assert_eq!(
            adapter
                .kura
                .merge_entry_by_hash(certified_hash)
                .expect("read exact unpublished merge entry"),
            None,
            "a poisoned process must not retry durable publication"
        );
    }

    #[test]
    fn merge_signature_state_is_bound_to_the_active_global_view() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let stale_digest = Hash::new(b"stale merge claim");
        adapter.merge_claims.insert((7, 0, 0), stale_digest);
        adapter
            .retain_merge_sidecars_for_global_view(1, None, None)
            .expect("install next unlocked reducer view");
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
            adapter
                .accept_merge_signature(stale, 1)
                .expect("reject stale remote signature without local durability work"),
            V2LaneIngressOutcome::Rejected
        );
        assert!(adapter.merge_claims.is_empty());
        assert!(adapter.merge_entries.is_empty());
    }
}
