//! Untrusted, independently rebuildable SCCP sparse-replay archive.
//!
//! Consensus retains only the constant-size forest projection. Archive replicas
//! retain sorted leaves plus authenticated-node caches, update exactly one
//! 248-level path per delta, independently rebuild snapshots, and serve
//! canonical compressed witnesses. Replica signatures establish availability
//! provenance, never a substitute safety boundary: every response remains
//! locally verifiable.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::OnceLock,
};

use iroha_data_model::NetworkId;
use iroha_data_model::bridge::{
    SCCP_REPLAY_SMT_DEPTH_V1, SCCP_REPLAY_SMT_SHARD_COUNT_V1, SccpReplayAccumulatorError,
    SccpReplayAccumulatorIdV1, SccpReplayDomainV1, SccpReplayForestV1, SccpReplayRecordV1,
    SccpSparseMerkleWitnessV1, sccp_replay_domain_hash_v1, sccp_replay_empty_hashes_v1,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

const SNAPSHOT_VERSION_V1: u8 = 1;
const CHECKPOINT_VERSION_V1: u8 = 1;
const CHECKPOINT_SET_VERSION_V1: u8 = 1;
const REPLAY_MAGIC_V1: &[u8; 18] = b"SCCP-REPLAY-SMT-V1";
const CHECKPOINT_SIGNATURE_DOMAIN_V1: &[u8] = b"SCCP-REPLAY-CHECKPOINT-SIGNATURE-V1";
const REPLICA_AGREEMENT_DOMAIN_V1: &[u8] = b"SCCP-REPLAY-REPLICA-AGREEMENT-V1";
const CHECKPOINT_SET_SIGNATURE_DOMAIN_V1: &[u8] = b"SCCP-REPLAY-CHECKPOINT-SET-SIGNATURE-V1";
const CHECKPOINT_SET_AGREEMENT_DOMAIN_V1: &[u8] = b"SCCP-REPLAY-CHECKPOINT-SET-AGREEMENT-V1";
const CHECKPOINT_SET_INVENTORY_DOMAIN_V1: &[u8] = b"SCCP-REPLAY-CHECKPOINT-SET-INVENTORY-V1";
/// SHA-256 domain prepended to the exact canonical complete checkpoint-set
/// frame fetched byte-for-byte from all three archive replicas.
pub const SCCP_REPLAY_ARCHIVE_CHECKPOINT_SET_FRAME_SHA256_DOMAIN_V1: &[u8] =
    b"SCCP-REPLAY-CHECKPOINT-SET-V1";

fn replay_empty_hashes() -> &'static [[u8; 32]; SCCP_REPLAY_SMT_DEPTH_V1 + 1] {
    static EMPTY_HASHES: OnceLock<[[u8; 32]; SCCP_REPLAY_SMT_DEPTH_V1 + 1]> = OnceLock::new();
    EMPTY_HASHES.get_or_init(sccp_replay_empty_hashes_v1)
}

/// Exact byte length of a canonical first-release SoraFS manifest-root CID.
pub const SCCP_REPLAY_SORAFS_MANIFEST_ROOT_CID_BYTES_V1: usize = 36;

/// Default maximum number of leaves admitted by one decoded snapshot.
pub const SCCP_REPLAY_ARCHIVE_DEFAULT_MAX_SNAPSHOT_LEAVES_V1: usize = 8 * 1024 * 1024;
/// Default maximum encoded snapshot size. Deployments may declare a different
/// finite streaming limit before accepting an artifact.
pub const SCCP_REPLAY_ARCHIVE_DEFAULT_MAX_SNAPSHOT_BYTES_V1: usize = 1024 * 1024 * 1024;

/// Explicit finite limits for one untrusted replay-snapshot decode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SccpReplayArchiveDecodeLimitsV1 {
    /// Maximum complete canonical snapshot frame size.
    pub max_snapshot_bytes: usize,
    /// Maximum retained leaf count.
    pub max_snapshot_leaves: usize,
}

impl Default for SccpReplayArchiveDecodeLimitsV1 {
    fn default() -> Self {
        Self {
            max_snapshot_bytes: SCCP_REPLAY_ARCHIVE_DEFAULT_MAX_SNAPSHOT_BYTES_V1,
            max_snapshot_leaves: SCCP_REPLAY_ARCHIVE_DEFAULT_MAX_SNAPSHOT_LEAVES_V1,
        }
    }
}

/// Finalized chain coordinate bound into one immutable snapshot.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveFinalityV1")]
pub struct SccpReplayArchiveFinalityV1 {
    /// SHA-256 of the exact canonical network identity.
    pub network_identity_sha256: [u8; 32],
    /// Nonzero finalized carrier height.
    pub finalized_height: u64,
    /// Exact finalized carrier block hash.
    pub finalized_block_hash: [u8; 32],
}

impl SccpReplayArchiveFinalityV1 {
    fn is_well_formed(self) -> bool {
        self.network_identity_sha256 != [0; 32]
            && self.finalized_height != 0
            && self.finalized_block_hash != [0; 32]
    }
}

/// One sorted key/digest pair retained outside the consensus safety boundary.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveLeafV1")]
pub struct SccpReplayArchiveLeafV1 {
    /// Complete replay key; byte zero selects its shard. Every 256-bit value is
    /// valid, including zero.
    pub key: [u8; 32],
    /// Canonical occupied-record digest.
    pub record_digest: [u8; 32],
}

/// Deterministic, content-addressable replay snapshot suitable for SoraFS.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveSnapshotV1")]
pub struct SccpReplayArchiveSnapshotV1 {
    /// Snapshot schema version. Final V1 requires exactly one.
    pub version: u8,
    /// Route and boundary whose leaves are captured.
    pub accumulator_id: SccpReplayAccumulatorIdV1,
    /// Complete validated replay domain, not only a caller-supplied hash.
    pub domain: SccpReplayDomainV1,
    /// Exact finalized chain coordinate.
    pub finality: SccpReplayArchiveFinalityV1,
    /// Rebuilt constant-size forest projection.
    pub forest: SccpReplayForestV1,
    /// Strictly increasing complete leaf inventory.
    pub leaves: Vec<SccpReplayArchiveLeafV1>,
}

impl SccpReplayArchiveSnapshotV1 {
    /// SHA-256 content address of the canonical Norito snapshot bytes.
    pub fn content_sha256(&self) -> Result<[u8; 32], SccpReplayArchiveError> {
        let encoded =
            norito::encode_canonical(self).map_err(|_| SccpReplayArchiveError::Malformed)?;
        Ok(sha256(&[&encoded]))
    }
}

/// Immutable identity and Ed25519 verification key for one archive replica.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveReplicaBindingV1")]
pub struct SccpReplayArchiveReplicaBindingV1 {
    /// Stable nonzero replica identity assigned by release policy.
    pub replica_id: [u8; 32],
    /// Exact canonical Ed25519 public key.
    pub ed25519_public_key: [u8; 32],
}

/// Exact three-replica release policy used to authenticate checkpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveReplicaPolicyV1")]
pub struct SccpReplayArchiveReplicaPolicyV1 {
    /// Strictly replica-id-ordered, independently keyed bindings.
    pub replicas: [SccpReplayArchiveReplicaBindingV1; 3],
}

impl SccpReplayArchiveReplicaPolicyV1 {
    /// Validate exact cardinality, ordering, key uniqueness, and canonical
    /// Ed25519 encodings.
    pub fn validate(&self) -> Result<(), SccpReplayArchiveError> {
        let mut keys = BTreeSet::new();
        let mut previous = None;
        for binding in self.replicas {
            if binding.replica_id == [0; 32]
                || previous.is_some_and(|value| value >= binding.replica_id)
                || !keys.insert(binding.ed25519_public_key)
                || iroha_crypto::ed25519_parse_public_key(&binding.ed25519_public_key).is_err()
            {
                return Err(SccpReplayArchiveError::ReplicaPolicy);
            }
            previous = Some(binding.replica_id);
        }
        Ok(())
    }
}

/// Common checkpoint statement signed independently by all three replicas.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveCheckpointBodyV1")]
pub struct SccpReplayArchiveCheckpointBodyV1 {
    /// Checkpoint schema version. Final V1 requires exactly one.
    pub version: u8,
    /// Content address of the exact sorted snapshot.
    pub snapshot_sha256: [u8; 32],
    /// Exact route/boundary accumulator.
    pub accumulator_id: SccpReplayAccumulatorIdV1,
    /// Complete validated replay domain.
    pub domain: SccpReplayDomainV1,
    /// Finalized chain coordinate committed by the snapshot.
    pub finality: SccpReplayArchiveFinalityV1,
    /// Rebuilt forest, including checked count and update sequence.
    pub forest: SccpReplayForestV1,
}

impl SccpReplayArchiveCheckpointBodyV1 {
    /// Build a statement from a snapshot whose complete contents were already
    /// rebuilt locally.
    pub fn from_snapshot(
        snapshot: &SccpReplayArchiveSnapshotV1,
    ) -> Result<Self, SccpReplayArchiveError> {
        let validated = validate_snapshot(snapshot, SccpReplayArchiveDecodeLimitsV1::default())?;
        Ok(Self {
            version: CHECKPOINT_VERSION_V1,
            snapshot_sha256: validated.content_sha256,
            accumulator_id: snapshot.accumulator_id.clone(),
            domain: snapshot.domain,
            finality: snapshot.finality,
            forest: snapshot.forest.clone(),
        })
    }

    /// Domain-separated digest on which all three replicas must agree.
    pub fn agreement_digest(&self) -> Result<[u8; 32], SccpReplayArchiveError> {
        validate_checkpoint_body(self)?;
        let encoded =
            norito::encode_canonical(self).map_err(|_| SccpReplayArchiveError::Malformed)?;
        Ok(sha256(&[
            REPLICA_AGREEMENT_DOMAIN_V1,
            &u64::try_from(encoded.len())
                .map_err(|_| SccpReplayArchiveError::Malformed)?
                .to_be_bytes(),
            &encoded,
        ]))
    }

    fn signing_message(&self) -> Result<[u8; 32], SccpReplayArchiveError> {
        let agreement = self.agreement_digest()?;
        Ok(sha256(&[CHECKPOINT_SIGNATURE_DOMAIN_V1, &agreement]))
    }
}

/// Derive the exact network-identity commitment used by replay checkpoint
/// finality coordinates.
#[must_use]
pub fn sccp_replay_archive_network_identity_sha256_v1(network_id: &NetworkId) -> [u8; 32] {
    sha256(&[b"SCCP-REPLAY-NETWORK-IDENTITY-V1", network_id.as_bytes()])
}

/// Return the exact domain-separated message signed by every pinned archive
/// replica.
pub fn sccp_replay_archive_checkpoint_signing_message_v1(
    body: &SccpReplayArchiveCheckpointBodyV1,
) -> Result<[u8; 32], SccpReplayArchiveError> {
    body.signing_message()
}

/// Return the exact domain-separated message signed over a complete replay
/// checkpoint inventory, including an empty inventory.
pub fn sccp_replay_archive_checkpoint_set_signing_message_v1(
    body: &SccpReplayArchiveCheckpointSetBodyV1,
) -> Result<[u8; 32], SccpReplayArchiveError> {
    body.signing_message()
}

/// Compute the content identity of one canonical complete checkpoint-set
/// frame.
///
/// This deliberately accepts bytes rather than a replica-transport type so
/// independent clients can hash the exact frame before decoding it. Callers
/// must first enforce their canonical framing and finite byte limits; this
/// helper preserves the byte-exact final-V1 identity used by Torii.
#[must_use]
pub fn sccp_replay_archive_checkpoint_set_frame_sha256_v1(canonical_frame: &[u8]) -> [u8; 32] {
    sha256(&[
        SCCP_REPLAY_ARCHIVE_CHECKPOINT_SET_FRAME_SHA256_DOMAIN_V1,
        canonical_frame,
    ])
}

/// One replica's exact detached Ed25519 attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveReplicaAttestationV1")]
pub struct SccpReplayArchiveReplicaAttestationV1 {
    /// Replica identity selecting one pinned release-policy key.
    pub replica_id: [u8; 32],
    /// Detached Ed25519 signature over the domain-separated agreement digest.
    pub signature: [u8; 64],
}

/// Exactly three matching, independently signed replica checkpoints.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveSignedCheckpointV1")]
pub struct SccpReplayArchiveSignedCheckpointV1 {
    /// Common statement agreed by every replica.
    pub body: SccpReplayArchiveCheckpointBodyV1,
    /// Attestations in the exact same order as the pinned policy.
    pub attestations: [SccpReplayArchiveReplicaAttestationV1; 3],
}

/// Common finalized coordinate for one complete replay checkpoint set.
///
/// This coordinate contains every value that must be identical across the
/// complete inventory, including an empty one.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveHeadFinalityV1")]
pub struct SccpReplayArchiveHeadFinalityV1 {
    /// SHA-256 of the exact canonical Iroha network identity.
    pub network_identity_sha256: [u8; 32],
    /// Nonzero finalized carrier height.
    pub finalized_height: u64,
    /// Exact finalized carrier block hash.
    pub finalized_block_hash: [u8; 32],
}

impl SccpReplayArchiveHeadFinalityV1 {
    fn is_well_formed(self) -> bool {
        self.network_identity_sha256 != [0; 32]
            && self.finalized_height != 0
            && self.finalized_block_hash != [0; 32]
    }

    /// Return whether one per-accumulator checkpoint uses this exact common
    /// finalized coordinate.
    #[must_use]
    pub fn matches_checkpoint(self, finality: SccpReplayArchiveFinalityV1) -> bool {
        self.network_identity_sha256 == finality.network_identity_sha256
            && self.finalized_height == finality.finalized_height
            && self.finalized_block_hash == finality.finalized_block_hash
    }
}

impl From<SccpReplayArchiveFinalityV1> for SccpReplayArchiveHeadFinalityV1 {
    fn from(value: SccpReplayArchiveFinalityV1) -> Self {
        Self {
            network_identity_sha256: value.network_identity_sha256,
            finalized_height: value.finalized_height,
            finalized_block_hash: value.finalized_block_hash,
        }
    }
}

/// Authenticated SoraFS publication of the exact ordered snapshot package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveSorafsManifestV1")]
pub struct SccpReplayArchiveSorafsManifestV1 {
    /// SHA-256 of the exact canonical SoraFS manifest bytes.
    pub manifest_sha256: [u8; 32],
    /// Canonical CIDv1/dag-cbor/BLAKE3-256 root carried by that manifest.
    pub manifest_root_cid: [u8; SCCP_REPLAY_SORAFS_MANIFEST_ROOT_CID_BYTES_V1],
    /// Exact canonical manifest byte length.
    pub manifest_size_bytes: u64,
    /// Sum of exact canonical snapshot byte lengths in inventory order.
    pub snapshot_total_bytes: u64,
}

impl SccpReplayArchiveSorafsManifestV1 {
    fn is_well_formed(self) -> bool {
        self.manifest_sha256 != [0; 32]
            && self.manifest_size_bytes != 0
            && self.manifest_root_cid[..4] == [1, 0x71, 0x1f, 32]
            && self.manifest_root_cid[4..].iter().any(|byte| *byte != 0)
    }
}

/// One exact accumulator commitment inside the signed complete inventory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveCheckpointSetEntryV1")]
pub struct SccpReplayArchiveCheckpointSetEntryV1 {
    /// Exact route/boundary accumulator identity.
    pub accumulator_id: SccpReplayAccumulatorIdV1,
    /// Content address of its canonical sorted snapshot.
    pub snapshot_sha256: [u8; 32],
    /// Exact canonical snapshot byte length.
    pub snapshot_size_bytes: u64,
    /// Domain-separated digest authenticated by the per-accumulator signatures.
    pub checkpoint_agreement_digest: [u8; 32],
}

impl SccpReplayArchiveCheckpointSetEntryV1 {
    /// Derive the unique inventory entry for one validated checkpoint and its
    /// exact canonical snapshot size.
    pub fn from_checkpoint(
        checkpoint: &SccpReplayArchiveSignedCheckpointV1,
        snapshot_size_bytes: u64,
    ) -> Result<Self, SccpReplayArchiveError> {
        validate_checkpoint_body(&checkpoint.body)?;
        if snapshot_size_bytes == 0 {
            return Err(SccpReplayArchiveError::Malformed);
        }
        Ok(Self {
            accumulator_id: checkpoint.body.accumulator_id.clone(),
            snapshot_sha256: checkpoint.body.snapshot_sha256,
            snapshot_size_bytes,
            checkpoint_agreement_digest: checkpoint.body.agreement_digest()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveCheckpointInventoryV1")]
struct SccpReplayArchiveCheckpointInventoryV1 {
    version: u8,
    finality: SccpReplayArchiveHeadFinalityV1,
    entries: Vec<SccpReplayArchiveCheckpointSetEntryV1>,
}

/// Common statement authenticating the complete ordered accumulator inventory.
///
/// This statement is valid for zero entries. That is the only signed shape
/// used for a chain whose current SCCP registry is empty.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveCheckpointSetBodyV1")]
pub struct SccpReplayArchiveCheckpointSetBodyV1 {
    /// Checkpoint-set schema version. Final V1 accepts exactly one.
    pub version: u8,
    /// Common finalized coordinate for every listed checkpoint.
    pub finality: SccpReplayArchiveHeadFinalityV1,
    /// Checked cardinality of `entries`.
    pub entry_count: u32,
    /// Domain-separated digest of the exact ordered inventory and finality.
    pub inventory_sha256: [u8; 32],
    /// Strictly accumulator-id-ordered complete inventory.
    pub entries: Vec<SccpReplayArchiveCheckpointSetEntryV1>,
    /// Content-addressed SoraFS publication of the same snapshot package.
    pub sorafs_manifest: SccpReplayArchiveSorafsManifestV1,
}

impl SccpReplayArchiveCheckpointSetBodyV1 {
    /// Build one strict final-V1 complete-inventory statement.
    pub fn new(
        finality: SccpReplayArchiveHeadFinalityV1,
        entries: Vec<SccpReplayArchiveCheckpointSetEntryV1>,
        sorafs_manifest: SccpReplayArchiveSorafsManifestV1,
    ) -> Result<Self, SccpReplayArchiveError> {
        let entry_count =
            u32::try_from(entries.len()).map_err(|_| SccpReplayArchiveError::Malformed)?;
        let inventory_sha256 = checkpoint_set_inventory_sha256(finality, &entries)?;
        let body = Self {
            version: CHECKPOINT_SET_VERSION_V1,
            finality,
            entry_count,
            inventory_sha256,
            entries,
            sorafs_manifest,
        };
        validate_checkpoint_set_body(&body)?;
        Ok(body)
    }

    /// Domain-separated digest on which all three replicas must agree.
    pub fn agreement_digest(&self) -> Result<[u8; 32], SccpReplayArchiveError> {
        validate_checkpoint_set_body(self)?;
        let encoded =
            norito::encode_canonical(self).map_err(|_| SccpReplayArchiveError::Malformed)?;
        Ok(sha256(&[
            CHECKPOINT_SET_AGREEMENT_DOMAIN_V1,
            &u64::try_from(encoded.len())
                .map_err(|_| SccpReplayArchiveError::Malformed)?
                .to_be_bytes(),
            &encoded,
        ]))
    }

    fn signing_message(&self) -> Result<[u8; 32], SccpReplayArchiveError> {
        Ok(sha256(&[
            CHECKPOINT_SET_SIGNATURE_DOMAIN_V1,
            &self.agreement_digest()?,
        ]))
    }
}

/// Exactly three matching replica signatures over one complete inventory.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayArchiveSignedCheckpointSetV1")]
pub struct SccpReplayArchiveSignedCheckpointSetV1 {
    /// Complete inventory statement, including its SoraFS publication.
    pub body: SccpReplayArchiveCheckpointSetBodyV1,
    /// Attestations in the exact same order as the pinned replica policy.
    pub attestations: [SccpReplayArchiveReplicaAttestationV1; 3],
}

/// Canonical, independently verifiable replay-root response served by Torii.
///
/// The signed set binds the complete accumulator inventory and its SoraFS
/// publication. The selected checkpoint binds this accumulator's exact domain,
/// forest roots, leaf count, update sequence, and finalized chain coordinate.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayRootResponseV1")]
pub struct SccpReplayRootResponseV1 {
    /// Response schema version. Final V1 accepts exactly one.
    pub version: u8,
    /// SHA-256 identity of the exact complete three-replica checkpoint-set frame.
    pub checkpoint_set_sha256: [u8; 32],
    /// Exact signed complete-inventory statement containing this accumulator.
    pub signed_set: SccpReplayArchiveSignedCheckpointSetV1,
    /// Exact signed accumulator checkpoint whose body carries the domain and
    /// constant-size forest projection.
    pub checkpoint: SccpReplayArchiveSignedCheckpointV1,
}

/// Canonical replay membership or non-membership response served by Torii.
///
/// The root and witness are one atomic response from the same locally rebuilt
/// head. Every 256-bit replay key is valid, including the all-zero key.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_sccp::replay_archive::SccpReplayWitnessResponseV1")]
pub struct SccpReplayWitnessResponseV1 {
    /// Response schema version. Final V1 accepts exactly one.
    pub version: u8,
    /// Locally revalidated signed root statement.
    pub root: SccpReplayRootResponseV1,
    /// Exact replay key requested by the caller.
    pub replay_key: [u8; 32],
    /// Unique canonical compressed membership or non-membership witness.
    pub witness: SccpSparseMerkleWitnessV1,
}

/// Replay archive validation failure. No variant retains attacker-controlled
/// paths, payloads, keys, signatures, or parser details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SccpReplayArchiveError {
    /// Record, witness, snapshot, checkpoint, or canonical framing is malformed.
    Malformed,
    /// A key was already occupied or leaves were not strictly ordered.
    DuplicateOrUnsortedLeaf,
    /// Rebuilt roots or counters do not match the claimed forest.
    RebuildMismatch,
    /// The requested accumulator is absent.
    UnknownAccumulator,
    /// Accumulator identity and complete domain disagree.
    AccumulatorDomainMismatch,
    /// A snapshot attempts an overwrite, fork, rollback, or network change.
    SnapshotRollback,
    /// Declared snapshot byte or leaf limits were exceeded.
    SnapshotLimit,
    /// The exact three-replica Ed25519 policy is invalid.
    ReplicaPolicy,
    /// A checkpoint is not signed by all three matching pinned replicas.
    ReplicaQuorum,
}

impl core::fmt::Display for SccpReplayArchiveError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Malformed => "malformed SCCP replay archive input",
            Self::DuplicateOrUnsortedLeaf => "duplicate or unsorted SCCP replay leaf",
            Self::RebuildMismatch => "SCCP replay archive rebuild mismatch",
            Self::UnknownAccumulator => "unknown SCCP replay accumulator",
            Self::AccumulatorDomainMismatch => "SCCP replay accumulator domain mismatch",
            Self::SnapshotRollback => "SCCP replay snapshot rollback or overwrite",
            Self::SnapshotLimit => "SCCP replay snapshot exceeds declared limits",
            Self::ReplicaPolicy => "invalid SCCP replay replica policy",
            Self::ReplicaQuorum => "SCCP replay archive replica quorum mismatch",
        })
    }
}

impl std::error::Error for SccpReplayArchiveError {}

/// Payload-free failure exposed by an archive service implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SccpReplayArchiveProviderErrorV1 {
    /// The accumulator or key is not registered by the provider.
    NotFound,
    /// The provider cannot currently serve independently verifiable data.
    Unavailable,
    /// Stored data failed local rebuilding or checkpoint authentication.
    Integrity,
}

impl core::fmt::Display for SccpReplayArchiveProviderErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::NotFound => "SCCP replay archive item not found",
            Self::Unavailable => "SCCP replay archive unavailable",
            Self::Integrity => "SCCP replay archive integrity failure",
        })
    }
}

impl std::error::Error for SccpReplayArchiveProviderErrorV1 {}

/// Narrow synchronous provider boundary suitable for Torii adapters.
///
/// Responses are owned so a provider cannot mutate them after validation.
/// Torii must still verify a witness against the returned forest before
/// serializing it.
pub trait SccpReplayArchiveProviderV1: Send + Sync {
    /// Return the complete validated domain and current forest.
    fn forest(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
    ) -> Result<(SccpReplayDomainV1, SccpReplayForestV1), SccpReplayArchiveProviderErrorV1>;

    /// Return a canonical membership or non-membership witness for one exact key.
    fn witness(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
        key: [u8; 32],
    ) -> Result<SccpSparseMerkleWitnessV1, SccpReplayArchiveProviderErrorV1>;

    /// Return the newest exactly-three-replica authenticated checkpoint.
    fn checkpoint(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
    ) -> Result<SccpReplayArchiveSignedCheckpointV1, SccpReplayArchiveProviderErrorV1>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotHeadV1 {
    content_sha256: [u8; 32],
    finality: SccpReplayArchiveFinalityV1,
    forest: SccpReplayForestV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedReplayShardV1 {
    shard: u8,
    /// Occupied records are hash-indexed for constant-time live mutation and
    /// lookup. Snapshot publication performs its own explicit canonical sort.
    leaves: HashMap<[u8; 32], [u8; 32]>,
    /// Non-default authenticated nodes indexed by leaf-up level. Level zero
    /// contains occupied leaf hashes and level 248 contains at most the shard
    /// root. Internal iteration order is never observable on the wire.
    levels: Vec<HashMap<[u8; 32], [u8; 32]>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedReplayInsertPlanV1 {
    key: [u8; 32],
    record_digest: [u8; 32],
    /// The prospective non-default node (or collision-produced default) at
    /// every level, starting with the occupied leaf and ending at the root.
    path: Vec<([u8; 32], [u8; 32])>,
    new_root: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CachedReplayWitnessPathV1 {
    expected_shard_root: [u8; 32],
    sibling_bitmap: [u8; 32],
    siblings: Vec<[u8; 32]>,
    /// Test-only counter proving witness work is independent of retained leaf
    /// cardinality rather than relying on wall-clock timing.
    #[cfg(test)]
    node_lookups: usize,
}

impl CachedReplayShardV1 {
    fn empty(shard: u8) -> Self {
        Self {
            shard,
            leaves: HashMap::new(),
            levels: (0..=SCCP_REPLAY_SMT_DEPTH_V1)
                .map(|_| HashMap::new())
                .collect(),
        }
    }

    fn from_leaves(
        shard: u8,
        leaves: BTreeMap<[u8; 32], [u8; 32]>,
    ) -> Result<Self, SccpReplayArchiveError> {
        let mut cached = Self::empty(shard);
        for (key, record_digest) in &leaves {
            if key[0] != shard || *record_digest == [0; 32] {
                return Err(SccpReplayArchiveError::Malformed);
            }
            cached.levels[0].insert(*key, occupied_leaf_hash(*key, *record_digest));
        }
        cached.leaves = leaves.into_iter().collect();

        let empty = replay_empty_hashes();
        for level in 0..SCCP_REPLAY_SMT_DEPTH_V1 {
            let parents = {
                let nodes = &cached.levels[level];
                let bases = nodes
                    .keys()
                    .copied()
                    .map(|position| clear_bit(position, level))
                    .collect::<BTreeSet<_>>();
                bases
                    .into_iter()
                    .filter_map(|base| {
                        let right_position = set_position_bit(base, level);
                        let left = nodes.get(&base).copied().unwrap_or(empty[level]);
                        let right = nodes.get(&right_position).copied().unwrap_or(empty[level]);
                        let parent = parent_hash(level, left, right);
                        (parent != empty[level + 1]).then_some((base, parent))
                    })
                    .collect::<Vec<_>>()
            };
            cached.levels[level + 1].extend(parents);
        }
        Ok(cached)
    }

    fn root(&self) -> [u8; 32] {
        let mut position = [0; 32];
        position[0] = self.shard;
        self.levels[SCCP_REPLAY_SMT_DEPTH_V1]
            .get(&position)
            .copied()
            .unwrap_or(replay_empty_hashes()[SCCP_REPLAY_SMT_DEPTH_V1])
    }

    fn plan_insert(
        &self,
        key: [u8; 32],
        record_digest: [u8; 32],
    ) -> Result<CachedReplayInsertPlanV1, SccpReplayArchiveError> {
        if key[0] != self.shard || record_digest == [0; 32] {
            return Err(SccpReplayArchiveError::Malformed);
        }
        if self.leaves.contains_key(&key) {
            return Err(SccpReplayArchiveError::DuplicateOrUnsortedLeaf);
        }

        let empty = replay_empty_hashes();
        let mut position = key;
        let mut current = occupied_leaf_hash(key, record_digest);
        let mut path = Vec::with_capacity(SCCP_REPLAY_SMT_DEPTH_V1 + 1);
        path.push((position, current));
        for level in 0..SCCP_REPLAY_SMT_DEPTH_V1 {
            let sibling = self.levels[level]
                .get(&toggle_bit(position, level))
                .copied()
                .unwrap_or(empty[level]);
            current = if position_bit_is_set(&position, level) {
                parent_hash(level, sibling, current)
            } else {
                parent_hash(level, current, sibling)
            };
            position = clear_bit(position, level);
            path.push((position, current));
        }
        Ok(CachedReplayInsertPlanV1 {
            key,
            record_digest,
            path,
            new_root: current,
        })
    }

    fn commit_insert(&mut self, plan: CachedReplayInsertPlanV1) {
        debug_assert_eq!(plan.key[0], self.shard);
        debug_assert_eq!(plan.path.len(), SCCP_REPLAY_SMT_DEPTH_V1 + 1);
        let previous = self.leaves.insert(plan.key, plan.record_digest);
        debug_assert!(previous.is_none());
        let empty = replay_empty_hashes();
        for (level, (position, node)) in plan.path.into_iter().enumerate() {
            if node == empty[level] {
                self.levels[level].remove(&position);
            } else {
                self.levels[level].insert(position, node);
            }
        }
    }

    fn witness_path(&self, key: [u8; 32]) -> CachedReplayWitnessPathV1 {
        debug_assert_eq!(key[0], self.shard);
        let empty = replay_empty_hashes();
        let mut position = key;
        let mut sibling_bitmap = [0; 32];
        let mut siblings = Vec::new();
        #[cfg(test)]
        let mut node_lookups = 0;
        for level in 0..SCCP_REPLAY_SMT_DEPTH_V1 {
            let sibling = self.levels[level]
                .get(&toggle_bit(position, level))
                .copied()
                .unwrap_or(empty[level]);
            #[cfg(test)]
            {
                node_lookups += 1;
            }
            if sibling != empty[level] {
                set_bit(&mut sibling_bitmap, level);
                siblings.push(sibling);
            }
            position = clear_bit(position, level);
        }
        CachedReplayWitnessPathV1 {
            expected_shard_root: self.root(),
            sibling_bitmap,
            siblings,
            #[cfg(test)]
            node_lookups,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CachedReplayTreeV1 {
    shards: BTreeMap<u8, CachedReplayShardV1>,
    leaf_count: u64,
}

impl CachedReplayTreeV1 {
    fn from_leaves(leaves: BTreeMap<[u8; 32], [u8; 32]>) -> Result<Self, SccpReplayArchiveError> {
        let leaf_count =
            u64::try_from(leaves.len()).map_err(|_| SccpReplayArchiveError::Malformed)?;
        let mut grouped = BTreeMap::<u8, BTreeMap<[u8; 32], [u8; 32]>>::new();
        for (key, record_digest) in leaves {
            grouped
                .entry(key[0])
                .or_default()
                .insert(key, record_digest);
        }
        let mut shards = BTreeMap::new();
        for (shard, leaves) in grouped {
            shards.insert(shard, CachedReplayShardV1::from_leaves(shard, leaves)?);
        }
        Ok(Self { shards, leaf_count })
    }

    fn contains_key(&self, key: &[u8; 32]) -> bool {
        self.shards
            .get(&key[0])
            .is_some_and(|shard| shard.leaves.contains_key(key))
    }

    fn leaf_digest(&self, key: &[u8; 32]) -> Option<[u8; 32]> {
        self.shards
            .get(&key[0])
            .and_then(|shard| shard.leaves.get(key).copied())
    }

    fn shard_root(&self, shard: u8) -> [u8; 32] {
        self.shards.get(&shard).map_or_else(
            || replay_empty_hashes()[SCCP_REPLAY_SMT_DEPTH_V1],
            CachedReplayShardV1::root,
        )
    }

    fn plan_insert(
        &self,
        key: [u8; 32],
        record_digest: [u8; 32],
    ) -> Result<CachedReplayInsertPlanV1, SccpReplayArchiveError> {
        self.shards.get(&key[0]).map_or_else(
            || CachedReplayShardV1::empty(key[0]).plan_insert(key, record_digest),
            |shard| shard.plan_insert(key, record_digest),
        )
    }

    fn commit_insert(&mut self, plan: CachedReplayInsertPlanV1) {
        let shard = plan.key[0];
        self.shards
            .entry(shard)
            .or_insert_with(|| CachedReplayShardV1::empty(shard))
            .commit_insert(plan);
        self.leaf_count = self
            .leaf_count
            .checked_add(1)
            .expect("validated replay leaf count remains in range");
    }

    fn witness_path(&self, key: [u8; 32]) -> CachedReplayWitnessPathV1 {
        self.shards.get(&key[0]).map_or_else(
            || CachedReplayShardV1::empty(key[0]).witness_path(key),
            |shard| shard.witness_path(key),
        )
    }

    fn iter_leaves(&self) -> impl Iterator<Item = (&[u8; 32], &[u8; 32])> {
        self.shards.values().flat_map(|shard| shard.leaves.iter())
    }

    fn sorted_snapshot_leaves(&self) -> Vec<SccpReplayArchiveLeafV1> {
        let mut leaves = self
            .iter_leaves()
            .map(|(key, record_digest)| SccpReplayArchiveLeafV1 {
                key: *key,
                record_digest: *record_digest,
            })
            .collect::<Vec<_>>();
        leaves.sort_unstable_by_key(|leaf| leaf.key);
        leaves
    }

    fn is_subset_of(&self, other: &Self) -> bool {
        self.leaf_count <= other.leaf_count
            && self
                .iter_leaves()
                .all(|(key, digest)| other.leaf_digest(key) == Some(*digest))
    }

    fn forest(&self) -> Result<SccpReplayForestV1, SccpReplayArchiveError> {
        let nonempty_shard_roots = self
            .shards
            .iter()
            .map(|(shard, cached)| (*shard, cached.root()))
            .collect();
        let forest = SccpReplayForestV1 {
            nonempty_shard_roots,
            leaf_count: self.leaf_count,
            update_sequence: self.leaf_count,
        };
        forest
            .validate()
            .map_err(|_| SccpReplayArchiveError::RebuildMismatch)?;
        Ok(forest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccumulatorArchiveV1 {
    domain: SccpReplayDomainV1,
    domain_hash: [u8; 32],
    tree: CachedReplayTreeV1,
    forest: SccpReplayForestV1,
    snapshot_head: Option<SnapshotHeadV1>,
}

fn apply_record_error(error: SccpReplayAccumulatorError) -> SccpReplayArchiveError {
    match error {
        SccpReplayAccumulatorError::InvalidDomain => {
            SccpReplayArchiveError::AccumulatorDomainMismatch
        }
        SccpReplayAccumulatorError::InvalidPrincipal
        | SccpReplayAccumulatorError::InvalidRecord
        | SccpReplayAccumulatorError::WrongBoundary
        | SccpReplayAccumulatorError::NonCanonicalWitness => SccpReplayArchiveError::Malformed,
        SccpReplayAccumulatorError::Occupied => SccpReplayArchiveError::DuplicateOrUnsortedLeaf,
        SccpReplayAccumulatorError::StaleRoot
        | SccpReplayAccumulatorError::InvalidPath
        | SccpReplayAccumulatorError::CounterExhausted
        | SccpReplayAccumulatorError::InvalidForest => SccpReplayArchiveError::RebuildMismatch,
    }
}

/// In-memory reference implementation used by independent archive services.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SccpReplayArchiveV1 {
    accumulators: BTreeMap<SccpReplayAccumulatorIdV1, AccumulatorArchiveV1>,
}

impl SccpReplayArchiveV1 {
    /// Register an empty route boundary from its complete governed domain before
    /// its first authenticated mutation arrives.
    pub fn initialize_accumulator(
        &mut self,
        accumulator_id: SccpReplayAccumulatorIdV1,
        domain: SccpReplayDomainV1,
    ) -> Result<(), SccpReplayArchiveError> {
        validate_accumulator_domain(&accumulator_id, &domain)?;
        let domain_hash = sccp_replay_domain_hash_v1(&domain)
            .map_err(|_| SccpReplayArchiveError::AccumulatorDomainMismatch)?;
        if let Some(existing) = self.accumulators.get(&accumulator_id) {
            return (existing.domain == domain && existing.domain_hash == domain_hash)
                .then_some(())
                .ok_or(SccpReplayArchiveError::AccumulatorDomainMismatch);
        }
        self.accumulators.insert(
            accumulator_id,
            AccumulatorArchiveV1 {
                domain,
                domain_hash,
                tree: CachedReplayTreeV1::default(),
                forest: SccpReplayForestV1::default(),
                snapshot_head: None,
            },
        );
        Ok(())
    }

    /// Apply one record against its authenticated consensus witness and cached path.
    ///
    /// All fallible verification precedes the atomic leaf and forest update.
    pub fn apply_record(
        &mut self,
        accumulator_id: SccpReplayAccumulatorIdV1,
        record: &SccpReplayRecordV1,
        witness: &SccpSparseMerkleWitnessV1,
    ) -> Result<(), SccpReplayArchiveError> {
        let existing = self
            .accumulators
            .get(&accumulator_id)
            .ok_or(SccpReplayArchiveError::UnknownAccumulator)?;

        if existing.tree.leaf_count != existing.forest.leaf_count {
            return Err(SccpReplayArchiveError::RebuildMismatch);
        }

        let mut next_forest = existing.forest.clone();
        let delta = next_forest
            .occupy(&existing.domain, record, witness)
            .map_err(apply_record_error)?;

        if delta.domain_hash != existing.domain_hash
            || delta.record_digest == [0; 32]
            || delta.shard != delta.key[0]
            || delta.old_root == delta.new_root
            || delta.old_root != existing.forest.shard_root(delta.shard)
            || delta.new_root != next_forest.shard_root(delta.shard)
            || delta.leaf_count != next_forest.leaf_count
            || delta.update_sequence != next_forest.update_sequence
        {
            return Err(SccpReplayArchiveError::RebuildMismatch);
        }
        next_forest
            .validate()
            .map_err(|_| SccpReplayArchiveError::RebuildMismatch)?;

        if existing.tree.contains_key(&delta.key) {
            return Err(SccpReplayArchiveError::DuplicateOrUnsortedLeaf);
        }
        if existing.tree.shard_root(delta.shard) != delta.old_root {
            return Err(SccpReplayArchiveError::RebuildMismatch);
        }
        let insert_plan = existing.tree.plan_insert(delta.key, delta.record_digest)?;
        if insert_plan.new_root != delta.new_root {
            return Err(SccpReplayArchiveError::RebuildMismatch);
        }
        let existing = self
            .accumulators
            .get_mut(&accumulator_id)
            .expect("checked accumulator remains present");
        existing.tree.commit_insert(insert_plan);
        existing.forest = next_forest;
        Ok(())
    }

    /// Generate the unique compressed witness for an occupied or empty leaf.
    pub fn witness(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
        key: [u8; 32],
    ) -> Result<SccpSparseMerkleWitnessV1, SccpReplayArchiveError> {
        let accumulator = self
            .accumulators
            .get(accumulator_id)
            .ok_or(SccpReplayArchiveError::UnknownAccumulator)?;
        let path = accumulator.tree.witness_path(key);
        let witness = SccpSparseMerkleWitnessV1 {
            expected_shard_root: path.expected_shard_root,
            prior_record_digest: accumulator.tree.leaf_digest(&key).unwrap_or([0; 32]),
            sibling_bitmap: path.sibling_bitmap,
            siblings: path.siblings,
        };
        accumulator
            .forest
            .verify_key_digest(key, witness.prior_record_digest, &witness)
            .map_err(|_| SccpReplayArchiveError::RebuildMismatch)?;
        Ok(witness)
    }

    /// Return the complete domain and rebuilt forest for one accumulator.
    pub fn forest(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
    ) -> Result<(SccpReplayDomainV1, &SccpReplayForestV1), SccpReplayArchiveError> {
        self.accumulators
            .get(accumulator_id)
            .map(|archive| (archive.domain, &archive.forest))
            .ok_or(SccpReplayArchiveError::UnknownAccumulator)
    }

    /// Publish the next immutable, strictly chained snapshot.
    pub fn publish_snapshot(
        &mut self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
        finality: SccpReplayArchiveFinalityV1,
    ) -> Result<SccpReplayArchiveSnapshotV1, SccpReplayArchiveError> {
        let archive = self
            .accumulators
            .get(accumulator_id)
            .ok_or(SccpReplayArchiveError::UnknownAccumulator)?;
        validate_snapshot_successor(archive, finality)?;
        let snapshot = SccpReplayArchiveSnapshotV1 {
            version: SNAPSHOT_VERSION_V1,
            accumulator_id: accumulator_id.clone(),
            domain: archive.domain,
            finality,
            forest: archive.forest.clone(),
            leaves: archive.tree.sorted_snapshot_leaves(),
        };
        validate_snapshot(&snapshot, SccpReplayArchiveDecodeLimitsV1::default())?;
        let head = SnapshotHeadV1 {
            content_sha256: snapshot.content_sha256()?,
            finality,
            forest: snapshot.forest.clone(),
        };
        self.accumulators
            .get_mut(accumulator_id)
            .expect("checked accumulator remains present")
            .snapshot_head = Some(head);
        Ok(snapshot)
    }

    /// Decode and restore one canonical snapshot under explicit finite limits.
    pub fn restore_snapshot_bytes(
        &mut self,
        bytes: &[u8],
        limits: SccpReplayArchiveDecodeLimitsV1,
    ) -> Result<(), SccpReplayArchiveError> {
        let (snapshot, validated) = decode_validated_snapshot(bytes, limits)?;
        self.restore_validated_snapshot(snapshot, validated)
    }

    /// Restore one already-decoded snapshot after recomputing every shard root,
    /// enforcing its declared limits, and refusing forks or rollbacks.
    pub fn restore_snapshot(
        &mut self,
        snapshot: SccpReplayArchiveSnapshotV1,
        limits: SccpReplayArchiveDecodeLimitsV1,
    ) -> Result<(), SccpReplayArchiveError> {
        let validated = validate_snapshot(&snapshot, limits)?;
        self.restore_validated_snapshot(snapshot, validated)
    }

    fn restore_validated_snapshot(
        &mut self,
        snapshot: SccpReplayArchiveSnapshotV1,
        validated: ValidatedSnapshotV1,
    ) -> Result<(), SccpReplayArchiveError> {
        let ValidatedSnapshotV1 {
            tree,
            content_sha256,
        } = validated;
        if let Some(existing) = self.accumulators.get(&snapshot.accumulator_id) {
            if existing.domain != snapshot.domain {
                return Err(SccpReplayArchiveError::AccumulatorDomainMismatch);
            }
            if existing
                .snapshot_head
                .as_ref()
                .is_some_and(|head| head.content_sha256 == content_sha256)
            {
                return (existing.forest == snapshot.forest && existing.tree == tree)
                    .then_some(())
                    .ok_or(SccpReplayArchiveError::SnapshotRollback);
            }
            validate_snapshot_successor(existing, snapshot.finality)?;
            if !existing.tree.is_subset_of(&tree)
                || snapshot.forest.leaf_count < existing.forest.leaf_count
                || snapshot.forest.update_sequence < existing.forest.update_sequence
            {
                return Err(SccpReplayArchiveError::SnapshotRollback);
            }
        }
        let domain_hash = sccp_replay_domain_hash_v1(&snapshot.domain)
            .map_err(|_| SccpReplayArchiveError::AccumulatorDomainMismatch)?;
        self.accumulators.insert(
            snapshot.accumulator_id,
            AccumulatorArchiveV1 {
                domain: snapshot.domain,
                domain_hash,
                tree,
                forest: snapshot.forest.clone(),
                snapshot_head: Some(SnapshotHeadV1 {
                    content_sha256,
                    finality: snapshot.finality,
                    forest: snapshot.forest,
                }),
            },
        );
        Ok(())
    }
}

/// Decode and fully rebuild one standalone canonical snapshot under explicit
/// finite limits, without accepting it as a successor of local state.
pub fn decode_sccp_replay_archive_snapshot_v1(
    bytes: &[u8],
    limits: SccpReplayArchiveDecodeLimitsV1,
) -> Result<SccpReplayArchiveSnapshotV1, SccpReplayArchiveError> {
    decode_validated_snapshot(bytes, limits).map(|(snapshot, _)| snapshot)
}

fn decode_validated_snapshot(
    bytes: &[u8],
    limits: SccpReplayArchiveDecodeLimitsV1,
) -> Result<(SccpReplayArchiveSnapshotV1, ValidatedSnapshotV1), SccpReplayArchiveError> {
    if limits.max_snapshot_bytes == 0 || limits.max_snapshot_leaves == 0 {
        return Err(SccpReplayArchiveError::SnapshotLimit);
    }
    if bytes.is_empty() {
        return Err(SccpReplayArchiveError::Malformed);
    }
    if bytes.len() > limits.max_snapshot_bytes {
        return Err(SccpReplayArchiveError::SnapshotLimit);
    }
    // Reject oversized variable collections before their backing allocations.
    // A forest can legitimately contain all 256 shard roots independently of
    // the leaf cap, so that fixed schema maximum is the per-sequence floor.
    let canonical_limits = norito::canonical_decode_limits(bytes.len());
    let decode_limits = norito::DecodeLimits::new(
        limits
            .max_snapshot_leaves
            .max(SCCP_REPLAY_SMT_SHARD_COUNT_V1),
        canonical_limits.max_field_bytes(),
        canonical_limits.max_total_elements(),
        canonical_limits.max_total_allocated_bytes(),
        canonical_limits.max_nesting_depth(),
    );
    let snapshot = match norito::decode_canonical_with_limits(bytes, decode_limits) {
        Ok(snapshot) => snapshot,
        Err(error) if error.is_decode_resource_limit() => {
            return Err(SccpReplayArchiveError::SnapshotLimit);
        }
        Err(_) => return Err(SccpReplayArchiveError::Malformed),
    };
    validate_snapshot_metadata(&snapshot, limits)?;
    let validated = ValidatedSnapshotV1 {
        tree: validate_snapshot_leaves(&snapshot)?,
        content_sha256: sha256(&[bytes]),
    };
    Ok((snapshot, validated))
}

impl SccpReplayArchiveProviderV1 for SccpReplayArchiveV1 {
    fn forest(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
    ) -> Result<(SccpReplayDomainV1, SccpReplayForestV1), SccpReplayArchiveProviderErrorV1> {
        SccpReplayArchiveV1::forest(self, accumulator_id)
            .map(|(domain, forest)| (domain, forest.clone()))
            .map_err(provider_error)
    }

    fn witness(
        &self,
        accumulator_id: &SccpReplayAccumulatorIdV1,
        key: [u8; 32],
    ) -> Result<SccpSparseMerkleWitnessV1, SccpReplayArchiveProviderErrorV1> {
        SccpReplayArchiveV1::witness(self, accumulator_id, key).map_err(provider_error)
    }

    fn checkpoint(
        &self,
        _accumulator_id: &SccpReplayAccumulatorIdV1,
    ) -> Result<SccpReplayArchiveSignedCheckpointV1, SccpReplayArchiveProviderErrorV1> {
        Err(SccpReplayArchiveProviderErrorV1::Unavailable)
    }
}

fn provider_error(error: SccpReplayArchiveError) -> SccpReplayArchiveProviderErrorV1 {
    match error {
        SccpReplayArchiveError::UnknownAccumulator => SccpReplayArchiveProviderErrorV1::NotFound,
        SccpReplayArchiveError::Malformed
        | SccpReplayArchiveError::DuplicateOrUnsortedLeaf
        | SccpReplayArchiveError::RebuildMismatch
        | SccpReplayArchiveError::AccumulatorDomainMismatch
        | SccpReplayArchiveError::SnapshotRollback
        | SccpReplayArchiveError::SnapshotLimit
        | SccpReplayArchiveError::ReplicaPolicy
        | SccpReplayArchiveError::ReplicaQuorum => SccpReplayArchiveProviderErrorV1::Integrity,
    }
}

/// Authenticate an exact three-replica agreement against pinned Ed25519 keys.
pub fn verify_sccp_replay_archive_checkpoint_v1(
    policy: &SccpReplayArchiveReplicaPolicyV1,
    checkpoint: &SccpReplayArchiveSignedCheckpointV1,
) -> Result<SccpReplayArchiveCheckpointBodyV1, SccpReplayArchiveError> {
    policy.validate()?;
    validate_checkpoint_body(&checkpoint.body)?;
    let message = checkpoint.body.signing_message()?;
    verify_replica_attestations(policy, message, &checkpoint.attestations)?;
    Ok(checkpoint.body.clone())
}

/// Authenticate the exact complete ordered checkpoint set against all three
/// pinned replica keys. An empty set is valid only through this signed shape.
pub fn verify_sccp_replay_archive_checkpoint_set_v1(
    policy: &SccpReplayArchiveReplicaPolicyV1,
    checkpoint_set: &SccpReplayArchiveSignedCheckpointSetV1,
) -> Result<SccpReplayArchiveCheckpointSetBodyV1, SccpReplayArchiveError> {
    policy.validate()?;
    validate_checkpoint_set_body(&checkpoint_set.body)?;
    let message = checkpoint_set.body.signing_message()?;
    verify_replica_attestations(policy, message, &checkpoint_set.attestations)?;
    Ok(checkpoint_set.body.clone())
}

/// Authenticate and cross-bind one public replay-root response.
///
/// `expected_checkpoint_set_sha256` and `expected_signed_set` must be obtained
/// by canonical decoding of the same separately authenticated complete
/// checkpoint-set frame. The digest is normally computed with
/// [`sccp_replay_archive_checkpoint_set_frame_sha256_v1`]. Both are explicit
/// inputs because the public root response contains neither the snapshots nor
/// the SoraFS manifest bytes that comprise the complete replica frame; a
/// response is never allowed to self-assert its frame identity by merely
/// copying an expected digest.
///
/// On success the returned checkpoint body is authenticated by all three
/// pinned replicas, is the exact accumulator requested by the caller, occurs
/// in the signed complete inventory, shares its finalized coordinate, and
/// binds the returned forest roots, checked leaf count, and update sequence.
pub fn verify_sccp_replay_root_response_v1(
    policy: &SccpReplayArchiveReplicaPolicyV1,
    expected_checkpoint_set_sha256: [u8; 32],
    expected_signed_set: &SccpReplayArchiveSignedCheckpointSetV1,
    expected_accumulator_id: &SccpReplayAccumulatorIdV1,
    response: &SccpReplayRootResponseV1,
) -> Result<SccpReplayArchiveCheckpointBodyV1, SccpReplayArchiveError> {
    if response.version != 1
        || expected_checkpoint_set_sha256 == [0; 32]
        || response.checkpoint_set_sha256 != expected_checkpoint_set_sha256
        || &response.signed_set != expected_signed_set
    {
        return Err(SccpReplayArchiveError::Malformed);
    }

    let set_body = verify_sccp_replay_archive_checkpoint_set_v1(policy, expected_signed_set)?;
    let checkpoint_body = verify_sccp_replay_archive_checkpoint_v1(policy, &response.checkpoint)?;
    if &checkpoint_body.accumulator_id != expected_accumulator_id {
        return Err(SccpReplayArchiveError::AccumulatorDomainMismatch);
    }

    let entry = set_body
        .entries
        .binary_search_by(|entry| entry.accumulator_id.cmp(expected_accumulator_id))
        .ok()
        .and_then(|index| set_body.entries.get(index))
        .ok_or(SccpReplayArchiveError::RebuildMismatch)?;
    if entry.snapshot_sha256 != checkpoint_body.snapshot_sha256
        || entry.checkpoint_agreement_digest != checkpoint_body.agreement_digest()?
        || !set_body
            .finality
            .matches_checkpoint(checkpoint_body.finality)
    {
        return Err(SccpReplayArchiveError::RebuildMismatch);
    }
    Ok(checkpoint_body)
}

/// Authenticate one public replay membership or non-membership response.
///
/// This performs every root-response check and additionally binds the exact
/// caller-requested replay key, prior occupied-record digest, compressed
/// sibling path, expected shard root, and signed forest into one result. The
/// all-zero replay key and a zero prior digest remain valid canonical
/// non-membership values.
pub fn verify_sccp_replay_witness_response_v1(
    policy: &SccpReplayArchiveReplicaPolicyV1,
    expected_checkpoint_set_sha256: [u8; 32],
    expected_signed_set: &SccpReplayArchiveSignedCheckpointSetV1,
    expected_accumulator_id: &SccpReplayAccumulatorIdV1,
    expected_replay_key: [u8; 32],
    response: &SccpReplayWitnessResponseV1,
) -> Result<(SccpReplayArchiveCheckpointBodyV1, [u8; 32]), SccpReplayArchiveError> {
    if response.version != 1 || response.replay_key != expected_replay_key {
        return Err(SccpReplayArchiveError::Malformed);
    }
    let checkpoint_body = verify_sccp_replay_root_response_v1(
        policy,
        expected_checkpoint_set_sha256,
        expected_signed_set,
        expected_accumulator_id,
        &response.root,
    )?;
    checkpoint_body
        .forest
        .verify_key_digest(
            response.replay_key,
            response.witness.prior_record_digest,
            &response.witness,
        )
        .map_err(|_| SccpReplayArchiveError::RebuildMismatch)?;
    Ok((checkpoint_body, response.witness.prior_record_digest))
}

fn verify_replica_attestations(
    policy: &SccpReplayArchiveReplicaPolicyV1,
    message: [u8; 32],
    attestations: &[SccpReplayArchiveReplicaAttestationV1; 3],
) -> Result<(), SccpReplayArchiveError> {
    for ((binding, attestation), index) in policy
        .replicas
        .iter()
        .zip(attestations.iter())
        .zip(0_usize..)
    {
        if binding.replica_id != attestation.replica_id
            || attestations[..index]
                .iter()
                .any(|prior| prior.replica_id == attestation.replica_id)
            || iroha_crypto::ed25519_verify_batch_deterministic(
                &[message.as_slice()],
                &[attestation.signature.as_slice()],
                &[binding.ed25519_public_key.as_slice()],
            )
            .is_err()
        {
            return Err(SccpReplayArchiveError::ReplicaQuorum);
        }
    }
    Ok(())
}

fn validate_checkpoint_body(
    body: &SccpReplayArchiveCheckpointBodyV1,
) -> Result<(), SccpReplayArchiveError> {
    if body.version != CHECKPOINT_VERSION_V1
        || body.snapshot_sha256 == [0; 32]
        || !body.finality.is_well_formed()
    {
        return Err(SccpReplayArchiveError::Malformed);
    }
    validate_accumulator_domain(&body.accumulator_id, &body.domain)?;
    body.forest
        .validate()
        .map_err(|_| SccpReplayArchiveError::RebuildMismatch)
}

fn checkpoint_set_inventory_sha256(
    finality: SccpReplayArchiveHeadFinalityV1,
    entries: &[SccpReplayArchiveCheckpointSetEntryV1],
) -> Result<[u8; 32], SccpReplayArchiveError> {
    let inventory = SccpReplayArchiveCheckpointInventoryV1 {
        version: CHECKPOINT_SET_VERSION_V1,
        finality,
        entries: entries.to_vec(),
    };
    let encoded =
        norito::encode_canonical(&inventory).map_err(|_| SccpReplayArchiveError::Malformed)?;
    Ok(sha256(&[
        CHECKPOINT_SET_INVENTORY_DOMAIN_V1,
        &u64::try_from(encoded.len())
            .map_err(|_| SccpReplayArchiveError::Malformed)?
            .to_be_bytes(),
        &encoded,
    ]))
}

/// Derive the exact inventory digest that a SoraFS replay-snapshot manifest
/// must carry in its sole SCCP metadata entry.
pub fn sccp_replay_archive_checkpoint_set_inventory_sha256_v1(
    finality: SccpReplayArchiveHeadFinalityV1,
    entries: &[SccpReplayArchiveCheckpointSetEntryV1],
) -> Result<[u8; 32], SccpReplayArchiveError> {
    checkpoint_set_inventory_sha256(finality, entries)
}

fn validate_checkpoint_set_body(
    body: &SccpReplayArchiveCheckpointSetBodyV1,
) -> Result<(), SccpReplayArchiveError> {
    if body.version != CHECKPOINT_SET_VERSION_V1
        || !body.finality.is_well_formed()
        || !body.sorafs_manifest.is_well_formed()
        || usize::try_from(body.entry_count).ok() != Some(body.entries.len())
        || checkpoint_set_inventory_sha256(body.finality, &body.entries)? != body.inventory_sha256
    {
        return Err(SccpReplayArchiveError::Malformed);
    }
    let mut previous = None;
    let mut snapshot_total_bytes = 0_u64;
    for entry in &body.entries {
        entry
            .accumulator_id
            .route_key
            .validate()
            .map_err(|_| SccpReplayArchiveError::Malformed)?;
        if entry.snapshot_sha256 == [0; 32]
            || entry.snapshot_size_bytes == 0
            || entry.checkpoint_agreement_digest == [0; 32]
            || previous
                .as_ref()
                .is_some_and(|prior| prior >= &entry.accumulator_id)
        {
            return Err(SccpReplayArchiveError::Malformed);
        }
        snapshot_total_bytes = snapshot_total_bytes
            .checked_add(entry.snapshot_size_bytes)
            .ok_or(SccpReplayArchiveError::Malformed)?;
        previous = Some(entry.accumulator_id.clone());
    }
    if snapshot_total_bytes != body.sorafs_manifest.snapshot_total_bytes {
        return Err(SccpReplayArchiveError::Malformed);
    }
    Ok(())
}

fn validate_accumulator_domain(
    accumulator_id: &SccpReplayAccumulatorIdV1,
    domain: &SccpReplayDomainV1,
) -> Result<(), SccpReplayArchiveError> {
    accumulator_id
        .validate_domain(domain)
        .map_err(|_| SccpReplayArchiveError::AccumulatorDomainMismatch)
}

struct ValidatedSnapshotV1 {
    tree: CachedReplayTreeV1,
    content_sha256: [u8; 32],
}

fn validate_snapshot(
    snapshot: &SccpReplayArchiveSnapshotV1,
    limits: SccpReplayArchiveDecodeLimitsV1,
) -> Result<ValidatedSnapshotV1, SccpReplayArchiveError> {
    validate_snapshot_metadata(snapshot, limits)?;
    let canonical_bytes =
        norito::encode_canonical(snapshot).map_err(|_| SccpReplayArchiveError::Malformed)?;
    if canonical_bytes.len() > limits.max_snapshot_bytes {
        return Err(SccpReplayArchiveError::SnapshotLimit);
    }
    let content_sha256 = sha256(&[&canonical_bytes]);
    drop(canonical_bytes);
    Ok(ValidatedSnapshotV1 {
        tree: validate_snapshot_leaves(snapshot)?,
        content_sha256,
    })
}

fn validate_snapshot_metadata(
    snapshot: &SccpReplayArchiveSnapshotV1,
    limits: SccpReplayArchiveDecodeLimitsV1,
) -> Result<(), SccpReplayArchiveError> {
    if limits.max_snapshot_bytes == 0 || limits.max_snapshot_leaves == 0 {
        return Err(SccpReplayArchiveError::SnapshotLimit);
    }
    if snapshot.version != SNAPSHOT_VERSION_V1 || !snapshot.finality.is_well_formed() {
        return Err(SccpReplayArchiveError::Malformed);
    }
    if snapshot.leaves.len() > limits.max_snapshot_leaves {
        return Err(SccpReplayArchiveError::SnapshotLimit);
    }
    validate_accumulator_domain(&snapshot.accumulator_id, &snapshot.domain)?;
    snapshot
        .forest
        .validate()
        .map_err(|_| SccpReplayArchiveError::RebuildMismatch)?;
    if u64::try_from(snapshot.leaves.len()).ok() != Some(snapshot.forest.leaf_count) {
        return Err(SccpReplayArchiveError::RebuildMismatch);
    }
    Ok(())
}

fn validate_snapshot_leaves(
    snapshot: &SccpReplayArchiveSnapshotV1,
) -> Result<CachedReplayTreeV1, SccpReplayArchiveError> {
    let mut leaves = BTreeMap::new();
    let mut previous = None;
    for leaf in &snapshot.leaves {
        if leaf.record_digest == [0; 32]
            || previous.is_some_and(|value| value >= leaf.key)
            || leaves.insert(leaf.key, leaf.record_digest).is_some()
        {
            return Err(SccpReplayArchiveError::DuplicateOrUnsortedLeaf);
        }
        previous = Some(leaf.key);
    }
    let tree = CachedReplayTreeV1::from_leaves(leaves)?;
    let rebuilt = tree.forest()?;
    if rebuilt != snapshot.forest {
        return Err(SccpReplayArchiveError::RebuildMismatch);
    }
    Ok(tree)
}

fn validate_snapshot_successor(
    archive: &AccumulatorArchiveV1,
    finality: SccpReplayArchiveFinalityV1,
) -> Result<(), SccpReplayArchiveError> {
    if !finality.is_well_formed() {
        return Err(SccpReplayArchiveError::Malformed);
    }
    match &archive.snapshot_head {
        None => Ok(()),
        Some(head)
            if finality.network_identity_sha256 == head.finality.network_identity_sha256
                && finality.finalized_height > head.finality.finalized_height
                && finality.finalized_block_hash != head.finality.finalized_block_hash
                && archive.forest.leaf_count >= head.forest.leaf_count
                && archive.forest.update_sequence >= head.forest.update_sequence =>
        {
            Ok(())
        }
        Some(_) => Err(SccpReplayArchiveError::SnapshotRollback),
    }
}

#[cfg(test)]
type WitnessPathV1 = ([u8; 32], Vec<[u8; 32]>);

#[cfg(test)]
fn reference_shard_root(
    leaves: &BTreeMap<[u8; 32], [u8; 32]>,
    shard: u8,
    witness_key: Option<[u8; 32]>,
) -> Result<([u8; 32], Option<WitnessPathV1>), SccpReplayArchiveError> {
    if witness_key.is_some_and(|key| key[0] != shard) {
        return Err(SccpReplayArchiveError::Malformed);
    }
    let empty = replay_empty_hashes();
    let mut nodes = leaves
        .iter()
        .filter(|(key, _)| key[0] == shard)
        .map(|(key, digest)| (*key, occupied_leaf_hash(*key, *digest)))
        .collect::<BTreeMap<_, _>>();
    let mut bitmap = [0_u8; 32];
    let mut siblings = Vec::new();
    let mut target_position = witness_key;

    for level in 0..SCCP_REPLAY_SMT_DEPTH_V1 {
        if let Some(position) = target_position {
            let sibling_position = toggle_bit(position, level);
            let sibling = nodes
                .get(&sibling_position)
                .copied()
                .unwrap_or(empty[level]);
            if sibling != empty[level] {
                set_bit(&mut bitmap, level);
                siblings.push(sibling);
            }
            target_position = Some(clear_bit(position, level));
        }

        let positions = nodes.keys().copied().collect::<Vec<_>>();
        let mut next = BTreeMap::new();
        for position in positions {
            let base = clear_bit(position, level);
            if next.contains_key(&base) {
                continue;
            }
            let right_position = set_position_bit(base, level);
            let left = nodes.get(&base).copied().unwrap_or(empty[level]);
            let right = nodes.get(&right_position).copied().unwrap_or(empty[level]);
            let parent = parent_hash(level, left, right);
            if parent != empty[level + 1] {
                next.insert(base, parent);
            }
        }
        nodes = next;
    }

    let mut root_position = [0_u8; 32];
    root_position[0] = shard;
    let root = nodes
        .get(&root_position)
        .copied()
        .unwrap_or(empty[SCCP_REPLAY_SMT_DEPTH_V1]);
    let path = witness_key.map(|_| (bitmap, siblings));
    Ok((root, path))
}

#[cfg(test)]
fn reference_rebuild_forest(
    leaves: &BTreeMap<[u8; 32], [u8; 32]>,
) -> Result<SccpReplayForestV1, SccpReplayArchiveError> {
    let mut nonempty_shard_roots = BTreeMap::new();
    for shard in leaves.keys().map(|key| key[0]).collect::<BTreeSet<_>>() {
        nonempty_shard_roots.insert(shard, reference_shard_root(leaves, shard, None)?.0);
    }
    let leaf_count = u64::try_from(leaves.len()).map_err(|_| SccpReplayArchiveError::Malformed)?;
    let forest = SccpReplayForestV1 {
        nonempty_shard_roots,
        leaf_count,
        update_sequence: leaf_count,
    };
    forest
        .validate()
        .map_err(|_| SccpReplayArchiveError::RebuildMismatch)?;
    Ok(forest)
}

fn occupied_leaf_hash(key: [u8; 32], record_digest: [u8; 32]) -> [u8; 32] {
    sha256(&[REPLAY_MAGIC_V1, &[0x11], &key, &record_digest])
}

fn parent_hash(level: usize, left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let level = u16::try_from(level).expect("replay tree depth fits u16");
    sha256(&[
        REPLAY_MAGIC_V1,
        &[0x12],
        &level.to_be_bytes(),
        &left,
        &right,
    ])
}

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

fn toggle_bit(mut value: [u8; 32], level: usize) -> [u8; 32] {
    value[31 - level / 8] ^= 1 << (level % 8);
    value
}

fn clear_bit(mut value: [u8; 32], level: usize) -> [u8; 32] {
    value[31 - level / 8] &= !(1 << (level % 8));
    value
}

fn set_position_bit(mut value: [u8; 32], level: usize) -> [u8; 32] {
    value[31 - level / 8] |= 1 << (level % 8);
    value
}

fn position_bit_is_set(value: &[u8; 32], level: usize) -> bool {
    value[31 - level / 8] & (1 << (level % 8)) != 0
}

fn set_bit(bitmap: &mut [u8; 32], level: usize) {
    bitmap[31 - level / 8] |= 1 << (level % 8);
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        account::AccountId,
        bridge::{
            SccpLaneIdV1, SccpNetworkV1, SccpReplayActorV1, SccpReplayBoundaryV1,
            SccpReplayPrincipalV1, SccpReplayRecordV1, SccpRouteKeyV1, sccp_replay_key_v1,
        },
    };

    use super::*;

    fn id() -> SccpReplayAccumulatorIdV1 {
        SccpReplayAccumulatorIdV1::from_domain(
            SccpRouteKeyV1::new(
                SccpLaneIdV1 {
                    source: SccpNetworkV1::EthereumMainnet,
                    target: SccpNetworkV1::SoraTaira,
                },
                "taira_eth_xor".to_owned(),
                "xor".to_owned(),
                7,
            )
            .expect("valid route key"),
            &domain(),
        )
        .expect("valid accumulator identity")
    }

    fn domain() -> SccpReplayDomainV1 {
        SccpReplayDomainV1 {
            source_network: SccpNetworkV1::SoraTaira,
            target_network: SccpNetworkV1::EthereumMainnet,
            boundary: SccpReplayBoundaryV1::SoraOutboundLock,
            route_revision: 7,
            route_configuration_hash: [0x44; 32],
            actor: SccpReplayActorV1::Route,
        }
    }

    fn record(replay_byte: u8) -> SccpReplayRecordV1 {
        let account = AccountId::new(
            KeyPair::from_seed(vec![0x77; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        SccpReplayRecordV1 {
            operation: SccpReplayBoundaryV1::SoraOutboundLock,
            replay_id: [replay_byte; 32],
            payload_sha256: [replay_byte.wrapping_add(1); 32],
            amount: u128::from(replay_byte),
            principal: SccpReplayPrincipalV1::SoraAccount(account),
            auxiliary_identity_sha256: [replay_byte.wrapping_add(2); 32],
        }
    }

    fn finality(height: u64) -> SccpReplayArchiveFinalityV1 {
        SccpReplayArchiveFinalityV1 {
            network_identity_sha256: [0x91; 32],
            finalized_height: height,
            finalized_block_hash: [height as u8; 32],
        }
    }

    fn initialized_archive() -> SccpReplayArchiveV1 {
        let mut archive = SccpReplayArchiveV1::default();
        archive
            .initialize_accumulator(id(), domain())
            .expect("valid accumulator initializes");
        archive
    }

    #[test]
    fn archive_applies_records_serves_witnesses_and_chains_snapshots() {
        let id = id();
        let domain = domain();
        let mut forest = SccpReplayForestV1::default();
        let mut archive = initialized_archive();

        let first = record(0x11);
        let first_witness = SccpSparseMerkleWitnessV1::empty_shard();
        forest
            .occupy(&domain, &first, &first_witness)
            .expect("first leaf occupies an empty shard");
        archive
            .apply_record(id.clone(), &first, &first_witness)
            .expect("archive accepts exact first record");
        assert_eq!(archive.forest(&id).expect("forest exists").1, &forest);

        let domain_hash = sccp_replay_domain_hash_v1(&domain).expect("valid domain");
        let second = record(0x12);
        let second_key = sccp_replay_key_v1(domain_hash, second.replay_id);
        let second_witness = archive.witness(&id, second_key).expect("witness is served");
        forest
            .occupy(&domain, &second, &second_witness)
            .expect("second leaf occupies against rebuilt witness");
        archive
            .apply_record(id.clone(), &second, &second_witness)
            .expect("archive accepts exact second record");

        let membership = archive
            .witness(&id, second_key)
            .expect("membership is served");
        forest
            .verify_membership(&domain, &second, &membership)
            .expect("served membership verifies in consensus code");

        let snapshot = archive
            .publish_snapshot(&id, finality(7))
            .expect("first snapshot publishes");
        let first_hash = snapshot.content_sha256().expect("snapshot hashes");
        let successor = archive
            .publish_snapshot(&id, finality(8))
            .expect("strict successor publishes");
        assert_ne!(
            successor.content_sha256().expect("successor hashes"),
            first_hash
        );

        let mut restored = SccpReplayArchiveV1::default();
        restored
            .restore_snapshot(snapshot.clone(), SccpReplayArchiveDecodeLimitsV1::default())
            .expect("first snapshot restores");
        restored
            .restore_snapshot(
                successor.clone(),
                SccpReplayArchiveDecodeLimitsV1::default(),
            )
            .expect("successor snapshot restores");
        restored
            .restore_snapshot(successor, SccpReplayArchiveDecodeLimitsV1::default())
            .expect("exact current snapshot is idempotent");
        assert_eq!(
            restored.restore_snapshot(snapshot, SccpReplayArchiveDecodeLimitsV1::default()),
            Err(SccpReplayArchiveError::SnapshotRollback)
        );
    }

    #[test]
    fn accumulator_must_be_preinitialized_with_its_complete_domain() {
        let mut archive = SccpReplayArchiveV1::default();
        let record = record(0x21);
        assert_eq!(
            archive.apply_record(id(), &record, &SccpSparseMerkleWitnessV1::empty_shard()),
            Err(SccpReplayArchiveError::UnknownAccumulator)
        );

        let mut wrong = domain();
        wrong.route_revision += 1;
        assert_eq!(
            archive.initialize_accumulator(id(), wrong),
            Err(SccpReplayArchiveError::AccumulatorDomainMismatch)
        );
    }

    #[test]
    fn record_application_failures_are_atomic() {
        let id = id();
        let domain = domain();
        let record = record(0x31);
        let stale_witness = SccpSparseMerkleWitnessV1::empty_shard();
        let mut archive = initialized_archive();
        archive
            .apply_record(id.clone(), &record, &stale_witness)
            .expect("first record applies");

        let after_success = archive.clone();
        assert_eq!(
            archive.apply_record(id.clone(), &record, &stale_witness),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );
        assert_eq!(archive, after_success);

        let domain_hash = sccp_replay_domain_hash_v1(&domain).expect("valid domain");
        let key = sccp_replay_key_v1(domain_hash, record.replay_id);
        let membership = archive.witness(&id, key).expect("membership is served");
        assert_eq!(
            archive.apply_record(id, &record, &membership),
            Err(SccpReplayArchiveError::DuplicateOrUnsortedLeaf)
        );
        assert_eq!(archive, after_success);
    }

    #[test]
    fn all_zero_key_is_archived_and_witnessed() {
        let digest = [0x51; 32];
        let mut leaves = BTreeMap::new();
        leaves.insert([0; 32], digest);
        let snapshot = SccpReplayArchiveSnapshotV1 {
            version: SNAPSHOT_VERSION_V1,
            accumulator_id: id(),
            domain: domain(),
            finality: finality(1),
            forest: reference_rebuild_forest(&leaves).expect("forest rebuilds"),
            leaves: vec![SccpReplayArchiveLeafV1 {
                key: [0; 32],
                record_digest: digest,
            }],
        };
        let mut archive = SccpReplayArchiveV1::default();
        archive
            .restore_snapshot(snapshot, SccpReplayArchiveDecodeLimitsV1::default())
            .expect("zero key is not an archive sentinel");
        let witness = archive.witness(&id(), [0; 32]).expect("witness exists");
        assert_eq!(witness.prior_record_digest, digest);
    }

    fn synthetic_leaf(shard: u8, ordinal: u64) -> ([u8; 32], [u8; 32]) {
        let mut key = [0; 32];
        key[0] = shard;
        key[8..16].copy_from_slice(&ordinal.rotate_left(17).to_be_bytes());
        key[24..].copy_from_slice(&ordinal.to_be_bytes());
        let record_digest = sha256(&[b"SCCP-REPLAY-CACHE-TEST", &key]);
        assert_ne!(record_digest, [0; 32]);
        (key, record_digest)
    }

    #[test]
    fn cached_nodes_match_reference_roots_and_witnesses() {
        let leaves = (0_u64..384)
            .map(|ordinal| synthetic_leaf(0x73, ordinal))
            .chain((0_u64..17).map(|ordinal| synthetic_leaf(0x19, ordinal)))
            .collect::<BTreeMap<_, _>>();
        let mut cached = CachedReplayTreeV1::from_leaves(leaves.clone()).expect("cache builds");
        assert_eq!(
            cached.forest().expect("cached forest validates"),
            reference_rebuild_forest(&leaves).expect("reference forest builds")
        );

        for key in [
            synthetic_leaf(0x73, 0).0,
            synthetic_leaf(0x73, 127).0,
            synthetic_leaf(0x73, 383).0,
            synthetic_leaf(0x73, 9_999).0,
            synthetic_leaf(0x19, 16).0,
            synthetic_leaf(0xA4, 1).0,
        ] {
            let cached_path = cached.witness_path(key);
            let (reference_root, reference_path) =
                reference_shard_root(&leaves, key[0], Some(key)).expect("reference witness builds");
            let (reference_bitmap, reference_siblings) =
                reference_path.expect("requested reference path exists");
            assert_eq!(cached_path.expected_shard_root, reference_root);
            assert_eq!(cached_path.sibling_bitmap, reference_bitmap);
            assert_eq!(cached_path.siblings, reference_siblings);
            assert_eq!(cached_path.node_lookups, SCCP_REPLAY_SMT_DEPTH_V1);
        }

        let (new_key, new_digest) = synthetic_leaf(0x73, 10_000);
        let plan = cached
            .plan_insert(new_key, new_digest)
            .expect("new leaf has one prospective path");
        let mut reference_after = leaves;
        reference_after.insert(new_key, new_digest);
        let expected_after =
            reference_rebuild_forest(&reference_after).expect("updated reference forest builds");
        assert_eq!(
            plan.new_root,
            expected_after
                .nonempty_shard_roots
                .get(&new_key[0])
                .copied()
                .expect("updated shard is nonempty")
        );
        cached.commit_insert(plan);
        assert_eq!(
            cached.forest().expect("updated cache validates"),
            expected_after
        );
    }

    #[test]
    fn cached_delta_and_witness_work_is_one_fixed_depth_path() {
        let populated_leaves = (0_u64..2_048)
            .map(|ordinal| synthetic_leaf(0x51, ordinal))
            .collect::<BTreeMap<_, _>>();
        let populated =
            CachedReplayTreeV1::from_leaves(populated_leaves).expect("populated cache builds");
        let empty = CachedReplayTreeV1::default();
        let (new_key, new_digest) = synthetic_leaf(0x51, 9_999);

        let empty_plan = empty
            .plan_insert(new_key, new_digest)
            .expect("empty-tree insert plans");
        let populated_plan = populated
            .plan_insert(new_key, new_digest)
            .expect("populated-tree insert plans");
        assert_eq!(empty_plan.path.len(), SCCP_REPLAY_SMT_DEPTH_V1 + 1);
        assert_eq!(
            populated_plan.path.len(),
            SCCP_REPLAY_SMT_DEPTH_V1 + 1,
            "delta work must not scale with retained leaf count"
        );
        assert_eq!(
            empty.witness_path(new_key).node_lookups,
            SCCP_REPLAY_SMT_DEPTH_V1
        );
        assert_eq!(
            populated.witness_path(new_key).node_lookups,
            SCCP_REPLAY_SMT_DEPTH_V1,
            "witness generation must read only the cached authentication path"
        );
        assert_eq!(populated.leaf_count, 2_048, "planning is mutation-free");
        assert!(!populated.contains_key(&new_key));
    }

    #[test]
    fn snapshot_limits_and_tampering_fail_without_overwrite() {
        let id = id();
        let mut archive = initialized_archive();
        let snapshot = archive
            .publish_snapshot(&id, finality(3))
            .expect("empty snapshot publishes");
        let encoded = norito::encode_canonical(&snapshot).expect("snapshot encodes");
        let mut restored = SccpReplayArchiveV1::default();
        assert_eq!(
            restored.restore_snapshot_bytes(
                &encoded,
                SccpReplayArchiveDecodeLimitsV1 {
                    max_snapshot_bytes: encoded.len() - 1,
                    max_snapshot_leaves: 1,
                }
            ),
            Err(SccpReplayArchiveError::SnapshotLimit)
        );
        restored
            .restore_snapshot_bytes(
                &encoded,
                SccpReplayArchiveDecodeLimitsV1 {
                    max_snapshot_bytes: encoded.len(),
                    max_snapshot_leaves: 1,
                },
            )
            .expect("exact bounded canonical snapshot restores");

        let over_limit_leaves =
            BTreeMap::from([([0x11; 32], [0x21; 32]), ([0x12; 32], [0x22; 32])]);
        let mut over_limit = snapshot.clone();
        over_limit.forest = reference_rebuild_forest(&over_limit_leaves).expect("forest rebuilds");
        over_limit.leaves = over_limit_leaves
            .into_iter()
            .map(|(key, record_digest)| SccpReplayArchiveLeafV1 { key, record_digest })
            .collect();
        let over_limit_encoded = norito::encode_canonical(&over_limit).expect("snapshot encodes");
        let mut bounded = SccpReplayArchiveV1::default();
        assert_eq!(
            bounded.restore_snapshot_bytes(
                &over_limit_encoded,
                SccpReplayArchiveDecodeLimitsV1 {
                    max_snapshot_bytes: over_limit_encoded.len(),
                    max_snapshot_leaves: 1,
                },
            ),
            Err(SccpReplayArchiveError::SnapshotLimit)
        );

        let mut tampered = snapshot;
        tampered.forest.leaf_count = 1;
        assert_eq!(
            restored.restore_snapshot(tampered, SccpReplayArchiveDecodeLimitsV1::default()),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );
    }

    fn replica_fixture() -> (
        SccpReplayArchiveReplicaPolicyV1,
        [KeyPair; 3],
        SccpReplayArchiveCheckpointBodyV1,
    ) {
        let pairs = [
            KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519),
            KeyPair::from_seed(vec![0x33; 32], Algorithm::Ed25519),
        ];
        let bindings = core::array::from_fn(|index| {
            let (algorithm, bytes) = pairs[index].public_key().to_bytes();
            assert_eq!(algorithm, Algorithm::Ed25519);
            SccpReplayArchiveReplicaBindingV1 {
                replica_id: [u8::try_from(index + 1).expect("small index"); 32],
                ed25519_public_key: bytes.try_into().expect("Ed25519 key is 32 bytes"),
            }
        });
        let mut archive = initialized_archive();
        let snapshot = archive
            .publish_snapshot(&id(), finality(9))
            .expect("snapshot publishes");
        (
            SccpReplayArchiveReplicaPolicyV1 { replicas: bindings },
            pairs,
            SccpReplayArchiveCheckpointBodyV1::from_snapshot(&snapshot)
                .expect("checkpoint body builds"),
        )
    }

    fn sign_checkpoint_body(
        policy: &SccpReplayArchiveReplicaPolicyV1,
        pairs: &[KeyPair; 3],
        body: SccpReplayArchiveCheckpointBodyV1,
    ) -> SccpReplayArchiveSignedCheckpointV1 {
        let message = body.signing_message().expect("checkpoint message hashes");
        SccpReplayArchiveSignedCheckpointV1 {
            body,
            attestations: core::array::from_fn(|index| {
                let signature = Signature::try_new(pairs[index].private_key(), &message)
                    .expect("checkpoint fixture signs");
                SccpReplayArchiveReplicaAttestationV1 {
                    replica_id: policy.replicas[index].replica_id,
                    signature: signature
                        .payload()
                        .try_into()
                        .expect("Ed25519 signature is 64 bytes"),
                }
            }),
        }
    }

    fn sign_checkpoint_set_body(
        policy: &SccpReplayArchiveReplicaPolicyV1,
        pairs: &[KeyPair; 3],
        body: SccpReplayArchiveCheckpointSetBodyV1,
    ) -> SccpReplayArchiveSignedCheckpointSetV1 {
        let message = body
            .signing_message()
            .expect("checkpoint-set message hashes");
        SccpReplayArchiveSignedCheckpointSetV1 {
            body,
            attestations: core::array::from_fn(|index| {
                let signature = Signature::try_new(pairs[index].private_key(), &message)
                    .expect("checkpoint-set fixture signs");
                SccpReplayArchiveReplicaAttestationV1 {
                    replica_id: policy.replicas[index].replica_id,
                    signature: signature
                        .payload()
                        .try_into()
                        .expect("Ed25519 signature is 64 bytes"),
                }
            }),
        }
    }

    fn manifest(snapshot_total_bytes: u64) -> SccpReplayArchiveSorafsManifestV1 {
        SccpReplayArchiveSorafsManifestV1 {
            manifest_sha256: [0xA1; 32],
            manifest_root_cid: {
                let mut cid = [0_u8; SCCP_REPLAY_SORAFS_MANIFEST_ROOT_CID_BYTES_V1];
                cid[..4].copy_from_slice(&[1, 0x71, 0x1f, 32]);
                cid[4..].fill(0xA2);
                cid
            },
            manifest_size_bytes: 321,
            snapshot_total_bytes,
        }
    }

    struct ResponseFixture {
        policy: SccpReplayArchiveReplicaPolicyV1,
        pairs: [KeyPair; 3],
        frame_sha256: [u8; 32],
        replay_key: [u8; 32],
        record_digest: [u8; 32],
        root: SccpReplayRootResponseV1,
        witness: SccpSparseMerkleWitnessV1,
    }

    fn response_fixture() -> ResponseFixture {
        let (policy, pairs, _) = replica_fixture();
        let mut archive = initialized_archive();
        let mut forest = SccpReplayForestV1::default();
        let record = (1_u8..=u8::MAX)
            .map(record)
            .find(|record| {
                let domain_hash =
                    sccp_replay_domain_hash_v1(&domain()).expect("fixture domain hashes");
                sccp_replay_key_v1(domain_hash, record.replay_id)[0] != 0
            })
            .expect("one deterministic replay key uses a nonzero shard");
        let delta = forest
            .occupy(
                &domain(),
                &record,
                &SccpSparseMerkleWitnessV1::empty_shard(),
            )
            .expect("fixture leaf occupies");
        archive
            .apply_record(id(), &record, &SccpSparseMerkleWitnessV1::empty_shard())
            .expect("fixture archive follows consensus forest");
        let replay_key = delta.key;
        let record_digest = delta.record_digest;
        let witness = archive
            .witness(&id(), replay_key)
            .expect("fixture membership witness exists");
        let snapshot = archive
            .publish_snapshot(&id(), finality(11))
            .expect("fixture snapshot publishes");
        let snapshot_size = u64::try_from(
            norito::encode_canonical(&snapshot)
                .expect("fixture snapshot encodes")
                .len(),
        )
        .expect("fixture snapshot length fits u64");
        let checkpoint_body = SccpReplayArchiveCheckpointBodyV1::from_snapshot(&snapshot)
            .expect("fixture checkpoint body builds");
        let checkpoint = sign_checkpoint_body(&policy, &pairs, checkpoint_body.clone());
        let entry =
            SccpReplayArchiveCheckpointSetEntryV1::from_checkpoint(&checkpoint, snapshot_size)
                .expect("fixture inventory entry builds");
        let set_body = SccpReplayArchiveCheckpointSetBodyV1::new(
            checkpoint_body.finality.into(),
            vec![entry],
            manifest(snapshot_size),
        )
        .expect("fixture set body builds");
        let signed_set = sign_checkpoint_set_body(&policy, &pairs, set_body);
        let frame_bytes =
            norito::encode_canonical(&signed_set).expect("fixture frame bytes encode");
        let frame_sha256 = sccp_replay_archive_checkpoint_set_frame_sha256_v1(&frame_bytes);
        let root = SccpReplayRootResponseV1 {
            version: 1,
            checkpoint_set_sha256: frame_sha256,
            signed_set,
            checkpoint,
        };
        ResponseFixture {
            policy,
            pairs,
            frame_sha256,
            replay_key,
            record_digest,
            root,
            witness,
        }
    }

    fn rebind_test_frame(response: &mut SccpReplayRootResponseV1) -> [u8; 32] {
        let frame_bytes =
            norito::encode_canonical(&response.signed_set).expect("substituted test frame encodes");
        let digest = sccp_replay_archive_checkpoint_set_frame_sha256_v1(&frame_bytes);
        response.checkpoint_set_sha256 = digest;
        digest
    }

    #[test]
    fn exactly_three_pinned_replica_signatures_must_agree() {
        let (policy, pairs, body) = replica_fixture();
        let message = body.signing_message().expect("message hashes");
        let attestations = core::array::from_fn(|index| {
            let signature =
                Signature::try_new(pairs[index].private_key(), &message).expect("fixture signs");
            SccpReplayArchiveReplicaAttestationV1 {
                replica_id: policy.replicas[index].replica_id,
                signature: signature
                    .payload()
                    .try_into()
                    .expect("Ed25519 signature is 64 bytes"),
            }
        });
        let checkpoint = SccpReplayArchiveSignedCheckpointV1 {
            body: body.clone(),
            attestations,
        };
        assert_eq!(
            verify_sccp_replay_archive_checkpoint_v1(&policy, &checkpoint),
            Ok(body)
        );

        let mut forged = checkpoint;
        forged.body.finality.finalized_height += 1;
        assert_eq!(
            verify_sccp_replay_archive_checkpoint_v1(&policy, &forged),
            Err(SccpReplayArchiveError::ReplicaQuorum)
        );
        let mut duplicated = forged;
        duplicated.body.finality.finalized_height -= 1;
        duplicated.attestations[1] = duplicated.attestations[0];
        assert_eq!(
            verify_sccp_replay_archive_checkpoint_v1(&policy, &duplicated),
            Err(SccpReplayArchiveError::ReplicaQuorum)
        );
    }

    #[test]
    fn checkpoint_set_frame_identity_is_byte_exact_and_domain_separated() {
        let frame = b"canonical checkpoint-set frame";
        assert_eq!(
            sccp_replay_archive_checkpoint_set_frame_sha256_v1(frame),
            sha256(&[
                SCCP_REPLAY_ARCHIVE_CHECKPOINT_SET_FRAME_SHA256_DOMAIN_V1,
                frame,
            ])
        );
        assert_eq!(
            sccp_replay_archive_checkpoint_set_frame_sha256_v1(frame),
            [
                0xd6, 0x10, 0xd4, 0xf9, 0xab, 0x69, 0xab, 0x0b, 0x57, 0x07, 0x84, 0x33, 0x34, 0xd0,
                0xbb, 0x3f, 0xe1, 0x7b, 0x24, 0x75, 0x7e, 0x7c, 0xf7, 0x4a, 0x3b, 0xfc, 0xb6, 0xd1,
                0x43, 0xed, 0x56, 0xf3,
            ]
        );
        assert_ne!(
            sccp_replay_archive_checkpoint_set_frame_sha256_v1(frame),
            sha256(&[frame])
        );
        assert_ne!(
            sccp_replay_archive_checkpoint_set_frame_sha256_v1(frame),
            sccp_replay_archive_checkpoint_set_frame_sha256_v1(b"canonical checkpoint-set frame\0",)
        );
    }

    #[test]
    fn public_root_response_cross_binds_frame_set_checkpoint_and_forest() {
        let fixture = response_fixture();
        let expected = fixture.root.checkpoint.body.clone();
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                &fixture.root,
            ),
            Ok(expected)
        );

        let mut wrong_version = fixture.root.clone();
        wrong_version.version = 2;
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                &wrong_version,
            ),
            Err(SccpReplayArchiveError::Malformed)
        );
        let mut wrong_frame = fixture.root.clone();
        wrong_frame.checkpoint_set_sha256[0] ^= 1;
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                &wrong_frame,
            ),
            Err(SccpReplayArchiveError::Malformed)
        );
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                [0; 32],
                &fixture.root.signed_set,
                &id(),
                &fixture.root,
            ),
            Err(SccpReplayArchiveError::Malformed)
        );

        let mut mismatched_expected_set = fixture.root.signed_set.clone();
        mismatched_expected_set.attestations[0].signature[0] ^= 1;
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &mismatched_expected_set,
                &id(),
                &fixture.root,
            ),
            Err(SccpReplayArchiveError::Malformed),
            "the response cannot self-label a different signed set with the expected frame hash"
        );

        let mut wrong_set_version = fixture.root.clone();
        wrong_set_version.signed_set.body.version = 2;
        let wrong_set_frame = rebind_test_frame(&mut wrong_set_version);
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                wrong_set_frame,
                &wrong_set_version.signed_set,
                &id(),
                &wrong_set_version,
            ),
            Err(SccpReplayArchiveError::Malformed)
        );
        let mut wrong_checkpoint_version = fixture.root.clone();
        wrong_checkpoint_version.checkpoint.body.version = 2;
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                &wrong_checkpoint_version,
            ),
            Err(SccpReplayArchiveError::Malformed)
        );

        let mut other_id = id();
        other_id.route_key = SccpRouteKeyV1::new(
            other_id.route_key.lane_id,
            "different_route".to_owned(),
            other_id.route_key.asset_key.clone(),
            other_id.route_key.revision,
        )
        .expect("alternate expected accumulator is valid");
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &other_id,
                &fixture.root,
            ),
            Err(SccpReplayArchiveError::AccumulatorDomainMismatch)
        );

        let mut forged_set_signature = fixture.root.clone();
        forged_set_signature.signed_set.attestations[0].signature[0] ^= 1;
        let forged_set_frame = rebind_test_frame(&mut forged_set_signature);
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                forged_set_frame,
                &forged_set_signature.signed_set,
                &id(),
                &forged_set_signature,
            ),
            Err(SccpReplayArchiveError::ReplicaQuorum)
        );
        let mut forged_checkpoint_signature = fixture.root.clone();
        forged_checkpoint_signature.checkpoint.attestations[0].signature[0] ^= 1;
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                &forged_checkpoint_signature,
            ),
            Err(SccpReplayArchiveError::ReplicaQuorum)
        );

        let mut root_substitution = fixture.root.clone();
        root_substitution
            .checkpoint
            .body
            .forest
            .nonempty_shard_roots
            .values_mut()
            .next()
            .expect("fixture forest is occupied")[0] ^= 1;
        root_substitution.checkpoint = sign_checkpoint_body(
            &fixture.policy,
            &fixture.pairs,
            root_substitution.checkpoint.body,
        );
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                &root_substitution,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch),
            "a re-signed root must still match the selected inventory entry"
        );

        let mut leaf_count = fixture.root.clone();
        leaf_count.checkpoint.body.forest.leaf_count += 1;
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                &leaf_count,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );
        let mut update_sequence = fixture.root.clone();
        update_sequence.checkpoint.body.forest.update_sequence += 1;
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                &update_sequence,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );

        let mut snapshot_substitution = fixture.root.clone();
        let mut entries = snapshot_substitution.signed_set.body.entries.clone();
        entries[0].snapshot_sha256[0] ^= 1;
        let set_body = SccpReplayArchiveCheckpointSetBodyV1::new(
            snapshot_substitution.signed_set.body.finality,
            entries,
            snapshot_substitution.signed_set.body.sorafs_manifest,
        )
        .expect("substituted snapshot inventory remains structurally valid");
        snapshot_substitution.signed_set =
            sign_checkpoint_set_body(&fixture.policy, &fixture.pairs, set_body);
        let snapshot_frame = rebind_test_frame(&mut snapshot_substitution);
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                snapshot_frame,
                &snapshot_substitution.signed_set,
                &id(),
                &snapshot_substitution,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );

        let mut agreement_substitution = fixture.root.clone();
        let mut entries = agreement_substitution.signed_set.body.entries.clone();
        entries[0].checkpoint_agreement_digest[0] ^= 1;
        let set_body = SccpReplayArchiveCheckpointSetBodyV1::new(
            agreement_substitution.signed_set.body.finality,
            entries,
            agreement_substitution.signed_set.body.sorafs_manifest,
        )
        .expect("substituted checkpoint inventory remains structurally valid");
        agreement_substitution.signed_set =
            sign_checkpoint_set_body(&fixture.policy, &fixture.pairs, set_body);
        let agreement_frame = rebind_test_frame(&mut agreement_substitution);
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                agreement_frame,
                &agreement_substitution.signed_set,
                &id(),
                &agreement_substitution,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );

        let mut finality_substitution = fixture.root.clone();
        let original_set = &finality_substitution.signed_set.body;
        let mut set_finality = original_set.finality;
        set_finality.finalized_height += 1;
        set_finality.finalized_block_hash[0] ^= 1;
        let set_body = SccpReplayArchiveCheckpointSetBodyV1::new(
            set_finality,
            original_set.entries.clone(),
            original_set.sorafs_manifest,
        )
        .expect("substituted common finality remains structurally valid");
        finality_substitution.signed_set =
            sign_checkpoint_set_body(&fixture.policy, &fixture.pairs, set_body);
        let finality_frame = rebind_test_frame(&mut finality_substitution);
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                finality_frame,
                &finality_substitution.signed_set,
                &id(),
                &finality_substitution,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );

        let mut omitted = fixture.root.clone();
        let empty_body = SccpReplayArchiveCheckpointSetBodyV1::new(
            omitted.signed_set.body.finality,
            Vec::new(),
            manifest(0),
        )
        .expect("signed empty inventory is independently valid");
        omitted.signed_set = sign_checkpoint_set_body(&fixture.policy, &fixture.pairs, empty_body);
        let omitted_frame = rebind_test_frame(&mut omitted);
        assert_eq!(
            verify_sccp_replay_root_response_v1(
                &fixture.policy,
                omitted_frame,
                &omitted.signed_set,
                &id(),
                &omitted,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );
    }

    #[test]
    fn public_witness_response_cross_binds_key_digest_path_and_signed_root() {
        let fixture = response_fixture();
        let response = SccpReplayWitnessResponseV1 {
            version: 1,
            root: fixture.root.clone(),
            replay_key: fixture.replay_key,
            witness: fixture.witness.clone(),
        };
        let verified = verify_sccp_replay_witness_response_v1(
            &fixture.policy,
            fixture.frame_sha256,
            &fixture.root.signed_set,
            &id(),
            fixture.replay_key,
            &response,
        )
        .expect("canonical membership response verifies");
        assert_eq!(verified.0, fixture.root.checkpoint.body.clone());
        assert_eq!(verified.1, fixture.record_digest);

        let nonmembership = SccpReplayWitnessResponseV1 {
            version: 1,
            root: fixture.root.clone(),
            replay_key: [0; 32],
            witness: SccpSparseMerkleWitnessV1::empty_shard(),
        };
        assert_eq!(
            verify_sccp_replay_witness_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                [0; 32],
                &nonmembership,
            )
            .map(|(_, digest)| digest),
            Ok([0; 32]),
            "the all-zero replay key is ordinary canonical non-membership"
        );

        let mut wrong_version = response.clone();
        wrong_version.version = 2;
        assert_eq!(
            verify_sccp_replay_witness_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                fixture.replay_key,
                &wrong_version,
            ),
            Err(SccpReplayArchiveError::Malformed)
        );

        let mut caller_key = fixture.replay_key;
        caller_key[31] ^= 1;
        assert_eq!(
            verify_sccp_replay_witness_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                caller_key,
                &response,
            ),
            Err(SccpReplayArchiveError::Malformed)
        );
        let mut key_substitution = response.clone();
        key_substitution.replay_key[31] ^= 1;
        assert_eq!(
            verify_sccp_replay_witness_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                key_substitution.replay_key,
                &key_substitution,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch),
            "changing both expected and returned key cannot reuse another path"
        );

        let mut root_substitution = response.clone();
        root_substitution.witness.expected_shard_root[0] ^= 1;
        assert_eq!(
            verify_sccp_replay_witness_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                fixture.replay_key,
                &root_substitution,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );
        let mut digest_substitution = response.clone();
        digest_substitution.witness.prior_record_digest[0] ^= 1;
        assert_eq!(
            verify_sccp_replay_witness_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                fixture.replay_key,
                &digest_substitution,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );
        let mut reserved_bitmap = response.clone();
        reserved_bitmap.witness.sibling_bitmap[0] = 1;
        assert_eq!(
            verify_sccp_replay_witness_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                fixture.replay_key,
                &reserved_bitmap,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );
        let mut wrong_count = response.clone();
        wrong_count.witness.sibling_bitmap[31] = 1;
        assert_eq!(
            verify_sccp_replay_witness_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                fixture.replay_key,
                &wrong_count,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );
        let mut wrong_sibling = response;
        wrong_sibling.witness.sibling_bitmap[31] = 1;
        wrong_sibling.witness.siblings.push([0xD1; 32]);
        assert_eq!(
            verify_sccp_replay_witness_response_v1(
                &fixture.policy,
                fixture.frame_sha256,
                &fixture.root.signed_set,
                &id(),
                fixture.replay_key,
                &wrong_sibling,
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );
    }

    #[test]
    fn complete_inventory_signature_covers_empty_and_nonempty_sets() {
        let (policy, pairs, checkpoint_body) = replica_fixture();
        let checkpoint_message = checkpoint_body.signing_message().expect("message hashes");
        let checkpoint = SccpReplayArchiveSignedCheckpointV1 {
            body: checkpoint_body.clone(),
            attestations: core::array::from_fn(|index| {
                let signature = Signature::try_new(pairs[index].private_key(), &checkpoint_message)
                    .expect("checkpoint signs");
                SccpReplayArchiveReplicaAttestationV1 {
                    replica_id: policy.replicas[index].replica_id,
                    signature: signature
                        .payload()
                        .try_into()
                        .expect("Ed25519 signature is 64 bytes"),
                }
            }),
        };
        let entry = SccpReplayArchiveCheckpointSetEntryV1::from_checkpoint(&checkpoint, 123)
            .expect("inventory entry derives");
        let finality = checkpoint_body.finality.into();
        let manifest = SccpReplayArchiveSorafsManifestV1 {
            manifest_sha256: [0xA1; 32],
            manifest_root_cid: {
                let mut cid = [0_u8; SCCP_REPLAY_SORAFS_MANIFEST_ROOT_CID_BYTES_V1];
                cid[..4].copy_from_slice(&[1, 0x71, 0x1f, 32]);
                cid[4..].fill(0xA2);
                cid
            },
            manifest_size_bytes: 321,
            snapshot_total_bytes: 123,
        };
        let body = SccpReplayArchiveCheckpointSetBodyV1::new(finality, vec![entry], manifest)
            .expect("complete inventory builds");
        let message = body.signing_message().expect("set message hashes");
        let signed = SccpReplayArchiveSignedCheckpointSetV1 {
            body: body.clone(),
            attestations: core::array::from_fn(|index| {
                let signature =
                    Signature::try_new(pairs[index].private_key(), &message).expect("set signs");
                SccpReplayArchiveReplicaAttestationV1 {
                    replica_id: policy.replicas[index].replica_id,
                    signature: signature
                        .payload()
                        .try_into()
                        .expect("Ed25519 signature is 64 bytes"),
                }
            }),
        };
        assert_eq!(
            verify_sccp_replay_archive_checkpoint_set_v1(&policy, &signed),
            Ok(body)
        );

        let empty_manifest = SccpReplayArchiveSorafsManifestV1 {
            snapshot_total_bytes: 0,
            ..manifest
        };
        let empty_body =
            SccpReplayArchiveCheckpointSetBodyV1::new(finality, Vec::new(), empty_manifest)
                .expect("empty complete inventory builds");
        let empty_message = empty_body.signing_message().expect("empty set hashes");
        let empty_signed = SccpReplayArchiveSignedCheckpointSetV1 {
            body: empty_body.clone(),
            attestations: core::array::from_fn(|index| {
                let signature = Signature::try_new(pairs[index].private_key(), &empty_message)
                    .expect("empty set signs");
                SccpReplayArchiveReplicaAttestationV1 {
                    replica_id: policy.replicas[index].replica_id,
                    signature: signature
                        .payload()
                        .try_into()
                        .expect("Ed25519 signature is 64 bytes"),
                }
            }),
        };
        assert_eq!(
            verify_sccp_replay_archive_checkpoint_set_v1(&policy, &empty_signed),
            Ok(empty_body)
        );

        let mut forged = signed;
        forged.body.entries.clear();
        forged.body.entry_count = 0;
        forged.body.sorafs_manifest.snapshot_total_bytes = 0;
        forged.body.inventory_sha256 =
            checkpoint_set_inventory_sha256(finality, &[]).expect("empty inventory hashes");
        assert_eq!(
            verify_sccp_replay_archive_checkpoint_set_v1(&policy, &forged),
            Err(SccpReplayArchiveError::ReplicaQuorum),
            "a correctly rehashed omission still lacks the three signatures"
        );
    }
    #[test]
    fn snapshot_and_checkpoint_canonical_bytes_ignore_ambient_layout() {
        let mut archive = initialized_archive();
        let snapshot = archive
            .publish_snapshot(&id(), finality(3))
            .expect("empty snapshot publishes");
        let body = SccpReplayArchiveCheckpointBodyV1::from_snapshot(&snapshot)
            .expect("checkpoint body builds");
        let canonical_snapshot =
            norito::encode_canonical(&snapshot).expect("snapshot canonically encodes");
        let canonical_body =
            norito::encode_canonical(&body).expect("checkpoint body canonically encodes");
        let canonical_content_sha256 = sha256(&[&canonical_snapshot]);
        let canonical_body_len =
            u64::try_from(canonical_body.len()).expect("fixture length fits u64");
        let canonical_agreement = sha256(&[
            REPLICA_AGREEMENT_DOMAIN_V1,
            &canonical_body_len.to_be_bytes(),
            &canonical_body,
        ]);

        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        let alternate_snapshot =
            norito::to_bytes(&snapshot).expect("alternate-layout snapshot encodes");
        let alternate_body =
            norito::to_bytes(&body).expect("alternate-layout checkpoint body encodes");

        assert_ne!(alternate_snapshot, canonical_snapshot);
        assert_ne!(alternate_body, canonical_body);
        assert_eq!(snapshot.content_sha256(), Ok(canonical_content_sha256));
        assert_eq!(body.agreement_digest(), Ok(canonical_agreement));
        assert_eq!(
            SccpReplayArchiveCheckpointBodyV1::from_snapshot(&snapshot),
            Ok(body)
        );
        assert_eq!(
            decode_sccp_replay_archive_snapshot_v1(
                &canonical_snapshot,
                SccpReplayArchiveDecodeLimitsV1::default()
            ),
            Ok(snapshot.clone())
        );
        assert_eq!(
            decode_sccp_replay_archive_snapshot_v1(
                &alternate_snapshot,
                SccpReplayArchiveDecodeLimitsV1::default()
            ),
            Err(SccpReplayArchiveError::Malformed)
        );
    }

    #[test]
    fn fully_signed_zero_snapshot_content_hash_is_malformed() {
        let (policy, pairs, mut body) = replica_fixture();
        body.snapshot_sha256 = [0; 32];

        // Sign the raw agreement statement so the rejection cannot be caused
        // by missing, mismatched, or forged attestations.
        let encoded = norito::encode_canonical(&body).expect("checkpoint body canonically encodes");
        let encoded_len = u64::try_from(encoded.len()).expect("fixture length fits u64");
        let agreement = sha256(&[
            REPLICA_AGREEMENT_DOMAIN_V1,
            &encoded_len.to_be_bytes(),
            &encoded,
        ]);
        let message = sha256(&[CHECKPOINT_SIGNATURE_DOMAIN_V1, &agreement]);
        let checkpoint = SccpReplayArchiveSignedCheckpointV1 {
            body,
            attestations: attestations_for_message(&policy, &pairs, message),
        };

        assert_eq!(
            verify_sccp_replay_archive_checkpoint_v1(&policy, &checkpoint),
            Err(SccpReplayArchiveError::Malformed)
        );
    }

    fn attestations_for_message(
        policy: &SccpReplayArchiveReplicaPolicyV1,
        pairs: &[KeyPair; 3],
        message: [u8; 32],
    ) -> [SccpReplayArchiveReplicaAttestationV1; 3] {
        core::array::from_fn(|index| {
            let signature =
                Signature::try_new(pairs[index].private_key(), &message).expect("fixture signs");
            SccpReplayArchiveReplicaAttestationV1 {
                replica_id: policy.replicas[index].replica_id,
                signature: signature
                    .payload()
                    .try_into()
                    .expect("Ed25519 signature is 64 bytes"),
            }
        })
    }
}

#[cfg(test)]
#[test]
fn captured_checkpoint_inventory_frame_identity() {
    crate::frame_identity_tests::assert_serialize::<SccpReplayArchiveCheckpointInventoryV1>(
        "iroha_sccp::replay_archive::SccpReplayArchiveCheckpointInventoryV1",
    );
}
