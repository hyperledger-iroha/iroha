//! Persistent storage backend for the embedded SoraFS node.
//!
//! This module wraps `sorafs_car::ChunkStore` with an on-disk manifest index,
//! deterministic chunk layout, Proof-of-Retrievability (PoR) resume support,
//! and quota enforcement derived from Torii storage configuration.

#![allow(unexpected_cfgs)]

use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use blake3::Hash;
use hex::ToHex;
use iroha_data_model::da::{ingest::DaStripeLayout, manifest::ChunkRole};
use norito::{
    core::Error as NoritoError,
    derive::{NoritoDeserialize, NoritoSerialize},
};
use sorafs_car::{
    self, CarBuildPlan, CarChunk, ChunkStore, ChunkStoreError, FilePlan, PayloadSource,
    PorChunkTree, PorLeaf, PorMerkleTree, PorProof, PorSegment, TaikaiSegmentHint,
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    MANIFEST_VERSION_V1, ManifestV1,
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
static GC_TRASH_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Errors raised by the SoraFS storage backend.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Encountered an unexpected I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    /// Failed to encode or decode a Norito payload while persisting metadata.
    #[error("Norito codec error: {0}")]
    Norito(#[from] NoritoError),
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
    /// Failed to rebuild the PoR tree from persisted chunk data.
    #[error("failed to build PoR tree: {0}")]
    ChunkStore(#[from] ChunkStoreError),
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
    state: RwLock<StorageState>,
}

#[derive(Debug)]
struct StorageState {
    index: ManifestIndex,
    manifests: BTreeMap<String, StoredManifest>,
    total_bytes: u64,
    reserved_bytes: u64,
    access_counter: u64,
    chunk_refcounts: BTreeMap<[u8; 32], u32>,
}

impl StorageState {
    fn available_capacity(&self, max_capacity: u64) -> u64 {
        max_capacity
            .saturating_sub(self.total_bytes)
            .saturating_sub(self.reserved_bytes)
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
    por_tree: StoredPorTree,
    manifest_path: PathBuf,
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
            por_tree: parts.por_tree,
            manifest_path: parts.manifest_path,
        }
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
        let end = offset
            .checked_add(len as u64)
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
            let chunk_end = chunk_start + u64::from(chunk.length);

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
                chunks.push(chunk.clone());
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
                chunks.push(chunk.clone());
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
        let bytes = fs::read(self.manifest_path())?;
        let manifest = norito::decode_from_bytes(&bytes)?;
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

    /// Build an in-memory PoR tree for the stored manifest.
    #[must_use]
    pub fn por_tree(&self) -> PorMerkleTree {
        self.por_tree.to_merkle_tree()
    }

    /// Path to the Norito-encoded manifest bytes stored on disk.
    #[must_use]
    pub fn manifest_path(&self) -> &Path {
        self.manifest_path.as_path()
    }
}

/// Metadata describing an individual stored chunk.
#[derive(Debug, Clone)]
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

/// Persistent representation of the PoR tree used for manifest resume.
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
pub struct StoredPorTree {
    root: [u8; 32],
    payload_len: u64,
    chunks: Vec<StoredPorChunk>,
}

impl StoredPorTree {
    /// Rebuild an in-memory PoR tree from the stored representation.
    #[must_use]
    pub fn to_merkle_tree(&self) -> PorMerkleTree {
        let mut chunk_nodes = Vec::with_capacity(self.chunks.len());
        let mut chunk_roots = Vec::with_capacity(self.chunks.len());
        for chunk in &self.chunks {
            let (node, root) = chunk.to_chunk_tree();
            chunk_nodes.push(node);
            chunk_roots.push(root);
        }
        PorMerkleTree::from_chunks(chunk_nodes, chunk_roots, self.payload_len)
    }
}

impl From<&PorMerkleTree> for StoredPorTree {
    fn from(tree: &PorMerkleTree) -> Self {
        let chunks = tree
            .chunks()
            .iter()
            .map(StoredPorChunk::from)
            .collect::<Vec<_>>();
        Self {
            root: *tree.root(),
            payload_len: tree.payload_len(),
            chunks,
        }
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct StoredPorChunk {
    chunk_index: u32,
    offset: u64,
    length: u32,
    chunk_digest: [u8; 32],
    root: [u8; 32],
    segments: Vec<StoredPorSegment>,
}

impl StoredPorChunk {
    fn to_chunk_tree(&self) -> (PorChunkTree, [u8; 32]) {
        let segments = self
            .segments
            .iter()
            .map(StoredPorSegment::to_segment)
            .collect::<Vec<_>>();
        (
            PorChunkTree {
                chunk_index: self.chunk_index as usize,
                offset: self.offset,
                length: self.length,
                chunk_digest: self.chunk_digest,
                root: self.root,
                segments: segments.clone(),
            },
            self.root,
        )
    }
}

impl From<&PorChunkTree> for StoredPorChunk {
    fn from(chunk: &PorChunkTree) -> Self {
        let segments = chunk
            .segments
            .iter()
            .map(StoredPorSegment::from)
            .collect::<Vec<_>>();
        Self {
            chunk_index: chunk.chunk_index as u32,
            offset: chunk.offset,
            length: chunk.length,
            chunk_digest: chunk.chunk_digest,
            root: chunk.root,
            segments,
        }
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct StoredPorSegment {
    offset: u64,
    length: u32,
    digest: [u8; 32],
    leaves: Vec<StoredPorLeaf>,
}

impl StoredPorSegment {
    fn to_segment(&self) -> PorSegment {
        PorSegment {
            offset: self.offset,
            length: self.length,
            digest: self.digest,
            leaves: self
                .leaves
                .iter()
                .map(StoredPorLeaf::to_leaf)
                .collect::<Vec<_>>(),
        }
    }
}

impl From<&PorSegment> for StoredPorSegment {
    fn from(segment: &PorSegment) -> Self {
        Self {
            offset: segment.offset,
            length: segment.length,
            digest: segment.digest,
            leaves: segment
                .leaves
                .iter()
                .map(StoredPorLeaf::from)
                .collect::<Vec<_>>(),
        }
    }
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize, PartialEq, Eq)]
struct StoredPorLeaf {
    offset: u64,
    length: u32,
    digest: [u8; 32],
}

impl StoredPorLeaf {
    fn to_leaf(&self) -> PorLeaf {
        PorLeaf {
            offset: self.offset,
            length: self.length,
            digest: self.digest,
        }
    }
}

impl From<&PorLeaf> for StoredPorLeaf {
    fn from(leaf: &PorLeaf) -> Self {
        Self {
            offset: leaf.offset,
            length: leaf.length,
            digest: leaf.digest,
        }
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
    por_tree: StoredPorTree,
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

fn chunk_refcount_map(entries: &[ChunkRefcountEntry]) -> BTreeMap<[u8; 32], u32> {
    let mut map = BTreeMap::new();
    for entry in entries {
        if entry.count == 0 {
            continue;
        }
        map.insert(entry.digest, entry.count);
    }
    map
}

fn chunk_refcount_entries(map: &BTreeMap<[u8; 32], u32>) -> Vec<ChunkRefcountEntry> {
    map.iter()
        .map(|(digest, count)| ChunkRefcountEntry {
            digest: *digest,
            count: *count,
        })
        .collect()
}

impl StorageBackend {
    /// Create a new storage backend rooted at the directory described by `config`.
    pub fn new(config: StorageConfig) -> Result<Self, StorageError> {
        let root_dir = config.data_dir().clone();
        let manifests_dir = root_dir.join(MANIFEST_DIR_NAME);
        let index_path = root_dir.join("index.norito");

        fs::create_dir_all(&manifests_dir)?;

        let mut index = if index_path.exists() {
            let bytes = fs::read(&index_path)?;
            norito::decode_from_bytes(&bytes)?
        } else {
            ManifestIndex::default()
        };

        let mut total_bytes = index.total_bytes;
        let mut manifests = BTreeMap::new();
        let mut chunk_refcounts: BTreeMap<[u8; 32], u32> = BTreeMap::new();
        let mut access_counter = 0u64;

        for entry in &mut index.entries {
            let manifest_dir = manifests_dir.join(&entry.manifest_id);
            let metadata_path = manifest_dir.join(METADATA_FILE_NAME);
            let manifest_path = manifest_dir.join(MANIFEST_FILE_NAME);

            let metadata_bytes = fs::read(&metadata_path)?;
            let mut record: StoredManifestRecord = norito::decode_from_bytes(&metadata_bytes)?;
            if record.retention_source.is_none() {
                record.retention_source = entry.retention_source.clone();
            }
            if entry.retention_source.is_none() {
                entry.retention_source = record.retention_source.clone();
            }
            let last_access = record.last_access.max(entry.last_access);
            record.last_access = last_access;
            entry.last_access = last_access;
            access_counter = access_counter.max(last_access);

            let manifest = StoredManifest::from_record(record, manifest_path);
            for chunk in &manifest.chunk_files {
                let counter = chunk_refcounts.entry(chunk.digest).or_insert(0);
                *counter = counter.saturating_add(1);
            }

            manifests.insert(entry.manifest_id.clone(), manifest);
        }

        let mut index_dirty = false;
        let stored_refcounts = chunk_refcount_map(&index.chunk_refcounts);
        if stored_refcounts != chunk_refcounts {
            iroha_logger::warn!(
                stored = stored_refcounts.len(),
                computed = chunk_refcounts.len(),
                "chunk refcount index mismatch; rebuilding from manifests"
            );
            index.chunk_refcounts = chunk_refcount_entries(&chunk_refcounts);
            index_dirty = true;
        }

        if total_bytes == 0 {
            total_bytes = manifests
                .values()
                .map(|manifest| manifest.content_length)
                .sum();
            index.total_bytes = total_bytes;
            index_dirty = true;
        }

        if index_dirty {
            if let Ok(bytes) = norito::to_bytes(&index) {
                if let Err(err) = write_atomic(&index_path, &bytes) {
                    iroha_logger::warn!(
                        %err,
                        path = %index_path.display(),
                        "failed to persist rebuilt storage index"
                    );
                }
            }
        }

        let state = StorageState {
            index,
            manifests,
            total_bytes,
            reserved_bytes: 0,
            access_counter,
            chunk_refcounts,
        };

        Ok(Self {
            config,
            root_dir,
            manifests_dir,
            index_path,
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

    /// Returns the remaining capacity (in bytes) under the configured quota.
    #[must_use]
    pub fn available_capacity(&self) -> u64 {
        let max_capacity = self.config.max_capacity_bytes().0;
        self.state
            .read()
            .expect("storage state poisoned")
            .available_capacity(max_capacity)
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
        chunk_refcount_entries(&state.chunk_refcounts)
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
        let state = self.state.read().expect("storage state poisoned");
        let manifest =
            state
                .manifests
                .get(manifest_id)
                .ok_or_else(|| StorageError::ManifestNotFound {
                    manifest_id: manifest_id.to_owned(),
                })?;
        for chunk in &manifest.chunk_files {
            if state
                .chunk_refcounts
                .get(&chunk.digest)
                .is_some_and(|count| *count > 1)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Evict a stored manifest and reclaim its payload bytes.
    pub fn evict_manifest(&self, manifest_id: &str) -> Result<u64, StorageError> {
        let mut state = self.state.write().expect("storage state poisoned");
        let stored = state
            .manifests
            .get(manifest_id)
            .ok_or_else(|| StorageError::ManifestNotFound {
                manifest_id: manifest_id.to_owned(),
            })?
            .clone();
        let manifest_dir = stored
            .manifest_path()
            .parent()
            .expect("manifest path must have parent")
            .to_path_buf();
        let mut new_index = state.index.clone();
        let mut refcounts = state.chunk_refcounts.clone();
        for chunk in &stored.chunk_files {
            if let Some(count) = refcounts.get_mut(&chunk.digest) {
                if *count <= 1 {
                    refcounts.remove(&chunk.digest);
                } else {
                    *count = count.saturating_sub(1);
                }
            }
        }
        new_index
            .entries
            .retain(|entry| entry.manifest_id != manifest_id);
        new_index.total_bytes = state.total_bytes.saturating_sub(stored.content_length());
        new_index.gc_freed_bytes_total = new_index
            .gc_freed_bytes_total
            .saturating_add(stored.content_length());
        new_index.gc_evictions_total = new_index.gc_evictions_total.saturating_add(1);
        new_index.chunk_refcounts = chunk_refcount_entries(&refcounts);

        let index_bytes = norito::to_bytes(&new_index).map_err(StorageError::Norito)?;
        write_atomic(&self.index_path, &index_bytes)?;

        state.index = new_index;
        state.total_bytes = state.total_bytes.saturating_sub(stored.content_length());
        state.manifests.remove(manifest_id);
        state.chunk_refcounts = refcounts;
        drop(state);

        let trash_path = self.gc_trash_path(manifest_id);
        if let Some(parent) = trash_path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                iroha_logger::warn!(
                    %err,
                    manifest_id = %manifest_id,
                    path = %parent.display(),
                    "failed to prepare GC trash directory"
                );
            }
        }
        if let Err(err) = fs::rename(&manifest_dir, &trash_path) {
            iroha_logger::warn!(
                %err,
                manifest_id = %manifest_id,
                path = %manifest_dir.display(),
                "failed to move manifest into GC trash; deleting in place"
            );
            if let Err(err) = fs::remove_dir_all(&manifest_dir) {
                iroha_logger::warn!(
                    %err,
                    manifest_id = %manifest_id,
                    path = %manifest_dir.display(),
                    "failed to delete manifest directory"
                );
            }
        } else if let Err(err) = fs::remove_dir_all(&trash_path) {
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
        let mut state = self.state.write().expect("storage state poisoned");
        let manifest =
            state
                .manifests
                .get_mut(manifest_id)
                .ok_or_else(|| StorageError::ManifestNotFound {
                    manifest_id: manifest_id.to_owned(),
                })?;

        let expected = manifest.chunk_files.len();
        if chunk_roles.len() != expected {
            return Err(StorageError::ChunkRoleLengthMismatch {
                expected,
                actual: chunk_roles.len(),
            });
        }

        manifest.stripe_layout = Some(stripe_layout);
        for (chunk, role) in manifest.chunk_files.iter_mut().zip(chunk_roles.iter()) {
            chunk.role = Some(role.role);
            chunk.group_id = Some(role.group_id);
        }

        let record = manifest.to_record();
        let metadata_path = manifest
            .manifest_path
            .parent()
            .expect("manifest path must have parent")
            .join(METADATA_FILE_NAME);
        write_manifest_metadata(&record, &metadata_path)?;
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
        if manifest.version != MANIFEST_VERSION_V1 {
            return Err(StorageError::UnsupportedManifestVersion {
                version: manifest.version,
            });
        }

        let manifest_bytes = manifest.encode()?;
        let manifest_digest: [u8; 32] = manifest.digest()?.into();
        let manifest_id = hex::encode(manifest_digest);
        let payload_digest = *plan.payload_digest.as_bytes();
        let required_bytes = plan.content_length;

        ensure_chunk_profile_match(manifest, plan)?;

        let mut state = self.state.write().expect("storage state poisoned");

        if state.manifests.contains_key(&manifest_id) {
            return Err(StorageError::ManifestExists {
                manifest_id: manifest_id.clone(),
            });
        }

        if state.manifests.len() >= self.config.max_pins() {
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

        state.reserved_bytes = state
            .reserved_bytes
            .checked_add(required_bytes)
            .expect("reserved bytes overflow");
        drop(state);

        let manifest_dir = self.manifests_dir.join(&manifest_id);
        let chunks_dir = manifest_dir.join(CHUNKS_DIR_NAME);
        let manifest_path = manifest_dir.join(MANIFEST_FILE_NAME);
        let metadata_path = manifest_dir.join(METADATA_FILE_NAME);

        let ingest_result = self.ingest_payload(plan, reader, &chunks_dir);

        let (mut chunk_records, por_tree) = match ingest_result {
            Ok(value) => value,
            Err(err) => {
                let mut state = self.state.write().expect("storage state poisoned");
                state.reserved_bytes = state.reserved_bytes.saturating_sub(required_bytes);
                drop(state);
                let _ = fs::remove_dir_all(&manifest_dir);
                return Err(err);
            }
        };

        if let Some(roles) = chunk_roles {
            let expected = chunk_records.len();
            if roles.len() != expected {
                let mut state = self.state.write().expect("storage state poisoned");
                state.reserved_bytes = state.reserved_bytes.saturating_sub(required_bytes);
                drop(state);
                let _ = fs::remove_dir_all(&manifest_dir);
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

        if let Err(err) = fs::create_dir_all(&manifest_dir) {
            let mut state = self.state.write().expect("storage state poisoned");
            state.reserved_bytes = state.reserved_bytes.saturating_sub(required_bytes);
            drop(state);
            let _ = fs::remove_dir_all(&manifest_dir);
            return Err(StorageError::from(err));
        }

        if let Err(err) = write_atomic(&manifest_path, &manifest_bytes) {
            let mut state = self.state.write().expect("storage state poisoned");
            state.reserved_bytes = state.reserved_bytes.saturating_sub(required_bytes);
            drop(state);
            let _ = fs::remove_dir_all(&manifest_dir);
            return Err(StorageError::from(err));
        }

        let stored_at_unix_secs = unix_timestamp();
        let retention_source = RetentionSourceV1::from_manifest(manifest)?;
        let retention_epoch = retention_source.effective_epoch();
        let files = stored_files_from_plan(plan);
        let persisted_files = files
            .iter()
            .map(|file| StoredFileRecordNorito {
                path: file.path.clone(),
                offset: file.offset,
                size: file.size,
                first_chunk: file.first_chunk as u32,
                chunk_count: file.chunk_count as u32,
            })
            .collect();
        let last_access = {
            let mut state = self.state.write().expect("storage state poisoned");
            let next_access = state.access_counter.saturating_add(1);
            state.access_counter = next_access;
            next_access
        };
        let chunk_count = plan.chunks.len() as u32;

        let metadata_record = StoredManifestRecord {
            manifest_id: manifest_id.clone(),
            manifest_cid: manifest.root_cid.clone(),
            manifest_digest,
            payload_digest,
            content_length: plan.content_length,
            chunk_profile_handle: canonical_profile_handle(manifest),
            stripe_layout,
            stored_at_unix_secs,
            retention_epoch,
            retention_source: Some(retention_source.clone()),
            last_access,
            files: persisted_files,
            chunk_files: chunk_records.clone(),
            por_tree: StoredPorTree::from(&por_tree),
        };

        if let Err(err) = write_manifest_metadata(&metadata_record, &metadata_path) {
            let mut state = self.state.write().expect("storage state poisoned");
            state.reserved_bytes = state.reserved_bytes.saturating_sub(required_bytes);
            drop(state);
            let _ = fs::remove_dir_all(&manifest_dir);
            return Err(err);
        }

        let mut state = self.state.write().expect("storage state poisoned");
        state.reserved_bytes = state.reserved_bytes.saturating_sub(required_bytes);

        let mut new_index = state.index.clone();
        new_index.total_bytes = state.total_bytes + required_bytes;
        let mut refcounts = state.chunk_refcounts.clone();
        for record in &chunk_records {
            let counter = refcounts.entry(record.digest).or_insert(0);
            *counter = counter.saturating_add(1);
        }
        new_index.chunk_refcounts = chunk_refcount_entries(&refcounts);
        new_index.entries.push(ManifestIndexEntry {
            manifest_id: manifest_id.clone(),
            manifest_cid: manifest.root_cid.clone(),
            manifest_digest,
            payload_digest,
            content_length: plan.content_length,
            chunk_profile_handle: canonical_profile_handle(manifest),
            chunk_count,
            stored_at_unix_secs,
            retention_epoch,
            retention_source: Some(retention_source.clone()),
            last_access,
        });

        let index_bytes = match norito::to_bytes(&new_index) {
            Ok(bytes) => bytes,
            Err(err) => {
                drop(state);
                let _ = fs::remove_dir_all(&manifest_dir);
                return Err(StorageError::Norito(err));
            }
        };

        if let Err(err) = write_atomic(&self.index_path, &index_bytes) {
            drop(state);
            let _ = fs::remove_dir_all(&manifest_dir);
            return Err(StorageError::from(err));
        }

        let stored_manifest = StoredManifest::from_record(metadata_record, manifest_path);

        state.index = new_index;
        state.total_bytes = state.total_bytes.saturating_add(required_bytes);
        state.manifests.insert(manifest_id.clone(), stored_manifest);
        state.chunk_refcounts = refcounts;

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

    fn manifest_for_access(&self, manifest_id: &str) -> Result<StoredManifest, StorageError> {
        let (record, metadata_path, index_bytes, manifest) = {
            let mut state = self.state.write().expect("storage state poisoned");
            let next_access = state.access_counter.saturating_add(1);
            state.access_counter = next_access;
            let (record, metadata_path, manifest, retention_source) = {
                let manifest = state.manifests.get_mut(manifest_id).ok_or_else(|| {
                    StorageError::ManifestNotFound {
                        manifest_id: manifest_id.to_owned(),
                    }
                })?;
                manifest.last_access = next_access;
                let retention_source = manifest.retention_source.clone();
                let record = manifest.to_record();
                let metadata_path = manifest
                    .manifest_path
                    .parent()
                    .expect("manifest path must have parent")
                    .join(METADATA_FILE_NAME);
                (record, metadata_path, manifest.clone(), retention_source)
            };

            if let Some(entry) = state
                .index
                .entries
                .iter_mut()
                .find(|entry| entry.manifest_id == manifest_id)
            {
                entry.last_access = next_access;
                entry.retention_source = retention_source;
            }
            let index_bytes = match norito::to_bytes(&state.index) {
                Ok(bytes) => Some(bytes),
                Err(err) => {
                    iroha_logger::warn!(
                        %err,
                        manifest_id = %manifest_id,
                        "failed to encode storage index while updating access"
                    );
                    None
                }
            };
            (record, metadata_path, index_bytes, manifest)
        };

        if let Err(err) = write_manifest_metadata(&record, &metadata_path) {
            iroha_logger::warn!(
                %err,
                manifest_id = %manifest_id,
                "failed to persist manifest access metadata"
            );
        }
        if let Some(bytes) = index_bytes {
            if let Err(err) = write_atomic(&self.index_path, &bytes) {
                iroha_logger::warn!(
                    %err,
                    manifest_id = %manifest_id,
                    "failed to persist storage index access metadata"
                );
            }
        }

        Ok(manifest)
    }

    /// Read an exact range from the stored payload.
    pub fn read_payload_range(
        &self,
        manifest_id: &str,
        offset: u64,
        len: usize,
    ) -> Result<Vec<u8>, StorageError> {
        if len == 0 {
            return Ok(Vec::new());
        }

        let manifest = self.manifest_for_access(manifest_id)?;

        if offset
            .checked_add(len as u64)
            .map(|end| end > manifest.content_length)
            .unwrap_or(true)
        {
            return Err(StorageError::RangeOutOfBounds {
                offset,
                len,
                content_length: manifest.content_length,
            });
        }

        let mut buffer = vec![0u8; len];
        read_into_manifest(&manifest, offset, &mut buffer)?;
        Ok(buffer)
    }

    /// Locate chunk metadata by digest for the provided manifest.
    pub fn chunk_by_digest(
        &self,
        manifest_id: &str,
        digest: &[u8; 32],
    ) -> Result<ChunkFileRecord, StorageError> {
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
        let manifest = self.manifest_for_access(manifest_id)?;
        let record = manifest
            .chunk_files
            .iter()
            .find(|record| record.digest == *digest)
            .cloned()
            .ok_or_else(|| StorageError::ChunkNotFound {
                manifest_id: manifest_id.to_owned(),
                digest_hex: digest.encode_hex::<String>(),
            })?;
        let bytes = fs::read(&record.path)?;
        if bytes.len() != record.length as usize {
            return Err(StorageError::PayloadLengthMismatch {
                expected: record.length as u64,
                actual: bytes.len() as u64,
            });
        }
        Ok(bytes)
    }

    /// Sample PoR leaves for the specified manifest.
    pub fn sample_por(
        &self,
        manifest_id: &str,
        count: usize,
        seed: u64,
    ) -> Result<Vec<(usize, PorProof)>, StorageError> {
        if count == 0 {
            return Ok(Vec::new());
        }

        let manifest = self.manifest_for_access(manifest_id)?;

        let por_tree = manifest.por_tree();
        let total = por_tree.leaf_count();
        if total == 0 {
            return Ok(Vec::new());
        }

        let target = count.min(total);
        let mut rng_state = seed;
        let mut seen = HashSet::new();
        let mut samples = Vec::with_capacity(target);

        while samples.len() < target {
            rng_state = splitmix64(rng_state);
            let leaf_index = (rng_state as usize) % total;
            if !seen.insert(leaf_index) {
                continue;
            }
            let Some((chunk_idx, segment_idx, leaf_idx)) = por_tree.leaf_path(leaf_index) else {
                continue;
            };
            let mut payload = ManifestPayload::new(&manifest);
            let proof = por_tree
                .prove_leaf_with(chunk_idx, segment_idx, leaf_idx, &mut payload)
                .map_err(StorageError::ChunkStore)?;
            if let Some(proof) = proof {
                samples.push((leaf_index, proof));
            }
        }

        Ok(samples)
    }

    fn ingest_payload<R: Read>(
        &self,
        plan: &CarBuildPlan,
        reader: &mut R,
        chunks_dir: &Path,
    ) -> Result<(Vec<StoredChunkRecord>, PorMerkleTree), StorageError> {
        let mut chunk_store = ChunkStore::with_profile(plan.chunk_profile);
        let output = chunk_store
            .ingest_plan_stream_to_directory(plan, reader, chunks_dir)
            .map_err(StorageError::ChunkStore)?;

        if output.total_bytes != plan.content_length {
            return Err(StorageError::PayloadLengthMismatch {
                expected: plan.content_length,
                actual: output.total_bytes,
            });
        }

        let records = output
            .records
            .into_iter()
            .map(|record| StoredChunkRecord {
                file_name: record.file_name,
                offset: record.offset,
                length: record.length,
                digest: record.digest,
                role: None,
            })
            .collect::<Vec<_>>();
        Ok((records, chunk_store.por_tree().clone()))
    }
}

impl StoredManifest {
    fn from_record(record: StoredManifestRecord, manifest_path: PathBuf) -> Self {
        let files = if record.files.is_empty() {
            vec![StoredFileRecord {
                path: Vec::new(),
                offset: 0,
                size: record.content_length,
                first_chunk: 0,
                chunk_count: record.chunk_files.len(),
            }]
        } else {
            record
                .files
                .iter()
                .map(|file| StoredFileRecord {
                    path: file.path.clone(),
                    offset: file.offset,
                    size: file.size,
                    first_chunk: file.first_chunk as usize,
                    chunk_count: file.chunk_count as usize,
                })
                .collect::<Vec<_>>()
        };
        let chunk_files = record
            .chunk_files
            .iter()
            .map(|chunk| ChunkFileRecord {
                path: manifest_path
                    .parent()
                    .expect("manifest path must have parent")
                    .join(CHUNKS_DIR_NAME)
                    .join(&chunk.file_name),
                offset: chunk.offset,
                length: chunk.length,
                digest: chunk.digest,
                role: chunk.role.as_ref().map(|role| role.role),
                group_id: chunk.role.as_ref().map(|role| role.group_id),
            })
            .collect::<Vec<_>>();

        Self {
            manifest_id: record.manifest_id,
            manifest_cid: record.manifest_cid,
            manifest_digest: record.manifest_digest,
            payload_digest: record.payload_digest,
            content_length: record.content_length,
            chunk_profile_handle: record.chunk_profile_handle,
            stripe_layout: record.stripe_layout,
            stored_at_unix_secs: record.stored_at_unix_secs,
            retention_epoch: record.retention_epoch,
            retention_source: record.retention_source,
            last_access: record.last_access,
            files,
            chunk_files,
            por_tree: record.por_tree,
            manifest_path,
        }
    }

    fn to_record(&self) -> StoredManifestRecord {
        let files = self
            .files
            .iter()
            .map(|file| StoredFileRecordNorito {
                path: file.path.clone(),
                offset: file.offset,
                size: file.size,
                first_chunk: file.first_chunk as u32,
                chunk_count: file.chunk_count as u32,
            })
            .collect();
        let chunk_files = self
            .chunk_files
            .iter()
            .map(|chunk| StoredChunkRecord {
                file_name: chunk
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_owned(),
                offset: chunk.offset,
                length: chunk.length,
                digest: chunk.digest,
                role: chunk.role.map(|role| StoredChunkRole {
                    role,
                    group_id: chunk.group_id.unwrap_or(0),
                }),
            })
            .collect();

        StoredManifestRecord {
            manifest_id: self.manifest_id.clone(),
            manifest_cid: self.manifest_cid.clone(),
            manifest_digest: self.manifest_digest,
            payload_digest: self.payload_digest,
            content_length: self.content_length,
            chunk_profile_handle: self.chunk_profile_handle.clone(),
            stripe_layout: self.stripe_layout,
            stored_at_unix_secs: self.stored_at_unix_secs,
            retention_epoch: self.retention_epoch,
            retention_source: self.retention_source.clone(),
            last_access: self.last_access,
            files,
            chunk_files,
            por_tree: self.por_tree.clone(),
        }
    }
}

fn stored_files_from_plan(plan: &CarBuildPlan) -> Vec<StoredFileRecord> {
    let mut offset = 0u64;
    plan.files
        .iter()
        .map(|file| {
            let record = StoredFileRecord {
                path: file.path.clone(),
                offset,
                size: file.size,
                first_chunk: file.first_chunk,
                chunk_count: file.chunk_count,
            };
            offset = offset.saturating_add(file.size);
            record
        })
        .collect()
}

fn canonical_profile_handle(manifest: &ManifestV1) -> String {
    format!(
        "{}.{}@{}",
        manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
    )
}

fn ensure_chunk_profile_match(
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
) -> Result<(), StorageError> {
    let profile = plan.chunk_profile;
    if profile.min_size as u32 != manifest.chunking.min_size
        || profile.target_size as u32 != manifest.chunking.target_size
        || profile.max_size as u32 != manifest.chunking.max_size
        || profile.break_mask as u32 != manifest.chunking.break_mask
    {
        return Err(StorageError::ChunkProfileMismatch);
    }
    Ok(())
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub(crate) fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent directory"))?;
    fs::create_dir_all(parent)?;
    let tmp = path.with_added_extension(ATOMIC_EXT);

    let mut file = File::create(&tmp)?;
    file.write_all(data)?;
    file.sync_all()?;
    drop(file);

    fs::rename(&tmp, path)?;
    Ok(())
}

fn write_manifest_metadata(
    record: &StoredManifestRecord,
    metadata_path: &Path,
) -> Result<(), StorageError> {
    let metadata_bytes = norito::to_bytes(record)?;
    write_atomic(metadata_path, &metadata_bytes)?;
    Ok(())
}

impl StorageBackend {
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
    let end = offset + remaining as u64;

    for chunk in manifest.chunk_files.iter() {
        let chunk_start = chunk.offset;
        let chunk_end = chunk_start + chunk.length as u64;

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

        let mut file = File::open(&chunk.path).map_err(ChunkStoreError::Io)?;
        let rel_offset = read_start - chunk_start;
        file.seek(SeekFrom::Start(rel_offset))
            .map_err(ChunkStoreError::Io)?;
        file.read_exact(&mut buf[cursor..cursor + bytes_to_read])
            .map_err(ChunkStoreError::Io)?;

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

fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use blake3;
    use sorafs_car::{CarPlanError, FileEntry};
    use sorafs_manifest::{DagCodecId, ManifestBuilder, PinPolicy};
    use tempfile::TempDir;

    use super::*;

    fn temp_config(temp_dir: &TempDir) -> StorageConfig {
        StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("storage"))
            .build()
    }

    fn single_file_plan(bytes: &[u8]) -> Result<CarBuildPlan, CarPlanError> {
        CarBuildPlan::single_file(bytes)
    }

    #[test]
    fn ingest_manifest_persists_metadata_and_chunks() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"Hello deterministic SoraFS!";
        let plan = single_file_plan(payload).expect("plan");

        let manifest = ManifestBuilder::new()
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

        let manifest = ManifestBuilder::new()
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

        let manifest = ManifestBuilder::new()
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

        let manifest = ManifestBuilder::new()
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

        let manifest = ManifestBuilder::new()
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
        let manifest = ManifestBuilder::new()
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
        let manifest = ManifestBuilder::new()
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
    fn chunk_slice_rejects_misaligned_range() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = vec![0xBB; 128];
        let plan = single_file_plan(&payload).expect("plan");

        let manifest = ManifestBuilder::new()
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

        let manifest = ManifestBuilder::new()
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

        let manifest = ManifestBuilder::new()
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
    fn last_access_persists_after_reads() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"last access persistence";
        let plan = single_file_plan(payload).expect("plan");
        let manifest = ManifestBuilder::new()
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
        let manifest = ManifestBuilder::new()
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

        let reloaded = StorageBackend::new(temp_config(&temp_dir)).expect("reload");
        assert_eq!(reloaded.manifest_count(), 0);
    }

    #[test]
    fn to_car_plan_matches_stored_chunks() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = vec![0xCC; 96];
        let plan = single_file_plan(&payload).expect("plan");

        let manifest = ManifestBuilder::new()
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
    }

    #[test]
    fn sample_por_returns_proofs() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"SoraFS deterministic sampling data for PoR";
        let plan = single_file_plan(payload).expect("plan");

        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xCD; 16])
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

        let samples = backend
            .sample_por(&manifest_id, 4, 42)
            .expect("PoR samples");
        let stored = backend.manifest(&manifest_id).expect("stored manifest");
        let expected = stored.por_tree().leaf_count().min(4);
        assert_eq!(samples.len(), expected);
        let root = *stored.por_tree().root();

        for (_idx, proof) in samples {
            assert!(proof.verify(&root));
        }
    }

    #[test]
    fn chunk_by_digest_returns_record() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let backend = StorageBackend::new(temp_config(&temp_dir)).expect("backend init");

        let payload = b"deterministic chunk access";
        let plan = single_file_plan(payload).expect("plan");

        let manifest = ManifestBuilder::new()
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

        let manifest = ManifestBuilder::new()
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

        let manifest = ManifestBuilder::new()
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
    fn restart_rehydrates_manifest_index() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let cfg = temp_config(&temp_dir);
        let backend = StorageBackend::new(cfg.clone()).expect("backend init");

        let payload = b"Persistent storage test payload";
        let plan = single_file_plan(payload).expect("plan");
        let manifest = ManifestBuilder::new()
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
    fn write_atomic_uses_added_extension_and_cleans_up_temp_file() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let target_with_ext = temp_dir.path().join("bundle.car");

        write_atomic(&target_with_ext, b"hello").expect("write with extension");
        assert_eq!(fs::read(&target_with_ext).expect("read bundle"), b"hello");
        assert!(
            !target_with_ext.with_added_extension(ATOMIC_EXT).exists(),
            "temporary file should not remain on disk"
        );

        let target_no_ext = temp_dir.path().join("manifest");
        write_atomic(&target_no_ext, b"x").expect("write without extension");
        assert_eq!(fs::read(&target_no_ext).expect("read manifest"), b"x");
        assert!(
            !target_no_ext.with_added_extension(ATOMIC_EXT).exists(),
            "temporary file should be removed even when original path has no extension"
        );
    }
}
