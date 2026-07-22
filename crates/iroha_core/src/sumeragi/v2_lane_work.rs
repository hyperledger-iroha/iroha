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
            CertPhase, LaneBlockCertificateV1, LaneBlockCommitment, LaneBlockDescriptorV1,
            LaneBlockProposalPayloadHintV1, LaneBlockProposalV1, LaneBlockQcV1,
            LaneSettlementReceipt, NativeAmxAttestationBodyV2, NativeAmxAttestationQcV2,
            NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt, SumeragiLanePayloadOwnership,
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
#[cfg(test)]
use iroha_p2p::network::NetworkReplyRouteError;
#[cfg(test)]
use iroha_p2p::network::NetworkReplyRouteTestFixture;
use iroha_p2p::network::{NetworkReplyRoute, NetworkReplyRoutes};
use iroha_primitives::numeric::Quantity;
use norito::codec::Encode as _;
use thiserror::Error;

use super::{
    FairV2IngressOwnershipEvidence, InboundBlockMessage, LaneRelayMessage,
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
    kura::{
        CertifiedLaneBlockArtifact, Kura, LaneBlockApplicationReceiptArtifact,
        LaneBlockPayloadAvailability, sumeragi_v2_validator_storage_supported,
    },
    lane_consensus::{
        CommittedLaneBlockSession, LaneBlockSessionCache, LaneBlockSessionInsertOutcome,
        LaneBlockVoteV1, validate_committed_lane_block_session, validate_lane_block_proposal,
        validate_lane_block_qc, validate_lane_block_qc_aggregate,
    },
    merge_sidecar::{
        CERTIFIED_MERGE_SIDECAR_VERSION_V1, CertifiedMergeSidecarChunkAdmission,
        CertifiedMergeSidecarMessage, ChunkIngestOutcome, MergeSidecarError, MergeSidecarPost,
        MergeSidecarTransport, MergeSigningContextV1, MergeSigningGuard,
        certified_merge_reference_digest, certified_merge_sidecar_holders,
        decode_certified_merge_sidecar,
    },
    native_amx::{
        MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD, NativeAmxAttestationRequestV2,
        NativeAmxCommitRequestV2, NativeAmxMessage, NativeAmxSessionCache, NativeAmxSessionError,
        NativeAmxSessionKey, NativeAmxSigningGuard, NativeAmxVoteV2, aggregate_votes_to_qc,
        validate_native_amx_qc,
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
    reply_source_capacity: NonZeroUsize,
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
        reply_source_capacity: NonZeroUsize,
    ) -> Self {
        Self {
            session_capacity,
            body_buckets_per_session,
            effect_capacity,
            relay_capacity,
            merge_capacity,
            native_request_capacity,
            reply_source_capacity,
        }
    }
}

fn native_amx_signing_guard_capacity(
    limits: V2LaneWorkLimits,
) -> Result<NonZeroUsize, V2LaneWorkError> {
    let requested = limits
        .session_capacity
        .get()
        .checked_mul(limits.body_buckets_per_session.get())
        .ok_or_else(|| {
            V2LaneWorkError::SigningGuard(
                "native AMX signing-record capacity overflows usize".to_owned(),
            )
        })?;
    NonZeroUsize::new(requested.min(MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD)).ok_or_else(|| {
        V2LaneWorkError::SigningGuard(
            "native AMX signing-record capacity must be non-zero".to_owned(),
        )
    })
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
    /// Send one exact certificate reconstructed from an immutable Kura lane artifact.
    PostDurableLaneCertificate {
        /// Authenticated requester of the durable certificate.
        peer: PeerId,
        /// Independent authenticated sources which delivered the idempotent request.
        reply_routes: Option<NetworkReplyRoutes>,
        /// Exact fair-ingress request ownership consumed by this response.
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
        /// Exact proposal and Prepare/Commit QCs read from Kura.
        certificate: LaneBlockCertificateV1,
    },
    /// Send a context-bound Native AMX request or vote to one peer.
    PostNativeAmx {
        /// Destination participant/coordinator.
        peer: PeerId,
        /// Independent request sources for request-induced votes; absent for coordinator traffic.
        reply_routes: Option<NetworkReplyRoutes>,
        /// Context-bound Native AMX v2 message.
        message: NativeAmxMessage,
    },
    /// Broadcast a merge signature share to the frozen voting roster.
    BroadcastMerge(MergeCommitteeSignature),
    /// Send one authenticated certified merge-sidecar request or response.
    PostCertifiedMergeSidecar {
        /// Exact destination selected by the sidecar transport.
        peer: PeerId,
        /// Independent request sources for response chunks; absent for local fetch requests.
        reply_routes: Option<NetworkReplyRoutes>,
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

/// Result of binding reducer-owned Prepare-lock identity into lane work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GlobalBodyLockOutcome {
    /// A first or strictly higher exact lock replaced local speculative ownership.
    Inserted,
    /// The exact round/subject lock was already installed.
    Duplicate,
}

/// Fail-closed adapter construction or durable-retention error.
#[derive(Debug, Error, PartialEq, Eq)]
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
    /// The host cannot provide the crash-safe filesystem contract required by a voter.
    #[error("Sumeragi v2 voting validators require the Linux/macOS storage contract")]
    UnsupportedValidatorStoragePlatform,
    /// Durable lane certificate persistence failed.
    #[error("failed to persist anchored lane-local certificate: {0}")]
    Persistence(String),
    /// Durable local merge-signing anti-equivocation state could not be opened.
    #[error("failed to open durable merge-signing guard: {0}")]
    SigningGuard(String),
    /// Reducer lock identity does not belong to the frozen height context.
    #[error("global Prepare lock does not belong to the frozen height context")]
    InvalidGlobalBodyLock,
    /// A conflicting exact subject did not carry a strictly higher Prepare round.
    #[error("global Prepare lock subject changed without a strictly higher round")]
    ConflictingGlobalBodyLock,
}

/// Reject a voting role unless the host satisfies the first-release storage contract.
pub(crate) fn require_validator_storage_platform(
    voting_enabled: bool,
    storage_supported: bool,
) -> Result<(), V2LaneWorkError> {
    if voting_enabled && !storage_supported {
        return Err(V2LaneWorkError::UnsupportedValidatorStoragePlatform);
    }
    Ok(())
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GlobalBodyLock {
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
}

#[derive(Clone, Copy, Debug)]
enum LockedGlobalBodyOrigin<'a> {
    ExactProposalView,
    FixedGenesisViewZero {
        authenticated_genesis: &'a SignedBlock,
    },
}

#[derive(Clone, Debug)]
struct PendingMerge {
    stage: PendingMergeStage,
    signatures: BTreeMap<wire::ValidatorIndex, Vec<u8>>,
}

/// Durable semantic source for one completed lane CommitQC fanout.
///
/// `next_validator` advances only after the exact per-peer effect has entered
/// the bounded effect FIFO (or coalesced with the identical queued effect).
/// The completed session remains available for reset filtering through the end
/// of the global height even after every destination has transferred ownership.
#[derive(Clone, Debug)]
struct PendingCommittedLaneOutput {
    session: CommittedLaneBlockSession,
    next_validator: usize,
}

/// Exact durable reconstruction source for one winning lane proposal.
#[derive(Clone, Debug)]
enum DurableLaneSessionSource {
    Persistent {
        proposal: LaneBlockProposalV1,
        signer_pops: BTreeMap<PublicKey, Vec<u8>>,
        durable_source_hash: Hash,
    },
    #[cfg(test)]
    ExactTestMessage {
        message_hash: HashOf<BlockMessage>,
        durable_source_hash: Hash,
    },
}

/// Complete typed authority for retiring lane-local output at one applied
/// global-height boundary.
///
/// The winning proposal set is complete and independently names every lane
/// session retained by the decided carrier. Output for a winning proposal
/// requires its exact durable certificate/application witness. Structurally
/// valid same-height output for every other proposal is explicitly superseded
/// by the finality artifact rather than being mistaken for reconstructible
/// winning output.
#[derive(Clone, Debug)]
pub(crate) struct DurableLaneRolloverAuthority {
    finality_artifact_hash: HashOf<wire::finality::V2FinalityArtifact>,
    height: u64,
    winning_proposal_hashes: BTreeSet<Hash>,
    durable_sessions: BTreeMap<Hash, DurableLaneSessionSource>,
}

impl DurableLaneRolloverAuthority {
    fn new(
        finality_artifact: &wire::finality::V2FinalityArtifact,
        winning_proposal_hashes: BTreeSet<Hash>,
        durable_sessions: BTreeMap<Hash, DurableLaneSessionSource>,
    ) -> Self {
        Self {
            finality_artifact_hash: HashOf::new(finality_artifact),
            height: finality_artifact.height,
            winning_proposal_hashes,
            durable_sessions,
        }
    }

    /// Return the exact durable/supersession source commitment for one lane
    /// output covered by this complete applied-height authority.
    pub(crate) fn covered_source_hash(
        &self,
        finality_artifact: &wire::finality::V2FinalityArtifact,
        message: &BlockMessage,
    ) -> Result<Option<Hash>, String> {
        if self.finality_artifact_hash != HashOf::new(finality_artifact)
            || self.height != finality_artifact.height
        {
            return Ok(None);
        }
        let Some((proposal_height, proposal_hash)) = lane_output_identity(message) else {
            return Ok(None);
        };
        if proposal_height != self.height {
            return Ok(None);
        }

        if self.winning_proposal_hashes.contains(&proposal_hash) {
            let source = self.durable_sessions.get(&proposal_hash).ok_or_else(|| {
                "winning Sumeragi v2 lane output lacks its exact durable session witness".to_owned()
            })?;
            let covered = match source {
                DurableLaneSessionSource::Persistent {
                    proposal,
                    signer_pops,
                    durable_source_hash,
                } => {
                    validate_winning_lane_output(message, proposal, signer_pops)?;
                    Some(*durable_source_hash)
                }
                #[cfg(test)]
                DurableLaneSessionSource::ExactTestMessage {
                    message_hash,
                    durable_source_hash,
                } => (HashOf::new(message) == *message_hash).then_some(*durable_source_hash),
            };
            return covered.ok_or_else(|| {
                "winning Sumeragi v2 lane output does not match its exact durable session witness"
                    .to_owned()
            }).map(Some);
        }

        validate_superseded_lane_output(message)?;
        let message_hash = HashOf::new(message);
        Ok(Some(Hash::new_from_chunks(&[
            b"iroha:sumeragi:v2:lane-rollover-superseded:v1\0",
            self.finality_artifact_hash.as_ref(),
            message_hash.as_ref(),
        ])))
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        finality_artifact: &wire::finality::V2FinalityArtifact,
        message: &BlockMessage,
    ) -> Self {
        let (_, proposal_hash) = lane_output_identity(message)
            .expect("lane rollover test authority requires lane-local output");
        let finality_artifact_hash = HashOf::new(finality_artifact);
        let message_hash = HashOf::new(message);
        let durable_source_hash = Hash::new_from_chunks(&[
            b"iroha:sumeragi:v2:lane-rollover-test-source:v1\0",
            finality_artifact_hash.as_ref(),
            message_hash.as_ref(),
        ]);
        Self::new(
            finality_artifact,
            BTreeSet::from([proposal_hash]),
            BTreeMap::from([(
                proposal_hash,
                DurableLaneSessionSource::ExactTestMessage {
                    message_hash,
                    durable_source_hash,
                },
            )]),
        )
    }

    #[cfg(test)]
    pub(crate) fn missing_winning_witness_for_test(
        finality_artifact: &wire::finality::V2FinalityArtifact,
        proposal_hash: Hash,
    ) -> Self {
        Self::new(
            finality_artifact,
            BTreeSet::from([proposal_hash]),
            BTreeMap::new(),
        )
    }
}

impl DurableLaneSessionSource {
    fn persistent(
        finality_artifact: &wire::finality::V2FinalityArtifact,
        durable_artifact: &CertifiedLaneBlockArtifact,
        application_receipt: &LaneBlockApplicationReceiptArtifact,
        signer_pops: BTreeMap<PublicKey, Vec<u8>>,
    ) -> Self {
        let finality_artifact_hash = HashOf::new(finality_artifact);
        let durable_artifact_hash = HashOf::new(durable_artifact);
        let application_receipt_hash = HashOf::new(application_receipt);
        let durable_source_hash = Hash::new_from_chunks(&[
            b"iroha:sumeragi:v2:lane-rollover-source:v1\0",
            finality_artifact_hash.as_ref(),
            durable_artifact_hash.as_ref(),
            application_receipt_hash.as_ref(),
        ]);
        Self::Persistent {
            proposal: durable_artifact.proposal.clone(),
            signer_pops,
            durable_source_hash,
        }
    }
}

pub(crate) fn lane_output_identity(message: &BlockMessage) -> Option<(u64, Hash)> {
    match message {
        BlockMessage::LaneBlockProposal(proposal) => {
            Some((proposal.descriptor.proposal_height, proposal.proposal_hash))
        }
        BlockMessage::LaneBlockVote(vote) => {
            Some((vote.body.proposal_height, vote.body.proposal_hash))
        }
        BlockMessage::LaneBlockQc(qc) => Some((qc.body.proposal_height, qc.body.proposal_hash)),
        BlockMessage::LaneBlockCertificate(certificate) => Some((
            certificate.proposal.descriptor.proposal_height,
            certificate.proposal.proposal_hash,
        )),
        _ => None,
    }
}

fn validate_winning_lane_output(
    message: &BlockMessage,
    proposal: &LaneBlockProposalV1,
    signer_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<(), String> {
    match message {
        BlockMessage::LaneBlockProposal(output) => {
            validate_lane_block_proposal(output).map_err(|error| error.to_string())?;
            if output != proposal {
                return Err(
                    "winning lane proposal differs from the exact durable proposal".to_owned(),
                );
            }
        }
        BlockMessage::LaneBlockVote(vote) => {
            let phase = vote.body.phase;
            vote.validate_ingress(phase)
                .map_err(|error| error.to_string())?;
            if vote.body != proposal.vote_body(phase)
                || !proposal.descriptor.validator_set.contains(&vote.signer)
            {
                return Err("winning lane vote differs from the exact durable proposal".to_owned());
            }
        }
        BlockMessage::LaneBlockQc(qc) => {
            validate_winning_lane_qc(qc, proposal, signer_pops)?;
        }
        BlockMessage::LaneBlockCertificate(certificate) => {
            if certificate.proposal != *proposal {
                return Err(
                    "winning lane certificate differs from the exact durable proposal".to_owned(),
                );
            }
            validate_winning_lane_qc(&certificate.prepare_qc, proposal, signer_pops)?;
            validate_winning_lane_qc(&certificate.commit_qc, proposal, signer_pops)?;
        }
        _ => return Err("rollover authority received non-lane output".to_owned()),
    }
    Ok(())
}

fn validate_winning_lane_qc(
    qc: &LaneBlockQcV1,
    proposal: &LaneBlockProposalV1,
    signer_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<(), String> {
    if !matches!(qc.body.phase, CertPhase::Prepare | CertPhase::Commit)
        || qc.body != proposal.vote_body(qc.body.phase)
        || qc.validator_set != proposal.descriptor.validator_set
    {
        return Err("winning lane QC differs from the exact durable proposal".to_owned());
    }
    validate_lane_block_qc_aggregate(qc, signer_pops).map_err(|error| error.to_string())
}

fn validate_superseded_lane_output(message: &BlockMessage) -> Result<(), String> {
    match message {
        BlockMessage::LaneBlockProposal(proposal) => {
            validate_lane_block_proposal(proposal).map_err(|error| error.to_string())
        }
        BlockMessage::LaneBlockVote(vote) => vote
            .validate_ingress(vote.body.phase)
            .map_err(|error| error.to_string()),
        BlockMessage::LaneBlockQc(qc) => {
            // This corridor accepts lane effects only from `V2LaneWorkAdapter`,
            // which verifies PoPs and the aggregate before emission. Recheck
            // the complete bounded QC shape here; the finality authority makes
            // its non-winning proposal obsolete, not reconstructible.
            validate_lane_block_qc(qc).map_err(|error| error.to_string())
        }
        BlockMessage::LaneBlockCertificate(certificate) => {
            let session = CommittedLaneBlockSession {
                proposal: certificate.proposal.clone(),
                prepare_qc: certificate.prepare_qc.clone(),
                commit_qc: certificate.commit_qc.clone(),
            };
            validate_committed_lane_block_session(&session).map_err(|error| error.to_string())
        }
        _ => Err("rollover authority received non-lane output".to_owned()),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HistoricalRecoveryPersistence {
    Complete,
    WaitingForCanonicalDependency,
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
    globally_locked_body: Option<GlobalBodyLock>,
    retained_merge_carrier_state: Option<(
        wire::View,
        Option<wire::BlockSubject>,
        Option<wire::BlockSubject>,
    )>,
    #[cfg(test)]
    merge_retention_scans: usize,
    locally_bound_lane_proposals: BTreeMap<Hash, LaneBlockProposalPayloadHintV1>,
    pending_committed_lanes: VecDeque<CommittedLaneBlockSession>,
    committed_lane_outputs: VecDeque<PendingCommittedLaneOutput>,
    /// Certified earlier-height sessions awaiting their strict Kura and
    /// application-witness boundary. These are not speculative work for the
    /// current global carrier and therefore must never be filtered by a
    /// current-height lock or Decision.
    historical_recovery_sessions: VecDeque<CommittedLaneBlockSession>,
    committed_lane_output_cursor: usize,
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
        require_validator_storage_platform(
            voting_enabled,
            sumeragi_v2_validator_storage_supported(),
        )?;
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
            let max_records = native_amx_signing_guard_capacity(limits)?;
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
            globally_locked_body: None,
            retained_merge_carrier_state: None,
            #[cfg(test)]
            merge_retention_scans: 0,
            locally_bound_lane_proposals: BTreeMap::new(),
            pending_committed_lanes: VecDeque::new(),
            committed_lane_outputs: VecDeque::new(),
            historical_recovery_sessions: VecDeque::new(),
            committed_lane_output_cursor: 0,
            admitted_relays: BTreeSet::new(),
            merge_entries: BTreeMap::new(),
            merge_claims: BTreeMap::new(),
            merge_signing_guard,
            merge_sidecars: MergeSidecarTransport::with_reply_source_capacity(
                limits.reply_source_capacity.get(),
            )
            .map_err(|error| V2LaneWorkError::InvalidContext(error.to_string()))?,
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
        let Some(global_leader) = usize::try_from(self.context.leader(round.view))
            .ok()
            .and_then(|index| self.context.roster.get(index))
            .map(|entry| entry.validator.clone())
        else {
            return V2LaneIngressOutcome::Rejected;
        };
        if self.local_peer != global_leader {
            return V2LaneIngressOutcome::Rejected;
        }
        let nexus = self.state.nexus_snapshot();
        let shared_committee =
            !nexus.enabled || !proposal_lookahead_enabled(&nexus, self.context.height);
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
            let Some(lane_author) = self.expected_lane_author(proposal).cloned() else {
                return V2LaneIngressOutcome::Rejected;
            };
            if (shared_committee && lane_author != global_leader)
                || !self.lane_proposal_authorized(proposal, Some(&lane_author), false, round.view)
            {
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
    pub(crate) fn mark_global_body_locked(
        &mut self,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
    ) -> Result<GlobalBodyLockOutcome, V2LaneWorkError> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard
            .acquire()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        if round.context_id != self.context.id() || round.height != self.context.height {
            return Err(V2LaneWorkError::InvalidGlobalBodyLock);
        }
        let superseded_subject = if let Some(existing) = self.globally_locked_body {
            if existing == (GlobalBodyLock { round, subject }) {
                return Ok(GlobalBodyLockOutcome::Duplicate);
            }
            if round.view <= existing.round.view {
                return Err(V2LaneWorkError::ConflictingGlobalBodyLock);
            }
            (existing.subject != subject).then_some(existing.subject)
        } else {
            None
        };
        self.globally_locked_body = Some(GlobalBodyLock { round, subject });
        if let Some(superseded_subject) = superseded_subject
            && superseded_subject.block_hash == subject.block_hash
        {
            self.lane_sessions
                .retire_uncommitted_global_anchor(superseded_subject.block_hash);
        } else {
            self.lane_sessions
                .retire_uncommitted_global_anchors_except(subject.block_hash);
        }
        self.locally_bound_lane_proposals.clear();
        self.pending_local_lane_proposals.clear();
        self.merge_entries.clear();
        self.merge_claims.clear();
        self.retain_committed_lane_outputs_for_subject(subject);
        self.purge_queued_global_body_effects_except_committed_outputs();
        self.schedule_committed_lane_outputs();
        Ok(GlobalBodyLockOutcome::Inserted)
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
        if let Some(decided) = decided_subject {
            // Publish the terminal carrier predicate before any retained lane
            // session is driven. Quorum-protected losing evidence may remain
            // cached for safety checks, but must never receive one final
            // outbound turn during Decision installation.
            self.retained_merge_carrier_state = Some(carrier_state);
            self.retire_speculative_work_after_decision(decided);
            self.merge_entries.clear();
            self.merge_claims.clear();
            return Ok(());
        }
        if locked_subject.is_some() {
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

    fn decision_pending(&self) -> bool {
        self.retained_merge_carrier_state
            .is_some_and(|(_, _, decided)| decided.is_some())
    }

    fn proposal_is_bound_to_decided_carrier(&self, proposal: &LaneBlockProposalV1) -> bool {
        let Some((_, _, Some(decided))) = self.retained_merge_carrier_state else {
            return false;
        };
        let Some(height) = usize::try_from(self.context.height)
            .ok()
            .and_then(NonZeroUsize::new)
        else {
            return false;
        };
        let Some(block) = self.kura.get_block(height) else {
            return false;
        };
        let Ok(canonical_payload_hash) = block.canonical_proposal_wire_hash() else {
            return false;
        };
        let canonical_subject = wire::BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: canonical_payload_hash,
        };
        proposal.descriptor.proposal_height == self.context.height
            && canonical_subject == decided
            && block.execution_context().is_some_and(|bundle| {
                bundle.lane_payload_ownerships.iter().any(|ownership| {
                    proposal_from_ownership(ownership, decided.block_hash).as_ref()
                        == Some(proposal)
                })
            })
    }

    fn lane_message_is_allowed_after_decision(&self, message: &BlockMessage) -> bool {
        match message {
            BlockMessage::LaneBlockProposal(proposal) => {
                self.proposal_is_bound_to_decided_carrier(proposal)
            }
            BlockMessage::LaneBlockVote(vote) => self
                .canonical_proposal_for_vote_body(&vote.body)
                .is_some_and(|proposal| self.proposal_is_bound_to_decided_carrier(&proposal)),
            BlockMessage::LaneBlockQc(qc) => self
                .canonical_proposal_for_vote_body(&qc.body)
                .is_some_and(|proposal| self.proposal_is_bound_to_decided_carrier(&proposal)),
            BlockMessage::LaneBlockCertificate(certificate) => {
                certificate.proposal.descriptor.proposal_height < self.context.height
                    || self.proposal_is_bound_to_decided_carrier(&certificate.proposal)
            }
            _ => false,
        }
    }

    fn retire_speculative_work_after_decision(&mut self, decided: wire::BlockSubject) {
        self.collect_committed_lane_sessions();
        self.lane_sessions
            .retire_uncommitted_global_anchors_except(decided.block_hash);
        self.planned_lane_proposals.clear();
        self.pending_local_lane_proposals.clear();
        // A certificate for the decided carrier may arrive after the global
        // Decision. Keep only that carrier's immutable body binding so the
        // late atomic certificate can complete; losing carriers remain
        // permanently retired.
        self.locally_bound_lane_proposals
            .retain(|_, hint| hint.proposal_block_hash == decided.block_hash);
        self.native_requests.clear();
        self.native_claims.clear();
        self.local_native_claims.clear();
        self.admitted_relays.clear();
        self.retain_committed_lane_outputs_for_subject(decided);
        self.purge_queued_global_body_effects_except_committed_outputs();
        // Global Decision retires every losing carrier, but it does not
        // supersede the decided carrier's lane certificate. Keep driving that
        // exact same-height session until its certificate and application
        // receipt cross the durable rollover boundary.
        self.drive_lane_sessions();
        self.schedule_lane_artifact_retransmissions();
        self.schedule_committed_lane_outputs();
    }

    /// Bind lane proposals reconstructed from the exact durable globally
    /// locked body, then release their bounded lane-local consensus sessions.
    ///
    /// This ordinary path requires the immutable block header view to equal the
    /// exact locked proposal round. Height-one recovery must use
    /// [`Self::bind_locked_genesis_body`] instead.
    pub(crate) fn bind_locked_global_body(&mut self, block: &SignedBlock) -> V2LaneIngressOutcome {
        self.bind_locked_global_body_from_origin(block, LockedGlobalBodyOrigin::ExactProposalView)
    }

    /// Bind the exact authenticated fixed view-zero genesis body under its
    /// durable proposal-origin lock.
    ///
    /// Genesis is signed once by its configured authority and therefore keeps
    /// a view-zero header when a certified view change moves its Proposal to a
    /// later round. The supplied staged genesis is the runner-authenticated
    /// source of truth; this path rejects every other byte sequence and every
    /// parented, context-bearing, or non-height-one body.
    pub(crate) fn bind_locked_genesis_body(
        &mut self,
        block: &SignedBlock,
        authenticated_genesis: &SignedBlock,
    ) -> V2LaneIngressOutcome {
        self.bind_locked_global_body_from_origin(
            block,
            LockedGlobalBodyOrigin::FixedGenesisViewZero {
                authenticated_genesis,
            },
        )
    }

    fn bind_locked_global_body_from_origin(
        &mut self,
        block: &SignedBlock,
        origin: LockedGlobalBodyOrigin<'_>,
    ) -> V2LaneIngressOutcome {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(validation_permit) = output_guard.acquire() else {
            return V2LaneIngressOutcome::Rejected;
        };
        let block_hash = block.hash();
        let Ok(canonical_wire) = block.encode_wire() else {
            return V2LaneIngressOutcome::Rejected;
        };
        let subject = wire::BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash,
            payload_hash: Hash::new(&canonical_wire),
        };
        let Some(global_lock) = self.globally_locked_body else {
            return V2LaneIngressOutcome::Rejected;
        };
        let origin_matches = match origin {
            LockedGlobalBodyOrigin::ExactProposalView => {
                global_lock.round.view == block.header().view_change_index()
            }
            LockedGlobalBodyOrigin::FixedGenesisViewZero {
                authenticated_genesis,
            } => {
                let Ok(authenticated_wire) = authenticated_genesis
                    .canonical_resultless_proposal()
                    .encode_wire()
                else {
                    return V2LaneIngressOutcome::Rejected;
                };
                self.context.height == 1
                    && self.context.parent_commit_qc.is_none()
                    && self.context.snapshot_bootstrap.is_none()
                    && block.header().height().get() == 1
                    && block.header().view_change_index() == 0
                    && block.header().prev_block_hash().is_none()
                    && block.header().execution_context_hash().is_none()
                    && block.execution_context().is_none()
                    && block.is_resultless_proposal()
                    && canonical_wire == authenticated_wire
            }
        };
        if global_lock.subject != subject
            || !origin_matches
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
        let nexus = self.state.nexus_snapshot();
        let shared_committee =
            !nexus.enabled || !proposal_lookahead_enabled(&nexus, self.context.height);
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
            let expected_lane_author = self.expected_lane_author(&proposal);
            if descriptor.proposal_height != self.context.height
                || ownership.proposal_view != global_view
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
                || expected_lane_author.is_none()
                || (shared_committee && expected_lane_author != Some(global_leader))
            {
                return V2LaneIngressOutcome::Rejected;
            }
            proposals.push(proposal);
        }

        let local = self.pending_local_lane_proposals.get(&block_hash).cloned();
        if local.as_ref().is_some_and(|planned| planned != &proposals) {
            return V2LaneIngressOutcome::Rejected;
        }
        let locally_authored = proposals
            .iter()
            .filter(|proposal| self.expected_lane_author(proposal) == Some(&self.local_peer))
            .cloned()
            .collect::<Vec<_>>();
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
        let Some(locally_bound_lane_proposals) = proposals
            .iter()
            .map(|proposal| {
                proposal
                    .payload_block_hint
                    .map(|hint| (proposal.proposal_hash, hint))
            })
            .collect::<Option<BTreeMap<_, _>>>()
        else {
            return V2LaneIngressOutcome::Rejected;
        };
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
        self.locally_bound_lane_proposals = locally_bound_lane_proposals;
        for proposal in locally_authored {
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
        // Block sync can deliver and apply the current canonical body after
        // this height's adapter was constructed. Rehydrate its exact Kura
        // ownerships at the rollover boundary so a validator which missed the
        // lane CommitQC retains a bounded proposal source for certificate
        // recovery instead of waiting forever with an already-applied block.
        self.hydrate_canonical_lane_artifacts();
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
            // `CommittedLaneBlockSession` is an internal transport container,
            // not a validated proof type. Recheck every proof variant before
            // the same-proposal shortcut can retire it against an earlier
            // durable certificate.
            let pops = self.pops_for_lane_session(&session);
            let candidate = CertifiedLaneBlockArtifact::new(session.clone(), pops.clone());
            Kura::validate_certified_lane_block_artifact(&candidate).map_err(|message| {
                V2LaneWorkError::Persistence(format!(
                    "pending committed lane certificate is invalid: {message}"
                ))
            })?;
            let descriptor = &session.proposal.descriptor;
            let durable_exact_proposal = self
                .kura
                .read_certified_lane_block_artifact(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                )
                .filter(|durable| durable.proposal == session.proposal);
            if let Some(durable) = durable_exact_proposal {
                if !self
                    .state
                    .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(&session)
                {
                    if !self
                        .state
                        .certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached(
                            &durable.proposal,
                        )
                    {
                        return Err(V2LaneWorkError::Persistence(
                            "globally applied lane block has no applied predecessor".to_owned(),
                        ));
                    }
                    let receipt_persisted = self
                        .kura
                        .persist_lane_block_application_receipt_if_ready(&durable.proposal)
                        .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
                    if !receipt_persisted {
                        return Err(V2LaneWorkError::Persistence(
                            "globally applied lane block has no recoverable canonical results"
                                .to_owned(),
                        ));
                    }
                }
                // Quorum signer subsets are proof variants, not lane-block
                // identity. Retain the first exact durable proof and retire
                // this volatile replay only after repairing or observing the
                // same proposal's application witness. Peers must otherwise
                // keep serving the lane session so a lagging validator can
                // reconstruct it.
                persisted = persisted.saturating_add(1);
                continue;
            }
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
        self.schedule_committed_lane_outputs();
        operation.complete();
        Ok(persisted)
    }

    /// Build the complete lane-output rollover authority after every winning
    /// current-height session is independently readable across Kura's strict
    /// certificate and application-receipt boundaries.
    pub(crate) fn durable_lane_rollover_authority(
        &self,
        finality_artifact: &wire::finality::V2FinalityArtifact,
    ) -> Result<Option<DurableLaneRolloverAuthority>, V2LaneWorkError> {
        let _permit = self
            .output_guard
            .acquire()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        finality_artifact
            .validate()
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        if finality_artifact.height_context != self.context {
            return Err(V2LaneWorkError::Persistence(
                "lane rollover finality authority differs from the frozen height context"
                    .to_owned(),
            ));
        }

        let Some(height) = usize::try_from(finality_artifact.height)
            .ok()
            .and_then(NonZeroUsize::new)
        else {
            return Err(V2LaneWorkError::Persistence(
                "lane rollover finality authority has an invalid zero height".to_owned(),
            ));
        };
        let block = self.kura.get_block(height).ok_or_else(|| {
            V2LaneWorkError::Persistence(
                "lane rollover finality authority has no canonical block body".to_owned(),
            )
        })?;
        if block.header().height().get() != finality_artifact.height
            || block.hash() != finality_artifact.block_hash
        {
            return Err(V2LaneWorkError::Persistence(
                "lane rollover finality authority differs from the canonical block body".to_owned(),
            ));
        }
        // This boundary audits canonical lane ownership, not the already
        // validated execution plan as a whole. In particular, result-bearing
        // genesis and any other already-finalized canonical empty ownership
        // set have no lane durability debt; external entries alone do not
        // manufacture one at rollover.
        let ownerships = block
            .execution_context()
            .map_or(&[][..], |bundle| bundle.lane_payload_ownerships.as_slice());
        if ownerships.len() > self.limits.session_capacity.get() {
            return Err(V2LaneWorkError::Persistence(
                "canonical lane payload exceeds the frozen session capacity".to_owned(),
            ));
        }
        let mut winning_proposals = BTreeMap::new();
        for ownership in ownerships {
            let proposal = proposal_from_ownership(ownership, finality_artifact.block_hash)
                .ok_or_else(|| {
                    V2LaneWorkError::Persistence(
                        "canonical lane ownership cannot reconstruct its exact proposal".to_owned(),
                    )
                })?;
            if winning_proposals
                .insert(proposal.proposal_hash, proposal)
                .is_some()
            {
                return Err(V2LaneWorkError::Persistence(
                    "canonical lane payload contains a duplicate winning proposal".to_owned(),
                ));
            }
        }
        let winning_proposal_hashes = winning_proposals.keys().copied().collect::<BTreeSet<_>>();
        let mut retained_proposal_hashes = BTreeSet::new();
        for output in &self.committed_lane_outputs {
            let proposal = &output.session.proposal;
            if winning_proposals.get(&proposal.proposal_hash) != Some(proposal)
                || !retained_proposal_hashes.insert(proposal.proposal_hash)
            {
                return Err(V2LaneWorkError::Persistence(
                    "retained lane CommitQC is not an exact unique winner of the applied block"
                        .to_owned(),
                ));
            }
        }
        if !durable_lane_completion_matches_finality(self.kura.as_ref(), finality_artifact)
            .map_err(V2LaneWorkError::Persistence)?
        {
            return Ok(None);
        }
        let mut durable_sessions = BTreeMap::new();
        for proposal in winning_proposals.values() {
            let descriptor = &proposal.descriptor;
            let durable = self
                .kura
                .read_certified_lane_block_artifact(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                )
                .ok_or_else(|| {
                    V2LaneWorkError::Persistence(
                        "retained lane CommitQC has no durable certified artifact".to_owned(),
                    )
                })?;
            let application_receipt = self
                .kura
                .read_lane_block_application_receipt(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                )
                .ok_or_else(|| {
                    V2LaneWorkError::Persistence(
                        "retained lane CommitQC has no durable application receipt".to_owned(),
                    )
                })?;
            let Some(hint) = proposal.payload_block_hint else {
                return Err(V2LaneWorkError::Persistence(
                    "retained current-height lane CommitQC has no global block anchor".to_owned(),
                ));
            };
            let retained = self
                .committed_lane_outputs
                .iter()
                .find(|output| output.session.proposal.proposal_hash == proposal.proposal_hash);
            if let Some(output) = retained {
                let session = &output.session;
                let pops = self.pops_for_lane_session(session);
                let candidate = CertifiedLaneBlockArtifact::new(session.clone(), pops);
                Kura::validate_certified_lane_block_artifact(&candidate).map_err(|message| {
                    V2LaneWorkError::Persistence(format!(
                        "retained lane CommitQC output is invalid: {message}"
                    ))
                })?;
                let same_commit_decision = session.commit_qc.body == durable.commit_qc.body
                    && session.commit_qc.validator_set_hash_version
                        == durable.commit_qc.validator_set_hash_version
                    && session.commit_qc.validator_set_hash == durable.commit_qc.validator_set_hash
                    && session.commit_qc.validator_set == durable.commit_qc.validator_set
                    && session.commit_qc.payload_availability_qc
                        == durable.commit_qc.payload_availability_qc;
                if !same_commit_decision {
                    return Err(V2LaneWorkError::Persistence(
                        "retained lane CommitQC differs from the exact durable decision".to_owned(),
                    ));
                }
            }
            if durable.proposal != *proposal
                || application_receipt.proposal != *proposal
                || descriptor.proposal_height != finality_artifact.height
                || hint.proposal_height != finality_artifact.height
                || hint.proposal_block_hash != finality_artifact.block_hash
                || application_receipt.application_block_height != finality_artifact.height
                || application_receipt.application_block_hash != finality_artifact.block_hash
            {
                return Err(V2LaneWorkError::Persistence(
                    "retained lane CommitQC is not bound to the exact applied global artifact"
                        .to_owned(),
                ));
            }
            let mut signer_pops = durable.signer_pops.clone();
            if let Some(output) = retained {
                signer_pops.extend(self.pops_for_lane_session(&output.session));
            }
            let source = DurableLaneSessionSource::persistent(
                finality_artifact,
                &durable,
                &application_receipt,
                signer_pops,
            );
            if durable_sessions
                .insert(proposal.proposal_hash, source)
                .is_some()
            {
                return Err(V2LaneWorkError::Persistence(
                    "duplicate winning lane proposal in rollover authority".to_owned(),
                ));
            }
        }
        Ok(Some(DurableLaneRolloverAuthority::new(
            finality_artifact,
            winning_proposal_hashes,
            durable_sessions,
        )))
    }

    /// Advance one earlier-height certificate through its strict durability
    /// and application-witness boundary.
    ///
    /// The FIFO is serviced independently of the current global carrier. A
    /// missing committed anchor, predecessor receipt, block body, or execution
    /// result keeps the exact session owned for a later fair turn. Invalid or
    /// conflicting durable evidence returns an error without retiring the
    /// owner, causing the process-wide output guard to fail closed.
    pub(crate) fn service_next_historical_recovery(&mut self) -> Result<bool, V2LaneWorkError> {
        let Some(session) = self.historical_recovery_sessions.pop_front() else {
            return Ok(false);
        };
        let output_guard = Arc::clone(&self.output_guard);
        let Some(operation) = output_guard.begin_fail_stop_operation() else {
            self.historical_recovery_sessions.push_front(session);
            return Err(V2LaneWorkError::RestartRequired);
        };
        match self.persist_historical_recovery_session(&session) {
            Ok(HistoricalRecoveryPersistence::Complete) => {
                operation.complete();
                Ok(true)
            }
            Ok(HistoricalRecoveryPersistence::WaitingForCanonicalDependency) => {
                self.historical_recovery_sessions.push_back(session);
                operation.complete();
                Ok(false)
            }
            Err(error) => {
                self.historical_recovery_sessions.push_front(session);
                Err(error)
            }
        }
    }

    /// Return whether an earlier-height certified session still owns local
    /// persistence or application work.
    pub(crate) fn has_pending_historical_recovery(&self) -> bool {
        !self.historical_recovery_sessions.is_empty()
    }

    fn persist_historical_recovery_session(
        &self,
        session: &CommittedLaneBlockSession,
    ) -> Result<HistoricalRecoveryPersistence, V2LaneWorkError> {
        if !self.session_has_canonical_anchor(session)
            || !self.proposal_anchor_is_committed_in_state(&session.proposal)
        {
            return Ok(HistoricalRecoveryPersistence::WaitingForCanonicalDependency);
        }

        let pops = self.pops_for_lane_session(session);
        let candidate = CertifiedLaneBlockArtifact::new(session.clone(), pops.clone());
        Kura::validate_certified_lane_block_artifact(&candidate).map_err(|message| {
            V2LaneWorkError::Persistence(format!(
                "historical lane certificate is invalid: {message}"
            ))
        })?;
        match self.kura.recover_lane_block_payload(&session.proposal) {
            Ok(_) => {}
            Err(
                LaneBlockPayloadAvailability::MissingLaneArtifact
                | LaneBlockPayloadAvailability::MissingProposalBlock,
            ) => return Ok(HistoricalRecoveryPersistence::WaitingForCanonicalDependency),
            Err(availability) => {
                return Err(V2LaneWorkError::Persistence(format!(
                    "historical lane certificate is anchored to invalid payload: {availability:?}"
                )));
            }
        }
        let descriptor = &session.proposal.descriptor;
        let durable = self
            .kura
            .read_certified_lane_block_artifact(descriptor.lane_id, descriptor.lane_block_height);
        let durable_proposal = match durable {
            Some(durable) if durable.proposal == session.proposal => durable.proposal,
            Some(_) => {
                return Err(V2LaneWorkError::Persistence(
                    "historical lane certificate conflicts with the durable lane slot".to_owned(),
                ));
            }
            None => {
                self.kura
                    .persist_committed_lane_block_session(session, &pops)
                    .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
                session.proposal.clone()
            }
        };

        if self
            .state
            .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(session)
        {
            return Ok(HistoricalRecoveryPersistence::Complete);
        }
        if !self
            .state
            .certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached(
                &durable_proposal,
            )
        {
            return Ok(HistoricalRecoveryPersistence::WaitingForCanonicalDependency);
        }
        let receipt_persisted = self
            .kura
            .persist_lane_block_application_receipt_if_ready(&durable_proposal)
            .map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?;
        if !receipt_persisted {
            return Ok(HistoricalRecoveryPersistence::WaitingForCanonicalDependency);
        }
        Ok(HistoricalRecoveryPersistence::Complete)
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
        let Some(anchor) = self.canonical_anchor_for_proposal(proposal) else {
            return false;
        };
        let Some(height) = usize::try_from(hint.proposal_height)
            .ok()
            .and_then(NonZeroUsize::new)
        else {
            return false;
        };
        let Some(block) = self.kura.get_block(height) else {
            return false;
        };
        hint.proposal_height == proposal.descriptor.proposal_height
            && hint.proposal_block_hash == anchor.proposal_block_hash
            && hint.proposal_view == anchor.ownership.proposal_view
            && block.hash() == hint.proposal_block_hash
            && block.header().view_change_index() == hint.proposal_view
            && block.execution_context().is_some_and(|bundle| {
                bundle
                    .lane_payload_ownerships
                    .iter()
                    .any(|ownership| ownership == &anchor.ownership)
            })
            && self
                .state
                .committed_block_hash_at_height(hint.proposal_height)
                == Some(hint.proposal_block_hash)
    }

    /// Consume the exact fair-ingress carrier while accepting a lane message.
    ///
    /// Missing, altered, or route-inconsistent ownership fails closed at this
    /// seam; callers cannot reconstruct it from public sender identities.
    pub(crate) fn accept_lane_message_with_ingress_ownership(
        &mut self,
        mut inbound: InboundBlockMessage,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let Some(ingress_ownership) = inbound.take_ingress_ownership() else {
            return V2LaneIngressOutcome::Rejected;
        };
        self.accept_lane_message_owned(inbound, Some(ingress_ownership), active_view)
    }

    /// Unit-test compatibility seam for constructing lane protocol fixtures
    /// without the outer fair queue. Production calls the ownership-requiring
    /// method above.
    #[cfg(test)]
    pub(crate) fn accept_lane_message(
        &mut self,
        mut inbound: InboundBlockMessage,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let ingress_ownership = inbound.take_ingress_ownership();
        self.accept_lane_message_owned(inbound, ingress_ownership, active_view)
    }

    fn accept_lane_message_owned(
        &mut self,
        inbound: InboundBlockMessage,
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let output_guard = Arc::clone(&self.output_guard);
        let Some(_permit) = output_guard.acquire() else {
            return V2LaneIngressOutcome::Rejected;
        };
        let (message, sender, reply_routes) = inbound.into_message_sender_and_reply_routes();
        if ingress_ownership.as_ref().is_some_and(|ownership| {
            !ownership.validate_exact()
                || !ownership.matches_message(&message)
                || !ownership.matches_reply_routes(reply_routes.as_ref())
        }) {
            return V2LaneIngressOutcome::Rejected;
        }
        if self.decision_pending()
            && let BlockMessage::LaneBlockProposal(proposal) = &message
            && proposal.descriptor.proposal_height >= self.context.height
            && !self.proposal_is_bound_to_decided_carrier(proposal)
        {
            return V2LaneIngressOutcome::Rejected;
        }
        if let BlockMessage::LaneBlockProposal(proposal) = &message
            && let Some(outcome) = self.serve_durable_lane_certificate(
                proposal,
                sender.as_ref(),
                reply_routes,
                ingress_ownership,
            )
        {
            return outcome;
        }
        if self.decision_pending() && !self.lane_message_is_allowed_after_decision(&message) {
            return V2LaneIngressOutcome::Rejected;
        }
        let outcome = match message {
            BlockMessage::LaneBlockProposal(proposal) => {
                self.insert_lane_proposal(proposal, sender.as_ref(), false, active_view)
            }
            BlockMessage::LaneBlockVote(vote) => {
                self.insert_lane_vote(vote, sender.as_ref(), active_view)
            }
            BlockMessage::LaneBlockQc(qc) => self.insert_lane_qc(qc, active_view),
            BlockMessage::LaneBlockCertificate(certificate) => {
                self.insert_lane_certificate(*certificate, active_view)
            }
            _ => V2LaneIngressOutcome::Rejected,
        };
        if outcome != V2LaneIngressOutcome::Rejected {
            self.drive_lane_sessions();
        }
        outcome
    }

    /// Treat an exact canonical proposal retransmission as an idempotent
    /// request for the complete Kura-backed certificate.
    ///
    /// The requester retains and periodically retransmits its incomplete
    /// proposal, so an occupied effect slot leaves reconstruction at the
    /// durable source instead of requiring an unbounded response queue here.
    fn serve_durable_lane_certificate(
        &mut self,
        proposal: &LaneBlockProposalV1,
        sender: Option<&PeerId>,
        reply_routes: Option<NetworkReplyRoutes>,
        ingress_ownership: Option<FairV2IngressOwnershipEvidence>,
    ) -> Option<V2LaneIngressOutcome> {
        let sender = sender?;
        let Some(reply_routes) = reply_routes else {
            return Some(V2LaneIngressOutcome::Rejected);
        };
        if !reply_routes_are_live_for_peer(&reply_routes, sender) {
            return Some(V2LaneIngressOutcome::Rejected);
        }
        let certificate = match self.reconstruct_durable_lane_certificate(proposal, sender) {
            Ok(Some(certificate)) => certificate,
            Ok(None) => return None,
            Err(()) => return Some(V2LaneIngressOutcome::Rejected),
        };
        let queued = self.push_effect(V2LaneWorkEffect::PostDurableLaneCertificate {
            peer: sender.clone(),
            reply_routes: Some(reply_routes),
            ingress_ownership,
            certificate,
        });
        Some(if queued {
            V2LaneIngressOutcome::Inserted
        } else {
            V2LaneIngressOutcome::Duplicate
        })
    }

    fn reconstruct_durable_lane_certificate(
        &self,
        proposal: &LaneBlockProposalV1,
        sender: &PeerId,
    ) -> Result<Option<LaneBlockCertificateV1>, ()> {
        let artifact = self.kura.read_certified_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        );
        let Some(artifact) = artifact else {
            return Ok(None);
        };
        if artifact.proposal != *proposal {
            return Ok(None);
        }
        // The requester need not belong to the historical lane committee: a
        // current global validator which missed the original lane fanout must
        // still be able to apply the already-committed global block. Limit the
        // idempotent response to an authenticated member of either the frozen
        // current global roster or the canonical historical lane committee.
        let requester_is_current_validator = self
            .context
            .roster
            .iter()
            .any(|entry| &entry.validator == sender);
        if !requester_is_current_validator && !artifact.commit_qc.validator_set.contains(sender) {
            return Err(());
        }
        Ok(Some(LaneBlockCertificateV1 {
            proposal: artifact.proposal,
            prepare_qc: artifact.prepare_qc,
            commit_qc: artifact.commit_qc,
        }))
    }

    /// Register a deterministic validation blocked only on one exact certified
    /// merge sidecar. The merge QC authenticates its own immutable carrier view,
    /// which may precede the enclosing proposal round and is never rebound to
    /// that proposal's view.
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
                "certified merge reference is not bound to the body's exact carrier height, parent, and carrier view"
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
                self.push_merge_sidecar_post_or_restart(post)?;
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
        if self.decision_pending()
            && !matches!(&message, LaneRelayMessage::CertifiedMergeSidecar { .. })
        {
            operation.complete();
            return V2LaneIngressOutcome::Rejected;
        }
        let result = match message {
            LaneRelayMessage::Envelope(envelope) => self.accept_lane_relay(envelope, active_view),
            LaneRelayMessage::MergeSignature(signature) => {
                self.accept_merge_signature(signature, active_view)
            }
            LaneRelayMessage::CertifiedMergeSidecar {
                sender,
                reply_route,
                message,
            } => self.accept_certified_merge_sidecar(sender, reply_route, message),
            LaneRelayMessage::NativeAmx {
                sender,
                reply_route,
                message,
            } => Ok(self.accept_native_amx(sender, reply_route, message, active_view)),
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

    /// Clone the next fairly selected effect without transferring its ownership.
    pub(crate) fn next_effect(&self) -> Option<V2LaneWorkEffect> {
        let output_guard = Arc::clone(&self.output_guard);
        let _permit = output_guard.acquire()?;
        let both_ready = !self.sidecar_effects.is_empty() && !self.effects.is_empty();
        let take_sidecar = if both_ready {
            self.drain_sidecar_next
        } else {
            !self.sidecar_effects.is_empty()
        };
        if take_sidecar {
            self.sidecar_effects.front().cloned()
        } else {
            self.effects.front().cloned()
        }
    }

    /// Number of bounded effects available for one complete fair scheduler scan.
    pub(crate) fn effect_count(&self) -> usize {
        self.effects
            .len()
            .saturating_add(self.sidecar_effects.len())
    }

    /// Return one temporarily unserviceable effect to the tail of its owner lane.
    pub(crate) fn requeue_effect(&mut self, effect: V2LaneWorkEffect) -> bool {
        match effect {
            V2LaneWorkEffect::PostCertifiedMergeSidecar {
                peer,
                reply_routes,
                message,
            } => self.push_merge_sidecar_effect(V2LaneWorkEffect::PostCertifiedMergeSidecar {
                peer,
                reply_routes,
                message,
            }),
            effect => self.push_effect(effect),
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
            self.push_merge_sidecar_post_or_restart(post)?;
        }

        if self.decision_pending() {
            self.collect_committed_lane_sessions();
            if let Some((_, _, Some(decided))) = self.retained_merge_carrier_state {
                self.retain_committed_lane_outputs_for_subject(decided);
            }
            self.purge_queued_global_body_effects_except_committed_outputs();
            self.drive_lane_sessions();
            self.schedule_lane_artifact_retransmissions();
            self.schedule_committed_lane_outputs();
            operation.complete();
            return Ok(());
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
        // Peer-queue admission is only a volatile delivery boundary. Begin a
        // fresh bounded fanout round after the previous round transferred all
        // destinations, so a connection-generation failure after enqueue
        // cannot permanently erase the final lane certificate. Once Decision
        // is installed, the decision branch above keeps starting bounded
        // rounds only for the exact decided-lane ownerships until their
        // certificates and application receipts cross the durable boundary.
        if !self.committed_lane_outputs.is_empty()
            && self
                .committed_lane_outputs
                .iter()
                .all(|output| output.next_validator >= output.session.commit_qc.validator_set.len())
        {
            for output in &mut self.committed_lane_outputs {
                output.next_validator = 0;
            }
        }
        self.schedule_committed_lane_outputs();
        let mut lane_artifacts = Vec::new();
        for proposal in self.lane_sessions.proposals_without_commit_qc() {
            if proposal.descriptor.proposal_height != self.context.height
                || (self.decision_pending()
                    && !self.proposal_is_bound_to_decided_carrier(&proposal))
                || !self.proposal_body_available(&proposal)
            {
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
            if proposal.descriptor.proposal_height != self.context.height
                || (self.decision_pending()
                    && !self.proposal_is_bound_to_decided_carrier(&proposal))
                || !self.proposal_body_available(&proposal)
            {
                continue;
            }
            lane_artifacts.push((
                BlockMessage::LaneBlockVote(vote),
                proposal.descriptor.validator_set,
            ));
        }
        for qc in self.lane_sessions.qcs_for_incomplete_sessions() {
            if qc.body.proposal_height != self.context.height
                || (self.decision_pending()
                    && !self.lane_message_is_allowed_after_decision(&BlockMessage::LaneBlockQc(
                        qc.clone(),
                    )))
                || !self.lane_vote_body_available(&qc.body)
            {
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
                if !self.push_effect(V2LaneWorkEffect::PostNativeAmx {
                    peer,
                    reply_routes: None,
                    message,
                }) {
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
        let reply_routes = match post.reply_route {
            Some(reply_route) => {
                let Ok(reply_routes) = NetworkReplyRoutes::try_from_route(reply_route) else {
                    return false;
                };
                Some(reply_routes)
            }
            None => None,
        };
        self.push_merge_sidecar_effect(V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: post.peer,
            reply_routes,
            message: post.message,
        })
    }

    /// Transfer an already-reserved sidecar post into the lane effect queue.
    ///
    /// These posts advance transport-owned request or chunk state before this
    /// boundary.  The transfer therefore must execute in every build and fail
    /// closed if the reserved queue cannot retain it.  Request production can
    /// be rolled back exactly; response production remains protected by the
    /// surrounding fail-stop operation until its peer-writer-flush receipt is
    /// applied.
    fn push_merge_sidecar_post_or_restart(
        &mut self,
        post: MergeSidecarPost,
    ) -> Result<(), V2LaneWorkError> {
        let retired_response_route = match (&post.message, &post.reply_route) {
            (CertifiedMergeSidecarMessage::Chunk(_), Some(route)) => Some(route.clone()),
            _ => None,
        };
        let unsent_request = match &post.message {
            CertifiedMergeSidecarMessage::Request(request) => Some(request.clone()),
            CertifiedMergeSidecarMessage::Chunk(_) => None,
        };
        if self.push_merge_sidecar_post(post) {
            return Ok(());
        }
        if retired_response_route.is_some_and(|route| !route.is_active()) {
            // The sidecar transport still owns the in-flight current chunk.
            // Its next bounded prune records that cursor for a reconnect; one
            // retired source must not poison live siblings already drained in
            // the same service slice.
            return Ok(());
        }
        if let Some(request) = unsent_request {
            self.merge_sidecars.release_unsent_request(&request);
        }
        Err(V2LaneWorkError::RestartRequired)
    }

    /// Apply one exact peer-writer flush receipt and schedule the source's next
    /// chunk without changing any sibling source cursor.
    pub(crate) fn acknowledge_certified_merge_sidecar_chunk_admission(
        &mut self,
        admission: &CertifiedMergeSidecarChunkAdmission,
        now: Instant,
    ) -> Result<(), V2LaneWorkError> {
        let output_guard = Arc::clone(&self.output_guard);
        let operation = output_guard
            .begin_fail_stop_operation()
            .ok_or(V2LaneWorkError::RestartRequired)?;
        let acknowledged = self
            .merge_sidecars
            .acknowledge_outbound_chunk(admission, now)
            .map_err(|_| V2LaneWorkError::RestartRequired)?;
        if acknowledged {
            let posts = self
                .merge_sidecars
                .drain_outbound_chunks(self.sidecar_effect_slots().min(8), now);
            for post in posts {
                self.push_merge_sidecar_post_or_restart(post)?;
            }
        }
        operation.complete();
        Ok(())
    }

    fn push_merge_sidecar_effect(&mut self, effect: V2LaneWorkEffect) -> bool {
        if !matches!(&effect, V2LaneWorkEffect::PostCertifiedMergeSidecar { .. })
            || !lane_work_effect_reply_routes_have_valid_shape(&effect)
        {
            return false;
        }
        let key = lane_work_effect_key(&effect);
        if self.sidecar_effect_keys.contains(&key) {
            return self
                .sidecar_effects
                .iter_mut()
                .find(|queued| lane_work_effect_key(queued) == key)
                .is_some_and(|queued| merge_lane_work_effect_reply_routes(queued, &effect));
        }
        if !lane_work_effect_reply_routes_are_valid(&effect) {
            return false;
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
        reply_route: Option<NetworkReplyRoute>,
        message: CertifiedMergeSidecarMessage,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        match message {
            CertifiedMergeSidecarMessage::Request(request) => {
                self.accept_certified_merge_sidecar_request(sender, reply_route, request)
            }
            CertifiedMergeSidecarMessage::Chunk(chunk) => {
                self.accept_certified_merge_sidecar_chunk(sender, chunk)
            }
        }
    }

    #[cfg(test)]
    pub(in crate::sumeragi) fn accept_certified_merge_sidecar_for_test(
        &mut self,
        sender: PeerId,
        reply_route: NetworkReplyRoute,
        request: crate::merge_sidecar::CertifiedMergeSidecarRequestV1,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        self.accept_certified_merge_sidecar(
            sender,
            Some(reply_route),
            CertifiedMergeSidecarMessage::Request(request),
        )
    }

    fn accept_certified_merge_sidecar_request(
        &mut self,
        sender: PeerId,
        reply_route: Option<NetworkReplyRoute>,
        request: crate::merge_sidecar::CertifiedMergeSidecarRequestV1,
    ) -> Result<V2LaneIngressOutcome, V2LaneWorkError> {
        let Some(reply_route) = reply_route else {
            return Ok(V2LaneIngressOutcome::Rejected);
        };
        let now = Instant::now();
        let materialize = match self.merge_sidecars.admit_server_request(
            &sender,
            &request,
            Some(&reply_route),
            &self.local_peer,
            now,
        ) {
            Ok(materialize) => materialize,
            Err(error) => {
                iroha_logger::debug!(%sender, ?error, "dropping v2 certified merge-sidecar request");
                return Ok(V2LaneIngressOutcome::Rejected);
            }
        };
        if !materialize {
            let posts = self
                .merge_sidecars
                .drain_outbound_chunks(self.sidecar_effect_slots().min(8), now);
            let inserted = !posts.is_empty();
            for post in posts {
                self.push_merge_sidecar_post_or_restart(post)?;
            }
            return Ok(if inserted {
                V2LaneIngressOutcome::Inserted
            } else {
                V2LaneIngressOutcome::Duplicate
            });
        }
        let entry = match self.kura.merge_entry_by_hash(request.entry_hash) {
            Ok(Some(entry)) => entry,
            Ok(None) => {
                self.merge_sidecars
                    .cancel_unmaterialized_server_request(&sender, &request);
                return Ok(V2LaneIngressOutcome::Rejected);
            }
            Err(error) => {
                self.merge_sidecars
                    .cancel_unmaterialized_server_request(&sender, &request);
                return Err(V2LaneWorkError::Persistence(error.to_string()));
            }
        };
        if entry.execution_batch.is_some() {
            self.merge_sidecars
                .cancel_unmaterialized_server_request(&sender, &request);
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let reference = CertifiedMergeLedgerReference::new(&entry);
        let metadata_matches = request.encoded_len == reference.encoded_len
            && request.epoch_id == reference.epoch_id
            && request.reference_digest == certified_merge_reference_digest(&reference);
        let local_is_holder = certified_merge_sidecar_holders(&reference)
            .is_ok_and(|holders| holders.contains(&self.local_peer));
        if !metadata_matches || !local_is_holder {
            self.merge_sidecars
                .cancel_unmaterialized_server_request(&sender, &request);
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        if let Err(error) = self.merge_sidecars.enqueue_response(
            request.clone(),
            Some(reply_route),
            entry.canonical_bytes(),
            now,
        ) {
            self.merge_sidecars
                .cancel_unmaterialized_server_request(&sender, &request);
            iroha_logger::debug!(%sender, ?error, "v2 merge-sidecar response budget rejected request");
            return Ok(V2LaneIngressOutcome::Rejected);
        }
        let posts = self
            .merge_sidecars
            .drain_outbound_chunks(self.sidecar_effect_slots().min(8), now);
        let inserted = !posts.is_empty();
        for post in posts {
            self.push_merge_sidecar_post_or_restart(post)?;
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

    fn insert_lane_certificate(
        &mut self,
        certificate: LaneBlockCertificateV1,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let LaneBlockCertificateV1 {
            proposal,
            prepare_qc,
            commit_qc,
        } = certificate;
        if prepare_qc.body != proposal.vote_body(CertPhase::Prepare)
            || commit_qc.body != proposal.vote_body(CertPhase::Commit)
            || !self.proposal_body_available(&proposal)
            || !self.lane_qc_authorized(&prepare_qc, active_view)
            || !self.lane_qc_authorized(&commit_qc, active_view)
        {
            return V2LaneIngressOutcome::Rejected;
        }

        if proposal.descriptor.proposal_height > self.context.height {
            return V2LaneIngressOutcome::Rejected;
        }
        if proposal.descriptor.proposal_height < self.context.height {
            return self.insert_historical_lane_certificate(proposal, prepare_qc, commit_qc);
        }
        if self
            .committed_lane_outputs
            .len()
            .saturating_add(self.historical_recovery_sessions.len())
            >= self.limits.session_capacity.get()
        {
            return V2LaneIngressOutcome::Rejected;
        }

        let mut next_sessions = self.lane_sessions.clone();
        let mut inserted = false;
        for outcome in [
            next_sessions.insert_recovered_proposal_replacing_uncommitted_conflict(proposal),
            next_sessions
                .insert_qc_with_pops(prepare_qc.clone(), &self.pops_for_lane_qc(&prepare_qc)),
            next_sessions
                .insert_qc_with_pops(commit_qc.clone(), &self.pops_for_lane_qc(&commit_qc)),
        ] {
            match outcome {
                Ok(LaneBlockSessionInsertOutcome::Inserted) => inserted = true,
                Ok(LaneBlockSessionInsertOutcome::Duplicate) => {}
                Err(_) => return V2LaneIngressOutcome::Rejected,
            }
        }
        self.lane_sessions = next_sessions;
        if inserted {
            V2LaneIngressOutcome::Inserted
        } else {
            V2LaneIngressOutcome::Duplicate
        }
    }

    fn insert_historical_lane_certificate(
        &mut self,
        proposal: LaneBlockProposalV1,
        prepare_qc: LaneBlockQcV1,
        commit_qc: LaneBlockQcV1,
    ) -> V2LaneIngressOutcome {
        let session = CommittedLaneBlockSession {
            proposal,
            prepare_qc,
            commit_qc,
        };
        let pops = self.pops_for_lane_session(&session);
        let candidate = CertifiedLaneBlockArtifact::new(session.clone(), pops);
        if Kura::validate_certified_lane_block_artifact(&candidate).is_err() {
            return V2LaneIngressOutcome::Rejected;
        }
        if self
            .state
            .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(&session)
        {
            return V2LaneIngressOutcome::Duplicate;
        }
        if self
            .historical_recovery_sessions
            .iter()
            .any(|pending| pending.proposal == session.proposal)
        {
            return V2LaneIngressOutcome::Duplicate;
        }
        let retained_sessions = self
            .committed_lane_outputs
            .len()
            .saturating_add(self.historical_recovery_sessions.len());
        if retained_sessions >= self.limits.session_capacity.get() {
            return V2LaneIngressOutcome::Rejected;
        }
        self.historical_recovery_sessions.push_back(session);
        V2LaneIngressOutcome::Inserted
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
        (local && &self.local_peer == author) || sender == Some(author)
    }

    fn expected_lane_author<'a>(&'a self, proposal: &'a LaneBlockProposalV1) -> Option<&'a PeerId> {
        let nexus = self.state.nexus_snapshot();
        if !nexus.enabled
            || !proposal_lookahead_enabled(&nexus, proposal.descriptor.proposal_height)
        {
            let proposal_view = proposal.payload_block_hint?.proposal_view;
            let index = usize::try_from(self.context.leader(proposal_view)).ok()?;
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
            if proposal.descriptor.proposal_height != self.context.height
                || (self.decision_pending()
                    && !self.proposal_is_bound_to_decided_carrier(&proposal))
                || !self.proposal_body_available(&proposal)
            {
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
            if request.proposal.descriptor.proposal_height != self.context.height
                || (self.decision_pending()
                    && !self.proposal_is_bound_to_decided_carrier(&request.proposal))
                || !self.proposal_body_available(&request.proposal)
            {
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
            if qc.body.proposal_height != self.context.height
                || (self.decision_pending()
                    && !self.lane_message_is_allowed_after_decision(&BlockMessage::LaneBlockQc(
                        qc.clone(),
                    )))
            {
                // Other-height certificates are recovery inputs, not fresh
                // current-carrier fanout. Their durable Kura source answers
                // exact proposal retransmissions after local persistence.
                continue;
            }
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
        if !lane_output_identity(&message)
            .is_some_and(|(proposal_height, _)| proposal_height == self.context.height)
            || (self.decision_pending() && !self.lane_message_is_allowed_after_decision(&message))
        {
            return;
        }
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
        if !lane_work_effect_reply_routes_have_valid_shape(&effect) {
            return false;
        }
        let key = lane_work_effect_key(&effect);
        if self.effect_keys.contains(&key) {
            return self
                .effects
                .iter_mut()
                .find(|queued| lane_work_effect_key(queued) == key)
                .is_some_and(|queued| merge_lane_work_effect_reply_routes(queued, &effect));
        }
        if !lane_work_effect_reply_routes_are_valid(&effect) {
            return false;
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

    fn purge_queued_global_body_effects_except_committed_outputs(&mut self) {
        let committed_proposals = self
            .committed_lane_outputs
            .iter()
            .map(|output| output.session.proposal.proposal_hash)
            .collect::<BTreeSet<_>>();
        let allowed_durable_certificates = self
            .effects
            .iter()
            .filter_map(|effect| {
                let V2LaneWorkEffect::PostDurableLaneCertificate { certificate, .. } = effect
                else {
                    return None;
                };
                (certificate.proposal.descriptor.proposal_height < self.context.height
                    || self.proposal_is_bound_to_decided_carrier(&certificate.proposal))
                .then(|| lane_work_effect_key(effect))
            })
            .collect::<BTreeSet<_>>();
        self.effects.retain(|effect| match effect {
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockQc(qc),
                ..
            } => {
                qc.body.phase == CertPhase::Commit
                    && committed_proposals.contains(&qc.body.proposal_hash)
            }
            V2LaneWorkEffect::PostLaneBlock { .. } | V2LaneWorkEffect::BroadcastMerge(_) => false,
            V2LaneWorkEffect::PostDurableLaneCertificate { .. } => {
                allowed_durable_certificates.contains(&lane_work_effect_key(effect))
            }
            _ => true,
        });
        self.effect_keys = self.effects.iter().map(lane_work_effect_key).collect();
    }

    fn retain_committed_lane_outputs_for_subject(&mut self, subject: wire::BlockSubject) {
        self.pending_committed_lanes.retain(|session| {
            session
                .proposal
                .payload_block_hint
                .as_ref()
                .is_some_and(|hint| hint.proposal_block_hash == subject.block_hash)
        });
        self.committed_lane_outputs.retain(|output| {
            output
                .session
                .proposal
                .payload_block_hint
                .is_some_and(|hint| hint.proposal_block_hash == subject.block_hash)
        });
    }

    fn schedule_committed_lane_outputs(&mut self) {
        let output_count = self.committed_lane_outputs.len();
        if output_count == 0 {
            self.committed_lane_output_cursor = 0;
            return;
        }

        let mut consecutive_complete = 0usize;
        while consecutive_complete < output_count {
            let output_index = self.committed_lane_output_cursor % output_count;
            self.committed_lane_output_cursor = (output_index + 1) % output_count;
            let attempt = {
                let output = &self.committed_lane_outputs[output_index];
                let validators = &output.session.commit_qc.validator_set;
                let mut validator_index = output.next_validator;
                while validator_index < validators.len()
                    && validators[validator_index] == self.local_peer
                {
                    validator_index = validator_index.saturating_add(1);
                }
                (validator_index < validators.len()).then(|| {
                    (
                        validator_index,
                        validators[validator_index].clone(),
                        output.session.commit_qc.clone(),
                    )
                })
            };
            let Some((validator_index, peer, commit_qc)) = attempt else {
                self.committed_lane_outputs[output_index].next_validator = self
                    .committed_lane_outputs[output_index]
                    .session
                    .commit_qc
                    .validator_set
                    .len();
                consecutive_complete = consecutive_complete.saturating_add(1);
                continue;
            };
            self.committed_lane_outputs[output_index].next_validator = validator_index;
            if !self.push_effect(V2LaneWorkEffect::PostLaneBlock {
                peer,
                message: BlockMessage::LaneBlockQc(commit_qc),
            }) {
                return;
            }
            self.committed_lane_outputs[output_index].next_validator =
                validator_index.saturating_add(1);
            consecutive_complete = 0;
        }
    }

    /// Return whether a completed lane CommitQC still awaits transfer into the
    /// network actor's exact-output corridor.
    pub(crate) fn has_pending_committed_output_handoff(&self) -> bool {
        let committed_proposals = self
            .committed_lane_outputs
            .iter()
            .map(|output| output.session.proposal.proposal_hash)
            .collect::<BTreeSet<_>>();
        self.committed_lane_outputs.iter().any(|output| {
            output.session.commit_qc.validator_set[output.next_validator..]
                .iter()
                .any(|peer| peer != &self.local_peer)
        }) || self.effects.iter().any(|effect| {
            matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockQc(qc),
                    ..
                } if qc.body.phase == CertPhase::Commit
                    && committed_proposals.contains(&qc.body.proposal_hash)
            )
        })
    }

    fn collect_committed_lane_sessions(&mut self) {
        let remaining = self.limits.session_capacity.get().saturating_sub(
            self.committed_lane_outputs
                .len()
                .saturating_add(self.historical_recovery_sessions.len()),
        );
        for session in self.lane_sessions.drain_committed_sessions_up_to(remaining) {
            if session.proposal.descriptor.proposal_height < self.context.height {
                if !self
                    .historical_recovery_sessions
                    .iter()
                    .any(|pending| pending.proposal == session.proposal)
                    && !self
                        .state
                        .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(
                            &session,
                        )
                {
                    self.historical_recovery_sessions.push_back(session);
                }
                continue;
            }
            self.committed_lane_outputs
                .push_back(PendingCommittedLaneOutput {
                    session: session.clone(),
                    next_validator: 0,
                });
            self.pending_committed_lanes.push_back(session);
        }
        self.schedule_committed_lane_outputs();
    }

    fn proposal_body_available(&self, proposal: &LaneBlockProposalV1) -> bool {
        self.canonical_anchor_for_proposal(proposal).is_some()
            || self
                .locally_bound_lane_proposals
                .get(&proposal.proposal_hash)
                .is_some_and(|hint| proposal.payload_block_hint.as_ref() == Some(hint))
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
            .enumerate()
            .filter(|(index, _)| {
                qc.signers_bitmap
                    .get(index / 8)
                    .is_some_and(|byte| byte & (1_u8 << (index % 8)) != 0)
            })
            .filter_map(|(_, peer)| {
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
        reply_route: Option<NetworkReplyRoute>,
        message: NativeAmxMessage,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        match message {
            NativeAmxMessage::PrepareRequest(request) => {
                self.accept_native_request(sender, reply_route, request, None, active_view)
            }
            NativeAmxMessage::CommitRequest(request) => self.accept_native_request(
                sender,
                reply_route,
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
        reply_route: Option<NetworkReplyRoute>,
        request: NativeAmxAttestationRequestV2,
        prepare_qc: Option<NativeAmxAttestationQcV2>,
        active_view: wire::View,
    ) -> V2LaneIngressOutcome {
        let Some(reply_route) = reply_route else {
            return V2LaneIngressOutcome::Rejected;
        };
        let body = request.body;
        if reply_route.semantic_target() != &sender {
            return V2LaneIngressOutcome::Rejected;
        }
        let Ok(reply_routes) = NetworkReplyRoutes::try_from_route(reply_route) else {
            return V2LaneIngressOutcome::Rejected;
        };
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
            reply_routes: Some(reply_routes),
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
        let validators = self.expected_lane_validators(participant_lane, authority_height)?;
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
            if self.push_effect(V2LaneWorkEffect::PostNativeAmx {
                peer,
                reply_routes: None,
                message,
            }) {
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
            || self.globally_locked_body.is_some()
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
        if self.globally_locked_body.is_some() || carrier_protected {
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

fn reply_routes_are_live_for_peer(reply_routes: &NetworkReplyRoutes, peer: &PeerId) -> bool {
    !reply_routes.is_empty()
        && reply_routes.semantic_target() == peer
        && reply_routes.iter().any(NetworkReplyRoute::is_active)
}

fn lane_work_effect_reply_routes_have_valid_shape(effect: &V2LaneWorkEffect) -> bool {
    let reply_routes_target_peer = |reply_routes: &NetworkReplyRoutes, peer: &PeerId| {
        !reply_routes.is_empty() && reply_routes.semantic_target() == peer
    };
    match effect {
        V2LaneWorkEffect::PostLaneBlock { .. } | V2LaneWorkEffect::BroadcastMerge(_) => true,
        V2LaneWorkEffect::PostDurableLaneCertificate {
            peer,
            reply_routes,
            ingress_ownership,
            ..
        } => reply_routes.as_ref().is_some_and(|routes| {
            reply_routes_target_peer(routes, peer)
                && ingress_ownership.as_ref().map_or(cfg!(test), |ownership| {
                    ownership.validate_exact() && ownership.matches_reply_routes(Some(routes))
                })
        }),
        V2LaneWorkEffect::PostNativeAmx {
            peer,
            reply_routes,
            message,
        } => match message {
            NativeAmxMessage::PrepareRequest(_) | NativeAmxMessage::CommitRequest(_) => {
                reply_routes.is_none()
            }
            NativeAmxMessage::PrepareVote(_) | NativeAmxMessage::CommitVote(_) => reply_routes
                .as_ref()
                .is_some_and(|routes| reply_routes_target_peer(routes, peer)),
        },
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer,
            reply_routes,
            message,
        } => match message {
            CertifiedMergeSidecarMessage::Request(_) => reply_routes.is_none(),
            CertifiedMergeSidecarMessage::Chunk(_) => reply_routes
                .as_ref()
                .is_some_and(|routes| reply_routes_target_peer(routes, peer)),
        },
    }
}

fn lane_work_effect_reply_routes_are_valid(effect: &V2LaneWorkEffect) -> bool {
    match effect {
        V2LaneWorkEffect::PostLaneBlock { .. } | V2LaneWorkEffect::BroadcastMerge(_) => true,
        V2LaneWorkEffect::PostDurableLaneCertificate {
            peer,
            reply_routes,
            ingress_ownership,
            ..
        } => reply_routes.as_ref().is_some_and(|routes| {
            reply_routes_are_live_for_peer(routes, peer)
                && ingress_ownership.as_ref().map_or(cfg!(test), |ownership| {
                    ownership.validate_exact() && ownership.matches_reply_routes(Some(routes))
                })
        }),
        V2LaneWorkEffect::PostNativeAmx {
            peer,
            reply_routes,
            message,
        } => match message {
            NativeAmxMessage::PrepareRequest(_) | NativeAmxMessage::CommitRequest(_) => {
                reply_routes.is_none()
            }
            NativeAmxMessage::PrepareVote(_) | NativeAmxMessage::CommitVote(_) => reply_routes
                .as_ref()
                .is_some_and(|routes| reply_routes_are_live_for_peer(routes, peer)),
        },
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer,
            reply_routes,
            message,
        } => match message {
            CertifiedMergeSidecarMessage::Request(_) => reply_routes.is_none(),
            CertifiedMergeSidecarMessage::Chunk(_) => reply_routes
                .as_ref()
                .is_some_and(|routes| reply_routes_are_live_for_peer(routes, peer)),
        },
    }
}

fn merge_optional_reply_routes(
    queued: &mut Option<NetworkReplyRoutes>,
    candidate: &Option<NetworkReplyRoutes>,
) -> bool {
    match (queued, candidate) {
        (Some(queued), Some(candidate)) => {
            // Reconcile the complete observed history on a shadow set. A
            // successful maintenance-only merge still commits retirement and
            // tombstone history even when no candidate delivery remains live.
            let mut merged = queued.clone();
            if merged.merge_observed(candidate).is_err() {
                return false;
            }
            let retained_active_candidate = candidate.iter().any(|candidate_route| {
                candidate_route.is_active()
                    && merged
                        .iter()
                        .any(|route| route.same_delivery(candidate_route))
            });
            *queued = merged;
            retained_active_candidate
        }
        (None, None) => true,
        (None, Some(_)) | (Some(_), None) => false,
    }
}

fn merge_lane_work_effect_reply_routes(
    queued: &mut V2LaneWorkEffect,
    candidate: &V2LaneWorkEffect,
) -> bool {
    // A queued source may have disconnected since reservation. The
    // per-source merge prunes such retained attempts while allowing other live
    // members of the same occurrence to attach.
    if !lane_work_effect_reply_routes_have_valid_shape(candidate) {
        return false;
    }
    match (queued, candidate) {
        (
            V2LaneWorkEffect::PostDurableLaneCertificate {
                reply_routes: queued_routes,
                ingress_ownership: queued_ownership,
                ..
            },
            V2LaneWorkEffect::PostDurableLaneCertificate {
                reply_routes: candidate_routes,
                ingress_ownership: candidate_ownership,
                ..
            },
        ) => {
            let mut merged_routes = queued_routes.clone();
            let retained_candidate_route =
                merge_optional_reply_routes(&mut merged_routes, candidate_routes);
            let mut merged_ownership = queued_ownership.clone();
            match (&mut merged_ownership, candidate_ownership) {
                (Some(retained), Some(candidate)) => {
                    if !retained.merge_downstream(candidate.clone()) {
                        return false;
                    }
                }
                (None, None) if cfg!(test) => {}
                (Some(_), None) | (None, Some(_)) | (None, None) => return false,
            }
            if merged_ownership.as_ref().is_some_and(|ownership| {
                !ownership.matches_reply_routes(merged_routes.as_ref())
                    || !ownership.validate_exact()
            }) {
                return false;
            }
            *queued_routes = merged_routes;
            *queued_ownership = merged_ownership;
            retained_candidate_route
        }
        (
            V2LaneWorkEffect::PostNativeAmx {
                reply_routes: queued,
                ..
            },
            V2LaneWorkEffect::PostNativeAmx {
                reply_routes: candidate,
                ..
            },
        )
        | (
            V2LaneWorkEffect::PostCertifiedMergeSidecar {
                reply_routes: queued,
                ..
            },
            V2LaneWorkEffect::PostCertifiedMergeSidecar {
                reply_routes: candidate,
                ..
            },
        ) => merge_optional_reply_routes(queued, candidate),
        (V2LaneWorkEffect::PostLaneBlock { .. }, V2LaneWorkEffect::PostLaneBlock { .. })
        | (V2LaneWorkEffect::BroadcastMerge(_), V2LaneWorkEffect::BroadcastMerge(_)) => true,
        _ => false,
    }
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
        V2LaneWorkEffect::PostDurableLaneCertificate {
            peer, certificate, ..
        } => {
            encoded.push(4);
            encoded.extend(peer.encode());
            encoded.extend(certificate.encode());
        }
        V2LaneWorkEffect::PostNativeAmx { peer, message, .. } => {
            encoded.push(1);
            encoded.extend(peer.encode());
            encoded.extend(message.encode());
        }
        V2LaneWorkEffect::BroadcastMerge(signature) => {
            encoded.push(2);
            encoded.extend(signature.encode());
        }
        V2LaneWorkEffect::PostCertifiedMergeSidecar { peer, message, .. } => {
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

/// Return whether every lane ownership in an authenticated finality artifact
/// has its exact durable certificate and global application receipt.
///
/// Missing evidence is the recoverable applied-tip crash boundary. Conflicting
/// evidence is an invalid durable state and therefore returns an error instead
/// of being treated as incomplete.
pub(crate) fn durable_lane_completion_matches_finality(
    kura: &Kura,
    finality_artifact: &wire::finality::V2FinalityArtifact,
) -> Result<bool, String> {
    finality_artifact
        .validate()
        .map_err(|error| error.to_string())?;
    let height = usize::try_from(finality_artifact.height)
        .ok()
        .and_then(NonZeroUsize::new)
        .ok_or_else(|| "lane finality has an invalid zero height".to_owned())?;
    let block = kura
        .get_block(height)
        .ok_or_else(|| "lane finality has no canonical block body".to_owned())?;
    let canonical_payload_hash = block
        .canonical_proposal_wire_hash()
        .map_err(|error| error.to_string())?;
    let canonical_subject = wire::BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: canonical_payload_hash,
    };
    if block.header().height().get() != finality_artifact.height
        || canonical_subject != finality_artifact.subject
        || finality_artifact.height_context.height != finality_artifact.height
    {
        return Err("lane finality differs from its canonical block or height context".to_owned());
    }
    let ownerships = block
        .execution_context()
        .map_or(&[][..], |bundle| bundle.lane_payload_ownerships.as_slice());
    let mut proposal_hashes = BTreeSet::new();
    for ownership in ownerships {
        if ownership.proposal_height != finality_artifact.height {
            return Err("lane ownership is bound to another global height".to_owned());
        }
        let proposal = proposal_from_ownership(ownership, finality_artifact.block_hash)
            .ok_or_else(|| "lane ownership cannot reconstruct its exact proposal".to_owned())?;
        if !proposal_hashes.insert(proposal.proposal_hash) {
            return Err("canonical block contains duplicate lane proposal ownership".to_owned());
        }
        let Some(certified) =
            kura.read_certified_lane_block_artifact(ownership.lane_id, ownership.lane_block_height)
        else {
            return Ok(false);
        };
        Kura::validate_certified_lane_block_artifact(&certified)
            .map_err(|message| format!("durable certified lane artifact is invalid: {message}"))?;
        let Some(receipt) = kura
            .read_lane_block_application_receipt(ownership.lane_id, ownership.lane_block_height)
        else {
            return Ok(false);
        };
        if certified.proposal != proposal
            || receipt.proposal != proposal
            || receipt.artifact.proposal_block_hash != finality_artifact.block_hash
            || receipt.artifact.ownership != *ownership
            || receipt.application_block_height != finality_artifact.height
            || receipt.application_block_hash != finality_artifact.block_hash
        {
            return Err(
                "durable lane certificate or receipt conflicts with the finalized ownership"
                    .to_owned(),
            );
        }
    }
    Ok(true)
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
        expected_author.is_some()
            && (!shared_committee || expected_author == Some(global_leader))
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
/// Shared durable-lane fixtures and lane-work unit tests.
pub(super) mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        num::{NonZeroU32, NonZeroU64, NonZeroUsize},
        sync::{
            Arc, Barrier,
            atomic::{AtomicUsize, Ordering},
            mpsc,
        },
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
        nexus::{
            DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog, LaneConfig,
            LaneFastpqProofMaterial, LaneId, LaneStorageProfile, LaneVisibility,
        },
        peer::PeerId,
        transaction::{TransactionBuilder, TransactionEntrypoint, signed::TransactionResultInner},
        trigger::DataTriggerSequence,
    };
    use mv::storage::StorageReadOnly;

    use super::*;
    use crate::{
        block::{CommittedBlock, ValidBlock},
        governance::manifest::{
            GovernanceRules, LaneManifestRegistry, LaneManifestStatus, ManifestValidatorBinding,
        },
        merge_sidecar::CertifiedMergeSidecarChunkV1,
        query::store::LiveQueryStore,
        state::World,
        sumeragi::{
            fair_v2_ingress_admit_for_test, network_topology::Topology,
            v2_worker::tests::service_for_history_context,
        },
    };

    pub(in crate::sumeragi) fn fixture(
        mode: wire::ConsensusMode,
    ) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
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

    /// Exact certified lane source shared with worker rollover tests.
    pub(in crate::sumeragi) struct DurableLaneHistoryFixture {
        /// Kura containing the certified lane artifact.
        pub(in crate::sumeragi) kura: Arc<Kura>,
        /// Exact certificate reconstructed from the Kura artifact.
        pub(in crate::sumeragi) certificate: LaneBlockCertificateV1,
        /// Global context which anchored the certified lane proposal.
        pub(in crate::sumeragi) context: wire::HeightContext,
        /// Validator keys used only by deterministic tests.
        pub(in crate::sumeragi) validators: Vec<KeyPair>,
    }

    /// Persist one exact certified lane artifact for worker rollover tests.
    pub(in crate::sumeragi) fn durable_lane_history_fixture() -> DurableLaneHistoryFixture {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (_, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        let session = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        let pops = adapter.pops_for_lane_session(&session);
        adapter
            .kura
            .persist_committed_lane_block_session(&session, &pops)
            .expect("persist durable lane-history fixture");
        let certificate = LaneBlockCertificateV1 {
            proposal: session.proposal,
            prepare_qc: session.prepare_qc,
            commit_qc: session.commit_qc,
        };
        DurableLaneHistoryFixture {
            kura: Arc::clone(&adapter.kura),
            certificate,
            context: adapter.context.clone(),
            validators: keys,
        }
    }

    fn fixture_at_height_inner(
        mode: wire::ConsensusMode,
        height: u64,
        persist_parent_chain: bool,
    ) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
        let nonzero = NonZeroUsize::new(8).expect("nonzero");
        fixture_at_height_inner_with_limits(
            mode,
            height,
            persist_parent_chain,
            V2LaneWorkLimits::new(
                nonzero, nonzero, nonzero, nonzero, nonzero, nonzero, nonzero,
            ),
        )
    }

    fn fixture_at_height_inner_with_limits(
        mode: wire::ConsensusMode,
        height: u64,
        persist_parent_chain: bool,
        limits: V2LaneWorkLimits,
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
                proposal_round: wire::ConsensusRound {
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
            limits,
            authenticated_genesis_nexus_amx_context,
            None,
            ConsensusOutputGuard::isolated(),
        )
        .expect("open lane adapter");
        (adapter, keys)
    }

    fn enable_multilane_nexus(
        adapter: &mut V2LaneWorkAdapter,
        keys: &[KeyPair],
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
    ) -> Vec<PeerId> {
        let lane_catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("non-zero lane count"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: lane_id,
                    dataspace_id,
                    alias: "independent-lane".to_owned(),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("multi-lane test catalog");
        let dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: dataspace_id,
                alias: "independent-dataspace".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("multi-lane test dataspace catalog");
        {
            let mut nexus = adapter.state.nexus.write();
            nexus.enabled = true;
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog);
            nexus.lane_catalog = lane_catalog;
            nexus.dataspace_catalog = dataspace_catalog;
        }
        adapter.state.reseed_static_lane_incarnations_for_tests();

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
            alias: "independent-lane".to_owned(),
            dataspace: dataspace_id,
            visibility: LaneVisibility::Public,
            storage: LaneStorageProfile::FullReplica,
            governance: Some("independent-lane-governance".to_owned()),
            manifest_path: Some(std::path::PathBuf::from(
                "/tmp/v2-independent-lane-manifest.json",
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
        adapter.context.nexus_amx_context_hash =
            super::super::v2_recovery::committed_nexus_amx_context_hash(adapter.state.as_ref());

        assert!(proposal_lookahead_enabled(
            &adapter.state.nexus_snapshot(),
            adapter.context.height,
        ));
        let mut validators = adapter
            .state
            .authoritative_lane_peer_ids_at_height(lane_id, adapter.context.height);
        validators.sort();
        validators.dedup();
        assert_eq!(validators.len(), keys.len());
        validators
    }

    fn limits_with_native_capacity(
        session_capacity: usize,
        body_buckets_per_session: usize,
    ) -> V2LaneWorkLimits {
        let one = NonZeroUsize::new(1).expect("non-zero fixture limit");
        V2LaneWorkLimits::new(
            NonZeroUsize::new(session_capacity).expect("non-zero session capacity"),
            NonZeroUsize::new(body_buckets_per_session).expect("non-zero body-bucket capacity"),
            one,
            one,
            one,
            one,
            one,
        )
    }

    #[test]
    fn native_amx_signing_guard_capacity_preserves_small_product() {
        let capacity = native_amx_signing_guard_capacity(limits_with_native_capacity(8, 16))
            .expect("small representable capacity");
        assert_eq!(capacity.get(), 128);
    }

    #[test]
    fn native_amx_signing_guard_capacity_preserves_exact_hard_boundary() {
        let capacity = native_amx_signing_guard_capacity(limits_with_native_capacity(2_048, 512))
            .expect("exact protocol boundary");
        assert_eq!(capacity.get(), MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD);
    }

    #[test]
    fn native_amx_signing_guard_capacity_caps_deployed_localnet_oversized_product() {
        let capacity =
            native_amx_signing_guard_capacity(limits_with_native_capacity(20_000, 10_000))
                .expect("representable deployed-localnet capacity");
        assert_eq!(capacity.get(), MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD);
    }

    #[test]
    fn native_amx_signing_guard_capacity_rejects_usize_overflow() {
        let error = native_amx_signing_guard_capacity(limits_with_native_capacity(usize::MAX, 2))
            .expect_err("overflow must fail closed");
        assert!(matches!(
            error,
            V2LaneWorkError::SigningGuard(message)
                if message == "native AMX signing-record capacity overflows usize"
        ));
    }

    #[test]
    fn validator_storage_platform_gate_rejects_voters_and_allows_observers() {
        assert_eq!(
            require_validator_storage_platform(true, false),
            Err(V2LaneWorkError::UnsupportedValidatorStoragePlatform),
            "every voter must fail before opening any key-specific signing guard"
        );
        assert_eq!(require_validator_storage_platform(false, false), Ok(()));
        assert_eq!(require_validator_storage_platform(true, true), Ok(()));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn native_amx_adapter_opens_with_bounded_production_like_limits() {
        assert!(sumeragi_v2_validator_storage_supported());

        let limits = limits_with_native_capacity(4_096, 512);
        let (adapter, _) = fixture_at_height_inner_with_limits(
            wire::ConsensusMode::Permissioned,
            9,
            false,
            limits,
        );
        assert_eq!(
            adapter
                .native_signing_guard
                .as_ref()
                .expect("BLS validator has a durable Native AMX guard")
                .max_records_for_test(),
            MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD,
        );
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

    /// Real Kura-backed sidecar server state shared with writer-flush runner tests.
    pub(in crate::sumeragi) struct CertifiedSidecarServerFixture {
        pub(in crate::sumeragi) adapter: V2LaneWorkAdapter,
        pub(in crate::sumeragi) validators: Vec<KeyPair>,
        pub(in crate::sumeragi) kura: Arc<Kura>,
        pub(in crate::sumeragi) context: wire::HeightContext,
        pub(in crate::sumeragi) local_validator: wire::ValidatorIndex,
        pub(in crate::sumeragi) requester: PeerId,
        pub(in crate::sumeragi) request: crate::merge_sidecar::CertifiedMergeSidecarRequestV1,
    }

    /// Persist one canonical merge entry and construct its exact server request.
    pub(in crate::sumeragi) fn certified_sidecar_server_fixture() -> CertifiedSidecarServerFixture {
        let (adapter, validators) = fixture(wire::ConsensusMode::Permissioned);
        let entry = pending_sidecar_entry(&adapter, &validators, 1);
        let entry_hash = adapter
            .kura
            .persist_pending_certified_merge_entry(&entry)
            .expect("persist Kura-backed sidecar server fixture");
        let reference = CertifiedMergeLedgerReference::new(&entry);
        assert_eq!(entry_hash, reference.entry_hash);
        let requester = adapter
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .find(|peer| peer != &adapter.local_peer)
            .expect("sidecar server fixture has a remote requester");
        let local_validator = adapter
            .context
            .roster
            .iter()
            .position(|entry| entry.validator == adapter.local_peer)
            .and_then(|index| wire::ValidatorIndex::try_from(index).ok())
            .expect("sidecar server local validator belongs to its context roster");
        let request = crate::merge_sidecar::CertifiedMergeSidecarRequestV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            request_id: Hash::new(b"writer flush sidecar server request"),
            entry_hash,
            encoded_len: reference.encoded_len,
            epoch_id: reference.epoch_id,
            reference_digest: certified_merge_reference_digest(&reference),
            requester: requester.clone(),
            responder: adapter.local_peer.clone(),
        };
        CertifiedSidecarServerFixture {
            kura: Arc::clone(&adapter.kura),
            context: adapter.context.clone(),
            local_validator,
            adapter,
            validators,
            requester,
            request,
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
    fn direct_decision_quiesces_losing_lane_and_retransmission_work() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (losing_block, losing_proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        let (losing_round, _) = global_lock_for_block(&adapter, &losing_block);
        adapter
            .lane_sessions
            .insert_proposal(losing_proposal.clone())
            .expect("retain losing lane proposal");
        adapter
            .planned_lane_proposals
            .insert(losing_round, vec![losing_proposal.clone()]);
        adapter
            .pending_local_lane_proposals
            .insert(losing_block.hash(), vec![losing_proposal.clone()]);
        let effect = V2LaneWorkEffect::PostLaneBlock {
            peer: adapter.context.roster[0].validator.clone(),
            message: BlockMessage::LaneBlockProposal(losing_proposal),
        };
        adapter.effect_keys.insert(lane_work_effect_key(&effect));
        adapter.effects.push_back(effect);
        let decided = wire::BlockSubject {
            parent_block_hash: losing_block.header().prev_block_hash(),
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"different decided block")),
            payload_hash: Hash::new(b"different decided payload"),
        };

        adapter
            .retain_merge_sidecars_for_global_view(0, None, Some(decided))
            .expect("install terminal Decision carrier state");

        assert_eq!(adapter.lane_sessions.len(), 0);
        assert!(adapter.planned_lane_proposals.is_empty());
        assert!(adapter.pending_local_lane_proposals.is_empty());
        assert!(adapter.locally_bound_lane_proposals.is_empty());
        assert!(adapter.native_requests.is_empty());
        assert!(adapter.effects.is_empty());
        adapter
            .schedule_retransmission()
            .expect("Decision permits only exact sidecar recovery retransmission");
        assert!(adapter.drain_effects(usize::MAX).is_empty());
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
    fn retired_sidecar_route_between_drain_and_lane_queue_preserves_live_sibling() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let requester = adapter.context.roster[1].validator.clone();
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut routes = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let route_a = routes.mint_via(requester.clone(), hub_a);
        let route_b = routes.mint_via(requester.clone(), hub_b);
        let chunk = CertifiedMergeSidecarChunkV1 {
            version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
            request_id: Hash::new(b"lane route retirement request"),
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"lane route retirement entry")),
            encoded_len: 1,
            epoch_id: 1,
            reference_digest: Hash::new(b"lane route retirement reference"),
            requester: requester.clone(),
            responder: adapter.local_peer.clone(),
            chunk_index: 0,
            chunk_count: 1,
            bytes: vec![0xA5],
        };
        let post = |reply_route| MergeSidecarPost {
            peer: requester.clone(),
            reply_route: Some(reply_route),
            message: CertifiedMergeSidecarMessage::Chunk(chunk.clone()),
        };

        assert!(routes.retire(&route_a));
        adapter
            .push_merge_sidecar_post_or_restart(post(route_a))
            .expect("retired source occurrence is conservatively released");
        adapter
            .push_merge_sidecar_post_or_restart(post(route_b.clone()))
            .expect("live sibling crosses the reserved lane queue");
        assert!(matches!(
            adapter.drain_effects(usize::MAX).as_slice(),
            [V2LaneWorkEffect::PostCertifiedMergeSidecar {
                reply_routes: Some(reply_routes),
                ..
            }] if reply_routes.len() == 1
                && reply_routes.iter().any(|route| route.same_delivery(&route_b))
        ));
        assert!(!adapter.output_guard.restart_required());
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
            false,
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
        let context = adapter.context.clone();
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
        let pre_apply = V2LaneWorkAdapter::new_with_output_guard(
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

        {
            let mut nexus = state.nexus.write();
            nexus.enabled = !nexus.enabled;
        }
        assert_ne!(
            context.nexus_amx_context_hash,
            super::super::v2_recovery::committed_nexus_amx_context_hash(state.as_ref()),
            "fixture must exercise the post-application context-hash exception without changing the frozen context id"
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
            let persisted = drifted.kura.store_block(block.clone()).is_ok();
            assert!(
                !persisted
                    || !canonical_v2_lane_payload_matches_kura(
                        drifted.state.as_ref(),
                        drifted.kura.as_ref(),
                        &drifted.context,
                        &block,
                    ),
                "Kura admission or canonical recovery must reject lifecycle and QC-tag drift"
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
    fn prior_height_hydration_stays_local_under_successor_backpressure() {
        const PRIOR_HEIGHT: u64 = 4;
        const SUCCESSOR_HEIGHT: u64 = 5;

        let (adapter, keys) =
            fixture_at_height(wire::ConsensusMode::Permissioned, SUCCESSOR_HEIGHT);
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, PRIOR_HEIGHT)
            .expect("canonical lane incarnation is active at the prior height");
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            lane_id,
            dataspace_id,
            incarnation,
            PRIOR_HEIGHT,
            1,
        );
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

        let mut recovered = V2LaneWorkAdapter::new(
            context.clone(),
            local_peer,
            local_key,
            true,
            state,
            Arc::clone(&kura),
            limits,
            None,
        )
        .expect("open successor-height adapter");
        assert!(
            recovered.lane_sessions.get(&session_key).is_some(),
            "the prior-height proposal remains available for historical certificate recovery"
        );
        let mut effects = recovered.drain_effects(usize::MAX);
        recovered
            .schedule_retransmission()
            .expect("schedule bounded successor-height retransmission");
        effects.extend(recovered.drain_effects(usize::MAX));
        assert!(
            effects.iter().all(|effect| !matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock { message, .. }
                    if lane_output_identity(message)
                        .is_some_and(|(proposal_height, _)| proposal_height == PRIOR_HEIGHT)
            )),
            "a hydrated prior-height artifact must never become successor-height lane fanout"
        );

        let target = canonical
            .descriptor
            .validator_set
            .iter()
            .find(|peer| *peer != &recovered.local_peer)
            .expect("fixture has a remote lane validator")
            .clone();
        let stale_effect = V2LaneWorkEffect::PostLaneBlock {
            peer: target.clone(),
            message: BlockMessage::LaneBlockProposal(canonical.clone()),
        };
        let mut service = service_for_history_context(kura, context, &keys);
        let attempts = Arc::new(AtomicUsize::new(0));
        let attempts_for_hook = Arc::clone(&attempts);
        service.set_exact_output_admission_hook(move |post, ticket| {
            attempts_for_hook.fetch_add(1, Ordering::Relaxed);
            Err(
                iroha_p2p::network::NetworkActorAdmissionError::Backpressured {
                    message: post,
                    ticket,
                    rank: 1,
                },
            )
        });

        let reservation_error = service
            .can_retain_lane_work_effect(&stale_effect)
            .expect_err("the h5 service must reject an h4 generic lane claim");
        assert!(reservation_error.contains("differs from immutable height context 5"));
        let post_error = service
            .post_lane_block(target, BlockMessage::LaneBlockProposal(canonical))
            .expect_err("the stale proposal must fail before actor admission");
        assert!(post_error.contains("differs from immutable height context 5"));
        assert_eq!(
            attempts.load(Ordering::Relaxed),
            0,
            "the stale output must not reach the backpressured target"
        );
        assert!(
            !service
                .has_pending_exact_output()
                .expect("inspect exact-output corridor"),
            "the h4 proposal must not be retained under an h5 rollover claim"
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
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
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
        let proposal_block = block.canonical_resultless_proposal();
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &proposal_block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_eq!(
            adapter.bind_locked_global_body(&proposal_block),
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
    fn decided_lane_ownership_blocks_rollover_until_its_session_is_durable() {
        // Result-bearing genesis carries external entrypoints before any lane
        // ownership can exist. Its empty ownership set is complete, not a
        // malformed lane plan or a missing lane certificate.
        {
            let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
            let transaction_key = KeyPair::try_from_seed(vec![0xE2; 32], Algorithm::Ed25519)
                .expect("external-only transaction key");
            let transaction = TransactionBuilder::new(
                adapter.context.chain_id.clone(),
                AccountId::new(transaction_key.public_key().clone()),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .sign(transaction_key.private_key());
            let entrypoint_hash = transaction.hash_as_entrypoint();
            let mut block =
                SignedBlock::genesis(vec![transaction], transaction_key.private_key(), None, None);
            block
                .set_transaction_results(
                    Vec::new(),
                    &[entrypoint_hash],
                    vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
                )
                .expect("attach deterministic genesis transaction result");
            assert!(block.header().is_genesis());
            assert_eq!(block.external_entrypoint_count(), 1);
            assert!(block.has_results());
            assert!(block.header().result_merkle_root().is_some());
            assert!(block.execution_context().is_none());
            adapter
                .kura
                .store_block(block.clone())
                .expect("persist canonical external-only carrier");
            let finality_artifact = finality_artifact_for_block(&adapter, &keys, &block);
            assert!(
                adapter
                    .durable_lane_rollover_authority(&finality_artifact)
                    .expect("validate external-only rollover")
                    .is_some(),
                "a canonical block with no lane ownership has no lane durability debt"
            );
        }

        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist canonical decided lane carrier");
        let proposal_block = block.canonical_resultless_proposal();
        let (locked_round, decided) = global_lock_for_block(&adapter, &proposal_block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, decided),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_eq!(
            adapter.bind_locked_global_body(&proposal_block),
            V2LaneIngressOutcome::Inserted
        );
        let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
        commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
        adapter
            .retain_merge_sidecars_for_global_view(locked_round.view, Some(decided), Some(decided))
            .expect("install exact global Decision");
        let finality_artifact = finality_artifact_for_block(&adapter, &keys, &block);

        assert!(
            adapter
                .durable_lane_rollover_authority(&finality_artifact)
                .expect("inspect incomplete decided lane boundary")
                .is_none(),
            "a raw ownership must not disappear from an empty rollover authority"
        );

        assert_eq!(proposal.descriptor.validator_count, 4);
        assert_eq!(proposal.descriptor.min_quorum, 3);
        let remote_keys = keys
            .iter()
            .filter(|key| PeerId::new(key.public_key().clone()) != adapter.local_peer)
            .take(2)
            .collect::<Vec<_>>();
        assert_eq!(
            remote_keys.len(),
            2,
            "two survivors plus the local vote must form the 3-of-4 quorum"
        );
        for phase in [CertPhase::Prepare, CertPhase::Commit] {
            for key in &remote_keys {
                let vote = signed_lane_vote(&proposal, phase, key);
                assert_ne!(
                    adapter.accept_lane_message(
                        InboundBlockMessage::new(
                            BlockMessage::LaneBlockVote(vote),
                            Some(PeerId::new(key.public_key().clone())),
                        ),
                        locked_round.view,
                    ),
                    V2LaneIngressOutcome::Rejected,
                    "the exact decided carrier must keep accepting quorum progress"
                );
            }
            let _ = adapter.drain_effects(usize::MAX);
        }
        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("persist decided lane certificate and application receipt"),
            1
        );
        let durable = adapter
            .kura
            .read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .expect("read exact decided lane certificate");
        assert!(
            adapter
                .kura
                .lane_block_application_receipt_available(&proposal)
        );
        let authority = adapter
            .durable_lane_rollover_authority(&finality_artifact)
            .expect("inspect completed decided lane boundary")
            .expect("the exact durable lane session must release successor activation");
        assert!(
            authority
                .covered_source_hash(
                    &finality_artifact,
                    &BlockMessage::LaneBlockQc(durable.commit_qc),
                )
                .expect("validate the durable decided CommitQC")
                .is_some(),
            "rollover must cover the decided carrier's exact durable CommitQC"
        );
    }

    #[test]
    fn applied_lane_certificate_retires_alternative_qc_replays_without_weakening_conflicts() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist canonical lane anchor");
        let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
        commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);

        let persisted = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
        };
        let persisted_pops = adapter.pops_for_lane_session(&persisted);
        adapter
            .kura
            .persist_committed_lane_block_session(&persisted, &persisted_pops)
            .expect("persist first valid quorum proof");
        assert!(
            adapter
                .kura
                .persist_lane_block_application_receipt_if_ready(&proposal)
                .expect("persist application receipt for the certified proposal")
        );
        adapter
            .kura
            .persist_committed_lane_block_session(&persisted, &persisted_pops)
            .expect("an exact durable duplicate remains idempotent");

        let alternative_qc = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Commit),
        };
        assert_ne!(
            persisted.prepare_qc.signers_bitmap, alternative_qc.prepare_qc.signers_bitmap,
            "fixture must model two valid 3-of-4 certificates for one proposal"
        );
        let alternative_pops = adapter.pops_for_lane_session(&alternative_qc);
        let certificate_error = adapter
            .kura
            .persist_committed_lane_block_session(&alternative_qc, &alternative_pops)
            .expect_err("Kura must not replace the retained certificate bytes");
        assert!(
            certificate_error
                .to_string()
                .contains("different active-incarnation payload")
        );

        let descriptor = &proposal.descriptor;
        let conflicting_proposal = proposal_for_route(
            &adapter,
            &keys,
            descriptor.lane_id,
            descriptor.dataspace_id,
            descriptor.lane_incarnation,
            descriptor.proposal_height,
            descriptor.lane_block_height,
        );
        assert_ne!(
            conflicting_proposal, proposal,
            "fixture must model a different valid body at the occupied lane height"
        );
        let conflicting_body = CommittedLaneBlockSession {
            prepare_qc: lane_qc_for_phase(&conflicting_proposal, &keys[..3], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&conflicting_proposal, &keys[..3], CertPhase::Commit),
            proposal: conflicting_proposal,
        };
        let conflicting_pops = adapter.pops_for_lane_session(&conflicting_body);
        let body_error = adapter
            .kura
            .persist_committed_lane_block_session(&conflicting_body, &conflicting_pops)
            .expect_err("Kura must reject a different certified body at the same active height");
        assert!(
            body_error
                .to_string()
                .contains("different active-incarnation payload")
        );
        adapter
            .pending_committed_lanes
            .push_back(conflicting_body.clone());
        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("retain a non-canonical conflicting body"),
            0,
            "the exact-proposal shortcut must not retire a different body"
        );
        assert_eq!(
            adapter
                .pending_committed_lanes
                .front()
                .map(|session| &session.proposal),
            Some(&conflicting_body.proposal),
            "a different body must remain pending until it has a canonical anchor"
        );
        adapter.pending_committed_lanes.clear();

        assert!(
            adapter.proposal_body_available(&proposal),
            "an applied peer must keep serving the canonical body to lagging validators"
        );
        assert!(
            adapter
                .canonical_proposal_for_vote_body(&proposal.vote_body(CertPhase::Prepare))
                .is_some(),
            "an applied peer must keep reconstructing canonical recovery evidence"
        );
        adapter
            .pending_committed_lanes
            .push_back(alternative_qc.clone());
        adapter
            .committed_lane_outputs
            .push_back(PendingCommittedLaneOutput {
                session: alternative_qc.clone(),
                next_validator: 0,
            });
        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("retire a replay after matching its durable applied proposal"),
            1,
            "a valid alternative proof must not replace the exact durable certificate"
        );
        assert!(adapter.pending_committed_lanes.is_empty());
        let durable = adapter
            .kura
            .read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .expect("retained certified artifact");
        assert_eq!(durable.proposal, persisted.proposal);
        assert_eq!(durable.prepare_qc, persisted.prepare_qc);
        assert_eq!(durable.commit_qc, persisted.commit_qc);

        let finality_artifact = finality_artifact_for_block(&adapter, &keys, &block);
        let authority = adapter
            .durable_lane_rollover_authority(&finality_artifact)
            .expect("inspect exact durable lane rollover authority")
            .expect("build exact durable lane rollover authority");
        let winning_vote = signed_lane_vote(&proposal, CertPhase::Prepare, &keys[3]);
        let winning_certificate = LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc: alternative_qc.prepare_qc.clone(),
            commit_qc: alternative_qc.commit_qc.clone(),
        };
        for message in [
            BlockMessage::LaneBlockProposal(proposal.clone()),
            BlockMessage::LaneBlockVote(winning_vote),
            BlockMessage::LaneBlockQc(alternative_qc.prepare_qc.clone()),
            BlockMessage::LaneBlockQc(alternative_qc.commit_qc.clone()),
            BlockMessage::LaneBlockCertificate(Box::new(winning_certificate)),
        ] {
            assert!(
                authority
                    .covered_source_hash(&finality_artifact, &message)
                    .expect("validate winning lane rollover output")
                    .is_some(),
                "every exact winning lane artifact must share the durable session witness"
            );
        }
        assert!(
            authority
                .covered_source_hash(
                    &finality_artifact,
                    &BlockMessage::LaneBlockProposal(conflicting_body.proposal.clone()),
                )
                .expect("classify same-height losing lane output")
                .is_some(),
            "the finality artifact must explicitly supersede a non-winning proposal"
        );
        let mut invalid_winning_qc = alternative_qc.commit_qc;
        invalid_winning_qc.bls_aggregate_signature[0] ^= 0x80;
        assert!(
            authority
                .covered_source_hash(
                    &finality_artifact,
                    &BlockMessage::LaneBlockQc(invalid_winning_qc),
                )
                .is_err(),
            "a winning proposal hash must not hide invalid proof bytes"
        );
    }

    #[test]
    fn same_proposal_shortcut_rejects_unvalidated_certificate_variants() {
        {
            let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
            let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
            adapter
                .kura
                .store_block(block.clone())
                .expect("persist canonical lane anchor");
            let committed = ValidBlock::committed_from_replay_signed_block(block);
            commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
            let pending = CommittedLaneBlockSession {
                proposal: proposal.clone(),
                prepare_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare),
                commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
            };
            let pops = adapter.pops_for_lane_session(&pending);
            adapter
                .kura
                .fail_progress_sidecar_ancestor_sync_attempts_for_tests(0, 1);
            adapter
                .kura
                .persist_committed_lane_block_session(&pending, &pops)
                .expect_err("failed ancestor barrier must leave only readable certificate bytes");
            adapter.pending_committed_lanes.push_back(pending.clone());
            adapter
                .kura
                .fail_progress_sidecar_ancestor_sync_attempts_for_tests(0, 2);
            let error = adapter
                .persist_anchored_sessions()
                .expect_err("shortcut and exact retry must both honor the failed ancestor barrier");
            assert!(
                error.to_string().contains("durable"),
                "unexpected durability rejection: {error}"
            );
            assert_eq!(
                adapter.pending_committed_lanes.front(),
                Some(&pending),
                "durability failure must retain the pending reconstruction source"
            );
        }

        #[derive(Clone, Copy, Debug)]
        enum QcUnderTest {
            Prepare,
            Commit,
        }

        impl QcUnderTest {
            fn phase(self) -> CertPhase {
                match self {
                    Self::Prepare => CertPhase::Prepare,
                    Self::Commit => CertPhase::Commit,
                }
            }

            fn select_mut(self, session: &mut CommittedLaneBlockSession) -> &mut LaneBlockQcV1 {
                match self {
                    Self::Prepare => &mut session.prepare_qc,
                    Self::Commit => &mut session.commit_qc,
                }
            }
        }

        #[derive(Clone, Copy, Debug)]
        enum InvalidQcVariant {
            ForgedAggregate,
            WrongPhase,
            WrongRound,
            WrongBody,
            WrongBitmap,
            OutOfRangeBitmap,
            InsufficientCount,
            MissingPop,
            InvalidPop,
        }

        for (qc_under_test, variant) in [QcUnderTest::Prepare, QcUnderTest::Commit]
            .into_iter()
            .flat_map(|qc_under_test| {
                [
                    InvalidQcVariant::ForgedAggregate,
                    InvalidQcVariant::WrongPhase,
                    InvalidQcVariant::WrongRound,
                    InvalidQcVariant::WrongBody,
                    InvalidQcVariant::WrongBitmap,
                    InvalidQcVariant::OutOfRangeBitmap,
                    InvalidQcVariant::InsufficientCount,
                    InvalidQcVariant::MissingPop,
                    InvalidQcVariant::InvalidPop,
                ]
                .into_iter()
                .map(move |variant| (qc_under_test, variant))
            })
        {
            let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
            let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
            adapter
                .kura
                .store_block(block.clone())
                .expect("persist canonical lane anchor");
            let committed = ValidBlock::committed_from_replay_signed_block(block);
            commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);

            let retained = CommittedLaneBlockSession {
                proposal: proposal.clone(),
                prepare_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare),
                commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
            };
            adapter
                .kura
                .persist_committed_lane_block_session(&retained, &lane_signer_pops(&keys[..3]))
                .expect("persist retained valid certificate");
            assert!(
                adapter
                    .kura
                    .persist_lane_block_application_receipt_if_ready(&proposal)
                    .expect("persist retained certificate receipt")
            );

            let target_qc = lane_qc_for_phase(&proposal, &keys[1..], qc_under_test.phase());
            let opposite_qc = lane_qc_for_phase(
                &proposal,
                &keys[..3],
                match qc_under_test {
                    QcUnderTest::Prepare => CertPhase::Commit,
                    QcUnderTest::Commit => CertPhase::Prepare,
                },
            );
            let mut pending = match qc_under_test {
                QcUnderTest::Prepare => CommittedLaneBlockSession {
                    proposal: proposal.clone(),
                    prepare_qc: target_qc,
                    commit_qc: opposite_qc,
                },
                QcUnderTest::Commit => CommittedLaneBlockSession {
                    proposal: proposal.clone(),
                    prepare_qc: opposite_qc,
                    commit_qc: target_qc,
                },
            };
            let valid_candidate = CertifiedLaneBlockArtifact::new(
                pending.clone(),
                adapter.pops_for_lane_session(&pending),
            );
            Kura::validate_certified_lane_block_artifact(&valid_candidate)
                .expect("the unmodified alternative proof must be valid");

            let qc = qc_under_test.select_mut(&mut pending);
            match variant {
                InvalidQcVariant::ForgedAggregate => {
                    qc.bls_aggregate_signature[0] ^= 0x80;
                }
                InvalidQcVariant::WrongPhase => {
                    qc.body.phase = match qc_under_test {
                        QcUnderTest::Prepare => CertPhase::Commit,
                        QcUnderTest::Commit => CertPhase::Prepare,
                    };
                }
                InvalidQcVariant::WrongRound => {
                    qc.body.lane_block_view = qc.body.lane_block_view.saturating_add(1);
                }
                InvalidQcVariant::WrongBody => {
                    qc.body.subject_hash = Hash::new(b"forged alternative certificate body");
                }
                InvalidQcVariant::WrongBitmap => {
                    assert_eq!(
                        qc.signers_bitmap,
                        vec![0b0000_1110],
                        "fixture target QC must select validators 1, 2, and 3"
                    );
                    qc.signers_bitmap[0] = 0b0000_1101;
                }
                InvalidQcVariant::OutOfRangeBitmap => {
                    qc.signers_bitmap[0] |= 0b1000_0000;
                }
                InvalidQcVariant::InsufficientCount => {
                    assert_eq!(
                        qc.signers_bitmap,
                        vec![0b0000_1110],
                        "fixture target QC must select validators 1, 2, and 3"
                    );
                    qc.signers_bitmap[0] = 0b0000_0110;
                }
                InvalidQcVariant::MissingPop | InvalidQcVariant::InvalidPop => {
                    let state = Arc::get_mut(&mut adapter.state)
                        .expect("isolated lane adapter uniquely owns its State");
                    let id =
                        ConsensusKeyId::new(ConsensusKeyRole::Validator, "validator3".to_owned());
                    let mut record = state
                        .world
                        .consensus_keys
                        .view()
                        .get(&id)
                        .expect("signer unique to the target alternative QC")
                        .clone();
                    record.pop = match variant {
                        InvalidQcVariant::MissingPop => None,
                        InvalidQcVariant::InvalidPop => Some(vec![0xA5; 96]),
                        _ => unreachable!("matched PoP variants"),
                    };
                    state.world.consensus_keys.insert(id, record);
                }
            }

            adapter.pending_committed_lanes.push_back(pending.clone());
            let error = adapter
                .persist_anchored_sessions()
                .expect_err("unvalidated proof variant must not use the same-proposal shortcut");
            assert!(
                error
                    .to_string()
                    .contains("pending committed lane certificate is invalid"),
                "unexpected {qc_under_test:?} {variant:?} rejection: {error}"
            );
            assert_eq!(
                adapter.pending_committed_lanes.front(),
                Some(&pending),
                "rejected {qc_under_test:?} {variant:?} proof must retain its volatile owner for fail-stop diagnosis"
            );
            let durable = adapter
                .kura
                .read_certified_lane_block_artifact(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                )
                .expect("retained certificate remains authoritative");
            assert_eq!(durable.prepare_qc, retained.prepare_qc);
            assert_eq!(durable.commit_qc, retained.commit_qc);
        }
    }

    #[test]
    fn alternative_qc_repairs_missing_receipt_from_retained_exact_certificate() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist canonical lane anchor");
        let committed = ValidBlock::committed_from_replay_signed_block(block);
        commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);

        let retained = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
        };
        let retained_pops = adapter.pops_for_lane_session(&retained);
        adapter
            .kura
            .persist_committed_lane_block_session(&retained, &retained_pops)
            .expect("persist certificate before the simulated crash");
        assert!(
            !adapter
                .kura
                .lane_block_application_receipt_available(&proposal),
            "fixture must stop between certificate and receipt durability"
        );

        let replayed = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Commit),
        };
        assert_ne!(
            retained.prepare_qc.signers_bitmap, replayed.prepare_qc.signers_bitmap,
            "the replay must carry a distinct valid quorum proof"
        );
        adapter.pending_committed_lanes.push_back(replayed);
        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("repair the receipt from the retained exact certificate"),
            1
        );
        assert!(
            adapter
                .kura
                .lane_block_application_receipt_available(&proposal),
            "the retained certificate must finish its interrupted receipt"
        );
        let durable = adapter
            .kura
            .read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .expect("retained exact certificate");
        assert_eq!(durable.proposal, retained.proposal);
        assert_eq!(durable.prepare_qc, retained.prepare_qc);
        assert_eq!(durable.commit_qc, retained.commit_qc);
    }

    #[test]
    fn globally_applied_lane_body_without_certificate_remains_recoverable() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        assert!(
            adapter
                .lane_sessions
                .proposals_without_commit_qc()
                .iter()
                .all(|pending| pending != &proposal),
            "the adapter must be constructed before the canonical ownership arrives"
        );
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist canonical lane anchor without its certificate");
        let (decided_round, decided_subject) = global_lock_for_block(&adapter, &block);
        let finality_artifact = finality_artifact_for_block(&adapter, &keys, &block);
        let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
        commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
        adapter
            .retain_merge_sidecars_for_global_view(
                decided_round.view,
                Some(decided_subject),
                Some(decided_subject),
            )
            .expect("install the direct block-sync Decision without binding its lane body");
        assert!(adapter.decision_pending());

        let recovered = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Commit),
        };
        assert!(adapter.proposal_anchor_is_committed_in_state(&proposal));
        assert!(
            adapter
                .kura
                .read_certified_lane_block_artifact(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                )
                .is_none(),
            "the globally applied body must begin without lane certificate durability"
        );
        assert!(
            !adapter
                .state
                .certified_lane_block_session_is_applied_or_snapshot_anchored_cached(&recovered),
            "global application alone must not impersonate lane certificate application"
        );
        assert!(
            adapter.proposal_body_available(&proposal),
            "the missing certificate must remain reconstructable from the canonical body"
        );

        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("rehydrate the late-applied canonical ownership"),
            0,
            "no certificate exists yet to persist"
        );
        assert!(
            adapter
                .lane_sessions
                .proposals_without_commit_qc()
                .iter()
                .any(|pending| pending == &proposal),
            "rollover must rehydrate ownership which arrived after adapter construction"
        );
        assert!(
            adapter
                .durable_lane_rollover_authority(&finality_artifact)
                .expect("inspect incomplete decided-lane rollover")
                .is_none(),
            "the decided height must remain open until its lane certificate is durable"
        );
        let _ = adapter.drain_effects(usize::MAX);
        adapter
            .schedule_retransmission()
            .expect("schedule exact missing-certificate discovery");
        assert!(
            adapter.drain_effects(usize::MAX).iter().any(|effect| {
                matches!(
                    effect,
                    V2LaneWorkEffect::PostLaneBlock {
                        message: BlockMessage::LaneBlockProposal(pending),
                        ..
                    } if pending == &proposal
                )
            }),
            "the rehydrated proposal must become a bounded certificate request source"
        );
        let certificate = LaneBlockCertificateV1 {
            proposal: recovered.proposal.clone(),
            prepare_qc: recovered.prepare_qc.clone(),
            commit_qc: recovered.commit_qc.clone(),
        };
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                    Some(PeerId::new(keys[1].public_key().clone())),
                ),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(
            adapter
                .persist_anchored_sessions()
                .expect("persist recovered certificate and application receipt"),
            1
        );
        let durable = adapter
            .kura
            .read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .expect("recovered durable certificate");
        assert_eq!(durable.proposal, recovered.proposal);
        assert_eq!(durable.prepare_qc, recovered.prepare_qc);
        assert_eq!(durable.commit_qc, recovered.commit_qc);
        assert!(
            adapter
                .kura
                .lane_block_application_receipt_available(&proposal),
            "certificate recovery must finish the lane application boundary"
        );
        assert!(
            adapter
                .durable_lane_rollover_authority(&finality_artifact)
                .expect("build recovered decided-lane rollover authority")
                .is_some(),
            "the exact recovered certificate and receipt must release successor activation"
        );
    }

    #[test]
    fn persisted_lane_session_uses_only_selected_qc_signer_pops() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (_, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        let selected_keys = &keys[..3];
        let prepare_qc = lane_qc_for_phase(&proposal, selected_keys, CertPhase::Prepare);
        let commit_qc = lane_qc_for_phase(&proposal, selected_keys, CertPhase::Commit);
        let session = CommittedLaneBlockSession {
            proposal,
            prepare_qc,
            commit_qc,
        };
        let signer_pops = adapter.pops_for_lane_session(&session);
        let expected_signers = selected_keys
            .iter()
            .map(|key| key.public_key().clone())
            .collect::<BTreeSet<_>>();

        assert_eq!(
            signer_pops.keys().cloned().collect::<BTreeSet<_>>(),
            expected_signers,
            "durable proof material must name exactly the bitmap-selected signers"
        );
        let artifact =
            crate::kura::CertifiedLaneBlockArtifact::new(session.clone(), signer_pops.clone());
        assert_eq!(
            Kura::validate_certified_lane_block_artifact(&artifact),
            Ok(())
        );
        adapter
            .kura
            .persist_committed_lane_block_session(&session, &signer_pops)
            .expect("persist a 3-of-4 lane certificate with exact signer PoPs");

        let extra_signer_artifact =
            crate::kura::CertifiedLaneBlockArtifact::new(session.clone(), lane_signer_pops(&keys));
        assert_eq!(
            Kura::validate_certified_lane_block_artifact(&extra_signer_artifact),
            Err("certified lane block signer PoPs do not match QC signers"),
            "a non-signer PoP must remain rejected instead of being persisted as unauthenticated metadata"
        );

        let mut missing_selected_pop = signer_pops;
        missing_selected_pop.remove(selected_keys[0].public_key());
        assert!(matches!(
            crate::lane_consensus::validate_lane_block_qc_aggregate(
                &session.prepare_qc,
                &missing_selected_pop,
            ),
            Err(crate::lane_consensus::LaneBlockQcIngressError::SignerPopMissing)
        ));

        let mut out_of_range_bitmap = session.prepare_qc;
        out_of_range_bitmap.signers_bitmap[0] |= 1_u8 << 4;
        assert!(matches!(
            crate::lane_consensus::validate_lane_block_qc_aggregate(
                &out_of_range_bitmap,
                &lane_signer_pops(&keys),
            ),
            Err(crate::lane_consensus::LaneBlockQcIngressError::SignerBitmapOutOfRange)
        ));
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
        let finality_artifact = finality_artifact_for_block(&adapter, &keys, &block);
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
        assert!(
            reopened.committed_lane_outputs.is_empty(),
            "volatile completed-output ownership must not survive restart"
        );
        assert!(
            reopened
                .durable_lane_rollover_authority(&finality_artifact)
                .expect("reconstruct rollover authority after restart")
                .is_some(),
            "canonical Kura evidence must reconstruct authority without a volatile output queue"
        );
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

    fn global_lock_for_block(
        adapter: &V2LaneWorkAdapter,
        block: &SignedBlock,
    ) -> (wire::ConsensusRound, wire::BlockSubject) {
        let canonical_payload_hash = block
            .canonical_proposal_wire_hash()
            .expect("encode locked block fixture");
        (
            wire::ConsensusRound {
                context_id: adapter.context.id(),
                height: adapter.context.height,
                view: block.header().view_change_index(),
            },
            wire::BlockSubject {
                parent_block_hash: block.header().prev_block_hash(),
                block_hash: block.hash(),
                payload_hash: canonical_payload_hash,
            },
        )
    }

    fn finality_artifact_for_block(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
        block: &SignedBlock,
    ) -> wire::finality::V2FinalityArtifact {
        let (round, subject) = global_lock_for_block(adapter, block);
        let artifact = wire::finality::V2FinalityArtifact::new(
            adapter.context.clone(),
            subject,
            wire::QuorumCertificate {
                round,
                proposal_round: round,
                phase: wire::GlobalPhase::Commit,
                subject,
                execution_commitment: wire::ExecutionCommitment::without_topups(
                    Hash::new(b"lane rollover parent state"),
                    Hash::new(b"lane rollover post state"),
                    Hash::new(b"lane rollover writes"),
                    Hash::new(b"lane rollover executed block"),
                ),
                signers: vec![0, 1, 2],
                aggregate_signature: vec![0xA5; 48],
            },
            keys.iter()
                .map(|key| {
                    iroha_crypto::bls_normal_pop_prove(key.private_key())
                        .expect("global validator proof of possession")
                })
                .collect(),
        );
        artifact
            .validate()
            .expect("valid lane rollover finality artifact");
        artifact
    }

    fn successor_context_for_parent(
        adapter: &V2LaneWorkAdapter,
        parent: &SignedBlock,
    ) -> wire::HeightContext {
        let parent_context_id = adapter.context.id();
        let parent_wire = parent.encode_wire().expect("encode parent block");
        let mut context = adapter.context.clone();
        context.height = context.height.checked_add(1).expect("successor height");
        context.parent_commit_qc = Some(wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: parent_context_id,
                height: parent.header().height().get(),
                view: parent.header().view_change_index(),
            },
            proposal_round: wire::ConsensusRound {
                context_id: parent_context_id,
                height: parent.header().height().get(),
                view: parent.header().view_change_index(),
            },
            phase: wire::GlobalPhase::Commit,
            subject: wire::BlockSubject {
                parent_block_hash: parent.header().prev_block_hash(),
                block_hash: parent.hash(),
                payload_hash: Hash::new(&parent_wire),
            },
            execution_commitment: wire::ExecutionCommitment::without_topups(
                Hash::new(b"lane-certificate parent state"),
                Hash::new(b"lane-certificate parent post-state"),
                Hash::new(b"lane-certificate parent writes"),
                Hash::new(&parent_wire),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xA5; 48],
        });
        context.snapshot_bootstrap = None;
        context.nexus_amx_context_hash =
            super::super::v2_recovery::committed_nexus_amx_context_hash(adapter.state.as_ref());
        context
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
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
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
        planned_lane_candidate_block_for_route_at_view(
            adapter,
            keys,
            view,
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
        )
    }

    fn planned_lane_candidate_block_for_route_at_view(
        adapter: &V2LaneWorkAdapter,
        keys: &[KeyPair],
        view: u64,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
    ) -> (SignedBlock, LaneBlockProposalV1) {
        let transaction_key = KeyPair::try_from_seed(
            vec![u8::try_from(view).unwrap_or(u8::MAX).wrapping_add(0x40); 32],
            Algorithm::Ed25519,
        )
        .expect("deterministic candidate transaction key");
        let transaction = TransactionBuilder::new(
            adapter.context.chain_id.clone(),
            AccountId::new(transaction_key.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
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
        try_store_canonical_anchor(adapter, proposal, signer)
            .expect("store canonical lane anchor block")
    }

    fn try_store_canonical_anchor(
        adapter: &V2LaneWorkAdapter,
        proposal: &LaneBlockProposalV1,
        signer: &KeyPair,
    ) -> Option<LaneBlockProposalV1> {
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
            adapter.kura.store_block(block).ok()?;
        }
        let ownership = ownership_from_proposal(proposal);
        ownership
            .validate_replay_material()
            .expect("canonical fixture ownership replay material validates");
        let block = test_block(target_height, parent, Some(ownership.clone()), signer);
        let block_hash = block.hash();
        adapter.kura.store_block(block).ok()?;
        proposal_from_ownership(&ownership, block_hash)
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
    fn native_amx_request_rejects_inactive_reply_route_before_signing() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let request = native_request(&adapter, &keys);
        let leader = usize::try_from(adapter.context.leader(request.body.round.view))
            .ok()
            .and_then(|index| adapter.context.roster.get(index))
            .expect("fixture view has a leader")
            .validator
            .clone();
        let relay = adapter
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .find(|peer| peer != &leader)
            .expect("fixture has a distinct authenticated relay");
        let mut routes = NetworkReplyRouteTestFixture::new(relay);
        let route = routes.mint(leader.clone());
        assert!(routes.retire(&route));

        assert_eq!(
            adapter.accept_native_amx(
                leader,
                Some(route),
                NativeAmxMessage::PrepareRequest(request),
                0,
            ),
            V2LaneIngressOutcome::Rejected
        );
        assert!(adapter.local_native_claims.is_empty());
        assert!(adapter.drain_effects(usize::MAX).is_empty());
        assert_eq!(
            adapter
                .native_signing_guard
                .as_ref()
                .expect("validator has durable Native AMX guard")
                .record_count_for_test(),
            0
        );
    }

    #[test]
    fn native_coordinator_height_ignores_retired_incarnation_artifacts() {
        let (adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let lane_id = LaneId::SINGLE;
        let dataspace_id = DataSpaceId::UNIVERSAL;
        let retired_incarnation = adapter
            .state
            .lane_incarnation_at_height(lane_id, adapter.context.height)
            .expect("fixture lane incarnation");
        let historical = proposal_for_route(
            &adapter,
            &keys,
            lane_id,
            dataspace_id,
            retired_incarnation,
            adapter.context.height,
            100,
        );
        let _ = store_canonical_anchor(&adapter, &historical, &keys[0]);
        assert!(
            adapter
                .kura
                .latest_lane_block_artifact(lane_id)
                .is_some_and(|artifact| artifact.ownership.lane_block_height == 100),
            "fixture must first install a reachable high lane-local artifact"
        );

        let recreated_catalog = LaneCatalog::new(
            NonZeroU32::new(1).expect("non-zero lane count"),
            vec![LaneConfig {
                alias: "recreated-default".to_owned(),
                ..LaneConfig::default()
            }],
        )
        .expect("recreated default-lane catalog");
        {
            let mut nexus = adapter.state.nexus.write();
            nexus.lane_config =
                iroha_config::parameters::actual::LaneConfig::from_catalog(&recreated_catalog);
            nexus.lane_catalog = recreated_catalog;
        }
        adapter.state.reseed_static_lane_incarnations_for_tests();
        assert_ne!(
            adapter
                .state
                .lane_incarnation_at_height(lane_id, adapter.context.height),
            Some(retired_incarnation),
            "lane recreation must retire the historical namespace"
        );
        assert!(
            adapter.kura.latest_lane_block_artifact(lane_id).is_none(),
            "the active Kura marker must hide the retired high artifact"
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
            reply_routes: None,
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
    fn duplicate_reply_effect_preserves_exact_source_delivery() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let body = native_body(&adapter);
        let vote = adapter
            .sign_native_vote_once(body)
            .expect("fixture validator signs one exact Native AMX vote");
        let message = NativeAmxMessage::PrepareVote(vote);
        let peer = adapter.context.roster[1].validator.clone();
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
        let route = route_fixture.mint(peer.clone());
        let effect = V2LaneWorkEffect::PostNativeAmx {
            peer: peer.clone(),
            reply_routes: Some(
                NetworkReplyRoutes::try_from_route(route.clone()).expect("live reply route"),
            ),
            message,
        };
        assert!(adapter.push_effect(effect.clone()));
        assert!(adapter.push_effect(effect));
        assert_eq!(adapter.effects.len(), 1);
        let Some(V2LaneWorkEffect::PostNativeAmx {
            reply_routes: Some(retained),
            ..
        }) = adapter.effects.front()
        else {
            panic!("exact duplicate retains one reply-route set");
        };
        assert_eq!(retained.len(), 1);
        assert!(
            retained
                .iter()
                .any(|retained| retained.same_delivery(&route))
        );
    }

    #[test]
    fn reply_effect_rejects_missing_or_retargeted_route_set() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let body = native_body(&adapter);
        let vote = adapter
            .sign_native_vote_once(body)
            .expect("fixture validator signs one exact Native AMX vote");
        let message = NativeAmxMessage::PrepareVote(vote);
        let peer = adapter.context.roster[1].validator.clone();
        assert!(!adapter.push_effect(V2LaneWorkEffect::PostNativeAmx {
            peer: peer.clone(),
            reply_routes: None,
            message: message.clone(),
        }));

        let different_target = adapter.context.roster[2].validator.clone();
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
        let retargeted = route_fixture.mint(different_target);
        assert!(!adapter.push_effect(V2LaneWorkEffect::PostNativeAmx {
            peer,
            reply_routes: Some(
                NetworkReplyRoutes::try_from_route(retargeted).expect("live reply route"),
            ),
            message,
        }));
        assert!(adapter.effects.is_empty());
    }

    #[test]
    fn duplicate_reply_effect_updates_only_later_delivery_from_same_source() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let body = native_body(&adapter);
        let vote = adapter
            .sign_native_vote_once(body)
            .expect("fixture validator signs one exact Native AMX vote");
        let message = NativeAmxMessage::PrepareVote(vote);
        let peer = adapter.context.roster[1].validator.clone();
        let hub = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
        let first = route_fixture.mint(peer.clone());
        let later = route_fixture
            .redeliver(&first)
            .expect("fixture owns the first route");
        let effect_for = |route| V2LaneWorkEffect::PostNativeAmx {
            peer: peer.clone(),
            reply_routes: Some(
                NetworkReplyRoutes::try_from_route(route).expect("live reply route"),
            ),
            message: message.clone(),
        };

        assert!(adapter.push_effect(effect_for(first.clone())));
        assert!(adapter.push_effect(effect_for(later.clone())));
        assert!(
            !adapter.push_effect(effect_for(first.clone())),
            "a stale delivery must fail without regressing retained ownership"
        );
        let Some(V2LaneWorkEffect::PostNativeAmx {
            reply_routes: Some(retained),
            ..
        }) = adapter.effects.front()
        else {
            panic!("same-source update retains one reply-route set");
        };
        assert_eq!(retained.len(), 1);
        assert!(retained.iter().any(|route| route.same_delivery(&later)));
        assert!(!retained.iter().any(|route| route.same_delivery(&first)));
    }

    #[test]
    fn duplicate_reply_effect_retains_alternate_sources_across_source_update() {
        let (mut adapter, _) = fixture(wire::ConsensusMode::Permissioned);
        let body = native_body(&adapter);
        let vote = adapter
            .sign_native_vote_once(body)
            .expect("fixture validator signs one exact Native AMX vote");
        let message = NativeAmxMessage::PrepareVote(vote);
        let peer = adapter.context.roster[1].validator.clone();
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture = NetworkReplyRouteTestFixture::new(hub_a.clone());
        let first_a = route_fixture.mint_via(peer.clone(), hub_a.clone());
        let route_b = route_fixture.mint_via(peer.clone(), hub_b);
        let effect_for = |route| V2LaneWorkEffect::PostNativeAmx {
            peer: peer.clone(),
            reply_routes: Some(
                NetworkReplyRoutes::try_from_route(route).expect("live reply route"),
            ),
            message: message.clone(),
        };

        assert!(adapter.push_effect(effect_for(first_a.clone())));
        assert!(route_fixture.retire(&first_a));
        assert!(adapter.push_effect(effect_for(route_b.clone())));
        let reconnected_a = route_fixture.mint_via(peer.clone(), hub_a);
        assert!(adapter.push_effect(effect_for(reconnected_a.clone())));
        let later_a = route_fixture
            .redeliver(&reconnected_a)
            .expect("fixture owns the reconnected source route");
        assert!(adapter.push_effect(effect_for(later_a.clone())));
        assert!(
            !adapter.push_effect(effect_for(reconnected_a.clone())),
            "a stale source must not reset its own attempt or erase an alternate source"
        );
        let Some(V2LaneWorkEffect::PostNativeAmx {
            reply_routes: Some(retained),
            ..
        }) = adapter.effects.front()
        else {
            panic!("alternate source merge retains one reply-route set");
        };
        assert_eq!(retained.len(), 2);
        assert!(retained.iter().any(|route| route.same_delivery(&later_a)));
        assert!(retained.iter().any(|route| route.same_delivery(&route_b)));
        assert!(
            !retained
                .iter()
                .any(|route| route.same_delivery(&reconnected_a))
        );

        let hub_c = PeerId::new(KeyPair::random().public_key().clone());
        let route_c = route_fixture.mint_via(peer.clone(), hub_c);
        let mut mixed = NetworkReplyRoutes::try_from_route(reconnected_a.clone())
            .expect("stale A is independently live");
        mixed
            .merge(
                &NetworkReplyRoutes::try_from_route(route_b.clone())
                    .expect("B is live while constructing the occurrence"),
            )
            .expect("candidate occurrence can carry B");
        mixed
            .merge(
                &NetworkReplyRoutes::try_from_route(route_c.clone()).expect("new source C is live"),
            )
            .expect("candidate occurrence can carry C");
        assert!(route_fixture.retire(&route_b));
        assert!(
            adapter.push_effect(V2LaneWorkEffect::PostNativeAmx {
                peer: peer.clone(),
                reply_routes: Some(mixed),
                message: message.clone(),
            }),
            "stale and inactive attempts must not suppress an independent live source"
        );
        let Some(V2LaneWorkEffect::PostNativeAmx {
            reply_routes: Some(retained),
            ..
        }) = adapter.effects.front()
        else {
            panic!("a mixed-liveness merge must retain its accepted live sources");
        };
        assert_eq!(retained.len(), 2);
        assert!(retained.iter().any(|route| route.same_delivery(&later_a)));
        assert!(!retained.iter().any(|route| route.same_delivery(&route_b)));
        assert!(retained.iter().any(|route| route.same_delivery(&route_c)));
        assert!(
            !retained
                .iter()
                .any(|route| route.same_delivery(&reconnected_a))
        );

        let retired_only = Some(
            NetworkReplyRoutes::try_from_route(route_c.clone())
                .expect("candidate captures source C before retirement"),
        );
        assert!(route_fixture.retire(&route_c));
        assert!(
            !adapter.push_effect(V2LaneWorkEffect::PostNativeAmx {
                peer,
                reply_routes: retired_only,
                message,
            }),
            "the adapter commits maintenance but reports no retained candidate delivery"
        );
        let Some(V2LaneWorkEffect::PostNativeAmx {
            reply_routes: queued,
            ..
        }) = adapter.effects.front()
        else {
            panic!("queued duplicate retains its route history");
        };
        let retained = queued
            .as_ref()
            .expect("maintenance keeps the live sibling route set");
        assert_eq!(retained.len(), 1);
        assert!(retained.iter().any(|route| route.same_delivery(&later_a)));
        assert!(!retained.iter().any(|route| route.same_delivery(&route_c)));
    }

    #[test]
    fn temporarily_unserviceable_effect_requeues_behind_later_reserved_work() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let message = NativeAmxMessage::PrepareRequest(native_request(&adapter, &keys));
        let first = V2LaneWorkEffect::PostNativeAmx {
            peer: adapter.context.roster[1].validator.clone(),
            reply_routes: None,
            message: message.clone(),
        };
        let second = V2LaneWorkEffect::PostNativeAmx {
            peer: adapter.context.roster[2].validator.clone(),
            reply_routes: None,
            message,
        };
        let first_key = lane_work_effect_key(&first);
        let second_key = lane_work_effect_key(&second);
        assert!(adapter.push_effect(first.clone()));
        assert!(adapter.push_effect(second.clone()));
        assert_eq!(
            adapter.next_effect().as_ref().map(lane_work_effect_key),
            Some(first_key)
        );

        let blocked = adapter
            .drain_effects(1)
            .pop()
            .expect("peeked effect remains drainable");
        assert_eq!(lane_work_effect_key(&blocked), first_key);
        assert!(adapter.requeue_effect(blocked));
        assert_eq!(
            adapter.next_effect().as_ref().map(lane_work_effect_key),
            Some(second_key)
        );
        let second = adapter
            .drain_effects(1)
            .pop()
            .expect("later reserved effect remains queued");
        assert_eq!(lane_work_effect_key(&second), second_key);
        let first = adapter
            .drain_effects(1)
            .pop()
            .expect("requeued effect remains owned");
        assert_eq!(lane_work_effect_key(&first), first_key);
        assert_eq!(adapter.effect_count(), 0);
    }

    #[test]
    fn retransmission_classes_rotate_fairly_at_capacity_one() {
        let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
        let (_, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        assert_eq!(
            adapter.lane_sessions.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        adapter.locally_bound_lane_proposals.insert(
            proposal.proposal_hash,
            proposal
                .payload_block_hint
                .expect("planned proposal carries its global block hint"),
        );

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
            reply_routes: None,
            message: CertifiedMergeSidecarMessage::Request(request.clone()),
        };

        assert!(adapter.push_effect(effect.clone()));
        assert!(adapter.push_effect(effect.clone()));
        assert_eq!(adapter.effects.len(), 1, "an exact retry is deduplicated");

        assert!(
            adapter.push_effect(V2LaneWorkEffect::PostCertifiedMergeSidecar {
                peer: alternate_destination,
                reply_routes: None,
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
                reply_routes: None,
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
    fn planner_view_one_binds_rotated_global_leader_to_fresh_lane_view() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let global_view = 1;
        let leader_index =
            usize::try_from(adapter.context.leader(global_view)).expect("leader index fits usize");
        adapter.local_peer = adapter.context.roster[leader_index].validator.clone();
        adapter.key_pair = keys[leader_index].clone();

        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, global_view);
        let ownership = &block
            .execution_context()
            .expect("planned block carries its execution context")
            .lane_payload_ownerships[0];
        assert_eq!(ownership.proposal_view, global_view);
        assert_eq!(ownership.lane_block_view, 0);
        assert_eq!(proposal.descriptor.lane_block_view, 0);
        assert_eq!(
            proposal
                .payload_block_hint
                .expect("planned proposal carries its global block hint")
                .proposal_view,
            global_view
        );

        let round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: global_view,
        };
        adapter
            .planned_lane_proposals
            .insert(round, vec![proposal.clone()]);
        assert_eq!(
            adapter.bind_local_candidate(round, block.hash()),
            V2LaneIngressOutcome::Inserted,
            "the rotated global leader must bind a fresh lane-local proposal"
        );
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected,
            "the exact locked body keeps its global view independently of the lane-local view"
        );

        let expected_hint = proposal
            .payload_block_hint
            .expect("planned proposal carries its global block hint");
        assert_eq!(
            adapter
                .locally_bound_lane_proposals
                .get(&proposal.proposal_hash),
            Some(&expected_hint)
        );
        let mut tampered = proposal;
        tampered
            .payload_block_hint
            .as_mut()
            .expect("planned proposal carries its global block hint")
            .proposal_view = 0;
        let forged_sender_index =
            usize::try_from(adapter.context.leader(0)).expect("leader index fits usize");
        let forged_sender = adapter.context.roster[forged_sender_index]
            .validator
            .clone();
        assert_eq!(
            adapter.insert_lane_proposal(tampered, Some(&forged_sender), false, global_view),
            V2LaneIngressOutcome::Rejected,
            "an advisory hint must exactly match the authenticated locked-body binding"
        );
    }

    #[test]
    fn enabled_nexus_binds_independent_lane_author_distinct_from_global_leader() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let lane_id = LaneId::new(1);
        let dataspace_id = DataSpaceId::new(7);
        let lane_validators = enable_multilane_nexus(&mut adapter, &keys, lane_id, dataspace_id);
        let lane_author = lane_validators
            .first()
            .expect("enabled lane committee is non-empty")
            .clone();
        let global_view = (0..u64::try_from(adapter.context.roster.len())
            .expect("roster length fits u64"))
            .find(|view| {
                let leader_index = usize::try_from(adapter.context.leader(*view))
                    .expect("leader index fits usize");
                adapter.context.roster[leader_index].validator != lane_author
            })
            .expect("rotating global roster contains a leader distinct from the lane author");
        let global_leader_index =
            usize::try_from(adapter.context.leader(global_view)).expect("leader index fits usize");
        let global_leader = adapter.context.roster[global_leader_index]
            .validator
            .clone();
        adapter.local_peer = global_leader.clone();
        adapter.key_pair = keys[global_leader_index].clone();

        let (block, proposal) = planned_lane_candidate_block_for_route_at_view(
            &adapter,
            &keys,
            global_view,
            lane_id,
            dataspace_id,
        );
        assert_eq!(lane_proposal_author(&proposal), Some(&lane_author));
        assert_ne!(lane_author, global_leader);
        assert_eq!(proposal.descriptor.lane_block_view, 0);
        assert_eq!(
            proposal
                .payload_block_hint
                .expect("planned proposal carries its global block hint")
                .proposal_view,
            global_view
        );

        let round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: adapter.context.height,
            view: global_view,
        };
        adapter
            .planned_lane_proposals
            .insert(round, vec![proposal.clone()]);
        assert_eq!(
            adapter.bind_local_candidate(round, block.hash()),
            V2LaneIngressOutcome::Inserted,
            "the global leader may bind work authored by the independent lane rotation"
        );
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected,
            "the exact global lock must not require the independent lane author to be its leader"
        );

        adapter
            .kura
            .store_block(block.clone())
            .expect("persist exact enabled-Nexus recovery body");
        assert!(canonical_v2_lane_payload_matches_kura(
            adapter.state.as_ref(),
            adapter.kura.as_ref(),
            &adapter.context,
            &block,
        ));
    }

    #[test]
    fn canonical_kura_recovery_accepts_global_view_one_with_fresh_lane_view() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let global_view = 1;
        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, global_view);
        assert_eq!(block.header().view_change_index(), global_view);
        assert_eq!(proposal.descriptor.lane_block_view, 0);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist planner-produced canonical recovery body");

        assert!(canonical_v2_lane_payload_matches_kura(
            adapter.state.as_ref(),
            adapter.kura.as_ref(),
            &adapter.context,
            &block,
        ));
        assert!(
            adapter.canonical_anchor_for_proposal(&proposal).is_some(),
            "the exact ownership/header global view must authenticate the lane-local proposal"
        );
    }

    #[test]
    fn canonical_kura_recovery_rejects_nonzero_planner_origin_lane_view() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (planned, _) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let mut ownership = planned
            .execution_context()
            .expect("planned block carries its execution context")
            .lane_payload_ownerships[0]
            .clone();
        ownership.lane_block_view = 1;
        let replay = ownership
            .compute_replay_hashes()
            .expect("nonzero lane-view ownership replay material recomputes");
        ownership.subject_hash = replay.subject_hash;
        ownership.payload_ownership_hash = replay.payload_ownership_hash;
        ownership.rbc_instance_hash = replay.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
        ownership
            .validate_replay_material()
            .expect("nonzero lane-view fixture must not rely on stale replay hashes");

        let leader_index = usize::try_from(adapter.context.leader(0)).expect("leader index");
        let block = test_block(1, None, Some(ownership), &keys[leader_index]);
        adapter
            .kura
            .store_block(block.clone())
            .expect("persist adversarial nonzero lane-view body");
        assert!(
            !canonical_v2_lane_payload_matches_kura(
                adapter.state.as_ref(),
                adapter.kura.as_ref(),
                &adapter.context,
                &block,
            ),
            "canonical recovery must enforce the planner-origin lane-view invariant"
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
        assert_eq!(proposal_at_later_view.descriptor.lane_block_view, 0);
        assert_eq!(
            proposal_at_later_view
                .payload_block_hint
                .expect("replanned proposal carries its global block hint")
                .proposal_view,
            later_view,
            "a full global-leader rotation must not advance the fresh lane-local view"
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
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &later_block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
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
    fn global_body_lock_replacement_requires_higher_prepare_round_and_exact_subject() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, losing_proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        let (_, subject_a) = global_lock_for_block(&adapter, &block);
        let block_hash = subject_a.block_hash;
        let subject_a = wire::BlockSubject {
            payload_hash: Hash::new(b"global lock payload A"),
            ..subject_a
        };
        let subject_b = wire::BlockSubject {
            payload_hash: Hash::new(b"global lock payload B"),
            ..subject_a
        };
        let context_id = adapter.context.id();
        let height = adapter.context.height;
        let round = |view| wire::ConsensusRound {
            context_id,
            height,
            view,
        };
        assert_eq!(
            adapter
                .lane_sessions
                .insert_proposal(losing_proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        assert_eq!(
            adapter.mark_global_body_locked(round(0), subject_a),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_eq!(
            adapter.mark_global_body_locked(round(0), subject_a),
            Ok(GlobalBodyLockOutcome::Duplicate)
        );
        assert!(matches!(
            adapter.mark_global_body_locked(round(0), subject_b),
            Err(V2LaneWorkError::ConflictingGlobalBodyLock)
        ));
        assert_eq!(
            adapter.globally_locked_body,
            Some(GlobalBodyLock {
                round: round(0),
                subject: subject_a,
            })
        );

        adapter
            .pending_local_lane_proposals
            .insert(block_hash, Vec::new());
        adapter.locally_bound_lane_proposals.insert(
            Hash::new(b"losing local lane proposal"),
            LaneBlockProposalPayloadHintV1 {
                proposal_height: adapter.context.height,
                proposal_view: 0,
                proposal_block_hash: block_hash,
            },
        );
        assert_eq!(
            adapter.mark_global_body_locked(round(1), subject_b),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_eq!(
            adapter.globally_locked_body,
            Some(GlobalBodyLock {
                round: round(1),
                subject: subject_b,
            }),
            "same block hash with different payload is a distinct higher lock"
        );
        assert!(adapter.pending_local_lane_proposals.is_empty());
        assert!(adapter.locally_bound_lane_proposals.is_empty());
        assert!(
            !adapter.lane_sessions.contains_proposal(&losing_proposal),
            "uncommitted lane sessions for the superseded carrier must release capacity"
        );
        assert!(matches!(
            adapter.mark_global_body_locked(round(0), subject_a),
            Err(V2LaneWorkError::ConflictingGlobalBodyLock)
        ));
        assert_eq!(
            adapter.globally_locked_body.map(|lock| lock.subject),
            Some(subject_b),
            "a lower lock cannot restore the retired exact subject"
        );
    }

    #[test]
    fn superseded_commit_protected_lane_session_cannot_retransmit() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected
        );
        assert_eq!(
            adapter.lane_sessions.insert_qc_with_pops(
                lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
                &lane_signer_pops(&keys),
            ),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        let commit_vote = signed_lane_vote(&proposal, CertPhase::Commit, &keys[0]);
        assert_eq!(
            adapter.lane_sessions.insert_vote(commit_vote, None),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        let replacement_round = wire::ConsensusRound {
            view: locked_round.view + 1,
            ..locked_round
        };
        let replacement_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"replacement global carrier block hash",
            )),
            payload_hash: Hash::new(b"replacement global carrier payload"),
            ..locked_subject
        };
        assert_eq!(
            adapter.mark_global_body_locked(replacement_round, replacement_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert!(
            adapter.lane_sessions.contains_proposal(&proposal),
            "Commit evidence remains cached as safety state"
        );

        adapter
            .schedule_retransmission()
            .expect("schedule after replacing the exact global lock");
        let effects = adapter.drain_effects(usize::MAX);
        assert!(
            !effects.iter().any(|effect| matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockProposal(candidate),
                    ..
                } if candidate.proposal_hash == proposal.proposal_hash
            ) || matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockVote(vote),
                    ..
                } if vote.body.proposal_hash == proposal.proposal_hash
            ) || matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockQc(qc),
                    ..
                } if qc.body.proposal_hash == proposal.proposal_hash
            )),
            "safety-retained state for the losing carrier must not remain live traffic"
        );
    }

    #[test]
    fn decision_cleanup_fairly_reconstructs_completed_commit_qc_fanout() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected
        );
        let _ = adapter.drain_effects(usize::MAX);
        adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("non-zero capacity");

        assert_eq!(
            adapter.insert_lane_qc(
                lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
                locked_round.view,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(
            adapter.insert_lane_qc(
                lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
                locked_round.view,
            ),
            V2LaneIngressOutcome::Inserted
        );
        adapter.drive_lane_sessions();
        assert!(adapter.has_pending_committed_output_handoff());

        adapter
            .retain_merge_sidecars_for_global_view(
                locked_round.view,
                Some(locked_subject),
                Some(locked_subject),
            )
            .expect("install decided carrier state");

        let expected = proposal
            .descriptor
            .validator_set
            .iter()
            .filter(|peer| *peer != &adapter.local_peer)
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut observed = BTreeSet::new();
        for _ in 0..=expected.len() {
            adapter
                .schedule_retransmission()
                .expect("reconstruct the next final CommitQC destination");
            for effect in adapter.drain_effects(1) {
                match effect {
                    V2LaneWorkEffect::PostLaneBlock {
                        peer,
                        message: BlockMessage::LaneBlockQc(qc),
                    } => {
                        assert_eq!(qc.body.phase, CertPhase::Commit);
                        assert_eq!(qc.body.proposal_hash, proposal.proposal_hash);
                        assert!(
                            observed.insert(peer),
                            "destination must transfer exactly once"
                        );
                    }
                    other => panic!("decision cleanup retained non-final lane output: {other:?}"),
                }
            }
            if !adapter.has_pending_committed_output_handoff() {
                break;
            }
        }

        assert_eq!(observed, expected);
        assert!(!adapter.has_pending_committed_output_handoff());
    }

    #[test]
    fn durable_lane_certificate_is_one_atomic_kura_backed_response() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (_, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        let session = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        let pops = adapter.pops_for_lane_session(&session);
        adapter
            .kura
            .persist_committed_lane_block_session(&session, &pops)
            .expect("persist the authoritative recovery source");
        adapter.effects.clear();
        adapter.effect_keys.clear();
        adapter.lane_sessions = LaneBlockSessionCache::new(adapter.limits.session_capacity.get());

        let requester = session
            .commit_qc
            .validator_set
            .iter()
            .find(|peer| *peer != &adapter.local_peer)
            .cloned()
            .expect("fixture has a remote committee member");
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    Some(requester.clone()),
                ),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
            "a durable response must never fall back to topology without an exact request route"
        );
        assert!(adapter.drain_effects(usize::MAX).is_empty());
        let relay = PeerId::new(
            KeyPair::try_from_seed(vec![0xD6; 32], Algorithm::BlsNormal)
                .expect("relay key")
                .public_key()
                .clone(),
        );
        assert_ne!(relay, requester);
        let mut routes = NetworkReplyRouteTestFixture::new(relay.clone());
        let cancelled_route = routes.mint(requester.clone());
        assert!(routes.retire(&cancelled_route));
        assert!(matches!(
            InboundBlockMessage::try_from_transport_with_reply_route(
                BlockMessage::LaneBlockProposal(proposal.clone()),
                requester.clone(),
                relay.clone(),
                cancelled_route,
            ),
            Err(NetworkReplyRouteError::Inactive)
        ));
        assert!(adapter.drain_effects(usize::MAX).is_empty());
        let reply_route = routes.mint(requester.clone());
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    requester.clone(),
                    relay,
                    reply_route.clone(),
                )
                .expect("active durable request route"),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        let effects = adapter.drain_effects(usize::MAX);
        assert_eq!(
            effects.len(),
            1,
            "Prepare and Commit must cross one owner boundary"
        );
        assert!(matches!(
            &effects[0],
            V2LaneWorkEffect::PostDurableLaneCertificate {
                peer,
                reply_routes: Some(emitted_routes),
                certificate,
                ..
            } if peer == &requester
                && emitted_routes.len() == 1
                && emitted_routes
                    .iter()
                    .any(|emitted_route| emitted_route.same_delivery(&reply_route))
                && certificate.proposal == session.proposal
                && certificate.prepare_qc == session.prepare_qc
                && certificate.commit_qc == session.commit_qc
        ));
    }

    #[test]
    fn durable_lane_certificate_coalescing_preserves_alternate_ingress_owners() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (_, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        let session = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        let pops = adapter.pops_for_lane_session(&session);
        adapter
            .kura
            .persist_committed_lane_block_session(&session, &pops)
            .expect("persist the authoritative recovery source");
        adapter.effects.clear();
        adapter.effect_keys.clear();
        adapter.lane_sessions = LaneBlockSessionCache::new(adapter.limits.session_capacity.get());

        let requester = session
            .commit_qc
            .validator_set
            .iter()
            .find(|peer| *peer != &adapter.local_peer)
            .cloned()
            .expect("fixture has a remote committee member");
        let hub_a = PeerId::new(KeyPair::random().public_key().clone());
        let hub_b = PeerId::new(KeyPair::random().public_key().clone());
        let mut route_fixture =
            NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
        let route_a = route_fixture.mint_via(requester.clone(), hub_a.clone());
        let route_b = route_fixture.mint_via(requester.clone(), hub_b.clone());
        let admitted = |via: PeerId, route: NetworkReplyRoute| {
            fair_v2_ingress_admit_for_test(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    requester.clone(),
                    via,
                    route,
                )
                .expect("durable request route is exact"),
            )
        };

        assert_eq!(
            adapter.accept_lane_message_with_ingress_ownership(admitted(hub_a, route_a.clone()), 0),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(
            adapter.accept_lane_message_with_ingress_ownership(admitted(hub_b, route_b.clone()), 0),
            V2LaneIngressOutcome::Inserted
        );
        let effect = adapter
            .drain_effects(1)
            .pop()
            .expect("one coalesced durable response");
        let V2LaneWorkEffect::PostDurableLaneCertificate {
            reply_routes: Some(reply_routes),
            ingress_ownership: Some(ownership),
            certificate,
            ..
        } = effect
        else {
            panic!("durable response retains routes and fair ownership")
        };
        assert_eq!(
            certificate,
            LaneBlockCertificateV1 {
                proposal: session.proposal,
                prepare_qc: session.prepare_qc,
                commit_qc: session.commit_qc,
            }
        );
        assert_eq!(reply_routes.len(), 2);
        assert!(
            reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_a))
        );
        assert!(
            reply_routes
                .iter()
                .any(|route| route.same_delivery(&route_b))
        );
        assert!(ownership.validate_exact());
        assert_eq!(ownership.admission_count, 2);
        assert!(ownership.matches_reply_routes(Some(&reply_routes)));
    }

    #[test]
    fn durable_lane_certificate_serves_rotated_validator_after_pressure() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let requester = adapter
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .find(|peer| peer != &adapter.local_peer)
            .expect("fixture has a remote current validator");
        let historical_keys = keys
            .iter()
            .filter(|key| key.public_key() != requester.public_key())
            .cloned()
            .collect::<Vec<_>>();
        let lane_incarnation = adapter
            .state
            .lane_incarnation_at_height(LaneId::SINGLE, adapter.context.height)
            .expect("default lane incarnation");
        let proposal = proposal_for_route(
            &adapter,
            &historical_keys,
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            lane_incarnation,
            adapter.context.height,
            1,
        );
        let proposal = store_canonical_anchor(&adapter, &proposal, &historical_keys[0]);
        let session = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &historical_keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &historical_keys, CertPhase::Commit),
        };
        assert!(!session.commit_qc.validator_set.contains(&requester));
        let pops = adapter.pops_for_lane_session(&session);
        adapter
            .kura
            .persist_committed_lane_block_session(&session, &pops)
            .expect("persist historical-committee certificate");
        adapter.effects.clear();
        adapter.effect_keys.clear();
        adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        assert!(adapter.push_effect(V2LaneWorkEffect::PostLaneBlock {
            peer: requester.clone(),
            message: BlockMessage::LaneBlockProposal(proposal.clone()),
        }));
        let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
        let requester_route = routes.mint(requester.clone());

        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    requester.clone(),
                    requester.clone(),
                    requester_route.clone(),
                )
                .expect("active historical request route"),
                0,
            ),
            V2LaneIngressOutcome::Duplicate,
            "a full response slot must leave reconstruction at the requester's exact proposal"
        );
        let _ = adapter.drain_effects(1);

        let unauthorized = PeerId::new(
            KeyPair::try_from_seed(vec![0xE7; 32], Algorithm::BlsNormal)
                .expect("outsider key")
                .public_key()
                .clone(),
        );
        let mut unauthorized_routes = NetworkReplyRouteTestFixture::new(unauthorized.clone());
        let unauthorized_route = unauthorized_routes.mint(unauthorized.clone());
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    unauthorized.clone(),
                    unauthorized.clone(),
                    unauthorized_route,
                )
                .expect("active unauthorized route reaches consensus validation"),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
            "an authenticated transport identity outside both canonical rosters is unauthorized"
        );
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::try_from_transport_with_reply_route(
                    BlockMessage::LaneBlockProposal(proposal.clone()),
                    requester.clone(),
                    requester.clone(),
                    requester_route.clone(),
                )
                .expect("active retried request route"),
                0,
            ),
            V2LaneIngressOutcome::Inserted,
            "the durable request must reconstruct the atomic response after capacity opens"
        );
        assert!(matches!(
            adapter.drain_effects(1).as_slice(),
            [V2LaneWorkEffect::PostDurableLaneCertificate {
                peer,
                reply_routes: Some(emitted_routes),
                certificate,
                ..
            }] if peer == &requester
                && emitted_routes.len() == 1
                && emitted_routes
                    .iter()
                    .any(|emitted_route| emitted_route.same_delivery(&requester_route))
                && certificate.proposal == session.proposal
        ));
    }

    #[test]
    fn decided_lane_accepts_atomic_certificate_recovery_and_rejects_mismatch() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected
        );
        let _ = adapter.drain_effects(usize::MAX);
        adapter
            .retain_merge_sidecars_for_global_view(
                locked_round.view,
                Some(locked_subject),
                Some(locked_subject),
            )
            .expect("install the exact decided carrier");

        let certificate = LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        let mut mismatched = certificate.clone();
        mismatched.prepare_qc.body.phase = CertPhase::Commit;
        let before = adapter.lane_sessions.len();
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(mismatched)),
                    Some(keys[0].public_key().clone().into()),
                ),
                locked_round.view,
            ),
            V2LaneIngressOutcome::Rejected
        );
        assert_eq!(adapter.lane_sessions.len(), before);

        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                    Some(keys[0].public_key().clone().into()),
                ),
                locked_round.view,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(adapter.pending_committed_lanes.len(), 1);
        assert!(adapter.has_pending_committed_output_handoff());
    }

    #[test]
    fn historical_certificate_survives_successor_lock_decision_persistence_and_restart() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let (parent_block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
        adapter
            .kura
            .store_block(parent_block.clone())
            .expect("persist the globally committed lane carrier");
        let committed_parent = ValidBlock::committed_from_replay_signed_block(parent_block.clone());
        commit_test_block_to_state(adapter.state.as_ref(), &committed_parent, &adapter.context);
        let certificate = LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        let successor_context = successor_context_for_parent(&adapter, &parent_block);
        let local_peer = adapter.local_peer.clone();
        let local_key = adapter.key_pair.clone();
        let state = Arc::clone(&adapter.state);
        let kura = Arc::clone(&adapter.kura);
        let limits = adapter.limits;
        drop(adapter);

        let mut successor = V2LaneWorkAdapter::new(
            successor_context.clone(),
            local_peer.clone(),
            local_key.clone(),
            true,
            Arc::clone(&state),
            Arc::clone(&kura),
            limits,
            None,
        )
        .expect("open the true successor-height adapter");
        assert!(
            successor
                .lane_sessions
                .proposals_without_commit_qc()
                .iter()
                .any(|pending| pending == &proposal),
            "the successor must hydrate the exact older proposal as its request source"
        );
        let _ = successor.drain_effects(usize::MAX);
        assert_eq!(
            successor.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                    Some(PeerId::new(keys[0].public_key().clone())),
                ),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(successor.historical_recovery_sessions.len(), 1);
        assert!(
            successor.pending_committed_lanes.is_empty(),
            "historical evidence must not enter current-carrier persistence ownership"
        );
        assert!(
            successor.committed_lane_outputs.is_empty(),
            "historical recovery must not create a fresh CommitQC fanout"
        );

        let successor_block = test_block(
            successor.context.height,
            Some(parent_block.hash()),
            None,
            &keys[0],
        );
        let (locked_round, locked_subject) = global_lock_for_block(&successor, &successor_block);
        assert_eq!(
            successor.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        successor
            .retain_merge_sidecars_for_global_view(
                locked_round.view,
                Some(locked_subject),
                Some(locked_subject),
            )
            .expect("install a distinct successor Decision");
        assert_eq!(
            successor.historical_recovery_sessions.len(),
            1,
            "successor lock and Decision filtering must preserve the historical owner"
        );

        assert!(
            successor
                .service_next_historical_recovery()
                .expect("persist historical certificate and application witness")
        );
        assert!(!successor.has_pending_historical_recovery());
        assert_eq!(
            kura.read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .map(|artifact| artifact.proposal),
            Some(proposal.clone())
        );
        assert!(kura.lane_block_application_receipt_available(&proposal));
        assert!(
            state
                .unapplied_lane_block_artifact_heights_snapshot_cached()
                .is_empty(),
            "the recovered application witness must unblock the lane frontier"
        );
        drop(successor);

        let reopened = V2LaneWorkAdapter::new(
            successor_context,
            local_peer,
            local_key,
            true,
            Arc::clone(&state),
            Arc::clone(&kura),
            limits,
            None,
        )
        .expect("restart after historical recovery remains self-sufficient");
        assert!(!reopened.has_pending_historical_recovery());
        assert!(
            state
                .unapplied_lane_block_artifact_heights_snapshot_cached()
                .is_empty()
        );
    }

    #[test]
    fn historical_certificate_keeps_exact_owner_while_state_anchor_is_not_ready() {
        let (mut adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 2);
        let lane_incarnation = adapter
            .state
            .lane_incarnation_at_height(LaneId::SINGLE, 1)
            .expect("historical default-lane incarnation");
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            lane_incarnation,
            1,
            1,
        );
        let proposal = store_canonical_anchor(&adapter, &proposal, &keys[0]);
        let certificate = LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        assert_ne!(
            adapter.state.committed_block_hash_at_height(1),
            proposal
                .payload_block_hint
                .map(|hint| hint.proposal_block_hash),
            "fixture must model Kura body arrival before the matching State anchor"
        );
        adapter.limits.session_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        adapter
            .committed_lane_outputs
            .push_back(PendingCommittedLaneOutput {
                session: CommittedLaneBlockSession {
                    proposal: certificate.proposal.clone(),
                    prepare_qc: certificate.prepare_qc.clone(),
                    commit_qc: certificate.commit_qc.clone(),
                },
                next_validator: 0,
            });
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(certificate.clone())),
                    Some(PeerId::new(keys[0].public_key().clone())),
                ),
                0,
            ),
            V2LaneIngressOutcome::Rejected,
            "historical recovery must be charged against the shared session bound"
        );
        assert!(adapter.historical_recovery_sessions.is_empty());
        adapter.committed_lane_outputs.clear();
        adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        assert!(adapter.push_effect(V2LaneWorkEffect::PostLaneBlock {
            peer: PeerId::new(keys[1].public_key().clone()),
            message: BlockMessage::LaneBlockProposal(proposal.clone()),
        }));
        let mut invalid = certificate.clone();
        invalid.commit_qc.bls_aggregate_signature[0] ^= 0x01;
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(invalid)),
                    Some(PeerId::new(keys[0].public_key().clone())),
                ),
                0,
            ),
            V2LaneIngressOutcome::Rejected
        );
        assert!(adapter.historical_recovery_sessions.is_empty());
        assert_eq!(
            adapter.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                    Some(PeerId::new(keys[0].public_key().clone())),
                ),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(
            adapter.effects.len(),
            1,
            "historical persistence ownership must not require effect capacity or create fanout"
        );
        assert!(
            !adapter
                .service_next_historical_recovery()
                .expect("an unavailable State dependency is retryable")
        );
        assert_eq!(adapter.historical_recovery_sessions.len(), 1);
        assert!(
            adapter
                .kura
                .read_certified_lane_block_artifact(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                )
                .is_none(),
            "certificate persistence must wait for the exact committed State anchor"
        );
    }

    #[test]
    fn historical_certificate_payload_corruption_is_fail_stop_and_retains_owner() {
        let (adapter, keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let lane_incarnation = adapter
            .state
            .lane_incarnation_at_height(LaneId::SINGLE, 1)
            .expect("default lane incarnation");
        let proposal = proposal_for_route(
            &adapter,
            &keys,
            LaneId::SINGLE,
            DataSpaceId::UNIVERSAL,
            lane_incarnation,
            1,
            1,
        );
        let proposal = store_canonical_anchor(&adapter, &proposal, &keys[0]);
        let parent_block = adapter
            .kura
            .get_block(NonZeroUsize::new(1).expect("non-zero parent height"))
            .expect("canonical corrupt-payload carrier")
            .as_ref()
            .clone();
        let committed_parent = ValidBlock::committed_from_replay_signed_block(parent_block.clone());
        commit_test_block_to_state(adapter.state.as_ref(), &committed_parent, &adapter.context);
        let certificate = LaneBlockCertificateV1 {
            proposal: proposal.clone(),
            prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
        };
        let successor_context = successor_context_for_parent(&adapter, &parent_block);
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
        )
        .expect("open successor before the historical certificate arrives");
        assert_eq!(
            successor.accept_lane_message(
                InboundBlockMessage::new(
                    BlockMessage::LaneBlockCertificate(Box::new(certificate)),
                    Some(PeerId::new(keys[0].public_key().clone())),
                ),
                0,
            ),
            V2LaneIngressOutcome::Inserted
        );

        let error = successor
            .service_next_historical_recovery()
            .expect_err("an immutable missing entrypoint must fail closed");
        assert!(
            error.to_string().contains("MissingEntrypoint"),
            "unexpected historical corruption error: {error}"
        );
        assert_eq!(
            successor.historical_recovery_sessions.len(),
            1,
            "fail-stop diagnosis must retain the exact recovery owner"
        );
        assert!(successor.output_guard.restart_required());
    }

    #[test]
    fn committed_output_source_remains_hard_bounded_after_persistence() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected
        );
        let _ = adapter.drain_effects(usize::MAX);

        let prepare_qc = lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare);
        let commit_qc = lane_qc_for_phase(&proposal, &keys, CertPhase::Commit);
        let completed = CommittedLaneBlockSession {
            proposal: proposal.clone(),
            prepare_qc: prepare_qc.clone(),
            commit_qc: commit_qc.clone(),
        };
        adapter.limits.session_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        adapter
            .committed_lane_outputs
            .push_back(PendingCommittedLaneOutput {
                next_validator: commit_qc.validator_set.len(),
                session: completed,
            });

        assert_eq!(
            adapter.insert_lane_qc(prepare_qc, locked_round.view),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(
            adapter.insert_lane_qc(commit_qc, locked_round.view),
            V2LaneIngressOutcome::Inserted
        );
        adapter.drive_lane_sessions();
        assert_eq!(adapter.committed_lane_outputs.len(), 1);
        assert!(
            adapter.pending_committed_lanes.is_empty(),
            "persisting the first source must not free its bounded reconstruction slot"
        );

        adapter.committed_lane_outputs.clear();
        adapter.collect_committed_lane_sessions();
        assert_eq!(adapter.committed_lane_outputs.len(), 1);
        assert_eq!(adapter.pending_committed_lanes.len(), 1);
    }

    #[test]
    fn carrier_replacement_filters_persistence_and_output_sources_together() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (losing_block, losing_proposal) =
            planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let (winning_block, winning_proposal) =
            planned_lane_candidate_block_at_view(&adapter, &keys, 1);
        let (_, winning_subject) = global_lock_for_block(&adapter, &winning_block);
        assert_ne!(
            global_lock_for_block(&adapter, &losing_block).1,
            winning_subject,
            "carrier replacement fixture must use distinct global subjects"
        );

        let sessions =
            [losing_proposal, winning_proposal].map(|proposal| CommittedLaneBlockSession {
                prepare_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
                commit_qc: lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
                proposal,
            });
        adapter.limits.session_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        for session in sessions {
            adapter.pending_committed_lanes.push_back(session.clone());
            adapter
                .committed_lane_outputs
                .push_back(PendingCommittedLaneOutput {
                    next_validator: session.commit_qc.validator_set.len(),
                    session,
                });
        }

        adapter.retain_committed_lane_outputs_for_subject(winning_subject);
        assert_eq!(adapter.pending_committed_lanes.len(), 1);
        assert_eq!(adapter.committed_lane_outputs.len(), 1);
        assert!(adapter.pending_committed_lanes.iter().all(|session| {
            session
                .proposal
                .payload_block_hint
                .as_ref()
                .is_some_and(|hint| hint.proposal_block_hash == winning_subject.block_hash)
        }));
        assert!(adapter.committed_lane_outputs.iter().all(|output| {
            output
                .session
                .proposal
                .payload_block_hint
                .as_ref()
                .is_some_and(|hint| hint.proposal_block_hash == winning_subject.block_hash)
        }));
    }

    #[test]
    fn completed_commit_qc_round_robin_does_not_restart_ahead_of_pending_source() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (_, first_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let (_, second_proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 1);
        let first_session = CommittedLaneBlockSession {
            prepare_qc: lane_qc_for_phase(&first_proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&first_proposal, &keys, CertPhase::Commit),
            proposal: first_proposal.clone(),
        };
        let second_session = CommittedLaneBlockSession {
            prepare_qc: lane_qc_for_phase(&second_proposal, &keys, CertPhase::Prepare),
            commit_qc: lane_qc_for_phase(&second_proposal, &keys, CertPhase::Commit),
            proposal: second_proposal.clone(),
        };
        adapter.effects.clear();
        adapter.effect_keys.clear();
        adapter.limits.effect_capacity = NonZeroUsize::new(1).expect("non-zero capacity");
        adapter
            .committed_lane_outputs
            .push_back(PendingCommittedLaneOutput {
                next_validator: first_session.commit_qc.validator_set.len(),
                session: first_session,
            });
        adapter
            .committed_lane_outputs
            .push_back(PendingCommittedLaneOutput {
                next_validator: 0,
                session: second_session,
            });

        adapter.schedule_lane_artifact_retransmissions();
        let effect = adapter
            .drain_effects(1)
            .pop()
            .expect("pending source must receive the only effect slot");
        assert!(matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockQc(qc),
                ..
            } if qc.body.phase == CertPhase::Commit
                && qc.body.proposal_hash == second_proposal.proposal_hash
        ));
    }

    #[test]
    fn completed_commit_qc_retransmits_after_volatile_peer_handoff() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected
        );
        let _ = adapter.drain_effects(usize::MAX);

        assert_eq!(
            adapter.insert_lane_qc(
                lane_qc_for_phase(&proposal, &keys, CertPhase::Prepare),
                locked_round.view,
            ),
            V2LaneIngressOutcome::Inserted
        );
        assert_eq!(
            adapter.insert_lane_qc(
                lane_qc_for_phase(&proposal, &keys, CertPhase::Commit),
                locked_round.view,
            ),
            V2LaneIngressOutcome::Inserted
        );
        adapter.drive_lane_sessions();
        let _ = adapter.drain_effects(usize::MAX);
        assert!(
            !adapter.has_pending_committed_output_handoff(),
            "the first complete fanout must have transferred to the volatile peer corridor"
        );

        adapter
            .schedule_retransmission()
            .expect("durable completed certificate starts another fanout round");
        let observed = adapter
            .drain_effects(usize::MAX)
            .into_iter()
            .filter_map(|effect| match effect {
                V2LaneWorkEffect::PostLaneBlock {
                    peer,
                    message: BlockMessage::LaneBlockQc(qc),
                } if qc.body.phase == CertPhase::Commit
                    && qc.body.proposal_hash == proposal.proposal_hash =>
                {
                    Some(peer)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let expected = proposal
            .descriptor
            .validator_set
            .iter()
            .filter(|peer| *peer != &adapter.local_peer)
            .cloned()
            .collect::<BTreeSet<_>>();
        assert_eq!(observed, expected);
    }

    #[test]
    fn fixed_view_zero_genesis_binds_under_a_later_proposal_lock() {
        let (mut adapter, _keys) = fixture_at_height(wire::ConsensusMode::Permissioned, 1);
        let genesis_key = KeyPair::try_from_seed(vec![0xE1; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let genesis_transaction = TransactionBuilder::new(
            ChainId::from("fixed-view-zero-genesis"),
            AccountId::new(genesis_key.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .sign(genesis_key.private_key());
        let staged_genesis = SignedBlock::genesis(
            vec![genesis_transaction.clone()],
            genesis_key.private_key(),
            None,
            None,
        );
        let proposal = staged_genesis.canonical_resultless_proposal();
        let canonical_wire = proposal.encode_wire().expect("encode genesis proposal");
        let subject = wire::BlockSubject {
            parent_block_hash: proposal.header().prev_block_hash(),
            block_hash: proposal.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let later_round = wire::ConsensusRound {
            context_id: adapter.context.id(),
            height: 1,
            view: 3,
        };
        assert_eq!(
            adapter.mark_global_body_locked(later_round, subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_eq!(
            adapter.bind_locked_global_body(&proposal),
            V2LaneIngressOutcome::Rejected,
            "the ordinary binding path must keep exact proposal-view semantics"
        );

        let wrong_key = KeyPair::try_from_seed(vec![0xE2; 32], Algorithm::Ed25519)
            .expect("different deterministic genesis key");
        let wrong_genesis = SignedBlock::genesis(
            vec![genesis_transaction],
            wrong_key.private_key(),
            None,
            None,
        );
        assert_eq!(
            adapter.bind_locked_genesis_body(&proposal, &wrong_genesis),
            V2LaneIngressOutcome::Rejected,
            "the fixed-view exception must match the authenticated staged genesis bytes"
        );
        assert_ne!(
            adapter.bind_locked_genesis_body(&proposal, &staged_genesis),
            V2LaneIngressOutcome::Rejected,
            "the exact authenticated view-zero genesis remains recoverable after a certified view change"
        );
    }

    #[test]
    fn cross_view_global_lock_fails_exact_body_binding() {
        let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
        let (block, _) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
        let (original_round, subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(original_round, subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_ne!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected
        );

        let higher_round = wire::ConsensusRound {
            view: original_round.view + 1,
            ..original_round
        };
        assert_eq!(
            adapter.mark_global_body_locked(higher_round, subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
        assert_eq!(
            adapter.bind_locked_global_body(&block),
            V2LaneIngressOutcome::Rejected,
            "the exact lock round must match the immutable body header view"
        );
        assert_eq!(
            adapter.bind_locked_genesis_body(&block, &block),
            V2LaneIngressOutcome::Rejected,
            "the fixed-view genesis path cannot weaken a successor-height lock"
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
        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
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

        let (locked_round, locked_subject) = global_lock_for_block(&adapter, &block);
        assert_eq!(
            adapter.mark_global_body_locked(locked_round, locked_subject),
            Ok(GlobalBodyLockOutcome::Inserted)
        );
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
            if let Some(canonical) = try_store_canonical_anchor(&adapter, &proposal, &keys[0]) {
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
            } else {
                assert!(
                    adapter.kura.read_lane_block_artifact(lane_id, 1).is_none(),
                    "Kura must not expose an artifact rejected for inactive route geometry"
                );
            }
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
            if let Some(canonical) = try_store_canonical_anchor(&adapter, &proposal, &keys[0]) {
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
            } else {
                assert!(
                    adapter.kura.read_lane_block_artifact(lane_id, 1).is_none(),
                    "Kura must not expose an artifact rejected for a retired incarnation"
                );
            }
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
            .with_lane_block_descriptor_hash(Some(Hash::new(
                b"v2 merge persistence retry lane descriptor",
            )))
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
