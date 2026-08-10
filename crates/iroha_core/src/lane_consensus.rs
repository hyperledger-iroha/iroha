//! Lane-local block vote validation, session caching, and QC aggregation helpers.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Instant;

use iroha_crypto::PrivateKey;
use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey, Signature};
use iroha_data_model::merge::MergeSignerProof;
use iroha_data_model::{
    NetworkId,
    block::{
        AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1, AutonomousLanePayloadEnvelopeV1, BlockHeader,
        consensus::{
            CertPhase, LaneBlockProposalPayloadHintV1, LaneBlockProposalV1, LaneBlockQcV1,
            LaneBlockVoteBodyV1, LanePayloadAvailabilityBodyV1, LanePayloadAvailabilityQcV1,
            NativeAmxReceipt, SumeragiLanePayloadOwnership,
        },
        consensus_v2::HeightContextId,
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    merge::{
        LaneDrainCertificateBodyV1, LaneDrainCertificateV1, LaneDrainFrontierV1, LaneDrainIntentV1,
        MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES, MAX_MERGE_EXECUTION_ENTRYPOINTS,
        MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES, lane_drain_empty_unresolved_evidence_root,
    },
    nexus::{DataSpaceId, LaneId},
    peer::PeerId,
    transaction::signed::TransactionEntrypoint,
};
use iroha_logger::prelude::*;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    json_macros::{JsonDeserialize, JsonSerialize},
    kura::LaneReadyAuthorization,
    queue::{LaneQueueReservationKeyV2, RouteLegRole, RoutingPlan},
    tx::AcceptedTransaction,
};

/// Maximum executable entrypoints retained in one autonomous lane payload.
///
/// The proposal path applies tighter configured block limits. This hard ceiling
/// is a final defence for artifacts recovered from untrusted storage or future
/// lane-local transport.
pub(crate) const MAX_LANE_EXECUTABLE_ENTRYPOINTS: usize = MAX_MERGE_EXECUTION_ENTRYPOINTS;

/// Maximum lane validators admitted into one vote or certificate.
pub(crate) const MAX_LANE_BLOCK_VALIDATORS: usize =
    iroha_data_model::consensus::MAX_LANE_CONSENSUS_VALIDATORS;
/// Canonical compressed BLS-normal signature and proof-of-possession length.
pub(crate) const LANE_BLS_PROOF_BYTES: usize = 96;
/// Maximum canonical READY body bytes before signing or hashing.
pub(crate) const MAX_LANE_PAYLOAD_AVAILABILITY_BODY_BYTES: usize = 4 * 1024;
/// Maximum self-contained READY QC bytes admitted from wire or storage.
pub(crate) const MAX_LANE_PAYLOAD_AVAILABILITY_QC_BYTES: usize = 64 * 1024;
/// Maximum self-contained lane drain certificate bytes admitted from wire or storage.
pub(crate) const MAX_LANE_DRAIN_CERTIFICATE_BYTES: usize = 64 * 1024;
/// Maximum canonical lane drain vote bytes admitted before signature verification.
pub(crate) const MAX_LANE_DRAIN_VOTE_BYTES: usize = 16 * 1024;
/// Maximum UTF-8 bytes retained for the READY consensus-domain tag.
const MAX_LANE_AVAILABILITY_QC_MODE_TAG_BYTES: usize = 256;
const MAX_LANE_NEW_VIEW_QC_MODE_TAG_BYTES: usize = 256;

/// Bytes reserved below the default consensus frame cap for the authenticated
/// view/QC envelope and the later globally certified merge transcript.
#[cfg(test)]
pub(crate) const LANE_EXECUTABLE_ENVELOPE_HEADROOM_BYTES: usize =
    iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS.get()
        - MAX_LANE_EXECUTABLE_PAYLOAD_BYTES;
/// Maximum canonical Norito body bytes retained for one autonomous lane payload.
///
/// The payload body is at most half of one authenticated source bundle because
/// that bundle also carries availability, view-change, prepare, commit, and PoP
/// evidence. The global merge batch then has a separate bounded budget for the
/// repeated executable transcript, deterministic results, and settlement.
pub(crate) const MAX_LANE_EXECUTABLE_PAYLOAD_BYTES: usize =
    MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES / 2;

/// Resource budget for decoding one untrusted canonical payload frame.
const AUTONOMOUS_LANE_PAYLOAD_DECODE_LIMITS: norito::DecodeLimits = norito::DecodeLimits::new(
    // The payload contains both bounded entrypoint vectors and legitimate byte
    // blobs such as a full native smart-contract upload chunk. Entrypoint
    // cardinality is validated semantically after decode; using that 4,096
    // ceiling as the generic sequence limit incorrectly rejects a 64-KiB
    // `Vec<u8>`. The nested payload-derived canonical limits remain the
    // effective allocation/element bomb boundary.
    MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES,
    MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES,
    MAX_LANE_EXECUTABLE_ENTRYPOINTS * 256,
    MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES * 4,
    128,
);

/// Current producer-authenticated executable payload layout.
///
/// Version two removes the advisory global block hint from both authenticated
/// preimages. Version one and unknown versions fail closed.
pub(crate) const LANE_EXECUTABLE_PAYLOAD_VERSION_V2: u8 = 2;

/// Return the unique height-rotated author for an autonomous lane block.
///
/// Autonomous authorship is independent of the global carrier view. A zero
/// lane height or empty committee has no valid author and therefore fails
/// closed at every caller.
pub(crate) fn deterministic_lane_author(
    validator_set: &[PeerId],
    lane_block_height: u64,
) -> Option<&PeerId> {
    let author_offset = lane_block_height.checked_sub(1)?;
    let validator_count = u64::try_from(validator_set.len()).ok()?;
    if validator_count == 0 {
        return None;
    }
    let author_index = usize::try_from(author_offset % validator_count).ok()?;
    validator_set.get(author_index)
}

/// Maximum authenticated view transitions retained for one lane height.
pub(crate) const MAX_LANE_NEW_VIEW_CERTIFICATES: usize = 256;

/// Canonical, producer-authenticated executable payload for one lane height.
///
/// `payload_hash` deliberately excludes the advisory global proposal hint, the
/// lane view cursor, and the producer. It is therefore stable when the
/// finalized carrier hint is attached or an authenticated NewView certificate
/// advances the synthetic retransmission cursor. The producer signature also
/// excludes only the advisory hint while separately binding the payload hash
/// to the immutable certification proposal.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LaneExecutablePayloadV1 {
    /// Artifact schema version. Only version two is accepted.
    pub version: u8,
    /// Exact genesis-derived network identity that owns this payload.
    pub network_id: NetworkId,
    /// Consensus epoch at the proposal height.
    pub epoch: u64,
    /// First canonical proposal that advertised this executable payload.
    pub origin_proposal: LaneBlockProposalV1,
    /// Canonical hashes of `entrypoints`, in descriptor order.
    pub entrypoint_hashes: Vec<Hash>,
    /// Executable transaction entrypoints owned by the lane.
    pub entrypoints: Vec<TransactionEntrypoint>,
    /// Exact queue reservation identities in entrypoint order. Every payload
    /// binds one reservation per entrypoint.
    pub reservation_keys: Vec<LaneQueueReservationKeyV2>,
    /// Full coordinator/participant routing plans in entrypoint order.
    pub routing_plans: Vec<RoutingPlan>,
    /// Native AMX certificates aligned exactly with entrypoints and routing plans.
    ///
    /// Autonomous payloads carry one element per entrypoint: `Some` for a
    /// cross-dataspace native AMX plan and `None` for a single-route plan.
    pub native_amx_receipts: Vec<Option<NativeAmxReceipt>>,
    /// View-neutral digest of network, epoch, lane coordinates, predecessor,
    /// committee, and every exact entrypoint/reservation/routing/receipt tuple.
    pub payload_hash: Hash,
    /// Lane committee member that authenticated the origin payload.
    pub producer: PeerId,
    /// BLS-normal signature over the producer-bound payload preimage.
    pub producer_signature: Vec<u8>,
}

#[derive(Clone, Debug, Encode)]
struct LaneExecutablePayloadPreimage {
    purpose: String,
    version: u8,
    network_id: NetworkId,
    epoch: u64,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    previous_lane_block_height: u64,
    previous_lane_block_descriptor_hash: Option<Hash>,
    lane_block_height: u64,
    accepted_candidate_indices: Vec<u64>,
    accepted_transaction_hashes: Vec<Hash>,
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_set: Vec<PeerId>,
    validator_count: u32,
    min_quorum: u32,
    qc_mode_tag: String,
    entrypoints: Vec<TransactionEntrypoint>,
    reservation_keys: Vec<LaneQueueReservationKeyV2>,
    routing_plans: Vec<RoutingPlan>,
    native_amx_receipts: Vec<Option<NativeAmxReceipt>>,
}

#[derive(Clone, Debug, Encode)]
struct LaneExecutablePayloadSignaturePreimage {
    purpose: String,
    version: u8,
    network_id: NetworkId,
    epoch: u64,
    origin_proposal_hash: Hash,
    origin_descriptor_hash: Hash,
    origin_lane_block_view: u64,
    payload_hash: Hash,
    producer: PeerId,
}

/// Authenticated request to advance one lane height to the next view while
/// retaining the exact executable payload.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
pub struct LaneBlockNewViewBodyV1 {
    /// Certificate schema version. Only version one is accepted.
    pub version: u8,
    /// Exact genesis-derived network identity that owns this transition.
    pub network_id: NetworkId,
    /// Consensus epoch at the proposal height.
    pub epoch: u64,
    /// Lane whose view advances.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane.
    pub dataspace_id: DataSpaceId,
    /// Exact active lane incarnation commitment.
    pub lane_incarnation: Hash,
    /// Global compatibility height that selected the lane work.
    pub proposal_height: u64,
    /// Lane-local height whose view advances.
    pub lane_block_height: u64,
    /// Synthetic cursor view containing the retained origin identity.
    pub from_view: u64,
    /// Exact next view authorized by this body.
    pub target_view: u64,
    /// Cursor proposal digest retained across the transition.
    pub locked_proposal_hash: Hash,
    /// Cursor descriptor digest retained across the transition.
    pub locked_descriptor_hash: Hash,
    /// View-neutral executable payload digest retained across the transition.
    pub executable_payload_hash: Hash,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Hash of the canonical lane committee.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Number of validators in the lane committee.
    pub validator_count: u32,
    /// Minimum distinct signers required for this transition.
    pub min_quorum: u32,
    /// Lane consensus domain tag.
    pub qc_mode_tag: String,
}

impl LaneBlockNewViewBodyV1 {
    /// Canonical, exact-network-bound BLS signature preimage.
    ///
    /// # Errors
    ///
    /// Returns [`LaneAutonomousArtifactError::InvalidNewViewBody`] if the
    /// canonical encoder rejects the body.
    pub(crate) fn signature_preimage(&self) -> Result<Vec<u8>, LaneAutonomousArtifactError> {
        let mut out = Vec::with_capacity(512);
        out.extend_from_slice(b"iroha:nexus:lane-new-view:v1");
        let encoded = norito::encode_canonical(self)
            .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewBody)?;
        out.extend_from_slice(&encoded);
        Ok(out)
    }
}

/// Individual committee vote for a lane-local view transition.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LaneBlockNewViewVoteV1 {
    /// Common body signed by the committee member.
    pub body: LaneBlockNewViewBodyV1,
    /// Committee member producing the vote.
    pub signer: PeerId,
    /// BLS-normal signature over `body.signature_preimage()`.
    pub bls_signature: Vec<u8>,
}

/// Individual authoritative-lane-committee vote closing an incarnation at an
/// exact globally merged frontier.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LaneDrainVoteV1 {
    /// Common drain body signed by the committee member.
    pub body: LaneDrainCertificateBodyV1,
    /// Committee member producing the vote.
    pub signer: PeerId,
    /// BLS proof of possession for `signer`, retained for restart-safe aggregation.
    pub proof_of_possession: Vec<u8>,
    /// BLS-normal signature over `body.signature_preimage()`.
    pub bls_signature: Vec<u8>,
}

/// Immutable identity used to detect one drain signer's conflicting decisions.
///
/// The intent hash deliberately excludes the mutable final frontier: a signer
/// may refresh its vote only when that frontier advances monotonically for the
/// same immutable drain intent.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LaneDrainRemoteSignerContext {
    intent_hash: HashOf<LaneDrainIntentV1>,
    signer: PeerId,
}

/// Most recent accepted frontier decision for one signer and drain intent.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LaneDrainRemoteSignerDecision {
    body_digest: Hash,
    final_frontier: LaneDrainFrontierV1,
    last_seen: Instant,
}

/// Bounded in-memory collector for votes over one canonical lane-drain body.
///
/// Signer decisions survive a monotonic frontier refresh for the same intent,
/// so a same-height conflict or frontier regression remains quarantined even
/// after the active body changes. A distinct intent starts a fresh lifecycle.
#[derive(Debug)]
pub(crate) struct LaneDrainVoteState {
    active_body: Option<LaneDrainCertificateBodyV1>,
    votes: BTreeMap<PeerId, LaneDrainVoteV1>,
    remote_signers: BTreeMap<LaneDrainRemoteSignerContext, LaneDrainRemoteSignerDecision>,
    remote_equivocators: BTreeMap<LaneDrainRemoteSignerContext, Instant>,
    certificate: Option<LaneDrainCertificateV1>,
}

/// Return the canonical, duplicate-free union of lane and global recipients.
///
/// The local peer is excluded because its vote is inserted directly before the
/// network broadcast. `BTreeSet` ordering makes the fan-out deterministic.
pub(crate) fn lane_drain_vote_recipients(
    lane_committee: &[PeerId],
    global_committee: &[PeerId],
    local_peer: &PeerId,
) -> Vec<PeerId> {
    lane_committee
        .iter()
        .chain(global_committee)
        .filter(|peer| *peer != local_peer)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

impl LaneDrainVoteState {
    const MAX_REMOTE_SIGNERS: usize = 4_096;

    /// Create an empty drain-vote collector.
    pub(crate) fn new() -> Self {
        Self {
            active_body: None,
            votes: BTreeMap::new(),
            remote_signers: BTreeMap::new(),
            remote_equivocators: BTreeMap::new(),
            certificate: None,
        }
    }

    /// Return the exact body currently eligible to receive votes.
    pub(crate) fn active_body(&self) -> Option<&LaneDrainCertificateBodyV1> {
        self.active_body.as_ref()
    }

    /// Return accepted votes keyed by their unique signer.
    pub(crate) fn votes(&self) -> &BTreeMap<PeerId, LaneDrainVoteV1> {
        &self.votes
    }

    /// Return the aggregate certificate, when the caller has sealed one.
    pub(crate) fn certificate(&self) -> Option<&LaneDrainCertificateV1> {
        self.certificate.as_ref()
    }

    /// Install the aggregate certificate for the active body.
    pub(crate) fn set_certificate(&mut self, certificate: LaneDrainCertificateV1) {
        self.certificate = Some(certificate);
    }

    fn body_digest(body: &LaneDrainCertificateBodyV1) -> Hash {
        Hash::new(body.signature_preimage())
    }

    /// Select the canonical body, resetting per-body votes and certificates.
    ///
    /// Decisions and quarantines are retained only while the immutable drain
    /// intent remains identical, allowing strictly higher frontier refreshes
    /// without forgetting prior signer behavior.
    pub(crate) fn retain_body(&mut self, body: Option<LaneDrainCertificateBodyV1>) {
        if self.active_body.as_ref() == body.as_ref() {
            return;
        }
        let retains_intent_decisions = self
            .active_body
            .as_ref()
            .zip(body.as_ref())
            .is_some_and(|(active, next)| active.intent == next.intent);
        self.active_body = body;
        self.votes.clear();
        // A final frontier can advance while pre-close, already-certified work
        // is globally applied. Keep signer decisions for the same immutable
        // intent so same-height drift and frontier regression remain detectable;
        // `insert_vote` permits only a strictly higher refreshed frontier. A
        // different/no intent is a distinct lifecycle and drops the cache.
        if !retains_intent_decisions {
            self.remote_signers.clear();
            self.remote_equivocators.clear();
        }
        self.certificate = None;
    }

    /// Insert a vote for the active canonical body.
    ///
    /// A signer may replace its decision only with a strictly higher frontier
    /// for the same immutable intent. Same-height drift and regression remove
    /// its vote and quarantine it for that intent's remaining lifetime.
    pub(crate) fn insert_vote(
        &mut self,
        vote: LaneDrainVoteV1,
        now: Instant,
    ) -> Result<bool, &'static str> {
        vote.validate_ingress()
            .map_err(|_| "vote failed lane-drain ingress validation")?;
        let context = LaneDrainRemoteSignerContext {
            intent_hash: vote.body.intent.canonical_hash(),
            signer: vote.signer.clone(),
        };
        if self.remote_equivocators.contains_key(&context) {
            return Err("signer already equivocated for this drain intent");
        }
        let body_digest = Self::body_digest(&vote.body);
        if let Some(existing) = self.remote_signers.get(&context)
            && existing.body_digest != body_digest
        {
            if vote.body.final_frontier.lane_block_height
                <= existing.final_frontier.lane_block_height
            {
                self.remote_signers.remove(&context);
                self.remote_equivocators.insert(context.clone(), now);
                self.votes.remove(&context.signer);
                self.certificate = None;
                self.prune_remote_signers();
                return Err("signer equivocated or regressed across drain bodies");
            }
        }
        if self.active_body.as_ref() != Some(&vote.body) {
            return Err("vote does not match the active canonical drain body");
        }
        self.remote_signers.insert(
            context,
            LaneDrainRemoteSignerDecision {
                body_digest,
                final_frontier: vote.body.final_frontier,
                last_seen: now,
            },
        );
        let changed = self
            .votes
            .insert(vote.signer.clone(), vote.clone())
            .as_ref()
            != Some(&vote);
        self.prune_remote_signers();
        Ok(changed)
    }

    fn prune_remote_signers(&mut self) {
        while self.remote_signers.len() > Self::MAX_REMOTE_SIGNERS {
            let oldest = self
                .remote_signers
                .iter()
                .min_by_key(|(context, decision)| (decision.last_seen, (*context).clone()))
                .map(|(context, _)| context.clone());
            let Some(oldest) = oldest else { break };
            self.remote_signers.remove(&oldest);
            self.votes.remove(&oldest.signer);
        }
        while self.remote_equivocators.len() > Self::MAX_REMOTE_SIGNERS {
            let oldest = self
                .remote_equivocators
                .iter()
                .min_by_key(|(context, observed)| (**observed, (*context).clone()))
                .map(|(context, _)| context.clone());
            let Some(oldest) = oldest else { break };
            self.remote_equivocators.remove(&oldest);
        }
    }
}

/// Individual READY vote for one exact autonomous lane executable payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize, JsonDeserialize)]
pub struct LanePayloadAvailabilityVoteV1 {
    /// Exact payload/session body signed by the committee member.
    pub body: LanePayloadAvailabilityBodyV1,
    /// Committee member that durably retained the payload before signing.
    pub signer: PeerId,
    /// PoPs aligned with the exact historical committee order.
    pub validator_set_pops: Vec<Vec<u8>>,
    /// BLS-normal signature over `body.signature_preimage()`.
    pub bls_signature: Vec<u8>,
}

/// Quorum certificate authorizing one lane-local view transition.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct LaneBlockNewViewCertificateV1 {
    /// Body certified by the aggregate signature.
    pub body: LaneBlockNewViewBodyV1,
    /// Canonical validator order indexed by `signers_bitmap`.
    pub validator_set: Vec<PeerId>,
    /// Compact signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate signature bytes.
    pub bls_aggregate_signature: Vec<u8>,
}

/// Persistable NewView certificate plus the exact PoPs needed to verify it
/// after restart without trusting current mutable topology state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(crate) struct DurableLaneBlockNewViewCertificateV1 {
    /// Authenticated lane-local NewView certificate.
    pub(crate) certificate: LaneBlockNewViewCertificateV1,
    /// Proofs of possession for exactly the selected certificate signers.
    pub(crate) signer_pops: BTreeMap<PublicKey, Vec<u8>>,
}

/// Restart-verifiable lane payload availability certificate.
///
/// An autonomous prepare vote carries a separate, domain-separated READY
/// signature only after the signer validates and durably persists the exact
/// producer-authenticated payload. The prepare QC embeds the aggregate READY
/// certificate and historical signer PoPs, so no unsigned sidecar field is
/// trusted after restart. This certificate always names
/// [`LaneExecutablePayloadV1::origin_proposal`]: NewView certificates may move
/// a synthetic lane-local cursor, but must never create a second availability
/// or certification subject for the immutable payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(crate) struct DurableLanePayloadAvailabilityCertificateV1 {
    /// Prepare QC containing the exact aggregate READY certificate.
    pub(crate) certificate: LaneBlockQcV1,
}

/// Restart-verifiable compaction point for an authenticated lane view chain.
///
/// The quorum certificate signs the exact `source_proposal` and authorizes the
/// exact next-view `target_proposal`. Both proposals are retained so restart
/// validation never has to trust a mutable topology, an implicit view number,
/// or certificates that were deliberately compacted away.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub(crate) struct DurableLaneBlockViewCheckpointV1 {
    /// Synthetic cursor proposal locked by the compacting NewView quorum.
    pub(crate) source_proposal: LaneBlockProposalV1,
    /// Synthetic next-view cursor authorized by `certificate`.
    pub(crate) target_proposal: LaneBlockProposalV1,
    /// Quorum-authenticated transition, including restart-verifiable PoPs.
    pub(crate) certificate: DurableLaneBlockNewViewCertificateV1,
}

/// Failure while building or validating a lane drain vote or certificate.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(crate) enum LaneDrainCertificateError {
    /// The intent or certificate uses an unsupported schema version.
    #[error("unsupported lane drain certificate version")]
    UnsupportedVersion,
    /// Intent coordinates, frontier, or committee shape are malformed.
    #[error("lane drain intent is malformed")]
    InvalidIntent,
    /// The certified final frontier precedes the frontier recorded by the intent.
    #[error("lane drain certificate regresses the initial merged frontier")]
    FrontierRegression,
    /// A zero/non-zero frontier was paired with the wrong optional descriptor hash shape.
    #[error("lane drain frontier descriptor hash shape is invalid")]
    FrontierHashMismatch,
    /// Frontier route/incarnation differs from its enclosing intent.
    #[error("lane drain frontier route or incarnation is invalid")]
    FrontierRouteMismatch,
    /// Native evidence is malformed or inconsistent with the bound frontier.
    #[error("lane drain Native frontier evidence is invalid")]
    InvalidFrontierEvidence,
    /// The frontier does not certify the canonical empty unresolved-evidence root.
    #[error("lane drain frontier still binds unresolved evidence")]
    UnresolvedEvidence,
    /// The supplied validator set does not match the intent's exact committee commitment.
    #[error("lane drain validator set is invalid")]
    InvalidValidatorSet,
    /// A vote signer is absent from the exact drain committee.
    #[error("lane drain vote signer is not in committee")]
    SignerNotInCommittee,
    /// The same drain signer appeared more than once.
    #[error("duplicate lane drain vote signer")]
    DuplicateSigner,
    /// Drain votes certify different bodies.
    #[error("lane drain vote body mismatch")]
    BodyMismatch,
    /// An individual drain vote signature is malformed or invalid.
    #[error("lane drain vote signature is invalid")]
    InvalidVoteSignature,
    /// A lane drain vote exceeds its control-plane byte envelope.
    #[error("lane drain vote exceeds its byte limit")]
    VoteTooLarge,
    /// The signer bitmap is malformed, has padding bits, or names an out-of-range signer.
    #[error("lane drain signer bitmap is invalid")]
    InvalidBitmap,
    /// The drain certificate does not contain enough distinct committee signatures.
    #[error("lane drain certificate quorum is not met")]
    QuorumNotMet,
    /// A selected signer's PoP is absent, reordered, malformed, or invalid.
    #[error("lane drain signer proof of possession is invalid")]
    InvalidProofOfPossession,
    /// Aggregate signature construction or verification failed.
    #[error("lane drain aggregate signature is invalid")]
    InvalidAggregateSignature,
    /// The self-contained certificate exceeds its protocol byte limit.
    #[error("lane drain certificate exceeds its byte limit")]
    CertificateTooLarge,
}

/// Failure while building or validating an autonomous lane payload or NewView proof.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(crate) enum LaneAutonomousArtifactError {
    /// An unsupported artifact version was supplied.
    #[error("unsupported autonomous lane artifact version")]
    UnsupportedVersion,
    /// The embedded standalone lane proposal is invalid.
    #[error("invalid autonomous lane proposal")]
    InvalidProposal,
    /// The artifact belongs to another exact network or epoch.
    #[error("autonomous lane artifact network or epoch mismatch")]
    NetworkOrEpochMismatch,
    /// Entrypoint count exceeds the defensive hard limit.
    #[error("autonomous lane payload entrypoint limit exceeded")]
    EntrypointLimitExceeded,
    /// Canonical entrypoint bytes exceed the defensive hard limit.
    #[error("autonomous lane payload byte limit exceeded")]
    PayloadByteLimitExceeded,
    /// Entrypoints do not match the descriptor hashes.
    #[error("autonomous lane payload entrypoints do not match descriptor")]
    EntrypointMismatch,
    /// Durable queue reservations do not exactly bind the executable payload.
    #[error("autonomous lane payload reservation bindings do not match payload")]
    ReservationMismatch,
    /// Full routing plans do not exactly bind coordinator-owned execution.
    #[error("autonomous lane payload routing plans do not match payload")]
    RoutingPlanMismatch,
    /// Native AMX receipts are missing, extra, reordered, or do not bind the exact session.
    #[error("autonomous lane payload native AMX receipts do not match payload")]
    NativeAmxReceiptMismatch,
    /// Canonical payload hashing failed or the stored digest mismatched.
    #[error("autonomous lane payload hash mismatch")]
    PayloadHashMismatch,
    /// Payload producer is not a member of the lane committee.
    #[error("autonomous lane payload producer is not in committee")]
    ProducerNotInCommittee,
    /// Payload producer is not the height-rotated lane author.
    #[error("autonomous lane payload producer is not the deterministic lane author")]
    ProducerNotDeterministicAuthor,
    /// Payload producer is not a BLS-normal consensus identity.
    #[error("autonomous lane payload producer is not BLS-normal")]
    ProducerNotBlsNormal,
    /// Producer signature is missing, malformed, or invalid.
    #[error("autonomous lane payload producer signature is invalid")]
    InvalidProducerSignature,
    /// A global block hint was attached where canonical pre-anchor bytes were required.
    #[error("autonomous lane payload global anchor hint is invalid")]
    InvalidGlobalAnchorHint,
    /// Canonical payload bytes are malformed, non-canonical, or contain trailing data.
    #[error("autonomous lane payload canonical encoding is invalid")]
    InvalidCanonicalPayloadEncoding,
    /// Envelope identity fields do not exactly match the authenticated payload.
    #[error("autonomous lane payload envelope identity mismatch")]
    PayloadEnvelopeMismatch,
    /// The complete framed payload or envelope exceeds its protocol budget.
    #[error("autonomous lane payload envelope byte limit exceeded")]
    PayloadEnvelopeByteLimitExceeded,
    /// NewView body is malformed, stale, or skips a view.
    #[error("lane NewView body is malformed")]
    InvalidNewViewBody,
    /// NewView signer is not in the certified committee.
    #[error("lane NewView signer is not in committee")]
    NewViewSignerNotInCommittee,
    /// NewView vote signature is invalid.
    #[error("lane NewView vote signature is invalid")]
    InvalidNewViewSignature,
    /// The same signer appeared more than once.
    #[error("duplicate lane NewView signer")]
    DuplicateNewViewSigner,
    /// Votes or certificate signers do not reach the bound quorum.
    #[error("lane NewView quorum is not met")]
    NewViewQuorumNotMet,
    /// Certificate bitmap is malformed.
    #[error("lane NewView signer bitmap is malformed")]
    InvalidNewViewBitmap,
    /// Required signer proof of possession is absent or invalid.
    #[error("lane NewView signer proof of possession is invalid")]
    InvalidNewViewPop,
    /// Aggregate signature is missing, malformed, or invalid.
    #[error("lane NewView aggregate signature is invalid")]
    InvalidNewViewAggregate,
    /// Votes certify different NewView bodies.
    #[error("lane NewView vote body mismatch")]
    NewViewBodyMismatch,
    /// NewView body does not bind the supplied source proposal and payload.
    #[error("lane NewView transition source mismatch")]
    NewViewSourceMismatch,
    /// Target proposal is not the deterministic next-view form of the source.
    #[error("lane NewView target proposal mismatch")]
    NewViewTargetMismatch,
    /// View arithmetic overflowed.
    #[error("lane NewView target overflow")]
    NewViewOverflow,
    /// BLS aggregation failed.
    #[error("lane NewView signature aggregation failed")]
    NewViewAggregation,
    /// Availability certificate does not bind the exact executable payload.
    #[error("lane payload availability certificate does not match payload")]
    AvailabilityMismatch,
    /// Availability body is malformed or exceeds defensive limits.
    #[error("lane payload availability body is invalid")]
    InvalidAvailabilityBody,
    /// READY signer is not in the exact certified lane committee.
    #[error("lane payload availability signer is not in committee")]
    AvailabilitySignerNotInCommittee,
    /// READY signer appeared more than once.
    #[error("duplicate lane payload availability signer")]
    DuplicateAvailabilitySigner,
    /// READY votes certify different payload/session bodies.
    #[error("lane payload availability vote body mismatch")]
    AvailabilityBodyMismatch,
    /// READY signer proof of possession is missing, malformed, or invalid.
    #[error("lane payload availability signer proof of possession is invalid")]
    InvalidAvailabilityPop,
    /// READY vote signature is missing, malformed, or invalid.
    #[error("lane payload availability vote signature is invalid")]
    InvalidAvailabilitySignature,
    /// READY votes do not satisfy the exact committee quorum.
    #[error("lane payload availability quorum is not met")]
    AvailabilityQuorumNotMet,
    /// READY signer bitmap is malformed or has trailing bits set.
    #[error("lane payload availability signer bitmap is invalid")]
    InvalidAvailabilityBitmap,
    /// READY aggregate signature construction or verification failed.
    #[error("lane payload availability aggregate signature is invalid")]
    InvalidAvailabilityAggregate,
    /// Move-only durable-input authority does not match the READY signing request.
    #[error("lane payload availability durable authorization does not match signing request")]
    AvailabilityAuthorizationMismatch,
    /// Availability certificate is not a valid aggregate prepare QC.
    #[error("lane payload availability certificate is invalid")]
    InvalidAvailabilityCertificate,
}

impl LaneExecutablePayloadV1 {
    /// Construct and sign an autonomous payload with exact durable queue
    /// ownership, routing-plan, and Native AMX receipt-slot bindings.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_signed_with_reservations(
        network_id: NetworkId,
        epoch: u64,
        origin_proposal: LaneBlockProposalV1,
        entrypoints: Vec<TransactionEntrypoint>,
        reservation_keys: Vec<LaneQueueReservationKeyV2>,
        routing_plans: Vec<RoutingPlan>,
        native_amx_receipts: Vec<Option<NativeAmxReceipt>>,
        producer: PeerId,
        private_key: &PrivateKey,
    ) -> Result<Self, LaneAutonomousArtifactError> {
        validate_lane_block_proposal(&origin_proposal)
            .map_err(|_| LaneAutonomousArtifactError::InvalidProposal)?;
        let entrypoint_hashes = entrypoints
            .iter()
            .map(|entrypoint| Hash::from(entrypoint.hash()))
            .collect::<Vec<_>>();
        let mut payload = Self {
            version: LANE_EXECUTABLE_PAYLOAD_VERSION_V2,
            network_id,
            epoch,
            origin_proposal,
            entrypoint_hashes,
            entrypoints,
            reservation_keys,
            routing_plans,
            native_amx_receipts,
            payload_hash: Hash::prehashed([0; Hash::LENGTH]),
            producer,
            producer_signature: Vec::new(),
        };
        payload.payload_hash = payload.computed_payload_hash()?;
        let producer_signature_preimage = payload.producer_signature_preimage()?;
        payload.producer_signature = Signature::try_new(private_key, &producer_signature_preimage)
            .map_err(|_| LaneAutonomousArtifactError::InvalidProducerSignature)?
            .payload()
            .to_vec();
        payload.validate(network_id, epoch)?;
        Ok(payload)
    }

    /// Compute the canonical view-neutral executable payload digest.
    pub(crate) fn computed_payload_hash(&self) -> Result<Hash, LaneAutonomousArtifactError> {
        compute_lane_executable_payload_hash(
            self.version,
            self.network_id,
            self.epoch,
            &self.origin_proposal,
            &self.entrypoints,
            &self.reservation_keys,
            &self.routing_plans,
            &self.native_amx_receipts,
        )
    }

    fn producer_signature_preimage(&self) -> Result<Vec<u8>, LaneAutonomousArtifactError> {
        let descriptor = &self.origin_proposal.descriptor;
        norito::encode_canonical(&LaneExecutablePayloadSignaturePreimage {
            purpose: "nexus:lane-executable-payload-signature:v2".to_owned(),
            version: self.version,
            network_id: self.network_id,
            epoch: self.epoch,
            origin_proposal_hash: self.origin_proposal.proposal_hash,
            origin_descriptor_hash: descriptor.descriptor_hash,
            origin_lane_block_view: descriptor.lane_block_view,
            payload_hash: self.payload_hash,
            producer: self.producer.clone(),
        })
        .map_err(|_| LaneAutonomousArtifactError::InvalidProducerSignature)
    }

    /// Validate shape, network/epoch binding, transaction hashes, producer
    /// authority, canonical digest, and producer signature.
    pub(crate) fn validate(
        &self,
        expected_network_id: NetworkId,
        expected_epoch: u64,
    ) -> Result<(), LaneAutonomousArtifactError> {
        validate_lane_executable_payload_body(
            self.version,
            self.network_id,
            self.epoch,
            &self.origin_proposal,
            &self.entrypoint_hashes,
            &self.entrypoints,
            &self.reservation_keys,
            &self.routing_plans,
            &self.native_amx_receipts,
            self.payload_hash,
            expected_network_id,
            expected_epoch,
        )?;
        if !self
            .origin_proposal
            .descriptor
            .validator_set
            .contains(&self.producer)
        {
            return Err(LaneAutonomousArtifactError::ProducerNotInCommittee);
        }
        if deterministic_lane_author(
            &self.origin_proposal.descriptor.validator_set,
            self.origin_proposal.descriptor.lane_block_height,
        ) != Some(&self.producer)
        {
            return Err(LaneAutonomousArtifactError::ProducerNotDeterministicAuthor);
        }
        if !peer_uses_bls_normal(&self.producer) {
            return Err(LaneAutonomousArtifactError::ProducerNotBlsNormal);
        }
        let producer_signature_preimage = self.producer_signature_preimage()?;
        Signature::try_from_bytes(&self.producer_signature)
            .map_err(|_| LaneAutonomousArtifactError::InvalidProducerSignature)?
            .verify(self.producer.public_key(), &producer_signature_preimage)
            .map_err(|_| LaneAutonomousArtifactError::InvalidProducerSignature)
    }

    /// Attach the finalized global carrier hint without changing any
    /// producer-authenticated payload identity.
    ///
    /// The helper validates both forms and proves that replacing the advisory
    /// hint leaves the payload hash, producer signature, exact reservations,
    /// and every consensus field unchanged.
    pub(crate) fn attach_global_hint_exact(
        &self,
        hint: LaneBlockProposalPayloadHintV1,
        expected_network_id: NetworkId,
        expected_epoch: u64,
    ) -> Result<Self, LaneAutonomousArtifactError> {
        self.validate(expected_network_id, expected_epoch)?;
        if self.origin_proposal.payload_block_hint.is_some()
            || hint.proposal_height == 0
            || hint.proposal_height != self.origin_proposal.descriptor.proposal_height
            || protocol_hash_bytes_are_zero(hint.proposal_block_hash.as_ref())
        {
            return Err(LaneAutonomousArtifactError::InvalidGlobalAnchorHint);
        }

        let mut attached = self.clone();
        attached.origin_proposal.payload_block_hint = Some(hint);
        attached.validate(expected_network_id, expected_epoch)?;

        let mut normalized = attached.clone();
        normalized.origin_proposal.payload_block_hint = self.origin_proposal.payload_block_hint;
        if normalized != *self
            || attached.payload_hash != self.payload_hash
            || attached.producer_signature != self.producer_signature
            || !attached
                .origin_proposal
                .same_consensus_identity(&self.origin_proposal)
            || attached.reservation_keys != self.reservation_keys
        {
            return Err(LaneAutonomousArtifactError::PayloadEnvelopeMismatch);
        }
        Ok(attached)
    }

    /// Return whether a proposal is the same view-neutral lane payload domain.
    pub(crate) fn matches_proposal_static(&self, proposal: &LaneBlockProposalV1) -> bool {
        if validate_lane_block_proposal(proposal).is_err() {
            return false;
        }
        let origin = &self.origin_proposal.descriptor;
        let candidate = &proposal.descriptor;
        origin.lane_id == candidate.lane_id
            && origin.dataspace_id == candidate.dataspace_id
            && origin.lane_incarnation == candidate.lane_incarnation
            && origin.proposal_height == candidate.proposal_height
            && origin.previous_lane_block_height == candidate.previous_lane_block_height
            && origin.previous_lane_block_descriptor_hash
                == candidate.previous_lane_block_descriptor_hash
            && origin.lane_block_height == candidate.lane_block_height
            && origin.accepted_candidate_indices == candidate.accepted_candidate_indices
            && origin.accepted_transaction_hashes == candidate.accepted_transaction_hashes
            && origin.validator_set_hash_version == candidate.validator_set_hash_version
            && origin.validator_set_hash == candidate.validator_set_hash
            && origin.validator_set == candidate.validator_set
            && origin.validator_count == candidate.validator_count
            && origin.min_quorum == candidate.min_quorum
            && origin.qc_mode_tag == candidate.qc_mode_tag
    }
}

/// Encode a validated hint-free payload into its exact global anchor envelope.
///
/// Callers must construct and sign the payload before the carrier block exists.
/// The finalized carrier hint can be attached with
/// [`LaneExecutablePayloadV1::attach_global_hint_exact`] after the block hash is
/// known.
pub(crate) fn autonomous_lane_payload_envelope(
    payload: &LaneExecutablePayloadV1,
    expected_network_id: NetworkId,
    expected_epoch: u64,
) -> Result<AutonomousLanePayloadEnvelopeV1, LaneAutonomousArtifactError> {
    payload.validate(expected_network_id, expected_epoch)?;
    if payload.origin_proposal.payload_block_hint.is_some()
        || payload.origin_proposal.descriptor.lane_block_view != 0
    {
        return Err(LaneAutonomousArtifactError::InvalidGlobalAnchorHint);
    }
    let canonical_payload = norito::encode_canonical(payload)
        .map_err(|_| LaneAutonomousArtifactError::InvalidCanonicalPayloadEncoding)?;
    if canonical_payload.is_empty()
        || canonical_payload.len() > MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES
    {
        return Err(LaneAutonomousArtifactError::PayloadEnvelopeByteLimitExceeded);
    }
    let descriptor = &payload.origin_proposal.descriptor;
    let envelope = AutonomousLanePayloadEnvelopeV1 {
        version: AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1,
        network_id: payload.network_id,
        epoch: payload.epoch,
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
        proposal_height: descriptor.proposal_height,
        lane_block_height: descriptor.lane_block_height,
        lane_block_view: descriptor.lane_block_view,
        proposal_hash: payload.origin_proposal.proposal_hash,
        descriptor_hash: descriptor.descriptor_hash,
        payload_hash: payload.payload_hash,
        producer: payload.producer.clone(),
        canonical_payload,
    };
    let envelope_bytes = norito::encode_canonical(&envelope)
        .map_err(|_| LaneAutonomousArtifactError::InvalidCanonicalPayloadEncoding)?;
    if envelope_bytes.len() > MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES {
        return Err(LaneAutonomousArtifactError::PayloadEnvelopeByteLimitExceeded);
    }
    Ok(envelope)
}

/// Canonically decode and validate one globally anchored autonomous payload.
///
/// Both the opaque payload frame and the complete envelope are checked against
/// their actual encoded byte budgets. Canonical decoding rejects alternate
/// layouts and trailing data before any identity is trusted.
pub(crate) fn decode_autonomous_lane_payload_envelope(
    envelope: &AutonomousLanePayloadEnvelopeV1,
    expected_network_id: NetworkId,
    expected_epoch: u64,
) -> Result<LaneExecutablePayloadV1, LaneAutonomousArtifactError> {
    if envelope.version != AUTONOMOUS_LANE_PAYLOAD_ENVELOPE_VERSION_V1 {
        return Err(LaneAutonomousArtifactError::UnsupportedVersion);
    }
    if envelope.canonical_payload.is_empty()
        || envelope.canonical_payload.len() > MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES
    {
        return Err(LaneAutonomousArtifactError::PayloadEnvelopeByteLimitExceeded);
    }
    let envelope_bytes = norito::encode_canonical(envelope)
        .map_err(|_| LaneAutonomousArtifactError::InvalidCanonicalPayloadEncoding)?;
    if envelope_bytes.len() > MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES {
        return Err(LaneAutonomousArtifactError::PayloadEnvelopeByteLimitExceeded);
    }

    let payload = norito::decode_canonical_with_limits::<LaneExecutablePayloadV1>(
        &envelope.canonical_payload,
        AUTONOMOUS_LANE_PAYLOAD_DECODE_LIMITS,
    )
    .map_err(|_| LaneAutonomousArtifactError::InvalidCanonicalPayloadEncoding)?;
    if payload.origin_proposal.payload_block_hint.is_some()
        || payload.origin_proposal.descriptor.lane_block_view != 0
    {
        return Err(LaneAutonomousArtifactError::InvalidGlobalAnchorHint);
    }
    payload.validate(expected_network_id, expected_epoch)?;

    let descriptor = &payload.origin_proposal.descriptor;
    if envelope.network_id != payload.network_id
        || envelope.epoch != payload.epoch
        || envelope.lane_id != descriptor.lane_id
        || envelope.dataspace_id != descriptor.dataspace_id
        || envelope.lane_incarnation != descriptor.lane_incarnation
        || envelope.proposal_height != descriptor.proposal_height
        || envelope.lane_block_height != descriptor.lane_block_height
        || envelope.lane_block_view != descriptor.lane_block_view
        || envelope.proposal_hash != payload.origin_proposal.proposal_hash
        || envelope.descriptor_hash != descriptor.descriptor_hash
        || envelope.payload_hash != payload.payload_hash
        || envelope.producer != payload.producer
    {
        return Err(LaneAutonomousArtifactError::PayloadEnvelopeMismatch);
    }
    Ok(payload)
}

/// Build the exact READY body for a producer-authenticated payload and the
/// view-specific proposal currently being prepared.
pub(crate) fn lane_payload_availability_body(
    executable_payload: &LaneExecutablePayloadV1,
    current_proposal: &LaneBlockProposalV1,
    expected_network_id: NetworkId,
    expected_epoch: u64,
) -> Result<LanePayloadAvailabilityBodyV1, LaneAutonomousArtifactError> {
    executable_payload.validate(expected_network_id, expected_epoch)?;
    validate_lane_block_proposal(current_proposal)
        .map_err(|_| LaneAutonomousArtifactError::InvalidProposal)?;
    if !executable_payload.matches_proposal_static(current_proposal) {
        return Err(LaneAutonomousArtifactError::AvailabilityMismatch);
    }
    let expected_current = if current_proposal.descriptor.lane_block_view
        == executable_payload
            .origin_proposal
            .descriptor
            .lane_block_view
    {
        executable_payload.origin_proposal.clone()
    } else {
        retarget_lane_block_proposal_exact_view(
            &executable_payload.origin_proposal,
            current_proposal.descriptor.lane_block_view,
        )?
    };
    if !expected_current.same_consensus_identity(current_proposal) {
        return Err(LaneAutonomousArtifactError::AvailabilityMismatch);
    }

    let origin = &executable_payload.origin_proposal.descriptor;
    let current = &current_proposal.descriptor;
    let body = LanePayloadAvailabilityBodyV1 {
        version: 1,
        network_id: expected_network_id,
        epoch: expected_epoch,
        lane_id: current.lane_id,
        dataspace_id: current.dataspace_id,
        lane_incarnation: current.lane_incarnation,
        proposal_height: current.proposal_height,
        lane_block_height: current.lane_block_height,
        origin_lane_block_view: origin.lane_block_view,
        origin_proposal_hash: executable_payload.origin_proposal.proposal_hash,
        origin_descriptor_hash: origin.descriptor_hash,
        current_lane_block_view: current.lane_block_view,
        current_proposal_hash: current_proposal.proposal_hash,
        current_descriptor_hash: current.descriptor_hash,
        current_subject_hash: current.subject_hash,
        current_payload_ownership_hash: current.payload_ownership_hash,
        current_rbc_instance_hash: current.rbc_instance_hash,
        executable_payload_hash: executable_payload.payload_hash,
        validator_set_hash_version: current.validator_set_hash_version,
        validator_set_hash: current.validator_set_hash,
        validator_count: current.validator_count,
        min_quorum: current.min_quorum,
        qc_mode_tag: current.qc_mode_tag.clone(),
    };
    validate_lane_payload_availability_body_shape(&body)?;
    Ok(body)
}

/// Verify that a READY body names the exact payload and current proposal.
pub(crate) fn validate_lane_payload_availability_body_against_payload(
    body: &LanePayloadAvailabilityBodyV1,
    executable_payload: &LaneExecutablePayloadV1,
    current_proposal: &LaneBlockProposalV1,
    expected_network_id: NetworkId,
    expected_epoch: u64,
) -> Result<(), LaneAutonomousArtifactError> {
    let expected = lane_payload_availability_body(
        executable_payload,
        current_proposal,
        expected_network_id,
        expected_epoch,
    )?;
    if body != &expected {
        return Err(LaneAutonomousArtifactError::AvailabilityMismatch);
    }
    Ok(())
}

fn validate_lane_payload_availability_body_shape(
    body: &LanePayloadAvailabilityBodyV1,
) -> Result<(), LaneAutonomousArtifactError> {
    let validator_count = usize::try_from(body.validator_count)
        .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilityBody)?;
    if body.version != 1
        || body.proposal_height == 0
        || body.lane_block_height == 0
        || protocol_hash_bytes_are_zero(body.lane_incarnation.as_ref())
        || body.origin_lane_block_view > body.current_lane_block_view
        || body.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
        || validator_count == 0
        || validator_count > MAX_LANE_BLOCK_VALIDATORS
        || body.min_quorum == 0
        || body.min_quorum > body.validator_count
        || usize::try_from(body.min_quorum).ok()
            != Some(crate::sumeragi::network_topology::commit_quorum_from_len(
                validator_count,
            ))
        || body.qc_mode_tag.trim().is_empty()
        || body.qc_mode_tag.len() > MAX_LANE_AVAILABILITY_QC_MODE_TAG_BYTES
    {
        return Err(LaneAutonomousArtifactError::InvalidAvailabilityBody);
    }
    if norito::encode_canonical(body).map_or(true, |encoded| {
        encoded.len() > MAX_LANE_PAYLOAD_AVAILABILITY_BODY_BYTES
    }) {
        return Err(LaneAutonomousArtifactError::InvalidAvailabilityBody);
    }
    Ok(())
}

fn validate_availability_body_matches_proposal(
    body: &LanePayloadAvailabilityBodyV1,
    proposal: &LaneBlockProposalV1,
) -> Result<(), LaneAutonomousArtifactError> {
    validate_lane_payload_availability_body_shape(body)?;
    let descriptor = &proposal.descriptor;
    if body.lane_id != descriptor.lane_id
        || body.dataspace_id != descriptor.dataspace_id
        || body.lane_incarnation != descriptor.lane_incarnation
        || body.proposal_height != descriptor.proposal_height
        || body.lane_block_height != descriptor.lane_block_height
        || body.current_lane_block_view != descriptor.lane_block_view
        || body.current_proposal_hash != proposal.proposal_hash
        || body.current_descriptor_hash != descriptor.descriptor_hash
        || body.current_subject_hash != descriptor.subject_hash
        || body.current_payload_ownership_hash != descriptor.payload_ownership_hash
        || body.current_rbc_instance_hash != descriptor.rbc_instance_hash
        || body.validator_set_hash_version != descriptor.validator_set_hash_version
        || body.validator_set_hash != descriptor.validator_set_hash
        || body.validator_count != descriptor.validator_count
        || body.min_quorum != descriptor.min_quorum
        || body.qc_mode_tag != descriptor.qc_mode_tag
    {
        return Err(LaneAutonomousArtifactError::AvailabilityMismatch);
    }
    Ok(())
}

fn availability_body_matches_lane_vote_body(
    availability: &LanePayloadAvailabilityBodyV1,
    vote: &LaneBlockVoteBodyV1,
) -> bool {
    availability.lane_id == vote.lane_id
        && availability.dataspace_id == vote.dataspace_id
        && availability.lane_incarnation == vote.lane_incarnation
        && availability.proposal_height == vote.proposal_height
        && availability.lane_block_height == vote.lane_block_height
        && availability.current_lane_block_view == vote.lane_block_view
        && availability.current_proposal_hash == vote.proposal_hash
        && availability.current_descriptor_hash == vote.descriptor_hash
        && availability.current_subject_hash == vote.subject_hash
        && availability.current_payload_ownership_hash == vote.payload_ownership_hash
        && availability.current_rbc_instance_hash == vote.rbc_instance_hash
        && availability.validator_set_hash_version == vote.validator_set_hash_version
        && availability.validator_set_hash == vote.validator_set_hash
        && availability.validator_count == vote.validator_count
        && availability.min_quorum == vote.min_quorum
        && availability.qc_mode_tag == vote.qc_mode_tag
}

impl LanePayloadAvailabilityVoteV1 {
    /// Consume Kura's exact durable-input authority and construct one READY vote.
    pub(crate) fn new_signed_with_authorization(
        authorization: LaneReadyAuthorization,
        proposal: &LaneBlockProposalV1,
        body: LanePayloadAvailabilityBodyV1,
        signer: PeerId,
        validator_set_pops: Vec<Vec<u8>>,
        private_key: &PrivateKey,
        height_context_id: HeightContextId,
    ) -> Result<Self, LaneAutonomousArtifactError> {
        if !authorization.consume_signing_request(proposal, &body, &signer, height_context_id) {
            return Err(LaneAutonomousArtifactError::AvailabilityAuthorizationMismatch);
        }
        validate_lane_payload_availability_body_shape(&body)?;
        let signature = Signature::try_new(private_key, &body.signature_preimage())
            .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilitySignature)?;
        let vote = Self {
            body,
            signer,
            validator_set_pops,
            bls_signature: signature.payload().to_vec(),
        };
        vote.validate_against_validator_set(&proposal.descriptor.validator_set)?;
        Ok(vote)
    }

    /// Construct a READY vote for a test fixture without a physical Kura boundary.
    #[cfg(test)]
    pub(crate) fn new_signed(
        body: LanePayloadAvailabilityBodyV1,
        signer: PeerId,
        validator_set_pops: Vec<Vec<u8>>,
        private_key: &PrivateKey,
    ) -> Result<Self, LaneAutonomousArtifactError> {
        validate_lane_payload_availability_body_shape(&body)?;
        let signature = Signature::try_new(private_key, &body.signature_preimage())
            .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilitySignature)?;
        let vote = Self {
            body,
            signer,
            validator_set_pops,
            bls_signature: signature.payload().to_vec(),
        };
        vote.validate_shape_and_signature()?;
        Ok(vote)
    }

    fn validate_shape(&self) -> Result<(), LaneAutonomousArtifactError> {
        validate_lane_payload_availability_body_shape(&self.body)?;
        let validator_count = usize::try_from(self.body.validator_count)
            .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilityBody)?;
        if self.validator_set_pops.len() != validator_count
            || self
                .validator_set_pops
                .iter()
                .any(|pop| pop.len() != LANE_BLS_PROOF_BYTES)
        {
            return Err(LaneAutonomousArtifactError::InvalidAvailabilityPop);
        }
        if !peer_uses_bls_normal(&self.signer) || self.bls_signature.len() != LANE_BLS_PROOF_BYTES {
            return Err(LaneAutonomousArtifactError::InvalidAvailabilitySignature);
        }
        Ok(())
    }

    fn verify_signature(&self) -> Result<(), LaneAutonomousArtifactError> {
        Signature::try_from_bytes(&self.bls_signature)
            .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilitySignature)?
            .verify(self.signer.public_key(), &self.body.signature_preimage())
            .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilitySignature)
    }

    fn validate_shape_and_signature(&self) -> Result<(), LaneAutonomousArtifactError> {
        self.validate_shape()?;
        self.verify_signature()
    }

    fn validate_against_validator_set(
        &self,
        validator_set: &[PeerId],
    ) -> Result<(), LaneAutonomousArtifactError> {
        self.validate_shape_and_signature()?;
        validate_lane_block_validator_set_fields(
            self.body.validator_set_hash_version,
            self.body.validator_set_hash,
            self.body.validator_count,
            self.body.min_quorum,
            validator_set,
        )
        .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilityBody)?;
        if validator_set.len() > MAX_LANE_BLOCK_VALIDATORS || !validator_set.contains(&self.signer)
        {
            return Err(LaneAutonomousArtifactError::AvailabilitySignerNotInCommittee);
        }
        for (validator, pop) in validator_set.iter().zip(&self.validator_set_pops) {
            if !peer_uses_bls_normal(validator)
                || iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop).is_err()
            {
                return Err(LaneAutonomousArtifactError::InvalidAvailabilityPop);
            }
        }
        Ok(())
    }
}

fn aggregate_lane_payload_availability_votes(
    body: LanePayloadAvailabilityBodyV1,
    validator_set: Vec<PeerId>,
    votes: &[LanePayloadAvailabilityVoteV1],
) -> Result<LanePayloadAvailabilityQcV1, LaneAutonomousArtifactError> {
    validate_lane_payload_availability_body_shape(&body)?;
    validate_lane_block_validator_set_fields(
        body.validator_set_hash_version,
        body.validator_set_hash,
        body.validator_count,
        body.min_quorum,
        &validator_set,
    )
    .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilityBody)?;
    if validator_set.len() > MAX_LANE_BLOCK_VALIDATORS {
        return Err(LaneAutonomousArtifactError::InvalidAvailabilityBody);
    }

    let mut indexed_signatures = BTreeMap::<usize, Vec<u8>>::new();
    let mut canonical_pops: Option<Vec<Vec<u8>>> = None;
    for vote in votes {
        if vote.body != body {
            return Err(LaneAutonomousArtifactError::AvailabilityBodyMismatch);
        }
        vote.validate_against_validator_set(&validator_set)?;
        let index = validator_set
            .iter()
            .position(|validator| validator == &vote.signer)
            .ok_or(LaneAutonomousArtifactError::AvailabilitySignerNotInCommittee)?;
        if indexed_signatures
            .insert(index, vote.bls_signature.clone())
            .is_some()
        {
            return Err(LaneAutonomousArtifactError::DuplicateAvailabilitySigner);
        }
        match &mut canonical_pops {
            Some(existing) if vote.validator_set_pops.as_slice() < existing.as_slice() => {
                *existing = vote.validator_set_pops.clone();
            }
            None => canonical_pops = Some(vote.validator_set_pops.clone()),
            Some(_) => {}
        }
    }
    if indexed_signatures.len()
        < usize::try_from(body.min_quorum)
            .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilityBody)?
    {
        return Err(LaneAutonomousArtifactError::AvailabilityQuorumNotMet);
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
        .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilityAggregate)?;
    let qc = LanePayloadAvailabilityQcV1 {
        body,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set,
        validator_set_pops: canonical_pops
            .ok_or(LaneAutonomousArtifactError::AvailabilityQuorumNotMet)?,
        signers_bitmap,
        bls_aggregate_signature,
    };
    validate_lane_payload_availability_qc(&qc)?;
    Ok(qc)
}

/// Validate a self-contained exact-payload READY quorum certificate.
pub(crate) fn validate_lane_payload_availability_qc(
    qc: &LanePayloadAvailabilityQcV1,
) -> Result<(), LaneAutonomousArtifactError> {
    validate_lane_payload_availability_body_shape(&qc.body)?;
    if qc.validator_set.len() > MAX_LANE_BLOCK_VALIDATORS
        || qc.validator_set_hash_version != qc.body.validator_set_hash_version
        || qc.validator_set_hash != qc.body.validator_set_hash
    {
        return Err(LaneAutonomousArtifactError::InvalidAvailabilityBody);
    }
    validate_lane_block_validator_set_fields(
        qc.validator_set_hash_version,
        qc.validator_set_hash,
        qc.body.validator_count,
        qc.body.min_quorum,
        &qc.validator_set,
    )
    .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilityBody)?;
    if qc.validator_set_pops.len() != qc.validator_set.len()
        || qc
            .validator_set_pops
            .iter()
            .any(|pop| pop.len() != LANE_BLS_PROOF_BYTES)
    {
        return Err(LaneAutonomousArtifactError::InvalidAvailabilityPop);
    }
    let expected_bitmap_len = qc.validator_set.len().div_ceil(8);
    if qc.signers_bitmap.len() != expected_bitmap_len
        || qc.bls_aggregate_signature.len() != LANE_BLS_PROOF_BYTES
    {
        return Err(LaneAutonomousArtifactError::InvalidAvailabilityBitmap);
    }
    if norito::encode_canonical(qc).map_or(true, |encoded| {
        encoded.len() > MAX_LANE_PAYLOAD_AVAILABILITY_QC_BYTES
    }) {
        return Err(LaneAutonomousArtifactError::InvalidAvailabilityBody);
    }
    if let Some(last) = qc.signers_bitmap.last().copied() {
        let used_bits = qc.validator_set.len() % 8;
        if used_bits != 0 && last & !((1_u8 << used_bits) - 1) != 0 {
            return Err(LaneAutonomousArtifactError::InvalidAvailabilityBitmap);
        }
    }

    let mut signer_count = 0_usize;
    let mut public_keys = Vec::<&PublicKey>::new();
    let mut pop_refs = Vec::<&[u8]>::new();
    for (index, (validator, pop)) in qc
        .validator_set
        .iter()
        .zip(&qc.validator_set_pops)
        .enumerate()
    {
        if !peer_uses_bls_normal(validator)
            || iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop).is_err()
        {
            return Err(LaneAutonomousArtifactError::InvalidAvailabilityPop);
        }
        if qc.signers_bitmap[index / 8] & (1_u8 << (index % 8)) != 0 {
            signer_count = signer_count.saturating_add(1);
            public_keys.push(validator.public_key());
            pop_refs.push(pop.as_slice());
        }
    }
    if signer_count
        < usize::try_from(qc.body.min_quorum)
            .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilityBody)?
    {
        return Err(LaneAutonomousArtifactError::AvailabilityQuorumNotMet);
    }
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &qc.body.signature_preimage(),
        &qc.bls_aggregate_signature,
        &public_keys,
        &pop_refs,
    )
    .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilityAggregate)
}

pub(crate) fn compute_lane_executable_payload_hash(
    version: u8,
    network_id: NetworkId,
    epoch: u64,
    origin_proposal: &LaneBlockProposalV1,
    entrypoints: &[TransactionEntrypoint],
    reservation_keys: &[LaneQueueReservationKeyV2],
    routing_plans: &[RoutingPlan],
    native_amx_receipts: &[Option<NativeAmxReceipt>],
) -> Result<Hash, LaneAutonomousArtifactError> {
    if version != LANE_EXECUTABLE_PAYLOAD_VERSION_V2 {
        return Err(LaneAutonomousArtifactError::UnsupportedVersion);
    }
    let descriptor = &origin_proposal.descriptor;
    let preimage = LaneExecutablePayloadPreimage {
        purpose: "nexus:lane-executable-payload:v2".to_owned(),
        version,
        network_id,
        epoch,
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
        proposal_height: descriptor.proposal_height,
        previous_lane_block_height: descriptor.previous_lane_block_height,
        previous_lane_block_descriptor_hash: descriptor.previous_lane_block_descriptor_hash,
        lane_block_height: descriptor.lane_block_height,
        accepted_candidate_indices: descriptor.accepted_candidate_indices.clone(),
        accepted_transaction_hashes: descriptor.accepted_transaction_hashes.clone(),
        validator_set_hash_version: descriptor.validator_set_hash_version,
        validator_set_hash: descriptor.validator_set_hash,
        validator_set: descriptor.validator_set.clone(),
        validator_count: descriptor.validator_count,
        min_quorum: descriptor.min_quorum,
        qc_mode_tag: descriptor.qc_mode_tag.clone(),
        entrypoints: entrypoints.to_vec(),
        reservation_keys: reservation_keys.to_vec(),
        routing_plans: routing_plans.to_vec(),
        native_amx_receipts: native_amx_receipts.to_vec(),
    };
    let bytes = norito::encode_canonical(&preimage)
        .map_err(|_| LaneAutonomousArtifactError::PayloadHashMismatch)?;
    Ok(Hash::new(bytes))
}

#[allow(clippy::too_many_arguments)]
fn validate_lane_executable_payload_body(
    version: u8,
    network_id: NetworkId,
    epoch: u64,
    origin_proposal: &LaneBlockProposalV1,
    entrypoint_hashes: &[Hash],
    entrypoints: &[TransactionEntrypoint],
    reservation_keys: &[LaneQueueReservationKeyV2],
    routing_plans: &[RoutingPlan],
    native_amx_receipts: &[Option<NativeAmxReceipt>],
    payload_hash: Hash,
    expected_network_id: NetworkId,
    expected_epoch: u64,
) -> Result<(), LaneAutonomousArtifactError> {
    if version != LANE_EXECUTABLE_PAYLOAD_VERSION_V2 {
        return Err(LaneAutonomousArtifactError::UnsupportedVersion);
    }
    if network_id != expected_network_id || epoch != expected_epoch {
        return Err(LaneAutonomousArtifactError::NetworkOrEpochMismatch);
    }
    validate_lane_block_proposal(origin_proposal)
        .map_err(|_| LaneAutonomousArtifactError::InvalidProposal)?;
    if origin_proposal
        .payload_block_hint
        .is_some_and(|hint| hint.proposal_height != origin_proposal.descriptor.proposal_height)
    {
        return Err(LaneAutonomousArtifactError::InvalidProposal);
    }
    if entrypoints.is_empty()
        || entrypoints.len() > MAX_LANE_EXECUTABLE_ENTRYPOINTS
        || entrypoint_hashes.len() > MAX_LANE_EXECUTABLE_ENTRYPOINTS
    {
        return Err(LaneAutonomousArtifactError::EntrypointLimitExceeded);
    }
    let encoded_payload_body_len = Encode::encode(&(
        entrypoints.to_vec(),
        reservation_keys.to_vec(),
        routing_plans.to_vec(),
        native_amx_receipts.to_vec(),
    ))
    .len();
    if !lane_executable_payload_body_within_limit(encoded_payload_body_len) {
        return Err(LaneAutonomousArtifactError::PayloadByteLimitExceeded);
    }
    let actual_hashes = entrypoints
        .iter()
        .map(|entrypoint| Hash::from(entrypoint.hash()))
        .collect::<Vec<_>>();
    if actual_hashes.iter().copied().collect::<BTreeSet<_>>().len() != actual_hashes.len()
        || entrypoint_hashes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != entrypoint_hashes.len()
    {
        return Err(LaneAutonomousArtifactError::EntrypointMismatch);
    }
    if actual_hashes != entrypoint_hashes
        || entrypoint_hashes != origin_proposal.descriptor.accepted_transaction_hashes
    {
        return Err(LaneAutonomousArtifactError::EntrypointMismatch);
    }
    let descriptor = &origin_proposal.descriptor;
    if reservation_keys.len() != entrypoints.len() {
        return Err(LaneAutonomousArtifactError::ReservationMismatch);
    }
    if routing_plans.len() != entrypoints.len() {
        return Err(LaneAutonomousArtifactError::RoutingPlanMismatch);
    }
    if native_amx_receipts.len() != entrypoints.len() {
        return Err(LaneAutonomousArtifactError::NativeAmxReceiptMismatch);
    }
    let mut reservation_digests = BTreeSet::new();
    let mut signed_transaction_hashes = BTreeSet::new();
    for ((((entrypoint, entrypoint_hash), key), routing_plan), native_amx_receipt) in entrypoints
        .iter()
        .zip(entrypoint_hashes)
        .zip(reservation_keys)
        .zip(routing_plans)
        .zip(native_amx_receipts)
    {
        let accepted = AcceptedTransaction::new_unchecked_entrypoint(std::borrow::Cow::Owned(
            entrypoint.clone(),
        ));
        if key.validate().is_err()
            || key.signed_transaction_hash != accepted.hash()
            || Hash::from(key.entrypoint_hash) != *entrypoint_hash
            || key.lane_id != descriptor.lane_id
            || key.dataspace_id != descriptor.dataspace_id
            || key.lane_incarnation != descriptor.lane_incarnation
            || key.proposal_height != descriptor.proposal_height
            || key.lane_block_height != descriptor.lane_block_height
            || key.lane_block_view != descriptor.lane_block_view
            || protocol_hash_bytes_are_zero(key.reservation_owner_hash.as_ref())
            || protocol_hash_bytes_are_zero(key.proposal_identity_hash.as_ref())
            || key.coordinator_leg.role != RouteLegRole::Coordinator
            || key.coordinator_leg.route.lane_id != descriptor.lane_id
            || key.coordinator_leg.route.dataspace_id != descriptor.dataspace_id
            || !reservation_digests.insert(key.digest())
            || !signed_transaction_hashes.insert(key.signed_transaction_hash)
        {
            return Err(LaneAutonomousArtifactError::ReservationMismatch);
        }
        if routing_plan.digest() != key.routing_plan_digest
            || routing_plan.coordinator_leg() != key.coordinator_leg
        {
            return Err(LaneAutonomousArtifactError::RoutingPlanMismatch);
        }
        if !crate::native_amx::receipt_shape_matches_coordinator_payload(
            native_amx_receipt.as_ref(),
            routing_plan,
            accepted.hash().as_ref(),
            *entrypoint_hash,
            network_id,
            origin_proposal,
        ) {
            return Err(LaneAutonomousArtifactError::NativeAmxReceiptMismatch);
        }
    }
    if compute_lane_executable_payload_hash(
        version,
        network_id,
        epoch,
        origin_proposal,
        entrypoints,
        reservation_keys,
        routing_plans,
        native_amx_receipts,
    )? != payload_hash
    {
        return Err(LaneAutonomousArtifactError::PayloadHashMismatch);
    }
    Ok(())
}

const fn lane_executable_payload_body_within_limit(encoded_len: usize) -> bool {
    encoded_len <= MAX_LANE_EXECUTABLE_PAYLOAD_BYTES
}

/// Derive the only valid next-view cursor for the exact same lane payload.
pub(crate) fn retarget_lane_block_proposal_view(
    source: &LaneBlockProposalV1,
    target_view: u64,
) -> Result<LaneBlockProposalV1, LaneAutonomousArtifactError> {
    validate_lane_block_proposal(source)
        .map_err(|_| LaneAutonomousArtifactError::InvalidProposal)?;
    if source.descriptor.lane_block_view.checked_add(1) != Some(target_view) {
        return Err(if source.descriptor.lane_block_view == u64::MAX {
            LaneAutonomousArtifactError::NewViewOverflow
        } else {
            LaneAutonomousArtifactError::InvalidNewViewBody
        });
    }
    retarget_lane_block_proposal_exact_view(source, target_view)
}

/// Derive the canonical synthetic cursor representation for an exact autonomous view.
///
/// This helper intentionally does not authorize a view jump. It is used only
/// to revalidate a quorum-signed checkpoint after the earlier, already
/// validated certificate chain has been compacted. Live transitions must use
/// [`retarget_lane_block_proposal_view`], which enforces `source + 1`.
pub(crate) fn retarget_lane_block_proposal_exact_view(
    source: &LaneBlockProposalV1,
    target_view: u64,
) -> Result<LaneBlockProposalV1, LaneAutonomousArtifactError> {
    validate_lane_block_proposal(source)
        .map_err(|_| LaneAutonomousArtifactError::InvalidProposal)?;
    let source_descriptor = &source.descriptor;
    let subject_hash = SumeragiLanePayloadOwnership::compute_replay_subject_hash(
        source_descriptor.lane_id,
        source_descriptor.dataspace_id,
        source_descriptor.lane_incarnation,
        source_descriptor.lane_block_height,
        target_view,
        &source_descriptor.accepted_candidate_indices,
        &source_descriptor.accepted_transaction_hashes,
        &source_descriptor.qc_mode_tag,
    )
    .map_err(|_| LaneAutonomousArtifactError::NewViewTargetMismatch)?;
    let payload_ownership_hash =
        SumeragiLanePayloadOwnership::compute_replay_payload_ownership_hash(
            source_descriptor.lane_id,
            source_descriptor.dataspace_id,
            source_descriptor.lane_incarnation,
            source_descriptor.lane_block_height,
            target_view,
            subject_hash,
            &source_descriptor.accepted_candidate_indices,
            &source_descriptor.accepted_transaction_hashes,
            &source_descriptor.qc_mode_tag,
        )
        .map_err(|_| LaneAutonomousArtifactError::NewViewTargetMismatch)?;
    let rbc_instance_hash = SumeragiLanePayloadOwnership::compute_replay_rbc_instance_hash(
        source_descriptor.lane_id,
        source_descriptor.dataspace_id,
        source_descriptor.lane_incarnation,
        source_descriptor.lane_block_height,
        target_view,
        subject_hash,
        payload_ownership_hash,
    )
    .map_err(|_| LaneAutonomousArtifactError::NewViewTargetMismatch)?;

    let mut descriptor = source_descriptor.clone();
    descriptor.lane_block_view = target_view;
    descriptor.subject_hash = subject_hash;
    descriptor.payload_ownership_hash = payload_ownership_hash;
    descriptor.rbc_instance_hash = rbc_instance_hash;
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        // The original global-block hint belongs only to the immutable
        // certification proposal, not this synthetic transport cursor.
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    Ok(proposal)
}

impl LaneBlockNewViewBodyV1 {
    /// Build the exact next-view body for a synthetic cursor and executable payload.
    pub(crate) fn for_transition(
        source: &LaneBlockProposalV1,
        executable_payload: &LaneExecutablePayloadV1,
        target_view: u64,
        expected_network_id: NetworkId,
        expected_epoch: u64,
    ) -> Result<Self, LaneAutonomousArtifactError> {
        executable_payload.validate(expected_network_id, expected_epoch)?;
        if !executable_payload.matches_proposal_static(source) {
            return Err(LaneAutonomousArtifactError::NewViewSourceMismatch);
        }
        retarget_lane_block_proposal_view(source, target_view)?;
        let descriptor = &source.descriptor;
        Ok(Self {
            version: 1,
            network_id: expected_network_id,
            epoch: expected_epoch,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            proposal_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            from_view: descriptor.lane_block_view,
            target_view,
            locked_proposal_hash: source.proposal_hash,
            locked_descriptor_hash: descriptor.descriptor_hash,
            executable_payload_hash: executable_payload.payload_hash,
            validator_set_hash_version: descriptor.validator_set_hash_version,
            validator_set_hash: descriptor.validator_set_hash,
            validator_count: descriptor.validator_count,
            min_quorum: descriptor.min_quorum,
            qc_mode_tag: descriptor.qc_mode_tag.clone(),
        })
    }
}

fn validate_lane_block_new_view_body(
    body: &LaneBlockNewViewBodyV1,
) -> Result<(), LaneAutonomousArtifactError> {
    if body.version != 1 {
        return Err(LaneAutonomousArtifactError::UnsupportedVersion);
    }
    let validator_count = usize::try_from(body.validator_count)
        .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewBody)?;
    if body.proposal_height == 0
        || body.lane_block_height == 0
        || protocol_hash_bytes_are_zero(body.lane_incarnation.as_ref())
        || body.qc_mode_tag.trim().is_empty()
        || body.qc_mode_tag.len() > MAX_LANE_NEW_VIEW_QC_MODE_TAG_BYTES
        || body.from_view.checked_add(1) != Some(body.target_view)
        || body.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
        || body.validator_count == 0
        || validator_count > MAX_LANE_BLOCK_VALIDATORS
        || body.min_quorum == 0
        || body.min_quorum > body.validator_count
        || usize::try_from(body.min_quorum).ok()
            != Some(crate::sumeragi::network_topology::commit_quorum_from_len(
                validator_count,
            ))
    {
        return Err(LaneAutonomousArtifactError::InvalidNewViewBody);
    }
    Ok(())
}

impl LaneBlockNewViewVoteV1 {
    /// Sign a canonical NewView body with a lane committee key.
    pub(crate) fn new_signed(
        body: LaneBlockNewViewBodyV1,
        signer: PeerId,
        private_key: &PrivateKey,
    ) -> Result<Self, LaneAutonomousArtifactError> {
        validate_lane_block_new_view_body(&body)?;
        if !peer_uses_bls_normal(&signer) {
            return Err(LaneAutonomousArtifactError::ProducerNotBlsNormal);
        }
        let preimage = body.signature_preimage()?;
        let bls_signature = Signature::try_new(private_key, &preimage)
            .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewSignature)?
            .payload()
            .to_vec();
        let vote = Self {
            body,
            signer,
            bls_signature,
        };
        vote.validate_ingress()?;
        Ok(vote)
    }

    /// Verify shape, signer algorithm, and individual BLS signature.
    pub(crate) fn validate_ingress(&self) -> Result<(), LaneAutonomousArtifactError> {
        validate_lane_block_new_view_body(&self.body)?;
        if !peer_uses_bls_normal(&self.signer) {
            return Err(LaneAutonomousArtifactError::ProducerNotBlsNormal);
        }
        let preimage = self.body.signature_preimage()?;
        Signature::try_from_bytes(&self.bls_signature)
            .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewSignature)?
            .verify(self.signer.public_key(), &preimage)
            .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewSignature)
    }
}

fn protocol_hash_bytes_are_zero(hash: &[u8]) -> bool {
    // These records reserve `Hash::prehashed([0; 32])` as a sentinel, but
    // `Hash` forces its marker bit on construction. A bytewise all-zero check
    // therefore can never recognize the sentinel.
    hash == Hash::prehashed([0; Hash::LENGTH]).as_ref()
}

fn hash_is_nonzero(hash: Hash) -> bool {
    !protocol_hash_bytes_are_zero(hash.as_ref())
}

fn lane_drain_frontier_shape_is_valid(
    frontier: &LaneDrainFrontierV1,
) -> Result<(), LaneDrainCertificateError> {
    if frontier.version != LaneDrainFrontierV1::VERSION {
        return Err(LaneDrainCertificateError::UnsupportedVersion);
    }
    if !hash_is_nonzero(frontier.lane_incarnation)
        || (frontier.lane_block_height == 0) != frontier.lane_block_descriptor_hash.is_none()
        || frontier
            .lane_block_descriptor_hash
            .is_some_and(|hash| !hash_is_nonzero(hash))
    {
        return Err(LaneDrainCertificateError::FrontierHashMismatch);
    }
    if frontier.unresolved_evidence_root != lane_drain_empty_unresolved_evidence_root() {
        return Err(LaneDrainCertificateError::UnresolvedEvidence);
    }
    let Some(native) = frontier.native_application else {
        return Ok(());
    };
    let typed_hash_is_nonzero = |hash: &[u8]| !protocol_hash_bytes_are_zero(hash);
    if native.version != 1
        || frontier.lane_block_height == 0
        || native.predecessor_height.checked_add(1) != Some(frontier.lane_block_height)
        || (native.predecessor_height == 0) != native.predecessor_descriptor_hash.is_none()
        || native
            .predecessor_descriptor_hash
            .is_some_and(|hash| !hash_is_nonzero(hash))
        || !hash_is_nonzero(native.participant_proposal_hash)
        || !typed_hash_is_nonzero(native.participant_settlement_hash.as_ref())
        || native.source_count == 0
        || usize::try_from(native.source_count)
            .map_or(true, |count| count > MAX_MERGE_EXECUTION_ENTRYPOINTS)
        || native.application_block_height == 0
        || !typed_hash_is_nonzero(native.application_block_hash.as_ref())
        || !hash_is_nonzero(native.executed_block_wire_hash)
        || !typed_hash_is_nonzero(native.finality_artifact_hash.as_ref())
        || !hash_is_nonzero(native.application_manifest_root)
        || native.application_manifest_leaf_count == 0
        || native.application_manifest_leaf_index >= native.application_manifest_leaf_count
        || !hash_is_nonzero(native.manifest_artifact_hash)
        || !hash_is_nonzero(native.receipt_artifact_hash)
        || !hash_is_nonzero(native.latest_index_artifact_hash)
    {
        return Err(LaneDrainCertificateError::InvalidFrontierEvidence);
    }
    Ok(())
}

/// Validate one evidence-aware drain frontier independently of mutable state.
pub(crate) fn validate_lane_drain_frontier(
    frontier: &LaneDrainFrontierV1,
) -> Result<(), LaneDrainCertificateError> {
    lane_drain_frontier_shape_is_valid(frontier)
}

/// Validate a canonical drain intent independently of mutable runtime state.
pub(crate) fn validate_lane_drain_intent(
    intent: &LaneDrainIntentV1,
) -> Result<(), LaneDrainCertificateError> {
    let validator_count = usize::try_from(intent.validator_count)
        .map_err(|_| LaneDrainCertificateError::InvalidIntent)?;
    if intent.version != 1 {
        return Err(LaneDrainCertificateError::UnsupportedVersion);
    }
    if intent.close_global_height == 0
        || protocol_hash_bytes_are_zero(intent.lane_incarnation.as_ref())
        || intent.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
        || validator_count == 0
        || validator_count > MAX_LANE_BLOCK_VALIDATORS
        || intent.validator_set.len() != validator_count
        || intent.validator_set_hash != HashOf::new(&intent.validator_set)
        || intent
            .validator_set
            .iter()
            .any(|peer| !peer_uses_bls_normal(peer))
        || intent
            .validator_set
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || intent.min_quorum == 0
        || intent.min_quorum > intent.validator_count
        || usize::try_from(intent.min_quorum).ok()
            != Some(crate::sumeragi::network_topology::commit_quorum_from_len(
                validator_count,
            ))
    {
        return Err(LaneDrainCertificateError::InvalidIntent);
    }
    lane_drain_frontier_shape_is_valid(&intent.initial_frontier)?;
    if !intent.initial_frontier.matches_route(
        intent.lane_id,
        intent.dataspace_id,
        intent.lane_incarnation,
    ) {
        return Err(LaneDrainCertificateError::FrontierRouteMismatch);
    }
    Ok(())
}

/// Validate a drain certificate body independently of signatures and mutable
/// lane lifecycle state.
pub(crate) fn validate_lane_drain_certificate_body(
    body: &LaneDrainCertificateBodyV1,
) -> Result<(), LaneDrainCertificateError> {
    if body.version != 1 {
        return Err(LaneDrainCertificateError::UnsupportedVersion);
    }
    validate_lane_drain_intent(&body.intent)?;
    if body.final_frontier.lane_block_height < body.intent.initial_frontier.lane_block_height {
        return Err(LaneDrainCertificateError::FrontierRegression);
    }
    lane_drain_frontier_shape_is_valid(&body.final_frontier)?;
    if !body.final_frontier.matches_route(
        body.intent.lane_id,
        body.intent.dataspace_id,
        body.intent.lane_incarnation,
    ) {
        return Err(LaneDrainCertificateError::FrontierRouteMismatch);
    }
    if body.final_frontier.lane_block_height == body.intent.initial_frontier.lane_block_height
        && body.final_frontier != body.intent.initial_frontier
    {
        return Err(LaneDrainCertificateError::InvalidFrontierEvidence);
    }
    Ok(())
}

impl LaneDrainVoteV1 {
    /// Sign one exact drain frontier with a lane committee key.
    pub(crate) fn new_signed(
        body: LaneDrainCertificateBodyV1,
        signer: PeerId,
        private_key: &PrivateKey,
    ) -> Result<Self, LaneDrainCertificateError> {
        validate_lane_drain_certificate_body(&body)?;
        if !peer_uses_bls_normal(&signer) {
            return Err(LaneDrainCertificateError::InvalidVoteSignature);
        }
        if !body.intent.validator_set.contains(&signer) {
            return Err(LaneDrainCertificateError::SignerNotInCommittee);
        }
        let bls_signature = Signature::try_new(private_key, &body.signature_preimage())
            .map_err(|_| LaneDrainCertificateError::InvalidVoteSignature)?
            .payload()
            .to_vec();
        let proof_of_possession = iroha_crypto::bls_normal_pop_prove(private_key)
            .map_err(|_| LaneDrainCertificateError::InvalidProofOfPossession)?;
        let vote = Self {
            body,
            signer,
            proof_of_possession,
            bls_signature,
        };
        vote.validate_ingress()?;
        Ok(vote)
    }

    /// Verify shape, signer algorithm, and the individual BLS signature.
    pub(crate) fn validate_ingress(&self) -> Result<(), LaneDrainCertificateError> {
        validate_lane_drain_certificate_body(&self.body)?;
        if !self.body.intent.validator_set.contains(&self.signer) {
            return Err(LaneDrainCertificateError::SignerNotInCommittee);
        }
        if norito::encode_canonical(self)
            .map_or(true, |encoded| encoded.len() > MAX_LANE_DRAIN_VOTE_BYTES)
        {
            return Err(LaneDrainCertificateError::VoteTooLarge);
        }
        if self.bls_signature.len() != LANE_BLS_PROOF_BYTES
            || self.proof_of_possession.len() != LANE_BLS_PROOF_BYTES
            || !peer_uses_bls_normal(&self.signer)
        {
            return Err(LaneDrainCertificateError::InvalidVoteSignature);
        }
        iroha_crypto::bls_normal_pop_verify(self.signer.public_key(), &self.proof_of_possession)
            .map_err(|_| LaneDrainCertificateError::InvalidProofOfPossession)?;
        Signature::try_from_bytes(&self.bls_signature)
            .map_err(|_| LaneDrainCertificateError::InvalidVoteSignature)?
            .verify(self.signer.public_key(), &self.body.signature_preimage())
            .map_err(|_| LaneDrainCertificateError::InvalidVoteSignature)
    }
}

/// Aggregate distinct valid drain votes into a restart-verifiable certificate.
pub(crate) fn aggregate_lane_drain_votes(
    body: LaneDrainCertificateBodyV1,
    validator_set: Vec<PeerId>,
    votes: &[LaneDrainVoteV1],
) -> Result<LaneDrainCertificateV1, LaneDrainCertificateError> {
    validate_lane_drain_certificate_body(&body)?;
    validate_lane_block_validator_set_fields(
        body.intent.validator_set_hash_version,
        body.intent.validator_set_hash,
        body.intent.validator_count,
        body.intent.min_quorum,
        &validator_set,
    )
    .map_err(|_| LaneDrainCertificateError::InvalidValidatorSet)?;
    if validator_set.as_slice() != body.intent.validator_set.as_slice() {
        return Err(LaneDrainCertificateError::InvalidValidatorSet);
    }

    let mut signatures = BTreeMap::<usize, (Vec<u8>, Vec<u8>)>::new();
    for vote in votes {
        if vote.body != body {
            return Err(LaneDrainCertificateError::BodyMismatch);
        }
        vote.validate_ingress()?;
        let index = validator_set
            .iter()
            .position(|validator| validator == &vote.signer)
            .ok_or(LaneDrainCertificateError::SignerNotInCommittee)?;
        if signatures
            .insert(
                index,
                (vote.bls_signature.clone(), vote.proof_of_possession.clone()),
            )
            .is_some()
        {
            return Err(LaneDrainCertificateError::DuplicateSigner);
        }
    }
    if signatures.len()
        < usize::try_from(body.intent.min_quorum)
            .map_err(|_| LaneDrainCertificateError::InvalidIntent)?
    {
        return Err(LaneDrainCertificateError::QuorumNotMet);
    }

    let mut signers_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
    let mut signer_proofs = Vec::with_capacity(signatures.len());
    let ordered_signatures = signatures
        .into_iter()
        .map(|(index, (signature, proof_of_possession))| {
            signers_bitmap[index / 8] |= 1_u8 << (index % 8);
            signer_proofs.push(MergeSignerProof {
                signer: u32::try_from(index)
                    .map_err(|_| LaneDrainCertificateError::InvalidBitmap)?,
                proof_of_possession,
            });
            Ok(signature)
        })
        .collect::<Result<Vec<_>, LaneDrainCertificateError>>()?;
    let signature_refs = ordered_signatures
        .iter()
        .map(Vec::as_slice)
        .collect::<Vec<_>>();
    let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .map_err(|_| LaneDrainCertificateError::InvalidAggregateSignature)?;
    let certificate = LaneDrainCertificateV1 {
        body,
        validator_set,
        signers_bitmap,
        signer_proofs,
        aggregate_signature,
    };
    validate_lane_drain_certificate(&certificate)?;
    Ok(certificate)
}

/// Validate a self-contained drain certificate, including signer PoPs and the
/// aggregate BLS-normal signature.
pub(crate) fn validate_lane_drain_certificate(
    certificate: &LaneDrainCertificateV1,
) -> Result<(), LaneDrainCertificateError> {
    let body = &certificate.body;
    validate_lane_drain_certificate_body(body)?;
    validate_lane_block_validator_set_fields(
        body.intent.validator_set_hash_version,
        body.intent.validator_set_hash,
        body.intent.validator_count,
        body.intent.min_quorum,
        &certificate.validator_set,
    )
    .map_err(|_| LaneDrainCertificateError::InvalidValidatorSet)?;
    if certificate.validator_set.as_slice() != body.intent.validator_set.as_slice() {
        return Err(LaneDrainCertificateError::InvalidValidatorSet);
    }
    if norito::encode_canonical(certificate).map_or(true, |encoded| {
        encoded.len() > MAX_LANE_DRAIN_CERTIFICATE_BYTES
    }) {
        return Err(LaneDrainCertificateError::CertificateTooLarge);
    }
    let expected_bitmap_len = certificate.validator_set.len().div_ceil(8);
    if certificate.signers_bitmap.len() != expected_bitmap_len
        || certificate.aggregate_signature.len() != LANE_BLS_PROOF_BYTES
    {
        return Err(LaneDrainCertificateError::InvalidBitmap);
    }
    if let Some(last) = certificate.signers_bitmap.last().copied() {
        let used_bits = certificate.validator_set.len() % 8;
        if used_bits != 0 && last & !((1_u8 << used_bits) - 1) != 0 {
            return Err(LaneDrainCertificateError::InvalidBitmap);
        }
    }

    let mut selected_indices = Vec::new();
    for (byte_index, byte) in certificate.signers_bitmap.iter().copied().enumerate() {
        for bit in 0..8 {
            if byte & (1_u8 << bit) == 0 {
                continue;
            }
            let index = byte_index * 8 + bit;
            if index >= certificate.validator_set.len() {
                return Err(LaneDrainCertificateError::InvalidBitmap);
            }
            selected_indices.push(index);
        }
    }
    if selected_indices.len()
        < usize::try_from(body.intent.min_quorum)
            .map_err(|_| LaneDrainCertificateError::InvalidIntent)?
        || certificate.signer_proofs.len() != selected_indices.len()
    {
        return Err(LaneDrainCertificateError::QuorumNotMet);
    }

    let mut public_keys = Vec::with_capacity(selected_indices.len());
    let mut pop_refs = Vec::with_capacity(selected_indices.len());
    for (index, proof) in selected_indices.iter().zip(&certificate.signer_proofs) {
        let expected_index =
            u32::try_from(*index).map_err(|_| LaneDrainCertificateError::InvalidBitmap)?;
        if proof.signer != expected_index || proof.proof_of_possession.len() != LANE_BLS_PROOF_BYTES
        {
            return Err(LaneDrainCertificateError::InvalidProofOfPossession);
        }
        let validator = certificate
            .validator_set
            .get(*index)
            .ok_or(LaneDrainCertificateError::InvalidBitmap)?;
        if !peer_uses_bls_normal(validator)
            || iroha_crypto::bls_normal_pop_verify(
                validator.public_key(),
                &proof.proof_of_possession,
            )
            .is_err()
        {
            return Err(LaneDrainCertificateError::InvalidProofOfPossession);
        }
        public_keys.push(validator.public_key());
        pop_refs.push(proof.proof_of_possession.as_slice());
    }
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &body.signature_preimage(),
        &certificate.aggregate_signature,
        &public_keys,
        &pop_refs,
    )
    .map_err(|_| LaneDrainCertificateError::InvalidAggregateSignature)
}

/// Aggregate distinct, individually valid NewView votes in canonical committee order.
pub(crate) fn aggregate_lane_block_new_view_votes(
    body: LaneBlockNewViewBodyV1,
    validator_set: Vec<PeerId>,
    votes: &[LaneBlockNewViewVoteV1],
) -> Result<LaneBlockNewViewCertificateV1, LaneAutonomousArtifactError> {
    validate_lane_block_new_view_body(&body)?;
    validate_lane_block_validator_set_fields(
        body.validator_set_hash_version,
        body.validator_set_hash,
        body.validator_count,
        body.min_quorum,
        &validator_set,
    )
    .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewBody)?;
    let mut signatures = BTreeMap::<usize, Vec<u8>>::new();
    for vote in votes {
        if vote.body != body {
            return Err(LaneAutonomousArtifactError::NewViewBodyMismatch);
        }
        vote.validate_ingress()?;
        let index = validator_set
            .iter()
            .position(|validator| validator == &vote.signer)
            .ok_or(LaneAutonomousArtifactError::NewViewSignerNotInCommittee)?;
        if signatures
            .insert(index, vote.bls_signature.clone())
            .is_some()
        {
            return Err(LaneAutonomousArtifactError::DuplicateNewViewSigner);
        }
    }
    if signatures.len()
        < usize::try_from(body.min_quorum)
            .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewBody)?
    {
        return Err(LaneAutonomousArtifactError::NewViewQuorumNotMet);
    }
    let mut signers_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
    let ordered = signatures
        .into_iter()
        .map(|(index, signature)| {
            signers_bitmap[index / 8] |= 1_u8 << (index % 8);
            signature
        })
        .collect::<Vec<_>>();
    let refs = ordered.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let bls_aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&refs)
        .map_err(|_| LaneAutonomousArtifactError::NewViewAggregation)?;
    Ok(LaneBlockNewViewCertificateV1 {
        body,
        validator_set,
        signers_bitmap,
        bls_aggregate_signature,
    })
}

/// Validate a NewView certificate, including exact signer PoPs and aggregate signature.
pub(crate) fn validate_lane_block_new_view_certificate(
    certificate: &LaneBlockNewViewCertificateV1,
    signer_pops: &BTreeMap<PublicKey, Vec<u8>>,
) -> Result<(), LaneAutonomousArtifactError> {
    let body = &certificate.body;
    validate_lane_block_new_view_body(body)?;
    validate_lane_block_validator_set_fields(
        body.validator_set_hash_version,
        body.validator_set_hash,
        body.validator_count,
        body.min_quorum,
        &certificate.validator_set,
    )
    .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewBody)?;
    if certificate.signers_bitmap.len() != certificate.validator_set.len().div_ceil(8)
        || certificate.bls_aggregate_signature.is_empty()
    {
        return Err(LaneAutonomousArtifactError::InvalidNewViewBitmap);
    }
    let mut signer_count = 0_usize;
    let mut public_keys = Vec::<&PublicKey>::new();
    let mut pop_refs = Vec::<&[u8]>::new();
    let mut selected_keys = BTreeSet::new();
    for (byte_index, byte) in certificate.signers_bitmap.iter().copied().enumerate() {
        for bit in 0..8 {
            if byte & (1_u8 << bit) == 0 {
                continue;
            }
            let signer_index = byte_index * 8 + bit;
            let signer = certificate
                .validator_set
                .get(signer_index)
                .ok_or(LaneAutonomousArtifactError::InvalidNewViewBitmap)?;
            if !peer_uses_bls_normal(signer) {
                return Err(LaneAutonomousArtifactError::ProducerNotBlsNormal);
            }
            let public_key = signer.public_key();
            let pop = signer_pops
                .get(public_key)
                .ok_or(LaneAutonomousArtifactError::InvalidNewViewPop)?;
            iroha_crypto::bls_normal_pop_verify(public_key, pop)
                .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewPop)?;
            selected_keys.insert(public_key.clone());
            public_keys.push(public_key);
            pop_refs.push(pop.as_slice());
            signer_count = signer_count.saturating_add(1);
        }
    }
    if signer_count
        < usize::try_from(body.min_quorum)
            .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewBody)?
    {
        return Err(LaneAutonomousArtifactError::NewViewQuorumNotMet);
    }
    if selected_keys != signer_pops.keys().cloned().collect::<BTreeSet<_>>() {
        return Err(LaneAutonomousArtifactError::InvalidNewViewPop);
    }
    let preimage = body.signature_preimage()?;
    iroha_crypto::bls_normal_verify_preaggregated_same_message(
        &preimage,
        &certificate.bls_aggregate_signature,
        &public_keys,
        &pop_refs,
    )
    .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewAggregate)
}

/// Validate an origin-view availability DELIVER certificate against the exact
/// producer-authenticated executable payload.
///
/// Availability is retained across authenticated NewView transitions, but its
/// durable proof is immutable: both the outer Prepare QC and embedded READY QC
/// must certify [`LaneExecutablePayloadV1::origin_proposal`]. A Prepare QC for a
/// retargeted NewView cursor is rejected even when its signatures are valid.
pub(crate) fn validate_lane_payload_availability_certificate(
    durable: &DurableLanePayloadAvailabilityCertificateV1,
    executable_payload: &LaneExecutablePayloadV1,
    expected_network_id: NetworkId,
    expected_epoch: u64,
) -> Result<(), LaneAutonomousArtifactError> {
    executable_payload.validate(expected_network_id, expected_epoch)?;
    if durable.certificate.body.phase != CertPhase::Prepare {
        return Err(LaneAutonomousArtifactError::InvalidAvailabilityCertificate);
    }
    let availability_qc = durable
        .certificate
        .payload_availability_qc
        .as_ref()
        .ok_or(LaneAutonomousArtifactError::InvalidAvailabilityCertificate)?;
    let certified_proposal = &executable_payload.origin_proposal;
    if durable.certificate.body != certified_proposal.vote_body(CertPhase::Prepare)
        || durable.certificate.validator_set != certified_proposal.descriptor.validator_set
        || durable.certificate.signers_bitmap != availability_qc.signers_bitmap
    {
        return Err(LaneAutonomousArtifactError::AvailabilityMismatch);
    }
    validate_lane_payload_availability_body_against_payload(
        &availability_qc.body,
        executable_payload,
        certified_proposal,
        expected_network_id,
        expected_epoch,
    )?;
    validate_lane_payload_availability_qc(availability_qc)
        .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilityCertificate)?;
    let historical_pops = availability_qc
        .validator_set
        .iter()
        .cloned()
        .zip(availability_qc.validator_set_pops.iter().cloned())
        .map(|(peer, pop)| (peer.public_key().clone(), pop))
        .collect::<BTreeMap<_, _>>();
    validate_lane_block_qc_aggregate(&durable.certificate, &historical_pops)
        .map_err(|_| LaneAutonomousArtifactError::InvalidAvailabilityCertificate)?;
    validate_qc_matches_proposal(&durable.certificate, certified_proposal)
        .map_err(|_| LaneAutonomousArtifactError::AvailabilityMismatch)
}

/// Validate a complete cursor-payload-certificate-cursor transition.
pub(crate) fn validate_lane_block_new_view_transition(
    source: &LaneBlockProposalV1,
    target: &LaneBlockProposalV1,
    executable_payload: &LaneExecutablePayloadV1,
    durable_certificate: &DurableLaneBlockNewViewCertificateV1,
    expected_network_id: NetworkId,
    expected_epoch: u64,
) -> Result<(), LaneAutonomousArtifactError> {
    executable_payload.validate(expected_network_id, expected_epoch)?;
    validate_lane_block_new_view_certificate(
        &durable_certificate.certificate,
        &durable_certificate.signer_pops,
    )?;
    let expected_body = LaneBlockNewViewBodyV1::for_transition(
        source,
        executable_payload,
        target.descriptor.lane_block_view,
        expected_network_id,
        expected_epoch,
    )?;
    if durable_certificate.certificate.body != expected_body {
        return Err(LaneAutonomousArtifactError::NewViewSourceMismatch);
    }
    if retarget_lane_block_proposal_view(source, expected_body.target_view)? != *target {
        return Err(LaneAutonomousArtifactError::NewViewTargetMismatch);
    }
    Ok(())
}

/// Validate a compacted lane view checkpoint without consulting discarded
/// certificates or mutable committee state.
pub(crate) fn validate_lane_block_view_checkpoint(
    checkpoint: &DurableLaneBlockViewCheckpointV1,
    executable_payload: &LaneExecutablePayloadV1,
    expected_network_id: NetworkId,
    expected_epoch: u64,
) -> Result<(), LaneAutonomousArtifactError> {
    executable_payload.validate(expected_network_id, expected_epoch)?;
    let source_view = checkpoint.source_proposal.descriptor.lane_block_view;
    if source_view == 0 {
        return Err(LaneAutonomousArtifactError::InvalidNewViewBody);
    }
    let expected_source =
        retarget_lane_block_proposal_exact_view(&executable_payload.origin_proposal, source_view)?;
    if expected_source != checkpoint.source_proposal {
        return Err(LaneAutonomousArtifactError::NewViewSourceMismatch);
    }
    let target_view = source_view
        .checked_add(1)
        .ok_or(LaneAutonomousArtifactError::NewViewOverflow)?;
    let expected_target =
        retarget_lane_block_proposal_view(&checkpoint.source_proposal, target_view)?;
    if expected_target != checkpoint.target_proposal {
        return Err(LaneAutonomousArtifactError::NewViewTargetMismatch);
    }
    validate_lane_block_new_view_transition(
        &checkpoint.source_proposal,
        &checkpoint.target_proposal,
        executable_payload,
        &checkpoint.certificate,
        expected_network_id,
        expected_epoch,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LaneBlockNewViewSlotKey {
    network_id: NetworkId,
    epoch: u64,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    target_view: u64,
}

impl From<&LaneBlockNewViewBodyV1> for LaneBlockNewViewSlotKey {
    fn from(body: &LaneBlockNewViewBodyV1) -> Self {
        Self {
            network_id: body.network_id,
            epoch: body.epoch,
            lane_id: body.lane_id,
            dataspace_id: body.dataspace_id,
            lane_incarnation: body.lane_incarnation,
            lane_block_height: body.lane_block_height,
            target_view: body.target_view,
        }
    }
}

/// Result of inserting a pre-validated NewView certificate into the bounded cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LaneBlockNewViewCacheOutcome {
    /// Certificate body was newly retained.
    Inserted,
    /// The same certified body was already retained.
    Duplicate,
}

/// Bounded conflict detector for independently advancing lane views.
#[derive(Clone, Debug)]
pub(crate) struct LaneBlockNewViewCertificateCache {
    capacity: usize,
    certificates: BTreeMap<LaneBlockNewViewSlotKey, LaneBlockNewViewCertificateV1>,
    order: VecDeque<LaneBlockNewViewSlotKey>,
}

/// Bounded vote collector for lane-local NewView certificates.
#[derive(Clone, Debug)]
pub(crate) struct LaneBlockNewViewVoteCache {
    capacity: usize,
    votes: BTreeMap<
        LaneBlockNewViewSlotKey,
        (
            LaneBlockNewViewBodyV1,
            BTreeMap<PeerId, LaneBlockNewViewVoteV1>,
        ),
    >,
    order: VecDeque<LaneBlockNewViewSlotKey>,
}

impl LaneBlockNewViewVoteCache {
    /// Construct a bounded vote collector.
    #[must_use]
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            votes: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Retain only vote collections whose exact transition body survives.
    pub(crate) fn retain(&mut self, mut retain: impl FnMut(&LaneBlockNewViewBodyV1) -> bool) {
        self.votes.retain(|_, (body, _)| retain(body));
        let votes = &self.votes;
        self.order.retain(|key| votes.contains_key(key));
    }

    /// Insert a signed vote and return a certificate once quorum is reached.
    pub(crate) fn insert_and_maybe_seal(
        &mut self,
        vote: LaneBlockNewViewVoteV1,
        validator_set: &[PeerId],
    ) -> Result<
        (
            LaneBlockNewViewCacheOutcome,
            Option<LaneBlockNewViewCertificateV1>,
        ),
        LaneAutonomousArtifactError,
    > {
        vote.validate_ingress()?;
        validate_lane_block_validator_set_fields(
            vote.body.validator_set_hash_version,
            vote.body.validator_set_hash,
            vote.body.validator_count,
            vote.body.min_quorum,
            validator_set,
        )
        .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewBody)?;
        if !validator_set.contains(&vote.signer) {
            return Err(LaneAutonomousArtifactError::NewViewSignerNotInCommittee);
        }
        let key = LaneBlockNewViewSlotKey::from(&vote.body);
        if let Some((existing_body, existing_votes)) = self.votes.get(&key) {
            if existing_body != &vote.body {
                if existing_votes.contains_key(&vote.signer) {
                    return Err(LaneAutonomousArtifactError::NewViewBodyMismatch);
                }
                // One target slot cannot collect competing bodies in the same
                // bounded cache: retain the first observed body fail-closed.
                return Err(LaneAutonomousArtifactError::NewViewBodyMismatch);
            }
            if let Some(existing) = existing_votes.get(&vote.signer) {
                if existing == &vote {
                    return Ok((LaneBlockNewViewCacheOutcome::Duplicate, None));
                }
                return Err(LaneAutonomousArtifactError::InvalidNewViewSignature);
            }
        }

        if !self.votes.contains_key(&key) {
            self.votes.insert(key, (vote.body.clone(), BTreeMap::new()));
            self.order.push_back(key);
        }
        while self.votes.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.votes.remove(&oldest);
            }
        }
        let (body, votes) = self
            .votes
            .get_mut(&key)
            .expect("newest NewView vote slot survives bounded eviction");
        votes.insert(vote.signer.clone(), vote);
        let certificate = if votes.len()
            >= usize::try_from(body.min_quorum)
                .map_err(|_| LaneAutonomousArtifactError::InvalidNewViewBody)?
        {
            Some(aggregate_lane_block_new_view_votes(
                body.clone(),
                validator_set.to_vec(),
                &votes.values().cloned().collect::<Vec<_>>(),
            )?)
        } else {
            None
        };
        Ok((LaneBlockNewViewCacheOutcome::Inserted, certificate))
    }

    /// Return whether an identical vote is already retained.
    pub(crate) fn contains(&self, vote: &LaneBlockNewViewVoteV1) -> bool {
        self.votes
            .get(&LaneBlockNewViewSlotKey::from(&vote.body))
            .and_then(|(_, votes)| votes.get(&vote.signer))
            == Some(vote)
    }

    /// Snapshot retained votes produced by one local committee identity.
    pub(crate) fn votes_for_signer(&self, signer: &PeerId) -> Vec<LaneBlockNewViewVoteV1> {
        self.votes
            .values()
            .filter_map(|(_, votes)| votes.get(signer).cloned())
            .collect()
    }
}

impl LaneBlockNewViewCertificateCache {
    /// Construct a cache with a strict minimum capacity of one.
    #[must_use]
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            certificates: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Retain only certificates whose exact transition body survives.
    pub(crate) fn retain(&mut self, mut retain: impl FnMut(&LaneBlockNewViewBodyV1) -> bool) {
        self.certificates
            .retain(|_, certificate| retain(&certificate.body));
        let certificates = &self.certificates;
        self.order.retain(|key| certificates.contains_key(key));
    }

    /// Insert a certificate after full aggregate validation.
    pub(crate) fn insert(
        &mut self,
        certificate: LaneBlockNewViewCertificateV1,
        signer_pops: &BTreeMap<PublicKey, Vec<u8>>,
    ) -> Result<LaneBlockNewViewCacheOutcome, LaneAutonomousArtifactError> {
        validate_lane_block_new_view_certificate(&certificate, signer_pops)?;
        let key = LaneBlockNewViewSlotKey::from(&certificate.body);
        if let Some(existing) = self.certificates.get(&key) {
            if existing.body == certificate.body {
                return Ok(LaneBlockNewViewCacheOutcome::Duplicate);
            }
            return Err(LaneAutonomousArtifactError::NewViewBodyMismatch);
        }
        self.certificates.insert(key, certificate);
        self.order.push_back(key);
        while self.certificates.len() > self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.certificates.remove(&oldest);
            }
        }
        Ok(LaneBlockNewViewCacheOutcome::Inserted)
    }

    /// Return whether the exact aggregate certificate is already retained.
    pub(crate) fn contains(&self, certificate: &LaneBlockNewViewCertificateV1) -> bool {
        self.certificates
            .get(&LaneBlockNewViewSlotKey::from(&certificate.body))
            == Some(certificate)
    }
}

/// Individual lane-local block vote before committee aggregation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, JsonSerialize)]
pub struct LaneBlockVoteV1 {
    /// Body signed by the lane validator.
    pub body: LaneBlockVoteBodyV1,
    /// Separate exact-payload READY vote paired with autonomous prepares.
    pub payload_availability_vote: Option<LanePayloadAvailabilityVoteV1>,
    /// Validator that produced the vote.
    pub signer: PeerId,
    /// BLS signature over [`LaneBlockVoteBodyV1::signature_preimage`].
    pub bls_signature: Vec<u8>,
}

struct RequiredLanePayloadAvailabilityVote(Option<LanePayloadAvailabilityVoteV1>);

impl norito::json::JsonDeserialize for RequiredLanePayloadAvailabilityVote {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        <Option<LanePayloadAvailabilityVoteV1> as norito::json::JsonDeserialize>::json_deserialize(
            parser,
        )
        .map(Self)
    }
}

#[derive(JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct LaneBlockVoteJson {
    body: LaneBlockVoteBodyV1,
    payload_availability_vote: RequiredLanePayloadAvailabilityVote,
    signer: PeerId,
    bls_signature: Vec<u8>,
}

impl norito::json::JsonDeserialize for LaneBlockVoteV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let decoded =
            <LaneBlockVoteJson as norito::json::JsonDeserialize>::json_deserialize(parser)?;
        Ok(Self {
            body: decoded.body,
            payload_availability_vote: decoded.payload_availability_vote.0,
            signer: decoded.signer,
            bls_signature: decoded.bls_signature,
        })
    }
}

/// Stable key for one lane-local proposal session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LaneBlockSessionKey {
    /// Lane whose local block is being certified.
    pub(crate) lane_id: LaneId,
    /// Dataspace bound to the lane-local block.
    pub(crate) dataspace_id: DataSpaceId,
    /// Exact lane lifecycle incarnation.
    pub(crate) lane_incarnation: Hash,
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
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
}

/// Stable key for signer commit-vote safety.
///
/// Lane views are intentionally excluded: a validator must not commit-vote two
/// different payloads for the same lane-local height, even if global proposal
/// retries move the lane block to a later view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct LaneBlockCommitSlotKey {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
}

/// Cached lane-local proposal, votes, and QCs for one proposal hash.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LaneBlockSession {
    /// Proposal artifact, when it has arrived.
    pub(crate) proposal: Option<LaneBlockProposalV1>,
    /// Exact autonomous payload/session body authorized from durable bytes.
    pub(crate) payload_availability_body: Option<LanePayloadAvailabilityBodyV1>,
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
    /// executable entrypoint is already owned by another live lane block
    #[error("lane block proposal reuses an entrypoint owned by another live lane block")]
    EntrypointAlreadyClaimed,
    /// vote body does not match the cached proposal artifact
    #[error("lane block vote body does not match proposal")]
    VoteProposalMismatch,
    /// vote signer is not in the cached proposal validator set
    #[error("lane block vote signer is not in validator set")]
    VoteSignerNotInValidatorSet,
    /// autonomous payload evidence arrived before exact durable authorization
    #[error("lane payload availability evidence is not authorized for this session")]
    AvailabilityNotAuthorized,
    /// autonomous payload evidence differs from the exact authorized body
    #[error("lane payload availability evidence does not match authorized payload")]
    AvailabilityMismatch,
    /// commit evidence arrived before a valid prepare certificate
    #[error("lane block commit evidence arrived before prepare certificate")]
    CommitBeforePrepareQc,
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
/// carry commit votes or a commit QC are protected from eviction because they
/// encode signer locks and safety evidence. Fully certified sessions remain
/// protected until the durable consumer boundary drains them, because dropping
/// certified lane blocks under queue backpressure can strand lane-local
/// progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LaneBlockSessionCache {
    capacity: usize,
    sessions: BTreeMap<LaneBlockSessionKey, LaneBlockSession>,
    slot_proposals: BTreeMap<LaneBlockSlotKey, Hash>,
    commit_vote_locks: BTreeMap<(LaneBlockCommitSlotKey, PeerId), Hash>,
    entrypoint_claims: BTreeMap<Hash, LaneBlockSessionKey>,
    order: VecDeque<LaneBlockSessionKey>,
}

impl LaneBlockSessionCache {
    /// Build a cache that stores at most `capacity.max(1)` unprotected sessions.
    #[must_use]
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            sessions: BTreeMap::new(),
            slot_proposals: BTreeMap::new(),
            commit_vote_locks: BTreeMap::new(),
            entrypoint_claims: BTreeMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Number of cached sessions.
    pub(crate) fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Return the unique lane-local slots represented by retained replay or
    /// commit-lock evidence.
    ///
    /// Global-height rollover uses this bounded projection to resolve each
    /// slot against its exact canonical Kura proposal before moving the cache
    /// into the successor. Payload bytes and votes remain private to the
    /// cache; callers receive only the lookup coordinates needed for that
    /// validation.
    pub(crate) fn rollover_slots(&self) -> BTreeSet<(LaneId, u64)> {
        self.sessions
            .keys()
            .map(|key| (key.lane_id, key.lane_block_height))
            .chain(
                self.commit_vote_locks
                    .keys()
                    .map(|(slot, _)| (slot.lane_id, slot.lane_block_height)),
            )
            .collect()
    }

    /// Return proposal hashes that still have replay ownership in the cache.
    pub(crate) fn rollover_proposal_hashes(&self) -> BTreeSet<Hash> {
        self.sessions
            .values()
            .filter_map(|session| {
                (!session.committed_session_drained)
                    .then(|| {
                        session
                            .proposal
                            .as_ref()
                            .map(|proposal| proposal.proposal_hash)
                    })
                    .flatten()
            })
            .collect()
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

    /// Check whether a standalone lane-block proposal can be accepted without mutating the cache.
    #[cfg(test)]
    pub(crate) fn can_accept_proposal(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> Result<(), LaneBlockSessionError> {
        validate_lane_block_proposal(proposal).map_err(LaneBlockSessionError::InvalidProposal)?;
        let key = LaneBlockSessionKey::from_proposal(proposal);
        let slot_key = LaneBlockSlotKey::from_session_key(key);
        if let Some(existing_hash) = self.slot_proposals.get(&slot_key).copied()
            && existing_hash != key.proposal_hash
        {
            return Err(LaneBlockSessionError::ConflictingProposal);
        }
        if let Some(session) = self.sessions.get(&key)
            && session
                .proposal
                .as_ref()
                .is_some_and(|existing| !existing.same_consensus_identity(proposal))
        {
            return Err(LaneBlockSessionError::ConflictingProposal);
        }
        self.ensure_entrypoints_available(proposal, key)?;
        Ok(())
    }

    /// Check whether a standalone lane-block vote can be accepted without mutating the cache.
    #[cfg(test)]
    pub(crate) fn can_accept_vote(
        &self,
        vote: &LaneBlockVoteV1,
        _sender: Option<&PeerId>,
    ) -> Result<(), LaneBlockSessionError> {
        vote.validate_ingress_shape(vote.body.phase)
            .map_err(LaneBlockSessionError::InvalidVote)?;
        let phase = vote.body.phase;
        let key = LaneBlockSessionKey::from_vote_body(&vote.body);
        let Some(session) = self.sessions.get(&key) else {
            if vote.payload_availability_vote.is_some() {
                return Err(LaneBlockSessionError::AvailabilityNotAuthorized);
            }
            if phase == CertPhase::Commit {
                return Err(LaneBlockSessionError::CommitBeforePrepareQc);
            }
            vote.verify_signatures()
                .map_err(LaneBlockSessionError::InvalidVote)?;
            return Ok(());
        };
        validate_vote_matches_session(vote, session)?;
        vote.verify_signatures()
            .map_err(LaneBlockSessionError::InvalidVote)?;
        let votes = votes_for_phase(session, phase).ok_or(LaneBlockSessionError::InvalidVote(
            LaneBlockVoteIngressError::InvalidBody,
        ))?;
        if let Some(existing) = votes.get(&vote.signer)
            && existing != vote
        {
            return Err(LaneBlockSessionError::ConflictingVote);
        }
        self.validate_commit_vote_lock(vote)?;
        Ok(())
    }

    /// Return whether the exact proposal artifact is already cached.
    pub(crate) fn contains_proposal(&self, proposal: &LaneBlockProposalV1) -> bool {
        let key = LaneBlockSessionKey::from_proposal(proposal);
        self.sessions
            .get(&key)
            .and_then(|session| session.proposal.as_ref())
            == Some(proposal)
    }

    /// Return whether the proposal's consensus identity is cached, ignoring its
    /// advisory global-block recovery hint.
    #[cfg(test)]
    pub(crate) fn contains_proposal_identity(&self, proposal: &LaneBlockProposalV1) -> bool {
        let key = LaneBlockSessionKey::from_proposal(proposal);
        self.sessions
            .get(&key)
            .and_then(|session| session.proposal.as_ref())
            .is_some_and(|cached| cached.same_consensus_identity(proposal))
    }

    /// Return whether the exact vote artifact is already cached.
    #[cfg(test)]
    pub(crate) fn contains_vote(&self, vote: &LaneBlockVoteV1) -> bool {
        let key = LaneBlockSessionKey::from_vote_body(&vote.body);
        self.sessions
            .get(&key)
            .and_then(|session| votes_for_phase(session, vote.body.phase))
            .and_then(|votes| votes.get(&vote.signer))
            == Some(vote)
    }

    /// Return the proposal bound to an exact session key.
    pub(crate) fn proposal_for_key(
        &self,
        key: &LaneBlockSessionKey,
    ) -> Option<LaneBlockProposalV1> {
        self.sessions
            .get(key)
            .and_then(|session| session.proposal.clone())
    }

    /// Return the proposal bound to an exact Prepare/Commit vote body.
    pub(crate) fn proposal_for_vote_body(
        &self,
        body: &LaneBlockVoteBodyV1,
    ) -> Option<LaneBlockProposalV1> {
        self.proposal_for_key(&LaneBlockSessionKey::from_vote_body(body))
    }

    /// Return whether an exact lane incarnation retains work that has not yet
    /// crossed the committed-session handoff boundary.
    pub(crate) fn has_undrained_work_for_lane(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
    ) -> bool {
        self.sessions.iter().any(|(key, session)| {
            key.lane_id == lane_id
                && key.dataspace_id == dataspace_id
                && key.lane_incarnation == lane_incarnation
                && !session.committed_session_drained
        })
    }

    /// Authorize the exact autonomous payload body derived from durable bytes.
    ///
    /// Existing compatibility prepare votes/QCs are purged before the body is
    /// installed, preventing reordered unsigned votes from being reused after
    /// an autonomous payload arrives.
    pub(crate) fn authorize_payload_availability(
        &mut self,
        proposal: &LaneBlockProposalV1,
        body: LanePayloadAvailabilityBodyV1,
    ) -> Result<(), LaneBlockSessionError> {
        validate_availability_body_matches_proposal(&body, proposal)
            .map_err(|_| LaneBlockSessionError::AvailabilityMismatch)?;
        let key = LaneBlockSessionKey::from_proposal(proposal);
        let session = self
            .sessions
            .get_mut(&key)
            .ok_or(LaneBlockSessionError::AvailabilityNotAuthorized)?;
        if !session
            .proposal
            .as_ref()
            .is_some_and(|cached| cached.same_consensus_identity(proposal))
        {
            return Err(LaneBlockSessionError::AvailabilityMismatch);
        }
        if let Some(existing) = &session.payload_availability_body {
            return if existing == &body {
                Ok(())
            } else {
                Err(LaneBlockSessionError::AvailabilityMismatch)
            };
        }

        session.payload_availability_body = Some(body.clone());
        session.prepare_votes.retain(|_, vote| {
            vote.payload_availability_vote
                .as_ref()
                .is_some_and(|availability| availability.body == body)
        });
        let prepare_qc_matches = session
            .prepare_qc
            .as_ref()
            .and_then(|qc| qc.payload_availability_qc.as_ref())
            .is_some_and(|availability| availability.body == body);
        if !prepare_qc_matches {
            session.prepare_qc = None;
            session.commit_votes.clear();
            session.commit_qc = None;
            session.pending_commit_vote_request = false;
            session.commit_vote_request_drained = false;
            session.pending_committed_session_drain = false;
            session.committed_session_drained = false;
        }
        try_seal_phase_qc(session, CertPhase::Prepare);
        refresh_commit_vote_request_ready(session);
        refresh_committed_session_ready(session);
        Ok(())
    }

    /// Return the exact READY body previously authorized from durable payload
    /// bytes for this proposal.
    pub(crate) fn authorized_payload_availability_body_for(
        &self,
        proposal: &LaneBlockProposalV1,
    ) -> Option<LanePayloadAvailabilityBodyV1> {
        let key = LaneBlockSessionKey::from_proposal(proposal);
        self.sessions.get(&key).and_then(|session| {
            session
                .proposal
                .as_ref()
                .is_some_and(|cached| cached.same_consensus_identity(proposal))
                .then(|| session.payload_availability_body.clone())
                .flatten()
        })
    }

    /// Return whether this validator still needs to synthesize a prepare vote for a proposal.
    pub(crate) fn local_prepare_vote_needed_for(
        &self,
        proposal: &LaneBlockProposalV1,
        signer: &PeerId,
    ) -> bool {
        let key = LaneBlockSessionKey::from_proposal(proposal);
        self.sessions.get(&key).is_some_and(|session| {
            session.proposal.as_ref() == Some(proposal)
                && session.prepare_qc.is_none()
                && !session.prepare_votes.contains_key(signer)
                && proposal.descriptor.validator_set.contains(signer)
        })
    }

    /// Return cached proposals that still need this validator's prepare vote.
    pub(crate) fn local_prepare_vote_proposals_for(
        &self,
        signer: &PeerId,
    ) -> Vec<LaneBlockProposalV1> {
        self.sessions
            .values()
            .filter_map(|session| {
                let proposal = session.proposal.as_ref()?;
                if session.prepare_qc.is_some()
                    || session.prepare_votes.contains_key(signer)
                    || !proposal.descriptor.validator_set.contains(signer)
                {
                    return None;
                }
                Some(proposal.clone())
            })
            .collect()
    }

    /// Return cached prepared sessions that still need this validator's commit vote.
    pub(crate) fn local_commit_vote_requests_for(
        &self,
        signer: &PeerId,
    ) -> Vec<LaneBlockCommitVoteRequest> {
        self.sessions
            .values()
            .filter_map(|session| {
                let proposal = session.proposal.as_ref()?;
                let prepare_qc = session.prepare_qc.as_ref()?;
                if session.commit_qc.is_some()
                    || session.commit_votes.contains_key(signer)
                    || !proposal.descriptor.validator_set.contains(signer)
                {
                    return None;
                }
                Some(LaneBlockCommitVoteRequest {
                    proposal: proposal.clone(),
                    prepare_qc: prepare_qc.clone(),
                })
            })
            .collect()
    }

    /// Return cached proposals that may still need committee fanout.
    pub(crate) fn proposals_without_commit_qc(&self) -> Vec<LaneBlockProposalV1> {
        self.sessions
            .values()
            .filter_map(|session| {
                if session.commit_qc.is_some() || session.committed_session_drained {
                    return None;
                }
                session.proposal.clone()
            })
            .collect()
    }

    /// Return this validator's cached votes and matching proposals that may still need fanout.
    pub(crate) fn local_vote_rebroadcast_artifacts_for(
        &self,
        signer: &PeerId,
    ) -> Vec<(LaneBlockProposalV1, LaneBlockVoteV1)> {
        self.sessions
            .values()
            .flat_map(|session| {
                let Some(proposal) = session.proposal.as_ref() else {
                    return Vec::new();
                };
                if !proposal.descriptor.validator_set.contains(signer) {
                    return Vec::new();
                }
                let mut votes = Vec::with_capacity(2);
                if session.prepare_qc.is_none()
                    && let Some(vote) = session.prepare_votes.get(signer)
                {
                    votes.push((proposal.clone(), vote.clone()));
                }
                if session.commit_qc.is_none()
                    && let Some(vote) = session.commit_votes.get(signer)
                {
                    votes.push((proposal.clone(), vote.clone()));
                }
                votes
            })
            .collect()
    }

    /// Return cached QCs that may still need committee fanout.
    pub(crate) fn qcs_for_incomplete_sessions(&self) -> Vec<LaneBlockQcV1> {
        self.sessions
            .values()
            .flat_map(|session| {
                if session.committed_session_drained {
                    return Vec::new();
                }
                let mut qcs = Vec::with_capacity(2);
                if let Some(qc) = session.prepare_qc.clone() {
                    qcs.push(qc);
                }
                if let Some(qc) = session.commit_qc.clone() {
                    qcs.push(qc);
                }
                qcs
            })
            .collect()
    }

    fn drain_newly_sealed_qcs_where(
        &mut self,
        mut may_drain: impl FnMut(&LaneBlockQcV1) -> bool,
    ) -> Vec<LaneBlockQcV1> {
        let mut sealed = Vec::new();
        for session in self.sessions.values_mut() {
            if session.pending_prepare_qc_broadcast
                && let Some(qc) = session.prepare_qc.as_ref()
                && may_drain(qc)
            {
                if let Some(qc) = session.prepare_qc.clone() {
                    sealed.push(qc);
                }
                session.pending_prepare_qc_broadcast = false;
            }
            if session.pending_commit_qc_broadcast
                && let Some(qc) = session.commit_qc.as_ref()
                && may_drain(qc)
            {
                if let Some(qc) = session.commit_qc.clone() {
                    sealed.push(qc);
                }
                session.pending_commit_qc_broadcast = false;
            }
        }
        sealed
    }

    /// Drain QCs sealed locally from cached votes and mark them as handed to transport.
    #[cfg(test)]
    pub(crate) fn drain_newly_sealed_qcs(&mut self) -> Vec<LaneBlockQcV1> {
        self.drain_newly_sealed_qcs_where(|_| true)
    }

    /// Drain only locally sealed QCs admitted by the caller's current protocol gate.
    ///
    /// A rejected QC retains its pending-broadcast bit so a later monotone
    /// authority transition can publish the same immutable evidence.
    pub(crate) fn drain_newly_sealed_qcs_matching(
        &mut self,
        admitted: &BTreeSet<HashOf<LaneBlockQcV1>>,
    ) -> Vec<LaneBlockQcV1> {
        self.drain_newly_sealed_qcs_where(|qc| admitted.contains(&HashOf::new(qc)))
    }

    /// Drain sessions that have a proposal and prepare QC and still need the
    /// supplied validator's local commit vote.
    #[cfg(test)]
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
    /// This is intentionally separate from [`Self::drain_newly_sealed_qcs_matching`]:
    /// inbound QCs are not transport work, but they can still make a session
    /// executable once the matching proposal and opposite phase QC are present.
    fn drain_committed_sessions_up_to_where(
        &mut self,
        limit: usize,
        mut may_drain: impl FnMut(&LaneBlockProposalV1) -> bool,
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
            let (Some(proposal), Some(prepare_qc), Some(commit_qc)) = (
                session.proposal.clone(),
                session.prepare_qc.clone(),
                session.commit_qc.clone(),
            ) else {
                continue;
            };
            if !may_drain(&proposal) {
                continue;
            }
            session.pending_committed_session_drain = false;
            session.committed_session_drained = true;
            committed.push(CommittedLaneBlockSession {
                proposal,
                prepare_qc,
                commit_qc,
            });
        }
        // The returned committed bundle now owns executor replay evidence. Retire
        // drained cache entries back under the ordinary capacity bound while the
        // independent signer commit locks continue to enforce equivocation safety.
        self.evict();
        committed
    }

    /// Drain up to `limit` complete sessions without an additional protocol gate.
    #[cfg(test)]
    pub(crate) fn drain_committed_sessions_up_to(
        &mut self,
        limit: usize,
    ) -> Vec<CommittedLaneBlockSession> {
        self.drain_committed_sessions_up_to_where(limit, |_| true)
    }

    /// Drain only complete sessions admitted by the caller's current protocol gate.
    ///
    /// Rejected sessions retain their pending-drain state and all quorum
    /// evidence so a later monotone authority transition can consume them.
    pub(crate) fn drain_committed_sessions_up_to_matching(
        &mut self,
        limit: usize,
        admitted: &BTreeSet<Hash>,
    ) -> Vec<CommittedLaneBlockSession> {
        self.drain_committed_sessions_up_to_where(limit, |proposal| {
            admitted.contains(&proposal.proposal_hash)
        })
    }

    /// Drain all sessions whose proposal, prepare QC, and commit QC are all cached.
    #[cfg(test)]
    pub(crate) fn drain_committed_sessions(&mut self) -> Vec<CommittedLaneBlockSession> {
        self.drain_committed_sessions_up_to(usize::MAX)
    }

    /// Remove cached sessions whose lane/dataspace/height no longer belongs to the active topology.
    #[cfg(test)]
    pub(crate) fn retain_sessions_for_admissible_lanes(
        &mut self,
        admissible_lane: impl Fn(LaneId, DataSpaceId, Hash, u64, u64) -> bool,
    ) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|key, session| {
            let proposal_height = session_proposal_height(session).unwrap_or(key.lane_block_height);
            admissible_lane(
                key.lane_id,
                key.dataspace_id,
                key.lane_incarnation,
                key.lane_block_height,
                proposal_height,
            )
        });
        self.rebuild_indices_after_session_retain();
        before.saturating_sub(self.sessions.len())
    }

    /// Retire speculative sessions tied only to a superseded global carrier.
    ///
    /// PrepareQCs, Commit votes, and CommitQCs remain protected: a quorum-certified
    /// conflicting lane identity must still be presented to locked-body binding so
    /// it can fail closed. Uncertified proposal/Prepare state may be reconstructed
    /// from the exact replacement global body and must not occupy capacity or drive
    /// retransmission for the losing carrier.
    pub(crate) fn retire_uncommitted_global_anchor(
        &mut self,
        active_global_height: u64,
        global_block_hash: HashOf<BlockHeader>,
    ) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|_, session| {
            let tied_to_anchor = session.proposal.as_ref().is_some_and(|proposal| {
                proposal.descriptor.proposal_height == active_global_height
                    && proposal
                        .payload_block_hint
                        .is_some_and(|hint| hint.proposal_block_hash == global_block_hash)
            });
            !tied_to_anchor
                || session_is_eviction_protected(session)
                || session_has_quorum_certificate(session)
        });
        self.rebuild_indices_after_session_retain();
        before.saturating_sub(self.sessions.len())
    }

    /// Retire every speculative session not tied to the installed global carrier.
    ///
    /// PrepareQCs, Commit votes, and CommitQCs remain protected. This first-lock
    /// sweep prevents uncertified carriers learned before any global lock from
    /// consuming bounded cache capacity forever without hiding a certified
    /// conflict from exact locked-body validation.
    pub(crate) fn retire_uncommitted_global_anchors_except(
        &mut self,
        active_global_height: u64,
        retained_global_block_hash: HashOf<BlockHeader>,
    ) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|_, session| {
            let losing_anchor = session.proposal.as_ref().is_some_and(|proposal| {
                proposal.descriptor.proposal_height == active_global_height
                    && proposal
                        .payload_block_hint
                        .is_some_and(|hint| hint.proposal_block_hash != retained_global_block_hash)
            });
            !losing_anchor
                || session_is_eviction_protected(session)
                || session_has_quorum_certificate(session)
        });
        self.rebuild_indices_after_session_retain();
        before.saturating_sub(self.sessions.len())
    }

    /// Retain only exact, canonical, unfinalized evidence across a global-height rollover.
    ///
    /// `canonical_proposal` resolves the one Kura-anchored proposal for a lane-local
    /// slot. Advisory payload hints are normalized to that exact proposal while the
    /// consensus identity, votes, QCs, locally sealed broadcast state, and signer
    /// commit locks are preserved. Orphan or losing speculative evidence is pruned.
    /// A quorum-certified identity that conflicts with the canonical proposal fails
    /// before any cache mutation.
    ///
    /// # Errors
    ///
    /// Returns [`LaneBlockSessionError::ConflictingProposal`] if retained quorum
    /// evidence certifies a different identity for a canonical lane-local slot.
    pub(crate) fn retain_canonical_rollover_evidence(
        &mut self,
        capacity: usize,
        canonical_proposal: impl Fn(LaneId, u64) -> Option<LaneBlockProposalV1>,
        active_slot: impl Fn(LaneId, DataSpaceId, Hash, u64) -> bool,
        unfinalized_slot: impl Fn(LaneId, DataSpaceId, Hash, u64) -> bool,
    ) -> Result<usize, LaneBlockSessionError> {
        let before = self
            .sessions
            .len()
            .saturating_add(self.commit_vote_locks.len());
        let mut next = self.clone();
        next.capacity = capacity.max(1);

        let canonical_slots = self
            .sessions
            .keys()
            .map(|key| (key.lane_id, key.lane_block_height))
            .chain(
                self.commit_vote_locks
                    .keys()
                    .map(|(slot, _)| (slot.lane_id, slot.lane_block_height)),
            )
            .collect::<BTreeSet<_>>();
        let canonical_proposals = canonical_slots
            .into_iter()
            .map(|(lane_id, lane_block_height)| {
                (
                    (lane_id, lane_block_height),
                    canonical_proposal(lane_id, lane_block_height),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let evidence_slots = self
            .sessions
            .keys()
            .map(|key| {
                (
                    key.lane_id,
                    key.dataspace_id,
                    key.lane_incarnation,
                    key.lane_block_height,
                )
            })
            .chain(self.commit_vote_locks.keys().map(|(slot, _)| {
                (
                    slot.lane_id,
                    slot.dataspace_id,
                    slot.lane_incarnation,
                    slot.lane_block_height,
                )
            }))
            .collect::<BTreeSet<_>>();
        let active_slots = evidence_slots
            .iter()
            .copied()
            .map(
                |slot @ (lane_id, dataspace_id, lane_incarnation, lane_block_height)| {
                    (
                        slot,
                        active_slot(lane_id, dataspace_id, lane_incarnation, lane_block_height),
                    )
                },
            )
            .collect::<BTreeMap<_, _>>();
        let unfinalized_slots = evidence_slots
            .into_iter()
            .map(
                |slot @ (lane_id, dataspace_id, lane_incarnation, lane_block_height)| {
                    (
                        slot,
                        unfinalized_slot(
                            lane_id,
                            dataspace_id,
                            lane_incarnation,
                            lane_block_height,
                        ),
                    )
                },
            )
            .collect::<BTreeMap<_, _>>();

        // A committed session may already have left the replay map while its
        // independent signer locks remain. Reconstruct enough of that durable
        // safety evidence to reject a conflicting canonical identity before
        // pruning, normalizing, or publishing any cache state. Counting only
        // signers in the canonical committee prevents stale or foreign-route
        // locks from manufacturing a conflict for this slot.
        let mut conflicting_lock_quorums =
            BTreeMap::<(LaneBlockCommitSlotKey, Hash), BTreeSet<PeerId>>::new();
        for ((slot, signer), locked_proposal_hash) in &self.commit_vote_locks {
            let evidence_slot = (
                slot.lane_id,
                slot.dataspace_id,
                slot.lane_incarnation,
                slot.lane_block_height,
            );
            if active_slots.get(&evidence_slot) != Some(&true) {
                continue;
            }
            let Some(canonical) = canonical_proposals
                .get(&(slot.lane_id, slot.lane_block_height))
                .and_then(Option::as_ref)
            else {
                continue;
            };
            let descriptor = &canonical.descriptor;
            if descriptor.lane_id != slot.lane_id
                || descriptor.dataspace_id != slot.dataspace_id
                || descriptor.lane_incarnation != slot.lane_incarnation
                || descriptor.lane_block_height != slot.lane_block_height
                || canonical.proposal_hash == *locked_proposal_hash
                || descriptor.validator_set.binary_search(signer).is_err()
            {
                continue;
            }
            conflicting_lock_quorums
                .entry((*slot, *locked_proposal_hash))
                .or_default()
                .insert(signer.clone());
        }
        if conflicting_lock_quorums.iter().any(|((slot, _), signers)| {
            canonical_proposals
                .get(&(slot.lane_id, slot.lane_block_height))
                .and_then(Option::as_ref)
                .is_some_and(|canonical| {
                    usize::try_from(canonical.descriptor.min_quorum)
                        .is_ok_and(|quorum| signers.len() >= quorum)
                })
        }) {
            return Err(LaneBlockSessionError::ConflictingProposal);
        }

        let mut retained_sessions = BTreeMap::new();
        for (key, session) in &self.sessions {
            let evidence_slot = (
                key.lane_id,
                key.dataspace_id,
                key.lane_incarnation,
                key.lane_block_height,
            );
            if active_slots.get(&evidence_slot) != Some(&true) {
                continue;
            }
            let Some(canonical) = canonical_proposals
                .get(&(key.lane_id, key.lane_block_height))
                .and_then(Option::as_ref)
            else {
                continue;
            };

            let canonical_key = LaneBlockSessionKey::from_proposal(canonical);
            let proposal_conflicts = session
                .proposal
                .as_ref()
                .is_some_and(|proposal| !proposal.same_consensus_identity(canonical));
            let certified_body_conflicts = session
                .prepare_qc
                .as_ref()
                .is_some_and(|qc| validate_qc_matches_proposal(qc, canonical).is_err())
                || session
                    .commit_qc
                    .as_ref()
                    .is_some_and(|qc| validate_qc_matches_proposal(qc, canonical).is_err());
            if session_has_quorum_certificate(session)
                && (canonical_key != *key || proposal_conflicts || certified_body_conflicts)
            {
                return Err(LaneBlockSessionError::ConflictingProposal);
            }
            if unfinalized_slots.get(&evidence_slot) != Some(&true) {
                continue;
            }
            if canonical_key != *key || proposal_conflicts {
                continue;
            }

            let mut exact = session.clone();
            reconcile_session_with_proposal(&mut exact, canonical);
            exact.proposal = Some(canonical.clone());
            try_seal_session_qcs(&mut exact);
            refresh_commit_vote_request_ready(&mut exact);
            refresh_committed_session_ready(&mut exact);
            retained_sessions.insert(*key, exact);
        }
        next.sessions = retained_sessions;
        next.slot_proposals = next
            .sessions
            .iter()
            .filter_map(|(key, session)| {
                session
                    .proposal
                    .as_ref()
                    .map(|_| (LaneBlockSlotKey::from_session_key(*key), key.proposal_hash))
            })
            .collect();
        let retained_keys = next.sessions.keys().copied().collect::<BTreeSet<_>>();
        next.order.retain(|key| retained_keys.contains(key));
        next.rebuild_entrypoint_claims();

        next.commit_vote_locks.retain(|(slot, _), proposal_hash| {
            canonical_proposals
                .get(&(slot.lane_id, slot.lane_block_height))
                .and_then(Option::as_ref)
                .is_some_and(|canonical| {
                    let descriptor = &canonical.descriptor;
                    let evidence_slot = (
                        slot.lane_id,
                        slot.dataspace_id,
                        slot.lane_incarnation,
                        slot.lane_block_height,
                    );
                    descriptor.lane_id == slot.lane_id
                        && descriptor.dataspace_id == slot.dataspace_id
                        && descriptor.lane_incarnation == slot.lane_incarnation
                        && descriptor.lane_block_height == slot.lane_block_height
                        && canonical.proposal_hash == *proposal_hash
                        && active_slots.get(&evidence_slot) == Some(&true)
                        && unfinalized_slots.get(&evidence_slot) == Some(&true)
                })
        });
        next.evict();

        let after = next
            .sessions
            .len()
            .saturating_add(next.commit_vote_locks.len());
        *self = next;
        Ok(before.saturating_sub(after))
    }

    /// Retire signer commit locks only after an external durable-finality predicate.
    ///
    /// Session pruning and capacity eviction intentionally never call this method:
    /// locks must outlive transient replay state. The actor supplies an applied or
    /// snapshot-anchored Kura boundary (or an explicit lane reset watermark).
    #[cfg(test)]
    pub(crate) fn prune_commit_vote_locks_for_finalized_slots(
        &mut self,
        finalized: impl Fn(LaneId, DataSpaceId, Hash, u64) -> bool,
    ) -> usize {
        let before = self.commit_vote_locks.len();
        self.commit_vote_locks.retain(|(slot, _), _| {
            !finalized(
                slot.lane_id,
                slot.dataspace_id,
                slot.lane_incarnation,
                slot.lane_block_height,
            )
        });
        before.saturating_sub(self.commit_vote_locks.len())
    }

    /// Retire signer locks whose exact lane incarnation is no longer active.
    #[cfg(test)]
    pub(crate) fn prune_commit_vote_locks_for_inactive_incarnations(
        &mut self,
        active: impl Fn(LaneId, DataSpaceId, Hash) -> bool,
    ) -> usize {
        let before = self.commit_vote_locks.len();
        self.commit_vote_locks
            .retain(|(slot, _), _| active(slot.lane_id, slot.dataspace_id, slot.lane_incarnation));
        before.saturating_sub(self.commit_vote_locks.len())
    }

    /// Atomically retire replay sessions and signer locks at a durable slot boundary.
    #[cfg(test)]
    pub(crate) fn prune_sessions_and_commit_vote_locks_for_finalized_slots(
        &mut self,
        finalized: impl Fn(LaneId, DataSpaceId, Hash, u64) -> bool,
    ) -> usize {
        let sessions_before = self.sessions.len();
        self.sessions.retain(|key, _| {
            !finalized(
                key.lane_id,
                key.dataspace_id,
                key.lane_incarnation,
                key.lane_block_height,
            )
        });
        self.rebuild_indices_after_session_retain();
        let sessions_pruned = sessions_before.saturating_sub(self.sessions.len());
        sessions_pruned.saturating_add(self.prune_commit_vote_locks_for_finalized_slots(finalized))
    }

    /// Snapshot unique slots currently retaining signer commit locks.
    #[cfg(test)]
    pub(crate) fn commit_vote_lock_slots(&self) -> BTreeSet<(LaneId, DataSpaceId, Hash, u64)> {
        self.commit_vote_locks
            .keys()
            .map(|(slot, _)| {
                (
                    slot.lane_id,
                    slot.dataspace_id,
                    slot.lane_incarnation,
                    slot.lane_block_height,
                )
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn commit_vote_lock_len(&self) -> usize {
        self.commit_vote_locks.len()
    }

    /// Remove prepared sibling sessions once the exact canonical lane proposal is known.
    ///
    /// Undrained sessions carrying commit votes or a commit QC are retained as safety evidence.
    /// Honest nodes do not create that evidence for noncanonical siblings, while retaining it
    /// prevents canonical discovery from concealing a conflicting commit lock or certificate.
    #[cfg(test)]
    pub(crate) fn prune_uncommitted_sessions_conflicting_with_canonical_proposal(
        &mut self,
        canonical: &LaneBlockProposalV1,
    ) -> usize {
        let canonical_key = LaneBlockSessionKey::from_proposal(canonical);
        let before = self.sessions.len();
        self.sessions.retain(|key, session| {
            let conflicts_with_canonical_height = key.lane_id == canonical_key.lane_id
                && key.dataspace_id == canonical_key.dataspace_id
                && key.lane_incarnation == canonical_key.lane_incarnation
                && key.lane_block_height == canonical_key.lane_block_height
                && key.proposal_hash != canonical_key.proposal_hash;
            !conflicts_with_canonical_height || session_has_live_commit_evidence(session)
        });
        self.rebuild_indices_after_session_retain();
        before.saturating_sub(self.sessions.len())
    }

    /// Remove prepare-only sessions from superseded global views.
    ///
    /// The compatibility lane scheduler currently binds `lane_block_view` to the
    /// global proposal view. If lane views become independently scheduled, this
    /// global-view pruning coupling must be removed. Exact canonical sessions and
    /// every undrained session carrying commit evidence are protected; signer
    /// commit locks deliberately outlive the removed replay state.
    #[cfg(test)]
    pub(crate) fn prune_uncommitted_sessions_below_proposal_view(
        &mut self,
        proposal_height: u64,
        min_view: u64,
        canonical_sessions: &BTreeSet<LaneBlockSessionKey>,
    ) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|key, session| {
            let stale = session_proposal_height(session) == Some(proposal_height)
                && key.lane_block_view < min_view;
            !stale || canonical_sessions.contains(key) || session_has_live_commit_evidence(session)
        });
        self.rebuild_indices_after_session_retain();
        before.saturating_sub(self.sessions.len())
    }

    /// Bound prepare-only sibling replay state within a historical route context.
    ///
    /// Grouping includes global proposal height because committee/route authority
    /// can differ between heights. Canonical sessions and undrained commit
    /// evidence are exempt from the bound; among the remaining siblings the
    /// newest views win, with proposal hash providing deterministic tie-breaking.
    #[cfg(test)]
    pub(crate) fn prune_excess_speculative_siblings(
        &mut self,
        retained_per_group: usize,
        canonical_sessions: &BTreeSet<LaneBlockSessionKey>,
    ) -> usize {
        let retained_per_group = retained_per_group.max(1);
        let mut groups: BTreeMap<(u64, LaneId, DataSpaceId, u64), Vec<LaneBlockSessionKey>> =
            BTreeMap::new();
        for (key, session) in &self.sessions {
            if canonical_sessions.contains(key) || session_has_live_commit_evidence(session) {
                continue;
            }
            let Some(proposal_height) = session_proposal_height(session) else {
                // Valid proposals/votes/QCs all carry proposal height. Preserve an
                // unclassifiable session rather than applying an unsafe bound.
                continue;
            };
            groups
                .entry((
                    proposal_height,
                    key.lane_id,
                    key.dataspace_id,
                    key.lane_block_height,
                ))
                .or_default()
                .push(*key);
        }

        let mut remove = BTreeSet::new();
        for siblings in groups.values_mut() {
            siblings.sort_by_key(|key| (key.lane_block_view, key.proposal_hash));
            let excess = siblings.len().saturating_sub(retained_per_group);
            remove.extend(siblings.iter().take(excess).copied());
        }
        if remove.is_empty() {
            return 0;
        }

        let before = self.sessions.len();
        self.sessions.retain(|key, _| !remove.contains(key));
        self.rebuild_indices_after_session_retain();
        before.saturating_sub(self.sessions.len())
    }

    fn rebuild_indices_after_session_retain(&mut self) {
        let retained_slot_proposals = self
            .sessions
            .keys()
            .map(|key| {
                (
                    LaneBlockSlotKey {
                        lane_id: key.lane_id,
                        dataspace_id: key.dataspace_id,
                        lane_incarnation: key.lane_incarnation,
                        lane_block_height: key.lane_block_height,
                        lane_block_view: key.lane_block_view,
                    },
                    key.proposal_hash,
                )
            })
            .collect::<BTreeSet<_>>();
        self.slot_proposals
            .retain(|key, proposal_hash| retained_slot_proposals.contains(&(*key, *proposal_hash)));
        let retained_keys = self.sessions.keys().copied().collect::<BTreeSet<_>>();
        self.order.retain(|key| retained_keys.contains(key));
        self.rebuild_entrypoint_claims();
    }

    /// Return lanes with committed lane-block sessions that have not yet drained to execution.
    #[cfg(test)]
    pub(crate) fn pending_lane_ids_for_admissible_lanes(
        &self,
        admissible_lane: impl Fn(LaneId, DataSpaceId, Hash, u64, u64) -> bool,
    ) -> BTreeSet<LaneId> {
        self.sessions
            .iter()
            .filter_map(|(key, session)| {
                session.proposal.as_ref()?;
                if session.commit_qc.is_none() || session.committed_session_drained {
                    return None;
                }
                let proposal_height =
                    session_proposal_height(session).unwrap_or(key.lane_block_height);
                admissible_lane(
                    key.lane_id,
                    key.dataspace_id,
                    key.lane_incarnation,
                    key.lane_block_height,
                    proposal_height,
                )
                .then_some(key.lane_id)
            })
            .collect()
    }

    /// Return lanes with non-drained lane-block consensus evidence.
    ///
    /// The predicate receives whether the matching session has any vote or QC
    /// evidence so proposal planning can distinguish proposal-only artifacts
    /// from committee-certified in-flight work. Drained committed sessions are
    /// excluded because the committed lane-block queue owns application
    /// ordering after that point.
    #[cfg(test)]
    pub(crate) fn inflight_lane_ids_for_admissible_lanes(
        &self,
        admissible_lane: impl Fn(LaneId, DataSpaceId, Hash, u64, u64, bool) -> bool,
    ) -> BTreeSet<LaneId> {
        self.sessions
            .iter()
            .filter_map(|(key, session)| {
                if session.committed_session_drained {
                    return None;
                }
                let has_consensus_evidence = session_has_consensus_evidence(session);
                if session.proposal.is_none() && !has_consensus_evidence {
                    return None;
                }
                let proposal_height =
                    session_proposal_height(session).unwrap_or(key.lane_block_height);
                admissible_lane(
                    key.lane_id,
                    key.dataspace_id,
                    key.lane_incarnation,
                    key.lane_block_height,
                    proposal_height,
                    has_consensus_evidence,
                )
                .then_some(key.lane_id)
            })
            .collect()
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
        self.ensure_entrypoints_available(&proposal, key)?;

        self.touch(key);
        let session = self.sessions.entry(key).or_default();
        if let Some(existing) = &mut session.proposal {
            if existing == &proposal {
                return Ok(LaneBlockSessionInsertOutcome::Duplicate);
            }
            if existing.same_consensus_identity(&proposal) {
                if existing.payload_block_hint.is_none() && proposal.payload_block_hint.is_some() {
                    existing.payload_block_hint = proposal.payload_block_hint;
                    try_seal_session_qcs(session);
                    refresh_commit_vote_request_ready(session);
                    refresh_committed_session_ready(session);
                    self.evict();
                    return Ok(LaneBlockSessionInsertOutcome::Inserted);
                }
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
        for entrypoint_hash in &session
            .proposal
            .as_ref()
            .expect("proposal was installed before entrypoint claims")
            .descriptor
            .accepted_transaction_hashes
        {
            self.entrypoint_claims
                .entry(*entrypoint_hash)
                .or_insert(key);
        }
        self.evict();
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    }

    /// Insert a proposal reconstructed from durable lane-block artifacts.
    ///
    /// Durable Kura artifacts prove the proposal payload already reached a
    /// canonical block. Recovery may therefore replace a conflicting in-memory
    /// proposal shell or individually voted proposal for the same lane slot.
    /// A verified quorum certificate remains protected because it proves the
    /// committee already certified that conflicting identity.
    pub(crate) fn insert_recovered_proposal_replacing_uncommitted_conflict(
        &mut self,
        proposal: LaneBlockProposalV1,
    ) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
        self.insert_trusted_proposal_replacing_uncommitted_conflict(proposal)
    }

    /// Replace losing local proposal work before the global body is locked.
    ///
    /// This has the same quorum-preserving conflict rule as durable recovery,
    /// but is a separate call site so untrusted network ingress cannot invoke
    /// replacement semantics. A changed global-block hint is normalized even
    /// when the lane consensus identity is unchanged because hints are advisory
    /// and are not signed by lane Prepare/Commit votes.
    pub(crate) fn insert_replanned_proposal_replacing_uncommitted_conflict(
        &mut self,
        proposal: LaneBlockProposalV1,
    ) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
        self.insert_trusted_proposal_replacing_uncommitted_conflict(proposal)
    }

    fn insert_trusted_proposal_replacing_uncommitted_conflict(
        &mut self,
        proposal: LaneBlockProposalV1,
    ) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
        validate_lane_block_proposal(&proposal).map_err(LaneBlockSessionError::InvalidProposal)?;
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let slot_key = LaneBlockSlotKey::from_session_key(key);

        if let Some(existing) = self
            .sessions
            .get(&key)
            .and_then(|session| session.proposal.as_ref())
            .filter(|existing| existing.same_consensus_identity(&proposal))
        {
            if existing == &proposal {
                return Ok(LaneBlockSessionInsertOutcome::Duplicate);
            }
            self.sessions
                .get_mut(&key)
                .expect("trusted proposal session was just observed")
                .proposal = Some(proposal);
            self.touch(key);
            self.evict();
            return Ok(LaneBlockSessionInsertOutcome::Inserted);
        }

        if let Some(existing_hash) = self.slot_proposals.get(&slot_key).copied()
            && existing_hash != key.proposal_hash
        {
            let existing_key = LaneBlockSessionKey {
                lane_id: key.lane_id,
                dataspace_id: key.dataspace_id,
                lane_incarnation: key.lane_incarnation,
                lane_block_height: key.lane_block_height,
                lane_block_view: key.lane_block_view,
                proposal_hash: existing_hash,
            };
            if let Some(existing_session) = self.sessions.get(&existing_key)
                && session_has_quorum_certificate(existing_session)
            {
                return Err(LaneBlockSessionError::ConflictingProposal);
            }
            self.remove_slot_conflict(slot_key, existing_hash);
        }

        self.insert_proposal(proposal)
    }

    /// Insert a standalone lane-block vote.
    pub(crate) fn insert_vote(
        &mut self,
        vote: LaneBlockVoteV1,
        _sender: Option<&PeerId>,
    ) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
        vote.validate_ingress_shape(vote.body.phase)
            .map_err(LaneBlockSessionError::InvalidVote)?;
        let phase = vote.body.phase;
        let key = LaneBlockSessionKey::from_vote_body(&vote.body);

        if let Some(session) = self.sessions.get(&key) {
            validate_vote_matches_session(&vote, session)?;
        } else {
            if vote.payload_availability_vote.is_some() {
                return Err(LaneBlockSessionError::AvailabilityNotAuthorized);
            }
            if phase == CertPhase::Commit {
                return Err(LaneBlockSessionError::CommitBeforePrepareQc);
            }
        }
        vote.verify_signatures()
            .map_err(LaneBlockSessionError::InvalidVote)?;
        if let Some(session) = self.sessions.get(&key) {
            let votes = votes_for_phase(session, phase).ok_or(
                LaneBlockSessionError::InvalidVote(LaneBlockVoteIngressError::InvalidBody),
            )?;
            if let Some(existing) = votes.get(&vote.signer) {
                if existing == &vote {
                    return Ok(LaneBlockSessionInsertOutcome::Duplicate);
                }
                return Err(LaneBlockSessionError::ConflictingVote);
            }
        }
        self.validate_commit_vote_lock(&vote)?;

        self.touch(key);
        {
            let session = self.sessions.entry(key).or_default();
            let votes = votes_for_phase_mut(session, phase).ok_or(
                LaneBlockSessionError::InvalidVote(LaneBlockVoteIngressError::InvalidBody),
            )?;
            votes.insert(vote.signer.clone(), vote.clone());
            try_seal_phase_qc(session, phase);
            refresh_commit_vote_request_ready(session);
            refresh_committed_session_ready(session);
        }
        self.record_commit_vote_lock(&vote);
        self.evict();
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    }

    /// Insert a standalone lane-block QC without aggregate verification.
    #[cfg(test)]
    pub(crate) fn insert_qc(
        &mut self,
        qc: LaneBlockQcV1,
    ) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
        self.validate_qc_session_preconditions(&qc)?;
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
        self.validate_qc_session_preconditions(&qc)?;
        validate_lane_block_qc_aggregate(&qc, pops).map_err(LaneBlockSessionError::InvalidQc)?;
        self.insert_validated_qc(qc)
    }

    fn validate_qc_session_preconditions(
        &self,
        qc: &LaneBlockQcV1,
    ) -> Result<(), LaneBlockSessionError> {
        let key = LaneBlockSessionKey::from_vote_body(&qc.body);
        if let Some(session) = self.sessions.get(&key) {
            validate_qc_matches_session(qc, session)
        } else if qc.payload_availability_qc.is_some() {
            Err(LaneBlockSessionError::AvailabilityNotAuthorized)
        } else if qc.body.phase == CertPhase::Commit {
            Err(LaneBlockSessionError::CommitBeforePrepareQc)
        } else {
            Ok(())
        }
    }

    fn insert_validated_qc(
        &mut self,
        qc: LaneBlockQcV1,
    ) -> Result<LaneBlockSessionInsertOutcome, LaneBlockSessionError> {
        let key = LaneBlockSessionKey::from_vote_body(&qc.body);

        if let Some(session) = self.sessions.get(&key) {
            validate_qc_matches_session(&qc, session)?;
            let slot = qc_for_phase(session, qc.body.phase).ok_or(
                LaneBlockSessionError::InvalidQc(LaneBlockQcIngressError::InvalidBody),
            )?;
            if let Some(existing) = slot.as_ref() {
                if existing == &qc {
                    return Ok(LaneBlockSessionInsertOutcome::Duplicate);
                }
                if lane_block_qc_certifies_same_body(existing, &qc) {
                    return Ok(LaneBlockSessionInsertOutcome::Duplicate);
                }
                return Err(LaneBlockSessionError::ConflictingQc);
            }
        } else {
            validate_qc_matches_session(&qc, &LaneBlockSession::default())?;
        }
        self.validate_commit_qc_locks(&qc)?;

        self.touch(key);
        {
            let session = self.sessions.entry(key).or_default();
            let slot = qc_for_phase_mut(session, qc.body.phase).ok_or(
                LaneBlockSessionError::InvalidQc(LaneBlockQcIngressError::InvalidBody),
            )?;
            *slot = Some(qc.clone());
            refresh_commit_vote_request_ready(session);
            refresh_committed_session_ready(session);
        }
        self.record_commit_qc_locks(&qc);
        self.evict();
        Ok(LaneBlockSessionInsertOutcome::Inserted)
    }

    fn validate_commit_vote_lock(
        &self,
        vote: &LaneBlockVoteV1,
    ) -> Result<(), LaneBlockSessionError> {
        if vote.body.phase != CertPhase::Commit {
            return Ok(());
        }
        let lock_key = (
            LaneBlockCommitSlotKey::from_vote_body(&vote.body),
            vote.signer.clone(),
        );
        if let Some(existing_hash) = self.commit_vote_locks.get(&lock_key)
            && *existing_hash != vote.body.proposal_hash
        {
            return Err(LaneBlockSessionError::ConflictingVote);
        }
        Ok(())
    }

    fn record_commit_vote_lock(&mut self, vote: &LaneBlockVoteV1) {
        if vote.body.phase != CertPhase::Commit {
            return;
        }
        self.commit_vote_locks.insert(
            (
                LaneBlockCommitSlotKey::from_vote_body(&vote.body),
                vote.signer.clone(),
            ),
            vote.body.proposal_hash,
        );
    }

    fn validate_commit_qc_locks(&self, qc: &LaneBlockQcV1) -> Result<(), LaneBlockSessionError> {
        if qc.body.phase != CertPhase::Commit {
            return Ok(());
        }
        for signer in qc_signers(qc) {
            let lock_key = (
                LaneBlockCommitSlotKey::from_vote_body(&qc.body),
                signer.clone(),
            );
            if let Some(existing_hash) = self.commit_vote_locks.get(&lock_key)
                && *existing_hash != qc.body.proposal_hash
            {
                return Err(LaneBlockSessionError::ConflictingVote);
            }
        }
        Ok(())
    }

    fn record_commit_qc_locks(&mut self, qc: &LaneBlockQcV1) {
        if qc.body.phase != CertPhase::Commit {
            return;
        }
        let slot = LaneBlockCommitSlotKey::from_vote_body(&qc.body);
        for signer in qc_signers(qc) {
            self.commit_vote_locks
                .insert((slot, signer), qc.body.proposal_hash);
        }
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

    fn remove_slot_conflict(&mut self, slot_key: LaneBlockSlotKey, existing_hash: Hash) {
        let existing_key = LaneBlockSessionKey {
            lane_id: slot_key.lane_id,
            dataspace_id: slot_key.dataspace_id,
            lane_incarnation: slot_key.lane_incarnation,
            lane_block_height: slot_key.lane_block_height,
            lane_block_view: slot_key.lane_block_view,
            proposal_hash: existing_hash,
        };
        self.sessions.remove(&existing_key);
        self.order.retain(|ordered| *ordered != existing_key);
        self.slot_proposals.remove(&slot_key);
        self.rebuild_entrypoint_claims();
    }

    fn unprotected_session_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|session| !session_is_eviction_protected(session))
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
            if session_is_eviction_protected(session) {
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
            self.rebuild_entrypoint_claims();
            return true;
        }
        false
    }

    fn ensure_entrypoints_available(
        &self,
        proposal: &LaneBlockProposalV1,
        key: LaneBlockSessionKey,
    ) -> Result<(), LaneBlockSessionError> {
        for entrypoint_hash in &proposal.descriptor.accepted_transaction_hashes {
            let Some(owner) = self.entrypoint_claims.get(entrypoint_hash) else {
                continue;
            };
            // A NewView certificate moves one immutable executable payload to
            // another view. Those proposals intentionally share entrypoints;
            // only an exact view-neutral payload match may share the claim.
            let shares_exact_payload = self
                .sessions
                .get(owner)
                .and_then(|session| session.proposal.as_ref())
                .is_some_and(|owner_proposal| {
                    same_lane_block_executable_payload(owner_proposal, proposal)
                });
            if *owner != key && !shares_exact_payload {
                return Err(LaneBlockSessionError::EntrypointAlreadyClaimed);
            }
        }
        Ok(())
    }

    fn rebuild_entrypoint_claims(&mut self) {
        self.entrypoint_claims.clear();
        for (key, session) in &self.sessions {
            let Some(proposal) = &session.proposal else {
                continue;
            };
            for entrypoint_hash in &proposal.descriptor.accepted_transaction_hashes {
                self.entrypoint_claims
                    .entry(*entrypoint_hash)
                    .or_insert(*key);
            }
        }
    }
}

fn same_lane_block_executable_payload(
    left: &LaneBlockProposalV1,
    right: &LaneBlockProposalV1,
) -> bool {
    let left = &left.descriptor;
    let right = &right.descriptor;
    left.lane_id == right.lane_id
        && left.dataspace_id == right.dataspace_id
        && left.lane_incarnation == right.lane_incarnation
        && left.proposal_height == right.proposal_height
        && left.previous_lane_block_height == right.previous_lane_block_height
        && left.previous_lane_block_descriptor_hash == right.previous_lane_block_descriptor_hash
        && left.lane_block_height == right.lane_block_height
        && left.accepted_candidate_indices == right.accepted_candidate_indices
        && left.accepted_transaction_hashes == right.accepted_transaction_hashes
        && left.validator_set_hash_version == right.validator_set_hash_version
        && left.validator_set_hash == right.validator_set_hash
        && left.validator_set == right.validator_set
        && left.validator_count == right.validator_count
        && left.min_quorum == right.min_quorum
        && left.qc_mode_tag == right.qc_mode_tag
}

impl Default for LaneBlockSessionCache {
    fn default() -> Self {
        Self::new(128)
    }
}

/// Validate a committed lane-block session recovered from durable certified sidecars.
///
/// This is the stateless restart/recovery counterpart to cache insertion:
/// callers without live PoP material can still reject malformed proposals,
/// malformed certificate bodies, wrong phase slots, missing aggregate
/// signatures, and proposal/QC committee drift before queueing execution work.
pub(crate) fn validate_committed_lane_block_session(
    session: &CommittedLaneBlockSession,
) -> Result<(), LaneBlockSessionError> {
    validate_lane_block_proposal(&session.proposal)
        .map_err(LaneBlockSessionError::InvalidProposal)?;
    validate_lane_block_qc(&session.prepare_qc).map_err(LaneBlockSessionError::InvalidQc)?;
    validate_lane_block_qc(&session.commit_qc).map_err(LaneBlockSessionError::InvalidQc)?;
    if session.prepare_qc.body.phase != CertPhase::Prepare
        || session.commit_qc.body.phase != CertPhase::Commit
    {
        return Err(LaneBlockSessionError::QcProposalMismatch);
    }
    validate_qc_matches_proposal(&session.prepare_qc, &session.proposal)?;
    validate_qc_matches_proposal(&session.commit_qc, &session.proposal)?;
    Ok(())
}

impl LaneBlockSessionKey {
    fn from_proposal(proposal: &LaneBlockProposalV1) -> Self {
        let descriptor = &proposal.descriptor;
        Self {
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            proposal_hash: proposal.proposal_hash,
        }
    }

    fn from_vote_body(body: &LaneBlockVoteBodyV1) -> Self {
        Self {
            lane_id: body.lane_id,
            dataspace_id: body.dataspace_id,
            lane_incarnation: body.lane_incarnation,
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
            lane_incarnation: key.lane_incarnation,
            lane_block_height: key.lane_block_height,
            lane_block_view: key.lane_block_view,
        }
    }
}

impl LaneBlockCommitSlotKey {
    fn from_vote_body(body: &LaneBlockVoteBodyV1) -> Self {
        Self {
            lane_id: body.lane_id,
            dataspace_id: body.dataspace_id,
            lane_incarnation: body.lane_incarnation,
            lane_block_height: body.lane_block_height,
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

fn votes_for_phase(
    session: &LaneBlockSession,
    phase: CertPhase,
) -> Option<&BTreeMap<PeerId, LaneBlockVoteV1>> {
    match phase {
        CertPhase::Prepare => Some(&session.prepare_votes),
        CertPhase::Commit => Some(&session.commit_votes),
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

fn qc_for_phase(session: &LaneBlockSession, phase: CertPhase) -> Option<&Option<LaneBlockQcV1>> {
    match phase {
        CertPhase::Prepare => Some(&session.prepare_qc),
        CertPhase::Commit => Some(&session.commit_qc),
        CertPhase::NewView => None,
    }
}

fn proposal_vote_body(proposal: &LaneBlockProposalV1, phase: CertPhase) -> LaneBlockVoteBodyV1 {
    proposal.vote_body(phase)
}

fn qc_signers(qc: &LaneBlockQcV1) -> Vec<PeerId> {
    let mut signers = Vec::new();
    for (byte_index, byte) in qc.signers_bitmap.iter().copied().enumerate() {
        if byte == 0 {
            continue;
        }
        for bit in 0..8 {
            if byte & (1_u8 << bit) == 0 {
                continue;
            }
            let signer_index = byte_index * 8 + bit;
            if let Some(signer) = qc.validator_set.get(signer_index) {
                signers.push(signer.clone());
            }
        }
    }
    signers
}

#[cfg(test)]
fn session_proposal_height(session: &LaneBlockSession) -> Option<u64> {
    session
        .proposal
        .as_ref()
        .map(|proposal| proposal.descriptor.proposal_height)
        .or_else(|| {
            session
                .prepare_votes
                .values()
                .next()
                .map(|vote| vote.body.proposal_height)
        })
        .or_else(|| {
            session
                .commit_votes
                .values()
                .next()
                .map(|vote| vote.body.proposal_height)
        })
        .or_else(|| {
            session
                .prepare_qc
                .as_ref()
                .map(|qc| qc.body.proposal_height)
        })
        .or_else(|| session.commit_qc.as_ref().map(|qc| qc.body.proposal_height))
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
    match &vote.payload_availability_vote {
        Some(availability_vote) => {
            if vote.body.phase != CertPhase::Prepare
                || availability_vote.signer != vote.signer
                || validate_availability_body_matches_proposal(&availability_vote.body, proposal)
                    .is_err()
                || availability_vote
                    .validate_against_validator_set(&proposal.descriptor.validator_set)
                    .is_err()
            {
                return Err(LaneBlockSessionError::AvailabilityMismatch);
            }
        }
        None => {}
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
    match &qc.payload_availability_qc {
        Some(availability_qc) => {
            if qc.body.phase != CertPhase::Prepare
                || validate_availability_body_matches_proposal(&availability_qc.body, proposal)
                    .is_err()
                || validate_lane_payload_availability_qc(availability_qc).is_err()
            {
                return Err(LaneBlockSessionError::AvailabilityMismatch);
            }
        }
        None => {}
    }
    Ok(())
}

fn validate_vote_matches_session(
    vote: &LaneBlockVoteV1,
    session: &LaneBlockSession,
) -> Result<(), LaneBlockSessionError> {
    match vote.body.phase {
        CertPhase::Prepare => match (
            session.payload_availability_body.as_ref(),
            vote.payload_availability_vote.as_ref(),
        ) {
            (Some(expected), Some(actual)) if &actual.body == expected => {}
            (None, None) => {}
            (None, Some(_)) => return Err(LaneBlockSessionError::AvailabilityNotAuthorized),
            _ => return Err(LaneBlockSessionError::AvailabilityMismatch),
        },
        CertPhase::Commit => {
            if vote.payload_availability_vote.is_some() {
                return Err(LaneBlockSessionError::AvailabilityMismatch);
            }
            if session.prepare_qc.is_none() {
                return Err(LaneBlockSessionError::CommitBeforePrepareQc);
            }
        }
        CertPhase::NewView => {
            return Err(LaneBlockSessionError::InvalidVote(
                LaneBlockVoteIngressError::InvalidBody,
            ));
        }
    }
    if let Some(proposal) = &session.proposal {
        validate_vote_matches_proposal(vote, proposal)?;
    }
    Ok(())
}

fn validate_qc_matches_session(
    qc: &LaneBlockQcV1,
    session: &LaneBlockSession,
) -> Result<(), LaneBlockSessionError> {
    match qc.body.phase {
        CertPhase::Prepare => match (
            session.payload_availability_body.as_ref(),
            qc.payload_availability_qc.as_ref(),
        ) {
            (Some(expected), Some(actual)) if &actual.body == expected => {}
            (None, None) => {}
            (None, Some(_)) => return Err(LaneBlockSessionError::AvailabilityNotAuthorized),
            _ => return Err(LaneBlockSessionError::AvailabilityMismatch),
        },
        CertPhase::Commit => {
            if qc.payload_availability_qc.is_some() {
                return Err(LaneBlockSessionError::AvailabilityMismatch);
            }
            if session.prepare_qc.is_none() {
                return Err(LaneBlockSessionError::CommitBeforePrepareQc);
            }
        }
        CertPhase::NewView => {
            return Err(LaneBlockSessionError::InvalidQc(
                LaneBlockQcIngressError::InvalidBody,
            ));
        }
    }
    if let Some(proposal) = &session.proposal {
        validate_qc_matches_proposal(qc, proposal)?;
    }
    Ok(())
}

fn lane_block_qc_certifies_same_body(left: &LaneBlockQcV1, right: &LaneBlockQcV1) -> bool {
    left.body == right.body
        && left.validator_set_hash_version == right.validator_set_hash_version
        && left.validator_set_hash == right.validator_set_hash
        && left.validator_set == right.validator_set
        && left.payload_availability_qc == right.payload_availability_qc
}

fn reconcile_session_with_proposal(session: &mut LaneBlockSession, proposal: &LaneBlockProposalV1) {
    let availability_body = session.payload_availability_body.clone();
    let has_prepare_qc = session.prepare_qc.is_some();
    for phase in [CertPhase::Prepare, CertPhase::Commit] {
        if let Some(votes) = votes_for_phase_mut(session, phase) {
            votes.retain(|_, vote| {
                if validate_vote_matches_proposal(vote, proposal).is_err() {
                    return false;
                }
                match phase {
                    CertPhase::Prepare => match (
                        availability_body.as_ref(),
                        vote.payload_availability_vote.as_ref(),
                    ) {
                        (Some(expected), Some(actual)) => &actual.body == expected,
                        (None, None) => true,
                        _ => false,
                    },
                    CertPhase::Commit => vote.payload_availability_vote.is_none() && has_prepare_qc,
                    CertPhase::NewView => false,
                }
            });
        }
        if let Some(slot) = qc_for_phase_mut(session, phase) {
            let keep_qc = slot.as_ref().is_none_or(|qc| {
                if validate_qc_matches_proposal(qc, proposal).is_err() {
                    return false;
                }
                match phase {
                    CertPhase::Prepare => match (
                        availability_body.as_ref(),
                        qc.payload_availability_qc.as_ref(),
                    ) {
                        (Some(expected), Some(actual)) => &actual.body == expected,
                        (None, None) => true,
                        _ => false,
                    },
                    CertPhase::Commit => qc.payload_availability_qc.is_none() && has_prepare_qc,
                    CertPhase::NewView => false,
                }
            });
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

fn session_has_commit_evidence(session: &LaneBlockSession) -> bool {
    !session.commit_votes.is_empty() || session.commit_qc.is_some()
}

fn session_has_live_commit_evidence(session: &LaneBlockSession) -> bool {
    session_has_commit_evidence(session) && !session.committed_session_drained
}

fn session_is_eviction_protected(session: &LaneBlockSession) -> bool {
    session_has_live_commit_evidence(session)
}

#[cfg(test)]
fn session_has_consensus_evidence(session: &LaneBlockSession) -> bool {
    !session.prepare_votes.is_empty()
        || !session.commit_votes.is_empty()
        || session.prepare_qc.is_some()
        || session.commit_qc.is_some()
}

fn session_has_quorum_certificate(session: &LaneBlockSession) -> bool {
    session.prepare_qc.is_some() || session.commit_qc.is_some()
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
        debug!(
            lane_id = ?proposal.descriptor.lane_id,
            dataspace_id = ?proposal.descriptor.dataspace_id,
            lane_block_height = proposal.descriptor.lane_block_height,
            lane_block_view = proposal.descriptor.lane_block_view,
            phase = ?phase,
            proposal_hash = ?proposal.proposal_hash,
            vote_count = votes.len(),
            min_quorum,
            "lane-block QC not sealed yet; vote quorum incomplete"
        );
        return;
    }
    match aggregate_lane_block_votes_to_qc(
        proposal_vote_body(&proposal, phase),
        proposal.descriptor.validator_set,
        &votes,
    ) {
        Ok(qc) => {
            debug!(
                lane_id = ?proposal.descriptor.lane_id,
                dataspace_id = ?proposal.descriptor.dataspace_id,
                lane_block_height = proposal.descriptor.lane_block_height,
                lane_block_view = proposal.descriptor.lane_block_view,
                phase = ?phase,
                proposal_hash = ?proposal.proposal_hash,
                vote_count = votes.len(),
                min_quorum,
                "sealed lane-block QC from cached votes"
            );
            if let Some(slot) = qc_for_phase_mut(session, phase) {
                *slot = Some(qc);
            }
            match phase {
                CertPhase::Prepare => session.pending_prepare_qc_broadcast = true,
                CertPhase::Commit => session.pending_commit_qc_broadcast = true,
                CertPhase::NewView => {}
            }
        }
        Err(err) => {
            debug!(
                ?err,
                lane_id = ?proposal.descriptor.lane_id,
                dataspace_id = ?proposal.descriptor.dataspace_id,
                lane_block_height = proposal.descriptor.lane_block_height,
                lane_block_view = proposal.descriptor.lane_block_view,
                phase = ?phase,
                proposal_hash = ?proposal.proposal_hash,
                vote_count = votes.len(),
                min_quorum,
                "failed to seal lane-block QC from cached votes"
            );
        }
    }
}

fn peer_uses_bls_normal(peer: &PeerId) -> bool {
    peer.public_key()
        .try_algorithm()
        .is_ok_and(|algorithm| algorithm == Algorithm::BlsNormal)
}

impl LaneBlockVoteV1 {
    /// Validate bounded vote shape before any BLS operation.
    pub fn validate_ingress_shape(
        &self,
        expected_phase: CertPhase,
    ) -> Result<(), LaneBlockVoteIngressError> {
        validate_lane_block_vote_body_shape(&self.body)?;
        if self.body.phase != expected_phase {
            return Err(LaneBlockVoteIngressError::PhaseMismatch {
                expected: expected_phase,
                actual: self.body.phase,
            });
        }
        if !peer_uses_bls_normal(&self.signer) {
            return Err(LaneBlockVoteIngressError::SignerNotBlsNormal);
        }
        if self.bls_signature.len() != LANE_BLS_PROOF_BYTES {
            return Err(LaneBlockVoteIngressError::InvalidSignature);
        }
        if let Some(availability) = &self.payload_availability_vote {
            if self.body.phase != CertPhase::Prepare
                || availability.signer != self.signer
                || !availability_body_matches_lane_vote_body(&availability.body, &self.body)
            {
                return Err(LaneBlockVoteIngressError::InvalidAvailability);
            }
            availability
                .validate_shape()
                .map_err(|_| LaneBlockVoteIngressError::InvalidAvailability)?;
        }
        Ok(())
    }

    fn verify_signatures(&self) -> Result<(), LaneBlockVoteIngressError> {
        Signature::try_from_bytes(&self.bls_signature)
            .map_err(|_| LaneBlockVoteIngressError::InvalidSignature)?
            .verify(self.signer.public_key(), &self.body.signature_preimage())
            .map_err(|_| LaneBlockVoteIngressError::InvalidSignature)?;
        if let Some(availability) = &self.payload_availability_vote {
            availability
                .verify_signature()
                .map_err(|_| LaneBlockVoteIngressError::InvalidAvailability)?;
        }
        Ok(())
    }

    /// Validate phase, BLS-normal identity, and vote signature.
    ///
    /// This is the stateless ingress prefilter. Callers that know the current
    /// world state must still verify that the signer belongs to the live lane
    /// committee and has a live proof of possession at the lane block height.
    ///
    /// # Errors
    ///
    /// Returns an error when the vote is carried by the wrong phase message,
    /// the signer is not BLS-normal, or the BLS signature does not verify
    /// against the canonical lane-block vote preimage.
    pub fn validate_ingress(
        &self,
        expected_phase: CertPhase,
    ) -> Result<(), LaneBlockVoteIngressError> {
        self.validate_ingress_shape(expected_phase)?;
        self.verify_signatures()
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
    /// lane block vote signer is not a BLS-normal consensus identity
    #[error("lane block vote signer is not BLS-normal")]
    SignerNotBlsNormal,
    /// lane block vote signature is missing, malformed, or invalid
    #[error("lane block vote signature is invalid")]
    InvalidSignature,
    /// paired autonomous payload READY vote is malformed or invalid
    #[error("lane payload availability vote is invalid")]
    InvalidAvailability,
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
    /// payload availability proof is present in a phase where it is forbidden
    #[error("lane block QC carries an unexpected payload availability proof")]
    UnexpectedAvailabilityQc,
    /// autonomous payload availability proof is malformed or invalid
    #[error("lane block QC payload availability proof is invalid")]
    InvalidAvailabilityQc,
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
    /// autonomous prepare votes do not carry one exact READY body
    #[error("lane block prepare votes carry mismatched payload availability evidence")]
    AvailabilityMismatch,
    /// autonomous READY vote aggregation failed
    #[error("failed to aggregate lane payload availability votes")]
    AvailabilityAggregate,
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
    if descriptor.proposal_height == 0
        || descriptor.lane_block_height == 0
        || protocol_hash_bytes_are_zero(descriptor.lane_incarnation.as_ref())
        || descriptor.qc_mode_tag.trim().is_empty()
        || descriptor.accepted_candidate_indices.is_empty()
        || descriptor.accepted_candidate_indices.len() > MAX_LANE_EXECUTABLE_ENTRYPOINTS
        || descriptor.accepted_candidate_indices.len()
            != descriptor.accepted_transaction_hashes.len()
        || descriptor.previous_lane_block_height == 0
            && descriptor.previous_lane_block_descriptor_hash.is_some()
        || descriptor.previous_lane_block_height > 0
            && descriptor.previous_lane_block_descriptor_hash.is_none()
        || descriptor.previous_lane_block_height.checked_add(1)
            != Some(descriptor.lane_block_height)
        || descriptor.min_quorum == 0
        || descriptor.validator_count == 0
        || descriptor.min_quorum > descriptor.validator_count
    {
        return Err(LaneBlockProposalIngressError::InvalidBody);
    }
    if descriptor
        .accepted_candidate_indices
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
        != descriptor.accepted_candidate_indices.len()
        || descriptor
            .accepted_transaction_hashes
            .iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .len()
            != descriptor.accepted_transaction_hashes.len()
    {
        return Err(LaneBlockProposalIngressError::InvalidBody);
    }
    validate_lane_block_validator_set_fields(
        descriptor.validator_set_hash_version,
        descriptor.validator_set_hash,
        descriptor.validator_count,
        descriptor.min_quorum,
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

    match &qc.payload_availability_qc {
        Some(availability_qc) => {
            if qc.body.phase != CertPhase::Prepare {
                return Err(LaneBlockQcIngressError::UnexpectedAvailabilityQc);
            }
            if !availability_body_matches_lane_vote_body(&availability_qc.body, &qc.body)
                || availability_qc.validator_set_hash_version != qc.validator_set_hash_version
                || availability_qc.validator_set_hash != qc.validator_set_hash
                || availability_qc.validator_set != qc.validator_set
            {
                return Err(LaneBlockQcIngressError::InvalidAvailabilityQc);
            }
            validate_lane_payload_availability_qc(availability_qc)
                .map_err(|_| LaneBlockQcIngressError::InvalidAvailabilityQc)?;
        }
        None => {}
    }

    let expected_bitmap_len = qc.validator_set.len().div_ceil(8);
    if qc.signers_bitmap.len() != expected_bitmap_len {
        return Err(LaneBlockQcIngressError::SignerBitmapLengthMismatch);
    }
    if qc.bls_aggregate_signature.len() != LANE_BLS_PROOF_BYTES {
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
/// Returns an error when the body, validator set, or canonical commit threshold
/// is malformed, votes do not match `body`, include duplicate or unknown
/// signers, fail to meet `body.min_quorum`, or cannot be aggregated as
/// BLS-normal signatures.
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

    let payload_availability_qc = if votes
        .iter()
        .any(|vote| vote.payload_availability_vote.is_some())
    {
        if body.phase != CertPhase::Prepare
            || votes
                .iter()
                .any(|vote| vote.payload_availability_vote.is_none())
        {
            return Err(LaneBlockQcBuildError::AvailabilityMismatch);
        }
        let availability_votes = votes
            .iter()
            .filter_map(|vote| vote.payload_availability_vote.clone())
            .collect::<Vec<_>>();
        let availability_body = availability_votes
            .first()
            .map(|vote| vote.body.clone())
            .ok_or(LaneBlockQcBuildError::AvailabilityMismatch)?;
        if !availability_body_matches_lane_vote_body(&availability_body, &body)
            || availability_votes
                .iter()
                .any(|vote| vote.body != availability_body)
        {
            return Err(LaneBlockQcBuildError::AvailabilityMismatch);
        }
        Some(
            aggregate_lane_payload_availability_votes(
                availability_body,
                validator_set.clone(),
                &availability_votes,
            )
            .map_err(|_| LaneBlockQcBuildError::AvailabilityAggregate)?,
        )
    } else {
        None
    };

    Ok(LaneBlockQcV1 {
        body,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set,
        signers_bitmap,
        bls_aggregate_signature,
        payload_availability_qc,
    })
}

fn validate_lane_block_vote_body_shape(
    body: &LaneBlockVoteBodyV1,
) -> Result<(), LaneBlockVoteIngressError> {
    let validator_count = usize::try_from(body.validator_count)
        .map_err(|_| LaneBlockVoteIngressError::InvalidBody)?;
    if body.phase == CertPhase::NewView
        || body.proposal_height == 0
        || body.lane_block_height == 0
        || protocol_hash_bytes_are_zero(body.lane_incarnation.as_ref())
        || body.qc_mode_tag.trim().is_empty()
        || body.accepted_candidate_indices.is_empty()
        || body.accepted_candidate_indices.len() > MAX_LANE_EXECUTABLE_ENTRYPOINTS
        || body.accepted_candidate_indices.len() != body.accepted_transaction_hashes.len()
        || body.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1
        || validator_count == 0
        || validator_count > MAX_LANE_BLOCK_VALIDATORS
        || body.min_quorum == 0
        || body.min_quorum > body.validator_count
        || usize::try_from(body.min_quorum).ok()
            != Some(crate::sumeragi::network_topology::commit_quorum_from_len(
                validator_count,
            ))
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
        body.min_quorum,
        validator_set,
    )
}

fn validate_lane_block_validator_set_fields(
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_count: u32,
    min_quorum: u32,
    validator_set: &[PeerId],
) -> Result<(), LaneBlockQcBuildError> {
    if validator_set.is_empty() {
        return Err(LaneBlockQcBuildError::EmptyValidatorSet);
    }
    if validator_set.len() > MAX_LANE_BLOCK_VALIDATORS {
        return Err(LaneBlockQcBuildError::ValidatorCountMismatch);
    }
    let actual_validator_count = u32::try_from(validator_set.len())
        .map_err(|_| LaneBlockQcBuildError::ValidatorCountMismatch)?;
    if actual_validator_count != validator_count {
        return Err(LaneBlockQcBuildError::ValidatorCountMismatch);
    }
    let expected_quorum = u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
        validator_set.len(),
    ))
    .map_err(|_| LaneBlockQcBuildError::InvalidBody)?;
    if min_quorum != expected_quorum {
        return Err(LaneBlockQcBuildError::InvalidBody);
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
    use std::{
        collections::{BTreeMap, BTreeSet},
        time::{Duration, Instant},
    };

    use iroha_crypto::{Hash, HashOf, KeyPair, PublicKey, bls_normal_pop_prove};
    use iroha_data_model::{
        account::AccountId,
        block::{
            Header as BlockHeader,
            consensus::{
                LaneBlockDescriptorV1, LaneBlockProposalPayloadHintV1, LaneBlockProposalV1,
            },
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        isi::InstructionBox,
        nexus::{DataSpaceId, LaneId},
        transaction::signed::{ExecutionStep, TransactionEntrypoint},
        trigger::time::TimeTriggerEntrypoint,
    };
    use iroha_primitives::const_vec::ConstVec;

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
            payload_availability_vote: None,
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

    fn lane_drain_fixture(keypairs: &[KeyPair]) -> (LaneDrainCertificateBodyV1, Vec<PeerId>) {
        let mut validator_set = keypairs.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let validator_count = u32::try_from(validator_set.len()).expect("fixture count fits");
        let min_quorum = u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
            validator_set.len(),
        ))
        .expect("fixture quorum fits");
        (
            LaneDrainCertificateBodyV1 {
                version: 1,
                intent: LaneDrainIntentV1 {
                    version: 1,
                    network_id: network_id(b"lane-drain-genesis"),
                    lane_id: LaneId::new(7),
                    dataspace_id: DataSpaceId::new(9),
                    lane_incarnation: Hash::new(b"lane-drain-incarnation"),
                    close_global_height: 41,
                    initial_frontier: LaneDrainFrontierV1::ordinary(
                        LaneId::new(7),
                        DataSpaceId::new(9),
                        Hash::new(b"lane-drain-incarnation"),
                        3,
                        Some(Hash::new(b"lane-drain-initial-tip")),
                    ),
                    validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                    validator_set_hash: HashOf::new(&validator_set),
                    validator_set: validator_set.clone(),
                    validator_count,
                    min_quorum,
                },
                final_frontier: LaneDrainFrontierV1::ordinary(
                    LaneId::new(7),
                    DataSpaceId::new(9),
                    Hash::new(b"lane-drain-incarnation"),
                    5,
                    Some(Hash::new(b"lane-drain-final-tip")),
                ),
            },
            validator_set,
        )
    }

    fn native_drain_frontier_fixture() -> LaneDrainFrontierV1 {
        LaneDrainFrontierV1 {
            version: LaneDrainFrontierV1::VERSION,
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(9),
            lane_incarnation: Hash::new(b"lane-drain-incarnation"),
            lane_block_height: 5,
            lane_block_descriptor_hash: Some(Hash::new(b"lane-drain-final-tip")),
            native_application: Some(iroha_data_model::merge::LaneDrainNativeFrontierEvidenceV1 {
                version: 1,
                participant_view: 3,
                predecessor_height: 4,
                predecessor_descriptor_hash: Some(Hash::new(b"lane-drain-native-predecessor")),
                participant_proposal_hash: Hash::new(b"lane-drain-native-proposal"),
                participant_settlement_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"lane-drain-native-settlement",
                )),
                source_count: 2,
                application_block_height: 51,
                application_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"lane-drain-native-application-block",
                )),
                executed_block_wire_hash: Hash::new(b"lane-drain-native-executed-wire"),
                finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"lane-drain-native-finality",
                )),
                application_manifest_root: Hash::new(b"lane-drain-native-manifest-root"),
                application_manifest_leaf_count: 2,
                application_manifest_leaf_index: 1,
                manifest_artifact_hash: Hash::new(b"lane-drain-native-manifest-artifact"),
                receipt_artifact_hash: Hash::new(b"lane-drain-native-receipt-artifact"),
                latest_index_artifact_hash: Hash::new(b"lane-drain-native-latest-index"),
            }),
            unresolved_evidence_root: lane_drain_empty_unresolved_evidence_root(),
        }
    }

    #[test]
    fn lane_drain_native_frontier_fails_closed_on_route_evidence_or_unresolved_drift() {
        let keys = [
            checked_bls_keypair(91),
            checked_bls_keypair(92),
            checked_bls_keypair(93),
            checked_bls_keypair(94),
        ];
        let (mut body, _) = lane_drain_fixture(&keys);
        body.final_frontier = native_drain_frontier_fixture();
        validate_lane_drain_certificate_body(&body).expect("valid Native-derived drain frontier");

        let without_native = {
            let mut changed = body.clone();
            changed.final_frontier.native_application = None;
            changed
        };
        assert_ne!(
            body.signature_preimage(),
            without_native.signature_preimage(),
            "missing Native evidence must change the committee-signed body"
        );

        let mut changed = body.clone();
        changed.final_frontier.lane_incarnation = Hash::new(b"stale-drain-incarnation");
        assert_eq!(
            validate_lane_drain_certificate_body(&changed),
            Err(LaneDrainCertificateError::FrontierRouteMismatch)
        );

        changed = body.clone();
        changed
            .final_frontier
            .native_application
            .as_mut()
            .expect("Native fixture")
            .manifest_artifact_hash = Hash::prehashed([0; Hash::LENGTH]);
        assert_eq!(
            validate_lane_drain_certificate_body(&changed),
            Err(LaneDrainCertificateError::InvalidFrontierEvidence)
        );

        changed = body;
        changed.final_frontier.unresolved_evidence_root = Hash::new(b"unresolved-lane-work");
        assert_eq!(
            validate_lane_drain_certificate_body(&changed),
            Err(LaneDrainCertificateError::UnresolvedEvidence)
        );
    }

    #[test]
    fn lane_drain_certificate_aggregates_exact_quorum_and_verifies_after_restart() {
        let keys = [
            checked_bls_keypair(101),
            checked_bls_keypair(102),
            checked_bls_keypair(103),
            checked_bls_keypair(104),
        ];
        let (body, validator_set) = lane_drain_fixture(&keys);
        let votes = keys[..3]
            .iter()
            .map(|keypair| {
                LaneDrainVoteV1::new_signed(body.clone(), peer(keypair), keypair.private_key())
                    .expect("valid drain vote")
            })
            .collect::<Vec<_>>();
        let certificate = aggregate_lane_drain_votes(body.clone(), validator_set.clone(), &votes)
            .expect("valid drain certificate");

        validate_lane_drain_certificate(&certificate)
            .expect("self-contained certificate verifies after restart");
        assert_eq!(certificate.body, body);
        assert_eq!(certificate.validator_set, validator_set);
        assert_eq!(certificate.signer_proofs.len(), 3);
        assert_eq!(
            certificate
                .signers_bitmap
                .iter()
                .map(|byte| byte.count_ones())
                .sum::<u32>(),
            3
        );

        let encoded = certificate.encode();
        let decoded = LaneDrainCertificateV1::decode(&mut encoded.as_slice())
            .expect("drain certificate round-trips");
        validate_lane_drain_certificate(&decoded)
            .expect("round-tripped drain certificate verifies");
    }

    #[test]
    fn lane_drain_vote_state_accepts_strictly_higher_frontier_refresh() {
        let keys = [
            checked_bls_keypair(111),
            checked_bls_keypair(112),
            checked_bls_keypair(113),
            checked_bls_keypair(114),
        ];
        let (body, _) = lane_drain_fixture(&keys);
        let signer = &keys[0];
        let now = Instant::now();
        let mut state = LaneDrainVoteState::new();
        state.retain_body(Some(body.clone()));
        let initial_vote =
            LaneDrainVoteV1::new_signed(body.clone(), peer(signer), signer.private_key())
                .expect("valid initial drain vote");
        assert_eq!(state.insert_vote(initial_vote, now), Ok(true));

        let mut advanced = body;
        advanced.final_frontier.lane_block_height += 1;
        advanced.final_frontier.lane_block_descriptor_hash =
            Some(Hash::new(b"advanced-drain-frontier"));
        state.retain_body(Some(advanced.clone()));
        let refreshed_vote =
            LaneDrainVoteV1::new_signed(advanced.clone(), peer(signer), signer.private_key())
                .expect("valid refreshed drain vote");

        assert_eq!(
            state.insert_vote(refreshed_vote.clone(), now + Duration::from_secs(1)),
            Ok(true)
        );
        assert_eq!(state.active_body(), Some(&advanced));
        assert_eq!(
            state.votes().get(&refreshed_vote.signer),
            Some(&refreshed_vote)
        );
        assert!(state.remote_equivocators.is_empty());
    }

    #[test]
    fn lane_drain_vote_state_quarantines_off_body_same_or_lower_frontier_conflicts() {
        let keys = [
            checked_bls_keypair(121),
            checked_bls_keypair(122),
            checked_bls_keypair(123),
            checked_bls_keypair(124),
        ];
        let (body, _) = lane_drain_fixture(&keys);
        let signer = &keys[0];

        for (conflicting_height, conflicting_hash) in [
            (
                body.final_frontier.lane_block_height,
                b"same-height-drift".as_slice(),
            ),
            (
                body.final_frontier.lane_block_height - 1,
                b"lower-frontier-regression".as_slice(),
            ),
        ] {
            let now = Instant::now();
            let mut state = LaneDrainVoteState::new();
            state.retain_body(Some(body.clone()));
            let initial_vote =
                LaneDrainVoteV1::new_signed(body.clone(), peer(signer), signer.private_key())
                    .expect("valid initial drain vote");
            assert_eq!(state.insert_vote(initial_vote.clone(), now), Ok(true));

            let mut conflict = body.clone();
            conflict.final_frontier.lane_block_height = conflicting_height;
            conflict.final_frontier.lane_block_descriptor_hash = Some(Hash::new(conflicting_hash));
            let conflicting_vote =
                LaneDrainVoteV1::new_signed(conflict, peer(signer), signer.private_key())
                    .expect("structurally valid conflicting drain vote");
            assert_eq!(
                state.insert_vote(conflicting_vote, now + Duration::from_secs(1)),
                Err("signer equivocated or regressed across drain bodies")
            );
            assert!(state.votes().is_empty());

            state.retain_body(Some(body.clone()));
            assert_eq!(
                state.insert_vote(initial_vote, now + Duration::from_secs(2)),
                Err("signer already equivocated for this drain intent")
            );
            assert!(state.votes().is_empty());
        }
    }

    #[test]
    fn lane_drain_vote_state_reaches_exact_quorum() {
        let keys = [
            checked_bls_keypair(131),
            checked_bls_keypair(132),
            checked_bls_keypair(133),
            checked_bls_keypair(134),
        ];
        let (body, validator_set) = lane_drain_fixture(&keys);
        let now = Instant::now();
        let mut state = LaneDrainVoteState::new();
        state.retain_body(Some(body.clone()));

        for (offset, signer) in keys[..2].iter().enumerate() {
            let vote =
                LaneDrainVoteV1::new_signed(body.clone(), peer(signer), signer.private_key())
                    .expect("valid under-quorum drain vote");
            assert_eq!(
                state.insert_vote(
                    vote,
                    now + Duration::from_secs(u64::try_from(offset).expect("offset fits")),
                ),
                Ok(true)
            );
        }
        let under_quorum = state.votes().values().cloned().collect::<Vec<_>>();
        assert_eq!(
            aggregate_lane_drain_votes(body.clone(), validator_set.clone(), &under_quorum),
            Err(LaneDrainCertificateError::QuorumNotMet)
        );

        let quorum_vote =
            LaneDrainVoteV1::new_signed(body.clone(), peer(&keys[2]), keys[2].private_key())
                .expect("valid quorum drain vote");
        assert_eq!(
            state.insert_vote(quorum_vote, now + Duration::from_secs(2)),
            Ok(true)
        );
        let quorum_votes = state.votes().values().cloned().collect::<Vec<_>>();
        let certificate = aggregate_lane_drain_votes(body, validator_set, &quorum_votes)
            .expect("exact quorum seals a drain certificate");
        state.set_certificate(certificate.clone());
        assert_eq!(state.certificate(), Some(&certificate));
    }

    #[test]
    fn lane_drain_vote_recipients_union_committees_and_exclude_local_peer() {
        let keys = (141_u8..=145).map(checked_bls_keypair).collect::<Vec<_>>();
        let peers = keys.iter().map(peer).collect::<Vec<_>>();
        let local_peer = &peers[0];
        let lane_committee = vec![peers[0].clone(), peers[1].clone(), peers[2].clone()];
        let global_committee = vec![
            peers[2].clone(),
            peers[3].clone(),
            peers[0].clone(),
            peers[4].clone(),
            peers[3].clone(),
        ];
        let expected = lane_committee
            .iter()
            .chain(&global_committee)
            .filter(|peer| *peer != local_peer)
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        assert_eq!(
            lane_drain_vote_recipients(&lane_committee, &global_committee, local_peer),
            expected
        );
    }

    #[test]
    fn lane_drain_vote_maximum_committee_fits_control_plane_envelope() {
        let keys = (0..MAX_LANE_BLOCK_VALIDATORS)
            .map(|index| {
                checked_bls_keypair(
                    u8::try_from(index + 1).expect("one-based validator index fits"),
                )
            })
            .collect::<Vec<_>>();
        let (body, _) = lane_drain_fixture(&keys);
        let vote = LaneDrainVoteV1::new_signed(body, peer(&keys[0]), keys[0].private_key())
            .expect("maximum-committee drain vote is valid");
        let encoded = norito::to_bytes(&vote).expect("drain vote encodes");
        assert!(
            encoded.len() <= MAX_LANE_DRAIN_VOTE_BYTES,
            "maximum-committee drain vote uses {} bytes, above its {}-byte envelope",
            encoded.len(),
            MAX_LANE_DRAIN_VOTE_BYTES
        );
    }

    #[test]
    fn lane_drain_certificate_maximum_committee_fits_persisted_envelope() {
        let keys = (0..MAX_LANE_BLOCK_VALIDATORS)
            .map(|index| {
                checked_bls_keypair(
                    u8::try_from(index + 1).expect("one-based validator index fits"),
                )
            })
            .collect::<Vec<_>>();
        let (body, validator_set) = lane_drain_fixture(&keys);
        let quorum = usize::try_from(body.intent.min_quorum).expect("fixture quorum fits");
        let votes = keys
            .iter()
            .take(quorum)
            .map(|keypair| {
                LaneDrainVoteV1::new_signed(body.clone(), peer(keypair), keypair.private_key())
                    .expect("maximum-committee drain vote is valid")
            })
            .collect::<Vec<_>>();
        let certificate = aggregate_lane_drain_votes(body, validator_set, &votes)
            .expect("maximum-committee drain certificate is valid");
        let encoded = norito::to_bytes(&certificate).expect("drain certificate encodes");

        assert!(
            encoded.len() <= MAX_LANE_DRAIN_CERTIFICATE_BYTES,
            "maximum-committee drain certificate uses {} bytes, above its {}-byte envelope",
            encoded.len(),
            MAX_LANE_DRAIN_CERTIFICATE_BYTES
        );
        validate_lane_drain_certificate(&certificate)
            .expect("maximum-committee certificate verifies from its own evidence");
    }

    #[test]
    fn lane_drain_certificate_rejects_forgery_downgrade_and_conflicting_votes() {
        let keys = [
            checked_bls_keypair(111),
            checked_bls_keypair(112),
            checked_bls_keypair(113),
            checked_bls_keypair(114),
        ];
        let (body, validator_set) = lane_drain_fixture(&keys);
        let votes = keys[..3]
            .iter()
            .map(|keypair| {
                LaneDrainVoteV1::new_signed(body.clone(), peer(keypair), keypair.private_key())
                    .expect("valid drain vote")
            })
            .collect::<Vec<_>>();
        let certificate = aggregate_lane_drain_votes(body.clone(), validator_set.clone(), &votes)
            .expect("valid drain certificate");

        let mut forged_bodies = Vec::new();
        let mut forged = certificate.clone();
        forged.body.intent.network_id = network_id(b"foreign-drain-genesis");
        forged_bodies.push(forged);
        let mut forged = certificate.clone();
        forged.body.intent.lane_id = LaneId::new(8);
        forged.body.intent.initial_frontier.lane_id = LaneId::new(8);
        forged.body.final_frontier.lane_id = LaneId::new(8);
        forged_bodies.push(forged);
        let mut forged = certificate.clone();
        forged.body.intent.dataspace_id = DataSpaceId::new(10);
        forged.body.intent.initial_frontier.dataspace_id = DataSpaceId::new(10);
        forged.body.final_frontier.dataspace_id = DataSpaceId::new(10);
        forged_bodies.push(forged);
        let mut forged = certificate.clone();
        forged.body.intent.lane_incarnation = Hash::new(b"wrong-incarnation");
        forged.body.intent.initial_frontier.lane_incarnation = Hash::new(b"wrong-incarnation");
        forged.body.final_frontier.lane_incarnation = Hash::new(b"wrong-incarnation");
        forged_bodies.push(forged);
        let mut forged = certificate.clone();
        forged.body.intent.close_global_height = 42;
        forged_bodies.push(forged);
        let mut forged = certificate.clone();
        forged.body.final_frontier.lane_block_descriptor_hash = Some(Hash::new(b"wrong-final-tip"));
        forged_bodies.push(forged);
        for forged in forged_bodies {
            assert_eq!(
                validate_lane_drain_certificate(&forged),
                Err(LaneDrainCertificateError::InvalidAggregateSignature),
                "every consensus-field mutation must invalidate the aggregate signature"
            );
        }

        let mut wrong_committee = certificate.clone();
        wrong_committee
            .validator_set
            .pop()
            .expect("certificate carries the four-validator committee");
        assert_eq!(
            validate_lane_drain_certificate(&wrong_committee),
            Err(LaneDrainCertificateError::InvalidValidatorSet)
        );

        let mut under_quorum = certificate.clone();
        let removed_index = usize::try_from(
            under_quorum
                .signer_proofs
                .pop()
                .expect("three signer proofs")
                .signer,
        )
        .expect("signer index fits");
        under_quorum.signers_bitmap[removed_index / 8] &= !(1_u8 << (removed_index % 8));
        assert_eq!(
            validate_lane_drain_certificate(&under_quorum),
            Err(LaneDrainCertificateError::QuorumNotMet)
        );

        let mut padded = certificate.clone();
        padded.signers_bitmap[0] |= 1_u8 << 7;
        assert_eq!(
            validate_lane_drain_certificate(&padded),
            Err(LaneDrainCertificateError::InvalidBitmap)
        );

        let mut oversized_vote = votes[0].clone();
        oversized_vote.bls_signature = vec![0_u8; MAX_LANE_DRAIN_VOTE_BYTES];
        assert_eq!(
            oversized_vote.validate_ingress(),
            Err(LaneDrainCertificateError::VoteTooLarge)
        );
        let mut forged_pop = votes[0].clone();
        forged_pop.proof_of_possession[0] ^= 0x80;
        assert_eq!(
            forged_pop.validate_ingress(),
            Err(LaneDrainCertificateError::InvalidProofOfPossession)
        );

        assert_eq!(
            aggregate_lane_drain_votes(
                body.clone(),
                validator_set.clone(),
                &[votes[0].clone(), votes[0].clone(), votes[1].clone()],
            ),
            Err(LaneDrainCertificateError::DuplicateSigner)
        );
        let mut conflicting_vote = votes[2].clone();
        conflicting_vote.body.final_frontier.lane_block_height = 6;
        conflicting_vote
            .body
            .final_frontier
            .lane_block_descriptor_hash = Some(Hash::new(b"conflicting-final-tip"));
        assert_eq!(
            aggregate_lane_drain_votes(
                body,
                validator_set,
                &[votes[0].clone(), votes[1].clone(), conflicting_vote],
            ),
            Err(LaneDrainCertificateError::BodyMismatch)
        );

        let (mut malformed, _) = lane_drain_fixture(&keys);
        malformed.final_frontier.lane_block_height = 0;
        assert_eq!(
            validate_lane_drain_certificate_body(&malformed),
            Err(LaneDrainCertificateError::FrontierRegression)
        );
        malformed.final_frontier.lane_block_height = 5;
        malformed.final_frontier.lane_block_descriptor_hash = None;
        assert_eq!(
            validate_lane_drain_certificate_body(&malformed),
            Err(LaneDrainCertificateError::FrontierHashMismatch)
        );
        let (mut noncanonical_committee, _) = lane_drain_fixture(&keys);
        noncanonical_committee.intent.validator_set.swap(0, 1);
        noncanonical_committee.intent.validator_set_hash =
            HashOf::new(&noncanonical_committee.intent.validator_set);
        assert_eq!(
            validate_lane_drain_certificate_body(&noncanonical_committee),
            Err(LaneDrainCertificateError::InvalidIntent)
        );
    }

    fn aligned_validator_pops(validator_set: &[PeerId], keypairs: &[KeyPair]) -> Vec<Vec<u8>> {
        validator_set
            .iter()
            .map(|validator| {
                let keypair = keypairs
                    .iter()
                    .find(|keypair| keypair.public_key() == validator.public_key())
                    .expect("fixture validator keypair");
                bls_normal_pop_prove(keypair.private_key()).expect("checked lane block fixture PoP")
            })
            .collect()
    }

    fn signed_autonomous_prepare_vote(
        payload: &LaneExecutablePayloadV1,
        current_proposal: &LaneBlockProposalV1,
        network_id: NetworkId,
        epoch: u64,
        keypair: &KeyPair,
        all_keypairs: &[KeyPair],
    ) -> LaneBlockVoteV1 {
        let body = current_proposal.vote_body(CertPhase::Prepare);
        let mut vote = signed_vote(&body, keypair);
        let availability_body =
            lane_payload_availability_body(payload, current_proposal, network_id, epoch)
                .expect("fixture availability body");
        vote.payload_availability_vote = Some(
            LanePayloadAvailabilityVoteV1::new_signed(
                availability_body,
                peer(keypair),
                aligned_validator_pops(&current_proposal.descriptor.validator_set, all_keypairs),
                keypair.private_key(),
            )
            .expect("fixture READY vote"),
        );
        vote
    }

    fn vote_body(validator_set: &[PeerId]) -> LaneBlockVoteBodyV1 {
        LaneBlockVoteBodyV1 {
            phase: CertPhase::Prepare,
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: Hash::new(b"lane-consensus-fixture-incarnation"),
            proposal_height: 13,
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
            min_quorum: u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
                validator_set.len(),
            ))
            .expect("fixture quorum fits"),
            qc_mode_tag: "permissioned:lane:7:dataspace:11".to_string(),
        }
    }

    fn lane_block_proposal(validator_set: &[PeerId]) -> LaneBlockProposalV1 {
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: Hash::new(b"lane-consensus-fixture-incarnation"),
            proposal_height: 13,
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
            min_quorum: u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
                validator_set.len(),
            ))
            .expect("fixture quorum fits"),
            qc_mode_tag: "permissioned:lane:7:dataspace:11".to_string(),
            descriptor_hash: Hash::prehashed([0x00; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0x00; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    fn autonomous_payload_fixture(
        keypairs: &[KeyPair],
    ) -> (NetworkId, u64, LaneExecutablePayloadV1) {
        let mut validator_set = keypairs.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let entrypoint = TransactionEntrypoint::Time(TimeTriggerEntrypoint {
            id: "lane-autonomous-checkpoint"
                .parse()
                .expect("fixture trigger id"),
            instructions: ExecutionStep(ConstVec::from(Vec::<InstructionBox>::new())),
            authority: AccountId::new(keypairs[0].public_key().clone()),
        });
        let entrypoint_hash = Hash::from(entrypoint.hash());
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: Hash::new(b"lane-autonomous-checkpoint-incarnation"),
            proposal_height: 13,
            previous_lane_block_height: 12,
            previous_lane_block_descriptor_hash: Some(Hash::new(b"lane-autonomous-predecessor")),
            lane_block_height: 13,
            lane_block_view: 0,
            subject_hash: Hash::new(b"lane-autonomous-subject"),
            payload_ownership_hash: Hash::new(b"lane-autonomous-ownership"),
            rbc_instance_hash: Hash::new(b"lane-autonomous-rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![entrypoint_hash],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count: u32::try_from(validator_set.len()).expect("validator count"),
            min_quorum: u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
                validator_set.len(),
            ))
            .expect("fixture quorum"),
            qc_mode_tag: "permissioned:lane:7:dataspace:11".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: Some(LaneBlockProposalPayloadHintV1 {
                proposal_height: 13,
                proposal_view: 3,
                proposal_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"lane-autonomous-anchor",
                )),
            }),
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"lane-autonomous-genesis",
        )));
        let epoch = 4;
        let producer =
            deterministic_lane_author(&validator_set, proposal.descriptor.lane_block_height)
                .cloned()
                .expect("fixture has a deterministic lane author");
        let producer_key = keypairs
            .iter()
            .find(|keypair| keypair.public_key() == producer.public_key())
            .expect("producer fixture key");
        let accepted = AcceptedTransaction::new_unchecked_entrypoint(std::borrow::Cow::Owned(
            entrypoint.clone(),
        ));
        let routing_plan = RoutingPlan::single(crate::queue::RoutingDecision::new(
            proposal.descriptor.lane_id,
            proposal.descriptor.dataspace_id,
        ));
        let reservation = LaneQueueReservationKeyV2 {
            version: LaneQueueReservationKeyV2::VERSION,
            signed_transaction_hash: accepted.hash(),
            entrypoint_hash: entrypoint.hash(),
            queue_plan_admission_binding_hash: Hash::new(
                b"lane-consensus-queue-plan-admission-binding",
            ),
            routing_plan_digest: routing_plan.digest(),
            coordinator_leg: routing_plan.coordinator_leg(),
            lane_id: proposal.descriptor.lane_id,
            dataspace_id: proposal.descriptor.dataspace_id,
            lane_incarnation: proposal.descriptor.lane_incarnation,
            proposal_height: proposal.descriptor.proposal_height,
            lane_block_height: proposal.descriptor.lane_block_height,
            lane_block_view: proposal.descriptor.lane_block_view,
            reservation_owner_hash: Hash::new(b"lane-autonomous-reservation-owner"),
            proposal_identity_hash: proposal.proposal_hash,
        };
        let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
            network_id,
            epoch,
            proposal,
            vec![entrypoint],
            vec![reservation],
            vec![routing_plan],
            vec![None],
            producer,
            producer_key.private_key(),
        )
        .expect("signed autonomous payload");
        (network_id, epoch, payload)
    }

    #[test]
    fn autonomous_payload_v2_is_hint_neutral_and_rejects_other_versions() {
        let keypairs = [
            checked_bls_keypair(71),
            checked_bls_keypair(72),
            checked_bls_keypair(73),
        ];
        let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);
        assert_eq!(payload.version, LANE_EXECUTABLE_PAYLOAD_VERSION_V2);

        let mut hint_free = payload.clone();
        hint_free.origin_proposal.payload_block_hint = None;
        hint_free
            .validate(network_id, epoch)
            .expect("removing the advisory hint preserves authentication");
        let payload_hash = hint_free.payload_hash;
        let producer_signature = hint_free.producer_signature.clone();
        let reservation_bytes = hint_free.reservation_keys.encode();
        let replacement_hint = LaneBlockProposalPayloadHintV1 {
            proposal_height: hint_free.origin_proposal.descriptor.proposal_height,
            proposal_view: 9,
            proposal_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"finalized-autonomous-global-anchor",
            )),
        };
        let attached = hint_free
            .attach_global_hint_exact(replacement_hint, network_id, epoch)
            .expect("the finalized hint does not change authenticated payload identity");
        assert_eq!(
            attached.origin_proposal.payload_block_hint,
            Some(replacement_hint)
        );
        assert_eq!(attached.payload_hash, payload_hash);
        assert_eq!(attached.producer_signature, producer_signature);
        assert_eq!(attached.reservation_keys.encode(), reservation_bytes);
        assert_eq!(
            payload.attach_global_hint_exact(replacement_hint, network_id, epoch),
            Err(LaneAutonomousArtifactError::InvalidGlobalAnchorHint),
            "an already hinted payload cannot be rebound to another carrier"
        );
        let mut zero_height = replacement_hint;
        zero_height.proposal_height = 0;
        assert_eq!(
            hint_free.attach_global_hint_exact(zero_height, network_id, epoch),
            Err(LaneAutonomousArtifactError::InvalidGlobalAnchorHint)
        );
        let mut zero_hash = replacement_hint;
        zero_hash.proposal_block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0; Hash::LENGTH]));
        assert_eq!(
            hint_free.attach_global_hint_exact(zero_hash, network_id, epoch),
            Err(LaneAutonomousArtifactError::InvalidGlobalAnchorHint)
        );

        let mut legacy = payload.clone();
        legacy.version = 1;
        assert_eq!(
            legacy.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::UnsupportedVersion)
        );
        let mut unknown = payload;
        unknown.version = LANE_EXECUTABLE_PAYLOAD_VERSION_V2 + 1;
        assert_eq!(
            unknown.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::UnsupportedVersion)
        );
    }

    #[test]
    fn autonomous_payload_requires_height_rotated_committee_author() {
        let keypairs = [
            checked_bls_keypair(74),
            checked_bls_keypair(75),
            checked_bls_keypair(76),
        ];
        let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);
        let descriptor = &payload.origin_proposal.descriptor;
        assert_eq!(
            deterministic_lane_author(&descriptor.validator_set, descriptor.lane_block_height),
            Some(&payload.producer),
            "the positive fixture must rotate to the exact lane-height author",
        );

        let wrong_key = keypairs
            .iter()
            .find(|keypair| keypair.public_key() != payload.producer.public_key())
            .expect("fixture contains another committee signer");
        let mut wrong_author = payload.clone();
        wrong_author.producer = peer(wrong_key);
        let preimage = wrong_author
            .producer_signature_preimage()
            .expect("canonical preimage");
        wrong_author.producer_signature = Signature::try_new(wrong_key.private_key(), &preimage)
            .expect("wrong lane author can still make a cryptographically valid signature")
            .payload()
            .to_vec();
        assert_eq!(
            wrong_author.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::ProducerNotDeterministicAuthor),
            "committee membership and a valid signature must not confer slot authorship",
        );

        let mut hint_free = payload.clone();
        hint_free.origin_proposal.payload_block_hint = None;
        let attached = hint_free
            .attach_global_hint_exact(
                LaneBlockProposalPayloadHintV1 {
                    proposal_height: descriptor.proposal_height,
                    proposal_view: 91,
                    proposal_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                        b"lane-author-independent-global-view",
                    )),
                },
                network_id,
                epoch,
            )
            .expect("global carrier view must not change autonomous authorship");
        assert_eq!(attached.producer, payload.producer);
        assert_eq!(attached.producer_signature, payload.producer_signature);
    }

    #[test]
    fn autonomous_payload_envelope_is_exact_bounded_and_fail_closed() {
        let keypairs = [
            checked_bls_keypair(81),
            checked_bls_keypair(82),
            checked_bls_keypair(83),
        ];
        let (network_id, epoch, mut payload) = autonomous_payload_fixture(&keypairs);
        payload.origin_proposal.payload_block_hint = None;
        payload
            .validate(network_id, epoch)
            .expect("V2 payload remains authenticated after removing its advisory hint");
        let reservation_bytes = payload.reservation_keys.encode();
        let envelope = autonomous_lane_payload_envelope(&payload, network_id, epoch)
            .expect("hint-free payload encodes into a bounded global anchor");
        let decoded = decode_autonomous_lane_payload_envelope(&envelope, network_id, epoch)
            .expect("canonical autonomous anchor decodes");
        assert_eq!(decoded, payload);
        assert_eq!(decoded.reservation_keys.encode(), reservation_bytes);
        let payload_hash = payload
            .computed_payload_hash()
            .expect("compute canonical payload identity");
        let producer_preimage = payload
            .producer_signature_preimage()
            .expect("producer signature preimage encodes canonically");
        {
            let alternate_flags =
                norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            assert_eq!(
                autonomous_lane_payload_envelope(&payload, network_id, epoch)
                    .expect("encode envelope under alternate ambient layout"),
                envelope,
                "autonomous envelope bytes must ignore the caller's ambient Norito layout"
            );
            assert_eq!(
                decode_autonomous_lane_payload_envelope(&envelope, network_id, epoch)
                    .expect("decode canonical envelope under alternate ambient layout"),
                payload
            );
            assert_eq!(
                payload
                    .computed_payload_hash()
                    .expect("compute payload identity under alternate ambient layout"),
                payload_hash
            );
            assert_eq!(
                payload
                    .producer_signature_preimage()
                    .expect("producer signature preimage encodes under alternate ambient layout"),
                producer_preimage,
                "producer signature identity must ignore the caller's ambient Norito layout"
            );
        }

        let producer_key = keypairs
            .iter()
            .find(|keypair| keypair.public_key() == payload.producer.public_key())
            .expect("payload producer fixture key");
        let nonzero_view_proposal =
            retarget_lane_block_proposal_exact_view(&payload.origin_proposal, 1)
                .expect("construct a canonical nonzero-view payload control");
        let mut nonzero_view_reservations = payload.reservation_keys.clone();
        for reservation in &mut nonzero_view_reservations {
            reservation.lane_block_view = 1;
            reservation.proposal_identity_hash = nonzero_view_proposal.proposal_hash;
        }
        let nonzero_view_payload = LaneExecutablePayloadV1::new_signed_with_reservations(
            network_id,
            epoch,
            nonzero_view_proposal,
            payload.entrypoints.clone(),
            nonzero_view_reservations,
            payload.routing_plans.clone(),
            payload.native_amx_receipts.clone(),
            payload.producer.clone(),
            producer_key.private_key(),
        )
        .expect("construct independently valid nonzero-view payload");
        assert_eq!(
            autonomous_lane_payload_envelope(&nonzero_view_payload, network_id, epoch),
            Err(LaneAutonomousArtifactError::InvalidGlobalAnchorHint)
        );
        let nonzero_view_descriptor = &nonzero_view_payload.origin_proposal.descriptor;
        let mut nonzero_view_envelope = envelope.clone();
        nonzero_view_envelope.lane_block_view = nonzero_view_descriptor.lane_block_view;
        nonzero_view_envelope.proposal_hash = nonzero_view_payload.origin_proposal.proposal_hash;
        nonzero_view_envelope.descriptor_hash = nonzero_view_descriptor.descriptor_hash;
        nonzero_view_envelope.payload_hash = nonzero_view_payload.payload_hash;
        nonzero_view_envelope.producer = nonzero_view_payload.producer.clone();
        nonzero_view_envelope.canonical_payload =
            norito::to_bytes(&nonzero_view_payload).expect("nonzero-view payload encodes");
        assert_eq!(
            decode_autonomous_lane_payload_envelope(&nonzero_view_envelope, network_id, epoch,),
            Err(LaneAutonomousArtifactError::InvalidGlobalAnchorHint)
        );

        let mut legacy_envelope = envelope.clone();
        legacy_envelope.version = 0;
        assert_eq!(
            decode_autonomous_lane_payload_envelope(&legacy_envelope, network_id, epoch),
            Err(LaneAutonomousArtifactError::UnsupportedVersion)
        );

        let mut field_substitution = envelope.clone();
        field_substitution.descriptor_hash = Hash::new(b"substituted-envelope-descriptor");
        assert_eq!(
            decode_autonomous_lane_payload_envelope(&field_substitution, network_id, epoch),
            Err(LaneAutonomousArtifactError::PayloadEnvelopeMismatch)
        );

        let mut substituted_reservations = payload.reservation_keys.clone();
        substituted_reservations[0].reservation_owner_hash =
            Hash::new(b"substituted-reservation-owner");
        let substituted_payload = LaneExecutablePayloadV1::new_signed_with_reservations(
            network_id,
            epoch,
            payload.origin_proposal.clone(),
            payload.entrypoints.clone(),
            substituted_reservations,
            payload.routing_plans.clone(),
            payload.native_amx_receipts.clone(),
            payload.producer.clone(),
            producer_key.private_key(),
        )
        .expect("construct a second independently valid payload body");
        let mut body_substitution = envelope.clone();
        body_substitution.canonical_payload =
            norito::to_bytes(&substituted_payload).expect("substituted payload encodes");
        assert_eq!(
            decode_autonomous_lane_payload_envelope(&body_substitution, network_id, epoch),
            Err(LaneAutonomousArtifactError::PayloadEnvelopeMismatch)
        );

        let mut trailing = envelope.clone();
        trailing.canonical_payload.push(0);
        assert_eq!(
            decode_autonomous_lane_payload_envelope(&trailing, network_id, epoch),
            Err(LaneAutonomousArtifactError::InvalidCanonicalPayloadEncoding)
        );

        let mut noncanonical = envelope.clone();
        noncanonical.canonical_payload =
            norito::to_compressed_bytes(&payload, Some(norito::CompressionConfig::default()))
                .expect("alternate valid payload framing encodes");
        assert_ne!(noncanonical.canonical_payload, envelope.canonical_payload);
        assert_eq!(
            decode_autonomous_lane_payload_envelope(&noncanonical, network_id, epoch),
            Err(LaneAutonomousArtifactError::InvalidCanonicalPayloadEncoding)
        );

        let mut oversized = envelope;
        oversized.canonical_payload = vec![0; MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES + 1];
        assert_eq!(
            decode_autonomous_lane_payload_envelope(&oversized, network_id, epoch),
            Err(LaneAutonomousArtifactError::PayloadEnvelopeByteLimitExceeded)
        );
    }

    fn durable_new_view_certificate(
        source: &LaneBlockProposalV1,
        payload: &LaneExecutablePayloadV1,
        keypairs: &[KeyPair],
        network_id: NetworkId,
        epoch: u64,
    ) -> DurableLaneBlockNewViewCertificateV1 {
        let target_view = source
            .descriptor
            .lane_block_view
            .checked_add(1)
            .expect("fixture view");
        let body =
            LaneBlockNewViewBodyV1::for_transition(source, payload, target_view, network_id, epoch)
                .expect("NewView body");
        let votes = keypairs
            .iter()
            .map(|keypair| {
                LaneBlockNewViewVoteV1::new_signed(
                    body.clone(),
                    peer(keypair),
                    keypair.private_key(),
                )
                .expect("NewView vote")
            })
            .collect::<Vec<_>>();
        let certificate = aggregate_lane_block_new_view_votes(
            body,
            payload.origin_proposal.descriptor.validator_set.clone(),
            &votes,
        )
        .expect("NewView certificate");
        DurableLaneBlockNewViewCertificateV1 {
            certificate,
            signer_pops: signer_pops(keypairs),
        }
    }

    #[test]
    fn payload_availability_deliver_binds_exact_durable_payload() {
        let keypairs = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);
        let body = payload.origin_proposal.vote_body(CertPhase::Prepare);
        let votes = [
            signed_autonomous_prepare_vote(
                &payload,
                &payload.origin_proposal,
                network_id,
                epoch,
                &keypairs[0],
                &keypairs,
            ),
            signed_autonomous_prepare_vote(
                &payload,
                &payload.origin_proposal,
                network_id,
                epoch,
                &keypairs[1],
                &keypairs,
            ),
            signed_autonomous_prepare_vote(
                &payload,
                &payload.origin_proposal,
                network_id,
                epoch,
                &keypairs[2],
                &keypairs,
            ),
        ];
        let certificate = aggregate_lane_block_votes_to_qc(
            body,
            payload.origin_proposal.descriptor.validator_set.clone(),
            &votes,
        )
        .expect("availability READY quorum");
        let durable = DurableLanePayloadAvailabilityCertificateV1 { certificate };
        validate_lane_payload_availability_certificate(&durable, &payload, network_id, epoch)
            .expect("availability DELIVER certificate");

        let mut wrong_payload = durable.clone();
        wrong_payload
            .certificate
            .payload_availability_qc
            .as_mut()
            .expect("availability QC")
            .body
            .executable_payload_hash = Hash::new(b"wrong-availability-payload");
        assert_eq!(
            validate_lane_payload_availability_certificate(
                &wrong_payload,
                &payload,
                network_id,
                epoch,
            ),
            Err(LaneAutonomousArtifactError::AvailabilityMismatch)
        );
        let mut forged = durable.clone();
        forged
            .certificate
            .payload_availability_qc
            .as_mut()
            .expect("availability QC")
            .bls_aggregate_signature[0] ^= 1;
        assert_eq!(
            validate_lane_payload_availability_certificate(&forged, &payload, network_id, epoch,),
            Err(LaneAutonomousArtifactError::InvalidAvailabilityCertificate)
        );
        let mut forged_prepare = durable;
        forged_prepare.certificate.bls_aggregate_signature[0] ^= 1;
        assert_eq!(
            validate_lane_payload_availability_certificate(
                &forged_prepare,
                &payload,
                network_id,
                epoch,
            ),
            Err(LaneAutonomousArtifactError::InvalidAvailabilityCertificate)
        );
    }

    #[test]
    fn payload_availability_qc_rejects_duplicate_bitmap_roster_and_pop_attacks() {
        let keypairs = [
            checked_bls_keypair(31),
            checked_bls_keypair(32),
            checked_bls_keypair(33),
        ];
        let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);
        let outer_votes = [
            signed_autonomous_prepare_vote(
                &payload,
                &payload.origin_proposal,
                network_id,
                epoch,
                &keypairs[0],
                &keypairs,
            ),
            signed_autonomous_prepare_vote(
                &payload,
                &payload.origin_proposal,
                network_id,
                epoch,
                &keypairs[1],
                &keypairs,
            ),
            signed_autonomous_prepare_vote(
                &payload,
                &payload.origin_proposal,
                network_id,
                epoch,
                &keypairs[2],
                &keypairs,
            ),
        ];
        let ready_votes = outer_votes
            .iter()
            .map(|vote| {
                vote.payload_availability_vote
                    .clone()
                    .expect("fixture READY vote")
            })
            .collect::<Vec<_>>();
        let availability_body = ready_votes[0].body.clone();
        let validator_set = payload.origin_proposal.descriptor.validator_set.clone();
        assert!(
            norito::to_bytes(&availability_body)
                .expect("availability body encodes")
                .len()
                <= MAX_LANE_PAYLOAD_AVAILABILITY_BODY_BYTES
        );

        let mut oversized_body = availability_body.clone();
        oversized_body.validator_count =
            u32::try_from(MAX_LANE_BLOCK_VALIDATORS + 1).expect("hard cap fits u32");
        assert_eq!(
            validate_lane_payload_availability_body_shape(&oversized_body),
            Err(LaneAutonomousArtifactError::InvalidAvailabilityBody)
        );
        let mut oversized_domain = availability_body.clone();
        oversized_domain.qc_mode_tag = "x".repeat(MAX_LANE_PAYLOAD_AVAILABILITY_BODY_BYTES + 1);
        assert_eq!(
            validate_lane_payload_availability_body_shape(&oversized_domain),
            Err(LaneAutonomousArtifactError::InvalidAvailabilityBody)
        );
        let mut lane_height_above_proposal = availability_body.clone();
        lane_height_above_proposal.proposal_height = lane_height_above_proposal
            .lane_block_height
            .saturating_sub(1);
        validate_lane_payload_availability_body_shape(&lane_height_above_proposal)
            .expect("availability coordinates use independent global and lane-local heights");

        assert_eq!(
            aggregate_lane_payload_availability_votes(
                availability_body.clone(),
                validator_set.clone(),
                &[ready_votes[0].clone(), ready_votes[0].clone()],
            ),
            Err(LaneAutonomousArtifactError::DuplicateAvailabilitySigner)
        );

        let qc = aggregate_lane_payload_availability_votes(
            availability_body,
            validator_set,
            &ready_votes,
        )
        .expect("valid READY QC");
        assert!(
            norito::to_bytes(&qc)
                .expect("availability QC encodes")
                .len()
                <= MAX_LANE_PAYLOAD_AVAILABILITY_QC_BYTES
        );

        let mut trailing_bit = qc.clone();
        *trailing_bit
            .signers_bitmap
            .last_mut()
            .expect("fixture signer bitmap") |= 0b1000_0000;
        assert_eq!(
            validate_lane_payload_availability_qc(&trailing_bit),
            Err(LaneAutonomousArtifactError::InvalidAvailabilityBitmap)
        );

        let mut below_quorum = qc.clone();
        below_quorum.signers_bitmap = vec![0b0000_0001];
        assert_eq!(
            validate_lane_payload_availability_qc(&below_quorum),
            Err(LaneAutonomousArtifactError::AvailabilityQuorumNotMet)
        );

        let mut duplicate_roster = qc.clone();
        duplicate_roster.validator_set[1] = duplicate_roster.validator_set[0].clone();
        assert_eq!(
            validate_lane_payload_availability_qc(&duplicate_roster),
            Err(LaneAutonomousArtifactError::InvalidAvailabilityBody)
        );

        let mut invalid_pop = qc;
        invalid_pop.validator_set_pops[0][0] ^= 1;
        assert_eq!(
            validate_lane_payload_availability_qc(&invalid_pop),
            Err(LaneAutonomousArtifactError::InvalidAvailabilityPop)
        );
    }

    #[test]
    fn payload_availability_rejects_authenticated_new_view_recertification() {
        let keypairs = [
            checked_bls_keypair(41),
            checked_bls_keypair(42),
            checked_bls_keypair(43),
        ];
        let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);
        let source = payload.origin_proposal.clone();
        let target = retarget_lane_block_proposal_view(&source, 1).expect("next-view proposal");
        let new_view =
            durable_new_view_certificate(&source, &payload, &keypairs, network_id, epoch);
        let mut lane_height_above_proposal = new_view.certificate.body.clone();
        lane_height_above_proposal.proposal_height = lane_height_above_proposal
            .lane_block_height
            .saturating_sub(1);
        validate_lane_block_new_view_body(&lane_height_above_proposal)
            .expect("NewView coordinates use independent global and lane-local heights");
        let mut oversized_mode_tag = new_view.certificate.body.clone();
        oversized_mode_tag.qc_mode_tag =
            "x".repeat(MAX_LANE_NEW_VIEW_QC_MODE_TAG_BYTES.saturating_add(1));
        assert_eq!(
            validate_lane_block_new_view_body(&oversized_mode_tag),
            Err(LaneAutonomousArtifactError::InvalidNewViewBody)
        );
        validate_lane_block_new_view_transition(
            &source, &target, &payload, &new_view, network_id, epoch,
        )
        .expect("authenticated contiguous NewView transition");

        let votes = [
            signed_autonomous_prepare_vote(
                &payload,
                &target,
                network_id,
                epoch,
                &keypairs[0],
                &keypairs,
            ),
            signed_autonomous_prepare_vote(
                &payload,
                &target,
                network_id,
                epoch,
                &keypairs[1],
                &keypairs,
            ),
            signed_autonomous_prepare_vote(
                &payload,
                &target,
                network_id,
                epoch,
                &keypairs[2],
                &keypairs,
            ),
        ];
        let certificate = aggregate_lane_block_votes_to_qc(
            target.vote_body(CertPhase::Prepare),
            target.descriptor.validator_set.clone(),
            &votes,
        )
        .expect("next-view exact availability QC");
        let durable = DurableLanePayloadAvailabilityCertificateV1 { certificate };
        assert_eq!(
            validate_lane_payload_availability_certificate(&durable, &payload, network_id, epoch,),
            Err(LaneAutonomousArtifactError::AvailabilityMismatch),
            "NewView is only a transport cursor and must not create a second READY subject",
        );

        let mut stale_origin = durable.clone();
        stale_origin
            .certificate
            .payload_availability_qc
            .as_mut()
            .expect("availability QC")
            .body
            .origin_proposal_hash = Hash::new(b"stale-origin-rebinding");
        assert_eq!(
            validate_lane_payload_availability_certificate(
                &stale_origin,
                &payload,
                network_id,
                epoch,
            ),
            Err(LaneAutonomousArtifactError::AvailabilityMismatch)
        );

        let mut stale_incarnation = durable.clone();
        stale_incarnation
            .certificate
            .payload_availability_qc
            .as_mut()
            .expect("availability QC")
            .body
            .lane_incarnation = Hash::new(b"recreated-lane-incarnation");
        assert_eq!(
            validate_lane_payload_availability_certificate(
                &stale_incarnation,
                &payload,
                network_id,
                epoch,
            ),
            Err(LaneAutonomousArtifactError::AvailabilityMismatch)
        );

        let unrelated = {
            let mut proposal = target.clone();
            proposal.descriptor.accepted_transaction_hashes[0] =
                Hash::new(b"unrelated-proposal-entrypoint");
            proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
            proposal.proposal_hash = proposal.computed_proposal_hash();
            proposal
        };
        assert_eq!(
            lane_payload_availability_body(&payload, &unrelated, network_id, epoch,),
            Err(LaneAutonomousArtifactError::AvailabilityMismatch)
        );

        let skipped = retarget_lane_block_proposal_exact_view(&source, 2)
            .expect("canonical but unauthorized skipped-view proposal");
        assert_eq!(
            validate_lane_block_new_view_transition(
                &source, &skipped, &payload, &new_view, network_id, epoch,
            ),
            Err(LaneAutonomousArtifactError::InvalidNewViewBody)
        );
    }

    #[test]
    fn autonomous_session_requires_authorized_ready_quorum_and_resists_cache_flood() {
        let keypairs = [
            checked_bls_keypair(51),
            checked_bls_keypair(52),
            checked_bls_keypair(53),
        ];
        let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);
        let proposal = payload.origin_proposal.clone();
        let availability_body =
            lane_payload_availability_body(&payload, &proposal, network_id, epoch)
                .expect("authorized availability body");
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let mut cache = LaneBlockSessionCache::new(1);
        cache
            .insert_proposal(proposal.clone())
            .expect("cache autonomous proposal");
        cache
            .authorize_payload_availability(&proposal, availability_body.clone())
            .expect("authorize exact durable payload");

        let unsigned_prepare = signed_vote(&proposal.vote_body(CertPhase::Prepare), &keypairs[0]);
        assert_eq!(
            cache.insert_vote(unsigned_prepare, None),
            Err(LaneBlockSessionError::AvailabilityMismatch)
        );

        for nonce in 0_u8..32 {
            let proposal_hash =
                Hash::new([b"unauthorized-ready-flood-".as_slice(), &[nonce]].concat());
            let descriptor_hash =
                Hash::new([b"unauthorized-ready-descriptor-".as_slice(), &[nonce]].concat());
            let mut body = proposal.vote_body(CertPhase::Prepare);
            body.proposal_hash = proposal_hash;
            body.descriptor_hash = descriptor_hash;
            let mut ready_body = availability_body.clone();
            ready_body.current_proposal_hash = proposal_hash;
            ready_body.current_descriptor_hash = descriptor_hash;
            let vote = LaneBlockVoteV1 {
                body,
                payload_availability_vote: Some(LanePayloadAvailabilityVoteV1 {
                    body: ready_body,
                    signer: peer(&keypairs[0]),
                    validator_set_pops: aligned_validator_pops(
                        &proposal.descriptor.validator_set,
                        &keypairs,
                    ),
                    // Shape-valid but deliberately unverified: authorization
                    // rejection must happen before any cryptographic work.
                    bls_signature: vec![0; LANE_BLS_PROOF_BYTES],
                }),
                signer: peer(&keypairs[0]),
                bls_signature: vec![0; LANE_BLS_PROOF_BYTES],
            };
            assert_eq!(
                cache.insert_vote(vote, None),
                Err(LaneBlockSessionError::AvailabilityNotAuthorized)
            );
        }
        assert_eq!(cache.len(), 1);
        assert_eq!(
            cache
                .get(&key)
                .and_then(|session| session.payload_availability_body.as_ref()),
            Some(&availability_body)
        );

        for keypair in &keypairs {
            cache
                .insert_vote(
                    signed_autonomous_prepare_vote(
                        &payload, &proposal, network_id, epoch, keypair, &keypairs,
                    ),
                    None,
                )
                .expect("cache exact READY vote");
        }
        let prepare_qc = cache
            .get(&key)
            .and_then(|session| session.prepare_qc.as_ref())
            .expect("READY quorum seals prepare QC");
        assert!(prepare_qc.payload_availability_qc.is_some());
    }

    #[test]
    fn autonomous_payload_body_cap_reserves_consensus_envelope_headroom() {
        assert_eq!(
            MAX_LANE_EXECUTABLE_PAYLOAD_BYTES * 2,
            MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES,
            "an executable payload must leave half of its source bundle for authenticated proof material"
        );
        assert_eq!(
            MAX_LANE_EXECUTABLE_PAYLOAD_BYTES + LANE_EXECUTABLE_ENVELOPE_HEADROOM_BYTES,
            iroha_config::parameters::defaults::network::MAX_FRAME_BYTES_CONSENSUS.get(),
        );
        assert!(lane_executable_payload_body_within_limit(
            MAX_LANE_EXECUTABLE_PAYLOAD_BYTES - 1
        ));
        assert!(lane_executable_payload_body_within_limit(
            MAX_LANE_EXECUTABLE_PAYLOAD_BYTES
        ));
        assert!(!lane_executable_payload_body_within_limit(
            MAX_LANE_EXECUTABLE_PAYLOAD_BYTES + 1
        ));
    }

    #[test]
    fn native_amx_receipt_vector_is_payload_hash_bound_and_exactly_aligned() {
        let keypairs = [
            checked_bls_keypair(21),
            checked_bls_keypair(22),
            checked_bls_keypair(23),
        ];
        let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);
        let descriptor = &payload.origin_proposal.descriptor;
        let receipt = NativeAmxReceipt {
            version: 2,
            source_id: [0xA5; Hash::LENGTH],
            network_id,
            plan_digest: Hash::new(b"payload-hash-bound-native-amx-plan"),
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            authority_context_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            coordinator_proposal_hash: payload.origin_proposal.proposal_hash,
            legs: Vec::new(),
        };
        let with_receipt = compute_lane_executable_payload_hash(
            payload.version,
            network_id,
            epoch,
            &payload.origin_proposal,
            &payload.entrypoints,
            &payload.reservation_keys,
            &payload.routing_plans,
            &[Some(receipt)],
        )
        .expect("receipt-bearing payload hash");
        assert_ne!(payload.payload_hash, with_receipt);

        let mut misaligned = payload;
        misaligned.native_amx_receipts.push(None);
        assert_eq!(
            misaligned.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::NativeAmxReceiptMismatch)
        );
    }

    #[test]
    fn autonomous_payload_rejects_missing_extra_and_cross_bound_exact_slots() {
        let keypairs = [
            checked_bls_keypair(61),
            checked_bls_keypair(62),
            checked_bls_keypair(63),
        ];
        let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);

        let mut missing_reservation = payload.clone();
        missing_reservation.reservation_keys.clear();
        assert_eq!(
            missing_reservation.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::ReservationMismatch)
        );
        let mut extra_reservation = payload.clone();
        extra_reservation
            .reservation_keys
            .push(payload.reservation_keys[0]);
        assert_eq!(
            extra_reservation.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::ReservationMismatch)
        );
        let mut zero_owner = payload.clone();
        zero_owner.reservation_keys[0].reservation_owner_hash = Hash::prehashed([0; Hash::LENGTH]);
        assert_eq!(
            zero_owner.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::ReservationMismatch)
        );
        let mut zero_proposal_identity = payload.clone();
        zero_proposal_identity.reservation_keys[0].proposal_identity_hash =
            Hash::prehashed([0; Hash::LENGTH]);
        assert_eq!(
            zero_proposal_identity.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::ReservationMismatch)
        );
        let mut unsupported_reservation_version = payload.clone();
        unsupported_reservation_version.reservation_keys[0].version =
            LaneQueueReservationKeyV2::VERSION + 1;
        assert_eq!(
            unsupported_reservation_version.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::ReservationMismatch)
        );

        let mut missing_plan = payload.clone();
        missing_plan.routing_plans.clear();
        assert_eq!(
            missing_plan.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::RoutingPlanMismatch)
        );
        let mut extra_plan = payload.clone();
        extra_plan
            .routing_plans
            .push(payload.routing_plans[0].clone());
        assert_eq!(
            extra_plan.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::RoutingPlanMismatch)
        );
        let mut cross_bound_plan = payload.clone();
        cross_bound_plan.routing_plans[0] = RoutingPlan::single(
            crate::queue::RoutingDecision::new(LaneId::new(99), DataSpaceId::new(101)),
        );
        assert_eq!(
            cross_bound_plan.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::RoutingPlanMismatch)
        );

        let mut missing_receipt_slot = payload.clone();
        missing_receipt_slot.native_amx_receipts.clear();
        assert_eq!(
            missing_receipt_slot.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::NativeAmxReceiptMismatch)
        );
        let mut extra_receipt_slot = payload.clone();
        extra_receipt_slot.native_amx_receipts.push(None);
        assert_eq!(
            extra_receipt_slot.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::NativeAmxReceiptMismatch)
        );

        let descriptor = payload.origin_proposal.descriptor.clone();
        let mut forged_receipt = payload;
        forged_receipt.native_amx_receipts[0] = Some(NativeAmxReceipt {
            version: 2,
            source_id: [0x7A; Hash::LENGTH],
            network_id,
            plan_digest: forged_receipt.routing_plans[0].digest(),
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            authority_context_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            coordinator_proposal_hash: forged_receipt.origin_proposal.proposal_hash,
            legs: Vec::new(),
        });
        assert_eq!(
            forged_receipt.validate(network_id, epoch),
            Err(LaneAutonomousArtifactError::NativeAmxReceiptMismatch)
        );
    }

    #[test]
    fn compacted_new_view_checkpoint_is_independently_restart_verifiable() {
        let keypairs = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let (network_id, epoch, payload) = autonomous_payload_fixture(&keypairs);
        let source = retarget_lane_block_proposal_exact_view(&payload.origin_proposal, 256)
            .expect("canonical checkpoint source");
        let target =
            retarget_lane_block_proposal_view(&source, 257).expect("canonical checkpoint target");
        let checkpoint = DurableLaneBlockViewCheckpointV1 {
            source_proposal: source.clone(),
            target_proposal: target,
            certificate: durable_new_view_certificate(
                &source, &payload, &keypairs, network_id, epoch,
            ),
        };

        validate_lane_block_view_checkpoint(&checkpoint, &payload, network_id, epoch)
            .expect("checkpoint validates without the first 256 certificates");
        assert_eq!(checkpoint.source_proposal.descriptor.lane_block_view, 256);
        assert_eq!(checkpoint.target_proposal.descriptor.lane_block_view, 257);
    }

    fn lane_block_fixture_entrypoint_hash(domain: &[u8], identity: &[u8], ordinal: u8) -> Hash {
        let mut preimage = b"lane-consensus-test-entrypoint:".to_vec();
        preimage.extend_from_slice(domain);
        preimage.push(0);
        preimage.extend_from_slice(identity);
        preimage.push(ordinal);
        Hash::new(preimage)
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
            lane_incarnation: Hash::new(b"lane-consensus-fixture-incarnation"),
            proposal_height: lane_block_height,
            previous_lane_block_height: lane_block_height - 1,
            previous_lane_block_descriptor_hash: Some(Hash::prehashed([tag - 1; Hash::LENGTH])),
            lane_block_height,
            lane_block_view: 2,
            subject_hash: Hash::prehashed([tag; Hash::LENGTH]),
            payload_ownership_hash: Hash::prehashed([tag.saturating_add(1); Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([tag.saturating_add(2); Hash::LENGTH]),
            accepted_candidate_indices: vec![3, 1],
            accepted_transaction_hashes: vec![
                lane_block_fixture_entrypoint_hash(b"height", &lane_block_height.to_le_bytes(), 0),
                lane_block_fixture_entrypoint_hash(b"height", &lane_block_height.to_le_bytes(), 1),
            ],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set.to_vec()),
            validator_set: validator_set.to_vec(),
            validator_count: u32::try_from(validator_set.len()).expect("fixture validator count"),
            min_quorum: u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
                validator_set.len(),
            ))
            .expect("fixture quorum fits"),
            qc_mode_tag: "permissioned:lane:7:dataspace:11".to_string(),
            descriptor_hash: Hash::prehashed([0x00; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0x00; Hash::LENGTH]),
            payload_block_hint: None,
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
        let mut entrypoint_identity = proposal.proposal_hash.as_ref().to_vec();
        entrypoint_identity.push(tag);
        proposal.descriptor.subject_hash = Hash::prehashed([tag; Hash::LENGTH]);
        proposal.descriptor.payload_ownership_hash =
            Hash::prehashed([tag.saturating_add(1); Hash::LENGTH]);
        proposal.descriptor.rbc_instance_hash =
            Hash::prehashed([tag.saturating_add(2); Hash::LENGTH]);
        proposal.descriptor.accepted_transaction_hashes = vec![
            lane_block_fixture_entrypoint_hash(b"retag", &entrypoint_identity, 0),
            lane_block_fixture_entrypoint_hash(b"retag", &entrypoint_identity, 1),
        ];
        proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    fn conflicting_lane_block_proposal_next_view(
        proposal: LaneBlockProposalV1,
        tag: u8,
    ) -> LaneBlockProposalV1 {
        let mut proposal = retag_lane_block_proposal_payload(proposal, tag);
        proposal.descriptor.lane_block_view = proposal.descriptor.lane_block_view.saturating_add(1);
        proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
        proposal.proposal_hash = proposal.computed_proposal_hash();
        proposal
    }

    fn lane_block_proposal_at_view(
        proposal: &LaneBlockProposalV1,
        lane_block_view: u64,
        tag: u8,
    ) -> LaneBlockProposalV1 {
        let mut proposal = retag_lane_block_proposal_payload(proposal.clone(), tag);
        proposal.descriptor.lane_block_view = lane_block_view;
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
    fn lane_block_proposal_ingress_accepts_coordinate_boundaries() {
        let keypairs = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keypairs.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();

        let mut first = lane_block_proposal(&validator_set);
        first.descriptor.proposal_height = 1;
        first.descriptor.previous_lane_block_height = 0;
        first.descriptor.previous_lane_block_descriptor_hash = None;
        first.descriptor.lane_block_height = 1;
        first.descriptor.descriptor_hash = first.descriptor.computed_descriptor_hash();
        first.proposal_hash = first.computed_proposal_hash();
        validate_lane_block_proposal(&first)
            .expect("the first lane block has a canonical zero-height predecessor");

        let mut highest = lane_block_proposal(&validator_set);
        highest.descriptor.proposal_height = 1;
        highest.descriptor.previous_lane_block_height = u64::MAX - 1;
        highest.descriptor.lane_block_height = u64::MAX;
        highest.descriptor.descriptor_hash = highest.descriptor.computed_descriptor_hash();
        highest.proposal_hash = highest.computed_proposal_hash();
        validate_lane_block_proposal(&highest)
            .expect("maximal contiguous lane coordinates are independent of proposal height");
    }

    #[test]
    fn lane_block_proposal_ingress_rejects_adversarial_coordinates() {
        let keypairs = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keypairs.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);

        let mut zero_proposal_height = proposal.clone();
        zero_proposal_height.descriptor.proposal_height = 0;
        zero_proposal_height.descriptor.descriptor_hash =
            zero_proposal_height.descriptor.computed_descriptor_hash();
        zero_proposal_height.proposal_hash = zero_proposal_height.computed_proposal_hash();
        assert_eq!(
            validate_lane_block_proposal(&zero_proposal_height),
            Err(LaneBlockProposalIngressError::InvalidBody)
        );

        let mut predecessor_gap = proposal.clone();
        predecessor_gap.descriptor.previous_lane_block_height =
            predecessor_gap.descriptor.lane_block_height - 2;
        predecessor_gap.descriptor.descriptor_hash =
            predecessor_gap.descriptor.computed_descriptor_hash();
        predecessor_gap.proposal_hash = predecessor_gap.computed_proposal_hash();
        assert_eq!(
            validate_lane_block_proposal(&predecessor_gap),
            Err(LaneBlockProposalIngressError::InvalidBody)
        );

        let mut missing_predecessor_hash = proposal.clone();
        assert!(
            missing_predecessor_hash
                .descriptor
                .previous_lane_block_height
                > 0
        );
        missing_predecessor_hash
            .descriptor
            .previous_lane_block_descriptor_hash = None;
        missing_predecessor_hash.descriptor.descriptor_hash = missing_predecessor_hash
            .descriptor
            .computed_descriptor_hash();
        missing_predecessor_hash.proposal_hash = missing_predecessor_hash.computed_proposal_hash();
        assert_eq!(
            validate_lane_block_proposal(&missing_predecessor_hash),
            Err(LaneBlockProposalIngressError::InvalidBody),
            "a non-genesis lane block must bind its exact predecessor descriptor"
        );

        let mut overflowing_predecessor = proposal.clone();
        overflowing_predecessor.descriptor.proposal_height = u64::MAX;
        overflowing_predecessor
            .descriptor
            .previous_lane_block_height = u64::MAX;
        overflowing_predecessor.descriptor.lane_block_height = u64::MAX;
        overflowing_predecessor.descriptor.descriptor_hash = overflowing_predecessor
            .descriptor
            .computed_descriptor_hash();
        overflowing_predecessor.proposal_hash = overflowing_predecessor.computed_proposal_hash();
        assert_eq!(
            validate_lane_block_proposal(&overflowing_predecessor),
            Err(LaneBlockProposalIngressError::InvalidBody)
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
        predecessor_at_genesis.descriptor.proposal_height = 1;
        predecessor_at_genesis.descriptor.previous_lane_block_height = 0;
        predecessor_at_genesis.descriptor.lane_block_height = 1;
        predecessor_at_genesis.descriptor.descriptor_hash =
            predecessor_at_genesis.descriptor.computed_descriptor_hash();
        predecessor_at_genesis.proposal_hash = predecessor_at_genesis.computed_proposal_hash();
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

        let mut lowered_quorum = proposal.clone();
        lowered_quorum.descriptor.min_quorum -= 1;
        lowered_quorum.descriptor.descriptor_hash =
            lowered_quorum.descriptor.computed_descriptor_hash();
        lowered_quorum.proposal_hash = lowered_quorum.computed_proposal_hash();
        assert_eq!(
            validate_lane_block_proposal(&lowered_quorum),
            Err(LaneBlockProposalIngressError::InvalidBody),
            "a self-consistent proposal must not lower the canonical committee threshold"
        );

        let mut duplicate = proposal.clone();
        duplicate.descriptor.validator_set =
            vec![validator_set[0].clone(), validator_set[0].clone()];
        duplicate.descriptor.validator_count = 2;
        duplicate.descriptor.min_quorum = 2;
        duplicate.descriptor.validator_set_hash = HashOf::new(&duplicate.descriptor.validator_set);
        assert_eq!(
            validate_lane_block_proposal(&duplicate),
            Err(LaneBlockProposalIngressError::DuplicateValidator)
        );
    }

    #[test]
    fn lane_block_consensus_rejects_work_above_global_merge_capacity() {
        let keypairs = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keypairs.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let indices = (0..u64::try_from(MAX_LANE_EXECUTABLE_ENTRYPOINTS)
            .expect("entrypoint ceiling fits u64"))
            .collect::<Vec<_>>();
        let hashes = indices
            .iter()
            .map(|index| Hash::new(index.to_le_bytes()))
            .collect::<Vec<_>>();

        let mut at_limit = lane_block_proposal(&validator_set);
        at_limit.descriptor.accepted_candidate_indices = indices.clone();
        at_limit.descriptor.accepted_transaction_hashes = hashes.clone();
        at_limit.descriptor.descriptor_hash = at_limit.descriptor.computed_descriptor_hash();
        at_limit.proposal_hash = at_limit.computed_proposal_hash();
        validate_lane_block_proposal(&at_limit)
            .expect("a lane proposal at the global merge entrypoint ceiling is admissible");

        let mut above_limit = at_limit;
        above_limit
            .descriptor
            .accepted_candidate_indices
            .push(u64::try_from(MAX_LANE_EXECUTABLE_ENTRYPOINTS).expect("ceiling fits u64"));
        above_limit
            .descriptor
            .accepted_transaction_hashes
            .push(Hash::new(b"above-global-merge-entrypoint-ceiling"));
        above_limit.descriptor.descriptor_hash = above_limit.descriptor.computed_descriptor_hash();
        above_limit.proposal_hash = above_limit.computed_proposal_hash();
        assert_eq!(
            validate_lane_block_proposal(&above_limit),
            Err(LaneBlockProposalIngressError::InvalidBody)
        );

        let mut oversized_vote = vote_body(&validator_set);
        oversized_vote.accepted_candidate_indices =
            above_limit.descriptor.accepted_candidate_indices;
        oversized_vote.accepted_transaction_hashes =
            above_limit.descriptor.accepted_transaction_hashes;
        assert_eq!(
            validate_lane_block_vote_body_shape(&oversized_vote),
            Err(LaneBlockVoteIngressError::InvalidBody)
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

        vote.validate_ingress(CertPhase::Prepare)
            .expect("valid signed lane block vote");
    }

    #[test]
    fn lane_block_vote_explicit_none_roundtrips_and_omission_fails_closed() {
        #[derive(Encode)]
        struct LegacyLaneBlockVoteV1 {
            body: LaneBlockVoteBodyV1,
            signer: PeerId,
            bls_signature: Vec<u8>,
        }

        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let vote = signed_vote(&vote_body(&validator_set), &keys[0]);
        assert_eq!(vote.payload_availability_vote, None);

        let legacy = LegacyLaneBlockVoteV1 {
            body: vote.body.clone(),
            signer: vote.signer.clone(),
            bls_signature: vote.bls_signature.clone(),
        };
        let legacy_bytes = legacy.encode();
        assert!(
            LaneBlockVoteV1::decode(&mut legacy_bytes.as_slice()).is_err(),
            "the former vote layout without an explicit READY field must fail closed"
        );

        let mut value =
            norito::json::to_value(&vote).expect("encode vote with explicit None READY vote");
        {
            let fields = value.as_object_mut().expect("lane vote JSON object");
            assert_eq!(
                fields.get("payload_availability_vote"),
                Some(&norito::json::Value::Null),
                "None must remain an explicit wire field"
            );
        }
        let decoded = norito::json::from_value::<LaneBlockVoteV1>(value.clone())
            .expect("explicit None READY vote must round-trip");
        assert_eq!(decoded, vote);
        assert_eq!(decoded.payload_availability_vote, None);

        assert!(
            value
                .as_object_mut()
                .expect("lane vote JSON object")
                .remove("payload_availability_vote")
                .is_some()
        );
        assert!(
            norito::json::from_value::<LaneBlockVoteV1>(value).is_err(),
            "a lane vote omitting payload_availability_vote must fail closed"
        );
    }

    #[test]
    fn lane_block_vote_and_qc_ingress_require_nonzero_proposal_height() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();

        let mut highest_body = vote_body(&validator_set);
        highest_body.proposal_height = 1;
        highest_body.lane_block_height = u64::MAX;
        let highest_votes = [
            signed_vote(&highest_body, &keys[0]),
            signed_vote(&highest_body, &keys[1]),
        ];
        for vote in &highest_votes {
            vote.validate_ingress(CertPhase::Prepare)
                .expect("maximal lane height is independent of proposal height");
        }
        let highest_qc =
            aggregate_lane_block_votes_to_qc(highest_body, validator_set, &highest_votes)
                .expect("maximal lane-height QC");
        validate_lane_block_qc(&highest_qc)
            .expect("maximal lane-height QC is valid at non-zero proposal height");

        let mut zero_proposal_body = vote_body(&highest_qc.validator_set);
        zero_proposal_body.proposal_height = 0;
        let zero_proposal_vote = signed_vote(&zero_proposal_body, &keys[0]);
        assert_eq!(
            zero_proposal_vote.validate_ingress(CertPhase::Prepare),
            Err(LaneBlockVoteIngressError::InvalidBody)
        );
        let mut zero_proposal_qc = highest_qc.clone();
        zero_proposal_qc.body.proposal_height = 0;
        assert_eq!(
            validate_lane_block_qc(&zero_proposal_qc),
            Err(LaneBlockQcIngressError::InvalidBody)
        );
    }

    #[test]
    fn lane_block_vote_ingress_rejects_phase_algorithm_and_signature_drift() {
        let bls = checked_bls_keypair(1);
        let other = checked_bls_keypair(2);
        let ed25519 = checked_ed25519_keypair(3);
        let mut validator_set = [peer(&bls), peer(&other)].to_vec();
        validator_set.sort();
        let body = vote_body(&validator_set);
        let vote = signed_vote(&body, &bls);

        assert_eq!(
            vote.validate_ingress(CertPhase::Commit),
            Err(LaneBlockVoteIngressError::PhaseMismatch {
                expected: CertPhase::Commit,
                actual: CertPhase::Prepare,
            })
        );
        vote.validate_ingress(CertPhase::Prepare)
            .expect("valid signed lane block vote should not depend on transport sender");

        let mut non_bls = signed_vote(&body, &ed25519);
        non_bls.signer = peer(&ed25519);
        assert_eq!(
            non_bls.validate_ingress(CertPhase::Prepare),
            Err(LaneBlockVoteIngressError::SignerNotBlsNormal)
        );

        let mut bad_signature = vote;
        bad_signature.bls_signature = signed_vote(&body, &other).bls_signature;
        assert_eq!(
            bad_signature.validate_ingress(CertPhase::Prepare),
            Err(LaneBlockVoteIngressError::InvalidSignature)
        );
    }

    #[test]
    fn aggregate_lane_block_votes_builds_sorted_bitmap_and_signature() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
            checked_bls_keypair(4),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let body = vote_body(&validator_set);
        let vote_a = signed_vote(&body, &keys[0]);
        let vote_c = signed_vote(&body, &keys[2]);
        let vote_d = signed_vote(&body, &keys[3]);

        let qc = aggregate_lane_block_votes_to_qc(
            body.clone(),
            validator_set.clone(),
            &[vote_c.clone(), vote_a.clone(), vote_d.clone()],
        )
        .expect("lane block QC");

        let expected_signer_indices = [vote_a.signer, vote_c.signer, vote_d.signer]
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
    fn lane_block_qc_preserves_sparse_high_index_signer_order() {
        let mut keys = (1_u8..=10).map(checked_bls_keypair).collect::<Vec<_>>();
        keys.sort_by_key(peer);
        let validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        let body = vote_body(&validator_set);
        let signer_indices = [0_usize, 1, 2, 3, 4, 8, 9];
        let votes = signer_indices
            .into_iter()
            .map(|index| signed_vote(&body, &keys[index]))
            .collect::<Vec<_>>();

        let qc = aggregate_lane_block_votes_to_qc(body, validator_set, &votes)
            .expect("exact-threshold sparse lane block QC");
        assert_eq!(qc.signers_bitmap, vec![0b0001_1111, 0b0000_0011]);
        validate_lane_block_qc_aggregate(&qc, &signer_pops(&keys))
            .expect("bitmap order must select the matching key and PoP at every index");

        let mut high_padding_bit = qc;
        high_padding_bit.signers_bitmap[1] |= 0b1000_0000;
        assert_eq!(
            validate_lane_block_qc(&high_padding_bit),
            Err(LaneBlockQcIngressError::SignerBitmapOutOfRange)
        );
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
        let vote_b = signed_vote(&body, &keys[1]);
        let vote_c = signed_vote(&body, &keys[2]);
        let qc = aggregate_lane_block_votes_to_qc(body, validator_set, &[vote_a, vote_b, vote_c])
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
        let vote_b = signed_vote(&body, &keys[1]);
        let vote_c = signed_vote(&body, &keys[2]);
        let qc = aggregate_lane_block_votes_to_qc(
            body,
            validator_set.clone(),
            &[vote_a.clone(), vote_b, vote_c],
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
        let vote_b = signed_vote(&body, &keys[1]);
        let vote_c = signed_vote(&body, &keys[2]);
        let qc = aggregate_lane_block_votes_to_qc(
            body.clone(),
            validator_set.clone(),
            &[vote_a, vote_b, vote_c],
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
        let vote_b = signed_vote(&body, &keys[1]);
        let vote_c = signed_vote(&body, &keys[2]);
        let qc = aggregate_lane_block_votes_to_qc(
            body,
            validator_set.clone(),
            &[vote_a.clone(), vote_b, vote_c],
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
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
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
            cache.insert_proposal(proposal.clone()),
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

        assert!(
            cache
                .drain_newly_sealed_qcs_matching(&BTreeSet::new())
                .is_empty(),
            "a closed protocol gate must leave sealed QC fanout pending"
        );
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
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
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

        assert!(
            cache
                .drain_committed_sessions_up_to_matching(usize::MAX, &BTreeSet::new())
                .is_empty(),
            "a closed protocol gate must retain the complete session"
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
    fn lane_block_session_cache_rejects_conflicting_commit_vote_after_view_change() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal_a = lane_block_proposal_at_height(&validator_set, 13);
        let proposal_b = conflicting_lane_block_proposal_next_view(proposal_a.clone(), 0x51);
        assert_eq!(
            proposal_a.descriptor.lane_block_height,
            proposal_b.descriptor.lane_block_height
        );
        assert_ne!(
            proposal_a.descriptor.lane_block_view,
            proposal_b.descriptor.lane_block_view
        );
        assert_ne!(proposal_a.proposal_hash, proposal_b.proposal_hash);

        let signer = &keys[0];
        let commit_a = signed_vote(&proposal_a.vote_body(CertPhase::Commit), signer);
        let commit_b_same_signer = signed_vote(&proposal_b.vote_body(CertPhase::Commit), signer);
        let commit_b_other_signer = signed_vote(&proposal_b.vote_body(CertPhase::Commit), &keys[1]);
        let prepare_a_same_signer = signed_vote(&proposal_a.vote_body(CertPhase::Prepare), signer);
        let prepare_a_other_signer =
            signed_vote(&proposal_a.vote_body(CertPhase::Prepare), &keys[1]);
        let prepare_b_same_signer = signed_vote(&proposal_b.vote_body(CertPhase::Prepare), signer);
        let prepare_b_other_signer =
            signed_vote(&proposal_b.vote_body(CertPhase::Prepare), &keys[1]);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(proposal_a),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(proposal_b.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(
                prepare_a_same_signer.clone(),
                Some(&prepare_a_same_signer.signer)
            ),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(
                prepare_a_other_signer.clone(),
                Some(&prepare_a_other_signer.signer)
            ),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(prepare_b_same_signer, Some(&commit_b_same_signer.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted),
            "prepare votes may move to a later lane view before any commit lock is taken"
        );
        assert_eq!(
            cache.insert_vote(
                prepare_b_other_signer.clone(),
                Some(&prepare_b_other_signer.signer)
            ),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(commit_a.clone(), Some(&commit_a.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        let duplicate_snapshot = cache.clone();
        assert_eq!(
            cache.insert_vote(commit_a.clone(), Some(&commit_a.signer)),
            Ok(LaneBlockSessionInsertOutcome::Duplicate)
        );
        assert_eq!(
            cache, duplicate_snapshot,
            "duplicate commit votes must not mutate replay state or signer locks"
        );
        assert_eq!(
            cache.can_accept_vote(&commit_b_same_signer, Some(&commit_b_same_signer.signer)),
            Err(LaneBlockSessionError::ConflictingVote)
        );
        let rejected_snapshot = cache.clone();
        assert_eq!(
            cache.insert_vote(
                commit_b_same_signer.clone(),
                Some(&commit_b_same_signer.signer)
            ),
            Err(LaneBlockSessionError::ConflictingVote),
            "a signer must not commit-vote two payloads for the same lane height"
        );
        assert_eq!(
            cache, rejected_snapshot,
            "rejected commit votes must not mutate replay state or signer locks"
        );
        assert_eq!(
            cache.insert_vote(
                commit_b_other_signer.clone(),
                Some(&commit_b_other_signer.signer)
            ),
            Ok(LaneBlockSessionInsertOutcome::Inserted),
            "other validators remain free to commit the later view"
        );
    }

    #[test]
    fn lane_block_session_cache_rejects_conflicting_commit_qc_with_overlapping_signer() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
            checked_bls_keypair(4),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal_a = lane_block_proposal_at_height(&validator_set, 13);
        let proposal_b = conflicting_lane_block_proposal_next_view(proposal_a.clone(), 0x61);
        let prepare_body_a = proposal_a.vote_body(CertPhase::Prepare);
        let prepare_votes_a = [
            signed_vote(&prepare_body_a, &keys[0]),
            signed_vote(&prepare_body_a, &keys[1]),
            signed_vote(&prepare_body_a, &keys[2]),
        ];
        let prepare_qc_a = aggregate_lane_block_votes_to_qc(
            prepare_body_a,
            validator_set.clone(),
            &prepare_votes_a,
        )
        .expect("first prepare QC");
        let prepare_body_b = proposal_b.vote_body(CertPhase::Prepare);
        let prepare_votes_b = [
            signed_vote(&prepare_body_b, &keys[1]),
            signed_vote(&prepare_body_b, &keys[2]),
            signed_vote(&prepare_body_b, &keys[3]),
        ];
        let prepare_qc_b = aggregate_lane_block_votes_to_qc(
            prepare_body_b,
            validator_set.clone(),
            &prepare_votes_b,
        )
        .expect("second prepare QC");
        let commit_body_a = proposal_a.vote_body(CertPhase::Commit);
        let commit_votes_a = [
            signed_vote(&commit_body_a, &keys[0]),
            signed_vote(&commit_body_a, &keys[1]),
            signed_vote(&commit_body_a, &keys[2]),
        ];
        let commit_qc_a =
            aggregate_lane_block_votes_to_qc(commit_body_a, validator_set.clone(), &commit_votes_a)
                .expect("first commit QC");
        let commit_body_b = proposal_b.vote_body(CertPhase::Commit);
        let commit_votes_b = [
            signed_vote(&commit_body_b, &keys[1]),
            signed_vote(&commit_body_b, &keys[2]),
            signed_vote(&commit_body_b, &keys[3]),
        ];
        let commit_qc_b =
            aggregate_lane_block_votes_to_qc(commit_body_b, validator_set, &commit_votes_b)
                .expect("conflicting commit QC with quorum intersection");
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(proposal_a),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(proposal_b),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc(prepare_qc_a),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc(prepare_qc_b),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc(commit_qc_a.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        let duplicate_snapshot = cache.clone();
        assert_eq!(
            cache.insert_qc(commit_qc_a),
            Ok(LaneBlockSessionInsertOutcome::Duplicate)
        );
        assert_eq!(
            cache, duplicate_snapshot,
            "duplicate commit QCs must not mutate replay state or signer locks"
        );
        let rejected_snapshot = cache.clone();
        assert_eq!(
            cache.insert_qc(commit_qc_b),
            Err(LaneBlockSessionError::ConflictingVote),
            "overlapping commit-QC signers must not certify two lane payloads at one height"
        );
        assert_eq!(
            cache, rejected_snapshot,
            "rejected commit QCs must not mutate replay state or signer locks"
        );
    }

    #[test]
    fn lane_block_session_cache_drains_committed_session_from_inbound_qcs() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
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
        let prepare_vote_c = signed_vote(&prepare_body, &keys[2]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set.clone(),
            &[prepare_vote_a, prepare_vote_b, prepare_vote_c],
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
    fn lane_block_session_cache_lists_prepare_vote_opportunities_until_vote_or_qc_arrives() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let signer = peer(&keys[2]);
        let prepare_body = proposal.vote_body(CertPhase::Prepare);
        let prepare_vote = signed_vote(&prepare_body, &keys[2]);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.local_prepare_vote_proposals_for(&signer),
            vec![proposal.clone()],
            "first local prepare vote scan should expose the cached proposal"
        );
        assert!(
            cache.local_prepare_vote_needed_for(&proposal, &signer),
            "cached proposal without a local prepare vote should request local signing"
        );
        assert_eq!(
            cache.local_prepare_vote_proposals_for(&signer),
            vec![proposal.clone()],
            "prepare vote scans must be non-consuming so readiness retries can poll"
        );
        assert_eq!(
            cache.insert_vote(prepare_vote.clone(), Some(&prepare_vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.local_prepare_vote_proposals_for(&signer).is_empty(),
            "an already cached local prepare vote must not be requested again"
        );
        assert!(
            !cache.local_prepare_vote_needed_for(&proposal, &signer),
            "cached local prepare vote should suppress duplicate inbound signing"
        );
    }

    #[test]
    fn lane_block_session_cache_lists_commit_vote_opportunities_without_draining() {
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
        let prepare_vote_c = signed_vote(&prepare_body, &keys[2]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set,
            &[prepare_vote_a, prepare_vote_b, prepare_vote_c],
        )
        .expect("prepare QC");
        let signer = peer(&keys[2]);
        let commit_body = proposal.vote_body(CertPhase::Commit);
        let commit_vote = signed_vote(&commit_body, &keys[2]);
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
        let requests = cache.local_commit_vote_requests_for(&signer);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].proposal, proposal);
        assert_eq!(requests[0].prepare_qc, prepare_qc);
        assert_eq!(
            cache.local_commit_vote_requests_for(&signer).len(),
            1,
            "commit vote scans must be non-consuming so readiness retries can poll"
        );
        assert_eq!(
            cache.insert_vote(commit_vote.clone(), Some(&commit_vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.local_commit_vote_requests_for(&signer).is_empty(),
            "an already cached local commit vote must not be requested again"
        );
    }

    #[test]
    fn lane_block_session_cache_lists_proposals_without_commit_qc_for_rebroadcast() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
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
            cache.proposals_without_commit_qc(),
            vec![proposal.clone()],
            "uncertified cached proposals should remain eligible for lane-committee fanout"
        );
        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.proposals_without_commit_qc(),
            vec![proposal],
            "prepared sessions still need proposal fanout until commit QC arrives"
        );
        assert_eq!(
            cache.insert_qc_with_pops(commit_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.proposals_without_commit_qc().is_empty(),
            "committed sessions should not keep rebroadcasting proposals"
        );
    }

    #[test]
    fn lane_block_session_cache_lists_local_vote_rebroadcast_artifacts() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let signer = peer(&keys[0]);
        let prepare_body = proposal.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let prepare_vote_c = signed_vote(&prepare_body, &keys[2]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set.clone(),
            &[
                prepare_vote_a.clone(),
                prepare_vote_b.clone(),
                prepare_vote_c,
            ],
        )
        .expect("prepare QC");
        let commit_body = proposal.vote_body(CertPhase::Commit);
        let commit_vote_a = signed_vote(&commit_body, &keys[0]);
        let commit_vote_b = signed_vote(&commit_body, &keys[1]);
        let commit_vote_c = signed_vote(&commit_body, &keys[2]);
        let commit_qc = aggregate_lane_block_votes_to_qc(
            commit_body,
            validator_set.clone(),
            &[commit_vote_a.clone(), commit_vote_b.clone(), commit_vote_c],
        )
        .expect("commit QC");
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
            cache.local_vote_rebroadcast_artifacts_for(&signer),
            vec![(proposal.clone(), prepare_vote_a.clone())],
            "local prepare vote should remain eligible for retry until prepare QC arrives"
        );

        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.local_vote_rebroadcast_artifacts_for(&signer),
            Vec::new(),
            "prepare vote retry should stop after prepare QC arrives"
        );
        assert_eq!(
            cache.insert_vote(commit_vote_a.clone(), Some(&commit_vote_a.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.local_vote_rebroadcast_artifacts_for(&signer),
            vec![(proposal, commit_vote_a)],
            "local commit vote should remain eligible for retry until commit QC arrives"
        );
        assert_eq!(
            cache.insert_qc_with_pops(commit_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache
                .local_vote_rebroadcast_artifacts_for(&signer)
                .is_empty(),
            "committed sessions should not keep rebroadcasting local votes"
        );
    }

    #[test]
    fn lane_block_session_cache_lists_qcs_for_incomplete_session_rebroadcast() {
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
        let prepare_vote_c = signed_vote(&prepare_body, &keys[2]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set.clone(),
            &[prepare_vote_a, prepare_vote_b, prepare_vote_c],
        )
        .expect("prepare QC");
        let commit_body = proposal.vote_body(CertPhase::Commit);
        let commit_vote_a = signed_vote(&commit_body, &keys[0]);
        let commit_vote_b = signed_vote(&commit_body, &keys[1]);
        let commit_vote_c = signed_vote(&commit_body, &keys[2]);
        let commit_qc = aggregate_lane_block_votes_to_qc(
            commit_body,
            validator_set,
            &[commit_vote_a, commit_vote_b, commit_vote_c],
        )
        .expect("commit QC");
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(proposal),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(
            cache.qcs_for_incomplete_sessions().is_empty(),
            "proposal-only sessions should not rebroadcast QCs"
        );
        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc.clone(), &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.qcs_for_incomplete_sessions(),
            vec![prepare_qc.clone()],
            "prepared sessions should rebroadcast prepare QC until commit QC arrives"
        );
        assert_eq!(
            cache.insert_qc_with_pops(commit_qc.clone(), &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.qcs_for_incomplete_sessions(),
            vec![prepare_qc, commit_qc],
            "committed-but-undrained sessions should rebroadcast both QCs"
        );
        assert_eq!(cache.drain_committed_sessions().len(), 1);
        assert!(
            cache.qcs_for_incomplete_sessions().is_empty(),
            "drained committed sessions should stop rebroadcasting QCs"
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
        let prepare_vote_c = signed_vote(&prepare_body, &keys[2]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set,
            &[prepare_vote_a, prepare_vote_b, prepare_vote_c],
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
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
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
    fn lane_block_session_cache_treats_same_body_alternate_quorum_qc_as_duplicate() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
            checked_bls_keypair(4),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let prepare_body = proposal.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let prepare_vote_c = signed_vote(&prepare_body, &keys[2]);
        let prepare_vote_d = signed_vote(&prepare_body, &keys[3]);
        let prepare_qc_ab = aggregate_lane_block_votes_to_qc(
            prepare_body.clone(),
            validator_set.clone(),
            &[
                prepare_vote_a,
                prepare_vote_b.clone(),
                prepare_vote_c.clone(),
            ],
        )
        .expect("prepare QC from first quorum");
        let prepare_qc_bc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set,
            &[prepare_vote_b, prepare_vote_c, prepare_vote_d],
        )
        .expect("prepare QC from alternate quorum");
        assert_ne!(
            prepare_qc_ab, prepare_qc_bc,
            "alternate quorum fixtures must produce distinct QC bytes"
        );
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(proposal),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc_ab.clone(), &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc_bc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Duplicate),
            "a different valid aggregate over the same body is equivalent certificate evidence"
        );
        assert_eq!(
            cache
                .get(&key)
                .expect("session remains cached")
                .prepare_qc
                .as_ref(),
            Some(&prepare_qc_ab)
        );
    }

    #[test]
    fn lane_block_session_cache_reconciles_orphan_qc_drift_before_commit_drain() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
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
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
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
    fn lane_block_session_cache_preflight_rejects_conflicting_proposal_without_mutation() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let conflicting = retag_lane_block_proposal_payload(proposal.clone(), 0x88);
        assert_ne!(
            conflicting.proposal_hash, proposal.proposal_hash,
            "fixture must keep the lane slot while changing the proposal identity"
        );
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.can_accept_proposal(&proposal),
            Ok(()),
            "empty cache should preflight the canonical proposal"
        );
        assert_eq!(
            cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.can_accept_proposal(&proposal),
            Ok(()),
            "duplicate proposal should remain admissible for replay/broadcast"
        );
        assert_eq!(
            cache.can_accept_proposal(&conflicting),
            Err(LaneBlockSessionError::ConflictingProposal),
            "same-slot conflicting proposal must fail preflight"
        );
        assert_eq!(cache.len(), 1);
        assert_eq!(
            cache
                .get(&key)
                .expect("original session remains cached")
                .proposal
                .as_ref(),
            Some(&proposal),
            "failed preflight must not mutate the cached proposal"
        );
    }

    #[test]
    fn lane_block_session_cache_preflight_rejects_conflicting_vote_without_mutation() {
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
        let vote = signed_vote(&body, &keys[0]);
        let mut drift_body = body;
        drift_body.descriptor_hash = Hash::prehashed([0xE1; Hash::LENGTH]);
        let drift_vote = signed_vote(&drift_body, &keys[0]);
        assert_ne!(
            drift_vote, vote,
            "fixture must keep the same signer and session key while changing the vote body"
        );
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.can_accept_vote(&drift_vote, Some(&drift_vote.signer)),
            Ok(()),
            "empty cache should preflight an orphan vote"
        );
        assert_eq!(
            cache.insert_vote(drift_vote.clone(), Some(&drift_vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.can_accept_vote(&vote, Some(&vote.signer)),
            Err(LaneBlockSessionError::ConflictingVote),
            "same-signer conflicting vote must fail preflight"
        );
        let session = cache.get(&key).expect("orphan vote session remains cached");
        assert_eq!(session.prepare_votes.len(), 1);
        assert_eq!(
            session.prepare_votes.get(&drift_vote.signer),
            Some(&drift_vote),
            "failed preflight must not overwrite the cached orphan vote"
        );
    }

    #[test]
    fn lane_block_session_cache_tracks_exact_duplicate_artifacts() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let body = proposal.vote_body(CertPhase::Prepare);
        let vote = signed_vote(&body, &keys[0]);
        let mut cache = LaneBlockSessionCache::new(4);

        assert!(!cache.contains_proposal(&proposal));
        assert!(!cache.contains_vote(&vote));
        assert_eq!(
            cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(vote.clone(), Some(&vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        assert!(cache.contains_proposal(&proposal));
        assert!(cache.contains_vote(&vote));
        assert!(
            !cache.contains_proposal(&retag_lane_block_proposal_payload(proposal.clone(), 0x91))
        );
        let mut conflicting_body = body;
        conflicting_body.descriptor_hash = Hash::prehashed([0x92; Hash::LENGTH]);
        let conflicting_vote = signed_vote(&conflicting_body, &keys[0]);
        assert!(!cache.contains_vote(&conflicting_vote));
    }

    #[test]
    fn lane_block_session_cache_merges_payload_hint_for_duplicate_proposal() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let hinted = proposal
            .clone()
            .with_payload_block_hint(LaneBlockProposalPayloadHintV1 {
                proposal_height: 42,
                proposal_view: 7,
                proposal_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                    Hash::prehashed([0x42; Hash::LENGTH]),
                ),
            });
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert!(!cache.contains_proposal(&hinted));
        assert!(
            cache.contains_proposal_identity(&hinted),
            "advisory payload hints must not change the cached consensus identity"
        );
        assert!(
            cache.can_accept_proposal(&hinted).is_ok(),
            "hint-only duplicate must pass preflight"
        );
        assert_eq!(
            cache.insert_proposal(hinted.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted),
            "a newly learned hint should wake repair paths"
        );
        assert_eq!(
            cache
                .get(&key)
                .and_then(|session| session.proposal.as_ref())
                .and_then(|proposal| proposal.payload_block_hint.as_ref()),
            hinted.payload_block_hint.as_ref()
        );
        assert_eq!(
            cache.insert_proposal(hinted),
            Ok(LaneBlockSessionInsertOutcome::Duplicate)
        );
    }

    #[test]
    fn lane_block_session_cache_refreshes_commit_drain_after_payload_hint_merge() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let hinted = proposal
            .clone()
            .with_payload_block_hint(LaneBlockProposalPayloadHintV1 {
                proposal_height: 42,
                proposal_view: 7,
                proposal_block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(
                    Hash::prehashed([0x42; Hash::LENGTH]),
                ),
            });
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
        let key = LaneBlockSessionKey::from_proposal(&proposal);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(proposal),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(commit_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        cache
            .sessions
            .get_mut(&key)
            .expect("session")
            .pending_committed_session_drain = false;

        assert_eq!(
            cache.insert_proposal(hinted.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        let committed = cache.drain_committed_sessions();
        assert_eq!(committed.len(), 1);
        assert_eq!(
            committed[0].proposal.payload_block_hint,
            hinted.payload_block_hint
        );
    }

    #[test]
    fn lane_block_session_cache_does_not_drain_inbound_qc() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
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
            cache.insert_proposal(conflicting.clone()),
            Err(LaneBlockSessionError::ConflictingProposal)
        );
    }

    #[test]
    fn lane_block_session_cache_rejects_cross_session_entrypoint_replays() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let mut later_height = lane_block_proposal_at_height(&validator_set, 14);
        later_height.descriptor.accepted_transaction_hashes =
            proposal.descriptor.accepted_transaction_hashes.clone();
        later_height.descriptor.descriptor_hash =
            later_height.descriptor.computed_descriptor_hash();
        later_height.proposal_hash = later_height.computed_proposal_hash();
        let different_route = rebind_lane_block_proposal_route(
            proposal.clone(),
            LaneId::new(8),
            DataSpaceId::new(12),
        );
        let mut different_incarnation = proposal.clone();
        different_incarnation.descriptor.lane_incarnation =
            Hash::new(b"adversarial-recreated-lane-incarnation");
        different_incarnation.descriptor.descriptor_hash =
            different_incarnation.descriptor.computed_descriptor_hash();
        different_incarnation.proposal_hash = different_incarnation.computed_proposal_hash();

        let mut cache = LaneBlockSessionCache::new(8);
        assert_eq!(
            cache.insert_proposal(proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        for replay in [&later_height, &different_route, &different_incarnation] {
            assert_eq!(
                cache.can_accept_proposal(replay),
                Err(LaneBlockSessionError::EntrypointAlreadyClaimed)
            );
            assert_eq!(
                cache.insert_proposal(replay.clone()),
                Err(LaneBlockSessionError::EntrypointAlreadyClaimed)
            );
        }

        let next_view =
            retarget_lane_block_proposal_view(&proposal, proposal.descriptor.lane_block_view + 1)
                .expect("exact NewView successor");
        assert_eq!(
            cache.insert_proposal(next_view),
            Ok(LaneBlockSessionInsertOutcome::Inserted),
            "an exact view transition must retain the immutable payload claim"
        );

        let mut reordered = LaneBlockSessionCache::new(8);
        assert_eq!(
            reordered.insert_proposal(later_height.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            reordered.insert_proposal(proposal.clone()),
            Err(LaneBlockSessionError::EntrypointAlreadyClaimed),
            "arrival order must not permit the same entrypoint in two live heights"
        );

        let removed = reordered.retain_sessions_for_admissible_lanes(
            |_lane_id, _dataspace_id, _incarnation, lane_height, _proposal_height| {
                lane_height != later_height.descriptor.lane_block_height
            },
        );
        assert_eq!(removed, 1);
        assert_eq!(
            reordered.insert_proposal(proposal),
            Ok(LaneBlockSessionInsertOutcome::Inserted),
            "pruning the former live session must release its entrypoint claims"
        );
    }

    #[test]
    fn lane_block_session_cache_recovered_proposal_replaces_uncertified_conflicting_slot() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let recovered = lane_block_proposal(&validator_set);
        let conflicting = retag_lane_block_proposal_payload(recovered.clone(), 0xB0);
        let recovered_key = LaneBlockSessionKey::from_proposal(&recovered);
        let conflicting_key = LaneBlockSessionKey::from_proposal(&conflicting);
        let recovered_vote_body = recovered.vote_body(CertPhase::Prepare);
        let recovered_vote = signed_vote(&recovered_vote_body, &keys[0]);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_ne!(recovered.proposal_hash, conflicting.proposal_hash);
        assert_eq!(
            cache.insert_vote(recovered_vote.clone(), Some(&recovered_vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(conflicting),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_recovered_proposal_replacing_uncommitted_conflict(recovered.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        assert!(cache.get(&conflicting_key).is_none());
        assert_eq!(
            cache
                .get(&recovered_key)
                .and_then(|session| session.proposal.as_ref()),
            Some(&recovered)
        );
        assert!(
            cache.contains_vote(&recovered_vote),
            "orphan artifacts for the recovered proposal should be reconciled after slot replacement"
        );
    }

    #[test]
    fn trusted_replanning_normalizes_only_the_advisory_global_hint() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let base = lane_block_proposal(&validator_set);
        let first = base.clone().with_payload_block_hint(
            iroha_data_model::block::consensus::LaneBlockProposalPayloadHintV1 {
                proposal_height: base.descriptor.proposal_height,
                proposal_view: 0,
                proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"losing-global-proposal",
                )),
            },
        );
        let canonical = base.clone().with_payload_block_hint(
            iroha_data_model::block::consensus::LaneBlockProposalPayloadHintV1 {
                proposal_height: base.descriptor.proposal_height,
                proposal_view: 4,
                proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"locked-global-proposal",
                )),
            },
        );
        let vote = signed_vote(&first.vote_body(CertPhase::Prepare), &keys[0]);
        let key = LaneBlockSessionKey::from_proposal(&first);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(first),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(vote.clone(), Some(&vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_replanned_proposal_replacing_uncommitted_conflict(canonical.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache
                .get(&key)
                .and_then(|session| session.proposal.as_ref()),
            Some(&canonical)
        );
        assert!(
            cache.contains_vote(&vote),
            "global hint normalization must preserve signatures over the unchanged lane subject",
        );
    }

    #[test]
    fn lane_block_session_cache_single_orphan_vote_cannot_displace_slot_proposal() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let proposal = lane_block_proposal(&validator_set);
        let conflicting = retag_lane_block_proposal_payload(proposal.clone(), 0xB1);
        let conflicting_key = LaneBlockSessionKey::from_proposal(&conflicting);
        let proposal_vote_body = proposal.vote_body(CertPhase::Prepare);
        let proposal_vote = signed_vote(&proposal_vote_body, &keys[0]);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_ne!(proposal.proposal_hash, conflicting.proposal_hash);
        assert_eq!(
            cache.insert_vote(proposal_vote.clone(), Some(&proposal_vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(conflicting.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.can_accept_proposal(&proposal),
            Err(LaneBlockSessionError::ConflictingProposal),
            "one committee member must not select a conflicting live proposal"
        );
        assert_eq!(
            cache.insert_proposal(proposal),
            Err(LaneBlockSessionError::ConflictingProposal)
        );

        assert_eq!(
            cache
                .get(&conflicting_key)
                .and_then(|session| session.proposal.as_ref()),
            Some(&conflicting)
        );
        assert!(
            cache.contains_vote(&proposal_vote),
            "rejected displacement must not erase the bounded orphan vote evidence"
        );
    }

    #[test]
    fn lane_block_session_cache_recovered_proposal_replaces_prepare_voted_conflicting_slot() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let recovered = lane_block_proposal(&validator_set);
        let conflicting = retag_lane_block_proposal_payload(recovered.clone(), 0xB0);
        let recovered_key = LaneBlockSessionKey::from_proposal(&recovered);
        let conflicting_key = LaneBlockSessionKey::from_proposal(&conflicting);
        let prepare_body = conflicting.vote_body(CertPhase::Prepare);
        let prepare_vote = signed_vote(&prepare_body, &keys[0]);
        let signer = prepare_vote.signer.clone();
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(conflicting.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_vote(prepare_vote, Some(&signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_recovered_proposal_replacing_uncommitted_conflict(recovered.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        assert_eq!(
            cache
                .get(&recovered_key)
                .and_then(|session| session.proposal.as_ref()),
            Some(&recovered),
            "durable canonical recovery must replace a speculative same-slot prepare vote"
        );
        assert_eq!(
            cache.get(&conflicting_key),
            None,
            "prepare-only sibling state must not keep canonical recovery in a retry loop"
        );
    }

    #[test]
    fn lane_block_session_cache_recovered_proposal_preserves_prepared_conflicting_slot() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let recovered = lane_block_proposal(&validator_set);
        let conflicting = retag_lane_block_proposal_payload(recovered.clone(), 0xB0);
        let recovered_key = LaneBlockSessionKey::from_proposal(&recovered);
        let conflicting_key = LaneBlockSessionKey::from_proposal(&conflicting);
        let prepare_body = conflicting.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set,
            &[prepare_vote_a, prepare_vote_b],
        )
        .expect("prepare QC");
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(conflicting.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_recovered_proposal_replacing_uncommitted_conflict(recovered),
            Err(LaneBlockSessionError::ConflictingProposal)
        );

        assert!(cache.get(&recovered_key).is_none());
        assert_eq!(
            cache
                .get(&conflicting_key)
                .and_then(|session| session.proposal.as_ref()),
            Some(&conflicting)
        );
    }

    #[test]
    fn lane_block_session_cache_recovered_proposal_preserves_commit_voted_conflicting_slot() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let recovered = lane_block_proposal(&validator_set);
        let conflicting = retag_lane_block_proposal_payload(recovered.clone(), 0xB0);
        let recovered_key = LaneBlockSessionKey::from_proposal(&recovered);
        let conflicting_key = LaneBlockSessionKey::from_proposal(&conflicting);
        let prepare_body = conflicting.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let commit_body = conflicting.vote_body(CertPhase::Commit);
        let commit_vote = signed_vote(&commit_body, &keys[0]);
        let signer = commit_vote.signer.clone();
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(conflicting.clone()),
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
        assert_eq!(
            cache.insert_vote(commit_vote, Some(&signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_recovered_proposal_replacing_uncommitted_conflict(recovered),
            Err(LaneBlockSessionError::ConflictingProposal),
            "canonical recovery must fail closed when the sibling carries a commit vote"
        );

        assert!(cache.get(&recovered_key).is_none());
        assert_eq!(
            cache
                .get(&conflicting_key)
                .and_then(|session| session.proposal.as_ref()),
            Some(&conflicting)
        );
    }

    #[test]
    fn lane_block_session_cache_recovered_proposal_preserves_committed_conflicting_slot() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let recovered = lane_block_proposal(&validator_set);
        let conflicting = retag_lane_block_proposal_payload(recovered.clone(), 0xB0);
        let recovered_key = LaneBlockSessionKey::from_proposal(&recovered);
        let conflicting_key = LaneBlockSessionKey::from_proposal(&conflicting);
        let prepare_body = conflicting.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            validator_set.clone(),
            &[prepare_vote_a, prepare_vote_b],
        )
        .expect("prepare QC");
        let commit_body = conflicting.vote_body(CertPhase::Commit);
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
            cache.insert_proposal(conflicting.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(commit_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_recovered_proposal_replacing_uncommitted_conflict(recovered),
            Err(LaneBlockSessionError::ConflictingProposal)
        );

        assert!(cache.get(&recovered_key).is_none());
        assert_eq!(
            cache
                .get(&conflicting_key)
                .and_then(|session| session.proposal.as_ref()),
            Some(&conflicting)
        );
    }

    #[test]
    fn lane_block_session_cache_rejects_forged_aggregate_qc() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let body = vote_body(&validator_set);
        let vote_a = signed_vote(&body, &keys[0]);
        let vote_b = signed_vote(&body, &keys[1]);
        let mut qc = aggregate_lane_block_votes_to_qc(body, validator_set, &[vote_a, vote_b])
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
        let proposal_a = lane_block_proposal_at_height(&validator_set, 13);
        let key_a = LaneBlockSessionKey::from_proposal(&proposal_a);
        let proposal_b = lane_block_proposal_at_height(&validator_set, 14);
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
    fn first_global_lock_retires_all_losing_speculation_but_keeps_commit_evidence() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let anchor = |label: &'static [u8]| LaneBlockProposalPayloadHintV1 {
            proposal_height: 13,
            proposal_view: 0,
            proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(label)),
        };
        let at_global_height = |mut proposal: LaneBlockProposalV1| {
            proposal.descriptor.proposal_height = 13;
            proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
            proposal.proposal_hash = proposal.computed_proposal_hash();
            proposal
        };
        let retained = lane_block_proposal_at_height(&validator_set, 13)
            .with_payload_block_hint(anchor(b"first-lock-retained-anchor"));
        let losing = at_global_height(lane_block_proposal_at_height(&validator_set, 14))
            .with_payload_block_hint(anchor(b"first-lock-losing-anchor"));
        let protected = at_global_height(lane_block_proposal_at_height(&validator_set, 15))
            .with_payload_block_hint(anchor(b"first-lock-protected-anchor"));
        let historical = lane_block_proposal_at_height(&validator_set, 12).with_payload_block_hint(
            LaneBlockProposalPayloadHintV1 {
                proposal_height: 12,
                proposal_view: 0,
                proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                    b"historical-canonical-anchor",
                )),
            },
        );
        let retained_key = LaneBlockSessionKey::from_proposal(&retained);
        let losing_key = LaneBlockSessionKey::from_proposal(&losing);
        let protected_key = LaneBlockSessionKey::from_proposal(&protected);
        let historical_key = LaneBlockSessionKey::from_proposal(&historical);
        let mut cache = LaneBlockSessionCache::new(8);
        cache
            .insert_proposal(retained.clone())
            .expect("insert retained speculative carrier");
        cache
            .insert_proposal(losing)
            .expect("insert losing speculative carrier");
        cache
            .insert_proposal(protected.clone())
            .expect("insert commit-protected losing carrier");
        cache
            .insert_proposal(historical)
            .expect("insert historical canonical carrier");
        for key in &keys {
            let prepare_vote = signed_vote(&protected.vote_body(CertPhase::Prepare), key);
            cache
                .insert_vote(prepare_vote.clone(), Some(&prepare_vote.signer))
                .expect("form the prerequisite PrepareQC");
        }
        let commit_vote = signed_vote(&protected.vote_body(CertPhase::Commit), &keys[0]);
        cache
            .insert_vote(commit_vote.clone(), Some(&commit_vote.signer))
            .expect("protect losing carrier with a Commit vote");

        assert_eq!(
            cache.retire_uncommitted_global_anchors_except(
                retained.descriptor.proposal_height,
                retained
                    .payload_block_hint
                    .as_ref()
                    .expect("retained carrier hint")
                    .proposal_block_hash,
            ),
            1
        );

        assert!(cache.get(&retained_key).is_some());
        assert!(cache.get(&losing_key).is_none());
        assert!(cache.get(&protected_key).is_some());
        assert!(cache.get(&historical_key).is_some());
        assert!(cache.contains_vote(&commit_vote));
    }

    #[test]
    fn lane_block_rollover_preserves_partial_votes_prepare_qc_and_commit_lock() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let partial = lane_block_proposal_at_height(&validator_set, 13);
        let canonical_partial =
            partial
                .clone()
                .with_payload_block_hint(LaneBlockProposalPayloadHintV1 {
                    proposal_height: partial.descriptor.proposal_height,
                    proposal_view: partial.descriptor.lane_block_view,
                    proposal_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                        b"rollover-partial-canonical-block",
                    )),
                });
        let prepared = lane_block_proposal_at_height(&validator_set, 14);
        let partial_key = LaneBlockSessionKey::from_proposal(&partial);
        let prepared_key = LaneBlockSessionKey::from_proposal(&prepared);
        let remote_vote = signed_vote(&partial.vote_body(CertPhase::Prepare), &keys[0]);
        let mut cache = LaneBlockSessionCache::new(4);

        cache
            .insert_proposal(partial)
            .expect("insert partial canonical proposal identity");
        cache
            .insert_vote(remote_vote.clone(), Some(&remote_vote.signer))
            .expect("insert partial remote prepare vote");
        cache
            .insert_proposal(prepared.clone())
            .expect("insert prepared canonical proposal");
        for key in &keys {
            let vote = signed_vote(&prepared.vote_body(CertPhase::Prepare), key);
            cache
                .insert_vote(vote.clone(), Some(&vote.signer))
                .expect("seal canonical prepare QC");
        }
        let commit_vote = signed_vote(&prepared.vote_body(CertPhase::Commit), &keys[0]);
        cache
            .insert_vote(commit_vote.clone(), Some(&commit_vote.signer))
            .expect("retain signer commit lock");
        let lock_count = cache.commit_vote_lock_len();

        cache
            .retain_canonical_rollover_evidence(
                1,
                |_, lane_height| match lane_height {
                    13 => Some(canonical_partial.clone()),
                    14 => Some(prepared.clone()),
                    _ => None,
                },
                |_, _, _, _| true,
                |_, _, _, _| true,
            )
            .expect("canonical rollover succeeds");

        assert!(cache.contains_vote(&remote_vote));
        assert_eq!(
            cache
                .get(&partial_key)
                .and_then(|session| session.proposal.as_ref()),
            Some(&canonical_partial),
            "rollover must normalize the advisory hint to the exact Kura anchor"
        );
        assert!(
            cache
                .get(&prepared_key)
                .is_some_and(|session| session.prepare_qc.is_some()),
            "a canonical PrepareQC must survive fast global-height rollover"
        );
        assert_eq!(cache.commit_vote_lock_len(), lock_count);
    }

    #[test]
    fn lane_block_rollover_prunes_unanchored_finalized_and_inactive_evidence() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let active = lane_block_proposal_at_height(&validator_set, 13);
        let unanchored = lane_block_proposal_at_height(&validator_set, 14);
        let inactive = rebind_lane_block_proposal_route(
            lane_block_proposal_at_height(&validator_set, 15),
            LaneId::new(8),
            DataSpaceId::new(12),
        );
        let finalized = lane_block_proposal_at_height(&validator_set, 16);
        let active_key = LaneBlockSessionKey::from_proposal(&active);
        let mut cache = LaneBlockSessionCache::new(8);

        for proposal in [&active, &unanchored, &inactive, &finalized] {
            cache
                .insert_proposal(proposal.clone())
                .expect("insert rollover pruning fixture");
        }
        for proposal in [&inactive, &finalized] {
            for key in &keys {
                let vote = signed_vote(&proposal.vote_body(CertPhase::Prepare), key);
                cache
                    .insert_vote(vote.clone(), Some(&vote.signer))
                    .expect("seal pruning fixture PrepareQC");
            }
            let vote = signed_vote(&proposal.vote_body(CertPhase::Commit), &keys[0]);
            cache
                .insert_vote(vote.clone(), Some(&vote.signer))
                .expect("record pruning fixture commit lock");
        }
        assert_eq!(cache.commit_vote_lock_len(), 2);

        cache
            .retain_canonical_rollover_evidence(
                8,
                |lane_id, lane_height| {
                    [active.clone(), inactive.clone(), finalized.clone()]
                        .into_iter()
                        .find(|proposal| {
                            proposal.descriptor.lane_id == lane_id
                                && proposal.descriptor.lane_block_height == lane_height
                        })
                },
                |lane_id, _, _, _| lane_id != inactive.descriptor.lane_id,
                |_, _, _, lane_height| lane_height != finalized.descriptor.lane_block_height,
            )
            .expect("prune non-rollover evidence");

        assert_eq!(cache.len(), 1);
        assert!(cache.get(&active_key).is_some());
        assert_eq!(cache.commit_vote_lock_len(), 0);
    }

    #[test]
    fn lane_block_rollover_fails_atomically_on_certified_canonical_conflict() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let canonical = lane_block_proposal_at_height(&validator_set, 13);
        let conflicting = retag_lane_block_proposal_payload(canonical.clone(), 0xD4);
        let mut cache = LaneBlockSessionCache::new(4);
        cache
            .insert_proposal(conflicting.clone())
            .expect("insert conflicting proposal");
        for key in &keys {
            let vote = signed_vote(&conflicting.vote_body(CertPhase::Prepare), key);
            cache
                .insert_vote(vote.clone(), Some(&vote.signer))
                .expect("seal conflicting PrepareQC");
        }
        let before = cache.clone();

        assert_eq!(
            cache.retain_canonical_rollover_evidence(
                4,
                |_, lane_height| (lane_height == 13).then(|| canonical.clone()),
                |_, _, _, _| true,
                |_, _, _, _| true,
            ),
            Err(LaneBlockSessionError::ConflictingProposal)
        );
        assert_eq!(cache, before, "conflict preflight must be mutation-free");
    }

    #[test]
    fn lane_block_rollover_fails_on_pruned_certified_commit_locks() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let canonical = lane_block_proposal_at_height(&validator_set, 13);
        let conflicting = retag_lane_block_proposal_payload(canonical.clone(), 0xD5);
        let mut cache = LaneBlockSessionCache::new(4);
        cache
            .insert_proposal(conflicting.clone())
            .expect("insert conflicting proposal");
        for phase in [CertPhase::Prepare, CertPhase::Commit] {
            for key in &keys {
                let vote = signed_vote(&conflicting.vote_body(phase), key);
                cache
                    .insert_vote(vote.clone(), Some(&vote.signer))
                    .expect("seal conflicting lane certificate");
            }
        }
        assert_eq!(cache.commit_vote_lock_len(), 3);
        assert_eq!(
            cache.retain_sessions_for_admissible_lanes(|_, _, _, _, _| false),
            1
        );
        assert!(cache.is_empty());
        let before = cache.clone();

        assert_eq!(
            cache.retain_canonical_rollover_evidence(
                4,
                |_, lane_height| (lane_height == 13).then(|| canonical.clone()),
                |_, _, _, _| true,
                |_, _, _, _| true,
            ),
            Err(LaneBlockSessionError::ConflictingProposal)
        );
        assert_eq!(
            cache, before,
            "surviving quorum commit locks must make conflict preflight mutation-free"
        );
    }

    #[test]
    fn lane_block_session_cache_prunes_inadmissible_lane_sessions_and_slot_claims() {
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
        let inactive_proposal = retag_lane_block_proposal_payload(
            rebind_lane_block_proposal_route(
                lane_block_proposal_at_height(&validator_set, 13),
                inactive_lane,
                inactive_dataspace,
            ),
            0xD0,
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
            cache.retain_sessions_for_admissible_lanes(
                |lane_id, dataspace_id, lane_incarnation, lane_block_height, _proposal_height| {
                    lane_id == active_lane
                        && dataspace_id == active_dataspace
                        && lane_incarnation == active_proposal.descriptor.lane_incarnation
                        && lane_block_height > 12
                },
            ),
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
    fn lane_block_session_cache_prunes_noncanonical_prepared_siblings_but_preserves_commit_evidence()
     {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let loser = lane_block_proposal_at_height(&validator_set, 13);
        let winner = conflicting_lane_block_proposal_next_view(loser.clone(), 0xD1);
        let protected = conflicting_lane_block_proposal_next_view(winner.clone(), 0xE1);
        let loser_key = LaneBlockSessionKey::from_proposal(&loser);
        let winner_key = LaneBlockSessionKey::from_proposal(&winner);
        let protected_key = LaneBlockSessionKey::from_proposal(&protected);
        let mut cache = LaneBlockSessionCache::new(8);

        for proposal in [&loser, &winner] {
            assert_eq!(
                cache.insert_proposal(proposal.clone()),
                Ok(LaneBlockSessionInsertOutcome::Inserted)
            );
            for signer in &keys {
                let vote = signed_vote(&proposal.vote_body(CertPhase::Prepare), signer);
                assert_eq!(
                    cache.insert_vote(vote.clone(), Some(&vote.signer)),
                    Ok(LaneBlockSessionInsertOutcome::Inserted)
                );
            }
        }
        assert!(cache.get(&loser_key).is_some_and(|session| {
            session.prepare_qc.is_some()
                && session.commit_votes.is_empty()
                && session.commit_qc.is_none()
        }));

        assert_eq!(
            cache.prune_uncommitted_sessions_conflicting_with_canonical_proposal(&winner),
            1
        );
        assert!(cache.get(&loser_key).is_none());
        assert!(cache.get(&winner_key).is_some());
        assert!(
            cache
                .proposals_without_commit_qc()
                .iter()
                .all(|proposal| proposal.proposal_hash == winner.proposal_hash)
        );
        assert!(
            cache
                .qcs_for_incomplete_sessions()
                .iter()
                .all(|qc| qc.body.proposal_hash == winner.proposal_hash)
        );
        assert!(
            cache
                .local_vote_rebroadcast_artifacts_for(&peer(&keys[0]))
                .iter()
                .all(|(proposal, _)| proposal.proposal_hash == winner.proposal_hash)
        );
        assert_eq!(
            cache.insert_proposal(loser.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted),
            "pruning must release the losing view's proposal slot"
        );
        assert_eq!(
            cache.prune_uncommitted_sessions_conflicting_with_canonical_proposal(&winner),
            1
        );

        assert_eq!(
            cache.insert_proposal(protected.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        for signer in &keys {
            let prepare_vote = signed_vote(&protected.vote_body(CertPhase::Prepare), signer);
            assert_eq!(
                cache.insert_vote(prepare_vote.clone(), Some(&prepare_vote.signer)),
                Ok(LaneBlockSessionInsertOutcome::Inserted)
            );
        }
        let commit_vote = signed_vote(&protected.vote_body(CertPhase::Commit), &keys[2]);
        assert_eq!(
            cache.insert_vote(commit_vote.clone(), Some(&commit_vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.prune_uncommitted_sessions_conflicting_with_canonical_proposal(&winner),
            0,
            "conflicting commit evidence must be retained for safety and diagnostics"
        );
        assert!(cache.get(&protected_key).is_some());
    }

    #[test]
    fn lane_block_session_cache_bounds_speculative_siblings_by_historical_context() {
        let keys = [
            checked_bls_keypair(1),
            checked_bls_keypair(2),
            checked_bls_keypair(3),
            checked_bls_keypair(4),
        ];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let base = lane_block_proposal_at_height(&validator_set, 13);
        let mut siblings = Vec::new();
        let mut cache = LaneBlockSessionCache::new(256);
        for view in 0_u64..100 {
            let proposal = lane_block_proposal_at_view(
                &base,
                view,
                u8::try_from(view).expect("fixture view fits u8"),
            );
            assert_eq!(
                cache.insert_proposal(proposal.clone()),
                Ok(LaneBlockSessionInsertOutcome::Inserted)
            );
            siblings.push(proposal);
        }

        for proposal in [&siblings[0], &siblings[1]] {
            for signer in &keys[..3] {
                let prepare_vote = signed_vote(&proposal.vote_body(CertPhase::Prepare), signer);
                assert_eq!(
                    cache.insert_vote(prepare_vote.clone(), Some(&prepare_vote.signer)),
                    Ok(LaneBlockSessionInsertOutcome::Inserted)
                );
            }
        }
        let commit_vote = signed_vote(&siblings[0].vote_body(CertPhase::Commit), &keys[0]);
        assert_eq!(
            cache.insert_vote(commit_vote.clone(), Some(&commit_vote.signer)),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        let commit_body = siblings[1].vote_body(CertPhase::Commit);
        let commit_votes = [
            signed_vote(&commit_body, &keys[1]),
            signed_vote(&commit_body, &keys[2]),
            signed_vote(&commit_body, &keys[3]),
        ];
        let commit_qc =
            aggregate_lane_block_votes_to_qc(commit_body, validator_set.clone(), &commit_votes)
                .expect("disjoint-signer commit QC");
        assert_eq!(
            cache.insert_qc(commit_qc),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        let canonical_key = LaneBlockSessionKey::from_proposal(&siblings[2]);
        let canonical = BTreeSet::from([canonical_key]);
        let mut other_height_base = base.clone();
        other_height_base.descriptor.proposal_height = other_height_base
            .descriptor
            .proposal_height
            .saturating_add(1);
        other_height_base.descriptor.descriptor_hash =
            other_height_base.descriptor.computed_descriptor_hash();
        other_height_base.proposal_hash = other_height_base.computed_proposal_hash();
        for view in 100_u64..103 {
            let proposal = lane_block_proposal_at_view(
                &other_height_base,
                view,
                u8::try_from(view).expect("fixture view fits u8"),
            );
            assert_eq!(
                cache.insert_proposal(proposal),
                Ok(LaneBlockSessionInsertOutcome::Inserted)
            );
        }

        assert_eq!(
            cache.prune_excess_speculative_siblings(2, &canonical),
            96,
            "only two ordinary siblings per proposal-height route context may remain"
        );
        let first_height_views = cache
            .sessions
            .iter()
            .filter_map(|(key, session)| {
                (session_proposal_height(session) == Some(base.descriptor.proposal_height))
                    .then_some(key.lane_block_view)
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(first_height_views, BTreeSet::from([0, 1, 2, 98, 99]));
        let second_height_views = cache
            .sessions
            .iter()
            .filter_map(|(key, session)| {
                (session_proposal_height(session)
                    == Some(other_height_base.descriptor.proposal_height))
                .then_some(key.lane_block_view)
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(second_height_views, BTreeSet::from([101, 102]));

        assert_eq!(
            cache.prune_uncommitted_sessions_below_proposal_view(
                base.descriptor.proposal_height,
                99,
                &canonical,
            ),
            1
        );
        let first_height_views = cache
            .sessions
            .iter()
            .filter_map(|(key, session)| {
                (session_proposal_height(session) == Some(base.descriptor.proposal_height))
                    .then_some(key.lane_block_view)
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            first_height_views,
            BTreeSet::from([0, 1, 2, 99]),
            "view pruning must preserve canonical and commit-evidence siblings"
        );
    }

    include!("lane_consensus/session_capacity_tests.rs");

    include!("lane_consensus/commit_vote_lock_incarnation_test.rs");

    #[test]
    fn lane_block_session_cache_reports_undrained_committed_admissible_lanes() {
        let keys = [checked_bls_keypair(1), checked_bls_keypair(2)];
        let mut validator_set = keys.iter().map(peer).collect::<Vec<_>>();
        validator_set.sort();
        let pending_lane = LaneId::new(7);
        let pending_dataspace = DataSpaceId::new(11);
        let drained_lane = LaneId::new(8);
        let drained_dataspace = DataSpaceId::new(12);
        let inactive_lane = LaneId::new(9);
        let inactive_dataspace = DataSpaceId::new(13);
        let pending_proposal = rebind_lane_block_proposal_route(
            lane_block_proposal_at_height(&validator_set, 13),
            pending_lane,
            pending_dataspace,
        );
        let drained_proposal = retag_lane_block_proposal_payload(
            rebind_lane_block_proposal_route(
                lane_block_proposal_at_height(&validator_set, 14),
                drained_lane,
                drained_dataspace,
            ),
            0x80,
        );
        let inactive_proposal = retag_lane_block_proposal_payload(
            rebind_lane_block_proposal_route(
                lane_block_proposal_at_height(&validator_set, 15),
                inactive_lane,
                inactive_dataspace,
            ),
            0xC0,
        );
        let prepare_body = drained_proposal.vote_body(CertPhase::Prepare);
        let prepare_vote_a = signed_vote(&prepare_body, &keys[0]);
        let prepare_vote_b = signed_vote(&prepare_body, &keys[1]);
        let prepare_qc = aggregate_lane_block_votes_to_qc(
            prepare_body,
            drained_proposal.descriptor.validator_set.clone(),
            &[prepare_vote_a, prepare_vote_b],
        )
        .expect("prepare QC");
        let commit_body = drained_proposal.vote_body(CertPhase::Commit);
        let commit_vote_a = signed_vote(&commit_body, &keys[0]);
        let commit_vote_b = signed_vote(&commit_body, &keys[1]);
        let commit_qc = aggregate_lane_block_votes_to_qc(
            commit_body,
            drained_proposal.descriptor.validator_set.clone(),
            &[commit_vote_a, commit_vote_b],
        )
        .expect("commit QC");
        let pops = signer_pops(&keys);
        let mut cache = LaneBlockSessionCache::new(4);

        assert_eq!(
            cache.insert_proposal(pending_proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(drained_proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(prepare_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_qc_with_pops(commit_qc, &pops),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );
        assert_eq!(
            cache.insert_proposal(inactive_proposal.clone()),
            Ok(LaneBlockSessionInsertOutcome::Inserted)
        );

        let admissible_pending = cache.pending_lane_ids_for_admissible_lanes(
            |lane_id, dataspace_id, lane_incarnation, lane_block_height, _proposal_height| {
                (lane_id == pending_lane
                    && dataspace_id == pending_dataspace
                    && lane_incarnation == pending_proposal.descriptor.lane_incarnation
                    && lane_block_height == pending_proposal.descriptor.lane_block_height)
                    || (lane_id == drained_lane
                        && dataspace_id == drained_dataspace
                        && lane_incarnation == drained_proposal.descriptor.lane_incarnation
                        && lane_block_height == drained_proposal.descriptor.lane_block_height)
            },
        );
        assert_eq!(
            admissible_pending,
            BTreeSet::from([drained_lane]),
            "only commit-certified sessions should block their lanes before drain"
        );

        let admissible_inflight_before_drain = cache.inflight_lane_ids_for_admissible_lanes(
            |lane_id,
             dataspace_id,
             lane_incarnation,
             lane_block_height,
             _proposal_height,
             _has_consensus_evidence| {
                (lane_id == pending_lane
                    && dataspace_id == pending_dataspace
                    && lane_incarnation == pending_proposal.descriptor.lane_incarnation
                    && lane_block_height == pending_proposal.descriptor.lane_block_height)
                    || (lane_id == drained_lane
                        && dataspace_id == drained_dataspace
                        && lane_incarnation == drained_proposal.descriptor.lane_incarnation
                        && lane_block_height == drained_proposal.descriptor.lane_block_height)
            },
        );
        assert_eq!(
            admissible_inflight_before_drain,
            BTreeSet::from([pending_lane, drained_lane]),
            "in-flight proposal planning should block both uncertified and commit-certified sessions before drain"
        );

        assert_eq!(cache.drain_committed_sessions().len(), 1);
        let admissible_after_drain = cache.pending_lane_ids_for_admissible_lanes(
            |lane_id, dataspace_id, lane_incarnation, lane_block_height, _proposal_height| {
                (lane_id == pending_lane
                    && dataspace_id == pending_dataspace
                    && lane_incarnation == pending_proposal.descriptor.lane_incarnation
                    && lane_block_height == pending_proposal.descriptor.lane_block_height)
                    || (lane_id == drained_lane
                        && dataspace_id == drained_dataspace
                        && lane_incarnation == drained_proposal.descriptor.lane_incarnation
                        && lane_block_height == drained_proposal.descriptor.lane_block_height)
                    || (lane_id == inactive_lane
                        && dataspace_id == inactive_dataspace
                        && lane_incarnation == inactive_proposal.descriptor.lane_incarnation
                        && lane_block_height == inactive_proposal.descriptor.lane_block_height)
            },
        );
        assert_eq!(
            admissible_after_drain,
            BTreeSet::new(),
            "drained committed and uncertified sessions should not block proposal retries"
        );

        let admissible_inflight_after_drain = cache.inflight_lane_ids_for_admissible_lanes(
            |lane_id,
             dataspace_id,
             lane_incarnation,
             lane_block_height,
             _proposal_height,
             _has_consensus_evidence| {
                (lane_id == pending_lane
                    && dataspace_id == pending_dataspace
                    && lane_incarnation == pending_proposal.descriptor.lane_incarnation
                    && lane_block_height == pending_proposal.descriptor.lane_block_height)
                    || (lane_id == drained_lane
                        && dataspace_id == drained_dataspace
                        && lane_incarnation == drained_proposal.descriptor.lane_incarnation
                        && lane_block_height == drained_proposal.descriptor.lane_block_height)
                    || (lane_id == inactive_lane
                        && dataspace_id == inactive_dataspace
                        && lane_incarnation == inactive_proposal.descriptor.lane_incarnation
                        && lane_block_height == inactive_proposal.descriptor.lane_block_height)
            },
        );
        assert_eq!(
            admissible_inflight_after_drain,
            BTreeSet::from([pending_lane, inactive_lane]),
            "drained committed sessions should leave the in-flight set after drain"
        );
    }

    // Backpressure and adversarial vote-set tests retain their stable libtest paths.
    include!("lane_consensus/backpressure_and_vote_set_tests.rs");
}
