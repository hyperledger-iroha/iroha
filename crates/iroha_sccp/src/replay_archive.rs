//! Untrusted, independently rebuildable SCCP sparse-replay archive.
//!
//! Consensus retains only the constant-size forest projection. Archive replicas
//! retain sorted leaves, rebuild every root, and serve canonical compressed
//! witnesses. Replica signatures establish availability provenance, never a
//! substitute safety boundary: every response remains locally verifiable.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use iroha_data_model::NetworkId;
use iroha_data_model::bridge::{
    SCCP_REPLAY_SMT_DEPTH_V1, SCCP_REPLAY_SMT_SHARD_COUNT_V1, SccpNetworkV1,
    SccpReplayAccumulatorError, SccpReplayAccumulatorIdV1, SccpReplayDomainV1, SccpReplayForestV1,
    SccpReplayRecordV1, SccpSparseMerkleWitnessV1, sccp_replay_domain_hash_v1,
    sccp_replay_empty_hashes_v1,
};
use norito::codec::{Decode, Encode};
use sha2::{Digest as _, Sha256};

const SNAPSHOT_VERSION_V1: u8 = 1;
const CHECKPOINT_VERSION_V1: u8 = 1;
const REPLAY_MAGIC_V1: &[u8; 18] = b"SCCP-REPLAY-SMT-V1";
const CHECKPOINT_SIGNATURE_DOMAIN_V1: &[u8] = b"SCCP-REPLAY-CHECKPOINT-SIGNATURE-V1";
const REPLICA_AGREEMENT_DOMAIN_V1: &[u8] = b"SCCP-REPLAY-REPLICA-AGREEMENT-V1";

/// Default maximum number of leaves admitted by one decoded snapshot.
pub const SCCP_REPLAY_ARCHIVE_DEFAULT_MAX_SNAPSHOT_LEAVES_V1: usize = 256 * 1024;
/// Default maximum complete encoded snapshot held by the in-memory validator.
pub const SCCP_REPLAY_ARCHIVE_DEFAULT_MAX_SNAPSHOT_BYTES_V1: usize = 32 * 1024 * 1024;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub struct SccpReplayArchiveFinalityV1 {
    /// SHA-256 of the exact canonical network identity.
    pub network_identity_sha256: [u8; 32],
    /// Nonzero finalized carrier height.
    pub finalized_height: u64,
    /// Exact finalized carrier block hash.
    pub finalized_block_hash: [u8; 32],
    /// Content hash of the previous snapshot for this accumulator, or zero for
    /// its first snapshot.
    pub predecessor_snapshot_sha256: [u8; 32],
}

impl SccpReplayArchiveFinalityV1 {
    fn is_well_formed(self) -> bool {
        self.network_identity_sha256 != [0; 32]
            && self.finalized_height != 0
            && self.finalized_block_hash != [0; 32]
    }
}

/// One sorted key/digest pair retained outside the consensus safety boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub struct SccpReplayArchiveLeafV1 {
    /// Complete replay key; byte zero selects its shard. Every 256-bit value is
    /// valid, including zero.
    pub key: [u8; 32],
    /// Canonical occupied-record digest.
    pub record_digest: [u8; 32],
}

/// Deterministic, content-addressable replay snapshot suitable for SoraFS.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub struct SccpReplayArchiveSnapshotV1 {
    /// Snapshot schema version. Final V1 requires exactly one.
    pub version: u8,
    /// Route and boundary whose leaves are captured.
    pub accumulator_id: SccpReplayAccumulatorIdV1,
    /// Complete validated replay domain, not only a caller-supplied hash.
    pub domain: SccpReplayDomainV1,
    /// Exact finalized chain coordinate and predecessor snapshot binding.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
pub struct SccpReplayArchiveReplicaBindingV1 {
    /// Stable nonzero replica identity assigned by release policy.
    pub replica_id: [u8; 32],
    /// Exact canonical Ed25519 public key.
    pub ed25519_public_key: [u8; 32],
}

/// Exact three-replica release policy used to authenticate checkpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode)]
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
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
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

/// One replica's exact detached Ed25519 attestation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode)]
pub struct SccpReplayArchiveReplicaAttestationV1 {
    /// Replica identity selecting one pinned release-policy key.
    pub replica_id: [u8; 32],
    /// Detached Ed25519 signature over the domain-separated agreement digest.
    pub signature: [u8; 64],
}

/// Exactly three matching, independently signed replica checkpoints.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode)]
pub struct SccpReplayArchiveSignedCheckpointV1 {
    /// Common statement agreed by every replica.
    pub body: SccpReplayArchiveCheckpointBodyV1,
    /// Attestations in the exact same order as the pinned policy.
    pub attestations: [SccpReplayArchiveReplicaAttestationV1; 3],
}

/// Replay archive validation failure. No variant retains attacker-controlled
/// paths, payloads, keys, signatures, or parser details.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SccpReplayArchiveError {
    /// Record, witness, snapshot, checkpoint, or canonical framing is malformed.
    Malformed,
    /// A key was already occupied or leaves were not strictly ordered.
    DuplicateOrUnsortedLeaf,
    /// An authenticated transition or rebuilt state disagrees with stored state.
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
struct AccumulatorArchiveV1 {
    domain: SccpReplayDomainV1,
    domain_hash: [u8; 32],
    leaves: BTreeMap<[u8; 32], [u8; 32]>,
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
                leaves: BTreeMap::new(),
                forest: SccpReplayForestV1::default(),
                snapshot_head: None,
            },
        );
        Ok(())
    }

    /// Apply one replay record against its authenticated consensus witness.
    ///
    /// All fallible work occurs against a bounded forest clone. The archive
    /// commits exactly one leaf insertion and the resulting forest together.
    pub fn apply_record(
        &mut self,
        accumulator_id: SccpReplayAccumulatorIdV1,
        record: &SccpReplayRecordV1,
        witness: &SccpSparseMerkleWitnessV1,
    ) -> Result<(), SccpReplayArchiveError> {
        let existing = self
            .accumulators
            .get_mut(&accumulator_id)
            .ok_or(SccpReplayArchiveError::UnknownAccumulator)?;

        if u64::try_from(existing.leaves.len()).ok() != Some(existing.forest.leaf_count) {
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

        match existing.leaves.entry(delta.key) {
            Entry::Occupied(_) => Err(SccpReplayArchiveError::DuplicateOrUnsortedLeaf),
            Entry::Vacant(entry) => {
                entry.insert(delta.record_digest);
                existing.forest = next_forest;
                Ok(())
            }
        }
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
        let (expected_shard_root, path) = shard_root(&accumulator.leaves, key[0], Some(key))?;
        let (sibling_bitmap, siblings) = path.ok_or(SccpReplayArchiveError::RebuildMismatch)?;
        let witness = SccpSparseMerkleWitnessV1 {
            expected_shard_root,
            prior_record_digest: accumulator.leaves.get(&key).copied().unwrap_or([0; 32]),
            sibling_bitmap,
            siblings,
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
            leaves: archive
                .leaves
                .iter()
                .map(|(key, record_digest)| SccpReplayArchiveLeafV1 {
                    key: *key,
                    record_digest: *record_digest,
                })
                .collect(),
        };
        let validated = validate_snapshot(&snapshot, SccpReplayArchiveDecodeLimitsV1::default())?;
        let head = SnapshotHeadV1 {
            content_sha256: validated.content_sha256,
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
            leaves,
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
                return (existing.forest == snapshot.forest && existing.leaves == leaves)
                    .then_some(())
                    .ok_or(SccpReplayArchiveError::SnapshotRollback);
            }
            validate_snapshot_successor(existing, snapshot.finality)?;
            if existing
                .leaves
                .iter()
                .any(|(key, digest)| leaves.get(key) != Some(digest))
                || snapshot.forest.leaf_count < existing.forest.leaf_count
                || snapshot.forest.update_sequence < existing.forest.update_sequence
            {
                return Err(SccpReplayArchiveError::SnapshotRollback);
            }
        } else if snapshot.finality.predecessor_snapshot_sha256 != [0; 32] {
            return Err(SccpReplayArchiveError::SnapshotRollback);
        }
        let domain_hash = sccp_replay_domain_hash_v1(&snapshot.domain)
            .map_err(|_| SccpReplayArchiveError::AccumulatorDomainMismatch)?;
        self.accumulators.insert(
            snapshot.accumulator_id,
            AccumulatorArchiveV1 {
                domain: snapshot.domain,
                domain_hash,
                leaves,
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
        leaves: validate_snapshot_leaves(&snapshot)?,
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
    for ((binding, attestation), index) in policy
        .replicas
        .iter()
        .zip(checkpoint.attestations.iter())
        .zip(0_usize..)
    {
        if binding.replica_id != attestation.replica_id
            || checkpoint.attestations[..index]
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
    Ok(checkpoint.body.clone())
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

fn validate_accumulator_domain(
    accumulator_id: &SccpReplayAccumulatorIdV1,
    domain: &SccpReplayDomainV1,
) -> Result<(), SccpReplayArchiveError> {
    accumulator_id
        .validate_domain(domain)
        .map_err(|_| SccpReplayArchiveError::AccumulatorDomainMismatch)
}

struct ValidatedSnapshotV1 {
    leaves: BTreeMap<[u8; 32], [u8; 32]>,
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
        leaves: validate_snapshot_leaves(snapshot)?,
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
) -> Result<BTreeMap<[u8; 32], [u8; 32]>, SccpReplayArchiveError> {
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
    let rebuilt = rebuild_forest(&leaves)?;
    if rebuilt != snapshot.forest {
        return Err(SccpReplayArchiveError::RebuildMismatch);
    }
    Ok(leaves)
}

fn validate_snapshot_successor(
    archive: &AccumulatorArchiveV1,
    finality: SccpReplayArchiveFinalityV1,
) -> Result<(), SccpReplayArchiveError> {
    if !finality.is_well_formed() {
        return Err(SccpReplayArchiveError::Malformed);
    }
    match &archive.snapshot_head {
        None if finality.predecessor_snapshot_sha256 == [0; 32] => Ok(()),
        None => Err(SccpReplayArchiveError::SnapshotRollback),
        Some(head)
            if finality.predecessor_snapshot_sha256 == head.content_sha256
                && finality.network_identity_sha256 == head.finality.network_identity_sha256
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

type WitnessPathV1 = ([u8; 32], Vec<[u8; 32]>);

fn shard_root(
    leaves: &BTreeMap<[u8; 32], [u8; 32]>,
    shard: u8,
    witness_key: Option<[u8; 32]>,
) -> Result<([u8; 32], Option<WitnessPathV1>), SccpReplayArchiveError> {
    if witness_key.is_some_and(|key| key[0] != shard) {
        return Err(SccpReplayArchiveError::Malformed);
    }
    let empty = sccp_replay_empty_hashes_v1();
    let mut first = [0_u8; 32];
    first[0] = shard;
    let mut last = [u8::MAX; 32];
    last[0] = shard;
    let mut nodes = leaves
        .range(first..=last)
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

fn rebuild_forest(
    leaves: &BTreeMap<[u8; 32], [u8; 32]>,
) -> Result<SccpReplayForestV1, SccpReplayArchiveError> {
    let mut nonempty_shard_roots = BTreeMap::new();
    for shard in leaves.keys().map(|key| key[0]).collect::<BTreeSet<_>>() {
        nonempty_shard_roots.insert(shard, shard_root(leaves, shard, None)?.0);
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

fn set_bit(bitmap: &mut [u8; 32], level: usize) {
    bitmap[31 - level / 8] |= 1 << (level % 8);
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::bridge::{
        SccpLaneIdV1, SccpReplayActorV1, SccpReplayBoundaryV1, SccpReplayPrincipalV1,
        SccpRouteKeyV1, sccp_replay_key_v1,
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
        SccpReplayRecordV1 {
            operation: SccpReplayBoundaryV1::SoraOutboundLock,
            replay_id: [replay_byte; 32],
            payload_sha256: [replay_byte.wrapping_add(1); 32],
            amount: u128::from(replay_byte),
            principal: SccpReplayPrincipalV1::Evm([0x33; 20]),
            auxiliary_identity_sha256: [replay_byte.wrapping_add(2); 32],
        }
    }

    fn finality(height: u64, predecessor_snapshot_sha256: [u8; 32]) -> SccpReplayArchiveFinalityV1 {
        SccpReplayArchiveFinalityV1 {
            network_identity_sha256: [0x91; 32],
            finalized_height: height,
            finalized_block_hash: [height as u8; 32],
            predecessor_snapshot_sha256,
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
            .publish_snapshot(&id, finality(7, [0; 32]))
            .expect("first snapshot publishes");
        let first_hash = snapshot.content_sha256().expect("snapshot hashes");
        let successor = archive
            .publish_snapshot(&id, finality(8, first_hash))
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
    fn all_zero_key_is_archived_and_witnessed() {
        let digest = [0x51; 32];
        let mut leaves = BTreeMap::new();
        leaves.insert([0; 32], digest);
        let snapshot = SccpReplayArchiveSnapshotV1 {
            version: SNAPSHOT_VERSION_V1,
            accumulator_id: id(),
            domain: domain(),
            finality: finality(1, [0; 32]),
            forest: rebuild_forest(&leaves).expect("forest rebuilds"),
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
    fn snapshot_and_checkpoint_canonical_bytes_ignore_ambient_layout() {
        let mut archive = initialized_archive();
        let snapshot = archive
            .publish_snapshot(&id(), finality(3, [0; 32]))
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
    fn snapshot_limits_and_tampering_fail_without_overwrite() {
        let id = id();
        let mut archive = initialized_archive();
        let snapshot = archive
            .publish_snapshot(&id, finality(3, [0; 32]))
            .expect("empty snapshot publishes");
        let encoded = norito::encode_canonical(&snapshot).expect("snapshot canonically encodes");
        let below_encoded_limit = SccpReplayArchiveDecodeLimitsV1 {
            max_snapshot_bytes: encoded.len() - 1,
            max_snapshot_leaves: 1,
        };
        let exact_encoded_limit = SccpReplayArchiveDecodeLimitsV1 {
            max_snapshot_bytes: encoded.len(),
            max_snapshot_leaves: 1,
        };
        assert_eq!(
            decode_sccp_replay_archive_snapshot_v1(&[], exact_encoded_limit),
            Err(SccpReplayArchiveError::Malformed)
        );
        let (_, decoded_validation) =
            decode_validated_snapshot(&encoded, exact_encoded_limit).expect("snapshot validates");
        assert_eq!(decoded_validation.content_sha256, sha256(&[&encoded]));
        assert_eq!(
            decoded_validation.content_sha256,
            snapshot.content_sha256().expect("typed snapshot hashes")
        );
        let mut typed_restored = SccpReplayArchiveV1::default();
        assert_eq!(
            typed_restored.restore_snapshot(snapshot.clone(), below_encoded_limit),
            Err(SccpReplayArchiveError::SnapshotLimit)
        );
        typed_restored
            .restore_snapshot(snapshot.clone(), exact_encoded_limit)
            .expect("typed snapshot at the exact byte limit restores");
        let mut mismatched_encoding = encoded.clone();
        mismatched_encoding[0] ^= 1;
        assert!(matches!(
            decode_sccp_replay_archive_snapshot_v1(&mismatched_encoding, exact_encoded_limit),
            Err(SccpReplayArchiveError::Malformed)
        ));

        let mut restored = SccpReplayArchiveV1::default();
        assert_eq!(
            restored.restore_snapshot_bytes(&encoded, below_encoded_limit),
            Err(SccpReplayArchiveError::SnapshotLimit)
        );
        restored
            .restore_snapshot_bytes(&encoded, exact_encoded_limit)
            .expect("exact bounded canonical snapshot restores");
        restored
            .publish_snapshot(&id, finality(4, sha256(&[&encoded])))
            .expect("byte-derived content hash chains the next snapshot");

        let mut wrong_version = snapshot.clone();
        wrong_version.version = 2;
        assert_eq!(
            restored.restore_snapshot(wrong_version, exact_encoded_limit),
            Err(SccpReplayArchiveError::Malformed)
        );
        let mut zero_finality = snapshot.clone();
        zero_finality.finality.finalized_height = 0;
        assert_eq!(
            restored.restore_snapshot(zero_finality, exact_encoded_limit),
            Err(SccpReplayArchiveError::Malformed)
        );

        let zero_digest_leaves = BTreeMap::from([([0x31; 32], [0; 32])]);
        let mut zero_digest = snapshot.clone();
        zero_digest.forest = rebuild_forest(&zero_digest_leaves).expect("forest rebuilds");
        zero_digest.leaves = vec![SccpReplayArchiveLeafV1 {
            key: [0x31; 32],
            record_digest: [0; 32],
        }];
        let zero_digest_bytes =
            norito::encode_canonical(&zero_digest).expect("malformed snapshot encodes");
        assert_eq!(
            decode_sccp_replay_archive_snapshot_v1(
                &zero_digest_bytes,
                SccpReplayArchiveDecodeLimitsV1 {
                    max_snapshot_bytes: zero_digest_bytes.len(),
                    max_snapshot_leaves: 1,
                }
            ),
            Err(SccpReplayArchiveError::DuplicateOrUnsortedLeaf)
        );

        let unsorted_leaves = BTreeMap::from([([0x41; 32], [0x51; 32]), ([0x42; 32], [0x52; 32])]);
        let mut unsorted = snapshot.clone();
        unsorted.forest = rebuild_forest(&unsorted_leaves).expect("forest rebuilds");
        unsorted.leaves = unsorted_leaves
            .iter()
            .rev()
            .map(|(key, record_digest)| SccpReplayArchiveLeafV1 {
                key: *key,
                record_digest: *record_digest,
            })
            .collect();
        let unsorted_bytes =
            norito::encode_canonical(&unsorted).expect("unsorted snapshot encodes");
        assert_eq!(
            decode_sccp_replay_archive_snapshot_v1(
                &unsorted_bytes,
                SccpReplayArchiveDecodeLimitsV1 {
                    max_snapshot_bytes: unsorted_bytes.len(),
                    max_snapshot_leaves: 2,
                }
            ),
            Err(SccpReplayArchiveError::DuplicateOrUnsortedLeaf)
        );

        let over_limit_leaves =
            BTreeMap::from([([0x11; 32], [0x21; 32]), ([0x12; 32], [0x22; 32])]);
        let mut over_limit = snapshot.clone();
        over_limit.forest = rebuild_forest(&over_limit_leaves).expect("forest rebuilds");
        over_limit.leaves = over_limit_leaves
            .into_iter()
            .map(|(key, record_digest)| SccpReplayArchiveLeafV1 { key, record_digest })
            .collect();
        let over_limit_encoded =
            norito::encode_canonical(&over_limit).expect("snapshot canonically encodes");
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

        let mut allocation_bomb = snapshot.clone();
        allocation_bomb.leaves = (0_u16..257)
            .map(|index| {
                let mut key = [0; 32];
                key[30..].copy_from_slice(&index.to_be_bytes());
                SccpReplayArchiveLeafV1 {
                    key,
                    record_digest: [0x71; 32],
                }
            })
            .collect();
        let allocation_bomb_bytes =
            norito::encode_canonical(&allocation_bomb).expect("oversized sequence encodes");
        assert_eq!(
            decode_sccp_replay_archive_snapshot_v1(
                &allocation_bomb_bytes,
                SccpReplayArchiveDecodeLimitsV1 {
                    max_snapshot_bytes: allocation_bomb_bytes.len(),
                    max_snapshot_leaves: 1,
                }
            ),
            Err(SccpReplayArchiveError::SnapshotLimit)
        );

        let mut tampered = snapshot;
        tampered.forest.leaf_count = 1;
        let tampered_bytes =
            norito::encode_canonical(&tampered).expect("tampered snapshot encodes");
        assert_eq!(
            decode_sccp_replay_archive_snapshot_v1(
                &tampered_bytes,
                SccpReplayArchiveDecodeLimitsV1 {
                    max_snapshot_bytes: tampered_bytes.len(),
                    max_snapshot_leaves: 1,
                }
            ),
            Err(SccpReplayArchiveError::RebuildMismatch)
        );
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
            .publish_snapshot(&id(), finality(9, [0; 32]))
            .expect("snapshot publishes");
        (
            SccpReplayArchiveReplicaPolicyV1 { replicas: bindings },
            pairs,
            SccpReplayArchiveCheckpointBodyV1::from_snapshot(&snapshot)
                .expect("checkpoint body builds"),
        )
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

    #[test]
    fn exactly_three_pinned_replica_signatures_must_agree() {
        let (policy, pairs, body) = replica_fixture();
        let message = body.signing_message().expect("message hashes");
        let attestations = attestations_for_message(&policy, &pairs, message);
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
}
