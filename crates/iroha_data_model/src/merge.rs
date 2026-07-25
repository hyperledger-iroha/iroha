//! Merge-ledger data structures.
//!
//! The merge ledger records a compact ordered log of lane tips along with the
//! deterministic global reduction state used to finalize world state updates.
//! These DTOs provide the on-wire and persistence representations of merge
//! entries. See `docs/source/merge_ledger.md` for the normative behaviour the
//! runtime must enforce when producing and validating these records.

use iroha_crypto::{Hash, HashOf, MerkleTree, PublicKey};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    block::{
        BlockHeader,
        consensus::{
            LaneBlockCommitment, LaneBlockProposalV1, LaneBlockQcV1, NativeAmxReceipt,
            ValidatorIndex,
        },
        consensus_v2::finality::V2FinalityArtifact,
    },
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    peer::PeerId,
    transaction::signed::{TransactionEntrypoint, TransactionResult},
};

const MERGE_LEDGER_ENTRY_HASH_DOMAIN: &[u8] = b"iroha:merge:ledger-entry:v2\0";
const LANE_DRAIN_INTENT_HASH_DOMAIN: &[u8] = b"iroha:nexus:lane-drain-intent:v1\0";
const LANE_DRAIN_CERTIFICATE_HASH_DOMAIN: &[u8] = b"iroha:nexus:lane-drain-certificate:v1\0";
const LANE_DRAIN_CERTIFICATE_SIGNATURE_DOMAIN: &[u8] =
    b"iroha:nexus:lane-drain-certificate-signature:v1\0";
const LANE_DRAIN_EMPTY_UNRESOLVED_EVIDENCE_ROOT_DOMAIN: &[u8] =
    b"iroha:nexus:lane-drain:unresolved-evidence:empty:v1\0";

/// Current-only first-release merge-ledger entry layout.
///
/// Version two adds globally ordered queue-plan admission controls. Version one
/// has no compatibility path and is intentionally rejected by live consensus.
pub const MERGE_LEDGER_ENTRY_VERSION_V2: u8 = 2;

/// Maximum canonical framed size of one full merge-ledger entry.
///
/// This is the protocol-wide limit used by pending sidecars, compact block
/// references, recovery, and admission. Keep all consumers on this constant so
/// a certified entry cannot be accepted by one layer and rejected by another.
pub const MAX_MERGE_LEDGER_ENTRY_BYTES: usize = 16 * 1024 * 1024;

/// Maximum canonical size of the execution-batch field inside a merge entry.
///
/// Four MiB of the full-entry envelope is reserved for the active-lane set,
/// snapshots, merge QC, and Norito framing. Admission still checks the exact
/// final full-entry size after the QC is attached.
pub const MAX_MERGE_EXECUTION_BATCH_BYTES: usize = 12 * 1024 * 1024;

/// Maximum number of ordered entrypoints in one certified merge execution batch.
///
/// Lane-local proposal admission uses the same ceiling so every certified lane
/// block is individually eligible for inclusion in a global merge batch.
pub const MAX_MERGE_EXECUTION_ENTRYPOINTS: usize = 4_096;

/// Maximum canonical size of one authenticated autonomous-lane source bundle.
///
/// A merge execution repeats selected payload material beside this bundle and
/// adds deterministic results, settlement evidence, and committee proofs. The
/// four-MiB source ceiling therefore preserves enough of the 12-MiB execution
/// envelope for every individually certified lane source to remain mergeable.
pub const MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES: usize = 4 * 1024 * 1024;

/// Maximum number of globally ordered queue-plan admission controls in one entry.
pub const MAX_MERGE_QUEUE_PLAN_ADMISSIONS: usize = 4_096;

/// Maximum canonical size of one opaque queue-plan admission certificate.
pub const MAX_MERGE_QUEUE_PLAN_ADMISSION_BYTES: usize = 1024 * 1024;

/// Maximum aggregate queue-plan admission bytes carried by one merge entry.
pub const MAX_MERGE_QUEUE_PLAN_ADMISSIONS_BYTES: usize = 4 * 1024 * 1024;

/// Maximum canonical size reserved for the certified-proposal half of one source bundle.
///
/// The certified envelope repeats the bounded entrypoint commitments in the proposal and
/// prepare/commit vote bodies and carries the validator sets, availability QC, and signer `PoPs`.
/// Keeping this reservation protocol-wide prevents an autonomous view chain from consuming the
/// complete source budget before the globally executable certificate is attached.
pub const MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES: usize = 1024 * 1024;

/// Maximum canonical size of the autonomous payload and authenticated view-chain half.
///
/// `NewView` persistence checkpoints before this budget is exceeded. The independently bounded
/// certified envelope and the exact final bundle check then keep every accepted source within
/// [`MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES`].
pub const MAX_MERGE_EXECUTION_AUTONOMOUS_SOURCE_BYTES: usize =
    MAX_MERGE_EXECUTION_SOURCE_BUNDLE_BYTES - MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES;

/// Current versioned merge-committee share layout.
///
/// Version two is the first layout that transports the leader's exact pre-QC
/// candidate body. The preceding unversioned layout has no compatibility path
/// and is intentionally not admitted by live consensus.
pub const MERGE_COMMITTEE_SIGNATURE_VERSION_V2: u8 = 2;

/// Return whether an exact canonical full-entry length is protocol-admissible.
#[must_use]
pub const fn merge_ledger_entry_size_within_limit(encoded_len: usize) -> bool {
    encoded_len <= MAX_MERGE_LEDGER_ENTRY_BYTES
}

/// Return whether an exact canonical execution-batch length fits its reserved envelope.
#[must_use]
pub const fn merge_execution_batch_size_within_limit(encoded_len: usize) -> bool {
    encoded_len <= MAX_MERGE_EXECUTION_BATCH_BYTES
}

/// Return whether opaque queue-plan admission bytes fit their protocol envelope.
#[must_use]
pub fn merge_queue_plan_admissions_within_limits(admissions: &[Vec<u8>]) -> bool {
    if admissions.len() > MAX_MERGE_QUEUE_PLAN_ADMISSIONS {
        return false;
    }
    admissions
        .iter()
        .try_fold(0_usize, |total, admission| {
            if admission.is_empty() || admission.len() > MAX_MERGE_QUEUE_PLAN_ADMISSION_BYTES {
                return None;
            }
            total.checked_add(admission.len())
        })
        .is_some_and(|total| total <= MAX_MERGE_QUEUE_PLAN_ADMISSIONS_BYTES)
}

/// Proof of possession for one signer selected by a merge QC bitmap.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeSignerProof {
    /// Signer index in [`MergeQuorumCertificate::validator_set`].
    pub signer: ValidatorIndex,
    /// BLS proof of possession for the indexed validator key.
    pub proof_of_possession: Vec<u8>,
}

/// Exact durable Native AMX application identity required by a drain frontier.
///
/// The hashes bind the independently persisted finality, manifest, receipt,
/// and bounded latest-index artifacts. Runtime admission must re-read and
/// fully revalidate those artifacts before treating this evidence as applied.
/// This is control evidence only; economic effects were executed by the named
/// canonical global application block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct LaneDrainNativeFrontierEvidenceV1 {
    /// Exact evidence layout version. Only version one is valid.
    pub version: u16,
    /// Participant-local consensus view.
    pub participant_view: u64,
    /// Exact predecessor participant-local height.
    pub predecessor_height: u64,
    /// Exact predecessor descriptor, absent only at incarnation genesis.
    pub predecessor_descriptor_hash: Option<Hash>,
    /// Exact participant proposal identity.
    pub participant_proposal_hash: Hash,
    /// Exact zero-effect participant settlement identity.
    pub participant_settlement_hash: HashOf<LaneBlockCommitment>,
    /// Number of unique grouped source transactions applied by the carrier.
    pub source_count: u32,
    /// Canonical global application height.
    pub application_block_height: u64,
    /// Canonical global application block identity.
    pub application_block_hash: HashOf<BlockHeader>,
    /// Hash of the canonical result-bearing global block wire.
    pub executed_block_wire_hash: Hash,
    /// Hash of the independently persisted and verified finality artifact.
    pub finality_artifact_hash: HashOf<V2FinalityArtifact>,
    /// Native application-manifest root authenticated by finality.
    pub application_manifest_root: Hash,
    /// Number of canonical route leaves authenticated by the manifest root.
    pub application_manifest_leaf_count: u32,
    /// Position of this route's leaf in canonical route order.
    pub application_manifest_leaf_index: u32,
    /// Hash of the exact durable per-route manifest leaf/proof artifact.
    pub manifest_artifact_hash: Hash,
    /// Hash of the exact durable participant application receipt artifact.
    pub receipt_artifact_hash: Hash,
    /// Hash of the exact bounded route/incarnation latest-index artifact.
    pub latest_index_artifact_hash: Hash,
}

impl LaneDrainNativeFrontierEvidenceV1 {
    /// Current exact first-release evidence layout version.
    pub const VERSION: u16 = 1;
}

/// One evidence-aware lane frontier shared by every drain phase.
///
/// `native_application` is present only when the replicated WSV frontier was
/// advanced by a separate Native AMX participant control. Same-route
/// coordinator legs never create this evidence. `unresolved_evidence_root`
/// must be the canonical empty root at every signing and retirement boundary;
/// local blocker predicates still decide which work contributes to the root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct LaneDrainFrontierV1 {
    /// Exact frontier layout version. Only version one is valid.
    pub version: u8,
    /// Lane whose contiguous frontier is bound.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane.
    pub dataspace_id: DataSpaceId,
    /// Exact lane incarnation.
    pub lane_incarnation: Hash,
    /// Contiguous lane-local height.
    pub lane_block_height: u64,
    /// Descriptor at `lane_block_height`, absent only for height zero.
    pub lane_block_descriptor_hash: Option<Hash>,
    /// Fully durable Native evidence when the frontier is Native-derived.
    pub native_application: Option<LaneDrainNativeFrontierEvidenceV1>,
    /// Canonical root of unresolved work/evidence. Drain admission requires the
    /// protocol empty root.
    pub unresolved_evidence_root: Hash,
}

impl LaneDrainFrontierV1 {
    /// Current exact first-release layout version.
    pub const VERSION: u8 = 1;

    /// Build an ordinary (non-Native-derived) frontier.
    #[must_use]
    pub fn ordinary(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        lane_block_height: u64,
        lane_block_descriptor_hash: Option<Hash>,
    ) -> Self {
        Self {
            version: Self::VERSION,
            lane_id,
            dataspace_id,
            lane_incarnation,
            lane_block_height,
            lane_block_descriptor_hash,
            native_application: None,
            unresolved_evidence_root: lane_drain_empty_unresolved_evidence_root(),
        }
    }

    /// Return whether this frontier binds the supplied active route.
    #[must_use]
    pub fn matches_route(
        &self,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
    ) -> bool {
        self.lane_id == lane_id
            && self.dataspace_id == dataspace_id
            && self.lane_incarnation == lane_incarnation
    }
}

/// Canonical root proving that no unresolved drain evidence was selected.
#[must_use]
pub fn lane_drain_empty_unresolved_evidence_root() -> Hash {
    Hash::new(LANE_DRAIN_EMPTY_UNRESOLVED_EVIDENCE_ROOT_DOMAIN)
}

/// Canonical first phase of an automatic lane retirement.
///
/// Committing an intent closes the named lane to new work after
/// `close_global_height`. It does not authorize retirement. The authoritative
/// lane committee must subsequently certify a final contiguous lane frontier,
/// and that certificate must be carried and applied by the global merge ledger.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct LaneDrainIntentV1 {
    /// Schema version. Only version one is valid.
    pub version: u8,
    /// Domain-separated digest of the chain identifier.
    pub chain_id_digest: Hash,
    /// Lane being closed to new work.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane at the close boundary.
    pub dataspace_id: DataSpaceId,
    /// Exact active lane incarnation being closed.
    pub lane_incarnation: Hash,
    /// Global height after which new lane work is inadmissible.
    pub close_global_height: u64,
    /// Evidence-aware globally applied frontier when draining began.
    pub initial_frontier: LaneDrainFrontierV1,
    /// Version of the canonical lane-committee hashing scheme.
    pub validator_set_hash_version: u16,
    /// Hash of the exact authoritative lane committee at the close boundary.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Exact authoritative lane committee at the close boundary.
    ///
    /// Persisting the order makes drain voting restart-safe even after the
    /// global commit topology or stake election changes.
    pub validator_set: Vec<PeerId>,
    /// Number of validators in the authoritative lane committee.
    pub validator_count: u32,
    /// Minimum distinct signers required for a drain certificate.
    pub min_quorum: u32,
}

impl LaneDrainIntentV1 {
    /// Return the domain-separated canonical intent hash.
    #[must_use]
    pub fn canonical_hash(&self) -> HashOf<Self> {
        let bytes = norito::to_bytes(self)
            .expect("lane drain intent must have a canonical Norito encoding");
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            LANE_DRAIN_INTENT_HASH_DOMAIN,
            bytes.as_slice(),
        ]))
    }
}

/// Lane-committee statement that no certified successor exists beyond one
/// final contiguous frontier for a committed drain intent.
///
/// Honest lane validators persist a close lock before signing this body and
/// refuse both a frontier below any commit QC they have signed and every later
/// lane-block commit for the closed incarnation. Quorum intersection therefore
/// prevents a certificate from coexisting with a higher certified lane block.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct LaneDrainCertificateBodyV1 {
    /// Schema version. Only version one is valid.
    pub version: u8,
    /// Exact committed drain intent authorized by this certificate.
    pub intent: LaneDrainIntentV1,
    /// Final evidence-aware contiguous frontier certified by the committee.
    pub final_frontier: LaneDrainFrontierV1,
}

impl LaneDrainCertificateBodyV1 {
    /// Build the domain-separated BLS signature preimage.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let encoded = norito::to_bytes(self)
            .expect("lane drain certificate body must have a canonical Norito encoding");
        let mut preimage =
            Vec::with_capacity(LANE_DRAIN_CERTIFICATE_SIGNATURE_DOMAIN.len() + encoded.len());
        preimage.extend_from_slice(LANE_DRAIN_CERTIFICATE_SIGNATURE_DOMAIN);
        preimage.extend_from_slice(&encoded);
        preimage
    }
}

/// Self-contained quorum certificate closing one lane incarnation at an exact
/// globally applied frontier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneDrainCertificateV1 {
    /// Body signed by the authoritative lane committee.
    pub body: LaneDrainCertificateBodyV1,
    /// Exact historical validator set indexed by `signers_bitmap`.
    pub validator_set: Vec<PeerId>,
    /// Bitmap encoding of participating validators (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// Canonical `PoPs` for the selected signer indices, in bitmap order.
    pub signer_proofs: Vec<MergeSignerProof>,
    /// Aggregate BLS-normal signature over [`LaneDrainCertificateBodyV1::signature_preimage`].
    pub aggregate_signature: Vec<u8>,
}

impl LaneDrainCertificateV1 {
    /// Return the domain-separated canonical certificate hash.
    #[must_use]
    pub fn canonical_hash(&self) -> HashOf<Self> {
        let bytes = norito::to_bytes(self)
            .expect("lane drain certificate must have a canonical Norito encoding");
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            LANE_DRAIN_CERTIFICATE_HASH_DOMAIN,
            bytes.as_slice(),
        ]))
    }
}

/// Globally carried proof that an exact lane drain certificate was accepted at
/// a specific canonical carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct LaneDrainCommitmentV1 {
    /// Exact commitment layout version. Only version one is valid.
    pub version: u8,
    /// Exact certificate accepted by the merge committee.
    pub certificate_hash: HashOf<LaneDrainCertificateV1>,
    /// Full merge entry that globally ordered the certificate.
    pub merge_entry_hash: HashOf<MergeLedgerEntry>,
    /// Canonical global block height that carried the merge entry.
    pub carrier_height: u64,
    /// Exact signed evidence-aware frontier carried by the certificate.
    pub frontier: LaneDrainFrontierV1,
}

impl LaneDrainCommitmentV1 {
    /// Current exact first-release commitment layout version.
    pub const VERSION: u8 = 1;
}

/// Consensus-persisted two-phase drain state embedded in an autoscale-managed
/// lane's reserved metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct LaneDrainStateV1 {
    /// Schema version. Only version one is valid.
    pub version: u8,
    /// Canonical close intent that made the lane reject new work.
    pub intent: LaneDrainIntentV1,
    /// Globally carried certificate commitment once the lane is fully drained.
    pub commitment: Option<LaneDrainCommitmentV1>,
}

impl LaneDrainStateV1 {
    /// Return `true` once the exact lane committee certificate has been
    /// globally ordered by a later merge carrier.
    #[must_use]
    pub const fn is_certified(&self) -> bool {
        self.commitment.is_some()
    }
}

/// Canonical active lane incarnation and first eligible proposal height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeLaneBinding {
    /// Active lane identifier.
    pub lane_id: LaneId,
    /// Dataspace bound to the active lane.
    pub dataspace_id: DataSpaceId,
    /// Canonical hash of the active lane configuration.
    pub lane_config_hash: Hash,
    /// Active incarnation commitment.
    pub incarnation: Hash,
    /// First global proposal height eligible to use this incarnation.
    pub activation_height: u64,
}

/// BFT quorum certificate produced by the merge committee for a merge-ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeQuorumCertificate {
    /// View number in which the merge committee formed the certificate.
    pub view: u64,
    /// Epoch identifier active when the merge entry was finalized.
    pub epoch_id: u64,
    /// Exact global block height authorized to carry this merge entry.
    pub carrier_height: u64,
    /// Exact canonical parent authorized for the global carrier.
    pub carrier_parent_hash: HashOf<BlockHeader>,
    /// Domain-separated digest of the chain identifier sealed by this QC.
    pub chain_id_digest: Hash,
    /// Version of the canonical validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Hash of the exact ordered validator set used by the signer bitmap.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Exact historical validator set used to form this QC.
    pub validator_set: Vec<PeerId>,
    /// Bitmap encoding of participating validators (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// Exact, canonical signer `PoPs` required to verify the aggregate signature after restart.
    pub signer_proofs: Vec<MergeSignerProof>,
    /// Aggregate signature bytes covering the serialized merge entry payload.
    pub aggregate_signature: Vec<u8>,
    /// Deterministic transcript hash used when verifying the certificate.
    pub message_digest: Hash,
}

impl MergeQuorumCertificate {
    /// Construct a new quorum certificate using explicit fields.
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor intentionally mirrors every fixed quorum-certificate field"
    )]
    pub fn new(
        view: u64,
        epoch_id: u64,
        carrier_height: u64,
        carrier_parent_hash: HashOf<BlockHeader>,
        chain_id_digest: Hash,
        validator_set_hash_version: u16,
        validator_set_hash: HashOf<Vec<PeerId>>,
        validator_set: Vec<PeerId>,
        signers_bitmap: Vec<u8>,
        signer_proofs: Vec<MergeSignerProof>,
        aggregate_signature: Vec<u8>,
        message_digest: Hash,
    ) -> Self {
        Self {
            view,
            epoch_id,
            carrier_height,
            carrier_parent_hash,
            chain_id_digest,
            validator_set_hash_version,
            validator_set_hash,
            validator_set,
            signers_bitmap,
            signer_proofs,
            aggregate_signature,
            message_digest,
        }
    }
}

/// Signature share emitted by a merge-committee member.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct MergeCommitteeSignature {
    /// Current-only first-release wire layout version.
    ///
    /// Legacy unversioned shares are not accepted by live consensus.
    pub version: u8,
    /// Merge-ledger entry epoch/height being signed.
    pub epoch_id: u64,
    /// Merge-committee view index aligned with lane tips for this entry.
    pub view: u64,
    /// Signer index in the merge-committee roster.
    pub signer: ValidatorIndex,
    /// Deterministic transcript hash used when verifying the signature.
    pub message_digest: Hash,
    /// BLS signature payload for the merge entry digest.
    pub bls_sig: Vec<u8>,
    /// Exact canonical pre-QC candidate body distributed by the round leader.
    ///
    /// The frozen round leader must include this body on every transmission;
    /// every other signer must leave it absent. Runtime admission
    /// canonical-decodes, fully revalidates, and durably persists these bytes
    /// before emitting a follower share.
    pub leader_candidate_body: Option<Vec<u8>>,
}

/// Canonical per-lane snapshot recorded inside a merge-ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct MergeLaneSnapshot {
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Active incarnation commitment for the lane-local height namespace.
    pub lane_incarnation: Hash,
    /// First global proposal height eligible to use this incarnation.
    pub incarnation_activation_height: u64,
    /// Global proposal height that selected this lane-local block.
    pub proposal_height: u64,
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Lane-local block height represented by this snapshot.
    pub lane_block_height: u64,
    /// Canonical tip hash for the lane.
    pub tip_hash: HashOf<BlockHeader>,
    /// Merge-hint root associated with this lane snapshot.
    pub merge_hint_root: Hash,
    /// Exact settlement payload durably replayed after merge-log recovery.
    pub settlement_commitment: LaneBlockCommitment,
    /// Hash binding [`Self::settlement_commitment`] to the certified relay.
    pub settlement_hash: HashOf<LaneBlockCommitment>,
    /// Exact authenticated relay envelope from which every snapshot field was derived.
    ///
    /// An explicitly encoded `None` keeps malformed/missing-proof candidates
    /// decodable for deterministic rejection. Omitting this field is not a
    /// supported legacy layout. Production admission requires `Some` and never
    /// consults a follower's opportunistic relay cache.
    pub relay_envelope: Option<LaneRelayEnvelope>,
}

/// Proof of possession retained for a signer of an embedded lane-local QC.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeLaneSignerProof {
    /// BLS public key whose ownership is proven.
    pub public_key: PublicKey,
    /// BLS proof of possession for `public_key`.
    pub proof_of_possession: Vec<u8>,
}

/// One commit-certified lane block and its deterministic execution transcript.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeLaneExecution {
    /// Canonical framed Norito bytes of the complete producer-authenticated
    /// payload, availability certificate, `NewView` chain, and lane QCs.
    pub source_bundle: Vec<u8>,
    /// Hash of `source_bundle`, used for holder fetch and corruption checks.
    pub source_bundle_hash: Hash,
    /// Commit-certified lane-local proposal.
    pub proposal: LaneBlockProposalV1,
    /// Producer-authenticated origin proposal whose view owns the durable queue
    /// reservations; `proposal` may be a later quorum-authorized view.
    pub origin_proposal: LaneBlockProposalV1,
    /// Prepare QC that also proves quorum payload availability.
    pub prepare_qc: LaneBlockQcV1,
    /// Commit QC authorizing execution of the proposal.
    pub commit_qc: LaneBlockQcV1,
    /// Canonically ordered `PoPs` needed to verify both embedded QCs after restart.
    pub signer_proofs: Vec<MergeLaneSignerProof>,
    /// Chain binding of the producer-authenticated autonomous payload.
    pub autonomous_chain_id_hash: Hash,
    /// Consensus epoch bound into the autonomous payload.
    pub autonomous_epoch: u64,
    /// Canonical digest of the exact autonomous executable payload.
    pub autonomous_payload_hash: Hash,
    /// Entrypoint hashes in descriptor order.
    pub entrypoint_hashes: Vec<Hash>,
    /// Exact entrypoints executed in descriptor order.
    pub entrypoints: Vec<TransactionEntrypoint>,
    /// Canonical framed Norito encodings of the exact durable queue reservation
    /// keys, aligned one-for-one with `entrypoints`.
    ///
    /// The concrete reservation type belongs to `iroha_core`, so the data model
    /// retains its exact canonical bytes without introducing a dependency cycle.
    /// Producers must use [`norito::to_bytes`]; headerless codec payloads are not
    /// valid at this protocol boundary.
    pub reservation_keys: Vec<Vec<u8>>,
    /// Canonical framed Norito encodings of the complete routing plans, aligned
    /// one-for-one with `entrypoints` and `reservation_keys`.
    /// Producers must use [`norito::to_bytes`]; headerless codec payloads are not
    /// valid at this protocol boundary.
    pub routing_plans: Vec<Vec<u8>>,
    /// Producer-authenticated native-AMX receipts aligned one-for-one with
    /// `entrypoints` and routing plans (`Some` only for native-AMX plans).
    pub native_amx_receipts: Vec<Option<NativeAmxReceipt>>,
    /// Deterministic result hashes in descriptor order.
    pub result_hashes: Vec<Hash>,
    /// Exact deterministic results in descriptor order.
    pub results: Vec<TransactionResult>,
    /// Settlement, Nexus-fee, and native-AMX evidence derived by the same execution.
    pub settlement_commitment: LaneBlockCommitment,
    /// Canonical hash of `settlement_commitment`.
    pub settlement_hash: HashOf<LaneBlockCommitment>,
}

/// Merge-committee-certified total-order execution batch.
///
/// The batch is self-contained so a node that crashes after the merge-log append
/// but before WSV publication can replay the exact transition without trusting
/// lane sidecars or local QC arrival order.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct MergeExecutionBatch {
    /// Schema version. Version one is the only currently valid value.
    pub version: u8,
    /// Canonical committed WSV height from which execution starts.
    pub base_state_height: u64,
    /// Canonical committed WSV identity from which execution starts.
    pub base_state_hash: HashOf<BlockHeader>,
    /// Deterministic block context used for stateful execution.
    pub application_block_header: BlockHeader,
    /// Lane executions in the canonical merge total order.
    pub lanes: Vec<MergeLaneExecution>,
    /// Number of ordered entrypoint/result leaves committed by this batch.
    pub entrypoint_count: u64,
    /// Canonical Merkle root of entrypoint hashes in lane/batch execution order.
    pub entrypoint_merkle_root: HashOf<MerkleTree<TransactionEntrypoint>>,
    /// Canonical Merkle root of result hashes in the same execution order.
    pub result_merkle_root: HashOf<MerkleTree<TransactionResult>>,
    /// Merkle-style domain hash of the ordered lane execution transcripts.
    pub execution_root: Hash,
    /// Canonical root of execution, settlement, event, and transaction-membership
    /// effects before deterministic replay markers are added.
    pub application_write_set_root: Hash,
    /// Canonical root of the complete ordered WSV write set, including the
    /// deterministic replay markers derived from the stable batch identity.
    pub write_set_root: Hash,
    /// Expected post-state identity derived from the canonical base WSV and write-set root.
    pub expected_post_state_hash: HashOf<BlockHeader>,
    /// Canonical digest of every preceding field in this batch.
    pub batch_hash: Hash,
}

/// Ordered log entry produced by the merge ledger.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct MergeLedgerEntry {
    /// Exact first-release entry layout. Only version two is supported.
    pub version: u8,
    /// Epoch in which the entry was committed.
    pub epoch_id: u64,
    /// Canonical hash of the active lane catalog used for admission.
    pub lane_catalog_hash: Hash,
    /// Canonical exact active lane bindings used for historical verification.
    pub active_lanes: Vec<MergeLaneBinding>,
    /// Root of the canonical `(lane_id, incarnation)` set.
    pub incarnation_root: Hash,
    /// Root of the canonical `(lane_id, incarnation, activation_height)` set.
    pub activation_root: Hash,
    /// Canonical per-lane snapshots included in this merge entry.
    pub lane_snapshots: Vec<MergeLaneSnapshot>,
    /// Deterministic reduction of `merge_hint_roots` across all lanes.
    pub global_state_root: Hash,
    /// Merge committee quorum certificate sealing the entry.
    pub merge_qc: MergeQuorumCertificate,
    /// Optional commit-certified autonomous lane execution batch.
    ///
    /// `None` is encoded explicitly; layouts which omit this field fail closed.
    pub execution_batch: Option<MergeExecutionBatch>,
    /// Lane-committee drain certificates globally ordered by this entry.
    ///
    /// This trailing field lets a quiet network make retirement progress
    /// without inventing an executable lane payload. Runtime admission limits
    /// an entry to the single highest autoscale retirement candidate.
    pub lane_drain_certificates: Vec<LaneDrainCertificateV1>,
    /// Canonical framed queue-plan admission certificates in strict source order.
    ///
    /// The concrete certificate type belongs to `iroha_core`, so the data
    /// model retains exact canonical bytes without introducing a dependency
    /// cycle. Runtime admission decodes, authenticates, and stages the
    /// certificate bindings through an immutable WSV compare-and-set.
    pub queue_plan_admissions: Vec<Vec<u8>>,
}

impl MergeLedgerEntry {
    /// Current supported entry layout.
    pub const VERSION: u8 = MERGE_LEDGER_ENTRY_VERSION_V2;

    /// Return whether this entry advertises the current first-release layout.
    #[must_use]
    pub const fn has_current_version(&self) -> bool {
        self.version == Self::VERSION
    }

    /// Return the canonical framed Norito bytes used by hash-addressed entry sidecars.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        norito::to_bytes(self).expect("merge-ledger entry must have a canonical Norito encoding")
    }

    /// Return the single canonical, domain-separated hash used by compact block
    /// references, pending sidecars, committed carrier indexes, and receipts.
    #[must_use]
    pub fn canonical_hash(&self) -> HashOf<Self> {
        let bytes = self.canonical_bytes();
        HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[
            MERGE_LEDGER_ENTRY_HASH_DOMAIN,
            bytes.as_slice(),
        ]))
    }

    /// Return the exact canonical framed length committed by compact references.
    #[must_use]
    pub fn canonical_encoded_len(&self) -> u64 {
        u64::try_from(self.canonical_bytes().len()).unwrap_or(u64::MAX)
    }

    /// Return whether this entry fits the protocol-wide full-entry envelope.
    #[must_use]
    pub fn canonical_size_within_limit(&self) -> bool {
        merge_ledger_entry_size_within_limit(self.canonical_bytes().len())
    }

    /// Number of lanes represented by this entry.
    #[must_use]
    pub fn lane_count(&self) -> usize {
        self.lane_snapshots.len()
    }

    /// Canonical lane tips derived from [`Self::lane_snapshots`].
    #[must_use]
    pub fn lane_tips(&self) -> Vec<HashOf<BlockHeader>> {
        self.lane_snapshots
            .iter()
            .map(|snapshot| snapshot.tip_hash)
            .collect()
    }

    /// Canonical merge-hint roots derived from [`Self::lane_snapshots`].
    #[must_use]
    pub fn merge_hint_roots(&self) -> Vec<Hash> {
        self.lane_snapshots
            .iter()
            .map(|snapshot| snapshot.merge_hint_root)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;

    #[derive(Encode)]
    struct UnversionedMergeLedgerEntry {
        epoch_id: u64,
        lane_catalog_hash: Hash,
        active_lanes: Vec<MergeLaneBinding>,
        incarnation_root: Hash,
        activation_root: Hash,
        lane_snapshots: Vec<MergeLaneSnapshot>,
        global_state_root: Hash,
        merge_qc: MergeQuorumCertificate,
    }

    #[derive(Encode)]
    struct LegacyMergeLedgerEntry {
        version: u8,
        epoch_id: u64,
        lane_catalog_hash: Hash,
        active_lanes: Vec<MergeLaneBinding>,
        incarnation_root: Hash,
        activation_root: Hash,
        lane_snapshots: Vec<MergeLaneSnapshot>,
        global_state_root: Hash,
        merge_qc: MergeQuorumCertificate,
    }

    #[derive(Encode)]
    struct PreDrainMergeLedgerEntry {
        version: u8,
        epoch_id: u64,
        lane_catalog_hash: Hash,
        active_lanes: Vec<MergeLaneBinding>,
        incarnation_root: Hash,
        activation_root: Hash,
        lane_snapshots: Vec<MergeLaneSnapshot>,
        global_state_root: Hash,
        merge_qc: MergeQuorumCertificate,
        execution_batch: Option<MergeExecutionBatch>,
    }

    #[derive(Encode)]
    struct PreviousMergeLedgerEntryV1 {
        version: u8,
        epoch_id: u64,
        lane_catalog_hash: Hash,
        active_lanes: Vec<MergeLaneBinding>,
        incarnation_root: Hash,
        activation_root: Hash,
        lane_snapshots: Vec<MergeLaneSnapshot>,
        global_state_root: Hash,
        merge_qc: MergeQuorumCertificate,
        execution_batch: Option<MergeExecutionBatch>,
        lane_drain_certificates: Vec<LaneDrainCertificateV1>,
    }

    fn sample_tip(label: &[u8]) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::new(label))
    }

    fn sample_hash(label: &[u8]) -> Hash {
        Hash::new(label)
    }

    fn sample_lane_drain_intent() -> LaneDrainIntentV1 {
        let validator_set = vec![PeerId::new(
            KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                .expect("BLS drain fixture keypair")
                .public_key()
                .clone(),
        )];
        LaneDrainIntentV1 {
            version: 1,
            chain_id_digest: sample_hash(b"drain-chain"),
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: sample_hash(b"drain-incarnation"),
            close_global_height: 42,
            initial_frontier: LaneDrainFrontierV1::ordinary(
                LaneId::new(7),
                DataSpaceId::new(11),
                sample_hash(b"drain-incarnation"),
                9,
                Some(sample_hash(b"initial-drain-tip")),
            ),
            validator_set_hash_version: 1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set,
            validator_count: 1,
            min_quorum: 1,
        }
    }

    fn sample_lane_drain_certificate() -> LaneDrainCertificateV1 {
        let intent = sample_lane_drain_intent();
        let validator_set = intent.validator_set.clone();
        LaneDrainCertificateV1 {
            body: LaneDrainCertificateBodyV1 {
                version: 1,
                intent,
                final_frontier: LaneDrainFrontierV1::ordinary(
                    LaneId::new(7),
                    DataSpaceId::new(11),
                    sample_hash(b"drain-incarnation"),
                    12,
                    Some(sample_hash(b"final-drain-tip")),
                ),
            },
            validator_set,
            signers_bitmap: Vec::new(),
            signer_proofs: Vec::new(),
            aggregate_signature: vec![0xA5; 96],
        }
    }

    fn sample_lane_drain_commitment() -> LaneDrainCommitmentV1 {
        LaneDrainCommitmentV1 {
            version: 1,
            certificate_hash: sample_lane_drain_certificate().canonical_hash(),
            merge_entry_hash: HashOf::from_untyped_unchecked(sample_hash(
                b"drain-carrier-merge-entry",
            )),
            carrier_height: 57,
            frontier: LaneDrainFrontierV1::ordinary(
                LaneId::new(7),
                DataSpaceId::new(11),
                sample_hash(b"drain-incarnation"),
                12,
                Some(sample_hash(b"final-drain-tip")),
            ),
        }
    }

    fn sample_execution_batch() -> MergeExecutionBatch {
        let base_state_hash =
            HashOf::from_untyped_unchecked(sample_hash(b"legacy-batch-base-state"));
        MergeExecutionBatch {
            version: 1,
            base_state_height: 1,
            base_state_hash,
            application_block_header: BlockHeader::new(
                NonZeroU64::new(2).expect("non-zero block height"),
                Some(base_state_hash),
                None,
                None,
                7,
                0,
            ),
            lanes: Vec::new(),
            entrypoint_count: 0,
            entrypoint_merkle_root: HashOf::from_untyped_unchecked(sample_hash(
                b"legacy-batch-entrypoint-root",
            )),
            result_merkle_root: HashOf::from_untyped_unchecked(sample_hash(
                b"legacy-batch-result-root",
            )),
            execution_root: sample_hash(b"legacy-batch-execution-root"),
            application_write_set_root: sample_hash(b"legacy-batch-application-write-set"),
            write_set_root: sample_hash(b"legacy-batch-write-set"),
            expected_post_state_hash: HashOf::from_untyped_unchecked(sample_hash(
                b"legacy-batch-post-state",
            )),
            batch_hash: sample_hash(b"legacy-batch-hash"),
        }
    }

    fn assert_commitment_canonical_wire_changes(
        baseline: &LaneDrainCommitmentV1,
        changed: &LaneDrainCommitmentV1,
        field: &str,
    ) {
        let baseline_bytes =
            norito::to_bytes(baseline).expect("drain commitment has canonical bytes");
        let changed_bytes =
            norito::to_bytes(changed).expect("changed drain commitment has canonical bytes");
        assert_ne!(
            baseline_bytes, changed_bytes,
            "{field} must bind canonical bytes"
        );
        assert_ne!(
            Hash::new(baseline_bytes.as_slice()),
            Hash::new(changed_bytes.as_slice()),
            "{field} must bind the hash-relevant canonical representation"
        );
    }

    fn assert_state_canonical_wire_changes(
        baseline: &LaneDrainStateV1,
        changed: &LaneDrainStateV1,
        field: &str,
    ) {
        let baseline_bytes = norito::to_bytes(baseline).expect("drain state has canonical bytes");
        let changed_bytes =
            norito::to_bytes(changed).expect("changed drain state has canonical bytes");
        assert_ne!(
            baseline_bytes, changed_bytes,
            "{field} must bind canonical bytes"
        );
        assert_ne!(
            Hash::new(baseline_bytes.as_slice()),
            Hash::new(changed_bytes.as_slice()),
            "{field} must bind the hash-relevant canonical representation"
        );
    }

    fn assert_canonical_decoder_rejects_invalid_wire<T>(encoded: &[u8], fixture: &str)
    where
        for<'de> T: norito::NoritoDeserialize<'de>,
    {
        for prefix_len in 0..encoded.len() {
            assert!(
                norito::decode_from_bytes::<T>(&encoded[..prefix_len]).is_err(),
                "{fixture} truncated at byte {prefix_len} must be rejected"
            );
        }

        let mut trailing = encoded.to_vec();
        trailing.push(0xA5);
        assert!(
            norito::decode_from_bytes::<T>(&trailing).is_err(),
            "{fixture} with trailing data must be rejected"
        );

        let mut malformed_header = encoded.to_vec();
        malformed_header[0] ^= 0xFF;
        assert!(
            norito::decode_from_bytes::<T>(&malformed_header).is_err(),
            "{fixture} with malformed Norito magic must be rejected"
        );

        let mut malformed_payload = encoded.to_vec();
        let payload_byte = malformed_payload
            .last_mut()
            .expect("canonical encoding is non-empty");
        *payload_byte ^= 0xFF;
        assert!(
            norito::decode_from_bytes::<T>(&malformed_payload).is_err(),
            "{fixture} with a checksum-invalid payload must be rejected"
        );
    }

    fn sample_settlement(
        lane_id: LaneId,
        lane_incarnation: Hash,
        dataspace_id: DataSpaceId,
        block_height: u64,
    ) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height,
            lane_id,
            lane_incarnation,
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
        }
    }

    #[test]
    fn merge_protocol_size_limits_are_inclusive_and_reserve_entry_headroom() {
        assert!(merge_execution_batch_size_within_limit(
            MAX_MERGE_EXECUTION_BATCH_BYTES
        ));
        assert!(!merge_execution_batch_size_within_limit(
            MAX_MERGE_EXECUTION_BATCH_BYTES + 1
        ));
        assert!(merge_ledger_entry_size_within_limit(
            MAX_MERGE_LEDGER_ENTRY_BYTES
        ));
        assert!(!merge_ledger_entry_size_within_limit(
            MAX_MERGE_LEDGER_ENTRY_BYTES + 1
        ));
        assert_eq!(
            MAX_MERGE_LEDGER_ENTRY_BYTES - MAX_MERGE_EXECUTION_BATCH_BYTES,
            4 * 1024 * 1024
        );
        assert!(merge_queue_plan_admissions_within_limits(&[vec![
            0xA5;
            MAX_MERGE_QUEUE_PLAN_ADMISSION_BYTES
        ]]));
        assert!(!merge_queue_plan_admissions_within_limits(&[Vec::new()]));
        assert!(!merge_queue_plan_admissions_within_limits(&[vec![
            0xA5;
            MAX_MERGE_QUEUE_PLAN_ADMISSION_BYTES
                + 1
        ]]));
        assert!(!merge_queue_plan_admissions_within_limits(&vec![
            vec![0xA5];
            MAX_MERGE_QUEUE_PLAN_ADMISSIONS
                + 1
        ]));
        assert!(!merge_queue_plan_admissions_within_limits(&vec![
                vec![0xA5; MAX_MERGE_QUEUE_PLAN_ADMISSION_BYTES];
                MAX_MERGE_QUEUE_PLAN_ADMISSIONS_BYTES
                    / MAX_MERGE_QUEUE_PLAN_ADMISSION_BYTES
                    + 1
            ]));
    }

    #[test]
    fn reserved_entry_headroom_fits_maximum_execution_committee_and_lane_bindings() {
        const MAX_ACTIVE_LANES: usize = 1_024;
        const MAX_MERGE_VALIDATORS: usize = 4_096;
        const BLS_PROOF_BYTES: usize = 96;

        let keypair =
            KeyPair::try_random_with_algorithm(Algorithm::BlsNormal).expect("BLS fixture keypair");
        let peer = PeerId::new(keypair.public_key().clone());
        let validator_set = vec![peer; MAX_MERGE_VALIDATORS];
        let validator_set_hash = HashOf::new(&validator_set);
        let signer_proofs = (0..MAX_MERGE_VALIDATORS)
            .map(|index| MergeSignerProof {
                signer: u32::try_from(index).expect("validator index fits"),
                proof_of_possession: vec![0; BLS_PROOF_BYTES],
            })
            .collect();
        let active_lanes = (0..MAX_ACTIVE_LANES)
            .map(|index| {
                let index_u64 = u64::try_from(index).expect("lane index fits u64");
                MergeLaneBinding {
                    lane_id: LaneId::new(u32::try_from(index).expect("lane index fits")),
                    dataspace_id: DataSpaceId::new(index_u64),
                    lane_config_hash: sample_hash(&index_u64.to_le_bytes()),
                    incarnation: sample_hash(&index_u64.saturating_add(1).to_le_bytes()),
                    activation_height: 1,
                }
            })
            .collect();
        let entry = MergeLedgerEntry {
            version: MergeLedgerEntry::VERSION,
            epoch_id: 1,
            lane_catalog_hash: sample_hash(b"max-overhead-catalog"),
            active_lanes,
            incarnation_root: sample_hash(b"max-overhead-incarnations"),
            activation_root: sample_hash(b"max-overhead-activations"),
            lane_snapshots: Vec::new(),
            global_state_root: sample_hash(b"max-overhead-global-root"),
            merge_qc: MergeQuorumCertificate::new(
                0,
                1,
                1,
                HashOf::from_untyped_unchecked(sample_hash(b"max-overhead-parent")),
                sample_hash(b"max-overhead-chain"),
                1,
                validator_set_hash,
                validator_set,
                vec![0xff; MAX_MERGE_VALIDATORS.div_ceil(8)],
                signer_proofs,
                vec![0; BLS_PROOF_BYTES],
                sample_hash(b"max-overhead-message"),
            ),
            execution_batch: None,
            lane_drain_certificates: Vec::new(),
            queue_plan_admissions: Vec::new(),
        };
        let envelope_overhead = entry.canonical_bytes().len();
        assert!(
            envelope_overhead <= MAX_MERGE_LEDGER_ENTRY_BYTES - MAX_MERGE_EXECUTION_BATCH_BYTES,
            "maximum committee/lane envelope {envelope_overhead} exceeds reserved headroom"
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one linear clean-break scenario compares current and legacy wire layouts"
    )]
    fn merge_entry_roundtrip() {
        let qc = MergeQuorumCertificate::new(
            7,
            3,
            4,
            HashOf::from_untyped_unchecked(sample_hash(b"carrier-parent")),
            sample_hash(b"chain"),
            1,
            HashOf::new(&Vec::<PeerId>::new()),
            Vec::new(),
            vec![0b1010_1010],
            Vec::new(),
            vec![0xAA, 0xBB, 0xCC],
            sample_hash(b"qc-digest"),
        );
        let entry = MergeLedgerEntry {
            version: MergeLedgerEntry::VERSION,
            epoch_id: 3,
            lane_catalog_hash: sample_hash(b"catalog"),
            active_lanes: vec![
                MergeLaneBinding {
                    lane_id: LaneId::new(1),
                    dataspace_id: DataSpaceId::new(7),
                    lane_config_hash: sample_hash(b"config-1"),
                    incarnation: sample_hash(b"incarnation-1"),
                    activation_height: 1,
                },
                MergeLaneBinding {
                    lane_id: LaneId::new(2),
                    dataspace_id: DataSpaceId::new(9),
                    lane_config_hash: sample_hash(b"config-2"),
                    incarnation: sample_hash(b"incarnation-2"),
                    activation_height: 1,
                },
            ],
            incarnation_root: sample_hash(b"incarnation-root"),
            activation_root: sample_hash(b"activation-root"),
            lane_snapshots: vec![
                MergeLaneSnapshot {
                    lane_id: LaneId::new(1),
                    lane_incarnation: sample_hash(b"incarnation-1"),
                    incarnation_activation_height: 1,
                    proposal_height: 2,
                    dataspace_id: DataSpaceId::new(7),
                    lane_block_height: 11,
                    tip_hash: sample_tip(b"lane-0"),
                    merge_hint_root: sample_hash(b"root-0"),
                    settlement_commitment: sample_settlement(
                        LaneId::new(1),
                        sample_hash(b"incarnation-1"),
                        DataSpaceId::new(7),
                        11,
                    ),
                    settlement_hash: crate::nexus::compute_settlement_hash(&sample_settlement(
                        LaneId::new(1),
                        sample_hash(b"incarnation-1"),
                        DataSpaceId::new(7),
                        11,
                    ))
                    .expect("sample settlement should hash canonically"),
                    relay_envelope: None,
                },
                MergeLaneSnapshot {
                    lane_id: LaneId::new(2),
                    lane_incarnation: sample_hash(b"incarnation-2"),
                    incarnation_activation_height: 1,
                    proposal_height: 2,
                    dataspace_id: DataSpaceId::new(9),
                    lane_block_height: 14,
                    tip_hash: sample_tip(b"lane-1"),
                    merge_hint_root: sample_hash(b"root-1"),
                    settlement_commitment: sample_settlement(
                        LaneId::new(2),
                        sample_hash(b"incarnation-2"),
                        DataSpaceId::new(9),
                        14,
                    ),
                    settlement_hash: crate::nexus::compute_settlement_hash(&sample_settlement(
                        LaneId::new(2),
                        sample_hash(b"incarnation-2"),
                        DataSpaceId::new(9),
                        14,
                    ))
                    .expect("sample settlement should hash canonically"),
                    relay_envelope: None,
                },
            ],
            execution_batch: None,
            lane_drain_certificates: vec![sample_lane_drain_certificate()],
            queue_plan_admissions: vec![
                norito::to_bytes(&sample_hash(b"queue-plan-admission"))
                    .expect("opaque queue-plan admission fixture encodes"),
            ],
            global_state_root: sample_hash(b"global"),
            merge_qc: qc.clone(),
        };

        assert_eq!(entry.lane_count(), 2);
        assert_eq!(entry.lane_tips().len(), 2);
        assert_eq!(entry.merge_hint_roots().len(), 2);

        let encoded = Encode::encode(&entry);
        let decoded = MergeLedgerEntry::decode(&mut &encoded[..])
            .expect("merge entry rounds trips through Norito");
        assert_eq!(decoded, entry);

        let unversioned = UnversionedMergeLedgerEntry {
            epoch_id: entry.epoch_id,
            lane_catalog_hash: entry.lane_catalog_hash,
            active_lanes: entry.active_lanes.clone(),
            incarnation_root: entry.incarnation_root,
            activation_root: entry.activation_root,
            lane_snapshots: entry.lane_snapshots.clone(),
            global_state_root: entry.global_state_root,
            merge_qc: entry.merge_qc.clone(),
        };
        let unversioned_encoded = unversioned.encode();
        assert!(
            MergeLedgerEntry::decode(&mut unversioned_encoded.as_slice()).is_err(),
            "the unversioned merge-entry layout must fail closed"
        );

        let legacy = LegacyMergeLedgerEntry {
            version: MergeLedgerEntry::VERSION,
            epoch_id: entry.epoch_id,
            lane_catalog_hash: entry.lane_catalog_hash,
            active_lanes: entry.active_lanes.clone(),
            incarnation_root: entry.incarnation_root,
            activation_root: entry.activation_root,
            lane_snapshots: entry.lane_snapshots.clone(),
            global_state_root: entry.global_state_root,
            merge_qc: entry.merge_qc.clone(),
        };
        let legacy_encoded = legacy.encode();
        assert!(
            MergeLedgerEntry::decode(&mut legacy_encoded.as_slice()).is_err(),
            "the layout omitting execution and drain fields must fail closed"
        );

        let previous_batch = sample_execution_batch();
        let previous = PreDrainMergeLedgerEntry {
            version: MergeLedgerEntry::VERSION,
            epoch_id: entry.epoch_id,
            lane_catalog_hash: entry.lane_catalog_hash,
            active_lanes: entry.active_lanes.clone(),
            incarnation_root: entry.incarnation_root,
            activation_root: entry.activation_root,
            lane_snapshots: entry.lane_snapshots.clone(),
            global_state_root: entry.global_state_root,
            merge_qc: entry.merge_qc.clone(),
            execution_batch: Some(previous_batch.clone()),
        };
        let previous_encoded = previous.encode();
        assert!(
            MergeLedgerEntry::decode(&mut previous_encoded.as_slice()).is_err(),
            "the layout omitting drain certificates must fail closed"
        );

        let previous_v1 = PreviousMergeLedgerEntryV1 {
            version: 1,
            epoch_id: entry.epoch_id,
            lane_catalog_hash: entry.lane_catalog_hash,
            active_lanes: entry.active_lanes.clone(),
            incarnation_root: entry.incarnation_root,
            activation_root: entry.activation_root,
            lane_snapshots: entry.lane_snapshots.clone(),
            global_state_root: entry.global_state_root,
            merge_qc: entry.merge_qc.clone(),
            execution_batch: Some(previous_batch),
            lane_drain_certificates: entry.lane_drain_certificates.clone(),
        };
        let previous_v1_encoded = previous_v1.encode();
        assert!(
            MergeLedgerEntry::decode(&mut previous_v1_encoded.as_slice()).is_err(),
            "the complete version-one layout must fail closed"
        );

        let mut unsupported = entry;
        unsupported.version = MergeLedgerEntry::VERSION.saturating_add(1);
        assert!(!unsupported.has_current_version());
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one adversarial legacy-layout matrix proves every optional multilane field requires an explicit Option discriminant"
    )]
    fn multilane_optional_fields_require_explicit_option_discriminants() {
        #[derive(Encode)]
        struct LegacyLaneDrainIntentV1 {
            version: u8,
            chain_id_digest: Hash,
            lane_id: LaneId,
            dataspace_id: DataSpaceId,
            lane_incarnation: Hash,
            close_global_height: u64,
            initial_merged_lane_height: u64,
            initial_merged_descriptor_hash: Option<Hash>,
            validator_set_hash_version: u16,
            validator_set_hash: HashOf<Vec<PeerId>>,
            validator_set: Vec<PeerId>,
            validator_count: u32,
            min_quorum: u32,
        }

        #[derive(Encode)]
        struct LegacyLaneDrainCertificateBodyV1 {
            version: u8,
            intent: LaneDrainIntentV1,
            final_lane_block_height: u64,
            final_lane_block_descriptor_hash: Option<Hash>,
        }

        #[derive(Encode)]
        struct LegacyLaneDrainCommitmentV1 {
            certificate_hash: HashOf<LaneDrainCertificateV1>,
            merge_entry_hash: HashOf<MergeLedgerEntry>,
            carrier_height: u64,
            final_lane_block_height: u64,
            final_lane_block_descriptor_hash: Option<Hash>,
        }

        #[derive(Encode)]
        struct LegacyLaneDrainStateV1 {
            version: u8,
            intent: LaneDrainIntentV1,
        }

        #[derive(Encode)]
        struct LegacyMergeLaneSnapshot {
            lane_id: LaneId,
            lane_incarnation: Hash,
            incarnation_activation_height: u64,
            proposal_height: u64,
            dataspace_id: DataSpaceId,
            lane_block_height: u64,
            tip_hash: HashOf<BlockHeader>,
            merge_hint_root: Hash,
            settlement_commitment: LaneBlockCommitment,
            settlement_hash: HashOf<LaneBlockCommitment>,
        }

        let intent = sample_lane_drain_intent();
        let legacy_intent = LegacyLaneDrainIntentV1 {
            version: intent.version,
            chain_id_digest: intent.chain_id_digest,
            lane_id: intent.lane_id,
            dataspace_id: intent.dataspace_id,
            lane_incarnation: intent.lane_incarnation,
            close_global_height: intent.close_global_height,
            initial_merged_lane_height: intent.initial_frontier.lane_block_height,
            initial_merged_descriptor_hash: intent.initial_frontier.lane_block_descriptor_hash,
            validator_set_hash_version: intent.validator_set_hash_version,
            validator_set_hash: intent.validator_set_hash,
            validator_set: intent.validator_set.clone(),
            validator_count: intent.validator_count,
            min_quorum: intent.min_quorum,
        }
        .encode();
        assert!(LaneDrainIntentV1::decode(&mut legacy_intent.as_slice()).is_err());

        let legacy_body = LegacyLaneDrainCertificateBodyV1 {
            version: 1,
            intent: intent.clone(),
            final_lane_block_height: 9,
            final_lane_block_descriptor_hash: Some(sample_hash(b"legacy-final-frontier")),
        }
        .encode();
        assert!(LaneDrainCertificateBodyV1::decode(&mut legacy_body.as_slice()).is_err());

        let legacy_commitment = LegacyLaneDrainCommitmentV1 {
            certificate_hash: sample_lane_drain_certificate().canonical_hash(),
            merge_entry_hash: HashOf::from_untyped_unchecked(sample_hash(b"legacy-merge-entry")),
            carrier_height: 12,
            final_lane_block_height: 9,
            final_lane_block_descriptor_hash: Some(sample_hash(b"legacy-final-frontier")),
        }
        .encode();
        assert!(LaneDrainCommitmentV1::decode(&mut legacy_commitment.as_slice()).is_err());

        let legacy_state = LegacyLaneDrainStateV1 { version: 1, intent }.encode();
        assert!(LaneDrainStateV1::decode(&mut legacy_state.as_slice()).is_err());

        let settlement = sample_settlement(
            LaneId::new(3),
            sample_hash(b"legacy-snapshot-incarnation"),
            DataSpaceId::new(5),
            7,
        );
        let legacy_snapshot = LegacyMergeLaneSnapshot {
            lane_id: LaneId::new(3),
            lane_incarnation: sample_hash(b"legacy-snapshot-incarnation"),
            incarnation_activation_height: 1,
            proposal_height: 8,
            dataspace_id: DataSpaceId::new(5),
            lane_block_height: 7,
            tip_hash: sample_tip(b"legacy-snapshot-tip"),
            merge_hint_root: sample_hash(b"legacy-snapshot-root"),
            settlement_hash: crate::nexus::compute_settlement_hash(&settlement)
                .expect("legacy snapshot settlement hashes"),
            settlement_commitment: settlement,
        }
        .encode();
        assert!(MergeLaneSnapshot::decode(&mut legacy_snapshot.as_slice()).is_err());
    }

    #[test]
    fn lane_drain_intent_and_certificate_hash_every_consensus_field() {
        let intent = sample_lane_drain_intent();
        let intent_hash = intent.canonical_hash();
        let encoded = intent.encode();
        let decoded = LaneDrainIntentV1::decode(&mut encoded.as_slice())
            .expect("lane drain intent round-trips");
        assert_eq!(decoded, intent);

        macro_rules! assert_intent_field_bound {
            ($mutation:expr, $field:literal) => {{
                let mut changed = intent.clone();
                ($mutation)(&mut changed);
                assert_ne!(
                    changed.canonical_hash(),
                    intent_hash,
                    concat!("intent hash must bind ", $field)
                );
            }};
        }
        assert_intent_field_bound!(
            |changed: &mut LaneDrainIntentV1| changed.version += 1,
            "version"
        );
        assert_intent_field_bound!(
            |changed: &mut LaneDrainIntentV1| changed.chain_id_digest = sample_hash(b"other-chain"),
            "chain_id_digest"
        );
        assert_intent_field_bound!(
            |changed: &mut LaneDrainIntentV1| changed.lane_id = LaneId::new(8),
            "lane_id"
        );
        assert_intent_field_bound!(
            |changed: &mut LaneDrainIntentV1| changed.dataspace_id = DataSpaceId::new(12),
            "dataspace_id"
        );
        assert_intent_field_bound!(
            |changed: &mut LaneDrainIntentV1| changed.lane_incarnation =
                sample_hash(b"other-incarnation"),
            "lane_incarnation"
        );
        let mut changed = intent.clone();
        changed.close_global_height = changed.close_global_height.saturating_add(1);
        assert_ne!(changed.canonical_hash(), intent_hash);
        assert_intent_field_bound!(
            |changed: &mut LaneDrainIntentV1| changed.initial_frontier.lane_block_height += 1,
            "initial_frontier.lane_block_height"
        );
        changed = intent.clone();
        changed.initial_frontier.lane_block_descriptor_hash =
            Some(sample_hash(b"different-initial-tip"));
        assert_ne!(changed.canonical_hash(), intent_hash);
        assert_intent_field_bound!(
            |changed: &mut LaneDrainIntentV1| changed.validator_set_hash_version += 1,
            "validator_set_hash_version"
        );
        changed = intent.clone();
        changed.validator_set_hash = HashOf::new(&Vec::<PeerId>::new());
        assert_ne!(changed.canonical_hash(), intent_hash);
        changed = intent.clone();
        changed.validator_set = vec![PeerId::new(
            KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                .expect("alternate BLS drain fixture keypair")
                .public_key()
                .clone(),
        )];
        assert_ne!(changed.canonical_hash(), intent_hash);
        assert_intent_field_bound!(
            |changed: &mut LaneDrainIntentV1| changed.validator_count += 1,
            "validator_count"
        );
        assert_intent_field_bound!(
            |changed: &mut LaneDrainIntentV1| changed.min_quorum += 1,
            "min_quorum"
        );

        let certificate = sample_lane_drain_certificate();
        let certificate_hash = certificate.canonical_hash();
        let signature_preimage = certificate.body.signature_preimage();
        let encoded = certificate.encode();
        let decoded = LaneDrainCertificateV1::decode(&mut encoded.as_slice())
            .expect("lane drain certificate round-trips");
        assert_eq!(decoded, certificate);

        let mut changed = certificate.clone();
        changed.body.final_frontier.lane_block_height = changed
            .body
            .final_frontier
            .lane_block_height
            .saturating_add(1);
        assert_ne!(changed.body.signature_preimage(), signature_preimage);
        assert_ne!(changed.canonical_hash(), certificate_hash);
        changed = certificate.clone();
        changed.body.intent.lane_incarnation = sample_hash(b"different-drain-incarnation");
        assert_ne!(changed.body.signature_preimage(), signature_preimage);
        assert_ne!(changed.canonical_hash(), certificate_hash);
        changed = certificate.clone();
        changed.aggregate_signature[0] ^= 0xFF;
        assert_ne!(changed.canonical_hash(), certificate_hash);
    }

    #[test]
    fn lane_drain_commitment_canonical_wire_roundtrip_and_field_binding() {
        let commitment = sample_lane_drain_commitment();
        assert_eq!(
            commitment.frontier,
            sample_lane_drain_certificate().body.final_frontier,
            "global commitment must carry the exact committee-signed frontier unchanged"
        );
        let canonical =
            norito::to_bytes(&commitment).expect("drain commitment has canonical bytes");
        let decoded: LaneDrainCommitmentV1 = norito::decode_from_bytes(&canonical)
            .expect("canonical drain commitment must round-trip");
        assert_eq!(decoded, commitment);
        assert_eq!(
            norito::to_bytes(&decoded).expect("decoded drain commitment re-encodes"),
            canonical,
            "decode/re-encode must preserve the exact canonical representation"
        );
        assert_canonical_decoder_rejects_invalid_wire::<LaneDrainCommitmentV1>(
            &canonical,
            "drain commitment",
        );

        let mut changed = commitment;
        changed.version = changed.version.saturating_add(1);
        assert_commitment_canonical_wire_changes(&commitment, &changed, "version");

        changed = commitment;
        changed.certificate_hash =
            HashOf::from_untyped_unchecked(sample_hash(b"different-drain-certificate"));
        assert_commitment_canonical_wire_changes(&commitment, &changed, "certificate_hash");

        changed = commitment;
        changed.merge_entry_hash =
            HashOf::from_untyped_unchecked(sample_hash(b"different-merge-entry"));
        assert_commitment_canonical_wire_changes(&commitment, &changed, "merge_entry_hash");

        changed = commitment;
        changed.carrier_height = changed.carrier_height.saturating_add(1);
        assert_commitment_canonical_wire_changes(&commitment, &changed, "carrier_height");

        changed = commitment;
        changed.frontier.lane_block_height = changed.frontier.lane_block_height.saturating_add(1);
        assert_commitment_canonical_wire_changes(
            &commitment,
            &changed,
            "frontier.lane_block_height",
        );

        changed = commitment;
        changed.frontier.lane_block_descriptor_hash =
            Some(sample_hash(b"different-final-drain-tip"));
        assert_commitment_canonical_wire_changes(
            &commitment,
            &changed,
            "frontier.lane_block_descriptor_hash value",
        );

        changed = commitment;
        changed.frontier.lane_block_descriptor_hash = None;
        assert_commitment_canonical_wire_changes(
            &commitment,
            &changed,
            "frontier.lane_block_descriptor_hash presence",
        );
    }

    #[test]
    fn lane_drain_state_canonical_wire_roundtrip_and_field_binding() {
        let intent_only = LaneDrainStateV1 {
            version: 1,
            intent: sample_lane_drain_intent(),
            commitment: None,
        };
        assert!(!intent_only.is_certified());
        let intent_only_bytes =
            norito::to_bytes(&intent_only).expect("intent-only drain state has canonical bytes");
        let decoded_intent_only: LaneDrainStateV1 = norito::decode_from_bytes(&intent_only_bytes)
            .expect("canonical intent-only drain state must round-trip");
        assert_eq!(decoded_intent_only, intent_only);
        assert_eq!(
            norito::to_bytes(&decoded_intent_only).expect("intent-only state re-encodes"),
            intent_only_bytes
        );

        let certified = LaneDrainStateV1 {
            commitment: Some(sample_lane_drain_commitment()),
            ..intent_only.clone()
        };
        assert!(certified.is_certified());
        let certified_bytes =
            norito::to_bytes(&certified).expect("certified drain state has canonical bytes");
        let decoded_certified: LaneDrainStateV1 = norito::decode_from_bytes(&certified_bytes)
            .expect("canonical certified drain state must round-trip");
        assert_eq!(decoded_certified, certified);
        assert_eq!(
            norito::to_bytes(&decoded_certified).expect("certified state re-encodes"),
            certified_bytes
        );
        assert_ne!(
            intent_only_bytes, certified_bytes,
            "commitment presence must bind the canonical state representation"
        );

        let mut changed = certified.clone();
        changed.version = changed.version.saturating_add(1);
        assert_state_canonical_wire_changes(&certified, &changed, "version");

        changed = certified.clone();
        changed.intent.chain_id_digest = sample_hash(b"different-state-intent");
        assert_state_canonical_wire_changes(&certified, &changed, "intent");

        changed = certified.clone();
        changed.commitment = None;
        assert_state_canonical_wire_changes(&certified, &changed, "commitment presence");

        changed = certified.clone();
        changed
            .commitment
            .as_mut()
            .expect("certified state has a commitment")
            .carrier_height += 1;
        assert_state_canonical_wire_changes(&certified, &changed, "commitment value");
    }

    #[test]
    fn lane_drain_state_canonical_decoder_rejects_invalid_wire() {
        let intent = sample_lane_drain_intent();
        let intent_only = norito::to_bytes(&LaneDrainStateV1 {
            version: 1,
            intent: intent.clone(),
            commitment: None,
        })
        .expect("intent-only drain state has canonical bytes");
        assert_canonical_decoder_rejects_invalid_wire::<LaneDrainStateV1>(
            &intent_only,
            "intent-only state",
        );
        let certified = norito::to_bytes(&LaneDrainStateV1 {
            version: 1,
            intent,
            commitment: Some(sample_lane_drain_commitment()),
        })
        .expect("certified drain state has canonical bytes");
        assert_canonical_decoder_rejects_invalid_wire::<LaneDrainStateV1>(
            &certified,
            "certified state",
        );
    }

    #[test]
    fn quorum_certificate_roundtrip() {
        let qc = MergeQuorumCertificate::new(
            11,
            5,
            6,
            HashOf::from_untyped_unchecked(sample_hash(b"carrier-parent")),
            sample_hash(b"chain"),
            1,
            HashOf::new(&Vec::<PeerId>::new()),
            Vec::new(),
            vec![0xFF, 0x00],
            Vec::new(),
            vec![0xDE, 0xAD, 0xBE, 0xEF],
            sample_hash(b"digest"),
        );
        let encoded = Encode::encode(&qc);
        let decoded = MergeQuorumCertificate::decode(&mut &encoded[..])
            .expect("quorum certificate round-trips");
        assert_eq!(decoded, qc);
    }

    #[test]
    fn merge_committee_signature_roundtrip() {
        let signature = MergeCommitteeSignature {
            version: MERGE_COMMITTEE_SIGNATURE_VERSION_V2,
            epoch_id: 9,
            view: 1,
            signer: 2,
            message_digest: sample_hash(b"merge-digest"),
            bls_sig: vec![0x10, 0x20, 0x30],
            leader_candidate_body: Some(vec![0x40, 0x50]),
        };
        let encoded = Encode::encode(&signature);
        let decoded = MergeCommitteeSignature::decode(&mut &encoded[..])
            .expect("merge signature round-trips");
        assert_eq!(decoded, signature);
    }
}
