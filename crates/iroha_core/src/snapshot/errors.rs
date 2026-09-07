//! Snapshot read and write error types.

use super::*;

/// Error variants for snapshot reading
#[derive(thiserror::Error, Debug, displaydoc::Display)]
#[ignore_extra_doc_attributes]
pub enum TryReadError {
    /// The snapshot was not found
    NotFound,
    /// Failed reading/writing {1:?} from disk
    IO(#[source] std::io::Error, PathBuf),
    /// Error (de)serializing state snapshot
    Serialization(#[source] norito::json::Error),
    /// Signed snapshot payload is not the single canonical first-release JSON encoding
    NonCanonicalSnapshotPayload,
    /// Snapshot exceeds a configured typed decode or transient resource boundary: {0}
    SnapshotResourceLimit(String),
    /// Snapshot artifact or directory binding changed at {0:?}
    SnapshotBindingChanged(PathBuf),
    /// Immutable snapshot generation at {path:?} is invalid: {reason}
    SnapshotGenerationInvalid {
        /// Invalid pointer, directory, or artifact path.
        path: PathBuf,
        /// Exact fail-closed integrity violation.
        reason: String,
    },
    /// Snapshot digest file missing at {0:?}
    ChecksumMissing(PathBuf),
    /// Snapshot digest mismatch (expected `{expected}`, got `{actual}`)
    ChecksumMismatch {
        /// Expected digest from the `.sha256` sidecar.
        expected: String,
        /// Actual digest computed from the snapshot payload.
        actual: String,
    },
    /// Snapshot signature file missing at {0:?}
    SignatureMissing(PathBuf),
    /// Snapshot signature malformed (`{0}`)
    SignatureMalformed(String),
    /// Snapshot signature invalid (`{0}`)
    SignatureInvalid(String),
    /// Snapshot Merkle metadata missing at {0:?}
    MerkleMissing(PathBuf),
    /// Snapshot Merkle metadata parse error
    MerkleMetadata(#[source] norito::json::Error),
    /// Snapshot Merkle metadata malformed (`{0}`)
    MerkleMetadataMalformed(String),
    /// Snapshot Merkle root mismatch (expected `{expected}`, got `{actual}`)
    MerkleMismatch {
        /// Root recorded in metadata.
        expected: String,
        /// Root derived from the snapshot payload.
        actual: String,
    },
    /// Snapshot Merkle chunk size mismatch (expected `{expected}`, got `{actual}`)
    MerkleChunkSizeMismatch {
        /// Chunk size requested by the caller.
        expected: NonZeroUsize,
        /// Chunk size recorded in metadata.
        actual: NonZeroUsize,
    },
    /// Snapshot length mismatch (expected `{expected}` bytes, got `{actual}` bytes)
    MerkleLengthMismatch {
        /// Length recorded in metadata.
        expected: u64,
        /// Length derived from the snapshot payload.
        actual: u64,
    },
    /// Snapshot Merkle proof invalid for chunk `{chunk}` (`{reason}`)
    MerkleProofInvalid {
        /// Index of the chunk that failed verification.
        chunk: usize,
        /// Reason the Merkle verification failed.
        reason: String,
    },
    /// Snapshot exact network id mismatch (expected `{expected}`, got `{actual}`)
    NetworkIdMismatch {
        /// Expected genesis-derived network id from configuration.
        expected: NetworkId,
        /// Exact network id recorded in the snapshot payload.
        actual: NetworkId,
    },
    /// Snapshot contains an invalid governed SCCP registry (`{0}`)
    InvalidSccpRegistry(String),
    /// Snapshot contains invalid SCCP state in its one-block MV revert candidate (`{0}`)
    InvalidSccpRevert(String),
    /// Snapshot bootstrap authorization or typed trust root is invalid (`{0}`)
    InvalidSnapshotBootstrap(String),
    /// Snapshot WSV checkpoint mismatch at height `{height}` (expected `{expected:?}`, got `{actual:?}`)
    WsvCheckpointMismatch {
        /// Committed snapshot height whose checkpoint was validated.
        height: usize,
        /// Canonical WSV hash retained by Kura.
        expected: Hash,
        /// Canonical WSV hash reconstructed from the signed snapshot.
        actual: Hash,
    },
    /// Snapshot state is incompatible with runtime ZK configuration: {0}
    ZkConfigInstall(#[source] ZkConfigInstallError),
    /// Snapshot is in a non-consistent state. Snapshot has greater height (`{snapshot_height}`) than kura block store (`{kura_height}`)
    MismatchedHeight {
        /// The amount of block hashes stored by snapshot
        snapshot_height: usize,
        /// The amount of blocks stored by [`Kura`]
        kura_height: usize,
    },
    /// Snapshot is in a non-consistent state. Hash of the block at height `{height}` is different between snapshot (`{snapshot_block_hash}`) and kura (`{kura_block_hash}`)
    MismatchedHash {
        /// Height at which block hashes differs between snapshot and [`Kura`]
        height: usize,
        /// Hash of the block stored in snapshot
        snapshot_block_hash: HashOf<BlockHeader>,
        /// Hash of the block stored in kura
        kura_block_hash: HashOf<BlockHeader>,
    },
    /// Snapshot is in a non-consistent state. Kura is missing block {height}.
    MissingBlock {
        /// Height of the missing block in [`Kura`].
        height: usize,
    },
    /// Snapshot at height `{snapshot_height}` is missing the durable Space Directory manifest section
    MissingSpaceDirectoryManifestSection {
        /// Height recorded by the malformed snapshot.
        snapshot_height: usize,
    },
    /// Failed to reconcile snapshot block hashes with Kura
    Kura(#[source] KuraError),
}
/// Error variants for snapshot writing
#[derive(thiserror::Error, Debug, displaydoc::Display)]
pub(super) enum TryWriteError {
    /// Failed reading/writing {1:?} from disk
    IO(#[source] std::io::Error, PathBuf),
    /// Error (de)serializing World State View snapshot
    Serialization(norito::json::Error),
    /// Generated snapshot is not admissible through the restart reader: {0}
    RestartValidation(#[source] TryReadError),
    /// Error (de)serializing snapshot Merkle metadata
    MerkleSerialization(norito::json::Error),
    /// Error signing snapshot digest
    Signing(#[source] iroha_crypto::Error),
    /// Snapshot publication failed its stable-file or stable-directory integrity check: {0}
    PublicationIntegrity(String),
    /// Canonical snapshot payload is `{actual}` bytes; configured maximum is `{maximum}`
    PayloadTooLarge {
        /// Canonical payload length.
        actual: usize,
        /// Configured reader/writer limit.
        maximum: NonZeroUsize,
    },
    /// Failed to read the exact durable Kura boundary
    ExactKuraBoundary(#[source] KuraError),
    /// Refusing to write snapshot at state height `{state_height}` because durable Kura height is `{kura_height}`
    StateAheadOfKura {
        /// Height recorded by state/block-hash journal.
        state_height: usize,
        /// Height durably indexed by Kura.
        kura_height: usize,
    },
    /// Refusing to write snapshot at height `{height}` because latest state hash `{state_hash:?}` does not match Kura hash `{kura_hash:?}`
    LatestBlockHashMismatch {
        /// Height being snapshotted.
        height: usize,
        /// Latest block hash recorded by state.
        state_hash: Option<HashOf<BlockHeader>>,
        /// Block hash recorded by Kura at the same height.
        kura_hash: Option<HashOf<BlockHeader>>,
    },
    /// Refusing to publish snapshot at height `{height}` because durable commit evidence is incomplete or inconsistent: {reason}
    CommitEvidence {
        /// Height encoded by the serialized snapshot itself.
        height: u64,
        /// Exact fail-closed evidence violation.
        reason: String,
    },
    /// Snapshot at height `{height}` is waiting for its in-flight durable commit tuple: {reason}
    CommitEvidenceDeferred {
        /// Height encoded by the serialized snapshot itself.
        height: u64,
        /// Missing publication step that a later snapshot interval must retry.
        reason: String,
    },
}
