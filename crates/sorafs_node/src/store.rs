//! Persistent storage backend for the embedded SoraFS node.
//!
//! This module wraps `sorafs_car::ChunkStore` with an on-disk manifest index,
//! deterministic chunk layout, verified Proof-of-Retrievability (PoR) recovery,
//! canonical Proof-of-Data-Possession (PDP) commitments, and quota enforcement
//! derived from Torii storage configuration.

#![allow(unexpected_cfgs)]

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    hash::{DefaultHasher, Hash as StdHash, Hasher},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex, MutexGuard, RwLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(any(target_os = "linux", target_os = "android"))]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

use blake3::Hash;
use hex::ToHex;
use iroha_data_model::da::{ingest::DaStripeLayout, manifest::ChunkRole};
use norito::{
    core::Error as NoritoError,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use sorafs_car::{
    self, CarBuildPlan, CarChunk, ChunkStore, ChunkStoreError, DirectoryPublicationStatus,
    FilePlan, PayloadSource, PorMerkleTree, PorProof, PorSampleIndices, TaikaiSegmentHint,
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    MANIFEST_VERSION_V1, MAX_PROOF_STREAM_SAMPLE_COUNT, ManifestV1,
    pdp::{
        PDP_MAX_SEGMENT_SAMPLES_V1, PdpCommitmentV1, PdpCommitmentValidationError,
        PdpMerkleReadError, PdpMerkleTreeError, PdpMerkleTreeV1, PdpProofLeafV1, PdpSampleV1,
        estimated_heap_bytes as estimated_pdp_heap_bytes,
    },
    retention::{RetentionMetadataError, RetentionSourceV1},
};
use thiserror::Error;

use crate::config::StorageConfig;

const INDEX_VERSION_V1: u8 = 1;
const MANIFEST_DIR_NAME: &str = "manifests";
const MANIFEST_FILE_NAME: &str = "manifest.to";
const METADATA_FILE_NAME: &str = "metadata.to";
const CHUNKS_DIR_NAME: &str = "chunks";
const ATOMIC_EXT: &str = "tmp";
const GC_TRASH_DIR_NAME: &str = "gc_trash";
const INGEST_STAGING_DIR_NAME: &str = ".ingest-staging";
const STORAGE_LOCK_FILE_NAME: &str = ".storage.lock";
const MAX_STORAGE_INDEX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST_METADATA_BYTES: u64 = 64 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
static ATOMIC_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);
const ATOMIC_PUBLICATION_LOCK_SHARDS: usize = 64;
static ATOMIC_PUBLICATION_LOCKS: LazyLock<[Mutex<()>; ATOMIC_PUBLICATION_LOCK_SHARDS]> =
    LazyLock::new(|| std::array::from_fn(|_| Mutex::new(())));
static GC_TRASH_COUNTER: AtomicU64 = AtomicU64::new(0);
static INGEST_STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
struct ChunkReadTestHook {
    path: PathBuf,
    entered: std::sync::mpsc::Sender<()>,
    release: std::sync::mpsc::Receiver<()>,
}

#[cfg(test)]
static CHUNK_READ_TEST_HOOK: Mutex<Option<ChunkReadTestHook>> = Mutex::new(None);
#[cfg(test)]
static CHUNK_READ_TEST_SERIAL: Mutex<()> = Mutex::new(());

fn storage_decode_limits(max_bytes: u64) -> norito::DecodeLimits {
    let byte_limit = usize::try_from(max_bytes).unwrap_or(usize::MAX);
    norito::DecodeLimits::new(
        byte_limit,
        byte_limit,
        byte_limit,
        byte_limit.saturating_mul(4),
        64,
    )
}

fn ensure_persistent_artifact_size(
    artifact: &'static str,
    bytes: &[u8],
    maximum: u64,
) -> Result<(), StorageError> {
    let actual =
        u64::try_from(bytes.len()).map_err(|_| StorageError::PersistentArtifactTooLarge {
            artifact,
            actual: bytes.len(),
            maximum,
        })?;
    if actual > maximum {
        return Err(StorageError::PersistentArtifactTooLarge {
            artifact,
            actual: bytes.len(),
            maximum,
        });
    }
    Ok(())
}

/// Errors raised by the SoraFS storage backend.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Encountered an unexpected I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    /// Failed to encode or decode a Norito payload while persisting metadata.
    #[error("Norito codec error: {0}")]
    Norito(#[from] NoritoError),
    /// Persisted storage index uses an unsupported schema version.
    #[error("unsupported storage index version {version}")]
    UnsupportedIndexVersion {
        /// Version found in the persisted index.
        version: u8,
    },
    /// Persisted index, manifest, metadata, or chunk state is inconsistent.
    #[error("corrupt SoraFS storage state at {path}: {reason}")]
    CorruptStorageState {
        /// Path of the persisted artifact that failed validation.
        path: String,
        /// Stable diagnostic explaining the rejected invariant.
        reason: String,
    },
    /// A canonical artifact exceeded the maximum size accepted during restart.
    #[error("{artifact} is {actual} bytes; maximum is {maximum}")]
    PersistentArtifactTooLarge {
        /// Stable artifact label.
        artifact: &'static str,
        /// Encoded byte length.
        actual: usize,
        /// Restart decode ceiling.
        maximum: u64,
    },
    /// Another process already owns the configured storage directory.
    #[error("SoraFS storage directory is already in use: {path}")]
    StorageDirectoryInUse {
        /// Lock file protecting the storage directory.
        path: String,
    },
    /// An in-memory layout value cannot be represented by the persistent schema.
    #[error("storage layout field {field} value {value} exceeds persistent limit {max}")]
    LayoutValueTooLarge {
        /// Name of the field that exceeded the persistent representation.
        field: &'static str,
        /// Value supplied by the in-memory build plan.
        value: usize,
        /// Largest value accepted by the persistent representation.
        max: u64,
    },
    /// A caller supplied a file layout that cannot describe the stored chunk plan.
    #[error("invalid storage file layout: {reason}")]
    InvalidFileLayout {
        /// Stable invariant rejected before persistence.
        reason: String,
    },
    /// An atomic replacement reached rename but post-commit verification or sync failed.
    #[error("storage durability is uncertain for {path}: {reason}")]
    DurabilityUncertain {
        /// Replaced artifact whose directory entry could not be confirmed durable.
        path: String,
        /// Underlying synchronization failure.
        reason: String,
    },
    /// A previous uncertain commit requires a process restart and recovery pass.
    #[error("SoraFS storage is fail-stopped after an uncertain commit: {reason}")]
    DurabilityPoisoned {
        /// First durability failure recorded by the backend.
        reason: String,
    },
    /// The provided manifest is already present in the index.
    #[error("manifest {manifest_id} already stored")]
    ManifestExists {
        /// Canonical manifest identifier (hex-encoded digest).
        manifest_id: String,
    },
    /// Capacity limit reached while attempting to store a manifest.
    #[error("storage capacity exceeded: required {required} bytes, available {available}")]
    CapacityExceeded {
        /// Bytes required by the manifest payload.
        required: u64,
        /// Bytes still available under the configured quota.
        available: u64,
    },
    /// Pin limit reached while attempting to accept another manifest.
    #[error("storage pin limit of {limit} manifests reached")]
    PinLimitReached {
        /// Maximum number of manifests permitted by configuration.
        limit: usize,
    },
    /// Chunk digests supplied by the plan did not match the payload.
    #[error("chunk {chunk_index} digest does not match payload bytes")]
    ChunkDigestMismatch {
        /// Index of the chunk that failed verification.
        chunk_index: usize,
    },
    /// Payload ended before all chunk data could be read.
    #[error("payload ended prematurely: expected {expected} bytes, read {actual}")]
    PayloadLengthMismatch {
        /// Expected payload size from the plan.
        expected: u64,
        /// Bytes actually read from the payload stream.
        actual: u64,
    },
    /// Ingested manifest was encoded with an unexpected version.
    #[error("unsupported manifest version {version}")]
    UnsupportedManifestVersion {
        /// Version discovered while ingesting the manifest.
        version: u8,
    },
    /// Chunk profile present in the manifest does not match the ingestion plan.
    #[error("chunk profile mismatch between manifest and plan")]
    ChunkProfileMismatch,
    /// Rebuilt provider PoR root does not match the canonical manifest commitment.
    #[error("provider PoR root does not match the manifest commitment")]
    PorRootMismatch,
    /// Requested PoR sample count exceeds the proof-stream protocol ceiling.
    #[error("PoR sample count {requested} exceeds the v1 maximum {maximum}")]
    PorSampleCountTooLarge {
        /// Number of samples requested by the caller.
        requested: usize,
        /// Maximum sample count accepted by the v1 protocol.
        maximum: u32,
    },
    /// Failed to rebuild the PoR tree from persisted chunk data.
    #[error("failed to build PoR tree: {0}")]
    ChunkStore(#[from] ChunkStoreError),
    /// Bounded PoR commitment geometry overflowed its persistent representation.
    #[error("PoR commitment geometry overflow while accounting for {context}")]
    PorCommitmentGeometryOverflow {
        /// Counter or range whose checked conversion failed.
        context: &'static str,
    },
    /// Checked allocation geometry overflowed before any allocation was attempted.
    #[error("storage allocation geometry overflow while accounting for {context}")]
    AllocationGeometryOverflow {
        /// Collection or byte buffer whose length calculation overflowed.
        context: &'static str,
    },
    /// Failed to construct or validate a canonical PDP tree.
    #[error("failed to build PDP tree: {0}")]
    PdpTree(#[from] PdpMerkleTreeError),
    /// Persisted or newly constructed PDP commitment metadata is invalid.
    #[error("invalid PDP commitment: {0}")]
    PdpCommitment(#[from] PdpCommitmentValidationError),
    /// Retaining another canonical PDP tree would exceed the configured aggregate budget.
    #[error("PDP tree memory budget exceeded: required {required} bytes, available {available}")]
    PdpTreeMemoryExceeded {
        /// Retained bytes required by the candidate tree.
        required: u64,
        /// Bytes remaining under the aggregate retained-tree budget.
        available: u64,
    },
    /// Configured PDP sample window is outside the v1 protocol range.
    #[error("PDP sample window {found} is outside 1..={maximum}")]
    InvalidPdpSampleWindow {
        /// Configured value.
        found: u16,
        /// V1 protocol ceiling.
        maximum: usize,
    },
    /// The requested manifest has no PDP commitment (valid only for an empty payload).
    #[error("manifest {manifest_id} has no PDP commitment")]
    PdpUnavailable {
        /// Canonical manifest identifier.
        manifest_id: String,
    },
    /// A canonical PDP witness could not be produced from the stored payload.
    #[error("PDP witness construction failed: {reason}")]
    PdpWitness {
        /// Stable diagnostic from bounded sample validation or verified payload I/O.
        reason: String,
    },
    /// Manifest content length does not match the payload described by the ingestion plan.
    #[error("manifest content length does not match the ingestion plan")]
    ManifestContentLengthMismatch,
    /// The system clock cannot produce a non-zero Unix timestamp for commitment sealing.
    #[error("system clock cannot produce a valid PDP commitment timestamp")]
    InvalidSystemTime,
    /// Manifest with the requested identifier does not exist.
    #[error("manifest {manifest_id} not found")]
    ManifestNotFound {
        /// Canonical manifest identifier (hex-encoded digest).
        manifest_id: String,
    },
    /// Requested byte range exceeds the payload bounds.
    #[error("requested range offset {offset} length {len} exceeds payload length {content_length}")]
    RangeOutOfBounds {
        /// Starting offset of the request.
        offset: u64,
        /// Number of bytes requested.
        len: usize,
        /// Total payload length available.
        content_length: u64,
    },
    /// Manifest does not contain a chunk with the requested digest.
    #[error("chunk {digest_hex} not found in manifest {manifest_id}")]
    ChunkNotFound {
        /// Canonical manifest identifier (hex-encoded digest).
        manifest_id: String,
        /// Hex-encoded chunk digest.
        digest_hex: String,
    },
    /// Chunk-role annotations length mismatched the stored chunk count.
    #[error("chunk role vector length {actual} does not match expected {expected}")]
    ChunkRoleLengthMismatch {
        /// Expected number of chunk role entries.
        expected: usize,
        /// Actual number of entries provided.
        actual: usize,
    },
    /// Retention metadata payload failed validation.
    #[error("retention metadata invalid: {0}")]
    RetentionMetadata(#[from] RetentionMetadataError),
}

/// Runtime facade over the deterministic, persistent chunk store.
#[derive(Debug)]
pub struct StorageBackend {
    config: StorageConfig,
    root_dir: PathBuf,
    manifests_dir: PathBuf,
    index_path: PathBuf,
    _lock_file: File,
    access_metadata_lock: Mutex<()>,
    persisted_access_counter: AtomicU64,
    durability_healthy: AtomicBool,
    durability_failure: Mutex<Option<String>>,
    state: RwLock<StorageState>,
}

#[derive(Debug)]
struct StorageState {
    index: ManifestIndex,
    manifests: BTreeMap<String, StoredManifest>,
    total_bytes: u64,
    reserved_bytes: u64,
    pdp_tree_bytes: u64,
    reserved_pdp_tree_bytes: u64,
    inflight_manifests: BTreeSet<String>,
    access_counter: u64,
    chunk_refcounts: Vec<ChunkRefcountEntry>,
}

struct IngestReservation<'a> {
    backend: &'a StorageBackend,
    manifest_id: String,
    reserved_bytes: u64,
    reserved_pdp_tree_bytes: u64,
    active: bool,
}

impl IngestReservation<'_> {
    fn release(&mut self, state: &mut StorageState) {
        if !self.active {
            return;
        }
        match state.reserved_bytes.checked_sub(self.reserved_bytes) {
            Some(remaining) => state.reserved_bytes = remaining,
            None => {
                let reserved = state.reserved_bytes;
                state.reserved_bytes = 0;
                iroha_logger::error!(
                    reserved,
                    release = self.reserved_bytes,
                    "storage byte reservation accounting underflow"
                );
            }
        }
        match state
            .reserved_pdp_tree_bytes
            .checked_sub(self.reserved_pdp_tree_bytes)
        {
            Some(remaining) => state.reserved_pdp_tree_bytes = remaining,
            None => {
                let reserved = state.reserved_pdp_tree_bytes;
                state.reserved_pdp_tree_bytes = 0;
                iroha_logger::error!(
                    reserved,
                    release = self.reserved_pdp_tree_bytes,
                    "PDP tree reservation accounting underflow"
                );
            }
        }
        state.inflight_manifests.remove(&self.manifest_id);
        self.active = false;
    }
}

impl Drop for IngestReservation<'_> {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let mut state = self
            .backend
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.release(&mut state);
    }
}

struct StagingDirectory {
    path: PathBuf,
    active: bool,
}

impl StagingDirectory {
    fn new(path: PathBuf) -> Self {
        Self { path, active: true }
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if self.active {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

impl StorageState {
    fn available_capacity(&self, max_capacity: u64) -> u64 {
        max_capacity
            .saturating_sub(self.total_bytes)
            .saturating_sub(self.reserved_bytes)
    }

    fn available_pdp_tree_memory(&self, max_bytes: u64) -> u64 {
        max_bytes
            .saturating_sub(self.pdp_tree_bytes)
            .saturating_sub(self.reserved_pdp_tree_bytes)
    }
}

/// Summary of a manifest stored on disk.
#[derive(Debug, Clone)]
pub struct StoredManifest {
    manifest_id: String,
    manifest_cid: Vec<u8>,
    manifest_digest: [u8; 32],
    payload_digest: [u8; 32],
    content_length: u64,
    chunk_profile_handle: String,
    stripe_layout: Option<DaStripeLayout>,
    stored_at_unix_secs: u64,
    retention_epoch: u64,
    retention_source: Option<RetentionSourceV1>,
    last_access: u64,
    files: Vec<StoredFileRecord>,
    chunk_files: Vec<ChunkFileRecord>,
    por_tree: Arc<PorMerkleTree>,
    por_commitment: Option<StoredPorCommitmentV1>,
    por_commitment_digest: Option<[u8; 32]>,
    pdp_commitment: Option<PdpCommitmentV1>,
    pdp_commitment_digest: Option<[u8; 32]>,
    pdp_tree: Option<Arc<PdpMerkleTreeV1>>,
    pdp_tree_memory_bytes: u64,
    manifest_path: PathBuf,
    io_lock: Arc<RwLock<()>>,
}

struct IngestedPayload {
    chunk_records: Vec<StoredChunkRecord>,
    por_tree: Arc<PorMerkleTree>,
    pdp_tree: Option<Arc<PdpMerkleTreeV1>>,
}

struct ManifestRuntimeProofs {
    por_commitment_digest: Option<[u8; 32]>,
    por_tree: Arc<PorMerkleTree>,
    pdp_commitment_digest: Option<[u8; 32]>,
    pdp_tree: Option<Arc<PdpMerkleTreeV1>>,
    pdp_tree_memory_bytes: u64,
}

/// Components required to construct a [`StoredManifest`] without hitting the storage backend.
#[derive(Debug)]
pub struct StoredManifestParts {
    /// Canonical identifier derived from the manifest digest (hex string).
    pub manifest_id: String,
    /// Canonical manifest CID bytes.
    pub manifest_cid: Vec<u8>,
    /// BLAKE3-256 digest over the canonical Norito manifest encoding.
    pub manifest_digest: [u8; 32],
    /// BLAKE3-256 digest over the payload bytes.
    pub payload_digest: [u8; 32],
    /// Total payload size represented by the manifest.
    pub content_length: u64,
    /// Negotiated chunk profile handle (`namespace.name@semver`).
    pub chunk_profile_handle: String,
    /// Optional stripe layout (row/column parity) recorded for the manifest.
    pub stripe_layout: Option<DaStripeLayout>,
    /// UNIX timestamp (seconds) when the manifest was persisted.
    pub stored_at_unix_secs: u64,
    /// Unix retention epoch for garbage collection (0 if not retained).
    pub retention_epoch: u64,
    /// Retention source record (optional for legacy manifests).
    pub retention_source: Option<RetentionSourceV1>,
    /// Monotonic access counter recorded for LRU eviction ordering.
    pub last_access: u64,
    /// File descriptors describing how the original dataset maps to payload offsets.
    pub files: Vec<StoredFileRecord>,
    /// Records describing each stored chunk file.
    pub chunk_files: Vec<ChunkFileRecord>,
    /// Proof-of-retrievability Merkle tree snapshot.
    pub por_tree: StoredPorTree,
    /// Filesystem path where the manifest resides.
    pub manifest_path: PathBuf,
}

impl StoredManifest {
    /// Construct a manifest summary from its component parts.
    ///
    /// This is primarily intended for tests and offline validation harnesses
    /// that need to stand up synthetic manifest metadata without persisting it
    /// through the storage backend.
    #[must_use]
    pub fn from_parts(parts: StoredManifestParts) -> Self {
        Self {
            manifest_id: parts.manifest_id,
            manifest_cid: parts.manifest_cid,
            manifest_digest: parts.manifest_digest,
            payload_digest: parts.payload_digest,
            content_length: parts.content_length,
            chunk_profile_handle: parts.chunk_profile_handle,
            stripe_layout: parts.stripe_layout,
            stored_at_unix_secs: parts.stored_at_unix_secs,
            retention_epoch: parts.retention_epoch,
            retention_source: parts.retention_source,
            last_access: parts.last_access,
            files: parts.files,
            chunk_files: parts.chunk_files,
            por_tree: parts.por_tree.into_arc(),
            por_commitment: None,
            por_commitment_digest: None,
            pdp_commitment: None,
            pdp_commitment_digest: None,
            pdp_tree: None,
            pdp_tree_memory_bytes: 0,
            manifest_path: parts.manifest_path,
            io_lock: Arc::new(RwLock::new(())),
        }
    }

    fn try_clone_runtime(&self) -> Result<Self, StorageError> {
        let mut files = Vec::new();
        files.try_reserve_exact(self.files.len()).map_err(|_| {
            StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                context: "runtime manifest file records",
                requested: self.files.len(),
            })
        })?;
        for file in &self.files {
            files.push(StoredFileRecord {
                path: try_clone_logical_path(&file.path, "runtime manifest logical path")?,
                offset: file.offset,
                size: file.size,
                first_chunk: file.first_chunk,
                chunk_count: file.chunk_count,
            });
        }
        let mut chunk_files = Vec::new();
        chunk_files
            .try_reserve_exact(self.chunk_files.len())
            .map_err(|_| {
                StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                    context: "runtime manifest chunk records",
                    requested: self.chunk_files.len(),
                })
            })?;
        for chunk in &self.chunk_files {
            chunk_files.push(try_clone_chunk_file_record(chunk)?);
        }
        Ok(Self {
            manifest_id: try_clone_text(&self.manifest_id, "runtime manifest id")?,
            manifest_cid: try_clone_bytes(&self.manifest_cid, "runtime manifest CID")?,
            manifest_digest: self.manifest_digest,
            payload_digest: self.payload_digest,
            content_length: self.content_length,
            chunk_profile_handle: try_clone_text(
                &self.chunk_profile_handle,
                "runtime chunk profile handle",
            )?,
            stripe_layout: self.stripe_layout,
            stored_at_unix_secs: self.stored_at_unix_secs,
            retention_epoch: self.retention_epoch,
            retention_source: try_clone_retention_source(self.retention_source.as_ref())?,
            last_access: self.last_access,
            files,
            chunk_files,
            por_tree: Arc::clone(&self.por_tree),
            por_commitment: self
                .por_commitment
                .as_ref()
                .map(try_clone_por_commitment)
                .transpose()?,
            por_commitment_digest: self.por_commitment_digest,
            pdp_commitment: try_clone_pdp_commitment(self.pdp_commitment.as_ref())?,
            pdp_commitment_digest: self.pdp_commitment_digest,
            pdp_tree: self.pdp_tree.as_ref().map(Arc::clone),
            pdp_tree_memory_bytes: self.pdp_tree_memory_bytes,
            manifest_path: try_clone_path_buf(&self.manifest_path, "runtime manifest path")?,
            io_lock: Arc::clone(&self.io_lock),
        })
    }

    /// Canonical identifier derived from the manifest digest (hex string).
    #[must_use]
    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    /// Returns the raw manifest CID bytes.
    #[must_use]
    pub fn manifest_cid(&self) -> &[u8] {
        &self.manifest_cid
    }

    /// Returns the manifest digest (BLAKE3-256 over canonical Norito encoding).
    #[must_use]
    pub fn manifest_digest(&self) -> &[u8; 32] {
        &self.manifest_digest
    }

    /// Returns the payload digest (BLAKE3-256 over raw payload bytes).
    #[must_use]
    pub fn payload_digest(&self) -> &[u8; 32] {
        &self.payload_digest
    }

    /// Total number of bytes stored for this manifest.
    #[must_use]
    pub fn content_length(&self) -> u64 {
        self.content_length
    }

    /// Returns the negotiated chunk profile handle (namespace.name@semver).
    #[must_use]
    pub fn chunk_profile_handle(&self) -> &str {
        &self.chunk_profile_handle
    }

    /// Optional stripe layout (row/column parity summary) recorded for the manifest.
    #[must_use]
    pub fn stripe_layout(&self) -> Option<&DaStripeLayout> {
        self.stripe_layout.as_ref()
    }

    /// Timestamp (seconds since UNIX epoch) when the manifest was persisted.
    #[must_use]
    pub fn stored_at_unix_secs(&self) -> u64 {
        self.stored_at_unix_secs
    }

    /// Retention epoch applied to the manifest (0 when unbounded).
    #[must_use]
    pub fn retention_epoch(&self) -> u64 {
        self.retention_epoch
    }

    /// Retention source metadata when available.
    #[must_use]
    pub fn retention_source(&self) -> Option<&RetentionSourceV1> {
        self.retention_source.as_ref()
    }

    /// Monotonic access counter recorded for LRU eviction ordering.
    #[must_use]
    pub fn last_access(&self) -> u64 {
        self.last_access
    }

    /// Number of chunks stored for the manifest.
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.chunk_files.len()
    }

    /// Returns the stored file descriptors in deterministic payload order.
    #[must_use]
    pub fn files(&self) -> &[StoredFileRecord] {
        &self.files
    }

    /// Returns metadata for the file identified by the supplied relative path.
    #[must_use]
    pub fn file_by_path(&self, path: &[String]) -> Option<&StoredFileRecord> {
        self.files.iter().find(|file| file.path == path)
    }

    /// Returns metadata for the requested chunk.
    #[must_use]
    pub fn chunk(&self, index: usize) -> Option<&ChunkFileRecord> {
        self.chunk_files.get(index)
    }

    /// Returns a contiguous slice of chunks covering the payload range starting at `offset`.
    pub fn chunk_slice(&self, offset: u64, len: usize) -> Result<ChunkSlice, StorageError> {
        if len == 0 {
            return Err(StorageError::RangeOutOfBounds {
                offset,
                len,
                content_length: self.content_length,
            });
        }
        if offset >= self.content_length {
            return Err(StorageError::RangeOutOfBounds {
                offset,
                len,
                content_length: self.content_length,
            });
        }
        let len_u64 = u64::try_from(len).map_err(|_| StorageError::RangeOutOfBounds {
            offset,
            len,
            content_length: self.content_length,
        })?;
        let end = offset
            .checked_add(len_u64)
            .ok_or(StorageError::RangeOutOfBounds {
                offset,
                len,
                content_length: self.content_length,
            })?;
        if end > self.content_length {
            return Err(StorageError::RangeOutOfBounds {
                offset,
                len,
                content_length: self.content_length,
            });
        }

        let mut chunks = Vec::new();
        let mut cursor = offset;
        let mut start_index = None;

        for (idx, chunk) in self.chunk_files.iter().enumerate() {
            let chunk_start = chunk.offset;
            let chunk_end = chunk_start.checked_add(u64::from(chunk.length)).ok_or(
                StorageError::RangeOutOfBounds {
                    offset,
                    len,
                    content_length: self.content_length,
                },
            )?;

            if chunk_end <= offset {
                continue;
            }

            if let Some(start) = start_index {
                if chunk_start != cursor {
                    return Err(StorageError::RangeOutOfBounds {
                        offset,
                        len,
                        content_length: self.content_length,
                    });
                }
                chunks.try_reserve(1).map_err(|_| {
                    StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                        context: "chunk range response",
                        requested: 1,
                    })
                })?;
                chunks.push(try_clone_chunk_file_record(chunk)?);
                cursor = chunk_end;
                if cursor == end {
                    return Ok(ChunkSlice {
                        start_index: start,
                        end_index: idx,
                        chunks,
                    });
                }
                if cursor > end {
                    break;
                }
            } else {
                if chunk_start != offset {
                    return Err(StorageError::RangeOutOfBounds {
                        offset,
                        len,
                        content_length: self.content_length,
                    });
                }
                start_index = Some(idx);
                chunks.try_reserve(1).map_err(|_| {
                    StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                        context: "chunk range response",
                        requested: 1,
                    })
                })?;
                chunks.push(try_clone_chunk_file_record(chunk)?);
                cursor = chunk_end;
                if cursor == end {
                    return Ok(ChunkSlice {
                        start_index: idx,
                        end_index: idx,
                        chunks,
                    });
                }
            }
        }

        Err(StorageError::RangeOutOfBounds {
            offset,
            len,
            content_length: self.content_length,
        })
    }

    /// Load and decode the persisted manifest payload from disk.
    pub fn load_manifest(&self) -> Result<ManifestV1, StorageError> {
        let _io_guard = self
            .io_lock
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let path = self.manifest_path();
        let bytes = read_bounded_regular_file(path, MAX_MANIFEST_BYTES)?;
        let manifest: ManifestV1 = norito::decode_from_bytes_with_limits(
            &bytes,
            storage_decode_limits(MAX_MANIFEST_BYTES),
        )?;
        let canonical = manifest.encode()?;
        if canonical != bytes
            || blake3::hash(&bytes).as_bytes() != &self.manifest_digest
            || manifest.root_cid != self.manifest_cid
            || manifest.content_length != self.content_length
            || canonical_profile_handle(&manifest) != self.chunk_profile_handle
            || manifest.por_root != *self.por_tree.root()
        {
            return Err(corrupt_storage_state(
                path,
                "manifest no longer matches its immutable stored identity",
            ));
        }
        Ok(manifest)
    }

    /// Reconstruct a [`CarBuildPlan`] matching the stored manifest chunk metadata.
    #[must_use]
    pub fn to_car_plan(&self, profile: ChunkProfile) -> CarBuildPlan {
        self.to_car_plan_with_hint(profile, None)
    }

    /// Reconstruct a [`CarBuildPlan`] matching the stored manifest chunk metadata and
    /// attach an optional Taikai hint to each chunk.
    #[must_use]
    pub fn to_car_plan_with_hint(
        &self,
        profile: ChunkProfile,
        taikai_hint: Option<TaikaiSegmentHint>,
    ) -> CarBuildPlan {
        let chunks = self
            .chunk_files
            .iter()
            .map(|chunk| CarChunk {
                offset: chunk.offset,
                length: chunk.length,
                digest: chunk.digest,
                taikai_segment_hint: taikai_hint.clone(),
            })
            .collect::<Vec<_>>();

        let files = self
            .files
            .iter()
            .map(|file| FilePlan {
                path: file.path.clone(),
                first_chunk: file.first_chunk,
                chunk_count: file.chunk_count,
                size: file.size,
            })
            .collect::<Vec<_>>();

        CarBuildPlan {
            chunk_profile: profile,
            payload_digest: Hash::from_bytes(*self.payload_digest()),
            content_length: self.content_length,
            chunks,
            files,
        }
    }

    /// Fallibly reconstruct a [`CarBuildPlan`] with an optional Taikai hint per chunk.
    ///
    /// # Errors
    ///
    /// Returns an allocation error when chunk, file, path, or hint metadata cannot be cloned.
    pub fn try_to_car_plan_with_hint(
        &self,
        profile: ChunkProfile,
        taikai_hint: Option<&TaikaiSegmentHint>,
    ) -> Result<CarBuildPlan, StorageError> {
        let mut chunks = Vec::new();
        chunks
            .try_reserve_exact(self.chunk_files.len())
            .map_err(|_| {
                StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                    context: "rebuild CAR chunk plan",
                    requested: self.chunk_files.len(),
                })
            })?;
        for chunk in &self.chunk_files {
            chunks.push(CarChunk {
                offset: chunk.offset,
                length: chunk.length,
                digest: chunk.digest,
                taikai_segment_hint: taikai_hint.map(try_clone_taikai_segment_hint).transpose()?,
            });
        }

        let mut files = Vec::new();
        files.try_reserve_exact(self.files.len()).map_err(|_| {
            StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                context: "rebuild CAR file plan",
                requested: self.files.len(),
            })
        })?;
        for file in &self.files {
            files.push(FilePlan {
                path: try_clone_logical_path(&file.path, "rebuild CAR logical path")?,
                first_chunk: file.first_chunk,
                chunk_count: file.chunk_count,
                size: file.size,
            });
        }
        Ok(CarBuildPlan {
            chunk_profile: profile,
            payload_digest: Hash::from_bytes(*self.payload_digest()),
            content_length: self.content_length,
            chunks,
            files,
        })
    }

    fn try_to_car_plan(&self, profile: ChunkProfile) -> Result<CarBuildPlan, StorageError> {
        self.try_to_car_plan_with_hint(profile, None)
    }

    /// Build an in-memory PoR tree for the stored manifest.
    #[must_use]
    pub fn por_tree(&self) -> PorMerkleTree {
        (*self.por_tree).clone()
    }

    /// Borrow the runtime PoR tree rebuilt from verified payload bytes.
    #[must_use]
    pub fn por_tree_ref(&self) -> &PorMerkleTree {
        self.por_tree.as_ref()
    }

    /// Domain-separated digest binding the bounded PoR commitment into the index.
    #[must_use]
    pub fn por_commitment_digest(&self) -> Option<&[u8; 32]> {
        self.por_commitment_digest.as_ref()
    }

    /// Canonical PDP commitment persisted for this non-empty payload.
    #[must_use]
    pub fn pdp_commitment(&self) -> Option<&PdpCommitmentV1> {
        self.pdp_commitment.as_ref()
    }

    /// Domain-separated digest binding the PDP commitment into the storage index.
    #[must_use]
    pub fn pdp_commitment_digest(&self) -> Option<&[u8; 32]> {
        self.pdp_commitment_digest.as_ref()
    }

    /// Canonical runtime PDP tree rebuilt or constructed from verified payload bytes.
    #[must_use]
    pub fn pdp_tree(&self) -> Option<&PdpMerkleTreeV1> {
        self.pdp_tree.as_deref()
    }

    /// Exact retained-node-slab bytes charged to the aggregate PDP tree budget.
    #[must_use]
    pub fn pdp_tree_memory_bytes(&self) -> u64 {
        self.pdp_tree_memory_bytes
    }

    /// Path to the Norito-encoded manifest bytes stored on disk.
    #[must_use]
    pub fn manifest_path(&self) -> &Path {
        self.manifest_path.as_path()
    }
}

/// Metadata describing an individual stored chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkFileRecord {
    /// Path to the stored chunk file.
    pub path: PathBuf,
    /// Byte offset within the original payload.
    pub offset: u64,
    /// Chunk length in bytes.
    pub length: u32,
    /// Expected BLAKE3-256 digest of the chunk.
    pub digest: [u8; 32],
    /// Optional role metadata for repair/placement planning.
    pub role: Option<ChunkRole>,
    /// Optional stripe/group identifier for the chunk.
    pub group_id: Option<u32>,
}

/// Metadata describing how a logical file maps into the stored payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFileRecord {
    /// Relative path components within the dataset root.
    pub path: Vec<String>,
    /// Byte offset where the file begins within the concatenated payload.
    pub offset: u64,
    /// File length in bytes.
    pub size: u64,
    /// Index of the first chunk covering the file.
    pub first_chunk: usize,
    /// Number of chunks covering the file.
    pub chunk_count: usize,
}

/// Role metadata attached to a stored chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkRoleMetadata {
    /// Role of the chunk within the erasure layout.
    pub role: ChunkRole,
    /// Stripe/group identifier associated with the chunk.
    pub group_id: u32,
}

/// Describes the subset of chunks that cover a contiguous payload range.
#[derive(Debug, Clone)]
pub struct ChunkSlice {
    /// Index of the first chunk in the manifest plan.
    pub start_index: usize,
    /// Index of the last chunk in the manifest plan.
    pub end_index: usize,
    /// Cloned chunk metadata entries covering the requested range.
    pub chunks: Vec<ChunkFileRecord>,
}

impl ChunkSlice {
    /// Number of chunks included in the slice.
    #[must_use]
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }
}

/// Compatibility wrapper used by [`StoredManifestParts`] for synthetic manifests.
///
/// Production persistence never serializes this value; only a bounded PoR commitment summary is
/// written to disk, and the full tree is rebuilt from verified chunks on startup.
#[derive(Debug, Clone)]
pub struct StoredPorTree {
    tree: Arc<PorMerkleTree>,
}

impl StoredPorTree {
    /// Clone the wrapped runtime tree.
    #[must_use]
    pub fn to_merkle_tree(&self) -> PorMerkleTree {
        (*self.tree).clone()
    }

    fn into_arc(self) -> Arc<PorMerkleTree> {
        self.tree
    }
}

impl From<&PorMerkleTree> for StoredPorTree {
    fn from(tree: &PorMerkleTree) -> Self {
        Self {
            tree: Arc::new(tree.clone()),
        }
    }
}

const POR_COMMITMENT_VERSION_V1: u8 = 1;
const POR_COMMITMENT_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.node.por.commitment.digest.v1\0";

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct StoredPorCommitmentV1 {
    version: u8,
    root: [u8; 32],
    payload_len: u64,
    chunk_count: u32,
    segment_count: u64,
    leaf_count: u64,
    chunks: Vec<StoredPorChunkCommitmentV1>,
}

#[derive(Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct StoredPorChunkCommitmentV1 {
    chunk_index: u32,
    offset: u64,
    length: u32,
    chunk_digest: [u8; 32],
    root: [u8; 32],
    segment_count: u32,
    leaf_count: u32,
}

impl StoredPorCommitmentV1 {
    fn from_tree(tree: &PorMerkleTree) -> Result<Self, StorageError> {
        let mut chunks = Vec::new();
        chunks.try_reserve_exact(tree.chunks().len()).map_err(|_| {
            StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                context: "bounded PoR commitment chunks",
                requested: tree.chunks().len(),
            })
        })?;
        let mut segment_count = 0_u64;
        let mut leaf_count = 0_u64;
        for chunk in tree.chunks() {
            let chunk_segments = persistent_u32("por.chunk.segment_count", chunk.segments.len())?;
            let chunk_leaves = chunk.segments.iter().try_fold(0_usize, |total, segment| {
                total.checked_add(segment.leaves.len()).ok_or(
                    StorageError::PorCommitmentGeometryOverflow {
                        context: "chunk leaf count",
                    },
                )
            })?;
            let chunk_leaves = persistent_u32("por.chunk.leaf_count", chunk_leaves)?;
            segment_count = segment_count.checked_add(u64::from(chunk_segments)).ok_or(
                StorageError::PorCommitmentGeometryOverflow {
                    context: "segment count",
                },
            )?;
            leaf_count = leaf_count.checked_add(u64::from(chunk_leaves)).ok_or(
                StorageError::PorCommitmentGeometryOverflow {
                    context: "leaf count",
                },
            )?;
            chunks.push(StoredPorChunkCommitmentV1 {
                chunk_index: persistent_u32("por.chunk_index", chunk.chunk_index)?,
                offset: chunk.offset,
                length: chunk.length,
                chunk_digest: chunk.chunk_digest,
                root: chunk.root,
                segment_count: chunk_segments,
                leaf_count: chunk_leaves,
            });
        }
        Ok(Self {
            version: POR_COMMITMENT_VERSION_V1,
            root: *tree.root(),
            payload_len: tree.payload_len(),
            chunk_count: persistent_u32("por.chunk_count", chunks.len())?,
            segment_count,
            leaf_count,
            chunks,
        })
    }

    fn digest(&self) -> Result<[u8; 32], StorageError> {
        let bytes = norito::to_bytes(self)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(POR_COMMITMENT_DIGEST_DOMAIN_V1);
        hasher.update(
            &u64::try_from(bytes.len())
                .map_err(|_| StorageError::LayoutValueTooLarge {
                    field: "por.commitment_bytes",
                    value: bytes.len(),
                    max: u64::MAX,
                })?
                .to_le_bytes(),
        );
        hasher.update(&bytes);
        Ok(hasher.finalize().into())
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct ManifestIndex {
    version: u8,
    total_bytes: u64,
    #[norito(default)]
    gc_freed_bytes_total: u64,
    #[norito(default)]
    gc_evictions_total: u64,
    #[norito(default)]
    chunk_refcounts: Vec<ChunkRefcountEntry>,
    entries: Vec<ManifestIndexEntry>,
}

impl Default for ManifestIndex {
    fn default() -> Self {
        Self {
            version: INDEX_VERSION_V1,
            total_bytes: 0,
            gc_freed_bytes_total: 0,
            gc_evictions_total: 0,
            chunk_refcounts: Vec::new(),
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub(crate) struct ChunkRefcountEntry {
    pub(crate) digest: [u8; 32],
    pub(crate) count: u32,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct ManifestIndexEntry {
    manifest_id: String,
    manifest_cid: Vec<u8>,
    manifest_digest: [u8; 32],
    payload_digest: [u8; 32],
    content_length: u64,
    chunk_profile_handle: String,
    chunk_count: u32,
    por_commitment_digest: [u8; 32],
    pdp_commitment_digest: Option<[u8; 32]>,
    stored_at_unix_secs: u64,
    #[norito(default)]
    retention_epoch: u64,
    #[norito(default)]
    retention_source: Option<RetentionSourceV1>,
    #[norito(default)]
    last_access: u64,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredManifestRecord {
    manifest_id: String,
    manifest_cid: Vec<u8>,
    manifest_digest: [u8; 32],
    payload_digest: [u8; 32],
    content_length: u64,
    chunk_profile_handle: String,
    #[norito(default)]
    stripe_layout: Option<DaStripeLayout>,
    stored_at_unix_secs: u64,
    #[norito(default)]
    retention_epoch: u64,
    #[norito(default)]
    retention_source: Option<RetentionSourceV1>,
    #[norito(default)]
    last_access: u64,
    #[norito(default)]
    files: Vec<StoredFileRecordNorito>,
    chunk_files: Vec<StoredChunkRecord>,
    por_commitment: StoredPorCommitmentV1,
    pdp_commitment: Option<PdpCommitmentV1>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredChunkRecord {
    file_name: String,
    offset: u64,
    length: u32,
    digest: [u8; 32],
    #[norito(default)]
    role: Option<StoredChunkRole>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredFileRecordNorito {
    path: Vec<String>,
    offset: u64,
    size: u64,
    first_chunk: u32,
    chunk_count: u32,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct StoredChunkRole {
    role: ChunkRole,
    #[norito(default)]
    group_id: u32,
}

fn reserve_refcount_entries(
    entries: &mut Vec<ChunkRefcountEntry>,
    additional: usize,
) -> Result<(), StorageError> {
    entries.try_reserve(additional).map_err(|_| {
        StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
            context: "chunk reference counts",
            requested: additional,
        })
    })
}

fn try_clone_refcount_entries(
    entries: &[ChunkRefcountEntry],
) -> Result<Vec<ChunkRefcountEntry>, StorageError> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(entries.len()).map_err(|_| {
        StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
            context: "chunk reference count snapshot",
            requested: entries.len(),
        })
    })?;
    cloned.extend_from_slice(entries);
    Ok(cloned)
}

fn try_clone_manifest_index(index: &ManifestIndex) -> Result<ManifestIndex, StorageError> {
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(index.entries.len())
        .map_err(|_| {
            StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                context: "storage index entries",
                requested: index.entries.len(),
            })
        })?;
    for entry in &index.entries {
        entries.push(ManifestIndexEntry {
            manifest_id: try_clone_text(&entry.manifest_id, "storage index manifest id")?,
            manifest_cid: try_clone_bytes(&entry.manifest_cid, "storage index manifest CID")?,
            manifest_digest: entry.manifest_digest,
            payload_digest: entry.payload_digest,
            content_length: entry.content_length,
            chunk_profile_handle: try_clone_text(
                &entry.chunk_profile_handle,
                "storage index chunk profile",
            )?,
            chunk_count: entry.chunk_count,
            por_commitment_digest: entry.por_commitment_digest,
            pdp_commitment_digest: entry.pdp_commitment_digest,
            stored_at_unix_secs: entry.stored_at_unix_secs,
            retention_epoch: entry.retention_epoch,
            retention_source: try_clone_retention_source(entry.retention_source.as_ref())?,
            last_access: entry.last_access,
        });
    }
    Ok(ManifestIndex {
        version: index.version,
        total_bytes: index.total_bytes,
        gc_freed_bytes_total: index.gc_freed_bytes_total,
        gc_evictions_total: index.gc_evictions_total,
        chunk_refcounts: try_clone_refcount_entries(&index.chunk_refcounts)?,
        entries,
    })
}

fn try_clone_bytes(value: &[u8], context: &'static str) -> Result<Vec<u8>, StorageError> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(value.len()).map_err(|_| {
        StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
            context,
            requested: value.len(),
        })
    })?;
    cloned.extend_from_slice(value);
    Ok(cloned)
}

fn try_clone_path_buf(path: &Path, context: &'static str) -> Result<PathBuf, StorageError> {
    let mut cloned = PathBuf::new();
    cloned
        .try_reserve_exact(path.as_os_str().len())
        .map_err(|_| {
            StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                context,
                requested: path.as_os_str().len(),
            })
        })?;
    cloned.push(path);
    Ok(cloned)
}

fn try_clone_chunk_file_record(chunk: &ChunkFileRecord) -> Result<ChunkFileRecord, StorageError> {
    Ok(ChunkFileRecord {
        path: try_clone_path_buf(&chunk.path, "runtime chunk path")?,
        offset: chunk.offset,
        length: chunk.length,
        digest: chunk.digest,
        role: chunk.role,
        group_id: chunk.group_id,
    })
}

fn try_clone_taikai_segment_hint(
    hint: &TaikaiSegmentHint,
) -> Result<TaikaiSegmentHint, StorageError> {
    Ok(TaikaiSegmentHint {
        event: try_clone_text(&hint.event, "Taikai event hint")?,
        stream: try_clone_text(&hint.stream, "Taikai stream hint")?,
        rendition: try_clone_text(&hint.rendition, "Taikai rendition hint")?,
        sequence: hint.sequence,
        payload_len: hint.payload_len,
        payload_digest: hint.payload_digest,
    })
}

fn try_clone_retention_source(
    source: Option<&RetentionSourceV1>,
) -> Result<Option<RetentionSourceV1>, StorageError> {
    let Some(source) = source else {
        return Ok(None);
    };
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(source.sources.len())
        .map_err(|_| {
            StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                context: "retention source kinds",
                requested: source.sources.len(),
            })
        })?;
    sources.extend_from_slice(&source.sources);
    Ok(Some(RetentionSourceV1 {
        version: source.version,
        pin_policy_epoch: source.pin_policy_epoch,
        deal_end_epoch: source.deal_end_epoch,
        governance_cap_epoch: source.governance_cap_epoch,
        effective_epoch: source.effective_epoch,
        sources,
    }))
}

fn increment_refcount(
    entries: &mut Vec<ChunkRefcountEntry>,
    digest: [u8; 32],
    path: &Path,
) -> Result<(), StorageError> {
    match entries.binary_search_by_key(&digest, |entry| entry.digest) {
        Ok(index) => {
            entries[index].count = entries[index]
                .count
                .checked_add(1)
                .ok_or_else(|| corrupt_storage_state(path, "chunk reference count overflow"))?;
        }
        Err(index) => entries.insert(index, ChunkRefcountEntry { digest, count: 1 }),
    }
    Ok(())
}

fn refcount(entries: &[ChunkRefcountEntry], digest: &[u8; 32]) -> Option<u32> {
    entries
        .binary_search_by_key(digest, |entry| entry.digest)
        .ok()
        .map(|index| entries[index].count)
}

fn corrupt_storage_state(path: &Path, reason: impl Into<String>) -> StorageError {
    StorageError::CorruptStorageState {
        path: path.display().to_string(),
        reason: reason.into(),
    }
}

fn acquire_storage_lock(root_dir: &Path) -> Result<File, StorageError> {
    let lock_path = root_dir.join(STORAGE_LOCK_FILE_NAME);
    validate_atomic_output_path(&lock_path)?;
    let before_open = match fs::symlink_metadata(&lock_path) {
        Ok(metadata) => Some(metadata),
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => return Err(StorageError::Io(err)),
    };
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true);
    set_no_follow_flag(&mut options);
    let file = options.open(&lock_path)?;
    let opened_metadata = file.metadata()?;
    if !opened_metadata.is_file() {
        return Err(corrupt_storage_state(
            &lock_path,
            "storage lock must be a regular file",
        ));
    }
    #[cfg(unix)]
    if opened_metadata.nlink() != 1 {
        return Err(corrupt_storage_state(
            &lock_path,
            format!(
                "storage lock must have exactly one hard link, found {}",
                opened_metadata.nlink()
            ),
        ));
    }
    if before_open
        .as_ref()
        .is_some_and(|metadata| !metadata_identifies_same_file(metadata, &opened_metadata))
    {
        return Err(corrupt_storage_state(
            &lock_path,
            "storage lock changed between inspection and open",
        ));
    }
    let after_open = fs::symlink_metadata(&lock_path)?;
    if !metadata_identifies_same_file(&opened_metadata, &after_open) {
        return Err(corrupt_storage_state(
            &lock_path,
            "storage lock path changed while opening",
        ));
    }
    validate_atomic_output_path(&lock_path)?;
    match file.try_lock() {
        Ok(()) => {
            let locked_path_metadata = fs::symlink_metadata(&lock_path)?;
            if !metadata_identifies_same_file(&opened_metadata, &locked_path_metadata) {
                return Err(corrupt_storage_state(
                    &lock_path,
                    "storage lock path changed while locking",
                ));
            }
            validate_atomic_output_path(&lock_path)?;
            Ok(file)
        }
        Err(fs::TryLockError::WouldBlock) => Err(StorageError::StorageDirectoryInUse {
            path: lock_path.display().to_string(),
        }),
        Err(fs::TryLockError::Error(err)) => Err(StorageError::Io(io::Error::new(
            err.kind(),
            format!(
                "failed to lock SoraFS storage directory via `{}`: {err}",
                lock_path.display()
            ),
        ))),
    }
}

#[cfg(unix)]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(unix)]
fn metadata_stable_during_read(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    metadata_identifies_same_file(left, right)
        && left.len() == right.len()
        && left.nlink() == right.nlink()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len()
}

#[cfg(not(unix))]
fn metadata_stable_during_read(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    metadata_identifies_same_file(left, right) && left.modified().ok() == right.modified().ok()
}

fn persistent_u32(field: &'static str, value: usize) -> Result<u32, StorageError> {
    u32::try_from(value).map_err(|_| StorageError::LayoutValueTooLarge {
        field,
        value,
        max: u64::from(u32::MAX),
    })
}

fn is_canonical_manifest_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn transaction_manifest_id(name: &str) -> Option<&str> {
    let manifest_id = name.get(..64)?;
    if !is_canonical_manifest_id(manifest_id) || name.as_bytes().get(64).copied() != Some(b'-') {
        return None;
    }
    let (pid, counter) = name.get(65..)?.split_once('-')?;
    if pid.is_empty()
        || counter.is_empty()
        || !pid.bytes().all(|byte| byte.is_ascii_digit())
        || !counter.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some(manifest_id)
}

fn validate_real_directory(path: &Path) -> Result<(), StorageError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(corrupt_storage_state(
            path,
            "storage transaction entry must be a real directory",
        ));
    }
    Ok(())
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        File::open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

fn rename_and_sync_directories(from: &Path, to: &Path) -> io::Result<()> {
    let from_parent = from
        .parent()
        .ok_or_else(|| io::Error::other("rename source has no parent directory"))?;
    let to_parent = to
        .parent()
        .ok_or_else(|| io::Error::other("rename destination has no parent directory"))?;
    fs::rename(from, to)?;
    sync_directory(from_parent)?;
    if to_parent != from_parent {
        sync_directory(to_parent)?;
    }
    Ok(())
}

fn remove_transaction_directory(path: &Path) -> Result<(), StorageError> {
    validate_real_directory(path)?;
    fs::remove_dir_all(path)?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn clean_stale_ingest_transactions(root_dir: &Path) -> Result<(), StorageError> {
    let staging_root = root_dir.join(INGEST_STAGING_DIR_NAME);
    match fs::symlink_metadata(&staging_root) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(StorageError::Io(err)),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(corrupt_storage_state(
                &staging_root,
                "ingest transaction root must be a real directory",
            ));
        }
        Ok(_) => {}
    }
    for entry in fs::read_dir(&staging_root)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| corrupt_storage_state(&entry.path(), "non-UTF-8 staging entry"))?;
        if transaction_manifest_id(&name).is_none() {
            return Err(corrupt_storage_state(
                &entry.path(),
                "invalid ingest transaction directory name",
            ));
        }
        remove_transaction_directory(&entry.path())?;
    }
    Ok(())
}

fn recover_gc_transactions(
    root_dir: &Path,
    manifests_dir: &Path,
    live_manifest_ids: &BTreeSet<String>,
) -> Result<(), StorageError> {
    let trash_root = root_dir.join(GC_TRASH_DIR_NAME);
    match fs::symlink_metadata(&trash_root) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(StorageError::Io(err)),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(corrupt_storage_state(
                &trash_root,
                "GC transaction root must be a real directory",
            ));
        }
        Ok(_) => {}
    }
    for entry in fs::read_dir(&trash_root)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| corrupt_storage_state(&entry.path(), "non-UTF-8 GC entry"))?;
        let Some(manifest_id) = transaction_manifest_id(&name) else {
            return Err(corrupt_storage_state(
                &entry.path(),
                "invalid GC transaction directory name",
            ));
        };
        validate_real_directory(&entry.path())?;
        let live_path = manifests_dir.join(manifest_id);
        if live_manifest_ids.contains(manifest_id) {
            match fs::symlink_metadata(&live_path) {
                Err(err) if err.kind() == io::ErrorKind::NotFound => {
                    fs::rename(entry.path(), &live_path)?;
                    sync_directory(manifests_dir)?;
                    sync_directory(&trash_root)?;
                }
                Err(err) => return Err(StorageError::Io(err)),
                Ok(_) => {
                    return Err(corrupt_storage_state(
                        &entry.path(),
                        "indexed manifest exists both live and in a GC transaction",
                    ));
                }
            }
        } else {
            remove_transaction_directory(&entry.path())?;
        }
    }
    Ok(())
}

fn clean_unindexed_manifests(
    manifests_dir: &Path,
    live_manifest_ids: &BTreeSet<String>,
) -> Result<(), StorageError> {
    for entry in fs::read_dir(manifests_dir)? {
        let entry = entry?;
        let manifest_id = entry
            .file_name()
            .into_string()
            .map_err(|_| corrupt_storage_state(&entry.path(), "non-UTF-8 manifest directory"))?;
        if !is_canonical_manifest_id(&manifest_id) {
            return Err(corrupt_storage_state(
                &entry.path(),
                "invalid manifest directory name",
            ));
        }
        validate_real_directory(&entry.path())?;
        if !live_manifest_ids.contains(&manifest_id) {
            remove_transaction_directory(&entry.path())?;
        }
    }
    Ok(())
}

fn read_bounded_regular_file(path: &Path, max_len: u64) -> Result<Vec<u8>, StorageError> {
    let before_open = fs::symlink_metadata(path)?;
    validate_bounded_file_metadata(path, &before_open, max_len)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let mut file = options.open(path)?;
    let opened_metadata = file.metadata()?;
    validate_bounded_file_metadata(path, &opened_metadata, max_len)?;
    if !metadata_identifies_same_file(&before_open, &opened_metadata) {
        return Err(corrupt_storage_state(
            path,
            "artifact changed between inspection and open",
        ));
    }
    let length = usize::try_from(opened_metadata.len()).map_err(|_| {
        corrupt_storage_state(path, "artifact length is not representable on this host")
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| {
        corrupt_storage_state(
            path,
            format!("failed to allocate {length} bytes for bounded artifact"),
        )
    })?;
    bytes.resize(length, 0);
    file.read_exact(&mut bytes)?;
    let mut trailing = [0_u8; 1];
    if file.read(&mut trailing)? != 0 {
        return Err(corrupt_storage_state(
            path,
            "artifact changed length while it was being read",
        ));
    }
    let after_read_file = file.metadata()?;
    let after_read_path = fs::symlink_metadata(path)?;
    validate_bounded_file_metadata(path, &after_read_path, max_len)?;
    if !metadata_stable_during_read(&opened_metadata, &after_read_file)
        || !metadata_identifies_same_file(&opened_metadata, &after_read_path)
    {
        return Err(corrupt_storage_state(
            path,
            "artifact identity or contents changed while being read",
        ));
    }
    Ok(bytes)
}

fn validate_bounded_file_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    max_len: u64,
) -> Result<(), StorageError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(corrupt_storage_state(
            path,
            "artifact is not a regular non-symlink file",
        ));
    }
    if metadata.len() > max_len {
        return Err(corrupt_storage_state(
            path,
            format!(
                "artifact length {} exceeds the {} byte safety limit",
                metadata.len(),
                max_len
            ),
        ));
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(corrupt_storage_state(
            path,
            format!(
                "artifact must have exactly one hard link, found {}",
                metadata.nlink()
            ),
        ));
    }
    Ok(())
}

fn validate_manifest_id(
    manifest_id: &str,
    manifest_digest: &[u8; 32],
    path: &Path,
) -> Result<(), StorageError> {
    if manifest_id != hex::encode(manifest_digest) {
        return Err(corrupt_storage_state(
            path,
            "manifest_id must be the lowercase hex encoding of manifest_digest",
        ));
    }
    Ok(())
}

fn validate_logical_file_path(path: &[String], metadata_path: &Path) -> Result<(), StorageError> {
    for component in path {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains('/')
            || component.contains('\\')
            || component.chars().any(char::is_control)
        {
            return Err(corrupt_storage_state(
                metadata_path,
                "logical file path contains a non-portable component",
            ));
        }
    }
    Ok(())
}

fn validate_persisted_manifest(
    entry: &ManifestIndexEntry,
    record: &StoredManifestRecord,
    manifest: &ManifestV1,
    manifest_bytes: &[u8],
    metadata_path: &Path,
    manifest_path: &Path,
    expected_pdp_sample_window: u16,
) -> Result<(), StorageError> {
    validate_manifest_id(&entry.manifest_id, &entry.manifest_digest, metadata_path)?;
    validate_manifest_id(&record.manifest_id, &record.manifest_digest, metadata_path)?;
    if record.manifest_id != entry.manifest_id
        || record.manifest_digest != entry.manifest_digest
        || record.manifest_cid != entry.manifest_cid
        || record.payload_digest != entry.payload_digest
        || record.content_length != entry.content_length
        || record.chunk_profile_handle != entry.chunk_profile_handle
        || record.stored_at_unix_secs != entry.stored_at_unix_secs
        || record.retention_epoch != entry.retention_epoch
        || record.chunk_files.len() != entry.chunk_count as usize
    {
        return Err(corrupt_storage_state(
            metadata_path,
            "manifest metadata does not match its index entry",
        ));
    }
    if manifest.version != MANIFEST_VERSION_V1 {
        return Err(corrupt_storage_state(
            manifest_path,
            format!("unsupported manifest version {}", manifest.version),
        ));
    }
    let canonical_manifest = manifest.encode().map_err(StorageError::Norito)?;
    if canonical_manifest != manifest_bytes {
        return Err(corrupt_storage_state(
            manifest_path,
            "manifest bytes are not the canonical Norito encoding",
        ));
    }
    let digest: [u8; 32] = blake3::hash(manifest_bytes).into();
    if digest != record.manifest_digest
        || manifest.root_cid != record.manifest_cid
        || manifest.content_length != record.content_length
        || canonical_profile_handle(manifest) != record.chunk_profile_handle
    {
        return Err(corrupt_storage_state(
            manifest_path,
            "manifest payload does not match persisted metadata",
        ));
    }
    validate_persisted_retention(entry, record, manifest, metadata_path)?;
    if record.files.is_empty() {
        return Err(corrupt_storage_state(
            metadata_path,
            "manifest file layout must not be empty",
        ));
    }
    let mut file_offset = 0_u64;
    let mut previous_path: Option<&[String]> = None;
    for file in &record.files {
        validate_logical_file_path(&file.path, metadata_path)?;
        if let Some(previous) = previous_path {
            if previous >= file.path.as_slice() {
                return Err(corrupt_storage_state(
                    metadata_path,
                    "manifest file paths are duplicated or not strictly ordered",
                ));
            }
            if file.path.starts_with(previous) {
                return Err(corrupt_storage_state(
                    metadata_path,
                    "manifest file path descends from another logical file",
                ));
            }
        }
        previous_path = Some(&file.path);
        if record.files.len() > 1 && file.path.is_empty() {
            return Err(corrupt_storage_state(
                metadata_path,
                "only a single-file manifest may use an empty logical path",
            ));
        }
        if file.offset != file_offset {
            return Err(corrupt_storage_state(
                metadata_path,
                "manifest file layout offsets are not contiguous",
            ));
        }
        file_offset = file_offset.checked_add(file.size).ok_or_else(|| {
            corrupt_storage_state(metadata_path, "manifest file layout length overflow")
        })?;
        let chunk_end = u64::from(file.first_chunk) + u64::from(file.chunk_count);
        if chunk_end > record.chunk_files.len() as u64 {
            return Err(corrupt_storage_state(
                metadata_path,
                "manifest file layout references chunks outside the manifest",
            ));
        }
        let first_chunk = usize::try_from(file.first_chunk).map_err(|_| {
            corrupt_storage_state(metadata_path, "file first_chunk is not representable")
        })?;
        let chunk_count = usize::try_from(file.chunk_count).map_err(|_| {
            corrupt_storage_state(metadata_path, "file chunk_count is not representable")
        })?;
        if file.size == 0 {
            if chunk_count != 0 {
                return Err(corrupt_storage_state(
                    metadata_path,
                    "empty logical files must not reference chunks",
                ));
            }
        } else {
            if chunk_count == 0 {
                return Err(corrupt_storage_state(
                    metadata_path,
                    "non-empty logical files must reference chunks",
                ));
            }
            let first = &record.chunk_files[first_chunk];
            let last = &record.chunk_files[first_chunk + chunk_count - 1];
            let file_end = file.offset.checked_add(file.size).ok_or_else(|| {
                corrupt_storage_state(metadata_path, "logical file range overflow")
            })?;
            let chunk_end = last
                .offset
                .checked_add(u64::from(last.length))
                .ok_or_else(|| {
                    corrupt_storage_state(metadata_path, "logical file chunk range overflow")
                })?;
            if first.offset != file.offset || chunk_end != file_end {
                return Err(corrupt_storage_state(
                    metadata_path,
                    "logical file range does not align with its chunk range",
                ));
            }
        }
    }
    if file_offset != record.content_length {
        return Err(corrupt_storage_state(
            metadata_path,
            "manifest file layout length does not match content_length",
        ));
    }

    let mut chunk_offset = 0_u64;
    for (index, chunk) in record.chunk_files.iter().enumerate() {
        let expected_name = format!("chunk_{index:05}.bin");
        if chunk.file_name != expected_name {
            return Err(corrupt_storage_state(
                metadata_path,
                format!("chunk #{index} has noncanonical file name"),
            ));
        }
        if chunk.offset != chunk_offset {
            return Err(corrupt_storage_state(
                metadata_path,
                format!("chunk #{index} has a noncontiguous offset"),
            ));
        }
        chunk_offset = chunk_offset
            .checked_add(u64::from(chunk.length))
            .ok_or_else(|| corrupt_storage_state(metadata_path, "chunk length overflow"))?;
    }
    if chunk_offset != record.content_length {
        return Err(corrupt_storage_state(
            metadata_path,
            "chunk lengths do not match content_length",
        ));
    }

    validate_persisted_por(entry, record, manifest, metadata_path)?;
    validate_persisted_pdp(
        entry,
        record,
        manifest,
        metadata_path,
        expected_pdp_sample_window,
    )?;
    Ok(())
}

fn validate_persisted_retention(
    entry: &ManifestIndexEntry,
    record: &StoredManifestRecord,
    manifest: &ManifestV1,
    metadata_path: &Path,
) -> Result<(), StorageError> {
    let entry_source = entry.retention_source.as_ref().ok_or_else(|| {
        corrupt_storage_state(
            metadata_path,
            "storage index is missing canonical retention source metadata",
        )
    })?;
    let record_source = record.retention_source.as_ref().ok_or_else(|| {
        corrupt_storage_state(
            metadata_path,
            "manifest record is missing canonical retention source metadata",
        )
    })?;
    entry_source.validate().map_err(|error| {
        corrupt_storage_state(
            metadata_path,
            format!("storage index retention source is invalid: {error}"),
        )
    })?;
    record_source.validate().map_err(|error| {
        corrupt_storage_state(
            metadata_path,
            format!("manifest record retention source is invalid: {error}"),
        )
    })?;
    if entry_source != record_source {
        return Err(corrupt_storage_state(
            metadata_path,
            "manifest retention source does not match its storage index entry",
        ));
    }
    if record.retention_epoch != record_source.effective_epoch() {
        return Err(corrupt_storage_state(
            metadata_path,
            "retention epoch does not match the canonical retention source",
        ));
    }
    let expected = RetentionSourceV1::from_manifest(manifest).map_err(|error| {
        corrupt_storage_state(
            metadata_path,
            format!("manifest retention metadata is invalid: {error}"),
        )
    })?;
    if record_source != &expected {
        return Err(corrupt_storage_state(
            metadata_path,
            "persisted retention source does not match the canonical manifest policy",
        ));
    }
    Ok(())
}

fn validate_persisted_por(
    entry: &ManifestIndexEntry,
    record: &StoredManifestRecord,
    manifest: &ManifestV1,
    metadata_path: &Path,
) -> Result<(), StorageError> {
    let commitment = &record.por_commitment;
    if commitment.version != POR_COMMITMENT_VERSION_V1 {
        return Err(corrupt_storage_state(
            metadata_path,
            format!("unsupported PoR commitment version {}", commitment.version),
        ));
    }
    let digest = commitment.digest().map_err(|error| {
        corrupt_storage_state(
            metadata_path,
            format!("failed to digest PoR commitment: {error}"),
        )
    })?;
    if digest != entry.por_commitment_digest {
        return Err(corrupt_storage_state(
            metadata_path,
            "PoR commitment digest does not match the storage index",
        ));
    }
    if commitment.root != manifest.por_root {
        return Err(corrupt_storage_state(
            metadata_path,
            "persisted PoR root does not match the canonical manifest commitment",
        ));
    }
    if commitment.payload_len != record.content_length
        || usize::try_from(commitment.chunk_count).ok() != Some(record.chunk_files.len())
        || commitment.chunks.len() != record.chunk_files.len()
    {
        return Err(corrupt_storage_state(
            metadata_path,
            "PoR commitment geometry does not match manifest chunks",
        ));
    }
    if record.content_length == 0
        && (commitment.chunk_count != 0
            || commitment.segment_count != 0
            || commitment.leaf_count != 0)
    {
        return Err(corrupt_storage_state(
            metadata_path,
            "empty payload has non-empty PoR commitment geometry",
        ));
    }
    let mut segment_count = 0_u64;
    let mut leaf_count = 0_u64;
    for (index, (por_chunk, chunk)) in commitment
        .chunks
        .iter()
        .zip(&record.chunk_files)
        .enumerate()
    {
        if usize::try_from(por_chunk.chunk_index).ok() != Some(index)
            || por_chunk.offset != chunk.offset
            || por_chunk.length != chunk.length
            || por_chunk.chunk_digest != chunk.digest
            || (chunk.length != 0 && (por_chunk.segment_count == 0 || por_chunk.leaf_count == 0))
        {
            return Err(corrupt_storage_state(
                metadata_path,
                format!("PoR chunk commitment #{index} does not match chunk metadata"),
            ));
        }
        segment_count = segment_count
            .checked_add(u64::from(por_chunk.segment_count))
            .ok_or_else(|| corrupt_storage_state(metadata_path, "PoR segment count overflow"))?;
        leaf_count = leaf_count
            .checked_add(u64::from(por_chunk.leaf_count))
            .ok_or_else(|| corrupt_storage_state(metadata_path, "PoR leaf count overflow"))?;
    }
    if segment_count != commitment.segment_count || leaf_count != commitment.leaf_count {
        return Err(corrupt_storage_state(
            metadata_path,
            "PoR commitment aggregate geometry does not match its chunks",
        ));
    }
    Ok(())
}

fn validate_persisted_pdp(
    entry: &ManifestIndexEntry,
    record: &StoredManifestRecord,
    manifest: &ManifestV1,
    metadata_path: &Path,
    expected_sample_window: u16,
) -> Result<(), StorageError> {
    match (&record.pdp_commitment, entry.pdp_commitment_digest) {
        (None, None) => {
            if record.content_length == 0 {
                Ok(())
            } else {
                Err(corrupt_storage_state(
                    metadata_path,
                    "non-empty payload is missing its PDP commitment",
                ))
            }
        }
        (Some(_), _) if record.content_length == 0 => Err(corrupt_storage_state(
            metadata_path,
            "empty payload must not contain a PDP commitment",
        )),
        (None, Some(_)) => {
            let message = if record.content_length == 0 {
                "empty payload index contains an orphan PDP commitment digest"
            } else {
                "non-empty payload is missing its PDP commitment"
            };
            Err(corrupt_storage_state(metadata_path, message))
        }
        (Some(commitment), expected_digest) => {
            commitment.validate().map_err(|error| {
                corrupt_storage_state(metadata_path, format!("invalid PDP commitment: {error}"))
            })?;
            let actual_digest = commitment.commitment_digest().map_err(|error| {
                corrupt_storage_state(
                    metadata_path,
                    format!("failed to digest PDP commitment: {error}"),
                )
            })?;
            if expected_digest != Some(actual_digest) {
                return Err(corrupt_storage_state(
                    metadata_path,
                    "PDP commitment digest does not match the storage index",
                ));
            }
            if commitment.manifest_digest != record.manifest_digest
                || commitment.payload_len != record.content_length
                || commitment.chunk_profile != manifest.chunking
                || commitment.sample_window != expected_sample_window
                || commitment.sealed_at != record.stored_at_unix_secs
            {
                return Err(corrupt_storage_state(
                    metadata_path,
                    "PDP commitment binding, geometry, profile, sample window, or seal time mismatched",
                ));
            }
            Ok(())
        }
    }
}

fn pdp_tree_memory_for_payload(payload_len: u64) -> Result<u64, StorageError> {
    if payload_len == 0 {
        return Ok(0);
    }
    let bytes = estimated_pdp_heap_bytes(payload_len)?;
    u64::try_from(bytes).map_err(|_| StorageError::PdpTree(PdpMerkleTreeError::GeometryOverflow))
}

fn chunk_profile_from_manifest(manifest: &ManifestV1) -> Result<ChunkProfile, StorageError> {
    let profile = &manifest.chunking;
    Ok(ChunkProfile {
        min_size: usize::try_from(profile.min_size)
            .map_err(|_| StorageError::ChunkProfileMismatch)?,
        target_size: usize::try_from(profile.target_size)
            .map_err(|_| StorageError::ChunkProfileMismatch)?,
        max_size: usize::try_from(profile.max_size)
            .map_err(|_| StorageError::ChunkProfileMismatch)?,
        break_mask: u64::from(profile.break_mask),
    })
}

fn rebuild_runtime_trees(
    manifest: &StoredManifest,
    profile: ChunkProfile,
) -> Result<(Arc<PorMerkleTree>, Option<Arc<PdpMerkleTreeV1>>), StorageError> {
    let plan = manifest.try_to_car_plan(profile)?;
    let heap_limit = plan
        .validate()
        .map_err(ChunkStoreError::from)?
        .estimated_ingest_heap_bytes()
        .max(1);
    let mut chunk_store = ChunkStore::with_profile_and_heap_limit(profile, heap_limit)?;
    let mut source = ManifestPayload::new(manifest);
    chunk_store.ingest_plan_source(&plan, &mut source)?;
    let por_tree = Arc::new(chunk_store.take_por_tree());
    let pdp_tree = chunk_store.take_pdp_tree().map(Arc::new);
    Ok((por_tree, pdp_tree))
}

fn validate_rebuilt_por(
    commitment: &StoredPorCommitmentV1,
    tree: &PorMerkleTree,
    metadata_path: &Path,
) -> Result<(), StorageError> {
    let rebuilt = StoredPorCommitmentV1::from_tree(tree).map_err(|error| {
        corrupt_storage_state(
            metadata_path,
            format!("rebuilt PoR commitment is invalid: {error}"),
        )
    })?;
    if rebuilt != *commitment {
        return Err(corrupt_storage_state(
            metadata_path,
            "PoR root or geometry differs from the tree rebuilt from chunk bytes",
        ));
    }
    Ok(())
}

fn validate_rebuilt_pdp(
    commitment: Option<&PdpCommitmentV1>,
    tree: Option<&PdpMerkleTreeV1>,
    metadata_path: &Path,
) -> Result<(), StorageError> {
    match (commitment, tree) {
        (None, None) => Ok(()),
        (None, Some(_)) => Err(corrupt_storage_state(
            metadata_path,
            "rebuilt a PDP tree for metadata without a commitment",
        )),
        (Some(_), None) => Err(corrupt_storage_state(
            metadata_path,
            "failed to rebuild the persisted PDP commitment tree",
        )),
        (Some(commitment), Some(tree)) => {
            let rebuilt = PdpCommitmentV1::from_tree(
                tree,
                commitment.manifest_digest,
                try_clone_chunking_profile(&commitment.chunk_profile)?,
                commitment.sample_window,
                commitment.sealed_at,
            )
            .map_err(|error| {
                corrupt_storage_state(
                    metadata_path,
                    format!("rebuilt PDP commitment is invalid: {error}"),
                )
            })?;
            if rebuilt != *commitment {
                return Err(corrupt_storage_state(
                    metadata_path,
                    "PDP roots or geometry differ from the tree rebuilt from chunk bytes",
                ));
            }
            Ok(())
        }
    }
}

impl StorageBackend {
    /// Create a new storage backend rooted at the directory described by `config`.
    pub fn new(config: StorageConfig) -> Result<Self, StorageError> {
        if config.pdp_sample_window() == 0
            || usize::from(config.pdp_sample_window()) > PDP_MAX_SEGMENT_SAMPLES_V1
        {
            return Err(StorageError::InvalidPdpSampleWindow {
                found: config.pdp_sample_window(),
                maximum: PDP_MAX_SEGMENT_SAMPLES_V1,
            });
        }
        if config.pdp_tree_memory_limit_bytes().0 == 0 {
            return Err(StorageError::PdpTreeMemoryExceeded {
                required: 1,
                available: 0,
            });
        }
        let root_dir = try_clone_path_buf(config.data_dir(), "storage root directory")?;
        let manifests_dir = root_dir.join(MANIFEST_DIR_NAME);
        let index_path = root_dir.join("index.norito");

        validate_atomic_output_path(&root_dir.join(".sorafs-root-probe"))?;
        fs::create_dir_all(&root_dir)?;
        validate_atomic_output_path(&root_dir.join(".sorafs-root-probe"))?;
        let lock_file = acquire_storage_lock(&root_dir)?;
        fs::create_dir_all(&manifests_dir)?;
        validate_real_directory(&manifests_dir)?;

        let mut index = if index_path.exists() {
            let bytes = read_bounded_regular_file(&index_path, MAX_STORAGE_INDEX_BYTES)?;
            let decoded: ManifestIndex = norito::decode_from_bytes_with_limits(
                &bytes,
                storage_decode_limits(MAX_STORAGE_INDEX_BYTES),
            )?;
            if norito::to_bytes(&decoded)? != bytes {
                return Err(corrupt_storage_state(
                    &index_path,
                    "storage index is not the canonical Norito encoding",
                ));
            }
            decoded
        } else {
            ManifestIndex::default()
        };
        if index.version != INDEX_VERSION_V1 {
            return Err(StorageError::UnsupportedIndexVersion {
                version: index.version,
            });
        }
        if index.entries.len() > config.max_pins() {
            return Err(StorageError::PinLimitReached {
                limit: config.max_pins(),
            });
        }
        let mut indexed_manifest_ids = BTreeSet::new();
        for entry in &index.entries {
            validate_manifest_id(&entry.manifest_id, &entry.manifest_digest, &index_path)?;
            if !indexed_manifest_ids
                .insert(try_clone_text(&entry.manifest_id, "indexed manifest id")?)
            {
                return Err(corrupt_storage_state(
                    &index_path,
                    format!("duplicate manifest id {}", entry.manifest_id),
                ));
            }
        }
        recover_gc_transactions(&root_dir, &manifests_dir, &indexed_manifest_ids)?;
        clean_unindexed_manifests(&manifests_dir, &indexed_manifest_ids)?;
        clean_stale_ingest_transactions(&root_dir)?;

        let mut total_bytes = 0_u64;
        let mut pdp_tree_bytes = 0_u64;
        let mut manifests = BTreeMap::new();
        let mut chunk_refcounts = Vec::new();
        let mut access_counter = 0u64;

        for entry in &mut index.entries {
            let manifest_dir = manifests_dir.join(&entry.manifest_id);
            let metadata_path = manifest_dir.join(METADATA_FILE_NAME);
            let manifest_path = manifest_dir.join(MANIFEST_FILE_NAME);

            let metadata_bytes =
                read_bounded_regular_file(&metadata_path, MAX_MANIFEST_METADATA_BYTES)?;
            let mut record: StoredManifestRecord = norito::decode_from_bytes_with_limits(
                &metadata_bytes,
                storage_decode_limits(MAX_MANIFEST_METADATA_BYTES),
            )?;
            if norito::to_bytes(&record)? != metadata_bytes {
                return Err(corrupt_storage_state(
                    &metadata_path,
                    "manifest metadata is not the canonical Norito encoding",
                ));
            }
            let manifest_bytes = read_bounded_regular_file(&manifest_path, MAX_MANIFEST_BYTES)?;
            let manifest: ManifestV1 = norito::decode_from_bytes_with_limits(
                &manifest_bytes,
                storage_decode_limits(MAX_MANIFEST_BYTES),
            )?;
            validate_persisted_manifest(
                entry,
                &record,
                &manifest,
                &manifest_bytes,
                &metadata_path,
                &manifest_path,
                config.pdp_sample_window(),
            )?;
            let last_access = record.last_access.max(entry.last_access);
            record.last_access = last_access;
            entry.last_access = last_access;
            access_counter = access_counter.max(last_access);

            let candidate_pdp_bytes = pdp_tree_memory_for_payload(record.content_length)?;
            let available_pdp_bytes = config
                .pdp_tree_memory_limit_bytes()
                .0
                .saturating_sub(pdp_tree_bytes);
            if candidate_pdp_bytes > available_pdp_bytes {
                return Err(StorageError::PdpTreeMemoryExceeded {
                    required: candidate_pdp_bytes,
                    available: available_pdp_bytes,
                });
            }

            let io_lock = Arc::new(RwLock::new(()));
            let mut stored_manifest = StoredManifest::from_record(
                record,
                manifest_path,
                io_lock,
                ManifestRuntimeProofs {
                    por_commitment_digest: Some(entry.por_commitment_digest),
                    por_tree: Arc::new(PorMerkleTree::empty()),
                    pdp_commitment_digest: entry.pdp_commitment_digest,
                    pdp_tree: None,
                    pdp_tree_memory_bytes: 0,
                },
            )?;
            let profile = chunk_profile_from_manifest(&manifest)?;
            let (rebuilt_por, rebuilt_pdp) = rebuild_runtime_trees(&stored_manifest, profile)
                .map_err(|error| {
                    corrupt_storage_state(
                        &metadata_path,
                        format!("failed to rebuild trees from verified chunk bytes: {error}"),
                    )
                })?;
            validate_rebuilt_por(
                stored_manifest.por_commitment.as_ref().ok_or_else(|| {
                    corrupt_storage_state(&metadata_path, "runtime PoR commitment is missing")
                })?,
                &rebuilt_por,
                &metadata_path,
            )?;
            validate_rebuilt_pdp(
                stored_manifest.pdp_commitment.as_ref(),
                rebuilt_pdp.as_deref(),
                &metadata_path,
            )?;
            stored_manifest.por_tree = rebuilt_por;
            stored_manifest.pdp_tree = rebuilt_pdp;
            stored_manifest.pdp_tree_memory_bytes = candidate_pdp_bytes;
            reserve_refcount_entries(&mut chunk_refcounts, stored_manifest.chunk_files.len())?;
            for chunk in &stored_manifest.chunk_files {
                increment_refcount(&mut chunk_refcounts, chunk.digest, &index_path)?;
            }

            total_bytes = total_bytes
                .checked_add(stored_manifest.content_length)
                .ok_or_else(|| corrupt_storage_state(&index_path, "total byte count overflow"))?;
            pdp_tree_bytes = pdp_tree_bytes
                .checked_add(candidate_pdp_bytes)
                .ok_or_else(|| {
                    corrupt_storage_state(&index_path, "PDP tree byte accounting overflow")
                })?;
            manifests.insert(
                try_clone_text(&entry.manifest_id, "runtime manifest map key")?,
                stored_manifest,
            );
        }

        let max_capacity = config.max_capacity_bytes().0;
        if total_bytes > max_capacity {
            return Err(StorageError::CapacityExceeded {
                required: total_bytes,
                available: max_capacity,
            });
        }

        let mut index_dirty = false;
        if index.chunk_refcounts != chunk_refcounts {
            iroha_logger::warn!(
                stored = index.chunk_refcounts.len(),
                computed = chunk_refcounts.len(),
                "chunk refcount index mismatch; rebuilding from manifests"
            );
            index.chunk_refcounts = try_clone_refcount_entries(&chunk_refcounts)?;
            index_dirty = true;
        }

        if index.total_bytes != total_bytes {
            iroha_logger::warn!(
                stored = index.total_bytes,
                computed = total_bytes,
                "storage byte accounting mismatch; rebuilding from manifests"
            );
            index.total_bytes = total_bytes;
            index_dirty = true;
        }

        if index_dirty {
            let bytes = norito::to_bytes(&index)?;
            ensure_persistent_artifact_size("storage index", &bytes, MAX_STORAGE_INDEX_BYTES)?;
            write_atomic_classified(&index_path, &bytes)
                .map_err(AtomicWriteError::into_storage_error)?;
        }

        let state = StorageState {
            index,
            manifests,
            total_bytes,
            reserved_bytes: 0,
            pdp_tree_bytes,
            reserved_pdp_tree_bytes: 0,
            inflight_manifests: BTreeSet::new(),
            access_counter,
            chunk_refcounts,
        };

        Ok(Self {
            config,
            root_dir,
            manifests_dir,
            index_path,
            _lock_file: lock_file,
            access_metadata_lock: Mutex::new(()),
            persisted_access_counter: AtomicU64::new(access_counter),
            durability_healthy: AtomicBool::new(true),
            durability_failure: Mutex::new(None),
            state: RwLock::new(state),
        })
    }

    /// Returns the number of stored manifests.
    #[must_use]
    pub fn manifest_count(&self) -> usize {
        self.state
            .read()
            .expect("storage state poisoned")
            .manifests
            .len()
    }

    /// Returns the total number of bytes currently stored.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.state
            .read()
            .expect("storage state poisoned")
            .total_bytes
    }

    /// Aggregate bytes retained by canonical runtime PDP node slabs.
    #[must_use]
    pub fn pdp_tree_memory_bytes(&self) -> u64 {
        self.state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pdp_tree_bytes
    }

    /// Bytes currently reserved by in-flight PDP tree builds.
    #[must_use]
    pub fn reserved_pdp_tree_memory_bytes(&self) -> u64 {
        self.state
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reserved_pdp_tree_bytes
    }

    /// Returns the remaining capacity (in bytes) under the configured quota.
    #[must_use]
    pub fn available_capacity(&self) -> u64 {
        let max_capacity = self.config.max_capacity_bytes().0;
        self.state
            .read()
            .expect("storage state poisoned")
            .available_capacity(max_capacity)
    }

    pub(crate) fn ensure_durability_healthy(&self) -> Result<(), StorageError> {
        if self.durability_healthy.load(Ordering::Acquire) {
            return Ok(());
        }
        let reason = self
            .durability_failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| "unknown durability failure".to_owned());
        Err(StorageError::DurabilityPoisoned { reason })
    }

    fn fail_stop_durability(&self, error: &AtomicWriteError) {
        let reason = error.to_string();
        let mut failure = self
            .durability_failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if failure.is_none() {
            *failure = Some(reason);
        }
        self.durability_healthy.store(false, Ordering::Release);
    }

    /// Returns the root directory where manifests are stored.
    #[must_use]
    pub fn root_dir(&self) -> &Path {
        self.root_dir.as_path()
    }

    /// Returns a clone of all stored manifest descriptors.
    #[must_use]
    pub fn manifests(&self) -> Vec<StoredManifest> {
        self.state
            .read()
            .expect("storage state poisoned")
            .manifests
            .values()
            .cloned()
            .collect()
    }

    /// Returns the count of manifests recorded in the on-disk index.
    #[must_use]
    pub(crate) fn index_manifest_count(&self) -> usize {
        self.state
            .read()
            .expect("storage state poisoned")
            .index
            .entries
            .len()
    }

    /// Snapshot of chunk refcounts keyed by digest (sorted by digest).
    #[must_use]
    pub(crate) fn chunk_refcount_snapshot(&self) -> Vec<ChunkRefcountEntry> {
        let state = self.state.read().expect("storage state poisoned");
        state.chunk_refcounts.clone()
    }

    /// Returns the total GC counters tracked in the index.
    #[must_use]
    pub(crate) fn gc_counters(&self) -> (u64, u64) {
        let state = self.state.read().expect("storage state poisoned");
        (
            state.index.gc_freed_bytes_total,
            state.index.gc_evictions_total,
        )
    }

    /// Returns true if any chunks in the manifest are referenced by more than one manifest.
    pub(crate) fn manifest_has_shared_chunks(
        &self,
        manifest_id: &str,
    ) -> Result<bool, StorageError> {
        self.ensure_durability_healthy()?;
        let state = self.state.read().expect("storage state poisoned");
        let manifest =
            state
                .manifests
                .get(manifest_id)
                .ok_or_else(|| StorageError::ManifestNotFound {
                    manifest_id: manifest_id.to_owned(),
                })?;
        for chunk in &manifest.chunk_files {
            if refcount(&state.chunk_refcounts, &chunk.digest).is_some_and(|count| count > 1) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Evict a stored manifest and reclaim its payload bytes.
    pub fn evict_manifest(&self, manifest_id: &str) -> Result<u64, StorageError> {
        self.ensure_durability_healthy()?;
        let mut state = self.state.write().expect("storage state poisoned");
        self.ensure_durability_healthy()?;
        let stored = state
            .manifests
            .get(manifest_id)
            .ok_or_else(|| StorageError::ManifestNotFound {
                manifest_id: manifest_id.to_owned(),
            })?
            .try_clone_runtime()?;
        let _io_guard = stored
            .io_lock
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.ensure_durability_healthy()?;
        let manifest_dir = stored
            .manifest_path()
            .parent()
            .ok_or_else(|| {
                corrupt_storage_state(stored.manifest_path(), "manifest path has no parent")
            })?
            .to_path_buf();
        let mut new_index = try_clone_manifest_index(&state.index)?;
        let mut refcounts = try_clone_refcount_entries(&state.chunk_refcounts)?;
        for chunk in &stored.chunk_files {
            match refcounts.binary_search_by_key(&chunk.digest, |entry| entry.digest) {
                Ok(index) if refcounts[index].count == 1 => {
                    refcounts.remove(index);
                }
                Ok(index) if refcounts[index].count > 1 => {
                    refcounts[index].count -= 1;
                }
                _ => {
                    return Err(corrupt_storage_state(
                        &self.index_path,
                        "manifest chunk is missing a positive reference count",
                    ));
                }
            }
        }
        let entries_before = new_index.entries.len();
        new_index
            .entries
            .retain(|entry| entry.manifest_id != manifest_id);
        if entries_before.checked_sub(new_index.entries.len()) != Some(1) {
            return Err(corrupt_storage_state(
                &self.index_path,
                "manifest index must contain exactly one entry for eviction",
            ));
        }
        new_index.total_bytes = state
            .total_bytes
            .checked_sub(stored.content_length())
            .ok_or_else(|| {
                corrupt_storage_state(
                    &self.index_path,
                    "manifest content length exceeds accounted storage bytes",
                )
            })?;
        new_index.gc_freed_bytes_total = new_index
            .gc_freed_bytes_total
            .checked_add(stored.content_length())
            .ok_or_else(|| {
                corrupt_storage_state(&self.index_path, "GC freed-byte counter overflow")
            })?;
        new_index.gc_evictions_total = new_index
            .gc_evictions_total
            .checked_add(1)
            .ok_or_else(|| corrupt_storage_state(&self.index_path, "GC counter overflow"))?;
        new_index.chunk_refcounts = try_clone_refcount_entries(&refcounts)?;
        let new_pdp_tree_bytes = state
            .pdp_tree_bytes
            .checked_sub(stored.pdp_tree_memory_bytes)
            .ok_or_else(|| {
                corrupt_storage_state(
                    &self.index_path,
                    "evicted manifest exceeds accounted PDP tree bytes",
                )
            })?;

        let index_bytes = norito::to_bytes(&new_index).map_err(StorageError::Norito)?;
        ensure_persistent_artifact_size("storage index", &index_bytes, MAX_STORAGE_INDEX_BYTES)?;
        let trash_path = self.gc_trash_path(manifest_id);
        let trash_root = trash_path.parent().ok_or_else(|| {
            corrupt_storage_state(&trash_path, "GC transaction path has no parent")
        })?;
        validate_atomic_output_path(&trash_root.join(".sorafs-gc-root-probe"))?;
        fs::create_dir_all(trash_root)?;
        validate_real_directory(trash_root)?;
        match fs::symlink_metadata(&trash_path) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(StorageError::Io(err)),
            Ok(_) => {
                return Err(corrupt_storage_state(
                    &trash_path,
                    "GC transaction path already exists",
                ));
            }
        }
        fs::rename(&manifest_dir, &trash_path)?;
        if let Err(primary) =
            sync_directory(&self.manifests_dir).and_then(|()| sync_directory(trash_root))
        {
            let rollback = rename_and_sync_directories(&trash_path, &manifest_dir);
            return match rollback {
                Ok(()) => Err(StorageError::Io(primary)),
                Err(rollback) => {
                    let error = AtomicWriteError::DurabilityUncertain {
                        path: manifest_dir,
                        source: io::Error::other(format!(
                            "failed to sync GC transaction ({primary}); rollback also failed: {rollback}"
                        )),
                    };
                    self.fail_stop_durability(&error);
                    Err(error.into_storage_error())
                }
            };
        }
        let durability_error = match write_atomic_classified(&self.index_path, &index_bytes) {
            Ok(()) => None,
            Err(error @ AtomicWriteError::DurabilityUncertain { .. }) => Some(error),
            Err(AtomicWriteError::BeforeCommit(primary)) => {
                let rollback = rename_and_sync_directories(&trash_path, &manifest_dir);
                return match rollback {
                    Ok(()) => Err(StorageError::Io(primary)),
                    Err(rollback) => {
                        let error = AtomicWriteError::DurabilityUncertain {
                            path: manifest_dir,
                            source: io::Error::other(format!(
                                "failed to persist GC index ({primary}); rollback also failed: {rollback}"
                            )),
                        };
                        self.fail_stop_durability(&error);
                        Err(error.into_storage_error())
                    }
                };
            }
        };

        state.index = new_index;
        state.total_bytes = state.index.total_bytes;
        state.pdp_tree_bytes = new_pdp_tree_bytes;
        state.manifests.remove(manifest_id);
        state.chunk_refcounts = refcounts;
        drop(state);

        if let Some(error) = durability_error {
            self.fail_stop_durability(&error);
            return Err(error.into_storage_error());
        }

        if let Err(err) = remove_transaction_directory(&trash_path) {
            iroha_logger::warn!(
                %err,
                manifest_id = %manifest_id,
                path = %trash_path.display(),
                "failed to purge GC trash directory"
            );
        }

        Ok(stored.content_length())
    }

    /// Persist stripe layout and chunk roles for an existing manifest.
    pub fn attach_stripe_layout(
        &self,
        manifest_id: &str,
        stripe_layout: DaStripeLayout,
        chunk_roles: Vec<ChunkRoleMetadata>,
    ) -> Result<(), StorageError> {
        self.ensure_durability_healthy()?;
        let mut state = self.state.write().expect("storage state poisoned");
        self.ensure_durability_healthy()?;
        let manifest =
            state
                .manifests
                .get_mut(manifest_id)
                .ok_or_else(|| StorageError::ManifestNotFound {
                    manifest_id: manifest_id.to_owned(),
                })?;
        let io_lock = Arc::clone(&manifest.io_lock);
        let _io_guard = io_lock
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.ensure_durability_healthy()?;

        let expected = manifest.chunk_files.len();
        if chunk_roles.len() != expected {
            return Err(StorageError::ChunkRoleLengthMismatch {
                expected,
                actual: chunk_roles.len(),
            });
        }

        let mut updated = manifest.try_clone_runtime()?;
        updated.stripe_layout = Some(stripe_layout);
        for (chunk, role) in updated.chunk_files.iter_mut().zip(chunk_roles.iter()) {
            chunk.role = Some(role.role);
            chunk.group_id = Some(role.group_id);
        }

        let record = updated.to_record()?;
        let metadata_path = updated
            .manifest_path
            .parent()
            .ok_or_else(|| {
                corrupt_storage_state(&updated.manifest_path, "manifest path has no parent")
            })?
            .join(METADATA_FILE_NAME);
        let metadata_bytes = norito::to_bytes(&record)?;
        ensure_persistent_artifact_size(
            "manifest metadata",
            &metadata_bytes,
            MAX_MANIFEST_METADATA_BYTES,
        )?;
        let durability_error = match write_atomic_classified(&metadata_path, &metadata_bytes) {
            Ok(()) => None,
            Err(error @ AtomicWriteError::DurabilityUncertain { .. }) => Some(error),
            Err(AtomicWriteError::BeforeCommit(source)) => return Err(StorageError::Io(source)),
        };
        *manifest = updated;
        if let Some(error) = durability_error {
            self.fail_stop_durability(&error);
            return Err(error.into_storage_error());
        }
        Ok(())
    }

    /// Persist updated file-layout and optional stripe metadata for an existing manifest.
    ///
    /// This supports idempotent re-pinning of the same manifest payload with richer logical-file
    /// metadata, such as upgrading a raw blob pin into a site manifest for the same content CID.
    pub fn attach_plan_metadata(
        &self,
        manifest_id: &str,
        plan: &CarBuildPlan,
        stripe_layout: Option<DaStripeLayout>,
        chunk_roles: Option<Vec<ChunkRoleMetadata>>,
    ) -> Result<(), StorageError> {
        self.ensure_durability_healthy()?;
        let files = stored_files_from_plan(plan)?;
        validate_persistent_file_layout(&files)?;
        let mut state = self.state.write().expect("storage state poisoned");
        self.ensure_durability_healthy()?;
        let manifest =
            state
                .manifests
                .get_mut(manifest_id)
                .ok_or_else(|| StorageError::ManifestNotFound {
                    manifest_id: manifest_id.to_owned(),
                })?;
        let io_lock = Arc::clone(&manifest.io_lock);
        let _io_guard = io_lock
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.ensure_durability_healthy()?;

        if manifest.content_length != plan.content_length
            || manifest.payload_digest != *plan.payload_digest.as_bytes()
        {
            return Err(StorageError::ManifestExists {
                manifest_id: manifest_id.to_owned(),
            });
        }

        let chunks_match = manifest.chunk_files.len() == plan.chunks.len()
            && manifest
                .chunk_files
                .iter()
                .zip(&plan.chunks)
                .all(|(stored, planned)| {
                    stored.offset == planned.offset
                        && stored.length == planned.length
                        && stored.digest == planned.digest
                });
        if !chunks_match {
            return Err(StorageError::ManifestExists {
                manifest_id: manifest_id.to_owned(),
            });
        }

        let mut updated = manifest.try_clone_runtime()?;
        if let Some(roles) = chunk_roles {
            let expected = manifest.chunk_files.len();
            if roles.len() != expected {
                return Err(StorageError::ChunkRoleLengthMismatch {
                    expected,
                    actual: roles.len(),
                });
            }
            for (chunk, role) in updated.chunk_files.iter_mut().zip(roles.iter()) {
                chunk.role = Some(role.role);
                chunk.group_id = Some(role.group_id);
            }
        }
        if let Some(layout) = stripe_layout {
            updated.stripe_layout = Some(layout);
        }
        updated.files = files;

        let record = updated.to_record()?;
        let metadata_path = updated
            .manifest_path
            .parent()
            .ok_or_else(|| {
                corrupt_storage_state(&updated.manifest_path, "manifest path has no parent")
            })?
            .join(METADATA_FILE_NAME);
        let metadata_bytes = norito::to_bytes(&record)?;
        ensure_persistent_artifact_size(
            "manifest metadata",
            &metadata_bytes,
            MAX_MANIFEST_METADATA_BYTES,
        )?;
        let durability_error = match write_atomic_classified(&metadata_path, &metadata_bytes) {
            Ok(()) => None,
            Err(error @ AtomicWriteError::DurabilityUncertain { .. }) => Some(error),
            Err(AtomicWriteError::BeforeCommit(source)) => return Err(StorageError::Io(source)),
        };
        *manifest = updated;
        if let Some(error) = durability_error {
            self.fail_stop_durability(&error);
            return Err(error.into_storage_error());
        }
        Ok(())
    }

    /// Ingest a manifest payload using the provided build plan and payload stream.
    ///
    /// The manifest bytes are encoded using Norito to ensure canonical hashing.
    /// Chunk data is written to `<data_dir>/manifests/<manifest_id>/chunks/chunk_{idx}.bin`.
    ///
    /// # Errors
    ///
    /// Returns an error if quota limits are exceeded, chunk digests do not match,
    /// manifest metadata is inconsistent with the provided plan, or persistence
    /// fails.
    pub fn ingest_manifest<R: Read>(
        &self,
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
        reader: &mut R,
    ) -> Result<String, StorageError> {
        self.ingest_manifest_with_layout(manifest, plan, reader, None, None)
    }

    /// Ingest a manifest while persisting optional stripe layout and chunk-role annotations.
    ///
    /// The `chunk_roles` vector, when provided, must align with `plan.chunks` in length so each
    /// chunk can be annotated deterministically.
    pub fn ingest_manifest_with_layout<R: Read>(
        &self,
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
        reader: &mut R,
        stripe_layout: Option<DaStripeLayout>,
        chunk_roles: Option<Vec<ChunkRoleMetadata>>,
    ) -> Result<String, StorageError> {
        self.ensure_durability_healthy()?;
        if manifest.version != MANIFEST_VERSION_V1 {
            return Err(StorageError::UnsupportedManifestVersion {
                version: manifest.version,
            });
        }
        plan.validate().map_err(ChunkStoreError::from)?;

        let manifest_bytes = manifest.encode()?;
        ensure_persistent_artifact_size("manifest", &manifest_bytes, MAX_MANIFEST_BYTES)?;
        let manifest_digest: [u8; 32] = blake3::hash(&manifest_bytes).into();
        let manifest_id = hex::encode(manifest_digest);
        let inflight_manifest_id = try_clone_text(&manifest_id, "in-flight manifest id")?;
        let reservation_manifest_id =
            try_clone_text(&manifest_id, "ingest reservation manifest id")?;
        let runtime_manifest_id = try_clone_text(&manifest_id, "runtime manifest map key")?;
        let payload_digest = *plan.payload_digest.as_bytes();
        let required_bytes = plan.content_length;

        ensure_chunk_profile_match(manifest, plan)?;
        if manifest.content_length != plan.content_length {
            return Err(StorageError::ManifestContentLengthMismatch);
        }
        let required_pdp_tree_bytes = pdp_tree_memory_for_payload(plan.content_length)?;

        let mut state = self
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.ensure_durability_healthy()?;

        if state.manifests.contains_key(&manifest_id)
            || state.inflight_manifests.contains(&manifest_id)
        {
            return Err(StorageError::ManifestExists {
                manifest_id: manifest_id.clone(),
            });
        }

        let reserved_manifest_count = state
            .manifests
            .len()
            .checked_add(state.inflight_manifests.len())
            .ok_or(StorageError::PinLimitReached {
                limit: self.config.max_pins(),
            })?;
        if reserved_manifest_count >= self.config.max_pins() {
            return Err(StorageError::PinLimitReached {
                limit: self.config.max_pins(),
            });
        }

        let max_capacity = self.config.max_capacity_bytes().0;
        if required_bytes > state.available_capacity(max_capacity) {
            return Err(StorageError::CapacityExceeded {
                required: required_bytes,
                available: state.available_capacity(max_capacity),
            });
        }

        let new_reserved_bytes = state.reserved_bytes.checked_add(required_bytes).ok_or(
            StorageError::CapacityExceeded {
                required: required_bytes,
                available: state.available_capacity(max_capacity),
            },
        )?;
        let pdp_tree_memory_limit = self.config.pdp_tree_memory_limit_bytes().0;
        let available_pdp_tree_memory = state.available_pdp_tree_memory(pdp_tree_memory_limit);
        if required_pdp_tree_bytes > available_pdp_tree_memory {
            return Err(StorageError::PdpTreeMemoryExceeded {
                required: required_pdp_tree_bytes,
                available: available_pdp_tree_memory,
            });
        }
        let new_reserved_pdp_tree_bytes = state
            .reserved_pdp_tree_bytes
            .checked_add(required_pdp_tree_bytes)
            .ok_or(StorageError::PdpTreeMemoryExceeded {
                required: required_pdp_tree_bytes,
                available: available_pdp_tree_memory,
            })?;
        state.reserved_bytes = new_reserved_bytes;
        state.reserved_pdp_tree_bytes = new_reserved_pdp_tree_bytes;
        state.inflight_manifests.insert(inflight_manifest_id);
        drop(state);

        let mut reservation = IngestReservation {
            backend: self,
            manifest_id: reservation_manifest_id,
            reserved_bytes: required_bytes,
            reserved_pdp_tree_bytes: required_pdp_tree_bytes,
            active: true,
        };

        let manifest_dir = self.manifests_dir.join(&manifest_id);
        let staging_dir = self.ingest_staging_path(&manifest_id);
        let mut staging_guard = StagingDirectory::new(try_clone_path_buf(
            &staging_dir,
            "ingest staging directory",
        )?);
        prepare_ingest_staging_directory(&staging_dir)?;
        let chunks_dir = staging_dir.join(CHUNKS_DIR_NAME);
        let staged_manifest_path = staging_dir.join(MANIFEST_FILE_NAME);
        let metadata_path = staging_dir.join(METADATA_FILE_NAME);

        let IngestedPayload {
            mut chunk_records,
            por_tree,
            pdp_tree,
        } = self.ingest_payload(plan, reader, &chunks_dir)?;
        ensure_manifest_por_root(manifest, &por_tree)?;

        if let Some(roles) = chunk_roles {
            let expected = chunk_records.len();
            if roles.len() != expected {
                return Err(StorageError::ChunkRoleLengthMismatch {
                    expected,
                    actual: roles.len(),
                });
            }
            for (record, role) in chunk_records.iter_mut().zip(roles) {
                record.role = Some(StoredChunkRole {
                    role: role.role,
                    group_id: role.group_id,
                });
            }
        }

        write_atomic(&staged_manifest_path, &manifest_bytes)?;

        let stored_at_unix_secs = unix_timestamp()?;
        let pdp_commitment = match pdp_tree.as_deref() {
            Some(tree) => Some(PdpCommitmentV1::from_tree(
                tree,
                manifest_digest,
                try_clone_chunking_profile(&manifest.chunking)?,
                self.config.pdp_sample_window(),
                stored_at_unix_secs,
            )?),
            None if plan.content_length == 0 => None,
            None => {
                return Err(StorageError::PdpTree(PdpMerkleTreeError::CorruptTree));
            }
        };
        let pdp_commitment_digest = pdp_commitment
            .as_ref()
            .map(PdpCommitmentV1::commitment_digest)
            .transpose()?;
        let por_commitment = StoredPorCommitmentV1::from_tree(&por_tree)?;
        let por_commitment_digest = por_commitment.digest()?;
        let retention_source = RetentionSourceV1::from_manifest(manifest)?;
        let retention_epoch = retention_source.effective_epoch();
        let files = stored_files_from_plan(plan)?;
        let persisted_files = persistent_file_records(&files)?;
        let chunk_profile_handle = try_canonical_profile_handle(manifest)?;
        let last_access = {
            let mut state = self.state.write().expect("storage state poisoned");
            self.ensure_durability_healthy()?;
            let next_access = state.access_counter.checked_add(1).ok_or_else(|| {
                corrupt_storage_state(&self.index_path, "manifest access counter overflow")
            })?;
            state.access_counter = next_access;
            next_access
        };
        let chunk_count = persistent_u32("chunk_count", plan.chunks.len())?;

        let metadata_record = StoredManifestRecord {
            manifest_id: try_clone_text(&manifest_id, "stored manifest id")?,
            manifest_cid: try_clone_bytes(&manifest.root_cid, "stored manifest CID")?,
            manifest_digest,
            payload_digest,
            content_length: plan.content_length,
            chunk_profile_handle,
            stripe_layout,
            stored_at_unix_secs,
            retention_epoch,
            retention_source: Some(retention_source),
            last_access,
            files: persisted_files,
            chunk_files: chunk_records,
            por_commitment,
            pdp_commitment,
        };

        write_manifest_metadata(&metadata_record, &metadata_path)?;
        let stored_manifest = StoredManifest::from_record(
            metadata_record,
            manifest_dir.join(MANIFEST_FILE_NAME),
            Arc::new(RwLock::new(())),
            ManifestRuntimeProofs {
                por_commitment_digest: Some(por_commitment_digest),
                por_tree,
                pdp_commitment_digest,
                pdp_tree,
                pdp_tree_memory_bytes: required_pdp_tree_bytes,
            },
        )?;

        let mut state = self
            .state
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.ensure_durability_healthy()?;
        if state.manifests.contains_key(&manifest_id) {
            return Err(StorageError::ManifestExists {
                manifest_id: manifest_id.clone(),
            });
        }

        let mut new_index = try_clone_manifest_index(&state.index)?;
        new_index.total_bytes = state.total_bytes.checked_add(required_bytes).ok_or(
            StorageError::CapacityExceeded {
                required: required_bytes,
                available: state.available_capacity(self.config.max_capacity_bytes().0),
            },
        )?;
        let new_pdp_tree_bytes = state
            .pdp_tree_bytes
            .checked_add(required_pdp_tree_bytes)
            .ok_or_else(|| {
                corrupt_storage_state(&self.index_path, "PDP tree byte accounting overflow")
            })?;
        if new_pdp_tree_bytes > self.config.pdp_tree_memory_limit_bytes().0 {
            return Err(StorageError::PdpTreeMemoryExceeded {
                required: required_pdp_tree_bytes,
                available: state
                    .available_pdp_tree_memory(self.config.pdp_tree_memory_limit_bytes().0),
            });
        }
        let mut refcounts = try_clone_refcount_entries(&state.chunk_refcounts)?;
        reserve_refcount_entries(&mut refcounts, stored_manifest.chunk_files.len())?;
        for record in &stored_manifest.chunk_files {
            increment_refcount(&mut refcounts, record.digest, &self.index_path)?;
        }
        new_index.chunk_refcounts = try_clone_refcount_entries(&refcounts)?;
        new_index.entries.try_reserve(1).map_err(|_| {
            StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                context: "storage index manifest entry",
                requested: 1,
            })
        })?;
        new_index.entries.push(ManifestIndexEntry {
            manifest_id: try_clone_text(&manifest_id, "storage index manifest id")?,
            manifest_cid: try_clone_bytes(
                &stored_manifest.manifest_cid,
                "storage index manifest CID",
            )?,
            manifest_digest,
            payload_digest,
            content_length: plan.content_length,
            chunk_profile_handle: try_clone_text(
                &stored_manifest.chunk_profile_handle,
                "storage index chunk profile",
            )?,
            chunk_count,
            por_commitment_digest,
            pdp_commitment_digest,
            stored_at_unix_secs,
            retention_epoch,
            retention_source: try_clone_retention_source(
                stored_manifest.retention_source.as_ref(),
            )?,
            last_access,
        });

        let index_bytes = match norito::to_bytes(&new_index) {
            Ok(bytes) => bytes,
            Err(err) => {
                drop(state);
                return Err(StorageError::Norito(err));
            }
        };
        ensure_persistent_artifact_size("storage index", &index_bytes, MAX_STORAGE_INDEX_BYTES)?;

        match fs::symlink_metadata(&manifest_dir) {
            Ok(_) => {
                drop(state);
                return Err(StorageError::ManifestExists {
                    manifest_id: manifest_id.clone(),
                });
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => {
                drop(state);
                return Err(StorageError::from(err));
            }
        }
        fs::rename(&staging_dir, &manifest_dir)?;
        let staging_root = staging_dir.parent().ok_or_else(|| {
            corrupt_storage_state(&staging_dir, "ingest transaction path has no parent")
        })?;
        if let Err(primary) =
            sync_directory(&self.manifests_dir).and_then(|()| sync_directory(staging_root))
        {
            let rollback = rename_and_sync_directories(&manifest_dir, &staging_dir);
            return match rollback {
                Ok(()) => Err(StorageError::Io(primary)),
                Err(rollback) => {
                    let error = AtomicWriteError::DurabilityUncertain {
                        path: manifest_dir,
                        source: io::Error::other(format!(
                            "failed to sync ingest transaction ({primary}); rollback also failed: {rollback}"
                        )),
                    };
                    self.fail_stop_durability(&error);
                    Err(error.into_storage_error())
                }
            };
        }
        staging_guard.disarm();

        let durability_error = match write_atomic_classified(&self.index_path, &index_bytes) {
            Ok(()) => None,
            Err(error @ AtomicWriteError::DurabilityUncertain { .. }) => Some(error),
            Err(AtomicWriteError::BeforeCommit(primary)) => {
                drop(state);
                if let Err(cleanup) = remove_transaction_directory(&manifest_dir) {
                    let error = AtomicWriteError::DurabilityUncertain {
                        path: manifest_dir,
                        source: io::Error::other(format!(
                            "index write failed before commit ({primary}); ingest rollback failed: {cleanup}"
                        )),
                    };
                    self.fail_stop_durability(&error);
                    return Err(error.into_storage_error());
                }
                return Err(StorageError::Io(primary));
            }
        };

        let new_total_bytes = new_index.total_bytes;
        state.index = new_index;
        state.total_bytes = new_total_bytes;
        state.pdp_tree_bytes = new_pdp_tree_bytes;
        state.manifests.insert(runtime_manifest_id, stored_manifest);
        state.chunk_refcounts = refcounts;
        reservation.release(&mut state);

        if let Some(error) = durability_error {
            drop(state);
            self.fail_stop_durability(&error);
            return Err(error.into_storage_error());
        }

        Ok(manifest_id)
    }

    /// Returns a clone of the stored manifest metadata, if present.
    #[must_use]
    pub fn manifest(&self, manifest_id: &str) -> Option<StoredManifest> {
        self.state
            .read()
            .expect("storage state poisoned")
            .manifests
            .get(manifest_id)
            .cloned()
    }

    /// Returns a clone of the stored manifest metadata, looked up by digest.
    #[must_use]
    pub fn manifest_by_digest(&self, digest: &[u8; 32]) -> Option<StoredManifest> {
        self.state
            .read()
            .expect("storage state poisoned")
            .manifests
            .values()
            .find(|manifest| manifest.manifest_digest == *digest)
            .cloned()
    }

    /// Run work while holding the manifest lifecycle read lease.
    ///
    /// The state read lock is retained until the lifecycle lease has been
    /// acquired, so eviction cannot remove the manifest between lookup and
    /// lease acquisition. The lifecycle lease remains held for the complete
    /// callback and blocks eviction while repair validates or reads chunk
    /// paths.
    pub(crate) fn with_manifest_io<T>(
        &self,
        manifest_id: &str,
        work: impl FnOnce(&StoredManifest) -> T,
    ) -> Result<T, StorageError> {
        self.ensure_durability_healthy()?;
        let state = self.state.read().expect("storage state poisoned");
        let manifest =
            state
                .manifests
                .get(manifest_id)
                .ok_or_else(|| StorageError::ManifestNotFound {
                    manifest_id: manifest_id.to_owned(),
                })?;
        let io_lock = Arc::clone(&manifest.io_lock);
        let io_guard = io_lock
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.ensure_durability_healthy()?;
        let manifest = manifest.try_clone_runtime()?;
        drop(state);
        let result = work(&manifest);
        drop(io_guard);
        Ok(result)
    }

    /// Run work for the digest-selected manifest under its lifecycle read lease.
    ///
    /// `Ok(None)` is exact proof that the digest was absent while holding the
    /// storage state read lock. Once present, the manifest cannot be evicted
    /// until the callback returns.
    pub(crate) fn with_manifest_io_by_digest<T>(
        &self,
        digest: &[u8; 32],
        work: impl FnOnce(&StoredManifest) -> T,
    ) -> Result<Option<T>, StorageError> {
        self.ensure_durability_healthy()?;
        let state = self.state.read().expect("storage state poisoned");
        let Some(manifest) = state
            .manifests
            .values()
            .find(|manifest| manifest.manifest_digest == *digest)
        else {
            return Ok(None);
        };
        let io_lock = Arc::clone(&manifest.io_lock);
        let io_guard = io_lock
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.ensure_durability_healthy()?;
        let manifest = manifest.try_clone_runtime()?;
        drop(state);
        let result = work(&manifest);
        drop(io_guard);
        Ok(Some(result))
    }

    /// Atomically replace one chunk during lifecycle-leased repair.
    ///
    /// Callers must hold the owning manifest's lifecycle lease for the complete
    /// repair operation. A failure after rename poisons storage durability and
    /// is returned instead of being converted into a successful terminal
    /// repair outcome.
    pub(crate) fn replace_chunk_for_repair(
        &self,
        manifest: &StoredManifest,
        chunk: &ChunkFileRecord,
        bytes: &[u8],
    ) -> Result<(), StorageError> {
        self.replace_chunk_for_repair_with_directory_sync(
            manifest,
            chunk,
            bytes,
            AtomicParentDirectory::sync,
        )
    }

    fn replace_chunk_for_repair_with_directory_sync<F>(
        &self,
        manifest: &StoredManifest,
        chunk: &ChunkFileRecord,
        bytes: &[u8],
        sync_parent: F,
    ) -> Result<(), StorageError>
    where
        F: FnOnce(&AtomicParentDirectory) -> io::Result<()>,
    {
        self.ensure_durability_healthy()?;
        let chunk_index = manifest
            .chunk_files
            .iter()
            .position(|candidate| candidate == chunk)
            .ok_or_else(|| {
                corrupt_storage_state(
                    manifest.manifest_path(),
                    "repair chunk is not bound to the lifecycle-leased manifest",
                )
            })?;
        if bytes.len() != chunk.length as usize || blake3::hash(bytes).as_bytes() != &chunk.digest {
            return Err(StorageError::ChunkDigestMismatch { chunk_index });
        }
        match write_atomic_with_directory_sync(&chunk.path, bytes, sync_parent) {
            Ok(()) => {}
            Err(error @ AtomicWriteError::DurabilityUncertain { .. }) => {
                self.fail_stop_durability(&error);
                return Err(error.into_storage_error());
            }
            Err(AtomicWriteError::BeforeCommit(error)) => return Err(StorageError::Io(error)),
        }
        self.ensure_durability_healthy()
    }

    fn with_manifest_for_access<T, F>(&self, manifest_id: &str, work: F) -> Result<T, StorageError>
    where
        F: FnOnce(&StoredManifest) -> Result<T, StorageError>,
    {
        self.ensure_durability_healthy()?;
        let mut state = self.state.write().expect("storage state poisoned");
        let io_lock = state
            .manifests
            .get(manifest_id)
            .map(|manifest| Arc::clone(&manifest.io_lock))
            .ok_or_else(|| StorageError::ManifestNotFound {
                manifest_id: manifest_id.to_owned(),
            })?;
        let _io_guard = io_lock
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        self.ensure_durability_healthy()?;
        let next_access = state.access_counter.checked_add(1).ok_or_else(|| {
            corrupt_storage_state(&self.index_path, "manifest access counter overflow")
        })?;
        let mut manifest = state
            .manifests
            .get(manifest_id)
            .ok_or_else(|| StorageError::ManifestNotFound {
                manifest_id: manifest_id.to_owned(),
            })?
            .try_clone_runtime()?;
        manifest.last_access = next_access;
        let retention_source = try_clone_retention_source(manifest.retention_source.as_ref())?;
        let record = manifest.to_record()?;
        let metadata_path = manifest
            .manifest_path
            .parent()
            .ok_or_else(|| {
                corrupt_storage_state(
                    &manifest.manifest_path,
                    "manifest path has no parent directory",
                )
            })?
            .join(METADATA_FILE_NAME);
        let mut new_index = try_clone_manifest_index(&state.index)?;
        let entry = new_index
            .entries
            .iter_mut()
            .find(|entry| entry.manifest_id == manifest_id)
            .ok_or_else(|| {
                corrupt_storage_state(
                    &self.index_path,
                    "stored manifest is missing its index entry",
                )
            })?;
        entry.last_access = next_access;
        entry.retention_source = retention_source;
        let metadata_bytes = norito::to_bytes(&record).map_err(StorageError::Norito)?;
        let index_bytes = norito::to_bytes(&new_index).map_err(StorageError::Norito)?;
        ensure_persistent_artifact_size(
            "manifest metadata",
            &metadata_bytes,
            MAX_MANIFEST_METADATA_BYTES,
        )?;
        ensure_persistent_artifact_size("storage index", &index_bytes, MAX_STORAGE_INDEX_BYTES)?;
        let state_manifest = manifest.try_clone_runtime()?;
        let state_entry =
            state
                .manifests
                .get_mut(manifest_id)
                .ok_or_else(|| StorageError::ManifestNotFound {
                    manifest_id: manifest_id.to_owned(),
                })?;
        *state_entry = state_manifest;
        state.access_counter = next_access;
        state.index = new_index;
        drop(state);

        let mut durability_error = None;
        {
            let _metadata_guard = self
                .access_metadata_lock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            self.ensure_durability_healthy()?;
            if next_access > self.persisted_access_counter.load(Ordering::Acquire) {
                let mut persisted = true;
                for (path, bytes, label) in [
                    (
                        metadata_path.as_path(),
                        metadata_bytes.as_slice(),
                        "manifest access metadata",
                    ),
                    (
                        self.index_path.as_path(),
                        index_bytes.as_slice(),
                        "storage index access metadata",
                    ),
                ] {
                    match write_atomic_classified(path, bytes) {
                        Ok(()) => {}
                        Err(error @ AtomicWriteError::DurabilityUncertain { .. }) => {
                            iroha_logger::error!(
                                %error,
                                manifest_id = %manifest_id,
                                "storage fail-stopped after uncertain access-metadata commit"
                            );
                            self.fail_stop_durability(&error);
                            durability_error = Some(error);
                            persisted = false;
                            break;
                        }
                        Err(AtomicWriteError::BeforeCommit(err)) => {
                            iroha_logger::warn!(
                                %err,
                                manifest_id = %manifest_id,
                                %label,
                                "failed to persist storage access metadata"
                            );
                            persisted = false;
                        }
                    }
                }
                if persisted {
                    self.persisted_access_counter
                        .store(next_access, Ordering::Release);
                }
            }
        }

        if let Some(error) = durability_error {
            return Err(error.into_storage_error());
        }
        self.ensure_durability_healthy()?;

        work(&manifest)
    }

    /// Read an exact range from the stored payload.
    pub fn read_payload_range(
        &self,
        manifest_id: &str,
        offset: u64,
        len: usize,
    ) -> Result<Vec<u8>, StorageError> {
        self.ensure_durability_healthy()?;
        if len == 0 {
            let manifest =
                self.manifest(manifest_id)
                    .ok_or_else(|| StorageError::ManifestNotFound {
                        manifest_id: manifest_id.to_owned(),
                    })?;
            if offset > manifest.content_length {
                return Err(StorageError::RangeOutOfBounds {
                    offset,
                    len,
                    content_length: manifest.content_length,
                });
            }
            return Ok(Vec::new());
        }

        self.with_manifest_for_access(manifest_id, |manifest| {
            let len_u64 = u64::try_from(len).map_err(|_| StorageError::RangeOutOfBounds {
                offset,
                len,
                content_length: manifest.content_length,
            })?;
            if offset
                .checked_add(len_u64)
                .map(|end| end > manifest.content_length)
                .unwrap_or(true)
            {
                return Err(StorageError::RangeOutOfBounds {
                    offset,
                    len,
                    content_length: manifest.content_length,
                });
            }

            let mut buffer = Vec::new();
            buffer.try_reserve_exact(len).map_err(|_| {
                StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                    context: "payload range response",
                    requested: len,
                })
            })?;
            buffer.resize(len, 0);
            read_into_manifest(manifest, offset, &mut buffer)?;
            Ok(buffer)
        })
    }

    /// Locate chunk metadata by digest for the provided manifest.
    pub fn chunk_by_digest(
        &self,
        manifest_id: &str,
        digest: &[u8; 32],
    ) -> Result<ChunkFileRecord, StorageError> {
        self.ensure_durability_healthy()?;
        let manifest =
            self.manifest(manifest_id)
                .ok_or_else(|| StorageError::ManifestNotFound {
                    manifest_id: manifest_id.to_owned(),
                })?;

        manifest
            .chunk_files
            .iter()
            .find(|record| record.digest == *digest)
            .cloned()
            .ok_or_else(|| StorageError::ChunkNotFound {
                manifest_id: manifest_id.to_owned(),
                digest_hex: digest.encode_hex::<String>(),
            })
    }

    /// Read the full chunk payload identified by `digest`.
    pub fn read_chunk(
        &self,
        manifest_id: &str,
        digest: &[u8; 32],
    ) -> Result<Vec<u8>, StorageError> {
        self.with_manifest_for_access(manifest_id, |manifest| {
            let (chunk_index, record) = manifest
                .chunk_files
                .iter()
                .enumerate()
                .find(|(_, record)| record.digest == *digest)
                .ok_or_else(|| StorageError::ChunkNotFound {
                    manifest_id: manifest_id.to_owned(),
                    digest_hex: digest.encode_hex::<String>(),
                })?;
            read_verified_chunk(record, chunk_index).map_err(StorageError::ChunkStore)
        })
    }

    /// Sample PoR leaves for the specified manifest.
    pub fn sample_por(
        &self,
        manifest_id: &str,
        count: usize,
        seed: u64,
    ) -> Result<Vec<(usize, PorProof)>, StorageError> {
        self.ensure_durability_healthy()?;
        if count
            > usize::try_from(MAX_PROOF_STREAM_SAMPLE_COUNT)
                .expect("u32 PoR sample ceiling must fit usize")
        {
            return Err(StorageError::PorSampleCountTooLarge {
                requested: count,
                maximum: MAX_PROOF_STREAM_SAMPLE_COUNT,
            });
        }
        if count == 0 {
            if self.manifest(manifest_id).is_none() {
                return Err(StorageError::ManifestNotFound {
                    manifest_id: manifest_id.to_owned(),
                });
            }
            return Ok(Vec::new());
        }

        self.with_manifest_for_access(manifest_id, |manifest| {
            let por_tree = manifest.por_tree_ref();
            let total = por_tree.leaf_count();
            if total == 0 {
                return Ok(Vec::new());
            }

            let target = count.min(total);
            let mut samples = Vec::new();
            samples.try_reserve_exact(target).map_err(|_| {
                StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                    context: "PoR proof samples",
                    requested: target,
                })
            })?;

            for flat_index in PorSampleIndices::new(por_tree.leaf_count_u64(), target, seed)
                .map_err(StorageError::ChunkStore)?
            {
                let leaf_index = usize::try_from(flat_index).map_err(|_| {
                    StorageError::ChunkStore(ChunkStoreError::PorCountOverflow {
                        context: "PoR sampled leaf index host width",
                    })
                })?;
                let (chunk_idx, segment_idx, leaf_idx) =
                    por_tree.leaf_path(leaf_index).ok_or_else(|| {
                        StorageError::ChunkStore(ChunkStoreError::PorInvariant {
                            context: "canonical PoR sample leaf path",
                        })
                    })?;
                let mut payload = ManifestPayload::new(manifest);
                let proof = por_tree
                    .prove_leaf_with(chunk_idx, segment_idx, leaf_idx, &mut payload)
                    .map_err(StorageError::ChunkStore)?;
                let proof = proof.ok_or_else(|| {
                    StorageError::ChunkStore(ChunkStoreError::PorInvariant {
                        context: "canonical PoR sample proof path",
                    })
                })?;
                samples.push((leaf_index, proof));
            }

            Ok(samples)
        })
    }

    /// Build canonical PDP witnesses from verified random-access chunk reads.
    ///
    /// The manifest lifecycle read lease remains held for sample validation, every exact
    /// no-follow chunk read, digest verification, and proof construction. Consequently an
    /// eviction cannot remove or replace the payload while witnesses are being assembled.
    pub fn prove_pdp_samples(
        &self,
        manifest_id: &str,
        samples: &[PdpSampleV1],
    ) -> Result<Vec<PdpProofLeafV1>, StorageError> {
        self.with_manifest_for_access(manifest_id, |manifest| {
            let tree =
                manifest
                    .pdp_tree
                    .as_deref()
                    .ok_or_else(|| StorageError::PdpUnavailable {
                        manifest_id: manifest_id.to_owned(),
                    })?;
            if manifest.pdp_commitment.is_none() || manifest.pdp_commitment_digest.is_none() {
                return Err(corrupt_storage_state(
                    manifest.manifest_path(),
                    "runtime PDP tree is missing its commitment binding",
                ));
            }
            tree.prove_samples_with(samples, |offset, buffer| {
                read_into_manifest(manifest, offset, buffer)?;
                Ok(buffer.len())
            })
            .map_err(pdp_witness_error)
        })
    }

    fn ingest_payload<R: Read>(
        &self,
        plan: &CarBuildPlan,
        reader: &mut R,
        chunks_dir: &Path,
    ) -> Result<IngestedPayload, StorageError> {
        let heap_limit = plan
            .validate()
            .map_err(ChunkStoreError::from)?
            .estimated_ingest_heap_bytes()
            .max(1);
        let mut chunk_store =
            ChunkStore::with_profile_and_heap_limit(plan.chunk_profile, heap_limit)?;
        let output = chunk_store
            .ingest_plan_stream_to_directory(plan, reader, chunks_dir)
            .map_err(StorageError::ChunkStore)?;

        self.ensure_chunk_publication_durable(output.publication, chunks_dir)?;

        if output.total_bytes != plan.content_length {
            return Err(StorageError::PayloadLengthMismatch {
                expected: plan.content_length,
                actual: output.total_bytes,
            });
        }

        let mut records = Vec::new();
        records
            .try_reserve_exact(output.records.len())
            .map_err(|_| {
                StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                    context: "node stored chunk records",
                    requested: output.records.len(),
                })
            })?;
        for record in output.records {
            records.push(StoredChunkRecord {
                file_name: record.file_name,
                offset: record.offset,
                length: record.length,
                digest: record.digest,
                role: None,
            });
        }
        let por_tree = Arc::new(chunk_store.take_por_tree());
        let pdp_tree = chunk_store.take_pdp_tree().map(Arc::new);
        Ok(IngestedPayload {
            chunk_records: records,
            por_tree,
            pdp_tree,
        })
    }

    fn ensure_chunk_publication_durable(
        &self,
        publication: DirectoryPublicationStatus,
        chunks_dir: &Path,
    ) -> Result<(), StorageError> {
        if publication == DirectoryPublicationStatus::PublishedButDurabilityUncertain {
            let error = AtomicWriteError::DurabilityUncertain {
                path: chunks_dir.to_path_buf(),
                source: io::Error::other(
                    "chunk directory was published but its identity or parent durability could not be confirmed",
                ),
            };
            self.fail_stop_durability(&error);
            return Err(error.into_storage_error());
        }
        Ok(())
    }
}

fn pdp_witness_error(error: PdpMerkleReadError<ChunkStoreError>) -> StorageError {
    StorageError::PdpWitness {
        reason: error.to_string(),
    }
}

impl StoredManifest {
    fn from_record(
        record: StoredManifestRecord,
        manifest_path: PathBuf,
        io_lock: Arc<RwLock<()>>,
        runtime_proofs: ManifestRuntimeProofs,
    ) -> Result<Self, StorageError> {
        let manifest_dir = manifest_path.parent().ok_or_else(|| {
            corrupt_storage_state(&manifest_path, "manifest path has no parent directory")
        })?;
        let StoredManifestRecord {
            manifest_id,
            manifest_cid,
            manifest_digest,
            payload_digest,
            content_length,
            chunk_profile_handle,
            stripe_layout,
            stored_at_unix_secs,
            retention_epoch,
            retention_source,
            last_access,
            files: persistent_files,
            chunk_files: persistent_chunks,
            por_commitment,
            pdp_commitment,
        } = record;
        let mut files = Vec::new();
        files
            .try_reserve_exact(persistent_files.len())
            .map_err(|_| {
                StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                    context: "stored manifest file records",
                    requested: persistent_files.len(),
                })
            })?;
        for file in persistent_files {
            files.push(StoredFileRecord {
                path: file.path,
                offset: file.offset,
                size: file.size,
                first_chunk: usize::try_from(file.first_chunk).map_err(|_| {
                    corrupt_storage_state(
                        &manifest_path,
                        "file first_chunk is not representable on this host",
                    )
                })?,
                chunk_count: usize::try_from(file.chunk_count).map_err(|_| {
                    corrupt_storage_state(
                        &manifest_path,
                        "file chunk_count is not representable on this host",
                    )
                })?,
            });
        }
        let mut chunk_files = Vec::new();
        chunk_files
            .try_reserve_exact(persistent_chunks.len())
            .map_err(|_| {
                StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                    context: "stored manifest chunk records",
                    requested: persistent_chunks.len(),
                })
            })?;
        for chunk in persistent_chunks {
            chunk_files.push(ChunkFileRecord {
                path: manifest_dir.join(CHUNKS_DIR_NAME).join(&chunk.file_name),
                offset: chunk.offset,
                length: chunk.length,
                digest: chunk.digest,
                role: chunk.role.as_ref().map(|role| role.role),
                group_id: chunk.role.as_ref().map(|role| role.group_id),
            });
        }

        Ok(Self {
            manifest_id,
            manifest_cid,
            manifest_digest,
            payload_digest,
            content_length,
            chunk_profile_handle,
            stripe_layout,
            stored_at_unix_secs,
            retention_epoch,
            retention_source,
            last_access,
            files,
            chunk_files,
            por_tree: runtime_proofs.por_tree,
            por_commitment: Some(por_commitment),
            por_commitment_digest: runtime_proofs.por_commitment_digest,
            pdp_commitment,
            pdp_commitment_digest: runtime_proofs.pdp_commitment_digest,
            pdp_tree: runtime_proofs.pdp_tree,
            pdp_tree_memory_bytes: runtime_proofs.pdp_tree_memory_bytes,
            manifest_path,
            io_lock,
        })
    }

    fn to_record(&self) -> Result<StoredManifestRecord, StorageError> {
        let files = persistent_file_records(&self.files)?;
        let mut chunk_files = Vec::new();
        chunk_files
            .try_reserve_exact(self.chunk_files.len())
            .map_err(|_| {
                StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                    context: "persistent chunk records",
                    requested: self.chunk_files.len(),
                })
            })?;
        for chunk in &self.chunk_files {
            let file_name = chunk
                .path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    corrupt_storage_state(
                        &chunk.path,
                        "stored chunk path has no canonical UTF-8 file name",
                    )
                })?;
            let file_name = try_clone_text(file_name, "persistent chunk file name")?;
            chunk_files.push(StoredChunkRecord {
                file_name,
                offset: chunk.offset,
                length: chunk.length,
                digest: chunk.digest,
                role: chunk.role.map(|role| StoredChunkRole {
                    role,
                    group_id: chunk.group_id.unwrap_or(0),
                }),
            });
        }

        Ok(StoredManifestRecord {
            manifest_id: try_clone_text(&self.manifest_id, "manifest id")?,
            manifest_cid: try_clone_bytes(&self.manifest_cid, "manifest CID")?,
            manifest_digest: self.manifest_digest,
            payload_digest: self.payload_digest,
            content_length: self.content_length,
            chunk_profile_handle: try_clone_text(
                &self.chunk_profile_handle,
                "chunk profile handle",
            )?,
            stripe_layout: self.stripe_layout,
            stored_at_unix_secs: self.stored_at_unix_secs,
            retention_epoch: self.retention_epoch,
            retention_source: try_clone_retention_source(self.retention_source.as_ref())?,
            last_access: self.last_access,
            files,
            chunk_files,
            por_commitment: match &self.por_commitment {
                Some(commitment) => try_clone_por_commitment(commitment)?,
                None => StoredPorCommitmentV1::from_tree(self.por_tree.as_ref())?,
            },
            pdp_commitment: try_clone_pdp_commitment(self.pdp_commitment.as_ref())?,
        })
    }
}

fn persistent_file_records(
    files: &[StoredFileRecord],
) -> Result<Vec<StoredFileRecordNorito>, StorageError> {
    let mut records = Vec::new();
    records.try_reserve_exact(files.len()).map_err(|_| {
        StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
            context: "persistent file records",
            requested: files.len(),
        })
    })?;
    for file in files {
        records.push(StoredFileRecordNorito {
            path: try_clone_logical_path(&file.path, "persistent logical file path")?,
            offset: file.offset,
            size: file.size,
            first_chunk: persistent_u32("file.first_chunk", file.first_chunk)?,
            chunk_count: persistent_u32("file.chunk_count", file.chunk_count)?,
        });
    }
    Ok(records)
}

fn try_clone_text(value: &str, context: &'static str) -> Result<String, StorageError> {
    let mut cloned = String::new();
    cloned.try_reserve_exact(value.len()).map_err(|_| {
        StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
            context,
            requested: value.len(),
        })
    })?;
    cloned.push_str(value);
    Ok(cloned)
}

fn try_clone_logical_path(
    path: &[String],
    context: &'static str,
) -> Result<Vec<String>, StorageError> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(path.len()).map_err(|_| {
        StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
            context,
            requested: path.len(),
        })
    })?;
    for component in path {
        cloned.push(try_clone_text(component, context)?);
    }
    Ok(cloned)
}

fn try_clone_chunking_profile(
    profile: &sorafs_manifest::ChunkingProfileV1,
) -> Result<sorafs_manifest::ChunkingProfileV1, StorageError> {
    let mut aliases = Vec::new();
    aliases
        .try_reserve_exact(profile.aliases.len())
        .map_err(|_| {
            StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                context: "PDP chunk profile aliases",
                requested: profile.aliases.len(),
            })
        })?;
    for alias in &profile.aliases {
        aliases.push(try_clone_text(alias, "PDP chunk profile alias")?);
    }
    Ok(sorafs_manifest::ChunkingProfileV1 {
        profile_id: profile.profile_id,
        namespace: try_clone_text(&profile.namespace, "PDP chunk profile namespace")?,
        name: try_clone_text(&profile.name, "PDP chunk profile name")?,
        semver: try_clone_text(&profile.semver, "PDP chunk profile semver")?,
        min_size: profile.min_size,
        target_size: profile.target_size,
        max_size: profile.max_size,
        break_mask: profile.break_mask,
        multihash_code: profile.multihash_code,
        aliases,
    })
}

fn try_clone_pdp_commitment(
    commitment: Option<&PdpCommitmentV1>,
) -> Result<Option<PdpCommitmentV1>, StorageError> {
    let Some(commitment) = commitment else {
        return Ok(None);
    };
    Ok(Some(PdpCommitmentV1 {
        version: commitment.version,
        manifest_digest: commitment.manifest_digest,
        chunk_profile: try_clone_chunking_profile(&commitment.chunk_profile)?,
        payload_len: commitment.payload_len,
        hot_leaf_size: commitment.hot_leaf_size,
        segment_size: commitment.segment_size,
        hot_leaf_count: commitment.hot_leaf_count,
        segment_count: commitment.segment_count,
        commitment_root_hot: commitment.commitment_root_hot,
        commitment_root_segment: commitment.commitment_root_segment,
        hash_algorithm: commitment.hash_algorithm,
        hot_tree_height: commitment.hot_tree_height,
        segment_tree_height: commitment.segment_tree_height,
        sample_window: commitment.sample_window,
        sealed_at: commitment.sealed_at,
    }))
}

fn try_clone_por_commitment(
    commitment: &StoredPorCommitmentV1,
) -> Result<StoredPorCommitmentV1, StorageError> {
    let mut chunks = Vec::new();
    chunks
        .try_reserve_exact(commitment.chunks.len())
        .map_err(|_| {
            StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
                context: "PoR commitment chunks",
                requested: commitment.chunks.len(),
            })
        })?;
    chunks.extend_from_slice(&commitment.chunks);
    Ok(StoredPorCommitmentV1 {
        version: commitment.version,
        root: commitment.root,
        payload_len: commitment.payload_len,
        chunk_count: commitment.chunk_count,
        segment_count: commitment.segment_count,
        leaf_count: commitment.leaf_count,
        chunks,
    })
}

fn validate_persistent_file_layout(files: &[StoredFileRecord]) -> Result<(), StorageError> {
    for file in files {
        persistent_u32("file.first_chunk", file.first_chunk)?;
        persistent_u32("file.chunk_count", file.chunk_count)?;
    }
    Ok(())
}

fn stored_files_from_plan(plan: &CarBuildPlan) -> Result<Vec<StoredFileRecord>, StorageError> {
    if plan.files.is_empty() {
        return Err(StorageError::InvalidFileLayout {
            reason: "file inventory must not be empty".to_owned(),
        });
    }
    let mut offset = 0u64;
    let mut previous_path: Option<&[String]> = None;
    let mut records = Vec::new();
    records.try_reserve_exact(plan.files.len()).map_err(|_| {
        StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
            context: "stored file records",
            requested: plan.files.len(),
        })
    })?;
    for file in &plan.files {
        if file.path.iter().any(|component| {
            component.is_empty()
                || component == "."
                || component == ".."
                || component.contains('/')
                || component.contains('\\')
                || component.chars().any(char::is_control)
        }) {
            return Err(StorageError::InvalidFileLayout {
                reason: "logical file path contains a non-portable component".to_owned(),
            });
        }
        if plan.files.len() > 1 && file.path.is_empty() {
            return Err(StorageError::InvalidFileLayout {
                reason: "only a single-file plan may use an empty logical path".to_owned(),
            });
        }
        if let Some(previous) = previous_path {
            if previous >= file.path.as_slice() {
                return Err(StorageError::InvalidFileLayout {
                    reason: "logical file paths must be strictly ordered and unique".to_owned(),
                });
            }
            if file.path.starts_with(previous) {
                return Err(StorageError::InvalidFileLayout {
                    reason: "logical file path descends from another file".to_owned(),
                });
            }
        }
        previous_path = Some(&file.path);

        let file_end =
            offset
                .checked_add(file.size)
                .ok_or_else(|| StorageError::InvalidFileLayout {
                    reason: "logical file byte range overflowed u64".to_owned(),
                })?;
        let chunk_end = file
            .first_chunk
            .checked_add(file.chunk_count)
            .ok_or_else(|| StorageError::InvalidFileLayout {
                reason: "logical file chunk range overflowed usize".to_owned(),
            })?;
        if chunk_end > plan.chunks.len() {
            return Err(StorageError::InvalidFileLayout {
                reason: "logical file references chunks outside the plan".to_owned(),
            });
        }
        if file.size == 0 {
            if file.chunk_count != 0 {
                return Err(StorageError::InvalidFileLayout {
                    reason: "empty logical files must not reference chunks".to_owned(),
                });
            }
        } else {
            if file.chunk_count == 0 {
                return Err(StorageError::InvalidFileLayout {
                    reason: "non-empty logical files must reference chunks".to_owned(),
                });
            }
            let first = &plan.chunks[file.first_chunk];
            let last = &plan.chunks[chunk_end - 1];
            let planned_end = last
                .offset
                .checked_add(u64::from(last.length))
                .ok_or_else(|| StorageError::InvalidFileLayout {
                    reason: "logical file chunk byte range overflowed u64".to_owned(),
                })?;
            if first.offset != offset || planned_end != file_end {
                return Err(StorageError::InvalidFileLayout {
                    reason: "logical file range must align exactly with its chunk range".to_owned(),
                });
            }
        }

        records.push(StoredFileRecord {
            path: try_clone_logical_path(&file.path, "stored logical file path")?,
            offset,
            size: file.size,
            first_chunk: file.first_chunk,
            chunk_count: file.chunk_count,
        });
        offset = file_end;
    }
    if offset != plan.content_length {
        return Err(StorageError::InvalidFileLayout {
            reason: "logical file sizes must equal plan content length".to_owned(),
        });
    }
    Ok(records)
}

fn canonical_profile_handle(manifest: &ManifestV1) -> String {
    format!(
        "{}.{}@{}",
        manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
    )
}

fn try_canonical_profile_handle(manifest: &ManifestV1) -> Result<String, StorageError> {
    let required = manifest
        .chunking
        .namespace
        .len()
        .checked_add(manifest.chunking.name.len())
        .and_then(|length| length.checked_add(manifest.chunking.semver.len()))
        .and_then(|length| length.checked_add(2))
        .ok_or(StorageError::AllocationGeometryOverflow {
            context: "chunk profile handle length",
        })?;
    let mut handle = String::new();
    handle.try_reserve_exact(required).map_err(|_| {
        StorageError::ChunkStore(ChunkStoreError::AllocationFailed {
            context: "chunk profile handle",
            requested: required,
        })
    })?;
    handle.push_str(&manifest.chunking.namespace);
    handle.push('.');
    handle.push_str(&manifest.chunking.name);
    handle.push('@');
    handle.push_str(&manifest.chunking.semver);
    Ok(handle)
}

fn ensure_chunk_profile_match(
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
) -> Result<(), StorageError> {
    let profile = plan.chunk_profile;
    if u32::try_from(profile.min_size).ok() != Some(manifest.chunking.min_size)
        || u32::try_from(profile.target_size).ok() != Some(manifest.chunking.target_size)
        || u32::try_from(profile.max_size).ok() != Some(manifest.chunking.max_size)
        || u32::try_from(profile.break_mask).ok() != Some(manifest.chunking.break_mask)
    {
        return Err(StorageError::ChunkProfileMismatch);
    }
    Ok(())
}

fn ensure_manifest_por_root(
    manifest: &ManifestV1,
    por_tree: &PorMerkleTree,
) -> Result<(), StorageError> {
    if manifest.por_root != *por_tree.root() {
        return Err(StorageError::PorRootMismatch);
    }
    Ok(())
}

fn unix_timestamp() -> Result<u64, StorageError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| StorageError::InvalidSystemTime)?;
    if timestamp == 0 {
        return Err(StorageError::InvalidSystemTime);
    }
    Ok(timestamp)
}

fn prepare_ingest_staging_directory(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("ingest staging path has no parent"))?;
    validate_atomic_output_path(&parent.join(".sorafs-staging-parent-probe"))?;
    fs::create_dir_all(parent)?;
    validate_atomic_output_path(&parent.join(".sorafs-staging-parent-probe"))?;
    fs::create_dir(path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "failed to create unique ingest staging directory `{}`: {err}",
                path.display()
            ),
        )
    })?;
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::other(format!(
            "ingest staging path `{}` must be a real directory",
            path.display()
        )));
    }
    Ok(())
}

#[derive(Debug, Error)]
enum AtomicWriteError {
    #[error("atomic replacement failed before commit: {0}")]
    BeforeCommit(#[source] io::Error),
    #[error(
        "atomic replacement of {path} was renamed but post-commit verification or directory sync failed: {source}"
    )]
    DurabilityUncertain {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl AtomicWriteError {
    fn into_io_error(self) -> io::Error {
        match self {
            Self::BeforeCommit(source) => source,
            Self::DurabilityUncertain { path, source } => io::Error::new(
                source.kind(),
                format!(
                    "atomic replacement of {} was renamed but post-commit verification or directory sync failed: {source}",
                    path.display()
                ),
            ),
        }
    }

    fn into_storage_error(self) -> StorageError {
        match self {
            Self::BeforeCommit(source) => StorageError::Io(source),
            Self::DurabilityUncertain { path, source } => StorageError::DurabilityUncertain {
                path: path.display().to_string(),
                reason: source.to_string(),
            },
        }
    }
}

pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    write_atomic_classified(path, data).map_err(AtomicWriteError::into_io_error)
}

fn write_atomic_classified(path: &Path, data: &[u8]) -> Result<(), AtomicWriteError> {
    write_atomic_with_directory_sync(path, data, AtomicParentDirectory::sync)
}

fn write_atomic_with_directory_sync<F>(
    path: &Path,
    data: &[u8],
    sync_parent: F,
) -> Result<(), AtomicWriteError>
where
    F: FnOnce(&AtomicParentDirectory) -> io::Result<()>,
{
    let parent = path.parent().ok_or_else(|| {
        AtomicWriteError::BeforeCommit(io::Error::other("missing parent directory"))
    })?;
    validate_atomic_output_path(path).map_err(AtomicWriteError::BeforeCommit)?;
    fs::create_dir_all(parent).map_err(|err| {
        AtomicWriteError::BeforeCommit(io::Error::new(
            err.kind(),
            format!(
                "failed to create output parent `{}`: {err}",
                parent.display()
            ),
        ))
    })?;
    validate_atomic_output_path(path).map_err(AtomicWriteError::BeforeCommit)?;
    let parent_directory =
        AtomicParentDirectory::open(parent).map_err(AtomicWriteError::BeforeCommit)?;
    let tmp = atomic_temp_path(path);
    let mut cleanup_identity = None;

    let write_result = (|| -> Result<(), AtomicWriteError> {
        let mut file = open_atomic_temp_file(&tmp).map_err(AtomicWriteError::BeforeCommit)?;
        cleanup_identity = Some(file.metadata().map_err(AtomicWriteError::BeforeCommit)?);
        file.write_all(data)
            .map_err(AtomicWriteError::BeforeCommit)?;
        file.sync_all().map_err(AtomicWriteError::BeforeCommit)?;
        let synced_metadata = file.metadata().map_err(AtomicWriteError::BeforeCommit)?;
        validate_atomic_open_file_identity(
            &tmp,
            &file,
            &synced_metadata,
            data.len(),
            "atomic temporary",
        )
        .map_err(AtomicWriteError::BeforeCommit)?;
        let _publication_guard = parent_directory
            .lock_publication(path)
            .map_err(AtomicWriteError::BeforeCommit)?;
        parent_directory
            .verify_path_identity()
            .map_err(AtomicWriteError::BeforeCommit)?;
        validate_atomic_output_path(path).map_err(AtomicWriteError::BeforeCommit)?;
        validate_atomic_open_file_identity(
            &tmp,
            &file,
            &synced_metadata,
            data.len(),
            "atomic temporary",
        )
        .map_err(AtomicWriteError::BeforeCommit)?;
        parent_directory
            .rename_entry(&tmp, path)
            .map_err(AtomicWriteError::BeforeCommit)?;

        let mut post_commit_error = None;
        retain_first_io_error(
            &mut post_commit_error,
            parent_directory.verify_path_identity(),
        );
        let published_metadata =
            match validate_atomic_published_file_identity(path, &file, data.len()) {
                Ok(metadata) => Some(metadata),
                Err(error) => {
                    if post_commit_error.is_none() {
                        post_commit_error = Some(error);
                    }
                    None
                }
            };
        retain_first_io_error(&mut post_commit_error, sync_parent(&parent_directory));
        retain_first_io_error(
            &mut post_commit_error,
            parent_directory.verify_path_identity(),
        );
        let final_identity = match published_metadata.as_ref() {
            Some(stable) => validate_atomic_open_file_identity(
                path,
                &file,
                stable,
                data.len(),
                "published atomic replacement",
            ),
            None => validate_atomic_published_file_identity(path, &file, data.len()).map(drop),
        };
        retain_first_io_error(&mut post_commit_error, final_identity);
        if let Some(source) = post_commit_error {
            return Err(AtomicWriteError::DurabilityUncertain {
                path: path.to_path_buf(),
                source,
            });
        }
        Ok(())
    })();

    if write_result.is_err() {
        if let Some(identity) = cleanup_identity.as_ref() {
            remove_atomic_temp_if_owned(&tmp, identity);
        }
    }
    write_result
}

fn retain_first_io_error(first: &mut Option<io::Error>, result: io::Result<()>) {
    if let Err(error) = result
        && first.is_none()
    {
        *first = Some(error);
    }
}

struct AtomicParentDirectory {
    path: PathBuf,
    identity: fs::Metadata,
    #[cfg(unix)]
    handle: File,
    #[cfg(unix)]
    anchor: PathBuf,
}

impl AtomicParentDirectory {
    fn open(path: &Path) -> io::Result<Self> {
        validate_atomic_parent_ancestry(path)?;
        #[cfg(unix)]
        {
            let expected = fs::symlink_metadata(path)?;
            validate_atomic_parent_metadata(path, &expected)?;
            let mut options = fs::OpenOptions::new();
            options.read(true);
            set_atomic_parent_open_flags(&mut options);
            let handle = options.open(path).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "failed to open atomic output parent `{}`: {error}",
                        path.display()
                    ),
                )
            })?;
            let identity = handle.metadata()?;
            let linked = fs::symlink_metadata(path)?;
            validate_atomic_parent_metadata(path, &identity)?;
            validate_atomic_parent_metadata(path, &linked)?;
            if !metadata_identifies_same_file(&expected, &identity)
                || !metadata_identifies_same_file(&expected, &linked)
            {
                return Err(io::Error::other(format!(
                    "atomic output parent `{}` changed while opening",
                    path.display()
                )));
            }
            let anchor = atomic_parent_anchor(&handle, &identity)?;
            validate_atomic_parent_anchor(&anchor, &identity)?;
            Ok(Self {
                path: path.to_path_buf(),
                identity,
                handle,
                anchor,
            })
        }
        #[cfg(not(unix))]
        {
            let identity = fs::symlink_metadata(path)?;
            validate_atomic_parent_metadata(path, &identity)?;
            Ok(Self {
                path: path.to_path_buf(),
                identity,
            })
        }
    }

    fn verify_path_identity(&self) -> io::Result<()> {
        validate_atomic_parent_ancestry(&self.path)?;
        let linked = fs::symlink_metadata(&self.path)?;
        validate_atomic_parent_metadata(&self.path, &linked)?;
        if !metadata_identifies_same_file(&self.identity, &linked) {
            return Err(io::Error::other(format!(
                "atomic output parent `{}` changed during replacement",
                self.path.display()
            )));
        }
        #[cfg(unix)]
        {
            let opened = self.handle.metadata()?;
            validate_atomic_parent_metadata(&self.path, &opened)?;
            if !metadata_identifies_same_file(&self.identity, &opened) {
                return Err(io::Error::other(format!(
                    "atomic output parent handle `{}` changed during replacement",
                    self.path.display()
                )));
            }
            validate_atomic_parent_anchor(&self.anchor, &self.identity)?;
        }
        Ok(())
    }

    fn sync(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            self.handle.sync_all()
        }
        #[cfg(not(unix))]
        {
            sync_directory(&self.path)
        }
    }

    fn lock_publication(&self, output: &Path) -> io::Result<MutexGuard<'static, ()>> {
        if output.parent() != Some(self.path.as_path()) {
            return Err(io::Error::other(
                "atomic publication target must belong to the pinned output parent",
            ));
        }
        let mut hasher = DefaultHasher::new();
        #[cfg(unix)]
        {
            self.identity.dev().hash(&mut hasher);
            self.identity.ino().hash(&mut hasher);
        }
        #[cfg(not(unix))]
        self.path.hash(&mut hasher);
        output
            .file_name()
            .ok_or_else(|| io::Error::other("atomic publication target has no file name"))?
            .hash(&mut hasher);
        let shard = usize::try_from(hasher.finish())
            .unwrap_or(usize::MAX)
            .wrapping_rem(ATOMIC_PUBLICATION_LOCK_SHARDS);
        ATOMIC_PUBLICATION_LOCKS[shard]
            .lock()
            .map_err(|_| io::Error::other("atomic publication lock is poisoned"))
    }

    fn rename_entry(&self, from: &Path, to: &Path) -> io::Result<()> {
        if from.parent() != Some(self.path.as_path()) || to.parent() != Some(self.path.as_path()) {
            return Err(io::Error::other(
                "atomic rename entries must share the pinned output parent",
            ));
        }
        #[cfg(unix)]
        {
            let from_name = atomic_entry_name(from)?;
            let to_name = atomic_entry_name(to)?;
            validate_atomic_parent_anchor(&self.anchor, &self.identity)?;
            fs::rename(self.anchor.join(from_name), self.anchor.join(to_name))
        }
        #[cfg(not(unix))]
        {
            fs::rename(from, to)
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn atomic_parent_anchor(handle: &File, _identity: &fs::Metadata) -> io::Result<PathBuf> {
    // `/proc/self/fd` follows the live descriptor rather than the mutable path
    // used to open it. Production Linux deployments therefore fail closed at
    // startup when procfs is unavailable instead of falling back to a racy
    // path-based replacement.
    Ok(Path::new("/proc/self/fd").join(handle.as_raw_fd().to_string()))
}

#[cfg(target_os = "macos")]
fn atomic_parent_anchor(_handle: &File, identity: &fs::Metadata) -> io::Result<PathBuf> {
    // macOS exposes a volume/file-id namespace whose directory identity
    // survives renames while the opened handle keeps the inode alive.
    Ok(Path::new("/.vol")
        .join(identity.dev().to_string())
        .join(identity.ino().to_string()))
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android", target_os = "macos"))
))]
fn atomic_parent_anchor(_handle: &File, _identity: &fs::Metadata) -> io::Result<PathBuf> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "atomic storage replacement requires a stable directory anchor on this platform",
    ))
}

#[cfg(unix)]
fn validate_atomic_parent_anchor(anchor: &Path, identity: &fs::Metadata) -> io::Result<()> {
    let anchored = fs::metadata(anchor).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "failed to resolve pinned atomic output parent `{}`: {error}",
                anchor.display()
            ),
        )
    })?;
    validate_atomic_parent_metadata(anchor, &anchored)?;
    if !metadata_identifies_same_file(identity, &anchored) {
        return Err(io::Error::other(format!(
            "pinned atomic output parent anchor `{}` changed identity",
            anchor.display()
        )));
    }
    Ok(())
}

fn validate_atomic_parent_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::other(format!(
            "atomic output parent `{}` must be a real directory",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn atomic_entry_name(path: &Path) -> io::Result<&std::ffi::OsStr> {
    path.file_name()
        .ok_or_else(|| io::Error::other("atomic rename entry has no file name"))
}

fn validate_atomic_open_file_identity(
    path: &Path,
    file: &File,
    stable: &fs::Metadata,
    expected_len: usize,
    label: &str,
) -> io::Result<()> {
    let opened = file.metadata()?;
    let linked = fs::symlink_metadata(path)?;
    if !opened.is_file() || linked.file_type().is_symlink() || !linked.is_file() {
        return Err(io::Error::other(format!(
            "{label} `{}` must be a regular non-symlink file",
            path.display()
        )));
    }
    let expected_len = u64::try_from(expected_len)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "atomic payload is too large"))?;
    if opened.len() != expected_len || linked.len() != expected_len {
        return Err(io::Error::other(format!(
            "{label} `{}` changed length before commit",
            path.display()
        )));
    }
    #[cfg(unix)]
    if opened.nlink() != 1 || linked.nlink() != 1 {
        return Err(io::Error::other(format!(
            "{label} `{}` must have exactly one hard link",
            path.display()
        )));
    }
    if !metadata_stable_during_read(stable, &opened)
        || !metadata_stable_during_read(&opened, &linked)
    {
        return Err(io::Error::other(format!(
            "{label} `{}` changed identity or metadata before commit",
            path.display()
        )));
    }
    Ok(())
}

fn validate_atomic_published_file_identity(
    path: &Path,
    file: &File,
    expected_len: usize,
) -> io::Result<fs::Metadata> {
    let opened = file.metadata()?;
    let linked = fs::symlink_metadata(path)?;
    if !opened.is_file() || linked.file_type().is_symlink() || !linked.is_file() {
        return Err(io::Error::other(format!(
            "published atomic replacement `{}` must be a regular non-symlink file",
            path.display()
        )));
    }
    let expected_len = u64::try_from(expected_len)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "atomic payload is too large"))?;
    if opened.len() != expected_len || linked.len() != expected_len {
        return Err(io::Error::other(format!(
            "published atomic replacement `{}` changed length after commit",
            path.display()
        )));
    }
    #[cfg(unix)]
    if opened.nlink() != 1 || linked.nlink() != 1 {
        return Err(io::Error::other(format!(
            "published atomic replacement `{}` must have exactly one hard link",
            path.display()
        )));
    }
    if !metadata_identifies_same_file(&opened, &linked) {
        return Err(io::Error::other(format!(
            "published atomic replacement `{}` does not match the committed inode",
            path.display()
        )));
    }
    Ok(opened)
}

fn remove_atomic_temp_if_owned(path: &Path, identity: &fs::Metadata) {
    let Ok(linked) = fs::symlink_metadata(path) else {
        return;
    };
    if linked.file_type().is_symlink()
        || !linked.is_file()
        || !metadata_identifies_same_file(identity, &linked)
    {
        return;
    }
    #[cfg(unix)]
    if linked.nlink() != 1 {
        return;
    }
    let _ = fs::remove_file(path);
}

fn atomic_temp_path(path: &Path) -> PathBuf {
    let pid = std::process::id();
    let counter = ATOMIC_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
    path.with_added_extension(format!("{ATOMIC_EXT}.{pid}.{counter}"))
}

fn open_atomic_temp_file(path: &Path) -> io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    set_no_follow_flag(&mut options);
    let file = options.open(path).map_err(|err| {
        io::Error::new(
            err.kind(),
            format!("failed to create atomic temp `{}`: {err}", path.display()),
        )
    })?;
    let metadata = file.metadata().map_err(|err| {
        io::Error::new(
            err.kind(),
            format!(
                "failed to inspect atomic temp `{}` after open: {err}",
                path.display()
            ),
        )
    })?;
    if !metadata.is_file() {
        return Err(io::Error::other(format!(
            "atomic temp `{}` must be a regular file",
            path.display()
        )));
    }
    Ok(file)
}

fn validate_atomic_output_path(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(io::Error::other(format!(
                    "output `{}` must not be a symlink",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                return Err(io::Error::other(format!(
                    "output `{}` must not be a directory",
                    path.display()
                )));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(io::Error::new(
                err.kind(),
                format!("failed to inspect output `{}`: {err}", path.display()),
            ));
        }
    }

    if let Some(parent) = path.parent() {
        validate_atomic_parent_ancestry(parent)?;
    }
    Ok(())
}

fn validate_atomic_parent_ancestry(parent: &Path) -> io::Result<()> {
    for ancestor in std::iter::once(parent).chain(parent.ancestors().skip(1)) {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(io::Error::other(format!(
                        "output parent `{}` must not be a symlink",
                        ancestor.display()
                    )));
                }
                if !metadata.is_dir() {
                    return Err(io::Error::other(format!(
                        "output parent `{}` must be a directory",
                        ancestor.display()
                    )));
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(io::Error::new(
                    err.kind(),
                    format!(
                        "failed to inspect output parent `{}`: {err}",
                        ancestor.display()
                    ),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_no_follow_flag(options: &mut fs::OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}

#[cfg(unix)]
fn set_atomic_parent_open_flags(options: &mut fs::OpenOptions) {
    options.custom_flags(platform_no_follow_flag() | platform_directory_only_flag());
}

#[cfg(not(unix))]
fn set_no_follow_flag(_options: &mut fs::OpenOptions) {}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_no_follow_flag() -> i32 {
    0o400000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x100
}

#[cfg(target_os = "macos")]
fn platform_no_follow_flag() -> i32 {
    // Unlike O_NOFOLLOW, O_NOFOLLOW_ANY rejects a symlink in every path
    // component during the open syscall and closes the validation/open race.
    0x2000_0000
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn platform_no_follow_flag() -> i32 {
    0
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_directory_only_flag() -> i32 {
    0o200000
}

#[cfg(target_os = "macos")]
fn platform_directory_only_flag() -> i32 {
    0x0010_0000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android", target_os = "macos"))
))]
fn platform_directory_only_flag() -> i32 {
    0
}

fn write_manifest_metadata(
    record: &StoredManifestRecord,
    metadata_path: &Path,
) -> Result<(), StorageError> {
    let metadata_bytes = norito::to_bytes(record)?;
    ensure_persistent_artifact_size(
        "manifest metadata",
        &metadata_bytes,
        MAX_MANIFEST_METADATA_BYTES,
    )?;
    write_atomic(metadata_path, &metadata_bytes)?;
    Ok(())
}

impl StorageBackend {
    fn ingest_staging_path(&self, manifest_id: &str) -> PathBuf {
        let counter = INGEST_STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let name = format!("{manifest_id}-{pid}-{counter}");
        self.root_dir.join(INGEST_STAGING_DIR_NAME).join(name)
    }

    fn gc_trash_path(&self, manifest_id: &str) -> PathBuf {
        let counter = GC_TRASH_COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let name = format!("{manifest_id}-{pid}-{counter}");
        self.root_dir.join(GC_TRASH_DIR_NAME).join(name)
    }
}

struct ManifestPayload<'a> {
    manifest: &'a StoredManifest,
}

impl<'a> ManifestPayload<'a> {
    fn new(manifest: &'a StoredManifest) -> Self {
        Self { manifest }
    }
}

impl PayloadSource for ManifestPayload<'_> {
    fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), ChunkStoreError> {
        read_into_manifest(self.manifest, offset, buf)
    }
}

fn read_into_manifest(
    manifest: &StoredManifest,
    offset: u64,
    buf: &mut [u8],
) -> Result<(), ChunkStoreError> {
    if buf.is_empty() {
        return Ok(());
    }

    let mut remaining = buf.len();
    let mut cursor = 0usize;
    let requested_len =
        u64::try_from(remaining).map_err(|_| ChunkStoreError::OffsetOutOfRange {
            offset,
            len: u64::MAX,
        })?;
    let end = offset
        .checked_add(requested_len)
        .ok_or(ChunkStoreError::OffsetOutOfRange {
            offset,
            len: requested_len,
        })?;

    for (chunk_index, chunk) in manifest.chunk_files.iter().enumerate() {
        let chunk_start = chunk.offset;
        let chunk_end = chunk_start.checked_add(u64::from(chunk.length)).ok_or(
            ChunkStoreError::OffsetOutOfRange {
                offset: chunk_start,
                len: u64::from(chunk.length),
            },
        )?;

        if end <= chunk_start {
            break;
        }
        if offset >= chunk_end {
            continue;
        }

        let read_start = offset.max(chunk_start);
        let read_end = chunk_end.min(end);
        let bytes_to_read = (read_end - read_start) as usize;
        if bytes_to_read == 0 {
            continue;
        }

        let verified = read_verified_chunk(chunk, chunk_index)?;
        let rel_offset = usize::try_from(read_start - chunk_start).map_err(|_| {
            ChunkStoreError::OffsetOutOfRange {
                offset: read_start,
                len: u64::from(chunk.length),
            }
        })?;
        let rel_end =
            rel_offset
                .checked_add(bytes_to_read)
                .ok_or(ChunkStoreError::OffsetOutOfRange {
                    offset: read_start,
                    len: u64::try_from(bytes_to_read).unwrap_or(u64::MAX),
                })?;
        let output_end =
            cursor
                .checked_add(bytes_to_read)
                .ok_or(ChunkStoreError::OffsetOutOfRange {
                    offset: u64::try_from(cursor).unwrap_or(u64::MAX),
                    len: u64::try_from(bytes_to_read).unwrap_or(u64::MAX),
                })?;
        buf[cursor..output_end].copy_from_slice(&verified[rel_offset..rel_end]);

        cursor += bytes_to_read;
        remaining = remaining.saturating_sub(bytes_to_read);

        if remaining == 0 {
            break;
        }
    }

    if remaining != 0 {
        return Err(ChunkStoreError::UnexpectedEof {
            chunk_index: manifest.chunk_files.len().saturating_sub(1),
            expected: buf.len() as u32,
        });
    }

    Ok(())
}

fn read_verified_chunk(
    record: &ChunkFileRecord,
    chunk_index: usize,
) -> Result<Vec<u8>, ChunkStoreError> {
    let expected_len =
        usize::try_from(record.length).map_err(|_| ChunkStoreError::ChunkLengthTooLarge {
            length: usize::MAX,
            limit: u32::MAX,
        })?;
    let before_open = fs::symlink_metadata(&record.path).map_err(ChunkStoreError::Io)?;
    validate_chunk_file_metadata(record, &before_open)?;
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let mut file = options.open(&record.path).map_err(ChunkStoreError::Io)?;
    let opened_metadata = file.metadata().map_err(ChunkStoreError::Io)?;
    validate_chunk_file_metadata(record, &opened_metadata)?;
    if !metadata_identifies_same_file(&before_open, &opened_metadata) {
        return Err(invalid_chunk_file(
            record,
            "changed between inspection and open",
        ));
    }

    #[cfg(test)]
    pause_chunk_read_for_test(&record.path)?;

    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_len)
        .map_err(|_| ChunkStoreError::AllocationFailed {
            context: "verified chunk bytes",
            requested: expected_len,
        })?;
    bytes.resize(expected_len, 0);
    file.read_exact(&mut bytes).map_err(ChunkStoreError::Io)?;
    let mut trailing = [0_u8; 1];
    if file.read(&mut trailing).map_err(ChunkStoreError::Io)? != 0 {
        return Err(ChunkStoreError::LengthMismatch {
            expected: u64::from(record.length),
            actual: u64::from(record.length).saturating_add(1),
        });
    }
    let after_read_file = file.metadata().map_err(ChunkStoreError::Io)?;
    let after_read_path = fs::symlink_metadata(&record.path).map_err(ChunkStoreError::Io)?;
    validate_chunk_file_metadata(record, &after_read_path)?;
    if !metadata_stable_during_read(&opened_metadata, &after_read_file)
        || !metadata_identifies_same_file(&opened_metadata, &after_read_path)
    {
        return Err(invalid_chunk_file(
            record,
            "identity or contents changed while being read",
        ));
    }
    if blake3::hash(&bytes).as_bytes() != &record.digest {
        return Err(ChunkStoreError::DigestMismatch { chunk_index });
    }
    Ok(bytes)
}

/// Read one complete chunk through the same no-follow, single-link, stable
/// inode/metadata, exact-length, and digest checks used by normal payload and
/// PDP witness reads.
pub(crate) fn read_verified_chunk_file(
    record: &ChunkFileRecord,
) -> Result<Vec<u8>, ChunkStoreError> {
    read_verified_chunk(record, 0)
}

#[cfg(test)]
fn pause_chunk_read_for_test(path: &Path) -> Result<(), ChunkStoreError> {
    let hook = {
        let mut guard = CHUNK_READ_TEST_HOOK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if guard.as_ref().is_some_and(|hook| hook.path == path) {
            guard.take()
        } else {
            None
        }
    };
    if let Some(hook) = hook {
        hook.entered.send(()).map_err(|_| {
            ChunkStoreError::Io(io::Error::other("chunk-read test observer dropped"))
        })?;
        hook.release.recv().map_err(|_| {
            ChunkStoreError::Io(io::Error::other("chunk-read test release dropped"))
        })?;
    }
    Ok(())
}

fn validate_chunk_file_metadata(
    record: &ChunkFileRecord,
    metadata: &fs::Metadata,
) -> Result<(), ChunkStoreError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(invalid_chunk_file(
            record,
            "is not a regular non-symlink file",
        ));
    }
    if metadata.len() != u64::from(record.length) {
        return Err(ChunkStoreError::LengthMismatch {
            expected: u64::from(record.length),
            actual: metadata.len(),
        });
    }
    #[cfg(unix)]
    if metadata.nlink() != 1 {
        return Err(invalid_chunk_file(
            record,
            &format!(
                "must have exactly one hard link, found {}",
                metadata.nlink()
            ),
        ));
    }
    Ok(())
}

fn invalid_chunk_file(record: &ChunkFileRecord, reason: &str) -> ChunkStoreError {
    ChunkStoreError::Io(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("chunk path `{}` {reason}", record.path.display()),
    ))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{self, Cursor, Read},
        sync::{Arc, mpsc},
        thread,
        time::{Duration, Instant},
    };

    use blake3;
    use sorafs_car::{CarPlanError, FileEntry, compute_chunk_plan_digest_sha3};
    use sorafs_manifest::{DagCodecId, ManifestBuilder, PinPolicy};
    use tempfile::TempDir;

    use super::*;

    fn temp_config(temp_dir: &TempDir) -> StorageConfig {
        let temp_path = temp_dir.path().canonicalize().expect("canonical tempdir");
        StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_path.join("storage"))
            .build()
    }

    fn temp_config_with_pdp_limit(temp_dir: &TempDir, limit: u64) -> StorageConfig {
        let temp_path = temp_dir.path().canonicalize().expect("canonical tempdir");
        StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_path.join("storage"))
            .pdp_tree_memory_limit_bytes(iroha_config::base::util::Bytes(limit))
            .build()
    }

    fn canonical_temp_path(temp_dir: &TempDir) -> PathBuf {
        temp_dir.path().canonicalize().expect("canonical tempdir")
    }

    fn single_file_plan(bytes: &[u8]) -> Result<CarBuildPlan, CarPlanError> {
        CarBuildPlan::single_file(bytes)
    }

    fn manifest_builder_for_plan(payload: &[u8], plan: &CarBuildPlan) -> ManifestBuilder {
        let heap_limit = plan
            .validate()
            .expect("valid manifest fixture plan")
            .estimated_ingest_heap_bytes()
            .max(1);
        let mut chunk_store =
            ChunkStore::with_profile_and_heap_limit(plan.chunk_profile, heap_limit)
                .expect("bounded manifest fixture chunk store");
        chunk_store
            .ingest_plan(payload, plan)
            .expect("manifest fixture payload matches plan");
        ManifestBuilder::new()
            .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
            .por_root(*chunk_store.por_tree().root())
    }

    fn empty_file_plan() -> CarBuildPlan {
        let plan = CarBuildPlan {
            chunk_profile: ChunkProfile::DEFAULT,
            payload_digest: blake3::hash(&[]),
            content_length: 0,
            chunks: Vec::new(),
            files: vec![FilePlan {
                path: Vec::new(),
                first_chunk: 0,
                chunk_count: 0,
                size: 0,
            }],
        };
        plan.validate().expect("canonical empty plan");
        plan
    }

    fn test_manifest(payload: &[u8], plan: &CarBuildPlan, root_byte: u8) -> ManifestV1 {
        manifest_builder_for_plan(payload, plan)
            .root_cid(vec![root_byte; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest")
    }

    fn ingest_test_payload(
        temp_dir: &TempDir,
        payload: &[u8],
        root_byte: u8,
    ) -> (StorageConfig, StorageBackend, String) {
        let config = temp_config(temp_dir);
        let backend = StorageBackend::new(config.clone()).expect("backend init");
        let plan = single_file_plan(payload).expect("plan");
        let manifest = test_manifest(payload, &plan, root_byte);
        let mut reader = payload;
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        (config, backend, manifest_id)
    }

    fn rewrite_manifest_record(
        backend: &StorageBackend,
        manifest_id: &str,
        mutate: impl FnOnce(&mut StoredManifestRecord),
    ) {
        let metadata_path = backend
            .manifests_dir
            .join(manifest_id)
            .join(METADATA_FILE_NAME);
        let bytes = fs::read(&metadata_path).expect("read manifest metadata");
        let mut record: StoredManifestRecord =
            norito::decode_from_bytes(&bytes).expect("decode manifest metadata");
        mutate(&mut record);
        fs::write(
            &metadata_path,
            norito::to_bytes(&record).expect("encode manifest metadata"),
        )
        .expect("rewrite manifest metadata");
    }

    fn rewrite_manifest_index(backend: &StorageBackend, mutate: impl FnOnce(&mut ManifestIndex)) {
        let bytes = fs::read(&backend.index_path).expect("read manifest index");
        let mut index: ManifestIndex =
            norito::decode_from_bytes(&bytes).expect("decode manifest index");
        mutate(&mut index);
        fs::write(
            &backend.index_path,
            norito::to_bytes(&index).expect("encode manifest index"),
        )
        .expect("rewrite manifest index");
    }

    fn first_pdp_sample() -> Vec<PdpSampleV1> {
        vec![PdpSampleV1 {
            segment_index: 0,
            hot_leaf_indices: vec![0],
        }]
    }

    fn replace_with_empty_index(config: &StorageConfig) {
        let index_path = config.data_dir().join("index.norito");
        let bytes = norito::to_bytes(&ManifestIndex::default()).expect("encode empty index");
        write_atomic(&index_path, &bytes).expect("replace storage index");
    }

    struct GatedReader {
        bytes: Cursor<Vec<u8>>,
        entered: Option<mpsc::Sender<()>>,
        release: mpsc::Receiver<()>,
        fail_after_release: bool,
    }

    impl Read for GatedReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if let Some(entered) = self.entered.take() {
                entered
                    .send(())
                    .map_err(|_| io::Error::other("test gate receiver dropped"))?;
                self.release
                    .recv()
                    .map_err(|_| io::Error::other("test gate sender dropped"))?;
                if self.fail_after_release {
                    return Err(io::Error::other("injected ingest reader failure"));
                }
            }
            self.bytes.read(buffer)
        }
    }

    fn assert_staging_empty(backend: &StorageBackend) {
        let staging_root = backend.root_dir().join(INGEST_STAGING_DIR_NAME);
        if staging_root.exists() {
            assert!(
                fs::read_dir(&staging_root)
                    .expect("read staging root")
                    .next()
                    .is_none(),
                "ingest staging root must not retain attempt directories"
            );
        }
    }

    #[test]
    fn storage_directory_has_single_process_owner() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let config = temp_config(&temp_dir);
        let owner = StorageBackend::new(config.clone()).expect("acquire storage ownership");

        assert!(matches!(
            StorageBackend::new(config.clone()),
            Err(StorageError::StorageDirectoryInUse { .. })
        ));

        drop(owner);
        StorageBackend::new(config).expect("storage ownership releases on drop");
    }

    #[cfg(unix)]
    #[test]
    fn storage_lock_rejects_symlink() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let config = temp_config(&temp_dir);
        fs::create_dir_all(config.data_dir()).expect("create storage root");
        let target = temp_dir.path().join("lock-target");
        fs::write(&target, b"must remain untouched").expect("write lock target");
        std::os::unix::fs::symlink(&target, config.data_dir().join(STORAGE_LOCK_FILE_NAME))
            .expect("create storage lock symlink");

        assert!(matches!(
            StorageBackend::new(config),
            Err(StorageError::Io(_))
        ));
        assert_eq!(
            fs::read(&target).expect("read lock target"),
            b"must remain untouched"
        );
    }

    #[cfg(unix)]
    #[test]
    fn storage_lock_rejects_hard_link() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let config = temp_config(&temp_dir);
        fs::create_dir_all(config.data_dir()).expect("create storage root");
        let target = temp_dir.path().join("lock-target");
        fs::write(&target, b"must remain untouched").expect("write lock target");
        fs::hard_link(&target, config.data_dir().join(STORAGE_LOCK_FILE_NAME))
            .expect("create storage lock hard link");

        assert!(matches!(
            StorageBackend::new(config),
            Err(StorageError::CorruptStorageState { .. })
        ));
        assert_eq!(
            fs::read(&target).expect("read target"),
            b"must remain untouched"
        );
    }

    #[test]
    fn startup_rolls_back_uncommitted_gc_move() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"GC transaction rollback payload";
        let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xA3);
        let manifest_dir = backend.manifests_dir.join(&manifest_id);
        let trash_root = backend.root_dir.join(GC_TRASH_DIR_NAME);
        let trash_path = trash_root.join(format!("{manifest_id}-999-1"));
        drop(backend);

        fs::create_dir_all(&trash_root).expect("create GC transaction root");
        fs::rename(&manifest_dir, &trash_path).expect("simulate pre-index GC move");

        let recovered = StorageBackend::new(config).expect("recover pre-index GC transaction");
        assert!(recovered.manifest(&manifest_id).is_some());
        assert!(manifest_dir.is_dir());
        assert!(!trash_path.exists());
    }

    #[test]
    fn startup_finishes_committed_gc_move() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"GC transaction completion payload";
        let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xA4);
        let manifest_dir = backend.manifests_dir.join(&manifest_id);
        let trash_root = backend.root_dir.join(GC_TRASH_DIR_NAME);
        let trash_path = trash_root.join(format!("{manifest_id}-999-2"));
        drop(backend);

        fs::create_dir_all(&trash_root).expect("create GC transaction root");
        fs::rename(&manifest_dir, &trash_path).expect("simulate committed GC move");
        replace_with_empty_index(&config);

        let recovered = StorageBackend::new(config).expect("finish committed GC transaction");
        assert_eq!(recovered.manifest_count(), 0);
        assert!(!manifest_dir.exists());
        assert!(!trash_path.exists());
    }

    #[test]
    fn startup_rejects_duplicate_live_and_gc_transaction_data() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"ambiguous duplicate GC transaction payload";
        let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xA6);
        let manifest_dir = backend.manifests_dir.join(&manifest_id);
        let trash_path = backend
            .root_dir
            .join(GC_TRASH_DIR_NAME)
            .join(format!("{manifest_id}-999-4"));
        drop(backend);
        fs::create_dir_all(&trash_path).expect("create duplicate GC transaction directory");

        assert!(matches!(
            StorageBackend::new(config),
            Err(StorageError::CorruptStorageState { .. })
        ));
        assert!(manifest_dir.is_dir());
        assert!(trash_path.is_dir());
    }

    #[test]
    fn startup_removes_unindexed_ingest_commit() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"unindexed ingest transaction payload";
        let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xA5);
        let manifest_dir = backend.manifests_dir.join(&manifest_id);
        drop(backend);
        replace_with_empty_index(&config);

        let recovered = StorageBackend::new(config).expect("remove unindexed manifest");
        assert_eq!(recovered.manifest_count(), 0);
        assert!(!manifest_dir.exists());
    }

    #[test]
    fn startup_cleans_stale_ingest_staging_directory() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let config = temp_config(&temp_dir);
        let backend = StorageBackend::new(config.clone()).expect("backend init");
        let staging_path = backend
            .root_dir
            .join(INGEST_STAGING_DIR_NAME)
            .join(format!("{}-999-3", "a".repeat(64)));
        drop(backend);
        fs::create_dir_all(&staging_path).expect("create stale staging directory");
        fs::write(staging_path.join("partial"), b"partial").expect("write staged fragment");

        StorageBackend::new(config).expect("clean stale staging transaction");
        assert!(!staging_path.exists());
    }

    #[test]
    fn startup_rejects_unknown_transaction_entries() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let config = temp_config(&temp_dir);
        let backend = StorageBackend::new(config.clone()).expect("backend init");
        let invalid_path = backend
            .root_dir
            .join(INGEST_STAGING_DIR_NAME)
            .join("unknown");
        drop(backend);
        fs::create_dir_all(&invalid_path).expect("create invalid transaction entry");

        assert!(matches!(
            StorageBackend::new(config),
            Err(StorageError::CorruptStorageState { .. })
        ));
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn persistent_layout_conversion_rejects_truncation() {
        let oversized = usize::try_from(u64::from(u32::MAX) + 1).expect("64-bit usize");
        assert!(matches!(
            persistent_u32("test", oversized),
            Err(StorageError::LayoutValueTooLarge {
                field: "test",
                value,
                max
            }) if value == oversized && max == u64::from(u32::MAX)
        ));
    }

    #[test]
    fn concurrent_same_manifest_ingest_has_single_owner() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let config = temp_config(&temp_dir);
        let backend = Arc::new(StorageBackend::new(config.clone()).expect("backend init"));
        let payload = vec![0xA5; 32 * 1024];
        let plan = single_file_plan(&payload).expect("plan");
        let manifest = test_manifest(&payload, &plan, 0xA1);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let worker_backend = Arc::clone(&backend);
        let worker_manifest = manifest.clone();
        let worker_plan = plan.clone();
        let worker_payload = payload.clone();
        let worker = thread::spawn(move || {
            let mut reader = GatedReader {
                bytes: Cursor::new(worker_payload),
                entered: Some(entered_tx),
                release: release_rx,
                fail_after_release: false,
            };
            worker_backend.ingest_manifest(&worker_manifest, &worker_plan, &mut reader)
        });

        entered_rx.recv().expect("worker reached read gate");
        let mut competing_reader = payload.as_slice();
        assert!(matches!(
            backend.ingest_manifest(&manifest, &plan, &mut competing_reader),
            Err(StorageError::ManifestExists { .. })
        ));
        release_tx.send(()).expect("release worker");
        let manifest_id = worker.join().expect("worker join").expect("worker ingest");

        assert_eq!(backend.manifest_count(), 1);
        assert_eq!(backend.total_bytes(), plan.content_length);
        assert!(backend.manifest(&manifest_id).is_some());
        assert_staging_empty(&backend);

        drop(backend);
        let reloaded = StorageBackend::new(config).expect("reload backend");
        assert_eq!(reloaded.manifest_count(), 1);
        assert_eq!(reloaded.total_bytes(), plan.content_length);
    }

    #[test]
    fn failed_ingest_releases_reservation_without_deleting_retry() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = Arc::new(StorageBackend::new(temp_config(&temp_dir)).expect("backend init"));
        let payload = vec![0x5A; 32 * 1024];
        let plan = single_file_plan(&payload).expect("plan");
        let manifest = test_manifest(&payload, &plan, 0xA2);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let worker_backend = Arc::clone(&backend);
        let worker_manifest = manifest.clone();
        let worker_plan = plan.clone();
        let worker_payload = payload.clone();
        let worker = thread::spawn(move || {
            let mut reader = GatedReader {
                bytes: Cursor::new(worker_payload),
                entered: Some(entered_tx),
                release: release_rx,
                fail_after_release: true,
            };
            worker_backend.ingest_manifest(&worker_manifest, &worker_plan, &mut reader)
        });

        entered_rx.recv().expect("worker reached read gate");
        let mut competing_reader = payload.as_slice();
        assert!(matches!(
            backend.ingest_manifest(&manifest, &plan, &mut competing_reader),
            Err(StorageError::ManifestExists { .. })
        ));
        release_tx.send(()).expect("release worker");
        assert!(worker.join().expect("worker join").is_err());
        assert_eq!(backend.manifest_count(), 0);
        assert_eq!(backend.total_bytes(), 0);
        assert_staging_empty(&backend);

        let mut retry_reader = payload.as_slice();
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut retry_reader)
            .expect("retry succeeds after failed owner");
        assert!(backend.manifest(&manifest_id).is_some());
        assert_eq!(backend.total_bytes(), plan.content_length);
        assert_staging_empty(&backend);
    }

    #[test]
    fn ingest_manifest_persists_metadata_and_chunks() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"Hello deterministic SoraFS!";
        let plan = single_file_plan(payload).expect("plan");

        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0x01, 0x02, 0x03])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];

        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        assert_eq!(backend.manifest_count(), 1);
        assert_eq!(backend.total_bytes(), plan.content_length);

        let manifest_digest = manifest.digest().expect("manifest digest");
        let manifest_digest_hex = hex::encode(manifest_digest.as_bytes());
        assert_eq!(manifest_id, manifest_digest_hex);
        assert_eq!(manifest_id.len(), 64, "manifest id must be 64 hex chars");
        let stored = backend.manifest(&manifest_id).expect("manifest stored");
        assert_eq!(stored.chunk_count(), plan.chunks.len());
        assert_eq!(stored.content_length(), plan.content_length);
        let expected_digest: [u8; 32] = *manifest_digest.as_bytes();
        assert_eq!(stored.manifest_digest(), &expected_digest);
        assert_eq!(stored.manifest_id(), manifest_digest_hex);

        for (index, chunk) in plan.chunks.iter().enumerate() {
            let record = stored.chunk(index).expect("chunk metadata");
            assert_eq!(record.offset, chunk.offset);
            assert_eq!(record.length, chunk.length);
            assert_eq!(record.digest, chunk.digest);
            assert!(record.path.exists(), "chunk file must exist on disk");
        }

        let por_tree = stored.por_tree();
        assert_eq!(por_tree.payload_len(), plan.content_length);
        assert_eq!(por_tree.chunks().len(), plan.chunks.len());
    }

    #[test]
    fn ingest_rejects_manifest_por_root_mismatch_without_publication() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");
        let payload = b"manifest PoR commitments must bind the provider payload";
        let plan = single_file_plan(payload).expect("plan");
        let mut manifest = test_manifest(payload, &plan, 0x91);
        manifest.por_root[0] ^= 0x80;
        let manifest_id = hex::encode(
            manifest
                .digest()
                .expect("mismatched manifest digest")
                .as_bytes(),
        );

        let mut reader = payload.as_slice();
        let error = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect_err("mismatched PoR commitment must fail closed");

        assert!(matches!(&error, StorageError::PorRootMismatch));
        assert_eq!(
            error.to_string(),
            "provider PoR root does not match the manifest commitment"
        );
        assert_eq!(backend.manifest_count(), 0);
        assert_eq!(backend.total_bytes(), 0);
        assert_eq!(backend.pdp_tree_memory_bytes(), 0);
        assert_eq!(backend.reserved_pdp_tree_memory_bytes(), 0);
        assert!(backend.manifest(&manifest_id).is_none());
        assert!(!backend.manifests_dir.join(manifest_id).exists());
        assert!(
            fs::read_dir(&backend.manifests_dir)
                .expect("read manifests directory")
                .next()
                .is_none(),
            "a rejected manifest must not publish a manifest directory"
        );
        let state = backend.state.read().expect("storage state");
        assert!(state.index.entries.is_empty());
        assert!(state.inflight_manifests.is_empty());
        assert_eq!(state.reserved_bytes, 0);
        drop(state);
        assert_staging_empty(&backend);
    }

    #[test]
    fn ingest_manifest_preserves_directory_file_layout() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let files = vec![
            FileEntry {
                path: vec!["assets".to_owned(), "app.js".to_owned()],
                data: b"console.log('sorafs');".to_vec(),
            },
            FileEntry {
                path: vec!["index.html".to_owned()],
                data: b"<!doctype html><title>SoraFS</title>".to_vec(),
            },
        ];
        let (plan, payload) =
            CarBuildPlan::from_files_with_profile(files, sorafs_chunker::ChunkProfile::DEFAULT)
                .expect("directory plan");

        let manifest = manifest_builder_for_plan(&payload, &plan)
            .root_cid(vec![0x42; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(&payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = payload.as_slice();
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest directory payload");

        let stored = backend.manifest(&manifest_id).expect("stored manifest");
        assert_eq!(stored.files().len(), plan.files.len());
        for (stored_file, planned_file) in stored.files().iter().zip(&plan.files) {
            assert_eq!(stored_file.path, planned_file.path);
            assert_eq!(stored_file.size, planned_file.size);
            assert_eq!(stored_file.first_chunk, planned_file.first_chunk);
            assert_eq!(stored_file.chunk_count, planned_file.chunk_count);
        }

        let rebuilt_plan = stored.to_car_plan(sorafs_chunker::ChunkProfile::DEFAULT);
        assert_eq!(rebuilt_plan.files, plan.files);

        drop(backend);
        let reloaded = StorageBackend::new(temp_config(&temp_dir)).expect("reload backend");
        let stored_reloaded = reloaded.manifest(&manifest_id).expect("reloaded manifest");
        assert_eq!(stored_reloaded.files(), stored.files());
        assert_eq!(
            stored_reloaded.file_by_path(&["assets".to_owned(), "app.js".to_owned()]),
            stored.file_by_path(&["assets".to_owned(), "app.js".to_owned()])
        );
    }

    #[test]
    fn capacity_enforced() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let mut config = temp_config(&temp_dir);
        config = StorageConfig::builder()
            .enabled(true)
            .data_dir(config.data_dir().clone())
            .max_capacity_bytes(iroha_config::base::util::Bytes(16))
            .build();

        let backend = StorageBackend::new(config).expect("backend init");

        let payload = b"this payload is definitely longer than sixteen bytes";
        let plan = single_file_plan(payload).expect("plan");

        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0x0A, 0x0B])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];

        let err = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect_err("should exceed capacity");

        match err {
            StorageError::CapacityExceeded { .. } => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn read_payload_range_returns_expected_bytes() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"The five boxing wizards jump quickly";
        let plan = single_file_plan(payload).expect("plan");

        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xAB; 32])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let slice = backend
            .read_payload_range(&manifest_id, 4, 5)
            .expect("read range");
        assert_eq!(slice, b"five "[..]);
    }

    #[test]
    fn chunk_slice_returns_expected_metadata() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = vec![0xAA; 64 * 3];
        let plan = single_file_plan(&payload).expect("plan");

        let manifest = manifest_builder_for_plan(&payload, &plan)
            .root_cid(vec![0x44; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(&payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        let stored = backend.manifest(&manifest_id).expect("stored manifest");

        let slice = stored.chunk_slice(0, payload.len()).expect("chunk slice");
        assert_eq!(slice.chunk_count(), plan.chunks.len());
        assert_eq!(slice.start_index, 0);
        assert_eq!(slice.end_index, plan.chunks.len().saturating_sub(1));
        for (record, expected) in slice.chunks.iter().zip(&plan.chunks) {
            assert_eq!(record.offset, expected.offset);
            assert_eq!(record.length, expected.length);
            assert_eq!(record.digest, expected.digest);
        }
    }

    #[test]
    fn attach_stripe_layout_persists_roles() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"stripe layout payload";
        let plan = single_file_plan(payload).expect("plan");
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xAA, 0xBB])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let layout = DaStripeLayout {
            total_stripes: 1,
            shards_per_stripe: plan.chunks.len() as u32,
            row_parity_stripes: 0,
        };
        let chunk_roles: Vec<ChunkRoleMetadata> = plan
            .chunks
            .iter()
            .enumerate()
            .map(|(idx, _)| ChunkRoleMetadata {
                role: ChunkRole::Data,
                group_id: idx as u32,
            })
            .collect();
        backend
            .attach_stripe_layout(&manifest_id, layout, chunk_roles.clone())
            .expect("attach layout");

        let stored = backend.manifest(&manifest_id).expect("stored manifest");
        assert_eq!(stored.stripe_layout(), Some(&layout));
        for (chunk, role) in stored.chunk_files.iter().zip(&chunk_roles) {
            assert_eq!(chunk.role, Some(role.role));
            assert_eq!(chunk.group_id, Some(role.group_id));
        }

        // Reload to ensure metadata persisted on disk.
        drop(backend);
        let reloaded = StorageBackend::new(temp_config(&temp_dir)).expect("reload");
        let stored_reloaded = reloaded.manifest(&manifest_id).expect("stored manifest");
        assert_eq!(stored_reloaded.stripe_layout(), Some(&layout));
        for (chunk, role) in stored_reloaded.chunk_files.iter().zip(chunk_roles) {
            assert_eq!(chunk.role, Some(role.role));
            assert_eq!(chunk.group_id, Some(role.group_id));
        }
    }

    #[test]
    fn attach_stripe_layout_rejects_mismatched_lengths() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"role length check";
        let plan = single_file_plan(payload).expect("plan");
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xFF, 0xEE])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let layout = DaStripeLayout {
            total_stripes: 1,
            shards_per_stripe: plan.chunks.len() as u32,
            row_parity_stripes: 0,
        };
        let err = backend
            .attach_stripe_layout(&manifest_id, layout, Vec::new())
            .expect_err("mismatched roles should error");
        matches!(
            err,
            StorageError::ChunkRoleLengthMismatch {
                expected: _,
                actual: _
            }
        );
    }

    #[test]
    fn attach_stripe_layout_does_not_mutate_memory_when_persistence_fails() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"stripe metadata failure payload";
        let (_config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xD1);
        let before = backend.manifest(&manifest_id).expect("stored manifest");
        let metadata_path = backend
            .manifests_dir
            .join(&manifest_id)
            .join(METADATA_FILE_NAME);
        fs::remove_file(&metadata_path).expect("remove metadata file");
        fs::create_dir(&metadata_path).expect("block metadata replacement");

        let layout = DaStripeLayout {
            total_stripes: 1,
            shards_per_stripe: before.chunk_count() as u32,
            row_parity_stripes: 0,
        };
        let roles = (0..before.chunk_count())
            .map(|index| ChunkRoleMetadata {
                role: ChunkRole::Data,
                group_id: index as u32,
            })
            .collect();
        assert!(matches!(
            backend.attach_stripe_layout(&manifest_id, layout, roles),
            Err(StorageError::Io(_))
        ));

        let after = backend.manifest(&manifest_id).expect("stored manifest");
        assert_eq!(after.stripe_layout(), before.stripe_layout());
        assert_eq!(after.chunk_files, before.chunk_files);
    }

    #[test]
    fn attach_plan_metadata_validates_plan_and_commits_only_after_persistence() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"logical file metadata payload";
        let (_config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xD2);
        let before = backend.manifest(&manifest_id).expect("stored manifest");

        let mut mismatched_chunks = single_file_plan(payload).expect("plan");
        mismatched_chunks.chunks[0].digest[0] ^= 1;
        assert!(matches!(
            backend.attach_plan_metadata(&manifest_id, &mismatched_chunks, None, None),
            Err(StorageError::ManifestExists { .. })
        ));

        let mut invalid_path = single_file_plan(payload).expect("plan");
        invalid_path.files[0].path = vec!["..".to_owned()];
        assert!(matches!(
            backend.attach_plan_metadata(&manifest_id, &invalid_path, None, None),
            Err(StorageError::InvalidFileLayout { .. })
        ));

        let mut updated_plan = single_file_plan(payload).expect("plan");
        updated_plan.files[0].path = vec!["index.html".to_owned()];
        let metadata_path = backend
            .manifests_dir
            .join(&manifest_id)
            .join(METADATA_FILE_NAME);
        fs::remove_file(&metadata_path).expect("remove metadata file");
        fs::create_dir(&metadata_path).expect("block metadata replacement");
        assert!(matches!(
            backend.attach_plan_metadata(&manifest_id, &updated_plan, None, None),
            Err(StorageError::Io(_))
        ));

        let after = backend.manifest(&manifest_id).expect("stored manifest");
        assert_eq!(after.files(), before.files());
        assert_eq!(after.chunk_files, before.chunk_files);
    }

    #[test]
    fn chunk_slice_rejects_misaligned_range() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = vec![0xBB; 128];
        let plan = single_file_plan(&payload).expect("plan");

        let manifest = manifest_builder_for_plan(&payload, &plan)
            .root_cid(vec![0x55; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(&payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        let stored = backend.manifest(&manifest_id).expect("stored manifest");

        let err = stored
            .chunk_slice(1, 4)
            .expect_err("misaligned slice must fail");
        assert!(matches!(err, StorageError::RangeOutOfBounds { .. }));
    }

    #[test]
    fn load_manifest_round_trips_original_manifest() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"manifest payload round trip bytes";
        let plan = single_file_plan(payload).expect("plan");

        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0x77; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        let stored = backend.manifest(&manifest_id).expect("stored manifest");

        let decoded = stored.load_manifest().expect("load manifest");
        assert_eq!(decoded, manifest);
    }

    #[test]
    fn retention_epoch_persists_in_metadata() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"retention epoch persistence";
        let plan = single_file_plan(payload).expect("plan");
        let mut policy = PinPolicy::default();
        policy.retention_epoch = 200;

        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xFA, 0xCE])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(policy)
            .add_metadata(
                sorafs_manifest::retention::RETENTION_DEAL_END_EPOCH_KEY,
                "150",
            )
            .add_metadata(
                sorafs_manifest::retention::RETENTION_GOVERNANCE_CAP_EPOCH_KEY,
                "180",
            )
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let stored = backend.manifest(&manifest_id).expect("stored");
        assert_eq!(stored.retention_epoch(), 150);
        let source = stored.retention_source().expect("retention source");
        assert_eq!(
            source.sources,
            vec![sorafs_manifest::retention::RetentionSourceKindV1::DealEnd]
        );

        drop(backend);
        let reloaded = StorageBackend::new(temp_config(&temp_dir)).expect("reload");
        let stored_reloaded = reloaded.manifest(&manifest_id).expect("stored manifest");
        assert_eq!(stored_reloaded.retention_epoch(), 150);
        let source_reloaded = stored_reloaded
            .retention_source()
            .expect("retention source");
        assert_eq!(
            source_reloaded.sources,
            vec![sorafs_manifest::retention::RetentionSourceKindV1::DealEnd]
        );
    }

    #[test]
    fn restart_rejects_noncanonical_or_unbound_retention_sources() {
        for case in 0..4_u8 {
            let temp_dir = tempfile::tempdir().expect("create temp dir");
            let (config, backend, manifest_id) =
                ingest_test_payload(&temp_dir, b"retention source tamper", 0xB0 + case);

            match case {
                0 => {
                    rewrite_manifest_record(&backend, &manifest_id, |record| {
                        record
                            .retention_source
                            .as_mut()
                            .expect("retention source")
                            .effective_epoch = 1;
                    });
                }
                1 => {
                    rewrite_manifest_record(&backend, &manifest_id, |record| {
                        record.retention_source = None;
                    });
                }
                2 => {
                    rewrite_manifest_index(&backend, |index| {
                        index.entries[0]
                            .retention_source
                            .as_mut()
                            .expect("retention source")
                            .version ^= 1;
                    });
                }
                3 => {
                    let rewrite_source = |source: &mut RetentionSourceV1| {
                        source.pin_policy_epoch = 7;
                        source.effective_epoch = 7;
                        source.sources =
                            vec![sorafs_manifest::retention::RetentionSourceKindV1::PinPolicy];
                    };
                    rewrite_manifest_record(&backend, &manifest_id, |record| {
                        record.retention_epoch = 7;
                        rewrite_source(record.retention_source.as_mut().expect("retention source"));
                    });
                    rewrite_manifest_index(&backend, |index| {
                        index.entries[0].retention_epoch = 7;
                        rewrite_source(
                            index.entries[0]
                                .retention_source
                                .as_mut()
                                .expect("retention source"),
                        );
                    });
                }
                _ => unreachable!("bounded test case"),
            }

            drop(backend);
            assert!(matches!(
                StorageBackend::new(config),
                Err(StorageError::CorruptStorageState { .. })
            ));
        }
    }

    #[test]
    fn last_access_persists_after_reads() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"last access persistence";
        let plan = single_file_plan(payload).expect("plan");
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0x11, 0x22])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let stored = backend.manifest(&manifest_id).expect("stored");
        let initial_access = stored.last_access();
        assert!(initial_access > 0);

        let _slice = backend
            .read_payload_range(&manifest_id, 0, 4)
            .expect("read");

        let updated = backend.manifest(&manifest_id).expect("stored");
        assert!(updated.last_access() > initial_access);

        drop(backend);
        let reloaded = StorageBackend::new(temp_config(&temp_dir)).expect("reload");
        let stored_reloaded = reloaded.manifest(&manifest_id).expect("stored");
        assert_eq!(stored_reloaded.last_access(), updated.last_access());
    }

    #[test]
    fn evict_manifest_removes_files_and_updates_index() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"payload for eviction";
        let plan = single_file_plan(payload).expect("plan");
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0x10, 0x20, 0x30])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let stored = backend.manifest(&manifest_id).expect("stored");
        let manifest_dir = stored.manifest_path().parent().expect("manifest dir");
        assert!(manifest_dir.exists());

        let freed = backend
            .evict_manifest(&manifest_id)
            .expect("evict manifest");
        assert_eq!(freed, plan.content_length);
        assert!(backend.manifest(&manifest_id).is_none());
        assert_eq!(backend.manifest_count(), 0);
        assert_eq!(backend.total_bytes(), 0);
        assert!(!manifest_dir.exists());

        drop(backend);
        let reloaded = StorageBackend::new(temp_config(&temp_dir)).expect("reload");
        assert_eq!(reloaded.manifest_count(), 0);
    }

    #[test]
    fn eviction_waits_for_active_manifest_io_lease() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"active readers must not race eviction";
        let (_config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xD1);
        let backend = Arc::new(backend);
        let stored = backend.manifest(&manifest_id).expect("stored manifest");
        let read_lease = stored
            .io_lock
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (done_tx, done_rx) = mpsc::channel();
        let worker_backend = Arc::clone(&backend);
        let worker_manifest_id = manifest_id.clone();
        let worker = thread::spawn(move || {
            let result = worker_backend.evict_manifest(&worker_manifest_id);
            done_tx.send(result).expect("send eviction result");
        });

        let deadline = Instant::now() + Duration::from_secs(2);
        while backend.state.try_read().is_ok() {
            assert!(
                Instant::now() < deadline,
                "eviction worker did not reach the manifest I/O lease"
            );
            thread::yield_now();
        }
        assert!(
            matches!(done_rx.try_recv(), Err(mpsc::TryRecvError::Empty)),
            "eviction must remain blocked while a reader holds the manifest lease"
        );

        drop(read_lease);
        assert_eq!(
            done_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("eviction must finish after releasing the reader")
                .expect("eviction succeeds"),
            payload.len() as u64
        );
        worker.join().expect("eviction worker joins");
        assert!(backend.manifest(&manifest_id).is_none());
    }

    #[test]
    fn access_metadata_update_cannot_resurrect_evicted_manifest() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"access metadata must share the manifest lifecycle lease";
        let (_config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xD2);
        let manifest_dir = backend.manifests_dir.join(&manifest_id);
        let backend = Arc::new(backend);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let access_backend = Arc::clone(&backend);
        let access_manifest_id = manifest_id.clone();
        let access = thread::spawn(move || {
            access_backend.with_manifest_for_access(&access_manifest_id, |_| {
                entered_tx.send(()).expect("announce active access");
                release_rx.recv().expect("release active access");
                Ok(())
            })
        });
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("access operation enters protected section");

        let (evicted_tx, evicted_rx) = mpsc::channel();
        let evict_backend = Arc::clone(&backend);
        let evict_manifest_id = manifest_id.clone();
        let eviction = thread::spawn(move || {
            evicted_tx
                .send(evict_backend.evict_manifest(&evict_manifest_id))
                .expect("send eviction result");
        });
        assert!(
            matches!(
                evicted_rx.recv_timeout(Duration::from_millis(50)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ),
            "eviction must wait for access metadata and payload work"
        );

        release_tx.send(()).expect("release access operation");
        access
            .join()
            .expect("access worker joins")
            .expect("access succeeds");
        evicted_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("eviction completes")
            .expect("eviction succeeds");
        eviction.join().expect("eviction worker joins");

        assert!(!manifest_dir.exists());
        assert!(backend.manifest(&manifest_id).is_none());
    }

    #[test]
    fn to_car_plan_matches_stored_chunks() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = vec![0xCC; 96];
        let plan = single_file_plan(&payload).expect("plan");

        let manifest = manifest_builder_for_plan(&payload, &plan)
            .root_cid(vec![0x99; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(&payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        let stored = backend.manifest(&manifest_id).expect("stored manifest");

        let rebuilt = stored.to_car_plan(sorafs_chunker::ChunkProfile::DEFAULT);
        assert_eq!(rebuilt.chunk_profile, plan.chunk_profile);
        assert_eq!(rebuilt.content_length, plan.content_length);
        assert_eq!(rebuilt.chunks, plan.chunks);
        assert_eq!(rebuilt.payload_digest, plan.payload_digest);
        assert_eq!(rebuilt.files, plan.files);

        let hint = TaikaiSegmentHint {
            event: "event-a".to_owned(),
            stream: "stream-a".to_owned(),
            rendition: "1080p".to_owned(),
            sequence: 7,
            payload_len: Some(plan.content_length),
            payload_digest: Some(*plan.payload_digest.as_bytes()),
        };
        let fallible = stored
            .try_to_car_plan_with_hint(sorafs_chunker::ChunkProfile::DEFAULT, Some(&hint))
            .expect("fallibly rebuild CAR plan with Taikai hint");
        assert_eq!(fallible.files, plan.files);
        assert_eq!(fallible.chunks.len(), plan.chunks.len());
        assert!(
            fallible
                .chunks
                .iter()
                .all(|chunk| chunk.taikai_segment_hint.as_ref() == Some(&hint))
        );
    }

    #[test]
    fn sample_por_returns_proofs() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = (0..(sorafs_car::POR_LEAF_SIZE * 4 + 17))
            .map(|index| u8::try_from(index % 251).expect("fixture byte"))
            .collect::<Vec<_>>();
        let plan = single_file_plan(&payload).expect("plan");

        let manifest = manifest_builder_for_plan(&payload, &plan)
            .root_cid(vec![0xCD; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(&payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = payload.as_slice();
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let stored = backend.manifest(&manifest_id).expect("stored manifest");
        let leaf_count = stored.por_tree().leaf_count_u64();
        let collision_seed = (0u64..)
            .find(|seed| {
                let first = sorafs_car::splitmix64(*seed);
                let second = sorafs_car::splitmix64(first);
                first % leaf_count == second % leaf_count
            })
            .expect("bounded fixture has a SplitMix reduction collision");
        let expected_indices = PorSampleIndices::new(leaf_count, 4, collision_seed)
            .expect("build shared canonical sample schedule")
            .map(|index| usize::try_from(index).expect("fixture index fits usize"))
            .collect::<Vec<_>>();
        let samples = backend
            .sample_por(&manifest_id, 4, collision_seed)
            .expect("PoR samples");
        let expected = stored.por_tree().leaf_count().min(4);
        assert_eq!(samples.len(), expected);
        assert_eq!(
            samples.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
            expected_indices,
            "provider storage sampling must use the shared collision schedule"
        );
        let root = *stored.por_tree().root();

        for (_idx, proof) in samples {
            assert!(proof.verify(&root));
        }
    }

    #[test]
    fn sample_por_rejects_unbounded_sample_count() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");
        let requested = usize::try_from(MAX_PROOF_STREAM_SAMPLE_COUNT)
            .expect("u32 PoR sample ceiling must fit usize")
            + 1;

        assert!(matches!(
            backend.sample_por("missing-manifest", requested, 7),
            Err(StorageError::PorSampleCountTooLarge {
                requested: found,
                maximum: MAX_PROOF_STREAM_SAMPLE_COUNT,
            }) if found == requested
        ));
    }

    #[test]
    fn chunk_by_digest_returns_record() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"deterministic chunk access";
        let plan = single_file_plan(payload).expect("plan");

        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xEE; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let first_chunk = plan.chunks.first().expect("at least one chunk");
        let record = backend
            .chunk_by_digest(&manifest_id, &first_chunk.digest)
            .expect("chunk metadata");
        assert_eq!(record.offset, first_chunk.offset);
        assert_eq!(record.length, first_chunk.length);
        assert_eq!(record.digest, first_chunk.digest);
        assert!(record.path.exists(), "chunk file must exist on disk");
    }

    #[test]
    fn chunk_by_digest_missing_returns_error() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"missing chunk digests";
        let plan = single_file_plan(payload).expect("plan");

        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xAA; 4])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let missing = [0xFFu8; 32];
        let err = backend
            .chunk_by_digest(&manifest_id, &missing)
            .expect_err("chunk should be missing");
        match err {
            StorageError::ChunkNotFound {
                manifest_id: mid, ..
            } => {
                assert_eq!(mid, manifest_id);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn read_chunk_returns_bytes() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"stream chunk payload";
        let plan = single_file_plan(payload).expect("plan");

        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xBB; 6])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let chunk = plan.chunks.first().expect("chunk");
        let bytes = backend
            .read_chunk(&manifest_id, &chunk.digest)
            .expect("chunk bytes");
        assert_eq!(bytes, payload);
    }

    #[test]
    fn same_length_chunk_corruption_fails_all_read_paths() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");
        let payload = b"immutable chunks must be verified before every read";
        let plan = single_file_plan(payload).expect("plan");
        let manifest = test_manifest(payload, &plan, 0xB1);
        let mut reader = payload.as_slice();
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        let chunk = backend
            .manifest(&manifest_id)
            .and_then(|stored| stored.chunk(0).cloned())
            .expect("stored chunk");

        let mut corrupted = payload.to_vec();
        corrupted[0] ^= 0x80;
        fs::write(&chunk.path, &corrupted).expect("replace chunk with same-length corruption");

        assert!(matches!(
            backend.read_chunk(&manifest_id, &chunk.digest),
            Err(StorageError::ChunkStore(ChunkStoreError::DigestMismatch {
                chunk_index: 0
            }))
        ));
        assert!(matches!(
            backend.read_payload_range(&manifest_id, 0, payload.len()),
            Err(StorageError::ChunkStore(ChunkStoreError::DigestMismatch {
                chunk_index: 0
            }))
        ));
        assert!(matches!(
            backend.sample_por(&manifest_id, 1, 42),
            Err(StorageError::ChunkStore(ChunkStoreError::DigestMismatch {
                chunk_index: 0
            }))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn chunk_reads_reject_symlink_replacement() {
        use std::os::unix::fs::symlink;

        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");
        let payload = b"symlink replacement must fail closed";
        let plan = single_file_plan(payload).expect("plan");
        let manifest = test_manifest(payload, &plan, 0xB2);
        let mut reader = payload.as_slice();
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        let chunk = backend
            .manifest(&manifest_id)
            .and_then(|stored| stored.chunk(0).cloned())
            .expect("stored chunk");
        let replacement = canonical_temp_path(&temp_dir).join("replacement.bin");
        fs::write(&replacement, payload).expect("write replacement");
        fs::remove_file(&chunk.path).expect("remove stored chunk");
        symlink(&replacement, &chunk.path).expect("install symlink");

        assert!(matches!(
            backend.read_chunk(&manifest_id, &chunk.digest),
            Err(StorageError::ChunkStore(ChunkStoreError::Io(_)))
        ));
        assert!(matches!(
            backend.read_payload_range(&manifest_id, 0, payload.len()),
            Err(StorageError::ChunkStore(ChunkStoreError::Io(_)))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn chunk_reads_reject_hard_link_aliases() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");
        let payload = b"hard-linked chunks must fail closed";
        let plan = single_file_plan(payload).expect("plan");
        let manifest = test_manifest(payload, &plan, 0xB3);
        let mut reader = payload.as_slice();
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        let chunk = backend
            .manifest(&manifest_id)
            .and_then(|stored| stored.chunk(0).cloned())
            .expect("stored chunk");
        let alias = canonical_temp_path(&temp_dir).join("chunk-alias.bin");
        fs::hard_link(&chunk.path, &alias).expect("create chunk hard link");

        assert!(matches!(
            backend.read_chunk(&manifest_id, &chunk.digest),
            Err(StorageError::ChunkStore(ChunkStoreError::Io(_)))
        ));
        assert!(matches!(
            backend.read_payload_range(&manifest_id, 0, payload.len()),
            Err(StorageError::ChunkStore(ChunkStoreError::Io(_)))
        ));
    }

    #[test]
    fn pdp_commitment_tree_and_witnesses_survive_restart() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = (0..300_000_u32)
            .map(|index| (index.wrapping_mul(31) & 0xff) as u8)
            .collect::<Vec<_>>();
        let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, &payload, 0xE1);
        let stored = backend.manifest(&manifest_id).expect("stored manifest");
        let commitment = stored
            .pdp_commitment()
            .cloned()
            .expect("non-empty payload commitment");
        let commitment_digest = commitment.commitment_digest().expect("commitment digest");
        assert_eq!(stored.pdp_commitment_digest(), Some(&commitment_digest));
        let tree = stored.pdp_tree().expect("runtime PDP tree");
        assert_eq!(tree.hot_root(), commitment.commitment_root_hot);
        assert_eq!(tree.segment_root(), commitment.commitment_root_segment);
        assert_eq!(tree.payload_len(), payload.len() as u64);
        assert_eq!(
            stored.pdp_tree_memory_bytes(),
            pdp_tree_memory_for_payload(payload.len() as u64).expect("tree memory")
        );
        let samples = vec![
            PdpSampleV1 {
                segment_index: 0,
                hot_leaf_indices: vec![0, 7, 63],
            },
            PdpSampleV1 {
                segment_index: 1,
                hot_leaf_indices: vec![0, 9],
            },
        ];
        let before = backend
            .prove_pdp_samples(&manifest_id, &samples)
            .expect("PDP witnesses before restart");
        assert_eq!(before.len(), 2);
        assert_eq!(before[0].hot_leaves[0].leaf_bytes, payload[..4_096]);
        let retained = backend.pdp_tree_memory_bytes();
        drop(stored);
        drop(backend);

        let reloaded = StorageBackend::new(config).expect("restart backend");
        assert_eq!(reloaded.pdp_tree_memory_bytes(), retained);
        let restored = reloaded.manifest(&manifest_id).expect("restored manifest");
        assert_eq!(restored.pdp_commitment(), Some(&commitment));
        assert_eq!(restored.pdp_commitment_digest(), Some(&commitment_digest));
        assert_eq!(
            reloaded
                .prove_pdp_samples(&manifest_id, &samples)
                .expect("PDP witnesses after restart"),
            before
        );
    }

    #[test]
    fn persisted_por_commitment_is_bounded_by_chunk_count_not_leaf_count() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = (0..2_097_152_u32)
            .map(|index| (index.wrapping_mul(17) & 0xff) as u8)
            .collect::<Vec<_>>();
        let (_config, backend, manifest_id) = ingest_test_payload(&temp_dir, &payload, 0xE0);
        let stored = backend.manifest(&manifest_id).expect("stored manifest");
        let metadata_path = stored
            .manifest_path()
            .parent()
            .expect("manifest directory")
            .join(METADATA_FILE_NAME);
        let metadata_bytes = fs::read(&metadata_path).expect("read bounded metadata");
        let record: StoredManifestRecord =
            norito::decode_from_bytes(&metadata_bytes).expect("decode metadata");
        let tree = stored.por_tree_ref();
        assert!(tree.segment_count() > tree.chunks().len());
        assert!(tree.leaf_count() > tree.segment_count());
        assert_eq!(
            record.por_commitment.chunks.len(),
            stored.chunk_count(),
            "persistent PoR summary has exactly one bounded entry per content chunk"
        );
        assert_eq!(record.por_commitment.leaf_count, tree.leaf_count() as u64);
        let linear_chunk_bound = 16_384_usize
            .checked_add(stored.chunk_count().saturating_mul(512))
            .expect("metadata size bound");
        assert!(
            metadata_bytes.len() <= linear_chunk_bound,
            "{} bytes of metadata exceeded O(chunk count) bound {linear_chunk_bound} for {} PoR leaves",
            metadata_bytes.len(),
            tree.leaf_count()
        );
    }

    #[test]
    fn restart_rejects_tampered_pdp_por_and_payload_bindings() {
        for case in 0_u8..10 {
            let temp_dir = tempfile::tempdir().expect("create temp dir");
            let payload = vec![case.wrapping_add(1); 300_000];
            let (config, backend, manifest_id) =
                ingest_test_payload(&temp_dir, &payload, 0xE2_u8.wrapping_add(case));
            match case {
                0 => rewrite_manifest_record(&backend, &manifest_id, |record| {
                    record
                        .pdp_commitment
                        .as_mut()
                        .expect("commitment")
                        .commitment_root_hot[0] ^= 0x80;
                }),
                1 => rewrite_manifest_index(&backend, |index| {
                    index.entries[0]
                        .pdp_commitment_digest
                        .as_mut()
                        .expect("commitment digest")[0] ^= 0x40;
                }),
                2 => rewrite_manifest_record(&backend, &manifest_id, |record| {
                    record
                        .pdp_commitment
                        .as_mut()
                        .expect("commitment")
                        .manifest_digest[0] ^= 0x20;
                }),
                3 => rewrite_manifest_record(&backend, &manifest_id, |record| {
                    record
                        .pdp_commitment
                        .as_mut()
                        .expect("commitment")
                        .sealed_at += 1;
                }),
                4 => rewrite_manifest_record(&backend, &manifest_id, |record| {
                    record
                        .pdp_commitment
                        .as_mut()
                        .expect("commitment")
                        .sample_window += 1;
                }),
                5 => rewrite_manifest_record(&backend, &manifest_id, |record| {
                    record
                        .pdp_commitment
                        .as_mut()
                        .expect("commitment")
                        .hot_leaf_count += 1;
                }),
                6 => rewrite_manifest_record(&backend, &manifest_id, |record| {
                    record.por_commitment.root[0] ^= 0x10;
                }),
                7 => {
                    rewrite_manifest_record(&backend, &manifest_id, |record| {
                        record.payload_digest[0] ^= 0x08;
                    });
                    rewrite_manifest_index(&backend, |index| {
                        index.entries[0].payload_digest[0] ^= 0x08;
                    });
                }
                8 => {
                    let metadata_path = backend
                        .manifests_dir
                        .join(&manifest_id)
                        .join(METADATA_FILE_NAME);
                    let bytes = fs::read(&metadata_path).expect("read manifest metadata");
                    let mut record: StoredManifestRecord =
                        norito::decode_from_bytes(&bytes).expect("decode manifest metadata");
                    let commitment = record.pdp_commitment.as_mut().expect("commitment");
                    commitment.commitment_root_segment[0] ^= 0x04;
                    let digest = commitment.commitment_digest().expect("commitment digest");
                    fs::write(
                        &metadata_path,
                        norito::to_bytes(&record).expect("encode manifest metadata"),
                    )
                    .expect("rewrite manifest metadata");
                    rewrite_manifest_index(&backend, |index| {
                        index.entries[0].pdp_commitment_digest = Some(digest);
                    });
                }
                9 => {
                    let metadata_path = backend
                        .manifests_dir
                        .join(&manifest_id)
                        .join(METADATA_FILE_NAME);
                    let bytes = fs::read(&metadata_path).expect("read manifest metadata");
                    let mut record: StoredManifestRecord =
                        norito::decode_from_bytes(&bytes).expect("decode manifest metadata");
                    record.por_commitment.root[0] ^= 0x02;
                    let digest = record
                        .por_commitment
                        .digest()
                        .expect("PoR commitment digest");
                    fs::write(
                        &metadata_path,
                        norito::to_bytes(&record).expect("encode manifest metadata"),
                    )
                    .expect("rewrite manifest metadata");
                    rewrite_manifest_index(&backend, |index| {
                        index.entries[0].por_commitment_digest = digest;
                    });
                }
                _ => unreachable!(),
            }
            drop(backend);
            assert!(
                matches!(
                    StorageBackend::new(config),
                    Err(StorageError::CorruptStorageState { .. })
                ),
                "tampering case {case} must fail closed"
            );
        }
    }

    #[test]
    fn restart_rejects_persisted_por_root_not_bound_to_manifest() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"persisted PoR roots remain bound across provider restart";
        let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xE9);
        let mut tampered_commitment_digest = None;
        rewrite_manifest_record(&backend, &manifest_id, |record| {
            record.por_commitment.root[0] ^= 0x40;
            tampered_commitment_digest = Some(
                record
                    .por_commitment
                    .digest()
                    .expect("tampered PoR commitment digest"),
            );
        });
        rewrite_manifest_index(&backend, |index| {
            index.entries[0].por_commitment_digest =
                tampered_commitment_digest.expect("tampered commitment digest recorded");
        });
        drop(backend);

        let error = StorageBackend::new(config)
            .expect_err("restart must reject a persisted PoR root detached from its manifest");
        match error {
            StorageError::CorruptStorageState { reason, .. } => assert_eq!(
                reason,
                "persisted PoR root does not match the canonical manifest commitment"
            ),
            other => panic!("unexpected restart error: {other:?}"),
        }
    }

    #[test]
    fn empty_payload_has_no_pdp_commitment_or_tree() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let config = temp_config(&temp_dir);
        let backend = StorageBackend::new(config.clone()).expect("backend init");
        let payload = [];
        let plan = empty_file_plan();
        let manifest = test_manifest(&payload, &plan, 0xEA);
        let mut reader = payload.as_slice();
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest empty payload");
        let stored = backend
            .manifest(&manifest_id)
            .expect("stored empty manifest");
        assert_eq!(stored.content_length(), 0);
        assert_eq!(stored.chunk_count(), 0);
        assert!(stored.pdp_commitment().is_none());
        assert!(stored.pdp_commitment_digest().is_none());
        assert!(stored.pdp_tree().is_none());
        assert_eq!(stored.pdp_tree_memory_bytes(), 0);
        assert!(stored.por_commitment_digest().is_some());
        assert!(stored.por_tree_ref().is_empty());
        assert_eq!(backend.pdp_tree_memory_bytes(), 0);
        assert!(matches!(
            backend.prove_pdp_samples(&manifest_id, &first_pdp_sample()),
            Err(StorageError::PdpUnavailable { .. })
        ));
        drop(stored);
        drop(backend);

        let restarted = StorageBackend::new(config).expect("restart empty payload");
        let restored = restarted
            .manifest(&manifest_id)
            .expect("restored empty payload");
        assert!(restored.pdp_commitment().is_none());
        assert!(restored.pdp_tree().is_none());
        assert!(restored.por_tree_ref().is_empty());
        assert_eq!(restarted.pdp_tree_memory_bytes(), 0);
    }

    #[test]
    fn empty_payload_rejects_noncanonical_por_summary_geometry_and_root() {
        for case in 0_u8..4 {
            let temp_dir = tempfile::tempdir().expect("create temp dir");
            let config = temp_config(&temp_dir);
            let backend = StorageBackend::new(config.clone()).expect("backend init");
            let payload = [];
            let plan = empty_file_plan();
            let manifest = test_manifest(&payload, &plan, 0xD0_u8.wrapping_add(case));
            let mut reader = payload.as_slice();
            let manifest_id = backend
                .ingest_manifest(&manifest, &plan, &mut reader)
                .expect("ingest empty payload");
            let metadata_path = backend
                .manifests_dir
                .join(&manifest_id)
                .join(METADATA_FILE_NAME);
            let bytes = fs::read(&metadata_path).expect("read manifest metadata");
            let mut record: StoredManifestRecord =
                norito::decode_from_bytes(&bytes).expect("decode manifest metadata");
            match case {
                0 => record.por_commitment.chunk_count = 1,
                1 => record.por_commitment.segment_count = 1,
                2 => record.por_commitment.leaf_count = 1,
                3 => record.por_commitment.root[0] ^= 0x01,
                _ => unreachable!(),
            }
            let digest = record
                .por_commitment
                .digest()
                .expect("PoR commitment digest");
            fs::write(
                &metadata_path,
                norito::to_bytes(&record).expect("encode manifest metadata"),
            )
            .expect("rewrite manifest metadata");
            rewrite_manifest_index(&backend, |index| {
                index.entries[0].por_commitment_digest = digest;
            });
            drop(backend);
            assert!(
                matches!(
                    StorageBackend::new(config),
                    Err(StorageError::CorruptStorageState { .. })
                ),
                "empty PoR tampering case {case} must fail closed"
            );
        }
    }

    #[test]
    fn pdp_tree_budget_rejects_ingest_and_restart_overcommit() {
        let payload = vec![0xA5; 300_000];
        let required =
            pdp_tree_memory_for_payload(payload.len() as u64).expect("PDP tree estimate");
        assert!(required > 0);

        let ingest_dir = tempfile::tempdir().expect("create ingest temp dir");
        let ingest_config = temp_config_with_pdp_limit(&ingest_dir, required - 1);
        let ingest_backend = StorageBackend::new(ingest_config).expect("backend init");
        let plan = single_file_plan(&payload).expect("plan");
        let manifest = test_manifest(&payload, &plan, 0xEB);
        let mut reader = payload.as_slice();
        assert!(matches!(
            ingest_backend.ingest_manifest(&manifest, &plan, &mut reader),
            Err(StorageError::PdpTreeMemoryExceeded {
                required: found,
                available
            }) if found == required && available == required - 1
        ));
        assert_eq!(ingest_backend.manifest_count(), 0);
        assert_eq!(ingest_backend.pdp_tree_memory_bytes(), 0);
        assert_eq!(ingest_backend.reserved_pdp_tree_memory_bytes(), 0);

        let restart_dir = tempfile::tempdir().expect("create restart temp dir");
        let generous = temp_config_with_pdp_limit(&restart_dir, required);
        let backend = StorageBackend::new(generous).expect("backend init");
        let manifest = test_manifest(&payload, &plan, 0xEC);
        let mut reader = payload.as_slice();
        backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest within tree budget");
        drop(backend);
        let constrained = temp_config_with_pdp_limit(&restart_dir, required - 1);
        assert!(matches!(
            StorageBackend::new(constrained),
            Err(StorageError::PdpTreeMemoryExceeded {
                required: found,
                available
            }) if found == required && available == required - 1
        ));
    }

    #[test]
    fn backend_rejects_invalid_direct_pdp_configuration_before_filesystem_mutation() {
        for (sample_window, memory_limit) in [(0, 1), (501, 1), (1, 0)] {
            let temp_dir = tempfile::tempdir().expect("create temp dir");
            let data_dir = canonical_temp_path(&temp_dir).join("storage");
            let config = StorageConfig::builder()
                .enabled(true)
                .data_dir(data_dir.clone())
                .pdp_sample_window(sample_window)
                .pdp_tree_memory_limit_bytes(iroha_config::base::util::Bytes(memory_limit))
                .build();
            assert!(StorageBackend::new(config).is_err());
            assert!(
                !data_dir.exists(),
                "invalid PDP config must fail before creating storage"
            );
        }
    }

    #[test]
    fn uncertain_chunk_directory_publication_fail_stops_backend() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");
        let chunks_dir = backend.root_dir().join("uncertain-chunks");
        assert!(matches!(
            backend.ensure_chunk_publication_durable(
                DirectoryPublicationStatus::PublishedButDurabilityUncertain,
                &chunks_dir,
            ),
            Err(StorageError::DurabilityUncertain { .. })
        ));
        assert!(matches!(
            backend.ensure_durability_healthy(),
            Err(StorageError::DurabilityPoisoned { .. })
        ));
        assert!(matches!(
            backend.read_payload_range("missing", 0, 1),
            Err(StorageError::DurabilityPoisoned { .. })
        ));
    }

    #[test]
    fn restart_sums_all_retained_pdp_trees_against_budget() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let first_payload = vec![0x91; 300_000];
        let second_payload = vec![0x92; 300_000];
        let per_tree =
            pdp_tree_memory_for_payload(first_payload.len() as u64).expect("PDP tree estimate");
        let total_limit = per_tree.checked_mul(2).expect("two-tree budget");
        let generous = temp_config_with_pdp_limit(&temp_dir, total_limit);
        let backend = StorageBackend::new(generous).expect("backend init");
        for (payload, root) in [(&first_payload, 0xE7), (&second_payload, 0xE8)] {
            let plan = single_file_plan(payload).expect("plan");
            let manifest = test_manifest(payload, &plan, root);
            let mut reader = payload.as_slice();
            backend
                .ingest_manifest(&manifest, &plan, &mut reader)
                .expect("ingest within summed budget");
        }
        assert_eq!(backend.pdp_tree_memory_bytes(), total_limit);
        drop(backend);

        let constrained = temp_config_with_pdp_limit(&temp_dir, total_limit - 1);
        assert!(matches!(
            StorageBackend::new(constrained),
            Err(StorageError::PdpTreeMemoryExceeded {
                required,
                available
            }) if required == per_tree && available == per_tree - 1
        ));
    }

    #[test]
    fn pdp_tree_reservation_blocks_concurrent_ingest_and_releases_on_failure() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = vec![0xB6; 300_000];
        let required =
            pdp_tree_memory_for_payload(payload.len() as u64).expect("PDP tree estimate");
        let config = temp_config_with_pdp_limit(&temp_dir, required);
        let backend = Arc::new(StorageBackend::new(config).expect("backend init"));
        let plan = single_file_plan(&payload).expect("plan");
        let first_manifest = test_manifest(&payload, &plan, 0xED);
        let second_manifest = test_manifest(&payload, &plan, 0xEE);
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let mut gated = GatedReader {
            bytes: Cursor::new(payload.clone()),
            entered: Some(entered_tx),
            release: release_rx,
            fail_after_release: true,
        };
        let worker_backend = Arc::clone(&backend);
        let worker_plan = plan.clone();
        let worker = thread::spawn(move || {
            worker_backend.ingest_manifest(&first_manifest, &worker_plan, &mut gated)
        });
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("first ingest reaches payload read");
        assert_eq!(backend.reserved_pdp_tree_memory_bytes(), required);

        let mut competing_reader = payload.as_slice();
        assert!(matches!(
            backend.ingest_manifest(&second_manifest, &plan, &mut competing_reader),
            Err(StorageError::PdpTreeMemoryExceeded {
                required: found,
                available: 0
            }) if found == required
        ));
        release_tx.send(()).expect("release failed ingest");
        assert!(worker.join().expect("worker joins").is_err());
        assert_eq!(backend.reserved_pdp_tree_memory_bytes(), 0);
        assert_eq!(backend.pdp_tree_memory_bytes(), 0);

        let mut retry_reader = payload.as_slice();
        backend
            .ingest_manifest(&second_manifest, &plan, &mut retry_reader)
            .expect("reservation released for retry");
        assert_eq!(backend.pdp_tree_memory_bytes(), required);
        assert_eq!(backend.reserved_pdp_tree_memory_bytes(), 0);
    }

    #[test]
    fn pdp_tree_budget_is_released_after_eviction() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let first_payload = vec![0xC7; 300_000];
        let second_payload = vec![0xD8; 300_000];
        let required =
            pdp_tree_memory_for_payload(first_payload.len() as u64).expect("PDP tree estimate");
        let config = temp_config_with_pdp_limit(&temp_dir, required);
        let backend = StorageBackend::new(config).expect("backend init");
        let first_plan = single_file_plan(&first_payload).expect("first plan");
        let first_manifest = test_manifest(&first_payload, &first_plan, 0xEF);
        let mut first_reader = first_payload.as_slice();
        let first_id = backend
            .ingest_manifest(&first_manifest, &first_plan, &mut first_reader)
            .expect("first ingest");
        assert_eq!(backend.pdp_tree_memory_bytes(), required);

        let second_plan = single_file_plan(&second_payload).expect("second plan");
        let second_manifest = test_manifest(&second_payload, &second_plan, 0xF0);
        let mut blocked_reader = second_payload.as_slice();
        assert!(matches!(
            backend.ingest_manifest(&second_manifest, &second_plan, &mut blocked_reader),
            Err(StorageError::PdpTreeMemoryExceeded { available: 0, .. })
        ));
        backend
            .evict_manifest(&first_id)
            .expect("evict first manifest");
        assert_eq!(backend.pdp_tree_memory_bytes(), 0);

        let mut admitted_reader = second_payload.as_slice();
        backend
            .ingest_manifest(&second_manifest, &second_plan, &mut admitted_reader)
            .expect("second ingest after eviction");
        assert_eq!(backend.pdp_tree_memory_bytes(), required);
    }

    #[test]
    fn pdp_witnesses_reject_noncanonical_duplicate_and_out_of_range_samples() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = vec![0x19; 300_000];
        let (_config, backend, manifest_id) = ingest_test_payload(&temp_dir, &payload, 0xF1);
        let invalid_samples = vec![
            Vec::new(),
            vec![PdpSampleV1 {
                segment_index: 0,
                hot_leaf_indices: Vec::new(),
            }],
            vec![
                PdpSampleV1 {
                    segment_index: 0,
                    hot_leaf_indices: vec![0],
                },
                PdpSampleV1 {
                    segment_index: 0,
                    hot_leaf_indices: vec![1],
                },
            ],
            vec![
                PdpSampleV1 {
                    segment_index: 1,
                    hot_leaf_indices: vec![0],
                },
                PdpSampleV1 {
                    segment_index: 0,
                    hot_leaf_indices: vec![0],
                },
            ],
            vec![PdpSampleV1 {
                segment_index: 2,
                hot_leaf_indices: vec![0],
            }],
            vec![PdpSampleV1 {
                segment_index: 0,
                hot_leaf_indices: vec![0, 0],
            }],
            vec![PdpSampleV1 {
                segment_index: 0,
                hot_leaf_indices: vec![2, 1],
            }],
            vec![PdpSampleV1 {
                segment_index: 0,
                hot_leaf_indices: vec![64],
            }],
            vec![PdpSampleV1 {
                segment_index: 1,
                hot_leaf_indices: vec![10],
            }],
        ];
        for (case, samples) in invalid_samples.iter().enumerate() {
            assert!(
                matches!(
                    backend.prove_pdp_samples(&manifest_id, samples),
                    Err(StorageError::PdpWitness { .. })
                ),
                "invalid sample case {case} must fail before returning a witness"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn pdp_witness_reads_reject_symlink_and_hardlink_chunks() {
        use std::os::unix::fs::symlink;

        for hardlink in [false, true] {
            let temp_dir = tempfile::tempdir().expect("create temp dir");
            let payload = vec![0x2A; 32_768];
            let (_config, backend, manifest_id) =
                ingest_test_payload(&temp_dir, &payload, if hardlink { 0xF2 } else { 0xF3 });
            let chunk = backend
                .manifest(&manifest_id)
                .and_then(|stored| stored.chunk(0).cloned())
                .expect("stored chunk");
            let alias = canonical_temp_path(&temp_dir).join("pdp-chunk-alias.bin");
            if hardlink {
                fs::hard_link(&chunk.path, &alias).expect("create hard link");
            } else {
                fs::write(&alias, &payload).expect("write symlink target");
                fs::remove_file(&chunk.path).expect("remove stored chunk");
                symlink(&alias, &chunk.path).expect("install symlink");
            }
            assert!(matches!(
                backend.prove_pdp_samples(&manifest_id, &first_pdp_sample()),
                Err(StorageError::PdpWitness { .. })
            ));
        }
    }

    #[test]
    fn pdp_witness_rejects_chunk_mutation_during_exact_read() {
        let _serial = CHUNK_READ_TEST_SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = vec![0x3B; 32_768];
        let (_config, backend, manifest_id) = ingest_test_payload(&temp_dir, &payload, 0xF4);
        let backend = Arc::new(backend);
        let chunk = backend
            .manifest(&manifest_id)
            .and_then(|stored| stored.chunk(0).cloned())
            .expect("stored chunk");
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        *CHUNK_READ_TEST_HOOK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ChunkReadTestHook {
            path: chunk.path.clone(),
            entered: entered_tx,
            release: release_rx,
        });
        let proof_backend = Arc::clone(&backend);
        let proof_manifest_id = manifest_id.clone();
        let proof = thread::spawn(move || {
            proof_backend.prove_pdp_samples(&proof_manifest_id, &first_pdp_sample())
        });
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("proof read opened the chunk");
        let mut mutated = fs::read(&chunk.path).expect("read chunk for mutation");
        mutated[0] ^= 0x80;
        fs::write(&chunk.path, mutated).expect("mutate opened chunk");
        release_tx.send(()).expect("release proof read");
        assert!(matches!(
            proof.join().expect("proof worker joins"),
            Err(StorageError::PdpWitness { .. })
        ));
    }

    #[test]
    fn pdp_proof_io_lease_blocks_concurrent_eviction() {
        let _serial = CHUNK_READ_TEST_SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = vec![0x4C; 32_768];
        let (_config, backend, manifest_id) = ingest_test_payload(&temp_dir, &payload, 0xF5);
        let backend = Arc::new(backend);
        let chunk = backend
            .manifest(&manifest_id)
            .and_then(|stored| stored.chunk(0).cloned())
            .expect("stored chunk");
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        *CHUNK_READ_TEST_HOOK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(ChunkReadTestHook {
            path: chunk.path,
            entered: entered_tx,
            release: release_rx,
        });
        let proof_backend = Arc::clone(&backend);
        let proof_manifest_id = manifest_id.clone();
        let proof = thread::spawn(move || {
            proof_backend.prove_pdp_samples(&proof_manifest_id, &first_pdp_sample())
        });
        entered_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("proof holds manifest I/O lease");

        let (evicted_tx, evicted_rx) = mpsc::channel();
        let evict_backend = Arc::clone(&backend);
        let evict_manifest_id = manifest_id.clone();
        let eviction = thread::spawn(move || {
            evicted_tx
                .send(evict_backend.evict_manifest(&evict_manifest_id))
                .expect("send eviction result");
        });
        assert!(matches!(
            evicted_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        release_tx.send(()).expect("release proof read");
        assert_eq!(
            proof
                .join()
                .expect("proof worker joins")
                .expect("proof completes")
                .len(),
            1
        );
        assert_eq!(
            evicted_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("eviction completes")
                .expect("eviction succeeds"),
            payload.len() as u64
        );
        eviction.join().expect("eviction worker joins");
        assert!(backend.manifest(&manifest_id).is_none());
        assert_eq!(backend.pdp_tree_memory_bytes(), 0);
    }

    #[test]
    fn restart_rehydrates_manifest_index() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let cfg = temp_config(&temp_dir);
        let backend = StorageBackend::new(cfg.clone()).expect("backend init");

        let payload = b"Persistent storage test payload";
        let plan = single_file_plan(payload).expect("plan");
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xEF; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = backend
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        drop(backend);

        let backend_reloaded = StorageBackend::new(cfg).expect("reloaded backend");
        let stored = backend_reloaded
            .manifest(&manifest_id)
            .expect("manifest restored");
        assert_eq!(stored.content_length(), plan.content_length);

        let bytes = backend_reloaded
            .read_payload_range(&manifest_id, 0, payload.len())
            .expect("read after restart");
        assert_eq!(bytes, payload);
    }

    #[test]
    fn restart_rejects_same_length_chunk_corruption() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"restart integrity verification";
        let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xC1);
        let chunk_path = backend
            .manifest(&manifest_id)
            .and_then(|stored| stored.chunk(0).map(|chunk| chunk.path.clone()))
            .expect("chunk path");
        drop(backend);
        let mut corrupted = payload.to_vec();
        corrupted[0] ^= 0x01;
        fs::write(&chunk_path, corrupted).expect("corrupt chunk");

        assert!(matches!(
            StorageBackend::new(config),
            Err(StorageError::CorruptStorageState { .. })
        ));
    }

    #[test]
    fn restart_rejects_unsupported_index_version_and_duplicate_ids() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"index validation payload";
        let (config, backend, _) = ingest_test_payload(&temp_dir, payload, 0xC2);
        let index_path = backend.index_path.clone();
        drop(backend);
        let bytes = fs::read(&index_path).expect("read index");
        let mut index: ManifestIndex = norito::decode_from_bytes(&bytes).expect("decode index");
        index.version = INDEX_VERSION_V1 + 1;
        fs::write(&index_path, norito::to_bytes(&index).expect("encode index"))
            .expect("write unsupported index");
        assert!(matches!(
            StorageBackend::new(config.clone()),
            Err(StorageError::UnsupportedIndexVersion { .. })
        ));

        index.version = INDEX_VERSION_V1;
        index.entries.push(index.entries[0].clone());
        fs::write(
            &index_path,
            norito::to_bytes(&index).expect("encode duplicate index"),
        )
        .expect("write duplicate index");
        assert!(matches!(
            StorageBackend::new(config),
            Err(StorageError::CorruptStorageState { .. })
        ));
    }

    #[test]
    fn restart_rejects_noncanonical_compressed_index() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"canonical index encoding";
        let (config, backend, _) = ingest_test_payload(&temp_dir, payload, 0xC9);
        let index_path = backend.index_path.clone();
        let bytes = fs::read(&index_path).expect("read index");
        let index: ManifestIndex = norito::decode_from_bytes(&bytes).expect("decode index");
        let compressed =
            norito::to_compressed_bytes(&index, Some(norito::CompressionConfig::default()))
                .expect("encode compressed index");
        assert_ne!(compressed, bytes, "compressed form must be noncanonical");
        drop(backend);
        fs::write(&index_path, compressed).expect("write compressed index");

        assert!(matches!(
            StorageBackend::new(config),
            Err(StorageError::CorruptStorageState { .. })
        ));
    }

    #[test]
    fn storage_decode_limits_bound_all_resource_dimensions() {
        let limits = storage_decode_limits(1_024);
        assert_eq!(limits.max_sequence_elements(), 1_024);
        assert_eq!(limits.max_field_bytes(), 1_024);
        assert_eq!(limits.max_total_elements(), 1_024);
        assert_eq!(limits.max_total_allocated_bytes(), 4_096);
        assert_eq!(limits.max_nesting_depth(), 64);
    }

    #[test]
    fn restart_rejects_manifest_id_path_traversal() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"manifest id containment";
        let (config, backend, _) = ingest_test_payload(&temp_dir, payload, 0xC3);
        let index_path = backend.index_path.clone();
        drop(backend);
        let bytes = fs::read(&index_path).expect("read index");
        let mut index: ManifestIndex = norito::decode_from_bytes(&bytes).expect("decode index");
        index.entries[0].manifest_id = "../outside".to_string();
        fs::write(&index_path, norito::to_bytes(&index).expect("encode index"))
            .expect("write traversing index");

        assert!(matches!(
            StorageBackend::new(config),
            Err(StorageError::CorruptStorageState { .. })
        ));
    }

    #[test]
    fn restart_rejects_chunk_filename_traversal() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"chunk metadata containment";
        let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xC4);
        let metadata_path = backend
            .manifests_dir
            .join(manifest_id)
            .join(METADATA_FILE_NAME);
        drop(backend);
        let bytes = fs::read(&metadata_path).expect("read metadata");
        let mut record: StoredManifestRecord =
            norito::decode_from_bytes(&bytes).expect("decode metadata");
        record.chunk_files[0].file_name = "../../outside.bin".to_string();
        fs::write(
            &metadata_path,
            norito::to_bytes(&record).expect("encode metadata"),
        )
        .expect("write traversing metadata");

        assert!(matches!(
            StorageBackend::new(config),
            Err(StorageError::CorruptStorageState { .. })
        ));
    }

    #[test]
    fn restart_rejects_nonportable_logical_file_paths() {
        for component in [".", "..", "nested/name", "windows\\name", "line\nbreak"] {
            let temp_dir = tempfile::tempdir().expect("create temp dir");
            let payload = b"logical path containment";
            let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xC6);
            let metadata_path = backend
                .manifests_dir
                .join(manifest_id)
                .join(METADATA_FILE_NAME);
            drop(backend);
            let bytes = fs::read(&metadata_path).expect("read metadata");
            let mut record: StoredManifestRecord =
                norito::decode_from_bytes(&bytes).expect("decode metadata");
            record.files[0].path = vec![component.to_string()];
            fs::write(
                &metadata_path,
                norito::to_bytes(&record).expect("encode metadata"),
            )
            .expect("write invalid metadata");

            assert!(
                matches!(
                    StorageBackend::new(config),
                    Err(StorageError::CorruptStorageState { .. })
                ),
                "component {component:?} must be rejected"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn restart_rejects_symlinked_manifest_artifacts() {
        use std::os::unix::fs::symlink;

        for artifact_name in [MANIFEST_FILE_NAME, METADATA_FILE_NAME] {
            let temp_dir = tempfile::tempdir().expect("create temp dir");
            let payload = b"symlinked storage metadata";
            let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xC7);
            let artifact_path = backend.manifests_dir.join(manifest_id).join(artifact_name);
            let replacement_path = canonical_temp_path(&temp_dir).join(artifact_name);
            fs::copy(&artifact_path, &replacement_path).expect("copy artifact");
            drop(backend);
            fs::remove_file(&artifact_path).expect("remove artifact");
            symlink(&replacement_path, &artifact_path).expect("install symlink");

            assert!(
                StorageBackend::new(config).is_err(),
                "symlinked {artifact_name} must be rejected"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn restart_rejects_hardlinked_persistent_artifacts() {
        for artifact_name in ["index", MANIFEST_FILE_NAME, METADATA_FILE_NAME] {
            let temp_dir = tempfile::tempdir().expect("create temp dir");
            let payload = b"hard-linked storage metadata";
            let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xC8);
            let artifact_path = if artifact_name == "index" {
                backend.index_path.clone()
            } else {
                backend.manifests_dir.join(manifest_id).join(artifact_name)
            };
            let alias_path = canonical_temp_path(&temp_dir).join(format!("{artifact_name}.alias"));
            fs::hard_link(&artifact_path, &alias_path).expect("create artifact hard link");
            drop(backend);

            assert!(
                matches!(
                    StorageBackend::new(config),
                    Err(StorageError::CorruptStorageState { .. })
                ),
                "hard-linked {artifact_name} must be rejected"
            );
        }
    }

    #[test]
    fn restart_rejects_oversized_metadata_before_allocation() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"bounded metadata loading";
        let (config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xC5);
        let metadata_path = backend
            .manifests_dir
            .join(manifest_id)
            .join(METADATA_FILE_NAME);
        drop(backend);
        fs::OpenOptions::new()
            .write(true)
            .open(&metadata_path)
            .expect("open metadata")
            .set_len(MAX_MANIFEST_METADATA_BYTES + 1)
            .expect("extend metadata sparsely");

        assert!(matches!(
            StorageBackend::new(config),
            Err(StorageError::CorruptStorageState { .. })
        ));
    }

    #[test]
    fn write_atomic_uses_added_extension_and_cleans_up_temp_file() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let temp_path = canonical_temp_path(&temp_dir);
        let target_with_ext = temp_path.join("bundle.car");

        write_atomic(&target_with_ext, b"hello").expect("write with extension");
        assert_eq!(fs::read(&target_with_ext).expect("read bundle"), b"hello");
        assert!(
            !target_with_ext.with_added_extension(ATOMIC_EXT).exists(),
            "temporary file should not remain on disk"
        );

        let target_no_ext = temp_path.join("manifest");
        write_atomic(&target_no_ext, b"x").expect("write without extension");
        assert_eq!(fs::read(&target_no_ext).expect("read manifest"), b"x");
        assert!(
            !target_no_ext.with_added_extension(ATOMIC_EXT).exists(),
            "temporary file should be removed even when original path has no extension"
        );
    }

    #[test]
    fn write_atomic_reports_post_rename_directory_sync_failure() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let target = canonical_temp_path(&temp_dir).join("index.norito");
        fs::write(&target, b"old").expect("seed target");

        let error = write_atomic_with_directory_sync(&target, b"new", |_| {
            Err(io::Error::other("injected directory sync failure"))
        })
        .expect_err("post-rename sync failure must be classified");

        assert!(matches!(
            error,
            AtomicWriteError::DurabilityUncertain { .. }
        ));
        assert_eq!(
            fs::read(&target).expect("read committed replacement"),
            b"new",
            "rename committed even though directory durability is uncertain"
        );
        let leftovers = fs::read_dir(target.parent().expect("target parent"))
            .expect("read target parent")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("index.norito.tmp.")
            })
            .collect::<Vec<_>>();
        assert!(leftovers.is_empty(), "temporary file must not remain");
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
    #[test]
    fn pinned_atomic_parent_rename_cannot_be_redirected_by_path_swap() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = canonical_temp_path(&temp_dir);
        let parent = root.join("live");
        let moved_parent = root.join("moved");
        let redirect = root.join("redirect");
        fs::create_dir(&parent).expect("create live parent");
        fs::create_dir(&redirect).expect("create redirect parent");
        let temporary = parent.join("index.tmp");
        let destination = parent.join("index");
        fs::write(&temporary, b"pinned").expect("write pinned temporary");
        let pinned = AtomicParentDirectory::open(&parent).expect("pin live parent");

        fs::rename(&parent, &moved_parent).expect("move pinned parent");
        std::os::unix::fs::symlink(&redirect, &parent).expect("redirect live parent path");
        assert!(
            pinned.verify_path_identity().is_err(),
            "path identity change must be detected"
        );
        pinned
            .rename_entry(&temporary, &destination)
            .expect("descriptor-relative rename remains inside pinned directory");

        assert_eq!(
            fs::read(moved_parent.join("index")).expect("read pinned destination"),
            b"pinned"
        );
        assert!(
            !redirect.join("index").exists(),
            "swapped path must not receive the committed entry"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "android", target_os = "macos"))]
    #[test]
    fn post_rename_parent_path_swap_is_durability_uncertain_not_precommit() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = canonical_temp_path(&temp_dir);
        let parent = root.join("live");
        let moved_parent = root.join("moved");
        let redirect = root.join("redirect");
        fs::create_dir(&parent).expect("create live parent");
        fs::create_dir(&redirect).expect("create redirect parent");
        let target = parent.join("index");
        fs::write(&target, b"old").expect("seed target");

        let error = write_atomic_with_directory_sync(&target, b"new", |pinned| {
            fs::rename(&parent, &moved_parent)?;
            std::os::unix::fs::symlink(&redirect, &parent)?;
            pinned.sync()
        })
        .expect_err("post-rename parent identity loss must be uncertain");

        assert!(matches!(
            error,
            AtomicWriteError::DurabilityUncertain { .. }
        ));
        assert_eq!(
            fs::read(moved_parent.join("index")).expect("read committed replacement"),
            b"new"
        );
        assert!(
            !redirect.join("index").exists(),
            "swapped parent must not receive the replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_temp_commit_validation_rejects_path_replacement_and_hardlinks() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = canonical_temp_path(&temp_dir);
        let temporary = root.join("index.tmp");
        let displaced = root.join("index.displaced");
        let mut file = open_atomic_temp_file(&temporary).expect("open atomic temporary");
        file.write_all(b"stable").expect("write temporary");
        file.sync_all().expect("sync temporary");
        let stable = file.metadata().expect("capture stable temporary identity");
        validate_atomic_open_file_identity(
            &temporary,
            &file,
            &stable,
            b"stable".len(),
            "atomic temporary",
        )
        .expect("stable temporary is accepted");

        fs::hard_link(&temporary, root.join("index.alias")).expect("hard-link temporary");
        assert!(
            validate_atomic_open_file_identity(
                &temporary,
                &file,
                &stable,
                b"stable".len(),
                "atomic temporary",
            )
            .is_err(),
            "a second hard link must invalidate the temporary"
        );
        fs::remove_file(root.join("index.alias")).expect("remove hard-link alias");

        fs::rename(&temporary, &displaced).expect("displace opened temporary");
        fs::write(&temporary, b"stable").expect("replace temporary path with same-size inode");
        assert!(
            validate_atomic_open_file_identity(
                &temporary,
                &file,
                &stable,
                b"stable".len(),
                "atomic temporary",
            )
            .is_err(),
            "same-size path replacement must not retain trusted identity"
        );
    }

    #[test]
    fn uncertain_commit_fail_stops_subsequent_storage_operations() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"fail-stop durability payload";
        let (_config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xD3);
        let uncertainty = AtomicWriteError::DurabilityUncertain {
            path: backend.index_path.clone(),
            source: io::Error::other("injected directory sync failure"),
        };
        backend.fail_stop_durability(&uncertainty);

        assert!(matches!(
            backend.read_payload_range(&manifest_id, 0, 1),
            Err(StorageError::DurabilityPoisoned { .. })
        ));

        let plan = single_file_plan(payload).expect("plan");
        let manifest = test_manifest(payload, &plan, 0xD4);
        let mut reader = &payload[..];
        assert!(matches!(
            backend.ingest_manifest(&manifest, &plan, &mut reader),
            Err(StorageError::DurabilityPoisoned { .. })
        ));
    }

    #[test]
    fn repair_replacement_fail_stops_after_post_rename_sync_failure() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let payload = b"repair replacement durability payload";
        let (_config, backend, manifest_id) = ingest_test_payload(&temp_dir, payload, 0xD5);

        let replacement = backend
            .with_manifest_io(&manifest_id, |manifest| {
                let chunk = manifest.chunk(0).expect("repair chunk").clone();
                backend.replace_chunk_for_repair_with_directory_sync(
                    manifest,
                    &chunk,
                    payload,
                    |_| Err(io::Error::other("injected repair directory sync failure")),
                )
            })
            .expect("acquire repair lifecycle lease");

        assert!(matches!(
            replacement,
            Err(StorageError::DurabilityUncertain { .. })
        ));
        assert!(matches!(
            backend.ensure_durability_healthy(),
            Err(StorageError::DurabilityPoisoned { .. })
        ));
    }

    #[test]
    fn write_atomic_allows_concurrent_writes_to_same_target() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let temp_path = canonical_temp_path(&temp_dir);
        let target = temp_path.join("index.norito");
        let payloads = (0..16)
            .map(|idx| format!("payload-{idx}").into_bytes())
            .collect::<Vec<_>>();

        std::thread::scope(|scope| {
            for payload in &payloads {
                let target = &target;
                scope.spawn(move || write_atomic(target, payload).expect("concurrent write"));
            }
        });

        let final_payload = fs::read(&target).expect("read final payload");
        assert!(payloads.iter().any(|payload| payload == &final_payload));

        let leftovers = fs::read_dir(temp_path)
            .expect("read temp dir")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("index.norito.tmp.")
            })
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "temporary files should not remain after concurrent writes"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_rejects_symlink_output() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let temp_path = canonical_temp_path(&temp_dir);
        let target_path = temp_path.join("target.norito");
        fs::write(&target_path, b"unchanged\n").expect("write target");
        let output_path = temp_path.join("index.norito");
        std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");

        let err = write_atomic(&output_path, b"replace").expect_err("reject symlink output");
        let message = err.to_string();

        assert!(
            message.contains("must not be a symlink"),
            "unexpected error: {message}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_rejects_symlink_parent() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let temp_path = canonical_temp_path(&temp_dir);
        let real_dir = temp_path.join("real");
        fs::create_dir(&real_dir).expect("create real dir");
        let linked_dir = temp_path.join("linked");
        std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
        let output_path = linked_dir.join("index.norito");

        let err = write_atomic(&output_path, b"replace").expect_err("reject symlink parent");
        let message = err.to_string();

        assert!(
            message.contains("parent") && message.contains("must not be a symlink"),
            "unexpected error: {message}"
        );
        assert!(
            !real_dir.join("index.norito").exists(),
            "symlink parent should not receive output"
        );
    }

    #[cfg(unix)]
    #[test]
    fn open_atomic_temp_file_rejects_preexisting_symlink() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let temp_path = canonical_temp_path(&temp_dir);
        let target_path = temp_path.join("target.tmp");
        fs::write(&target_path, b"unchanged\n").expect("write target");
        let tmp_path = temp_path.join("index.norito.tmp");
        std::os::unix::fs::symlink(&target_path, &tmp_path).expect("create symlink");

        let err = open_atomic_temp_file(&tmp_path).expect_err("reject temp symlink");
        let message = err.to_string();

        assert!(
            message.contains("failed to create atomic temp"),
            "unexpected error: {message}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
    }
}
