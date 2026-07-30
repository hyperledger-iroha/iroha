//! Planning and encoding utilities for assembling CARv2 archives for SoraFS.
//!
//! The crate exposes deterministic planning structures that describe which
//! chunks must be included (and in which order) alongside a reference
//! implementation of a spec-compliant CARv2 writer. Downstream tooling can use
//! `CarBuildPlan` to reason about chunk boundaries and pass the plan to
//! `CarWriter` when it is time to emit a CARv2 archive (pragma + header +
//! CARv1 payload + MultihashIndexSorted index), or to `CarStreamingWriter`
//! when the source payload cannot be buffered in memory.

#![allow(unexpected_cfgs)]

#[cfg(feature = "manifest")]
use std::str::FromStr;
use std::{
    collections::{BTreeMap, HashSet},
    convert::TryFrom,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::{
    DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
};

use blake3::{Hash, Hasher};
#[cfg(feature = "manifest")]
use iroha_data_model::{
    da::{
        manifest::DaManifestV1,
        types::{BlobClass, ExtraMetadata, MetadataEncryption, MetadataVisibility},
    },
    name::Name,
};
#[cfg(feature = "manifest")]
use norito::json::{self, Value};
use norito::{NoritoDeserialize, NoritoSerialize};
pub use sorafs_chunker;
use sorafs_chunker::{ChunkDigest, ChunkProfile};
#[cfg(feature = "manifest")]
use sorafs_manifest::{
    ManifestV1 as SorafsManifestV1, PdpMerkleReadError, PdpMerkleTreeBuilderV1, PdpMerkleTreeError,
    PdpMerkleTreeV1, PdpProofLeafV1, PdpSampleV1, estimated_heap_bytes as estimated_pdp_heap_bytes,
};
use thiserror::Error;

pub mod bundle_archive;
pub mod chunker_registry;
mod chunker_registry_data;
pub mod fetch_plan;
pub mod fixtures;
#[cfg(feature = "manifest")]
pub mod gateway;
pub mod local_fetch;
pub mod multi_fetch;
pub mod policy;
#[cfg(feature = "manifest")]
#[path = "proof_stream.rs"]
pub mod proof_stream;
#[cfg(feature = "manifest")]
#[path = "proof_stream_transport.rs"]
pub mod proof_stream_transport;
#[cfg(feature = "manifest")]
pub mod reference;
pub mod scoreboard;
#[cfg(feature = "manifest")]
pub mod streaming_verifier;
#[cfg(feature = "cli")]
pub mod taikai;
#[cfg(feature = "manifest")]
pub mod trustless;
#[cfg(feature = "manifest")]
pub mod verifier;

#[cfg(feature = "manifest")]
pub use reference::{validate_manifest_car_replay, validate_manifest_car_replay_bytes};
#[cfg(feature = "manifest")]
pub use trustless::{
    TrustlessVerificationError, TrustlessVerificationOutcome, TrustlessVerifier,
    TrustlessVerifierConfig,
};
#[cfg(feature = "manifest")]
pub use verifier::{CarVerificationReport, CarVerifier, CarVerifyError};

/// Compute the BLAKE3 digest of the provided payload.
#[must_use]
pub fn compute_chunk_digest(payload: &[u8]) -> [u8; 32] {
    blake3::hash(payload).into()
}

/// Compute the SHA3-256 commitment of a deterministic CAR chunk plan.
///
/// The canonical transcript is the ordered concatenation of each chunk's
/// little-endian 64-bit offset, little-endian 64-bit length, and 32-byte
/// BLAKE3 content digest.
#[must_use]
pub fn compute_chunk_plan_digest_sha3(chunks: &[CarChunk]) -> [u8; 32] {
    sorafs_chunker::compute_chunk_plan_digest_sha3(
        chunks
            .iter()
            .map(|chunk| (chunk.offset, u64::from(chunk.length), chunk.digest)),
    )
}

/// Compute the canonical PoR commitment for `payload` under an existing CAR plan.
///
/// The plan is revalidated against the payload before the root is returned, so manifest
/// producers cannot commit to geometry that differs from the bytes they package.
pub fn compute_por_root(payload: &[u8], plan: &CarBuildPlan) -> Result<[u8; 32], ChunkStoreError> {
    let mut store = ChunkStore::with_profile(plan.chunk_profile);
    store.ingest_plan(payload, plan)?;
    Ok(*store.por_tree().root())
}

/// Identifier assigned to registered chunking profiles.
#[allow(unexpected_cfgs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ProfileId(pub u32);

#[cfg(feature = "manifest")]
impl From<sorafs_manifest::ProfileId> for ProfileId {
    fn from(id: sorafs_manifest::ProfileId) -> Self {
        Self(id.0)
    }
}

pub mod por_json;

/// Errors that can occur while building a CAR plan.
#[derive(Debug, Error)]
pub enum CarPlanError {
    #[error("input payload is empty")]
    EmptyInput,
    #[error("file path is invalid: {0}")]
    InvalidPath(String),
    #[error("duplicate file path: {0}")]
    DuplicatePath(String),
    #[error("non-UTF-8 path: {0}")]
    NonUtf8Path(String),
    #[error("file path '{new}' conflicts with ancestor '{existing}'")]
    PathConflict { existing: String, new: String },
    #[error("chunking failed: {0}")]
    Chunking(#[from] sorafs_chunker::ChunkerError),
    #[error("chunking profile max_size {max_size} exceeds CAR chunk length limit {limit}")]
    ChunkProfileMaxSizeTooLarge { max_size: usize, limit: u32 },
    #[error("chunk length {length} exceeds CAR chunk length limit {limit}")]
    ChunkLengthTooLarge { length: usize, limit: u32 },
    #[error("chunk offset {offset} exceeds u64::MAX")]
    ChunkOffsetTooLarge { offset: usize },
    #[error("content length exceeds u64::MAX")]
    ContentLengthTooLarge,
    #[error("secure directory ingestion is unsupported on this platform")]
    SecureDirectoryUnsupported,
    #[error("directory operation failed at {path}: {source}")]
    DirectoryIo {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("directory inventory has {count} entries; maximum is {maximum}")]
    TooManyDirectoryEntries { count: usize, maximum: usize },
    #[error("directory inventory has {count} files; maximum is {maximum}")]
    TooManyFiles { count: usize, maximum: usize },
    #[error("CAR plan would have {count} chunks; maximum is {maximum}")]
    TooManyChunks { count: usize, maximum: usize },
    #[error("eager directory payload has {bytes} bytes; maximum is {maximum}")]
    DirectoryPayloadTooLarge { bytes: u64, maximum: u64 },
    #[error("failed to reserve {requested} entries/bytes for {context}")]
    AllocationFailed {
        context: &'static str,
        requested: usize,
    },
    #[error(
        "estimated heap for {context} is {estimated} bytes; production maximum is {limit} bytes"
    )]
    EstimatedHeapLimitExceeded {
        context: &'static str,
        estimated: usize,
        limit: usize,
    },
    #[error("generated CAR plan is invalid: {0}")]
    InvalidPlan(#[from] CarPlanValidationError),
}

/// Errors surfaced by the future CAR writer.
#[derive(Debug, Error)]
pub enum CarWriteError {
    #[error("writer failed: {0}")]
    Io(#[from] io::Error),
    #[error("payload length does not match plan content length")]
    PayloadMismatch,
    #[error("payload digest does not match plan payload digest")]
    PayloadDigestMismatch,
    #[error("chunk {chunk_index} extends beyond payload length")]
    ChunkOutOfBounds { chunk_index: usize },
    #[error("chunk {chunk_index} digest does not match payload bytes")]
    DigestMismatch { chunk_index: usize },
    #[error("root length exceeds supported bounds")]
    RootTooLarge,
    #[error("expected roots do not match computed roots")]
    RootMismatch,
    #[error("CAR DAG invariant failed while computing {context}")]
    DagInvariant { context: &'static str },
    #[error("logical file paths conflict while building the directory DAG")]
    DirectoryPathConflict,
    #[error("CAR layout arithmetic overflow while computing {context}")]
    ArithmeticOverflow { context: &'static str },
    #[error("failed to reserve {requested} entries/bytes for {context}")]
    AllocationFailed {
        context: &'static str,
        requested: usize,
    },
    #[error("CAR plan is invalid: {0}")]
    InvalidPlan(#[from] CarPlanValidationError),
}

/// Errors that can occur while ingesting chunk metadata from a stream.
#[derive(Debug, Error)]
pub enum ChunkStoreError {
    #[error("reader failed: {0}")]
    Io(#[from] io::Error),
    #[error("chunk {chunk_index} ended before reading {expected} bytes")]
    UnexpectedEof { chunk_index: usize, expected: u32 },
    #[error("chunk {chunk_index} digest does not match payload bytes")]
    DigestMismatch { chunk_index: usize },
    #[error("payload length mismatch: expected {expected} bytes, read {actual} bytes")]
    LengthMismatch { expected: u64, actual: u64 },
    #[error("payload offset {offset} out of range (len {len})")]
    OffsetOutOfRange { offset: u64, len: u64 },
    #[error("chunking failed: {0}")]
    Chunking(#[from] sorafs_chunker::ChunkerError),
    #[error("chunking profile max_size {max_size} exceeds chunk length limit {limit}")]
    ChunkProfileMaxSizeTooLarge { max_size: usize, limit: u32 },
    #[error("chunk length {length} exceeds chunk length limit {limit}")]
    ChunkLengthTooLarge { length: usize, limit: u32 },
    #[error("chunk offset {offset} exceeds u64::MAX")]
    ChunkOffsetTooLarge { offset: usize },
    #[error("payload length exceeds u64::MAX")]
    PayloadLengthTooLarge,
    #[error("PoR tree invariant failed while validating {context}")]
    PorInvariant { context: &'static str },
    #[error("PoR tree count overflow while computing {context}")]
    PorCountOverflow { context: &'static str },
    #[error(
        "PoR proof payload does not match leaf {leaf_index} in segment {segment_index}, chunk {chunk_index}"
    )]
    PorProofLeafDigestMismatch {
        chunk_index: usize,
        segment_index: usize,
        leaf_index: usize,
    },
    #[error("payload digest does not match plan payload digest")]
    PayloadDigestMismatch,
    #[error("failed to reserve {requested} entries/bytes for {context}")]
    AllocationFailed {
        context: &'static str,
        requested: usize,
    },
    #[error("CAR plan is invalid: {0}")]
    InvalidPlan(#[from] CarPlanValidationError),
    #[error(
        "CAR ingest requires an estimated {estimated} bytes of heap; configured limit is {limit}"
    )]
    EstimatedHeapLimitExceeded { estimated: usize, limit: usize },
    #[error("CAR ingest heap limit must be greater than zero")]
    InvalidEstimatedHeapLimit,
    #[error("chunk sink expected chunk index {expected}, got {actual}")]
    SinkChunkOrder { expected: usize, actual: usize },
    #[error("chunk sink metadata for chunk {chunk_index} does not match the validated plan")]
    SinkChunkMetadataMismatch { chunk_index: usize },
    #[error("chunk sink payload length for chunk {chunk_index} must be {expected}, got {actual}")]
    SinkChunkLengthMismatch {
        chunk_index: usize,
        expected: usize,
        actual: usize,
    },
    #[error("chunk sink payload digest for chunk {chunk_index} does not match the validated plan")]
    SinkChunkDigestMismatch { chunk_index: usize },
    #[error(
        "chunk sink is incomplete: wrote {actual_chunks}/{expected_chunks} chunks and {actual_bytes}/{expected_bytes} bytes"
    )]
    SinkIncomplete {
        expected_chunks: usize,
        actual_chunks: usize,
        expected_bytes: u64,
        actual_bytes: u64,
    },
    #[cfg(feature = "manifest")]
    #[error("canonical PDP tree construction failed: {0}")]
    PdpTree(#[from] PdpMerkleTreeError),
}

/// Abstraction over payload sources that support random-access reads.
pub trait PayloadSource {
    /// Reads exactly `buf.len()` bytes starting at `offset` into `buf`.
    fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), ChunkStoreError>;

    /// Verify that the source contains exactly the planned payload length when knowable.
    ///
    /// Random-access sources with no cheap length operation may retain the default no-op.
    fn ensure_exhausted(&mut self, _expected_len: u64) -> Result<(), ChunkStoreError> {
        Ok(())
    }
}

/// Streaming payload backed by a sequential reader.
struct ReaderPayload<'a, R> {
    reader: &'a mut R,
    consumed: u64,
}

impl<'a, R> ReaderPayload<'a, R> {
    fn new(reader: &'a mut R) -> Self {
        Self {
            reader,
            consumed: 0,
        }
    }
}

impl<R: Read> PayloadSource for ReaderPayload<'_, R> {
    fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), ChunkStoreError> {
        if offset != self.consumed {
            return Err(ChunkStoreError::OffsetOutOfRange {
                offset,
                len: self.consumed,
            });
        }
        self.reader.read_exact(buf).map_err(ChunkStoreError::Io)?;
        let read_len =
            u64::try_from(buf.len()).map_err(|_| ChunkStoreError::PayloadLengthTooLarge)?;
        self.consumed = self
            .consumed
            .checked_add(read_len)
            .ok_or(ChunkStoreError::PayloadLengthTooLarge)?;
        Ok(())
    }

    fn ensure_exhausted(&mut self, expected_len: u64) -> Result<(), ChunkStoreError> {
        let mut trailing = [0u8; 1];
        match self
            .reader
            .read(&mut trailing)
            .map_err(ChunkStoreError::Io)?
        {
            0 => Ok(()),
            count => {
                let count =
                    u64::try_from(count).map_err(|_| ChunkStoreError::PayloadLengthTooLarge)?;
                Err(ChunkStoreError::LengthMismatch {
                    expected: expected_len,
                    actual: self
                        .consumed
                        .checked_add(count)
                        .ok_or(ChunkStoreError::PayloadLengthTooLarge)?,
                })
            }
        }
    }
}

/// Payload source backed by an in-memory byte slice.
pub struct InMemoryPayload<'a> {
    data: &'a [u8],
}

impl<'a> InMemoryPayload<'a> {
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl PayloadSource for InMemoryPayload<'_> {
    fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), ChunkStoreError> {
        let data_len =
            u64::try_from(self.data.len()).map_err(|_| ChunkStoreError::PayloadLengthTooLarge)?;
        let start = usize::try_from(offset).map_err(|_| ChunkStoreError::OffsetOutOfRange {
            offset,
            len: data_len,
        })?;
        let end = start
            .checked_add(buf.len())
            .ok_or(ChunkStoreError::OffsetOutOfRange {
                offset,
                len: data_len,
            })?;
        if end > self.data.len() {
            return Err(ChunkStoreError::OffsetOutOfRange {
                offset,
                len: data_len,
            });
        }
        buf.copy_from_slice(&self.data[start..end]);
        Ok(())
    }

    fn ensure_exhausted(&mut self, expected_len: u64) -> Result<(), ChunkStoreError> {
        let actual =
            u64::try_from(self.data.len()).map_err(|_| ChunkStoreError::PayloadLengthTooLarge)?;
        if actual != expected_len {
            return Err(ChunkStoreError::LengthMismatch {
                expected: expected_len,
                actual,
            });
        }
        Ok(())
    }
}

/// Payload source backed by a stable no-follow regular file on Unix.
///
/// Other platforms fail closed with [`io::ErrorKind::Unsupported`] until equivalent file-identity
/// and no-follow handle support is implemented.
pub struct FilePayload {
    file: File,
    path: PathBuf,
    metadata: fs::Metadata,
}

impl FilePayload {
    /// Open a stable no-follow regular-file payload source.
    pub fn open(path: &Path) -> Result<Self, io::Error> {
        #[cfg(not(unix))]
        {
            let _ = path;
            return Err(unsupported_secure_filesystem_error());
        }
        #[cfg(unix)]
        {
            let linked = fs::symlink_metadata(path)?;
            if linked.file_type().is_symlink() || !linked.is_file() || linked.nlink() != 1 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "file payload must be a no-follow regular file with one hard link",
                ));
            }
            let mut options = OpenOptions::new();
            options.read(true);
            set_atomic_no_follow(&mut options);
            let file = options.open(path)?;
            validate_payload_file_handle(path, &file, linked.len(), Some(&linked))?;
            let metadata = file.metadata()?;
            Ok(Self {
                file,
                path: path.to_path_buf(),
                metadata,
            })
        }
    }

    fn validate_unchanged(&self) -> Result<(), ChunkStoreError> {
        validate_payload_file_handle(
            &self.path,
            &self.file,
            self.metadata.len(),
            Some(&self.metadata),
        )
        .map_err(ChunkStoreError::Io)
    }
}

impl PayloadSource for FilePayload {
    fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), ChunkStoreError> {
        self.validate_unchanged()?;
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(ChunkStoreError::Io)?;
        self.file.read_exact(buf).map_err(ChunkStoreError::Io)?;
        Ok(())
    }

    fn ensure_exhausted(&mut self, expected_len: u64) -> Result<(), ChunkStoreError> {
        self.validate_unchanged()?;
        let actual = self.file.metadata().map_err(ChunkStoreError::Io)?.len();
        if actual != expected_len {
            return Err(ChunkStoreError::LengthMismatch {
                expected: expected_len,
                actual,
            });
        }
        Ok(())
    }
}

struct FileSpan {
    start: u64,
    end: u64,
    path: PathBuf,
    metadata: fs::Metadata,
}

/// Payload source backed by multiple files described by a [`CarBuildPlan`].
///
/// Secure file-backed operation is currently Unix-only; other platforms fail closed.
pub struct DirectoryPayload {
    canonical_root: PathBuf,
    root_metadata: fs::Metadata,
    spans: Vec<FileSpan>,
    total_len: u64,
    cached_index: Option<usize>,
    cached_file: Option<File>,
}

impl DirectoryPayload {
    /// Open a root-confined payload inventory and capture every file's identity and exact size.
    ///
    /// Logical paths must be strictly ordered portable components. Symlinks, hard links,
    /// non-regular files, root escapes, and actual sizes different from [`FilePlan::size`] are
    /// rejected before the source can be read.
    pub fn new(root: &Path, files: &[FilePlan]) -> Result<Self, io::Error> {
        if !cfg!(unix) {
            let _ = (root, files);
            return Err(unsupported_secure_filesystem_error());
        }
        let root_metadata = fs::symlink_metadata(root)?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "directory payload root must be a real directory",
            ));
        }
        let canonical_root = fs::canonicalize(root)?;
        let canonical_metadata = fs::symlink_metadata(&canonical_root)?;
        if !metadata_identifies_same_file(&root_metadata, &canonical_metadata) {
            return Err(io::Error::other(
                "directory payload root changed during canonicalization",
            ));
        }
        if files.len() > CAR_PLAN_MAX_FILES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "directory payload has {} files; maximum is {CAR_PLAN_MAX_FILES}",
                    files.len()
                ),
            ));
        }

        let mut preliminary = Vec::new();
        preliminary.try_reserve_exact(files.len()).map_err(|_| {
            io::Error::other(format!(
                "failed to reserve directory payload spans for {} files",
                files.len()
            ))
        })?;
        let mut offset = 0u64;
        let mut previous_path: Option<&[String]> = None;
        for file in files {
            if file.path.is_empty()
                || file.path.len() > CAR_LOGICAL_PATH_MAX_COMPONENTS
                || file.path.iter().any(|component| {
                    !is_portable_normal_component(component)
                        || component.len() > CAR_LOGICAL_PATH_COMPONENT_MAX_BYTES
                })
                || file
                    .path
                    .iter()
                    .try_fold(0usize, |total, component| {
                        total.checked_add(component.len().saturating_add(1))
                    })
                    .is_none_or(|bytes| bytes.saturating_sub(1) > CAR_LOGICAL_PATH_MAX_BYTES)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "directory payload file path must contain only portable normal components",
                ));
            }
            if let Some(previous) = previous_path
                && (previous >= file.path.as_slice() || file.path.starts_with(previous))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "directory payload file paths must be strictly ordered and non-conflicting",
                ));
            }
            previous_path = Some(&file.path);

            let mut path = canonical_root.clone();
            for component in &file.path {
                path.push(component);
            }
            let end = offset.checked_add(file.size).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "directory payload file sizes overflow u64",
                )
            })?;
            preliminary.push((offset, end, path));
            offset = end;
        }

        let mut spans = Vec::new();
        spans.try_reserve_exact(preliminary.len()).map_err(|_| {
            io::Error::other(format!(
                "failed to reserve directory payload metadata for {} files",
                preliminary.len()
            ))
        })?;
        for ((start, end, path), file_plan) in preliminary.into_iter().zip(files) {
            let file = open_confined_payload_file(&canonical_root, &path, file_plan.size, None)?;
            let metadata = file.metadata()?;
            spans.push(FileSpan {
                start,
                end,
                path,
                metadata,
            });
        }
        Ok(Self {
            canonical_root,
            root_metadata: canonical_metadata,
            total_len: offset,
            spans,
            cached_index: None,
            cached_file: None,
        })
    }

    fn validate_root(&self) -> Result<(), ChunkStoreError> {
        let current = fs::symlink_metadata(&self.canonical_root).map_err(ChunkStoreError::Io)?;
        if !current.is_dir()
            || current.file_type().is_symlink()
            || !metadata_snapshot_matches(&self.root_metadata, &current)
        {
            return Err(ChunkStoreError::Io(io::Error::other(
                "directory payload root changed after validation",
            )));
        }
        Ok(())
    }

    fn open_file(&mut self, span_index: usize) -> Result<&mut File, ChunkStoreError> {
        self.validate_root()?;
        if self.cached_index != Some(span_index) {
            let span = &self.spans[span_index];
            let file = open_confined_payload_file(
                &self.canonical_root,
                &span.path,
                span.end - span.start,
                Some(&span.metadata),
            )
            .map_err(ChunkStoreError::Io)?;
            self.cached_file = Some(file);
            self.cached_index = Some(span_index);
        }
        let file = self.cached_file.as_mut().ok_or_else(|| {
            ChunkStoreError::Io(io::Error::other("failed to cache directory file handle"))
        })?;
        let span = &self.spans[span_index];
        validate_payload_file_handle(
            &span.path,
            file,
            span.end - span.start,
            Some(&span.metadata),
        )
        .map_err(ChunkStoreError::Io)?;
        Ok(file)
    }
}

impl PayloadSource for DirectoryPayload {
    fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), ChunkStoreError> {
        if buf.is_empty() {
            return Ok(());
        }
        if offset > self.total_len {
            return Err(ChunkStoreError::OffsetOutOfRange {
                offset,
                len: self.total_len,
            });
        }
        let requested =
            u64::try_from(buf.len()).map_err(|_| ChunkStoreError::OffsetOutOfRange {
                offset,
                len: self.total_len,
            })?;
        let requested_end =
            offset
                .checked_add(requested)
                .ok_or(ChunkStoreError::OffsetOutOfRange {
                    offset,
                    len: self.total_len,
                })?;
        if requested_end > self.total_len {
            return Err(ChunkStoreError::OffsetOutOfRange {
                offset,
                len: self.total_len,
            });
        }

        let mut remaining = buf.len();
        let mut current_offset = offset;
        let mut buf_cursor = 0usize;

        while remaining > 0 {
            let span_index = self
                .spans
                .iter()
                .position(|span| current_offset < span.end)
                .ok_or(ChunkStoreError::OffsetOutOfRange {
                    offset: current_offset,
                    len: self.total_len,
                })?;
            let span = &self.spans[span_index];

            let span_offset = current_offset - span.start;
            let span_remaining = usize::try_from(span.end - current_offset).map_err(|_| {
                ChunkStoreError::OffsetOutOfRange {
                    offset: current_offset,
                    len: self.total_len,
                }
            })?;
            let to_read = span_remaining.min(remaining);

            let file = self.open_file(span_index)?;
            file.seek(SeekFrom::Start(span_offset))
                .map_err(ChunkStoreError::Io)?;
            file.read_exact(&mut buf[buf_cursor..buf_cursor + to_read])
                .map_err(ChunkStoreError::Io)?;

            remaining -= to_read;
            buf_cursor += to_read;
            current_offset = current_offset
                .checked_add(u64::try_from(to_read).map_err(|_| {
                    ChunkStoreError::OffsetOutOfRange {
                        offset: current_offset,
                        len: self.total_len,
                    }
                })?)
                .ok_or(ChunkStoreError::OffsetOutOfRange {
                    offset: current_offset,
                    len: self.total_len,
                })?;
        }

        Ok(())
    }

    fn ensure_exhausted(&mut self, expected_len: u64) -> Result<(), ChunkStoreError> {
        if self.total_len != expected_len {
            return Err(ChunkStoreError::LengthMismatch {
                expected: expected_len,
                actual: self.total_len,
            });
        }
        self.validate_root()?;
        for span in &self.spans {
            open_confined_payload_file(
                &self.canonical_root,
                &span.path,
                span.end - span.start,
                Some(&span.metadata),
            )
            .map_err(ChunkStoreError::Io)?;
        }
        Ok(())
    }
}

fn open_confined_payload_file(
    canonical_root: &Path,
    path: &Path,
    expected_len: u64,
    captured: Option<&fs::Metadata>,
) -> io::Result<File> {
    validate_no_symlinks_below_root(canonical_root, path)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("directory payload file has no parent"))?;
    let canonical_parent = fs::canonicalize(parent)?;
    if !canonical_parent.starts_with(canonical_root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "directory payload file escapes its validated root",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    set_atomic_no_follow(&mut options);
    let file = options.open(path)?;
    validate_payload_file_handle(path, &file, expected_len, captured)?;
    let canonical_path = fs::canonicalize(path)?;
    if !canonical_path.starts_with(canonical_root) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "directory payload file resolves outside its validated root",
        ));
    }
    Ok(file)
}

#[cfg(unix)]
fn validate_no_symlinks_below_root(canonical_root: &Path, path: &Path) -> io::Result<()> {
    let relative = path.strip_prefix(canonical_root).map_err(|_| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "directory payload path is outside its validated root",
        )
    })?;
    let mut components = relative.components().peekable();
    let mut current = canonical_root.to_path_buf();
    while let Some(component) = components.next() {
        let Component::Normal(value) = component else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "directory payload path contains a non-normal component",
            ));
        };
        current.push(value);
        let metadata = fs::symlink_metadata(&current)?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "directory payload path contains a symbolic link",
            ));
        }
        if components.peek().is_some() && !metadata.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "directory payload path ancestor is not a directory",
            ));
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_no_symlinks_below_root(_canonical_root: &Path, _path: &Path) -> io::Result<()> {
    Err(unsupported_secure_filesystem_error())
}

fn validate_payload_file_handle(
    path: &Path,
    file: &File,
    expected_len: u64,
    captured: Option<&fs::Metadata>,
) -> io::Result<()> {
    let opened = file.metadata()?;
    let linked = fs::symlink_metadata(path)?;
    if !opened.is_file() || linked.file_type().is_symlink() || !linked.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "directory payload entry must be a no-follow regular file",
        ));
    }
    if !metadata_identifies_same_file(&opened, &linked) {
        return Err(io::Error::other(
            "directory payload entry changed while being opened",
        ));
    }
    #[cfg(unix)]
    if opened.nlink() != 1 || linked.nlink() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "directory payload entry must have exactly one hard link",
        ));
    }
    if opened.len() != expected_len || linked.len() != expected_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "directory payload file length mismatch: expected {expected_len}, found {}",
                opened.len()
            ),
        ));
    }
    if let Some(captured) = captured
        && (!metadata_snapshot_matches(captured, &opened)
            || !metadata_snapshot_matches(captured, &linked))
    {
        return Err(io::Error::other(
            "directory payload entry changed after validation",
        ));
    }
    Ok(())
}

fn unsupported_secure_filesystem_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "secure SoraFS file-backed payload and directory publication require Unix file identities and no-follow opens",
    )
}

/// Planning structure describing the chunks required to build a CAR payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarBuildPlan {
    /// Chunker profile used when deriving chunk boundaries.
    pub chunk_profile: ChunkProfile,
    /// Digest of the original payload (BLAKE3-256).
    pub payload_digest: Hash,
    /// Total number of bytes represented by the plan.
    pub content_length: u64,
    /// Chunk metadata that must be written into the CAR.
    pub chunks: Vec<CarChunk>,
    /// File descriptors describing which chunks belong to each file.
    pub files: Vec<FilePlan>,
}

/// File entry used by [`CarBuildPlan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePlan {
    pub path: Vec<String>,
    pub first_chunk: usize,
    pub chunk_count: usize,
    pub size: u64,
}

/// Hint describing the Taikai segment a chunk belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TaikaiSegmentHint {
    /// Taikai event identifier (Name literal encoded as a UTF-8 string).
    pub event: String,
    /// Stream identifier within the event.
    pub stream: String,
    /// Rendition identifier for the ladder rung.
    pub rendition: String,
    /// Segment sequence number within the rendition.
    pub sequence: u64,
    /// Total payload length in bytes, when provided by the ingest metadata.
    pub payload_len: Option<u64>,
    /// BLAKE3 digest of the payload, when provided by the ingest metadata.
    pub payload_digest: Option<[u8; 32]>,
}

/// Chunk entry used by [`CarBuildPlan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarChunk {
    pub offset: u64,
    pub length: u32,
    pub digest: [u8; 32],
    /// Optional Taikai segment hint carried alongside the chunk metadata.
    pub taikai_segment_hint: Option<TaikaiSegmentHint>,
}

const CAR_CHUNK_LENGTH_LIMIT: u32 = u32::MAX;
// Keep a single untrusted plan entry from requesting a multi-gigabyte allocation. Canonical
// SoraFS profiles are at most 512 KiB and DA ingest is specified at no more than 2 MiB, so this
// ceiling retains headroom without making allocation size attacker-controlled.
/// Maximum byte length of one chunk accepted by the canonical CAR/PoR pipeline.
pub const CHUNK_STORE_MAX_CHUNK_BYTES: u32 = 4 * 1024 * 1024;
/// Maximum number of chunks accepted by one canonical CAR plan.
pub const CAR_PLAN_MAX_CHUNKS: usize = 4_194_304;
/// Maximum authentication-path depth for the canonical chunk Merkle tree.
pub const POR_CHUNK_MERKLE_MAX_DEPTH: usize = 22;
const CAR_PLAN_MAX_FILES: usize = 1_000_000;
#[cfg(unix)]
const CAR_DIRECTORY_MAX_ENTRIES: usize = 1_000_000;
// Directory scanning and canonical DAG materialisation recurse once per logical component. Keep
// the protocol depth comfortably below platform thread-stack limits before descending into an
// attacker-controlled tree.
const CAR_LOGICAL_PATH_MAX_COMPONENTS: usize = 64;
const CAR_LOGICAL_PATH_COMPONENT_MAX_BYTES: usize = 255;
const CAR_LOGICAL_PATH_MAX_BYTES: usize = 4 * 1024;
/// Default upper bound for heap retained or temporarily allocated while ingesting one CAR plan.
pub const DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES: usize = 512 * 1024 * 1024;
#[cfg(unix)]
const CAR_EAGER_DIRECTORY_MAX_BYTES: u64 = DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES as u64;

/// Allocation geometry returned after a complete, allocation-free CAR plan validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarPlanValidation {
    max_chunk_len: usize,
    estimated_ingest_heap_bytes: usize,
}

impl CarPlanValidation {
    /// Largest chunk buffer required by the plan.
    #[must_use]
    pub fn max_chunk_len(self) -> usize {
        self.max_chunk_len
    }

    /// Conservative checked estimate of peak heap used by chunk-store ingestion.
    #[must_use]
    pub fn estimated_ingest_heap_bytes(self) -> usize {
        self.estimated_ingest_heap_bytes
    }
}

/// Structural, canonicality, and checked-geometry failures for [`CarBuildPlan`].
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CarPlanValidationError {
    /// Chunk profile parameters are internally inconsistent.
    #[error("invalid chunking profile: {0}")]
    InvalidProfile(#[from] sorafs_chunker::ChunkerError),
    /// The profile permits chunks larger than the production ingest ceiling.
    #[error("chunking profile max_size {max_size} exceeds production limit {limit}")]
    ChunkProfileTooLarge { max_size: usize, limit: u32 },
    /// A non-empty payload omitted its chunk inventory.
    #[error("non-empty payload must contain at least one chunk")]
    MissingChunks,
    /// Chunk inventory exceeded the production protocol bound.
    #[error("CAR plan has {count} chunks; maximum is {maximum}")]
    TooManyChunks { count: usize, maximum: usize },
    /// An empty payload advertised chunk entries.
    #[error("empty payload must not contain chunks; found {count}")]
    EmptyPayloadHasChunks { count: usize },
    /// An empty payload used a non-canonical digest.
    #[error("empty payload digest must equal BLAKE3(empty)")]
    InvalidEmptyPayloadDigest,
    /// A chunk had no bytes; empty logical files are represented by zero chunks.
    #[error("chunk {chunk_index} has zero length")]
    ZeroLengthChunk { chunk_index: usize },
    /// A chunk exceeded the validated profile/production ceiling.
    #[error("chunk {chunk_index} length {length} exceeds limit {limit}")]
    ChunkTooLarge {
        chunk_index: usize,
        length: u32,
        limit: u32,
    },
    /// A non-final chunk within one logical file was below the profile minimum.
    #[error("chunk {chunk_index} length {length} is below profile minimum {minimum}")]
    ChunkBelowProfileMinimum {
        chunk_index: usize,
        length: u32,
        minimum: usize,
    },
    /// Chunk byte range overflowed `u64`.
    #[error("chunk {chunk_index} byte range overflows u64")]
    ChunkRangeOverflow { chunk_index: usize },
    /// Chunk offsets did not form one contiguous payload.
    #[error("chunk {chunk_index} offset {actual} must equal {expected}")]
    NonContiguousChunk {
        chunk_index: usize,
        expected: u64,
        actual: u64,
    },
    /// Chunk ranges did not exactly cover `content_length`.
    #[error("chunk ranges cover {actual} bytes but content_length is {expected}")]
    ContentLengthMismatch { expected: u64, actual: u64 },
    /// A plan omitted its logical file inventory.
    #[error("CAR plan must contain at least one logical file")]
    MissingFiles,
    /// File inventory exceeded the production protocol bound.
    #[error("CAR plan has {count} files; maximum is {maximum}")]
    TooManyFiles { count: usize, maximum: usize },
    /// Only a single-file plan may use the implicit empty logical path.
    #[error("file {file_index} has an empty logical path in a multi-file plan")]
    EmptyPathInMultiFilePlan { file_index: usize },
    /// Logical path component was not one portable normal component.
    #[error("file {file_index} path component {component_index} is not portable and normal")]
    InvalidPathComponent {
        file_index: usize,
        component_index: usize,
    },
    /// One logical path contained too many components.
    #[error("file {file_index} path has {count} components; maximum is {maximum}")]
    PathTooDeep {
        file_index: usize,
        count: usize,
        maximum: usize,
    },
    /// One logical path component exceeded its byte bound.
    #[error(
        "file {file_index} path component {component_index} has {bytes} bytes; maximum is {maximum}"
    )]
    PathComponentTooLong {
        file_index: usize,
        component_index: usize,
        bytes: usize,
        maximum: usize,
    },
    /// One logical path exceeded its aggregate byte bound.
    #[error("file {file_index} path has {bytes} bytes; maximum is {maximum}")]
    PathTooLong {
        file_index: usize,
        bytes: usize,
        maximum: usize,
    },
    /// Logical files were not strictly ordered or had duplicate paths.
    #[error("file {file_index} path is not strictly ordered after its predecessor")]
    NonCanonicalFileOrder { file_index: usize },
    /// A logical file conflicted with an ancestor file path.
    #[error("file {file_index} path descends from a logical file")]
    FilePathConflict { file_index: usize },
    /// A file's chunk range overflowed `usize`.
    #[error("file {file_index} chunk range overflows usize")]
    FileChunkRangeOverflow { file_index: usize },
    /// A file referenced chunks beyond the plan.
    #[error("file {file_index} chunk range ends at {end}, beyond chunk count {chunk_count}")]
    FileChunkRangeOutOfBounds {
        file_index: usize,
        end: usize,
        chunk_count: usize,
    },
    /// Logical files must own the chunk sequence exactly once and in order.
    #[error("file {file_index} first_chunk {actual} must equal {expected}")]
    NonCanonicalFileChunkStart {
        file_index: usize,
        expected: usize,
        actual: usize,
    },
    /// Empty files must not own a synthetic zero-length chunk.
    #[error("empty file {file_index} must reference zero chunks")]
    EmptyFileHasChunks { file_index: usize },
    /// Non-empty files must own at least one chunk.
    #[error("non-empty file {file_index} has no chunks")]
    NonEmptyFileHasNoChunks { file_index: usize },
    /// Logical file byte range overflowed `u64`.
    #[error("file {file_index} byte range overflows u64")]
    FileByteRangeOverflow { file_index: usize },
    /// File byte range and owned chunk byte range differed.
    #[error("file {file_index} byte range does not align with its chunk range")]
    FileChunkByteRangeMismatch { file_index: usize },
    /// File sizes did not exactly cover the payload.
    #[error("logical files cover {actual} bytes but content_length is {expected}")]
    FileCoverageMismatch { expected: u64, actual: u64 },
    /// File chunk ranges did not exactly partition all chunks.
    #[error("logical files cover {actual} chunks but the plan contains {expected}")]
    ChunkCoverageMismatch { expected: usize, actual: usize },
    /// Canonical per-file chunk count exceeded the maximum implied by its size/profile.
    #[error("file {file_index} has {actual} chunks; canonical maximum is {maximum}")]
    TooManyFileChunks {
        file_index: usize,
        maximum: usize,
        actual: usize,
    },
    /// A checked allocation estimate overflowed host geometry.
    #[error("allocation estimate overflowed while accounting for {context}")]
    EstimateOverflow { context: &'static str },
}

fn ensure_car_plan_profile(profile: ChunkProfile) -> Result<(), CarPlanError> {
    profile.validate()?;
    let limit = CHUNK_STORE_MAX_CHUNK_BYTES as usize;
    if profile.max_size > limit {
        return Err(CarPlanError::ChunkProfileMaxSizeTooLarge {
            max_size: profile.max_size,
            limit: CHUNK_STORE_MAX_CHUNK_BYTES,
        });
    }
    Ok(())
}

fn ensure_chunk_store_profile(profile: ChunkProfile) -> Result<(), ChunkStoreError> {
    profile.validate()?;
    let limit = CHUNK_STORE_MAX_CHUNK_BYTES as usize;
    if profile.max_size > limit {
        return Err(ChunkStoreError::ChunkProfileMaxSizeTooLarge {
            max_size: profile.max_size,
            limit: CHUNK_STORE_MAX_CHUNK_BYTES,
        });
    }
    Ok(())
}

fn preflight_chunk_store_plan(
    plan: &CarBuildPlan,
    max_estimated_heap_bytes: usize,
) -> Result<CarPlanValidation, ChunkStoreError> {
    plan.validate_for_ingest_with_limit(max_estimated_heap_bytes)
}

#[cfg(feature = "manifest")]
fn build_canonical_pdp_tree(payload: &[u8]) -> Result<Option<PdpMerkleTreeV1>, PdpMerkleTreeError> {
    if payload.is_empty() {
        return Ok(None);
    }
    let mut builder = PdpMerkleTreeBuilderV1::new();
    builder.update(payload)?;
    builder.finish().map(Some)
}

#[cfg(feature = "manifest")]
const META_TAIKAI_EVENT_ID: &str = "taikai.event_id";
#[cfg(feature = "manifest")]
const META_TAIKAI_STREAM_ID: &str = "taikai.stream_id";
#[cfg(feature = "manifest")]
const META_TAIKAI_RENDITION_ID: &str = "taikai.rendition_id";
#[cfg(feature = "manifest")]
const META_TAIKAI_SEGMENT_SEQUENCE: &str = "taikai.segment.sequence";
#[cfg(feature = "manifest")]
const META_TAIKAI_CACHE_HINT: &str = "taikai.cache_hint";
#[cfg(feature = "manifest")]
const TAIKAI_NAME_MAX_BYTES: usize = 255;
#[cfg(feature = "manifest")]
const TAIKAI_CACHE_HINT_MAX_BYTES: usize = 4 * 1024;

/// Errors emitted when deriving a [`CarBuildPlan`] from a DA manifest.
#[cfg(feature = "manifest")]
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanFromManifestError {
    /// Manifest did not contain any chunk commitments.
    #[error("manifest does not contain any chunk commitments")]
    EmptyChunks,
    /// Manifest advertised a zero chunk size.
    #[error("manifest chunk_size must be non-zero")]
    ZeroChunkSize,
    /// Manifest chunk size exceeded host bounds when converting to usize.
    #[error("manifest chunk_size {0} exceeds host limits")]
    ChunkSizeTooLarge(u32),
    /// Manifest omitted a required Taikai metadata field.
    #[error("manifest missing required Taikai metadata `{0}`")]
    MissingTaikaiMetadata(&'static str),
    /// Manifest contained an invalid Taikai metadata field.
    #[error("manifest contained invalid Taikai metadata `{field}`: {reason}")]
    InvalidTaikaiMetadata {
        /// The metadata key that failed validation.
        field: &'static str,
        /// Description of why validation failed.
        reason: String,
    },
    /// Manifest-derived plan storage exceeded the production planning heap ceiling.
    #[error(
        "manifest-derived CAR plan requires at least {estimated} bytes of heap; maximum is {limit}"
    )]
    EstimatedPlanHeapLimitExceeded { estimated: usize, limit: usize },
    /// A bounded manifest-derived plan allocation could not be reserved.
    #[error("failed to reserve {requested} entries/bytes for {context}")]
    AllocationFailed {
        context: &'static str,
        requested: usize,
    },
    /// Manifest-derived chunk/file geometry was not canonical or bounded.
    #[error("manifest produced an invalid CAR plan: {0}")]
    InvalidPlan(#[from] CarPlanValidationError),
}

/// Build a [`CarBuildPlan`] directly from a canonical DA manifest.
#[cfg(feature = "manifest")]
pub fn build_plan_from_da_manifest(
    manifest: &DaManifestV1,
) -> Result<CarBuildPlan, PlanFromManifestError> {
    if manifest.chunks.is_empty() {
        return Err(PlanFromManifestError::EmptyChunks);
    }
    if manifest.chunk_size == 0 {
        return Err(PlanFromManifestError::ZeroChunkSize);
    }
    let chunk_count = manifest.chunks.len();
    if chunk_count > CAR_PLAN_MAX_CHUNKS {
        return Err(PlanFromManifestError::InvalidPlan(
            CarPlanValidationError::TooManyChunks {
                count: chunk_count,
                maximum: CAR_PLAN_MAX_CHUNKS,
            },
        ));
    }
    let chunk_size = usize::try_from(manifest.chunk_size)
        .map_err(|_| PlanFromManifestError::ChunkSizeTooLarge(manifest.chunk_size))?;
    if manifest.chunk_size > CHUNK_STORE_MAX_CHUNK_BYTES {
        return Err(PlanFromManifestError::InvalidPlan(
            CarPlanValidationError::ChunkProfileTooLarge {
                max_size: chunk_size,
                limit: CHUNK_STORE_MAX_CHUNK_BYTES,
            },
        ));
    }
    let chunk_profile = ChunkProfile {
        min_size: chunk_size,
        target_size: chunk_size,
        max_size: chunk_size,
        break_mask: 1,
    };
    preflight_da_manifest_chunks(manifest, chunk_size)?;
    let payload_digest = Hash::from(*manifest.blob_hash.as_ref());
    let taikai_hint = taikai_segment_hint_from_manifest(manifest)?;
    preflight_manifest_plan_heap(chunk_count, taikai_hint.as_ref())?;

    let mut chunks = Vec::new();
    try_reserve_manifest(&mut chunks, chunk_count, "manifest CAR chunk inventory")?;
    for chunk in &manifest.chunks {
        chunks.push(CarChunk {
            offset: chunk.offset,
            length: chunk.length,
            digest: *chunk.commitment.as_ref(),
            taikai_segment_hint: taikai_hint
                .as_ref()
                .map(try_clone_taikai_hint_for_manifest)
                .transpose()?,
        });
    }
    let mut path = Vec::new();
    try_reserve_manifest(&mut path, 1, "manifest logical path")?;
    path.push(try_owned_manifest_string(
        "payload.bin",
        "manifest logical path component",
    )?);
    let mut files = Vec::new();
    try_reserve_manifest(&mut files, 1, "manifest file inventory")?;
    files.push(FilePlan {
        path,
        first_chunk: 0,
        chunk_count: chunks.len(),
        size: manifest.total_size,
    });
    let plan = CarBuildPlan {
        chunk_profile,
        payload_digest,
        content_length: manifest.total_size,
        chunks,
        files,
    };
    plan.validate()?;
    Ok(plan)
}

#[cfg(feature = "manifest")]
fn try_reserve_manifest<T>(
    values: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), PlanFromManifestError> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| PlanFromManifestError::AllocationFailed {
            context,
            requested: additional,
        })
}

#[cfg(feature = "manifest")]
fn try_owned_manifest_string(
    value: &str,
    context: &'static str,
) -> Result<String, PlanFromManifestError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| PlanFromManifestError::AllocationFailed {
            context,
            requested: value.len(),
        })?;
    owned.push_str(value);
    Ok(owned)
}

#[cfg(feature = "manifest")]
fn try_clone_taikai_hint_for_manifest(
    hint: &TaikaiSegmentHint,
) -> Result<TaikaiSegmentHint, PlanFromManifestError> {
    Ok(TaikaiSegmentHint {
        event: try_owned_manifest_string(&hint.event, "Taikai event hint clone")?,
        stream: try_owned_manifest_string(&hint.stream, "Taikai stream hint clone")?,
        rendition: try_owned_manifest_string(&hint.rendition, "Taikai rendition hint clone")?,
        sequence: hint.sequence,
        payload_len: hint.payload_len,
        payload_digest: hint.payload_digest,
    })
}

#[cfg(feature = "manifest")]
fn preflight_manifest_plan_heap(
    chunk_count: usize,
    hint: Option<&TaikaiSegmentHint>,
) -> Result<(), PlanFromManifestError> {
    let hint_bytes = match hint {
        Some(hint) => hint
            .event
            .len()
            .checked_add(hint.stream.len())
            .and_then(|bytes| bytes.checked_add(hint.rendition.len()))
            .ok_or(PlanFromManifestError::EstimatedPlanHeapLimitExceeded {
                estimated: usize::MAX,
                limit: DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
            })?,
        None => 0,
    };
    let per_chunk = std::mem::size_of::<CarChunk>()
        .checked_add(hint_bytes)
        .ok_or(PlanFromManifestError::EstimatedPlanHeapLimitExceeded {
            estimated: usize::MAX,
            limit: DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
        })?;
    let estimated = per_chunk
        .checked_mul(chunk_count)
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<FilePlan>()))
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<String>()))
        .and_then(|bytes| bytes.checked_add("payload.bin".len()))
        .ok_or(PlanFromManifestError::EstimatedPlanHeapLimitExceeded {
            estimated: usize::MAX,
            limit: DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
        })?;
    if estimated > DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES {
        return Err(PlanFromManifestError::EstimatedPlanHeapLimitExceeded {
            estimated,
            limit: DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
        });
    }
    Ok(())
}

#[cfg(feature = "manifest")]
fn preflight_da_manifest_chunks(
    manifest: &DaManifestV1,
    chunk_size: usize,
) -> Result<(), PlanFromManifestError> {
    if manifest.total_size == 0 {
        return Err(PlanFromManifestError::InvalidPlan(
            CarPlanValidationError::EmptyPayloadHasChunks {
                count: manifest.chunks.len(),
            },
        ));
    }
    let profile_limit = u32::try_from(chunk_size)
        .map_err(|_| PlanFromManifestError::ChunkSizeTooLarge(manifest.chunk_size))?;
    let mut expected_offset = 0u64;
    for (chunk_index, chunk) in manifest.chunks.iter().enumerate() {
        if chunk.length == 0 {
            return Err(PlanFromManifestError::InvalidPlan(
                CarPlanValidationError::ZeroLengthChunk { chunk_index },
            ));
        }
        if chunk.length > profile_limit {
            return Err(PlanFromManifestError::InvalidPlan(
                CarPlanValidationError::ChunkTooLarge {
                    chunk_index,
                    length: chunk.length,
                    limit: profile_limit,
                },
            ));
        }
        if chunk.offset != expected_offset {
            return Err(PlanFromManifestError::InvalidPlan(
                CarPlanValidationError::NonContiguousChunk {
                    chunk_index,
                    expected: expected_offset,
                    actual: chunk.offset,
                },
            ));
        }
        let chunk_length = usize::try_from(chunk.length).map_err(|_| {
            PlanFromManifestError::InvalidPlan(CarPlanValidationError::EstimateOverflow {
                context: "manifest chunk length host width",
            })
        })?;
        if chunk_index + 1 < manifest.chunks.len() && chunk_length < chunk_size {
            return Err(PlanFromManifestError::InvalidPlan(
                CarPlanValidationError::ChunkBelowProfileMinimum {
                    chunk_index,
                    length: chunk.length,
                    minimum: chunk_size,
                },
            ));
        }
        expected_offset = chunk.offset.checked_add(u64::from(chunk.length)).ok_or(
            PlanFromManifestError::InvalidPlan(CarPlanValidationError::ChunkRangeOverflow {
                chunk_index,
            }),
        )?;
    }
    if expected_offset != manifest.total_size {
        return Err(PlanFromManifestError::InvalidPlan(
            CarPlanValidationError::ContentLengthMismatch {
                expected: manifest.total_size,
                actual: expected_offset,
            },
        ));
    }
    let chunk_size_u64 = u64::try_from(chunk_size)
        .map_err(|_| PlanFromManifestError::ChunkSizeTooLarge(manifest.chunk_size))?;
    let maximum_count =
        usize::try_from(manifest.total_size.div_ceil(chunk_size_u64)).map_err(|_| {
            PlanFromManifestError::InvalidPlan(CarPlanValidationError::EstimateOverflow {
                context: "manifest canonical chunk count",
            })
        })?;
    if manifest.chunks.len() > maximum_count {
        return Err(PlanFromManifestError::InvalidPlan(
            CarPlanValidationError::TooManyFileChunks {
                file_index: 0,
                maximum: maximum_count,
                actual: manifest.chunks.len(),
            },
        ));
    }
    Ok(())
}

#[cfg(feature = "manifest")]
fn parse_canonical_taikai_sequence(raw: &str) -> Result<u64, PlanFromManifestError> {
    if raw.is_empty()
        || raw.len() > 20
        || !raw.bytes().all(|byte| byte.is_ascii_digit())
        || (raw.len() > 1 && raw.starts_with('0'))
    {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_SEGMENT_SEQUENCE,
            reason: "sequence must be canonical unsigned decimal without whitespace, sign, or leading zeroes"
                .into(),
        });
    }
    raw.parse::<u64>()
        .map_err(|err| PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_SEGMENT_SEQUENCE,
            reason: err.to_string(),
        })
}

/// Extract a Taikai segment hint from a manifest when the blob class indicates a Taikai segment.
#[cfg(feature = "manifest")]
pub fn taikai_segment_hint_from_manifest(
    manifest: &DaManifestV1,
) -> Result<Option<TaikaiSegmentHint>, PlanFromManifestError> {
    if manifest.blob_class != BlobClass::TaikaiSegment {
        return Ok(None);
    }

    let metadata = &manifest.metadata;
    let event = parse_taikai_name(metadata, META_TAIKAI_EVENT_ID)?;
    let stream = parse_taikai_name(metadata, META_TAIKAI_STREAM_ID)?;
    let rendition = parse_taikai_name(metadata, META_TAIKAI_RENDITION_ID)?;
    let sequence_raw = read_taikai_metadata_field(metadata, META_TAIKAI_SEGMENT_SEQUENCE)?;
    let sequence = parse_canonical_taikai_sequence(sequence_raw)?;
    let cache_hint = decode_taikai_cache_hint(metadata)?;

    let mut hint = TaikaiSegmentHint {
        event: try_owned_manifest_string(event.as_ref(), "Taikai event hint")?,
        stream: try_owned_manifest_string(stream.as_ref(), "Taikai stream hint")?,
        rendition: try_owned_manifest_string(rendition.as_ref(), "Taikai rendition hint")?,
        sequence,
        payload_len: None,
        payload_digest: None,
    };
    if let Some(cache_hint) = cache_hint {
        hint.payload_len = cache_hint.payload_len;
        hint.payload_digest = cache_hint.payload_digest;
    }

    Ok(Some(hint))
}

/// Derive a Taikai segment hint from a stored SoraFS manifest (metadata-based).
///
/// Returns `Ok(None)` when no Taikai metadata keys are present; otherwise
/// validates the fields and surfaces the same errors as
/// [`taikai_segment_hint_from_manifest`].
#[cfg(feature = "manifest")]
pub fn taikai_segment_hint_from_sorafs_manifest(
    manifest: &SorafsManifestV1,
) -> Result<Option<TaikaiSegmentHint>, PlanFromManifestError> {
    fn lookup<'a>(
        manifest: &'a SorafsManifestV1,
        key: &'static str,
    ) -> Result<Option<&'a str>, PlanFromManifestError> {
        let mut matches = manifest.metadata.iter().filter(|entry| entry.key == key);
        let value = matches.next().map(|entry| entry.value.as_str());
        if matches.next().is_some() {
            return Err(PlanFromManifestError::InvalidTaikaiMetadata {
                field: key,
                reason: "metadata field must occur at most once".into(),
            });
        }
        Ok(value)
    }

    let Some(event_raw) = lookup(manifest, META_TAIKAI_EVENT_ID)? else {
        return Ok(None);
    };
    let stream_raw = lookup(manifest, META_TAIKAI_STREAM_ID)?.ok_or(
        PlanFromManifestError::MissingTaikaiMetadata(META_TAIKAI_STREAM_ID),
    )?;
    let rendition_raw = lookup(manifest, META_TAIKAI_RENDITION_ID)?.ok_or(
        PlanFromManifestError::MissingTaikaiMetadata(META_TAIKAI_RENDITION_ID),
    )?;
    let sequence_raw = lookup(manifest, META_TAIKAI_SEGMENT_SEQUENCE)?.ok_or(
        PlanFromManifestError::MissingTaikaiMetadata(META_TAIKAI_SEGMENT_SEQUENCE),
    )?;

    let event = parse_taikai_name_value(event_raw, META_TAIKAI_EVENT_ID)?;
    let stream = parse_taikai_name_value(stream_raw, META_TAIKAI_STREAM_ID)?;
    let rendition = parse_taikai_name_value(rendition_raw, META_TAIKAI_RENDITION_ID)?;
    let sequence = parse_canonical_taikai_sequence(sequence_raw)?;
    let cache_hint = decode_taikai_cache_hint_from_sorafs_manifest(manifest)?;

    let mut hint = TaikaiSegmentHint {
        event: try_owned_manifest_string(event.as_ref(), "Taikai event hint")?,
        stream: try_owned_manifest_string(stream.as_ref(), "Taikai stream hint")?,
        rendition: try_owned_manifest_string(rendition.as_ref(), "Taikai rendition hint")?,
        sequence,
        payload_len: None,
        payload_digest: None,
    };
    if let Some(cache_hint) = cache_hint {
        hint.payload_len = cache_hint.payload_len;
        hint.payload_digest = cache_hint.payload_digest;
    }

    Ok(Some(hint))
}

#[cfg(feature = "manifest")]
fn parse_taikai_name(
    metadata: &ExtraMetadata,
    key: &'static str,
) -> Result<Name, PlanFromManifestError> {
    let raw = read_taikai_metadata_field(metadata, key)?;
    parse_taikai_name_value(raw, key)
}

#[cfg(feature = "manifest")]
fn parse_taikai_name_value(raw: &str, key: &'static str) -> Result<Name, PlanFromManifestError> {
    if raw.len() > TAIKAI_NAME_MAX_BYTES {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: key,
            reason: format!("name exceeds {TAIKAI_NAME_MAX_BYTES} UTF-8 bytes"),
        });
    }
    let name = Name::from_str(raw).map_err(|err| PlanFromManifestError::InvalidTaikaiMetadata {
        field: key,
        reason: err.to_string(),
    })?;
    if name.as_ref() != raw {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: key,
            reason: "name must use its exact canonical Unicode representation".into(),
        });
    }
    Ok(name)
}

#[cfg(feature = "manifest")]
fn read_taikai_metadata_field<'a>(
    metadata: &'a ExtraMetadata,
    key: &'static str,
) -> Result<&'a str, PlanFromManifestError> {
    let mut matches = metadata.items.iter().filter(|entry| entry.key == key);
    let entry = matches
        .next()
        .ok_or(PlanFromManifestError::MissingTaikaiMetadata(key))?;
    if matches.next().is_some() {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: key,
            reason: "metadata field must occur exactly once".into(),
        });
    }
    if entry.visibility != MetadataVisibility::Public {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: key,
            reason: "metadata must be public".into(),
        });
    }
    if !matches!(entry.encryption, MetadataEncryption::None) {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: key,
            reason: "metadata must be unencrypted".into(),
        });
    }
    std::str::from_utf8(&entry.value).map_err(|err| PlanFromManifestError::InvalidTaikaiMetadata {
        field: key,
        reason: err.to_string(),
    })
}

#[cfg(feature = "manifest")]
#[derive(Debug, Default)]
struct CacheHintFields {
    payload_len: Option<u64>,
    payload_digest: Option<[u8; 32]>,
}

#[cfg(feature = "manifest")]
fn decode_taikai_cache_hint(
    metadata: &ExtraMetadata,
) -> Result<Option<CacheHintFields>, PlanFromManifestError> {
    let mut matches = metadata
        .items
        .iter()
        .filter(|entry| entry.key == META_TAIKAI_CACHE_HINT);
    let Some(entry) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: "metadata field must occur at most once".into(),
        });
    }
    if entry.visibility != MetadataVisibility::Public {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: "metadata must be public".into(),
        });
    }
    if !matches!(entry.encryption, MetadataEncryption::None) {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: "metadata must be unencrypted".into(),
        });
    }
    let raw = std::str::from_utf8(&entry.value).map_err(|err| {
        PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: format!("invalid UTF-8: {err}"),
        }
    })?;
    if raw.len() > TAIKAI_CACHE_HINT_MAX_BYTES {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: format!("cache hint exceeds {TAIKAI_CACHE_HINT_MAX_BYTES} UTF-8 bytes"),
        });
    }
    let value: Value =
        json::from_str(raw).map_err(|err| PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: format!("invalid JSON: {err}"),
        })?;
    let hint_obj = value
        .as_object()
        .ok_or(PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: "cache hint must be a JSON object".into(),
        })?;

    let payload_len = hint_obj
        .get("payload_len")
        .map(|value| {
            value
                .as_u64()
                .ok_or(PlanFromManifestError::InvalidTaikaiMetadata {
                    field: META_TAIKAI_CACHE_HINT,
                    reason: "payload_len must be an unsigned integer".into(),
                })
        })
        .transpose()?;
    let payload_digest = match hint_obj.get("payload_blake3_hex") {
        Some(Value::String(hex)) => Some(decode_digest_hex_hint(hex)?),
        Some(Value::Null) | None => None,
        Some(_) => {
            return Err(PlanFromManifestError::InvalidTaikaiMetadata {
                field: META_TAIKAI_CACHE_HINT,
                reason: "payload_blake3_hex must be a hex string".into(),
            });
        }
    };

    Ok(Some(CacheHintFields {
        payload_len,
        payload_digest,
    }))
}

#[cfg(feature = "manifest")]
fn decode_taikai_cache_hint_from_sorafs_manifest(
    manifest: &SorafsManifestV1,
) -> Result<Option<CacheHintFields>, PlanFromManifestError> {
    let mut matches = manifest
        .metadata
        .iter()
        .filter(|entry| entry.key == META_TAIKAI_CACHE_HINT);
    let Some(entry) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: "metadata field must occur at most once".into(),
        });
    }
    if entry.value.len() > TAIKAI_CACHE_HINT_MAX_BYTES {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: format!("cache hint exceeds {TAIKAI_CACHE_HINT_MAX_BYTES} UTF-8 bytes"),
        });
    }
    let value: Value = json::from_str(entry.value.as_str()).map_err(|err| {
        PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: format!("invalid JSON: {err}"),
        }
    })?;
    let hint_obj = value
        .as_object()
        .ok_or(PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: "cache hint must be a JSON object".into(),
        })?;

    let payload_len = hint_obj
        .get("payload_len")
        .map(|value| {
            value
                .as_u64()
                .ok_or(PlanFromManifestError::InvalidTaikaiMetadata {
                    field: META_TAIKAI_CACHE_HINT,
                    reason: "payload_len must be an unsigned integer".into(),
                })
        })
        .transpose()?;
    let payload_digest = match hint_obj.get("payload_blake3_hex") {
        Some(Value::String(hex)) => Some(decode_digest_hex_hint(hex)?),
        Some(Value::Null) | None => None,
        Some(_) => {
            return Err(PlanFromManifestError::InvalidTaikaiMetadata {
                field: META_TAIKAI_CACHE_HINT,
                reason: "payload_blake3_hex must be a hex string".into(),
            });
        }
    };

    Ok(Some(CacheHintFields {
        payload_len,
        payload_digest,
    }))
}

#[cfg(feature = "manifest")]
fn decode_digest_hex_hint(hex: &str) -> Result<[u8; 32], PlanFromManifestError> {
    if hex.len() != 64 {
        return Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: "payload_blake3_hex must be 64 hex characters".into(),
        });
    }
    let mut bytes = [0u8; 32];
    for (idx, chunk) in hex.as_bytes().chunks_exact(2).enumerate() {
        let hi = decode_hex_nibble_hint(chunk[0])?;
        let lo = decode_hex_nibble_hint(chunk[1])?;
        bytes[idx] = (hi << 4) | lo;
    }
    Ok(bytes)
}

#[cfg(feature = "manifest")]
fn decode_hex_nibble_hint(byte: u8) -> Result<u8, PlanFromManifestError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_CACHE_HINT,
            reason: "payload_blake3_hex contains non-hex characters".into(),
        }),
    }
}

/// Specification for fetching a chunk from storage or remote peers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkFetchSpec {
    /// Index of the chunk within the plan (0-based).
    pub chunk_index: usize,
    /// Byte offset within the original payload.
    pub offset: u64,
    /// Length of the chunk in bytes.
    pub length: u32,
    /// Expected BLAKE3-256 digest of the chunk contents.
    pub digest: [u8; 32],
    /// Optional Taikai segment hint propagated from the ingest manifest.
    pub taikai_segment_hint: Option<TaikaiSegmentHint>,
}

/// Canonical chunk store used during SoraFS node ingestion.
///
/// Captures chunk metadata, payload digests, and the two-level (64 KiB / 4 KiB) PoR sampling tree.
/// With the `manifest` feature it also builds the canonical global 4 KiB / 256 KiB PDP v1 tree.
#[derive(Debug, Clone)]
pub struct ChunkStore {
    profile: sorafs_chunker::ChunkProfile,
    chunks: Vec<StoredChunk>,
    por_tree: PorMerkleTree,
    payload_digest: Hash,
    payload_len: u64,
    max_estimated_heap_bytes: usize,
    #[cfg(feature = "manifest")]
    pdp_tree: Option<PdpMerkleTreeV1>,
}

/// Sink trait used by [`ChunkStore::ingest_plan_source_with_sink`] to persist chunk payloads.
pub trait ChunkSink {
    /// Output produced after all chunks have been written.
    type Output;

    /// Prepare internal state before chunk ingestion begins.
    fn prepare(&mut self, plan: &CarBuildPlan) -> Result<(), ChunkStoreError>;

    /// Write an individual chunk payload.
    fn write_chunk(
        &mut self,
        index: usize,
        chunk: &CarChunk,
        data: &[u8],
    ) -> Result<(), ChunkStoreError>;

    /// Finish the sink and return the final output.
    fn finish(self) -> Result<Self::Output, ChunkStoreError>;
}

#[derive(Debug, Default)]
struct NoopSink;

impl ChunkSink for NoopSink {
    type Output = ();

    fn prepare(&mut self, _plan: &CarBuildPlan) -> Result<(), ChunkStoreError> {
        Ok(())
    }

    fn write_chunk(
        &mut self,
        _index: usize,
        _chunk: &CarChunk,
        _data: &[u8],
    ) -> Result<(), ChunkStoreError> {
        Ok(())
    }

    fn finish(self) -> Result<Self::Output, ChunkStoreError> {
        Ok(())
    }
}

/// Metadata describing a chunk persisted to disk by [`DirectoryChunkSink`].
#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
pub struct PersistedChunkRecord {
    /// File name relative to the sink directory.
    pub file_name: String,
    /// Chunk offset within the original payload.
    pub offset: u64,
    /// Chunk length in bytes.
    pub length: u32,
    /// Expected BLAKE3 digest of the chunk.
    pub digest: [u8; 32],
}

/// Output returned by [`DirectoryChunkSink`].
#[derive(Debug)]
pub struct DirectoryChunkSinkOutput {
    /// Records describing each persisted chunk file.
    pub records: Vec<PersistedChunkRecord>,
    /// Total number of bytes written across all chunks.
    pub total_bytes: u64,
    /// Durability result after the staging directory became visible at its immutable root.
    pub publication: DirectoryPublicationStatus,
}

/// Outcome of the irreversible staging-to-root rename.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryPublicationStatus {
    /// Published root identity was confirmed and its parent directory fsync succeeded.
    Durable,
    /// The complete root was published, but a post-rename identity check or parent fsync failed.
    ///
    /// This is a successful, non-retryable publication outcome. Callers must retain the returned
    /// records and reconcile durability on restart rather than attempting the ingest again.
    PublishedButDurabilityUncertain,
}

const ATOMIC_PATH_RETRIES: usize = 32;

fn normalized_parent(path: &Path) -> Result<&Path, ChunkStoreError> {
    path.parent()
        .map(|parent| {
            if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            }
        })
        .ok_or_else(|| ChunkStoreError::Io(io::Error::other("missing parent directory")))
}

#[cfg(unix)]
fn canonical_existing_private_directory(path: &Path) -> Result<PathBuf, ChunkStoreError> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ChunkStoreError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "chunk sink parent must not contain parent-directory components",
        )));
    }
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(ChunkStoreError::Io)?
            .join(path)
    };
    let mut lexical = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => lexical.push(prefix.as_os_str()),
            Component::RootDir => lexical.push(Path::new("/")),
            Component::CurDir => {}
            Component::Normal(value) => lexical.push(value),
            Component::ParentDir => {
                return Err(ChunkStoreError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "chunk sink parent contains a parent-directory component",
                )));
            }
        }
    }
    let canonical = fs::canonicalize(&lexical).map_err(ChunkStoreError::Io)?;
    if canonical != lexical {
        return Err(ChunkStoreError::Io(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "chunk sink parent chain must not contain symbolic links",
        )));
    }
    let metadata = fs::symlink_metadata(&canonical).map_err(ChunkStoreError::Io)?;
    validate_directory_metadata(&canonical, &metadata)?;
    if metadata.permissions().mode() & 0o022 != 0 {
        return Err(ChunkStoreError::Io(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "chunk sink parent must not be group- or world-writable",
        )));
    }
    Ok(canonical)
}

#[cfg(not(unix))]
fn canonical_existing_private_directory(_path: &Path) -> Result<PathBuf, ChunkStoreError> {
    Err(ChunkStoreError::Io(unsupported_secure_filesystem_error()))
}

fn validate_directory_path(path: &Path) -> Result<(), ChunkStoreError> {
    let metadata = fs::symlink_metadata(path).map_err(ChunkStoreError::Io)?;
    validate_directory_metadata(path, &metadata)
}

fn validate_directory_metadata(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), ChunkStoreError> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ChunkStoreError::Io(io::Error::other(format!(
            "chunk sink path `{}` must be a real directory",
            path.display()
        ))));
    }
    Ok(())
}

fn validate_atomic_destination_absent(path: &Path) -> Result<(), ChunkStoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            let kind = if metadata.file_type().is_symlink() {
                "symlink"
            } else if metadata.is_dir() {
                "directory"
            } else {
                #[cfg(unix)]
                if metadata.nlink() != 1 {
                    "hard-linked file"
                } else {
                    "file"
                }
                #[cfg(not(unix))]
                {
                    "file"
                }
            };
            Err(ChunkStoreError::Io(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "refusing to replace {kind} at atomic chunk destination `{}`",
                    path.display()
                ),
            )))
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(ChunkStoreError::Io(err)),
    }
}

fn validate_atomic_temp(path: &Path, file: &File) -> Result<(), ChunkStoreError> {
    let opened = file.metadata().map_err(ChunkStoreError::Io)?;
    let linked = fs::symlink_metadata(path).map_err(ChunkStoreError::Io)?;
    if !opened.is_file() || linked.file_type().is_symlink() || !linked.is_file() {
        return Err(ChunkStoreError::Io(io::Error::other(format!(
            "atomic chunk temporary `{}` is not a regular file",
            path.display()
        ))));
    }
    if !metadata_identifies_same_file(&opened, &linked) {
        return Err(ChunkStoreError::Io(io::Error::other(format!(
            "atomic chunk temporary `{}` changed after opening",
            path.display()
        ))));
    }
    #[cfg(unix)]
    if opened.nlink() != 1 || linked.nlink() != 1 {
        return Err(ChunkStoreError::Io(io::Error::other(format!(
            "atomic chunk temporary `{}` has {} hard links",
            path.display(),
            opened.nlink().max(linked.nlink())
        ))));
    }
    Ok(())
}

#[cfg(unix)]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(unix)]
fn metadata_snapshot_matches(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    metadata_identifies_same_file(left, right)
        && left.file_type() == right.file_type()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
        && left.nlink() == right.nlink()
}

#[cfg(not(unix))]
fn metadata_identifies_same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.file_type() == right.file_type()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

#[cfg(not(unix))]
fn metadata_snapshot_matches(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    metadata_identifies_same_file(left, right) && left.created().ok() == right.created().ok()
}

fn remove_path_no_follow(path: &Path) {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            let _ = fs::remove_file(path);
        }
        Ok(_) => {
            let _ = fs::remove_dir_all(path);
        }
        Err(_) => {}
    }
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

#[cfg(unix)]
fn set_atomic_no_follow(options: &mut OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}

#[cfg(not(unix))]
fn set_atomic_no_follow(_options: &mut OpenOptions) {}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_no_follow_flag() -> i32 {
    0o400000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
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

/// Writes each chunk into a deterministic directory layout (`chunk_{idx:05}.bin`).
///
/// Writes are assembled in a private sibling directory and published only from [`Self::finish`].
/// Publication requires `root` to remain absent for the entire ingest; existing destinations are
/// never replaced. The final same-filesystem rename therefore exposes either no directory or the
/// complete immutable chunk set, including across a process crash.
/// Secure publication is currently Unix-only; other platforms fail closed.
#[derive(Debug)]
pub struct DirectoryChunkSink {
    root: PathBuf,
    max_estimated_heap_bytes: usize,
    records: Vec<PersistedChunkRecord>,
    total_bytes: u64,
    expected_chunks: Vec<ExpectedSinkChunk>,
    expected_total_bytes: u64,
    next_chunk_index: usize,
    staging_root: Option<PathBuf>,
    staging_before: Option<fs::Metadata>,
    parent_before: Option<fs::Metadata>,
    #[cfg(test)]
    commit_fault: Option<DirectoryCommitFault>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy)]
enum DirectoryCommitFault {
    PostRenameIdentity,
    ParentSync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExpectedSinkChunk {
    offset: u64,
    length: u32,
    digest: [u8; 32],
}

impl DirectoryChunkSink {
    /// Construct a sink that writes chunks into `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            max_estimated_heap_bytes: DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
            records: Vec::new(),
            total_bytes: 0,
            expected_chunks: Vec::new(),
            expected_total_bytes: 0,
            next_chunk_index: 0,
            staging_root: None,
            staging_before: None,
            parent_before: None,
            #[cfg(test)]
            commit_fault: None,
        }
    }

    /// Override the checked heap limit applied before this sink allocates plan-sized state.
    pub fn with_max_estimated_heap_bytes(
        mut self,
        max_estimated_heap_bytes: usize,
    ) -> Result<Self, ChunkStoreError> {
        if max_estimated_heap_bytes == 0 {
            return Err(ChunkStoreError::InvalidEstimatedHeapLimit);
        }
        self.max_estimated_heap_bytes = max_estimated_heap_bytes;
        Ok(self)
    }

    fn write_atomic(path: &Path, data: &[u8]) -> Result<(), ChunkStoreError> {
        let parent = normalized_parent(path)?;
        validate_directory_path(parent)?;
        let parent_before = fs::symlink_metadata(parent).map_err(ChunkStoreError::Io)?;
        validate_atomic_destination_absent(path)?;

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| ChunkStoreError::Io(io::Error::other("invalid chunk file name")))?;
        let mut temp_path = None;
        let mut temp_file = None;
        for _ in 0..ATOMIC_PATH_RETRIES {
            let nonce: [u8; 16] = rand::random();
            let candidate = parent.join(format!(".{file_name}.{}.partial", hex::encode(nonce)));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            set_atomic_no_follow(&mut options);
            #[cfg(unix)]
            options.mode(0o600);
            match options.open(&candidate) {
                Ok(file) => {
                    if let Err(error) = validate_atomic_temp(&candidate, &file) {
                        drop(file);
                        let _ = fs::remove_file(&candidate);
                        return Err(error);
                    }
                    temp_path = Some(candidate);
                    temp_file = Some(file);
                    break;
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(ChunkStoreError::Io(err)),
            }
        }
        let temp_path = temp_path.ok_or_else(|| {
            ChunkStoreError::Io(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "failed to allocate a collision-free chunk temporary file",
            ))
        })?;
        let mut file = temp_file.ok_or_else(|| {
            ChunkStoreError::Io(io::Error::other(
                "atomic chunk temporary file handle was not retained",
            ))
        })?;

        let result = (|| {
            file.write_all(data).map_err(ChunkStoreError::Io)?;
            file.sync_all().map_err(ChunkStoreError::Io)?;
            validate_atomic_temp(&temp_path, &file)?;
            drop(file);
            let parent_after = fs::symlink_metadata(parent).map_err(ChunkStoreError::Io)?;
            if !metadata_identifies_same_file(&parent_before, &parent_after) {
                return Err(ChunkStoreError::Io(io::Error::other(format!(
                    "atomic chunk parent `{}` changed during write",
                    parent.display()
                ))));
            }
            validate_directory_metadata(parent, &parent_after)?;
            validate_atomic_destination_absent(path)?;
            fs::rename(&temp_path, path).map_err(ChunkStoreError::Io)?;
            sync_directory(parent).map_err(ChunkStoreError::Io)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }

    fn create_staging_root(&self, parent: &Path) -> Result<PathBuf, ChunkStoreError> {
        for _ in 0..ATOMIC_PATH_RETRIES {
            let nonce: [u8; 16] = rand::random();
            let candidate = parent.join(format!(".sorafs-chunks.{}.partial", hex::encode(nonce)));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            builder.mode(0o700);
            match builder.create(&candidate) {
                Ok(()) => {
                    if let Err(error) = validate_directory_path(&candidate)
                        .and_then(|()| sync_directory(parent).map_err(ChunkStoreError::Io))
                    {
                        remove_path_no_follow(&candidate);
                        return Err(error);
                    }
                    return Ok(candidate);
                }
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(ChunkStoreError::Io(err)),
            }
        }
        Err(ChunkStoreError::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "failed to allocate a collision-free chunk staging directory",
        )))
    }

    fn validate_staging_unchanged(&self, staging: &Path) -> Result<(), ChunkStoreError> {
        let before = self
            .staging_before
            .as_ref()
            .ok_or_else(|| ChunkStoreError::Io(io::Error::other("chunk sink was not prepared")))?;
        let current = fs::symlink_metadata(staging).map_err(ChunkStoreError::Io)?;
        if !metadata_identifies_same_file(before, &current) {
            return Err(ChunkStoreError::Io(io::Error::other(format!(
                "chunk sink staging directory `{}` changed during ingest",
                staging.display()
            ))));
        }
        validate_directory_metadata(staging, &current)
    }

    fn validate_staged_chunks(&self, staging: &Path) -> Result<(), ChunkStoreError> {
        let mut entry_count = 0usize;
        for entry in fs::read_dir(staging).map_err(ChunkStoreError::Io)? {
            entry.map_err(ChunkStoreError::Io)?;
            entry_count = entry_count
                .checked_add(1)
                .ok_or(ChunkStoreError::AllocationFailed {
                    context: "staged directory entry count",
                    requested: usize::MAX,
                })?;
        }
        if entry_count != self.expected_chunks.len() {
            return Err(ChunkStoreError::SinkIncomplete {
                expected_chunks: self.expected_chunks.len(),
                actual_chunks: entry_count,
                expected_bytes: self.expected_total_bytes,
                actual_bytes: self.total_bytes,
            });
        }

        let mut read_buffer = [0u8; 64 * 1024];
        for (index, expected) in self.expected_chunks.iter().enumerate() {
            let path = staging.join(format!("chunk_{index:05}.bin"));
            let mut options = OpenOptions::new();
            options.read(true);
            set_atomic_no_follow(&mut options);
            let mut file = options.open(&path).map_err(ChunkStoreError::Io)?;
            validate_atomic_temp(&path, &file)?;
            let metadata = file.metadata().map_err(ChunkStoreError::Io)?;
            if metadata.len() != u64::from(expected.length) {
                return Err(ChunkStoreError::SinkChunkLengthMismatch {
                    chunk_index: index,
                    expected: expected.length as usize,
                    actual: usize::try_from(metadata.len()).unwrap_or(usize::MAX),
                });
            }
            let mut hasher = blake3::Hasher::new();
            loop {
                let read = file.read(&mut read_buffer).map_err(ChunkStoreError::Io)?;
                if read == 0 {
                    break;
                }
                hasher.update(&read_buffer[..read]);
            }
            if hasher.finalize().as_bytes() != &expected.digest {
                return Err(ChunkStoreError::SinkChunkDigestMismatch { chunk_index: index });
            }
        }
        Ok(())
    }

    fn commit_staging(
        &self,
        staging: &Path,
    ) -> Result<DirectoryPublicationStatus, ChunkStoreError> {
        let parent = normalized_parent(&self.root)?;
        let parent_now = fs::symlink_metadata(parent).map_err(ChunkStoreError::Io)?;
        let parent_before = self
            .parent_before
            .as_ref()
            .ok_or_else(|| ChunkStoreError::Io(io::Error::other("chunk sink was not prepared")))?;
        if !metadata_identifies_same_file(parent_before, &parent_now) {
            return Err(ChunkStoreError::Io(io::Error::other(format!(
                "chunk sink parent `{}` changed during ingest",
                parent.display()
            ))));
        }
        validate_directory_metadata(parent, &parent_now)?;
        validate_atomic_destination_absent(&self.root)?;
        self.validate_staging_unchanged(staging)?;
        self.validate_staged_chunks(staging)?;
        sync_directory(staging).map_err(ChunkStoreError::Io)?;
        validate_atomic_destination_absent(&self.root)?;
        fs::rename(staging, &self.root).map_err(ChunkStoreError::Io)?;
        let staged = self
            .staging_before
            .as_ref()
            .ok_or_else(|| ChunkStoreError::Io(io::Error::other("chunk sink was not prepared")))?;
        #[cfg(test)]
        let force_identity_failure = matches!(
            self.commit_fault,
            Some(DirectoryCommitFault::PostRenameIdentity)
        );
        #[cfg(not(test))]
        let force_identity_failure = false;
        let identity_confirmed = fs::symlink_metadata(&self.root)
            .map(|published| metadata_identifies_same_file(staged, &published))
            .unwrap_or(false)
            && !force_identity_failure;
        #[cfg(test)]
        let force_parent_sync_failure =
            matches!(self.commit_fault, Some(DirectoryCommitFault::ParentSync));
        #[cfg(not(test))]
        let force_parent_sync_failure = false;
        let parent_synced = if force_parent_sync_failure {
            false
        } else {
            sync_directory(parent).is_ok()
        };
        if identity_confirmed && parent_synced {
            Ok(DirectoryPublicationStatus::Durable)
        } else {
            Ok(DirectoryPublicationStatus::PublishedButDurabilityUncertain)
        }
    }
}

impl Clone for DirectoryChunkSink {
    fn clone(&self) -> Self {
        let mut clone = Self::new(self.root.clone());
        clone.max_estimated_heap_bytes = self.max_estimated_heap_bytes;
        clone
    }
}

impl Drop for DirectoryChunkSink {
    fn drop(&mut self) {
        if let (Some(staging), Some(before)) =
            (self.staging_root.take(), self.staging_before.take())
            && let Ok(current) = fs::symlink_metadata(&staging)
            && metadata_identifies_same_file(&before, &current)
        {
            remove_path_no_follow(&staging);
        }
    }
}

impl ChunkSink for DirectoryChunkSink {
    type Output = DirectoryChunkSinkOutput;

    fn prepare(&mut self, plan: &CarBuildPlan) -> Result<(), ChunkStoreError> {
        if !cfg!(unix) {
            let _ = plan;
            return Err(ChunkStoreError::Io(unsupported_secure_filesystem_error()));
        }
        if self.staging_root.is_some() {
            return Err(ChunkStoreError::Io(io::Error::other(
                "chunk sink is already prepared",
            )));
        }
        plan.validate_for_ingest_with_limit(self.max_estimated_heap_bytes)?;
        let root_name = match self.root.components().next_back() {
            Some(Component::Normal(name)) => name.to_os_string(),
            _ => {
                return Err(ChunkStoreError::Io(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "chunk sink root must end in a normal path component",
                )));
            }
        };
        let parent = normalized_parent(&self.root)?;
        let canonical_parent = canonical_existing_private_directory(parent)?;
        self.root = canonical_parent.join(root_name);
        validate_atomic_destination_absent(&self.root)?;
        let parent_before = fs::symlink_metadata(&canonical_parent).map_err(ChunkStoreError::Io)?;

        self.records.clear();
        self.records
            .try_reserve_exact(plan.chunks.len())
            .map_err(|_| ChunkStoreError::AllocationFailed {
                context: "directory sink records",
                requested: plan.chunks.len(),
            })?;
        self.expected_chunks.clear();
        self.expected_chunks
            .try_reserve_exact(plan.chunks.len())
            .map_err(|_| ChunkStoreError::AllocationFailed {
                context: "directory sink expected chunks",
                requested: plan.chunks.len(),
            })?;
        self.expected_chunks
            .extend(plan.chunks.iter().map(|chunk| ExpectedSinkChunk {
                offset: chunk.offset,
                length: chunk.length,
                digest: chunk.digest,
            }));
        self.total_bytes = 0;
        self.expected_total_bytes = plan.content_length;
        self.next_chunk_index = 0;
        self.parent_before = Some(parent_before);
        let staging = self.create_staging_root(&canonical_parent)?;
        let staging_before = match fs::symlink_metadata(&staging) {
            Ok(metadata) => metadata,
            Err(error) => {
                remove_path_no_follow(&staging);
                return Err(ChunkStoreError::Io(error));
            }
        };
        self.staging_root = Some(staging);
        self.staging_before = Some(staging_before);
        Ok(())
    }

    fn write_chunk(
        &mut self,
        index: usize,
        chunk: &CarChunk,
        data: &[u8],
    ) -> Result<(), ChunkStoreError> {
        if index != self.next_chunk_index {
            return Err(ChunkStoreError::SinkChunkOrder {
                expected: self.next_chunk_index,
                actual: index,
            });
        }
        let expected = self
            .expected_chunks
            .get(index)
            .ok_or(ChunkStoreError::SinkChunkOrder {
                expected: self.next_chunk_index,
                actual: index,
            })?;
        if expected.offset != chunk.offset
            || expected.length != chunk.length
            || expected.digest != chunk.digest
        {
            return Err(ChunkStoreError::SinkChunkMetadataMismatch { chunk_index: index });
        }
        let expected_len = usize::try_from(expected.length).map_err(|_| {
            ChunkStoreError::SinkChunkLengthMismatch {
                chunk_index: index,
                expected: usize::MAX,
                actual: data.len(),
            }
        })?;
        if data.len() != expected_len {
            return Err(ChunkStoreError::SinkChunkLengthMismatch {
                chunk_index: index,
                expected: expected_len,
                actual: data.len(),
            });
        }
        if blake3::hash(data).as_bytes() != &expected.digest {
            return Err(ChunkStoreError::SinkChunkDigestMismatch { chunk_index: index });
        }
        let file_name = format!("chunk_{index:05}.bin");
        let staging = self
            .staging_root
            .as_ref()
            .ok_or_else(|| ChunkStoreError::Io(io::Error::other("chunk sink was not prepared")))?;
        self.validate_staging_unchanged(staging)?;
        let path = staging.join(&file_name);
        Self::write_atomic(&path, data)?;
        self.records.push(PersistedChunkRecord {
            file_name,
            offset: chunk.offset,
            length: chunk.length,
            digest: chunk.digest,
        });
        let data_len =
            u64::try_from(data.len()).map_err(|_| ChunkStoreError::PayloadLengthTooLarge)?;
        self.total_bytes =
            self.total_bytes
                .checked_add(data_len)
                .ok_or(ChunkStoreError::LengthMismatch {
                    expected: u64::MAX,
                    actual: u64::MAX,
                })?;
        self.next_chunk_index += 1;
        Ok(())
    }

    fn finish(mut self) -> Result<Self::Output, ChunkStoreError> {
        if self.next_chunk_index != self.expected_chunks.len()
            || self.records.len() != self.expected_chunks.len()
            || self.total_bytes != self.expected_total_bytes
        {
            return Err(ChunkStoreError::SinkIncomplete {
                expected_chunks: self.expected_chunks.len(),
                actual_chunks: self.records.len(),
                expected_bytes: self.expected_total_bytes,
                actual_bytes: self.total_bytes,
            });
        }
        let staging = self
            .staging_root
            .as_ref()
            .ok_or_else(|| ChunkStoreError::Io(io::Error::other("chunk sink was not prepared")))?;
        let publication = self.commit_staging(staging)?;
        self.staging_root = None;
        self.staging_before = None;
        Ok(DirectoryChunkSinkOutput {
            records: std::mem::take(&mut self.records),
            total_bytes: self.total_bytes,
            publication,
        })
    }
}

impl ChunkStore {
    /// Creates a chunk store bound to the default `sorafs.sf1@1.0.0` profile.
    #[must_use]
    pub fn new() -> Self {
        Self::with_profile(sorafs_chunker::ChunkProfile::DEFAULT)
    }

    /// Creates a chunk store bound to the provided chunking profile.
    #[must_use]
    pub fn with_profile(profile: sorafs_chunker::ChunkProfile) -> Self {
        Self {
            profile,
            chunks: Vec::new(),
            por_tree: PorMerkleTree::empty(),
            payload_digest: blake3::hash(&[]),
            payload_len: 0,
            max_estimated_heap_bytes: DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
            #[cfg(feature = "manifest")]
            pdp_tree: None,
        }
    }

    /// Create a chunk store with an explicit per-ingest checked heap limit.
    pub fn with_profile_and_heap_limit(
        profile: sorafs_chunker::ChunkProfile,
        max_estimated_heap_bytes: usize,
    ) -> Result<Self, ChunkStoreError> {
        ensure_chunk_store_profile(profile)?;
        if max_estimated_heap_bytes == 0 {
            return Err(ChunkStoreError::InvalidEstimatedHeapLimit);
        }
        let mut store = Self::with_profile(profile);
        store.max_estimated_heap_bytes = max_estimated_heap_bytes;
        Ok(store)
    }

    /// Return the checked heap limit applied before every ingest operation.
    #[must_use]
    pub fn max_estimated_heap_bytes(&self) -> usize {
        self.max_estimated_heap_bytes
    }

    /// Update the checked heap limit used by subsequent ingest operations.
    pub fn set_max_estimated_heap_bytes(
        &mut self,
        max_estimated_heap_bytes: usize,
    ) -> Result<(), ChunkStoreError> {
        if max_estimated_heap_bytes == 0 {
            return Err(ChunkStoreError::InvalidEstimatedHeapLimit);
        }
        self.max_estimated_heap_bytes = max_estimated_heap_bytes;
        Ok(())
    }

    /// Returns the chunking profile used by this store.
    #[must_use]
    pub fn profile(&self) -> sorafs_chunker::ChunkProfile {
        self.profile
    }

    /// Returns the canonical payload digest (BLAKE3-256).
    #[must_use]
    pub fn payload_digest(&self) -> &Hash {
        &self.payload_digest
    }

    /// Returns total payload length (bytes) captured by the store.
    #[must_use]
    pub fn payload_len(&self) -> u64 {
        self.payload_len
    }

    /// Returns the stored chunk records.
    #[must_use]
    pub fn chunks(&self) -> &[StoredChunk] {
        &self.chunks
    }

    /// Returns the PoR sampling tree derived from the ingested payload.
    #[must_use]
    pub fn por_tree(&self) -> &PorMerkleTree {
        &self.por_tree
    }

    /// Move the PoR tree out of this store without cloning its retained metadata.
    ///
    /// The store retains its chunk inventory and payload digest; its PoR accessors expose an empty
    /// tree until another successful ingest rebuilds the tree.
    pub fn take_por_tree(&mut self) -> PorMerkleTree {
        std::mem::replace(&mut self.por_tree, PorMerkleTree::empty())
    }

    /// Returns the canonical PDP v1 tree, or `None` for an empty payload.
    #[cfg(feature = "manifest")]
    #[must_use]
    pub fn pdp_tree(&self) -> Option<&PdpMerkleTreeV1> {
        self.pdp_tree.as_ref()
    }

    /// Move the canonical PDP v1 tree out of this store without cloning its node slabs.
    ///
    /// The store retains its chunk and PoR metadata; subsequent PDP accessors return the
    /// empty-payload values until another successful ingest rebuilds the PDP tree.
    #[cfg(feature = "manifest")]
    pub fn take_pdp_tree(&mut self) -> Option<PdpMerkleTreeV1> {
        self.pdp_tree.take()
    }

    /// Returns the canonical PDP hot-leaf commitment root (4 KiB granularity).
    #[cfg(feature = "manifest")]
    #[must_use]
    pub fn pdp_hot_root(&self) -> Option<[u8; 32]> {
        self.pdp_tree.as_ref().map(PdpMerkleTreeV1::hot_root)
    }

    /// Returns the canonical PDP segment commitment root (256 KiB granularity).
    #[cfg(feature = "manifest")]
    #[must_use]
    pub fn pdp_segment_root(&self) -> Option<[u8; 32]> {
        self.pdp_tree.as_ref().map(PdpMerkleTreeV1::segment_root)
    }

    /// Returns the total number of canonical PDP hot leaves.
    #[cfg(feature = "manifest")]
    #[must_use]
    pub fn pdp_hot_leaf_count(&self) -> u64 {
        self.pdp_tree
            .as_ref()
            .map_or(0, PdpMerkleTreeV1::hot_leaf_count)
    }

    /// Returns the total number of canonical PDP segments.
    #[cfg(feature = "manifest")]
    #[must_use]
    pub fn pdp_segment_count(&self) -> u64 {
        self.pdp_tree
            .as_ref()
            .map_or(0, PdpMerkleTreeV1::segment_count)
    }

    /// Build canonical PDP witnesses using the supplied random-access payload source.
    #[cfg(feature = "manifest")]
    pub fn prove_pdp_samples_with<P: PayloadSource>(
        &self,
        samples: &[PdpSampleV1],
        source: &mut P,
    ) -> Result<Vec<PdpProofLeafV1>, PdpMerkleReadError<ChunkStoreError>> {
        let tree = self
            .pdp_tree
            .as_ref()
            .ok_or(PdpMerkleReadError::Tree(PdpMerkleTreeError::EmptyPayload))?;
        tree.prove_samples_with(samples, |offset, buffer| {
            source.read_exact(offset, buffer)?;
            Ok(buffer.len())
        })
    }

    /// Returns the total number of PoR leaves tracked by the current tree.
    #[must_use]
    pub fn por_leaf_count(&self) -> usize {
        self.por_tree.leaf_count()
    }

    /// Samples PoR leaves deterministically using `splitmix64` seeded with `seed`.
    pub fn sample_leaves(
        &self,
        count: usize,
        seed: u64,
        payload: &[u8],
    ) -> Result<Vec<(usize, PorProof)>, ChunkStoreError> {
        let mut source = InMemoryPayload::new(payload);
        self.sample_leaves_with(count, seed, &mut source)
    }

    pub fn sample_leaves_with<P: PayloadSource>(
        &self,
        count: usize,
        seed: u64,
        source: &mut P,
    ) -> Result<Vec<(usize, PorProof)>, ChunkStoreError> {
        let total = self.por_tree.try_leaf_count()?;
        if total == 0 || count == 0 {
            return Ok(Vec::new());
        }
        let target = count.min(total);
        let total_u64 = u64::try_from(total).map_err(|_| ChunkStoreError::PorCountOverflow {
            context: "PoR sampling population",
        })?;
        let estimated = self.por_tree.estimate_sample_heap(target)?;
        if estimated > self.max_estimated_heap_bytes {
            return Err(ChunkStoreError::EstimatedHeapLimitExceeded {
                estimated,
                limit: self.max_estimated_heap_bytes,
            });
        }
        let mut samples = Vec::new();
        try_reserve_store(&mut samples, target, "PoR samples")?;
        for flat_index in PorSampleIndices::new(total_u64, target, seed)? {
            let idx =
                usize::try_from(flat_index).map_err(|_| ChunkStoreError::PorCountOverflow {
                    context: "PoR sampled leaf index",
                })?;
            let (chunk_idx, segment_idx, leaf_idx) =
                self.por_tree
                    .leaf_path(idx)
                    .ok_or(ChunkStoreError::PorInvariant {
                        context: "sampled PoR leaf path",
                    })?;
            let proof = self
                .por_tree
                .prove_leaf_with_limit(
                    chunk_idx,
                    segment_idx,
                    leaf_idx,
                    source,
                    self.max_estimated_heap_bytes,
                )?
                .ok_or(ChunkStoreError::PorInvariant {
                    context: "sampled PoR proof path",
                })?;
            samples.push((idx, proof));
        }
        Ok(samples)
    }

    /// Clears the store and ingests the provided payload without panicking on invalid input,
    /// resource limits, or canonical PDP allocation failures.
    pub fn ingest_bytes(&mut self, payload: &[u8]) -> Result<(), ChunkStoreError> {
        self.try_ingest_bytes(payload)
    }

    /// Clears the store and ingests the provided payload, returning chunker
    /// validation errors instead of panicking on invalid profile parameters.
    pub fn try_ingest_bytes(&mut self, payload: &[u8]) -> Result<(), ChunkStoreError> {
        ensure_chunk_store_profile(self.profile)?;
        let estimated = estimate_direct_chunk_store_heap(payload.len(), self.profile)?;
        if estimated > self.max_estimated_heap_bytes {
            return Err(ChunkStoreError::EstimatedHeapLimitExceeded {
                estimated,
                limit: self.max_estimated_heap_bytes,
            });
        }
        let maximum_chunk_count = if payload.is_empty() {
            0
        } else {
            payload.len().div_ceil(self.profile.min_size)
        };
        let mut boundaries = Vec::new();
        boundaries.try_reserve(maximum_chunk_count).map_err(|_| {
            ChunkStoreError::AllocationFailed {
                context: "direct chunk boundaries",
                requested: maximum_chunk_count,
            }
        })?;
        if !payload.is_empty() {
            let mut chunker = sorafs_chunker::Chunker::try_with_profile(self.profile)?;
            let mut emitted_too_many = false;
            chunker.feed(payload, |chunk| {
                if boundaries.len() < maximum_chunk_count {
                    boundaries.push(chunk);
                } else {
                    emitted_too_many = true;
                }
            });
            chunker.finish(|chunk| {
                if boundaries.len() < maximum_chunk_count {
                    boundaries.push(chunk);
                } else {
                    emitted_too_many = true;
                }
            });
            if emitted_too_many {
                return Err(ChunkStoreError::InvalidPlan(
                    CarPlanValidationError::TooManyChunks {
                        count: maximum_chunk_count.saturating_add(1),
                        maximum: maximum_chunk_count,
                    },
                ));
            }
        }
        let mut chunks = Vec::new();
        chunks
            .try_reserve(boundaries.len())
            .map_err(|_| ChunkStoreError::AllocationFailed {
                context: "chunk metadata",
                requested: boundaries.len(),
            })?;
        for boundary in boundaries {
            let end =
                boundary
                    .checked_end()
                    .ok_or(sorafs_chunker::ChunkerError::ChunkRangeOverflow {
                        offset: boundary.offset,
                        length: boundary.length,
                    })?;
            if end > payload.len() {
                return Err(ChunkStoreError::Chunking(
                    sorafs_chunker::ChunkerError::ChunkRangeOverflow {
                        offset: boundary.offset,
                        length: boundary.length,
                    },
                ));
            }
            chunks.push(StoredChunk {
                offset: u64::try_from(boundary.offset).map_err(|_| {
                    ChunkStoreError::ChunkOffsetTooLarge {
                        offset: boundary.offset,
                    }
                })?,
                length: u32::try_from(boundary.length).map_err(|_| {
                    ChunkStoreError::ChunkLengthTooLarge {
                        length: boundary.length,
                        limit: CAR_CHUNK_LENGTH_LIMIT,
                    }
                })?,
                blake3: blake3::hash(&payload[boundary.offset..end]).into(),
            });
        }
        let payload_len =
            u64::try_from(payload.len()).map_err(|_| ChunkStoreError::PayloadLengthTooLarge)?;
        let por_tree = PorMerkleTree::try_from_payload(payload, &chunks)?;
        let payload_digest = blake3::hash(payload);
        #[cfg(feature = "manifest")]
        let pdp_tree = build_canonical_pdp_tree(payload)?;

        self.chunks = chunks;
        self.por_tree = por_tree;
        self.payload_digest = payload_digest;
        self.payload_len = payload_len;
        #[cfg(feature = "manifest")]
        {
            self.pdp_tree = pdp_tree;
        }
        Ok(())
    }

    /// Ingests a payload using chunk boundaries supplied by an existing plan.
    pub fn ingest_plan(
        &mut self,
        payload: &[u8],
        plan: &CarBuildPlan,
    ) -> Result<(), ChunkStoreError> {
        let mut source = InMemoryPayload::new(payload);
        self.ingest_plan_source(plan, &mut source)
    }

    /// Ingests chunk metadata by reading the payload stream according to `plan`.
    pub fn ingest_plan_stream<R: Read>(
        &mut self,
        plan: &CarBuildPlan,
        reader: &mut R,
    ) -> Result<(), ChunkStoreError> {
        let mut source = ReaderPayload::new(reader);
        self.ingest_plan_source(plan, &mut source)
    }

    /// Ingests chunk metadata using a random-access payload source.
    pub fn ingest_plan_source<P: PayloadSource>(
        &mut self,
        plan: &CarBuildPlan,
        source: &mut P,
    ) -> Result<(), ChunkStoreError> {
        self.ingest_plan_source_with_sink(plan, source, NoopSink)
            .map(|_| ())
    }

    /// Ingests chunk metadata using a random-access payload source and an output sink.
    pub fn ingest_plan_source_with_sink<P, S>(
        &mut self,
        plan: &CarBuildPlan,
        source: &mut P,
        mut sink: S,
    ) -> Result<S::Output, ChunkStoreError>
    where
        P: PayloadSource,
        S: ChunkSink,
    {
        let validation = preflight_chunk_store_plan(plan, self.max_estimated_heap_bytes)?;
        let max_chunk_len = validation.max_chunk_len();
        let chunk_count = plan.chunks.len();

        let mut chunks = Vec::new();
        chunks
            .try_reserve_exact(chunk_count)
            .map_err(|_| ChunkStoreError::AllocationFailed {
                context: "chunk metadata",
                requested: chunk_count,
            })?;
        let mut chunk_nodes = Vec::new();
        chunk_nodes.try_reserve_exact(chunk_count).map_err(|_| {
            ChunkStoreError::AllocationFailed {
                context: "PoR chunk nodes",
                requested: chunk_count,
            }
        })?;
        let mut chunk_roots = Vec::new();
        chunk_roots.try_reserve_exact(chunk_count).map_err(|_| {
            ChunkStoreError::AllocationFailed {
                context: "PoR chunk roots",
                requested: chunk_count,
            }
        })?;
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(max_chunk_len)
            .map_err(|_| ChunkStoreError::AllocationFailed {
                context: "chunk read buffer",
                requested: max_chunk_len,
            })?;

        sink.prepare(plan)?;

        let mut payload_hasher = blake3::Hasher::new();
        #[cfg(feature = "manifest")]
        let mut pdp_builder = (plan.content_length != 0).then(PdpMerkleTreeBuilderV1::new);

        let mut next_por_leaf_index = 0u64;
        for (idx, chunk_plan) in plan.chunks.iter().enumerate() {
            let expected_len = chunk_plan.length as usize;
            buffer.resize(expected_len, 0);
            match source.read_exact(chunk_plan.offset, &mut buffer) {
                Ok(()) => {}
                Err(ChunkStoreError::Io(err)) => {
                    if err.kind() == io::ErrorKind::UnexpectedEof {
                        return Err(ChunkStoreError::UnexpectedEof {
                            chunk_index: idx,
                            expected: chunk_plan.length,
                        });
                    } else {
                        return Err(ChunkStoreError::Io(err));
                    }
                }
                Err(other) => return Err(other),
            }

            let digest = blake3::hash(&buffer);
            if digest.as_bytes() != &chunk_plan.digest {
                return Err(ChunkStoreError::DigestMismatch { chunk_index: idx });
            }
            payload_hasher.update(&buffer);
            #[cfg(feature = "manifest")]
            if let Some(builder) = pdp_builder.as_mut() {
                builder.update(&buffer)?;
            }

            chunks.push(StoredChunk {
                offset: chunk_plan.offset,
                length: chunk_plan.length,
                blake3: chunk_plan.digest,
            });

            let (chunk_tree, chunk_root, next_leaf_index) =
                PorMerkleTree::build_chunk_tree_from_bytes(
                    idx,
                    chunk_plan.offset,
                    chunk_plan.length,
                    chunk_plan.digest,
                    next_por_leaf_index,
                    &buffer,
                )?;
            chunk_roots.push(chunk_root);
            chunk_nodes.push(chunk_tree);
            next_por_leaf_index = next_leaf_index;

            sink.write_chunk(idx, chunk_plan, &buffer)?;
        }

        source.ensure_exhausted(plan.content_length)?;

        let payload_digest = payload_hasher.finalize();
        if payload_digest != plan.payload_digest {
            return Err(ChunkStoreError::PayloadDigestMismatch);
        }

        let por_tree = if plan.content_length == 0 {
            PorMerkleTree::empty()
        } else {
            PorMerkleTree::try_from_chunks(chunk_nodes, chunk_roots, plan.content_length)?
        };
        #[cfg(feature = "manifest")]
        let pdp_tree = pdp_builder
            .map(PdpMerkleTreeBuilderV1::finish)
            .transpose()?;
        let output = sink.finish()?;

        self.profile = plan.chunk_profile;
        self.chunks = chunks;
        self.por_tree = por_tree;
        self.payload_digest = payload_digest;
        self.payload_len = plan.content_length;
        #[cfg(feature = "manifest")]
        {
            self.pdp_tree = pdp_tree;
        }
        Ok(output)
    }

    /// Ingests chunk metadata while persisting chunk bytes to `directory`.
    pub fn ingest_plan_to_directory<P: PayloadSource>(
        &mut self,
        plan: &CarBuildPlan,
        source: &mut P,
        directory: &Path,
    ) -> Result<DirectoryChunkSinkOutput, ChunkStoreError> {
        let sink = DirectoryChunkSink::new(directory)
            .with_max_estimated_heap_bytes(self.max_estimated_heap_bytes)?;
        self.ingest_plan_source_with_sink(plan, source, sink)
    }

    /// Streams chunk metadata from `reader`, persisting chunk bytes to `directory`.
    pub fn ingest_plan_stream_to_directory<R: Read>(
        &mut self,
        plan: &CarBuildPlan,
        reader: &mut R,
        directory: &Path,
    ) -> Result<DirectoryChunkSinkOutput, ChunkStoreError> {
        let mut source = ReaderPayload::new(reader);
        let sink = DirectoryChunkSink::new(directory)
            .with_max_estimated_heap_bytes(self.max_estimated_heap_bytes)?;
        self.ingest_plan_source_with_sink(plan, &mut source, sink)
    }
}

impl Default for ChunkStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata about an ingested chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredChunk {
    pub offset: u64,
    pub length: u32,
    pub blake3: [u8; 32],
}

/// Size of a PoR sampling segment (bytes).
pub const POR_SEGMENT_SIZE: usize = 64 * 1024;
/// Size of a PoR sampling leaf (bytes).
pub const POR_LEAF_SIZE: usize = 4 * 1024;

const POR_LEAF_DOMAIN: &[u8] = b"sorafs:por:leaf:v1";
const POR_SEGMENT_DOMAIN: &[u8] = b"sorafs:por:segment:v1";
const POR_CHUNK_DOMAIN: &[u8] = b"sorafs:por:chunk:v1";
const POR_CHUNK_NODE_DOMAIN: &[u8] = b"sorafs:por:chunk-node:v1";
const POR_ROOT_DOMAIN: &[u8] = b"sorafs:por:root-merkle:v1";

fn try_reserve_store<T>(
    values: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), ChunkStoreError> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| ChunkStoreError::AllocationFailed {
            context,
            requested: additional,
        })
}

fn checked_por_heap_add_product(
    total: &mut usize,
    count: usize,
    item_size: usize,
    context: &'static str,
) -> Result<(), ChunkStoreError> {
    let bytes = count
        .checked_mul(item_size)
        .ok_or(ChunkStoreError::PorCountOverflow { context })?;
    *total = total
        .checked_add(bytes)
        .ok_or(ChunkStoreError::PorCountOverflow { context })?;
    if *total > isize::MAX as usize {
        return Err(ChunkStoreError::PorCountOverflow { context });
    }
    Ok(())
}

fn ensure_por_chunk_count(chunk_count: usize) -> Result<(), ChunkStoreError> {
    if chunk_count > CAR_PLAN_MAX_CHUNKS {
        return Err(ChunkStoreError::InvalidPlan(
            CarPlanValidationError::TooManyChunks {
                count: chunk_count,
                maximum: CAR_PLAN_MAX_CHUNKS,
            },
        ));
    }
    Ok(())
}

/// Two-level Merkle tree used for Proof-of-Retrievability sampling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorMerkleTree {
    root: [u8; 32],
    chunks: Vec<PorChunkTree>,
    chunk_merkle_levels: Vec<Vec<[u8; 32]>>,
    payload_len: u64,
    leaf_count: u64,
}

impl PorMerkleTree {
    /// Returns an empty PoR tree.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            root: [0u8; 32],
            chunks: Vec::new(),
            chunk_merkle_levels: Vec::new(),
            payload_len: 0,
            leaf_count: 0,
        }
    }

    /// Builds and validates a PoR tree from the provided payload and chunk metadata.
    pub fn try_from_payload(
        payload: &[u8],
        chunks: &[StoredChunk],
    ) -> Result<Self, ChunkStoreError> {
        let payload_len =
            u64::try_from(payload.len()).map_err(|_| ChunkStoreError::PayloadLengthTooLarge)?;
        if payload.is_empty() {
            return if chunks.is_empty() {
                Ok(Self::empty())
            } else {
                Err(ChunkStoreError::PorInvariant {
                    context: "empty payload chunk inventory",
                })
            };
        }
        if chunks.is_empty() {
            return Err(ChunkStoreError::PorInvariant {
                context: "non-empty payload chunk inventory",
            });
        }
        ensure_por_chunk_count(chunks.len())?;

        let mut chunk_nodes = Vec::new();
        try_reserve_store(&mut chunk_nodes, chunks.len(), "PoR chunk nodes")?;
        let mut chunk_roots = Vec::new();
        try_reserve_store(&mut chunk_roots, chunks.len(), "PoR chunk roots")?;
        let mut expected_offset = 0u64;
        let mut next_leaf_index = 0u64;
        for (index, chunk) in chunks.iter().enumerate() {
            if chunk.length > CHUNK_STORE_MAX_CHUNK_BYTES {
                return Err(ChunkStoreError::ChunkLengthTooLarge {
                    length: chunk.length as usize,
                    limit: CHUNK_STORE_MAX_CHUNK_BYTES,
                });
            }
            if chunk.length == 0 || chunk.offset != expected_offset {
                return Err(ChunkStoreError::PorInvariant {
                    context: "payload chunk geometry",
                });
            }
            let chunk_end = chunk.offset.checked_add(u64::from(chunk.length)).ok_or(
                ChunkStoreError::PorInvariant {
                    context: "payload chunk range",
                },
            )?;
            if chunk_end > payload_len {
                return Err(ChunkStoreError::PorInvariant {
                    context: "payload chunk coverage",
                });
            }
            let chunk_start =
                usize::try_from(chunk.offset).map_err(|_| ChunkStoreError::PorInvariant {
                    context: "payload chunk offset host width",
                })?;
            let chunk_end =
                usize::try_from(chunk_end).map_err(|_| ChunkStoreError::PorInvariant {
                    context: "payload chunk end host width",
                })?;
            let bytes =
                payload
                    .get(chunk_start..chunk_end)
                    .ok_or(ChunkStoreError::PorInvariant {
                        context: "payload chunk slice",
                    })?;
            if blake3::hash(bytes).as_bytes() != &chunk.blake3 {
                return Err(ChunkStoreError::DigestMismatch { chunk_index: index });
            }
            let (chunk_tree, chunk_root, next_index) = Self::build_chunk_tree_from_bytes(
                index,
                chunk.offset,
                chunk.length,
                chunk.blake3,
                next_leaf_index,
                bytes,
            )?;
            chunk_roots.push(chunk_root);
            chunk_nodes.push(chunk_tree);
            next_leaf_index = next_index;
            expected_offset =
                u64::try_from(chunk_end).map_err(|_| ChunkStoreError::PorInvariant {
                    context: "payload chunk end conversion",
                })?;
        }
        if expected_offset != payload_len {
            return Err(ChunkStoreError::PorInvariant {
                context: "payload chunk final coverage",
            });
        }

        Self::try_from_chunks(chunk_nodes, chunk_roots, payload_len)
    }

    /// Builds a PoR tree from precomputed chunk subtrees and validates all canonical geometry and
    /// commitment relationships before publishing it.
    pub fn try_from_chunks(
        chunks: Vec<PorChunkTree>,
        chunk_roots: Vec<[u8; 32]>,
        payload_len: u64,
    ) -> Result<Self, ChunkStoreError> {
        if chunks.is_empty() || chunk_roots.is_empty() {
            return if chunks.is_empty() && chunk_roots.is_empty() && payload_len == 0 {
                Ok(Self::empty())
            } else {
                Err(ChunkStoreError::PorInvariant {
                    context: "empty PoR tree geometry",
                })
            };
        }
        if payload_len == 0 || chunks.len() != chunk_roots.len() {
            return Err(ChunkStoreError::PorInvariant {
                context: "PoR chunk root inventory",
            });
        }
        ensure_por_chunk_count(chunks.len())?;

        let mut expected_chunk_offset = 0u64;
        let mut total_segments = 0usize;
        let mut total_leaves = 0u64;
        let mut expected_leaf_index = 0u64;
        for (chunk_index, chunk) in chunks.iter().enumerate() {
            if chunk.chunk_index != chunk_index
                || chunk.length == 0
                || chunk.length > CHUNK_STORE_MAX_CHUNK_BYTES
                || chunk.offset != expected_chunk_offset
                || chunk.root != chunk_roots[chunk_index]
                || chunk.segments.is_empty()
            {
                return Err(ChunkStoreError::PorInvariant {
                    context: "PoR chunk metadata",
                });
            }
            let chunk_end = chunk.offset.checked_add(u64::from(chunk.length)).ok_or(
                ChunkStoreError::PorInvariant {
                    context: "PoR chunk range",
                },
            )?;
            if chunk_end > payload_len {
                return Err(ChunkStoreError::PorInvariant {
                    context: "PoR chunk payload coverage",
                });
            }

            total_segments = total_segments.checked_add(chunk.segments.len()).ok_or(
                ChunkStoreError::PorCountOverflow {
                    context: "PoR segment count",
                },
            )?;
            let mut expected_segment_offset = chunk.offset;
            let mut remaining_chunk =
                usize::try_from(chunk.length).map_err(|_| ChunkStoreError::PorInvariant {
                    context: "PoR chunk length host width",
                })?;
            for segment in &chunk.segments {
                let expected_segment_len = remaining_chunk.min(POR_SEGMENT_SIZE);
                let expected_segment_len_u32 =
                    u32::try_from(expected_segment_len).map_err(|_| {
                        ChunkStoreError::PorInvariant {
                            context: "PoR segment length width",
                        }
                    })?;
                if expected_segment_len == 0
                    || segment.offset != expected_segment_offset
                    || segment.length != expected_segment_len_u32
                    || segment.leaves.is_empty()
                {
                    return Err(ChunkStoreError::PorInvariant {
                        context: "PoR segment geometry",
                    });
                }
                let segment_leaf_count = u64::try_from(segment.leaves.len()).map_err(|_| {
                    ChunkStoreError::PorCountOverflow {
                        context: "PoR segment leaf count",
                    }
                })?;
                total_leaves = total_leaves.checked_add(segment_leaf_count).ok_or(
                    ChunkStoreError::PorCountOverflow {
                        context: "PoR leaf count",
                    },
                )?;

                let mut expected_leaf_offset = segment.offset;
                let mut remaining_segment = expected_segment_len;
                for leaf in &segment.leaves {
                    let expected_leaf_len = remaining_segment.min(POR_LEAF_SIZE);
                    let expected_leaf_len_u32 = u32::try_from(expected_leaf_len).map_err(|_| {
                        ChunkStoreError::PorInvariant {
                            context: "PoR leaf length width",
                        }
                    })?;
                    if expected_leaf_len == 0
                        || leaf.flat_index != expected_leaf_index
                        || leaf.offset != expected_leaf_offset
                        || leaf.length != expected_leaf_len_u32
                    {
                        return Err(ChunkStoreError::PorInvariant {
                            context: "PoR leaf geometry",
                        });
                    }
                    expected_leaf_offset = expected_leaf_offset
                        .checked_add(u64::from(leaf.length))
                        .ok_or(ChunkStoreError::PorInvariant {
                            context: "PoR leaf range",
                        })?;
                    expected_leaf_index = expected_leaf_index.checked_add(1).ok_or(
                        ChunkStoreError::PorCountOverflow {
                            context: "PoR canonical flat leaf index",
                        },
                    )?;
                    remaining_segment -= expected_leaf_len;
                }
                if remaining_segment != 0
                    || expected_leaf_offset
                        != segment
                            .offset
                            .checked_add(u64::from(segment.length))
                            .ok_or(ChunkStoreError::PorInvariant {
                                context: "PoR segment end",
                            })?
                {
                    return Err(ChunkStoreError::PorInvariant {
                        context: "PoR segment leaf coverage",
                    });
                }
                let recomputed_segment = hash_segment_from_entries(segment)?;
                if recomputed_segment != segment.digest {
                    return Err(ChunkStoreError::PorInvariant {
                        context: "PoR segment commitment",
                    });
                }
                expected_segment_offset = expected_segment_offset
                    .checked_add(u64::from(segment.length))
                    .ok_or(ChunkStoreError::PorInvariant {
                        context: "PoR segment range",
                    })?;
                remaining_chunk -= expected_segment_len;
            }
            if remaining_chunk != 0 || expected_segment_offset != chunk_end {
                return Err(ChunkStoreError::PorInvariant {
                    context: "PoR chunk segment coverage",
                });
            }
            let chunk_index_u64 =
                u64::try_from(chunk_index).map_err(|_| ChunkStoreError::PorInvariant {
                    context: "PoR chunk index width",
                })?;
            if hash_chunk_from_entries(chunk_index_u64, chunk)? != chunk.root {
                return Err(ChunkStoreError::PorInvariant {
                    context: "PoR chunk commitment",
                });
            }
            expected_chunk_offset = chunk_end;
        }
        if expected_chunk_offset != payload_len {
            return Err(ChunkStoreError::PorInvariant {
                context: "PoR final payload coverage",
            });
        }
        // Keep the checked accumulations above even though the values are not otherwise required:
        // they prove the infallible compatibility count accessors cannot overflow.
        let _ = total_segments;
        let chunk_merkle_levels = build_chunk_merkle_levels(&chunk_roots)?;
        let chunk_tree_root = if chunk_roots.len() == 1 {
            chunk_roots[0]
        } else {
            chunk_merkle_levels
                .last()
                .and_then(|level| level.first())
                .copied()
                .ok_or(ChunkStoreError::PorInvariant {
                    context: "PoR chunk Merkle root",
                })?
        };
        let root = hash_root(
            payload_len,
            chunk_roots.len(),
            total_leaves,
            &chunk_tree_root,
        )?;
        Ok(Self {
            root,
            chunks,
            chunk_merkle_levels,
            payload_len,
            leaf_count: total_leaves,
        })
    }

    /// Returns the root digest of the PoR tree.
    #[must_use]
    pub fn root(&self) -> &[u8; 32] {
        &self.root
    }

    /// Returns the chunk-level PoR subtrees.
    #[must_use]
    pub fn chunks(&self) -> &[PorChunkTree] {
        &self.chunks
    }

    /// Returns the total payload length represented by this tree.
    #[must_use]
    pub fn payload_len(&self) -> u64 {
        self.payload_len
    }

    /// Returns the authenticated total number of PoR leaves represented by this tree.
    #[must_use]
    pub fn leaf_count_u64(&self) -> u64 {
        self.leaf_count
    }

    /// Returns true if the tree is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Returns the total number of PoR leaves tracked by this tree, rejecting host-width overflow.
    pub fn try_leaf_count(&self) -> Result<usize, ChunkStoreError> {
        usize::try_from(self.leaf_count).map_err(|_| ChunkStoreError::PorCountOverflow {
            context: "PoR leaf count host width",
        })
    }

    /// Returns the total number of PoR leaves tracked by this validated tree.
    ///
    /// Validated constructors prove this cannot overflow. `usize::MAX` is returned defensively if
    /// a future deserialization path bypasses those constructors.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        match self.try_leaf_count() {
            Ok(count) => count,
            Err(_) => usize::MAX,
        }
    }

    /// Returns the total number of segments tracked by this tree, rejecting host-width overflow.
    pub fn try_segment_count(&self) -> Result<usize, ChunkStoreError> {
        self.chunks.iter().try_fold(0usize, |total, chunk| {
            total
                .checked_add(chunk.segments.len())
                .ok_or(ChunkStoreError::PorCountOverflow {
                    context: "PoR segment count",
                })
        })
    }

    /// Returns the total number of segments tracked by this validated tree.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        match self.try_segment_count() {
            Ok(count) => count,
            Err(_) => usize::MAX,
        }
    }

    /// Returns the `(chunk, segment, leaf)` tuple for the provided flattened leaf index.
    #[must_use]
    pub fn leaf_path(&self, mut leaf_index: usize) -> Option<(usize, usize, usize)> {
        for (chunk_idx, chunk) in self.chunks.iter().enumerate() {
            for (segment_idx, segment) in chunk.segments.iter().enumerate() {
                let leaf_len = segment.leaves.len();
                if leaf_index < leaf_len {
                    return Some((chunk_idx, segment_idx, leaf_index));
                }
                leaf_index -= leaf_len;
            }
        }
        None
    }

    /// Constructs a PoR proof for the specified chunk/segment/leaf tuple.
    pub fn try_prove_leaf(
        &self,
        chunk_index: usize,
        segment_index: usize,
        leaf_index: usize,
        payload: &[u8],
    ) -> Result<Option<PorProof>, ChunkStoreError> {
        let mut source = InMemoryPayload::new(payload);
        self.prove_leaf_with(chunk_index, segment_index, leaf_index, &mut source)
    }

    /// Alias for [`Self::try_prove_leaf`] retained while callers migrate to explicit `try_`
    /// proof construction.
    pub fn prove_leaf(
        &self,
        chunk_index: usize,
        segment_index: usize,
        leaf_index: usize,
        payload: &[u8],
    ) -> Result<Option<PorProof>, ChunkStoreError> {
        self.try_prove_leaf(chunk_index, segment_index, leaf_index, payload)
    }

    pub fn prove_leaf_with<P: PayloadSource>(
        &self,
        chunk_index: usize,
        segment_index: usize,
        leaf_index: usize,
        source: &mut P,
    ) -> Result<Option<PorProof>, ChunkStoreError> {
        self.prove_leaf_with_limit(
            chunk_index,
            segment_index,
            leaf_index,
            source,
            DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
        )
    }

    fn prove_leaf_with_limit<P: PayloadSource>(
        &self,
        chunk_index: usize,
        segment_index: usize,
        leaf_index: usize,
        source: &mut P,
        max_estimated_heap_bytes: usize,
    ) -> Result<Option<PorProof>, ChunkStoreError> {
        let chunk = match self.chunks.get(chunk_index) {
            Some(chunk) => chunk,
            None => return Ok(None),
        };
        let segment = match chunk.segments.get(segment_index) {
            Some(segment) => segment,
            None => return Ok(None),
        };
        let leaf = match segment.leaves.get(leaf_index) {
            Some(leaf) => leaf,
            None => return Ok(None),
        };
        let estimated = self.estimate_proof_heap(chunk, segment, leaf)?;
        if estimated > max_estimated_heap_bytes {
            return Err(ChunkStoreError::EstimatedHeapLimitExceeded {
                estimated,
                limit: max_estimated_heap_bytes,
            });
        }
        let leaf_len = usize::try_from(leaf.length).map_err(|_| ChunkStoreError::PorInvariant {
            context: "PoR proof leaf length host width",
        })?;
        let mut leaf_bytes = Vec::new();
        try_reserve_store(&mut leaf_bytes, leaf_len, "PoR proof leaf bytes")?;
        leaf_bytes.resize(leaf_len, 0);
        source.read_exact(leaf.offset, &mut leaf_bytes)?;
        if hash_leaf(leaf.flat_index, leaf.offset, &leaf_bytes) != leaf.digest {
            return Err(ChunkStoreError::PorProofLeafDigestMismatch {
                chunk_index,
                segment_index,
                leaf_index,
            });
        }

        let mut segment_leaves = Vec::new();
        try_reserve_store(
            &mut segment_leaves,
            segment.leaves.len(),
            "PoR proof segment leaves",
        )?;
        for entry in &segment.leaves {
            segment_leaves.push(entry.digest);
        }
        let mut chunk_segments = Vec::new();
        try_reserve_store(
            &mut chunk_segments,
            chunk.segments.len(),
            "PoR proof chunk segments",
        )?;
        for entry in &chunk.segments {
            chunk_segments.push(entry.digest);
        }
        let chunk_merkle_path = self.chunk_merkle_path(chunk_index)?;

        Ok(Some(PorProof {
            payload_len: self.payload_len,
            chunk_count: u64::try_from(self.chunks.len()).map_err(|_| {
                ChunkStoreError::PorCountOverflow {
                    context: "PoR proof chunk count",
                }
            })?,
            leaf_count: self.leaf_count,
            leaf_index_flat: leaf.flat_index,
            chunk_index,
            chunk_offset: chunk.offset,
            chunk_length: chunk.length,
            chunk_digest: chunk.chunk_digest,
            chunk_root: chunk.root,
            segment_index,
            segment_offset: segment.offset,
            segment_length: segment.length,
            segment_digest: segment.digest,
            leaf_index,
            leaf_offset: leaf.offset,
            leaf_length: leaf.length,
            leaf_bytes,
            leaf_digest: leaf.digest,
            segment_leaves,
            chunk_segments,
            chunk_merkle_path,
        }))
    }

    fn chunk_merkle_path(&self, chunk_index: usize) -> Result<Vec<[u8; 32]>, ChunkStoreError> {
        let depth = chunk_merkle_depth(self.chunks.len());
        let mut path = Vec::new();
        try_reserve_store(&mut path, depth, "PoR proof chunk Merkle path")?;
        let mut index = chunk_index;
        let mut width = self.chunks.len();
        for level in 0..depth {
            let sibling_index = if index.is_multiple_of(2) {
                index.checked_add(1).filter(|sibling| *sibling < width)
            } else {
                Some(index - 1)
            }
            .unwrap_or(index);
            let sibling = if level == 0 {
                self.chunks.get(sibling_index).map(|chunk| chunk.root)
            } else {
                self.chunk_merkle_levels
                    .get(level - 1)
                    .and_then(|nodes| nodes.get(sibling_index))
                    .copied()
            }
            .ok_or(ChunkStoreError::PorInvariant {
                context: "PoR chunk Merkle path",
            })?;
            path.push(sibling);
            index /= 2;
            width = width.div_ceil(2);
        }
        Ok(path)
    }

    fn estimate_proof_heap(
        &self,
        chunk: &PorChunkTree,
        segment: &PorSegment,
        leaf: &PorLeaf,
    ) -> Result<usize, ChunkStoreError> {
        let leaf_bytes =
            usize::try_from(leaf.length).map_err(|_| ChunkStoreError::PorCountOverflow {
                context: "PoR proof leaf bytes",
            })?;
        let mut estimated = std::mem::size_of::<PorProof>()
            .checked_add(leaf_bytes)
            .ok_or(ChunkStoreError::PorCountOverflow {
                context: "PoR proof heap",
            })?;
        checked_por_heap_add_product(
            &mut estimated,
            segment.leaves.len(),
            std::mem::size_of::<[u8; 32]>(),
            "PoR proof segment leaves",
        )?;
        checked_por_heap_add_product(
            &mut estimated,
            chunk.segments.len(),
            std::mem::size_of::<[u8; 32]>(),
            "PoR proof chunk segments",
        )?;
        checked_por_heap_add_product(
            &mut estimated,
            chunk_merkle_depth(self.chunks.len()),
            std::mem::size_of::<[u8; 32]>(),
            "PoR proof chunk Merkle path",
        )?;
        Ok(estimated)
    }

    fn estimate_sample_heap(&self, target: usize) -> Result<usize, ChunkStoreError> {
        if target == 0 {
            return Ok(0);
        }
        let mut maximum_proof = 0usize;
        for chunk in &self.chunks {
            for segment in &chunk.segments {
                for leaf in &segment.leaves {
                    maximum_proof =
                        maximum_proof.max(self.estimate_proof_heap(chunk, segment, leaf)?);
                }
            }
        }
        let mut estimated = 0usize;
        checked_por_heap_add_product(
            &mut estimated,
            target,
            maximum_proof
                .checked_add(std::mem::size_of::<usize>())
                .ok_or(ChunkStoreError::PorCountOverflow {
                    context: "PoR sample proof heap",
                })?,
            "PoR sampled proofs",
        )?;
        // HashSet uses buckets and control bytes in addition to each key. Four words per sample is
        // a conservative, allocator-independent upper estimate for this bounded uniqueness set.
        checked_por_heap_add_product(
            &mut estimated,
            target,
            std::mem::size_of::<usize>() * 4,
            "PoR sample uniqueness set",
        )?;
        Ok(estimated)
    }

    fn build_chunk_tree_from_bytes(
        chunk_index: usize,
        chunk_offset: u64,
        chunk_length: u32,
        chunk_digest: [u8; 32],
        first_leaf_index: u64,
        bytes: &[u8],
    ) -> Result<(PorChunkTree, [u8; 32], u64), ChunkStoreError> {
        let expected_len =
            usize::try_from(chunk_length).map_err(|_| ChunkStoreError::PorInvariant {
                context: "PoR chunk length host width",
            })?;
        if bytes.len() != expected_len || bytes.is_empty() {
            return Err(ChunkStoreError::PorInvariant {
                context: "PoR chunk byte coverage",
            });
        }

        let segment_count = bytes.len().div_ceil(POR_SEGMENT_SIZE);
        let mut segments = Vec::new();
        try_reserve_store(&mut segments, segment_count, "PoR chunk segments")?;
        let mut segment_hashes = Vec::new();
        try_reserve_store(
            &mut segment_hashes,
            segment_count,
            "PoR chunk segment roots",
        )?;
        let mut segment_start = 0usize;
        let mut next_leaf_index = first_leaf_index;
        while segment_start < bytes.len() {
            let segment_len = (bytes.len() - segment_start).min(POR_SEGMENT_SIZE);
            let segment_end =
                segment_start
                    .checked_add(segment_len)
                    .ok_or(ChunkStoreError::PorInvariant {
                        context: "PoR segment byte range",
                    })?;
            let leaf_count = segment_len.div_ceil(POR_LEAF_SIZE);
            let mut leaves = Vec::new();
            try_reserve_store(&mut leaves, leaf_count, "PoR segment leaves")?;
            let mut leaf_hashes = Vec::new();
            try_reserve_store(&mut leaf_hashes, leaf_count, "PoR segment leaf roots")?;
            let mut leaf_start = segment_start;
            while leaf_start < segment_end {
                let leaf_len = (segment_end - leaf_start).min(POR_LEAF_SIZE);
                let leaf_end =
                    leaf_start
                        .checked_add(leaf_len)
                        .ok_or(ChunkStoreError::PorInvariant {
                            context: "PoR leaf byte range",
                        })?;
                let relative_offset =
                    u64::try_from(leaf_start).map_err(|_| ChunkStoreError::PorInvariant {
                        context: "PoR leaf relative offset width",
                    })?;
                let absolute_offset = chunk_offset.checked_add(relative_offset).ok_or(
                    ChunkStoreError::PorInvariant {
                        context: "PoR leaf absolute offset",
                    },
                )?;
                let digest = hash_leaf(
                    next_leaf_index,
                    absolute_offset,
                    &bytes[leaf_start..leaf_end],
                );
                leaves.push(PorLeaf {
                    flat_index: next_leaf_index,
                    offset: absolute_offset,
                    length: u32::try_from(leaf_len).map_err(|_| ChunkStoreError::PorInvariant {
                        context: "PoR leaf length width",
                    })?,
                    digest,
                });
                next_leaf_index =
                    next_leaf_index
                        .checked_add(1)
                        .ok_or(ChunkStoreError::PorCountOverflow {
                            context: "PoR canonical flat leaf index",
                        })?;
                leaf_hashes.push(digest);
                leaf_start = leaf_end;
            }
            let segment_relative_offset =
                u64::try_from(segment_start).map_err(|_| ChunkStoreError::PorInvariant {
                    context: "PoR segment relative offset width",
                })?;
            let segment_offset = chunk_offset.checked_add(segment_relative_offset).ok_or(
                ChunkStoreError::PorInvariant {
                    context: "PoR segment absolute offset",
                },
            )?;
            let segment_length =
                u32::try_from(segment_len).map_err(|_| ChunkStoreError::PorInvariant {
                    context: "PoR segment length width",
                })?;
            let segment_digest = hash_segment(segment_offset, segment_length, &leaf_hashes);
            segments.push(PorSegment {
                offset: segment_offset,
                length: segment_length,
                digest: segment_digest,
                leaves,
            });
            segment_hashes.push(segment_digest);
            segment_start = segment_end;
        }
        let chunk_index_u64 =
            u64::try_from(chunk_index).map_err(|_| ChunkStoreError::PorInvariant {
                context: "PoR chunk index width",
            })?;
        let chunk_root = hash_chunk(
            chunk_index_u64,
            chunk_offset,
            chunk_length,
            &chunk_digest,
            &segment_hashes,
        );

        Ok((
            PorChunkTree {
                chunk_index,
                offset: chunk_offset,
                length: chunk_length,
                chunk_digest,
                root: chunk_root,
                segments,
            },
            chunk_root,
            next_leaf_index,
        ))
    }
}

/// PoR metadata for a single chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorChunkTree {
    pub chunk_index: usize,
    pub offset: u64,
    pub length: u32,
    pub chunk_digest: [u8; 32],
    pub root: [u8; 32],
    pub segments: Vec<PorSegment>,
}

/// PoR metadata for a sampling segment (64 KiB target).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorSegment {
    pub offset: u64,
    pub length: u32,
    pub digest: [u8; 32],
    pub leaves: Vec<PorLeaf>,
}

/// PoR metadata for a sampling leaf (4 KiB target).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorLeaf {
    /// Canonical flattened index committed into this leaf digest.
    pub flat_index: u64,
    pub offset: u64,
    pub length: u32,
    pub digest: [u8; 32],
}

/// Proof-of-Retrievability witness for a single leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorProof {
    pub payload_len: u64,
    pub chunk_count: u64,
    pub leaf_count: u64,
    pub leaf_index_flat: u64,
    pub chunk_index: usize,
    pub chunk_offset: u64,
    pub chunk_length: u32,
    pub chunk_digest: [u8; 32],
    pub chunk_root: [u8; 32],
    pub segment_index: usize,
    pub segment_offset: u64,
    pub segment_length: u32,
    pub segment_digest: [u8; 32],
    pub leaf_index: usize,
    pub leaf_offset: u64,
    pub leaf_length: u32,
    pub leaf_bytes: Vec<u8>,
    pub leaf_digest: [u8; 32],
    pub segment_leaves: Vec<[u8; 32]>,
    pub chunk_segments: Vec<[u8; 32]>,
    pub chunk_merkle_path: Vec<[u8; 32]>,
}

impl PorProof {
    fn reconstructed_root(&self) -> Option<[u8; 32]> {
        let chunk_count = usize::try_from(self.chunk_count).ok()?;
        if !(1..=CAR_PLAN_MAX_CHUNKS).contains(&chunk_count)
            || self.chunk_index >= chunk_count
            || self.chunk_merkle_path.len() != chunk_merkle_depth(chunk_count)
            || self.chunk_merkle_path.len() > POR_CHUNK_MERKLE_MAX_DEPTH
        {
            return None;
        }
        let mut chunk_tree_root = self.chunk_root;
        let mut chunk_tree_index = self.chunk_index;
        let mut chunk_tree_width = chunk_count;
        for (level, sibling) in self.chunk_merkle_path.iter().enumerate() {
            let is_unpaired_tail =
                chunk_tree_width % 2 == 1 && chunk_tree_index + 1 == chunk_tree_width;
            if is_unpaired_tail && sibling != &chunk_tree_root {
                return None;
            }
            let (left, right) = if chunk_tree_index.is_multiple_of(2) {
                (&chunk_tree_root, sibling)
            } else {
                (sibling, &chunk_tree_root)
            };
            chunk_tree_root = hash_chunk_node(
                u32::try_from(level).ok()?,
                u64::try_from(chunk_tree_index / 2).ok()?,
                left,
                right,
            );
            chunk_tree_index /= 2;
            chunk_tree_width = chunk_tree_width.div_ceil(2);
        }
        hash_root(
            self.payload_len,
            chunk_count,
            self.leaf_count,
            &chunk_tree_root,
        )
        .ok()
    }

    /// Checks that every digest and geometry claim inside this witness agrees with the others.
    ///
    /// This is not an authenticity check: the root is reconstructed from the untrusted witness
    /// itself. Call [`Self::verify`] with a trusted, externally authenticated PoR root before
    /// accepting the proof.
    #[must_use]
    pub fn is_internally_consistent(&self) -> bool {
        self.reconstructed_root()
            .is_some_and(|witness_root| self.verify(&witness_root))
    }

    /// Verifies the proof against the expected PoR root.
    #[must_use]
    pub fn verify(&self, expected_root: &[u8; 32]) -> bool {
        let segment_size = match u64::try_from(POR_SEGMENT_SIZE) {
            Ok(value) => value,
            Err(_) => return false,
        };
        let leaf_size = match u64::try_from(POR_LEAF_SIZE) {
            Ok(value) => value,
            Err(_) => return false,
        };
        let chunk_count = match usize::try_from(self.chunk_count) {
            Ok(count) if (1..=CAR_PLAN_MAX_CHUNKS).contains(&count) => count,
            _ => return false,
        };
        if self.chunk_index >= chunk_count
            || self.leaf_count == 0
            || self.leaf_index_flat >= self.leaf_count
            || self.chunk_merkle_path.len() != chunk_merkle_depth(chunk_count)
            || self.chunk_merkle_path.len() > POR_CHUNK_MERKLE_MAX_DEPTH
            || self.chunk_length == 0
            || self.chunk_length > CHUNK_STORE_MAX_CHUNK_BYTES
            || self.segment_length == 0
            || self.leaf_length == 0
        {
            return false;
        }
        let expected_segment_count = u64::from(self.chunk_length).div_ceil(segment_size);
        let expected_segment_count = match usize::try_from(expected_segment_count) {
            Ok(value) => value,
            Err(_) => return false,
        };
        if self.chunk_segments.len() != expected_segment_count
            || self.segment_index >= expected_segment_count
        {
            return false;
        }
        let segment_relative = match u64::try_from(self.segment_index)
            .ok()
            .and_then(|index| index.checked_mul(segment_size))
        {
            Some(value) => value,
            None => return false,
        };
        let expected_segment_offset = match self.chunk_offset.checked_add(segment_relative) {
            Some(value) => value,
            None => return false,
        };
        let remaining_chunk = match u64::from(self.chunk_length).checked_sub(segment_relative) {
            Some(value) => value,
            None => return false,
        };
        let expected_segment_length = remaining_chunk.min(segment_size);
        if self.segment_offset != expected_segment_offset
            || u64::from(self.segment_length) != expected_segment_length
        {
            return false;
        }
        let expected_leaf_count = u64::from(self.segment_length).div_ceil(leaf_size);
        let expected_leaf_count = match usize::try_from(expected_leaf_count) {
            Ok(value) => value,
            Err(_) => return false,
        };
        if self.segment_leaves.len() != expected_leaf_count
            || self.leaf_index >= expected_leaf_count
        {
            return false;
        }
        let leaf_relative = match u64::try_from(self.leaf_index)
            .ok()
            .and_then(|index| index.checked_mul(leaf_size))
        {
            Some(value) => value,
            None => return false,
        };
        let expected_leaf_offset = match self.segment_offset.checked_add(leaf_relative) {
            Some(value) => value,
            None => return false,
        };
        let remaining_segment = match u64::from(self.segment_length).checked_sub(leaf_relative) {
            Some(value) => value,
            None => return false,
        };
        let expected_leaf_length = remaining_segment.min(leaf_size);
        let leaf_bytes_len = match u64::try_from(self.leaf_bytes.len()) {
            Ok(value) => value,
            Err(_) => return false,
        };
        if self.leaf_offset != expected_leaf_offset
            || u64::from(self.leaf_length) != expected_leaf_length
            || leaf_bytes_len != expected_leaf_length
        {
            return false;
        }
        let chunk_end = match self.chunk_offset.checked_add(u64::from(self.chunk_length)) {
            Some(value) => value,
            None => return false,
        };
        if chunk_end > self.payload_len {
            return false;
        }
        if self.segment_leaves.get(self.leaf_index) != Some(&self.leaf_digest) {
            return false;
        }
        if self.chunk_segments.get(self.segment_index) != Some(&self.segment_digest) {
            return false;
        }
        let recomputed_leaf = hash_leaf(self.leaf_index_flat, self.leaf_offset, &self.leaf_bytes);
        if recomputed_leaf != self.leaf_digest {
            return false;
        }

        let recomputed_segment = hash_segment(
            self.segment_offset,
            self.segment_length,
            &self.segment_leaves,
        );
        if recomputed_segment != self.segment_digest {
            return false;
        }

        let chunk_index = match u64::try_from(self.chunk_index) {
            Ok(index) => index,
            Err(_) => return false,
        };
        let recomputed_chunk = hash_chunk(
            chunk_index,
            self.chunk_offset,
            self.chunk_length,
            &self.chunk_digest,
            &self.chunk_segments,
        );
        if recomputed_chunk != self.chunk_root {
            return false;
        }

        self.reconstructed_root()
            .is_some_and(|recomputed_root| &recomputed_root == expected_root)
    }
}

fn hash_leaf(flat_index: u64, offset: u64, bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(POR_LEAF_DOMAIN);
    hasher.update(&flat_index.to_le_bytes());
    hasher.update(&offset.to_le_bytes());
    hasher.update(&(bytes.len() as u32).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hash_segment(offset: u64, length: u32, leaves: &[[u8; 32]]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(POR_SEGMENT_DOMAIN);
    hasher.update(&offset.to_le_bytes());
    hasher.update(&length.to_le_bytes());
    hasher.update(&(leaves.len() as u64).to_le_bytes());
    for digest in leaves {
        hasher.update(digest);
    }
    hasher.finalize().into()
}

fn hash_segment_from_entries(segment: &PorSegment) -> Result<[u8; 32], ChunkStoreError> {
    let mut hasher = Hasher::new();
    hasher.update(POR_SEGMENT_DOMAIN);
    hasher.update(&segment.offset.to_le_bytes());
    hasher.update(&segment.length.to_le_bytes());
    let leaf_count =
        u64::try_from(segment.leaves.len()).map_err(|_| ChunkStoreError::PorCountOverflow {
            context: "PoR segment commitment leaf count",
        })?;
    hasher.update(&leaf_count.to_le_bytes());
    for leaf in &segment.leaves {
        hasher.update(&leaf.digest);
    }
    Ok(hasher.finalize().into())
}

fn hash_chunk(
    index: u64,
    offset: u64,
    length: u32,
    chunk_digest: &[u8; 32],
    segments: &[[u8; 32]],
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(POR_CHUNK_DOMAIN);
    hasher.update(&index.to_le_bytes());
    hasher.update(&offset.to_le_bytes());
    hasher.update(&length.to_le_bytes());
    hasher.update(chunk_digest);
    hasher.update(&(segments.len() as u64).to_le_bytes());
    for digest in segments {
        hasher.update(digest);
    }
    hasher.finalize().into()
}

fn hash_chunk_from_entries(index: u64, chunk: &PorChunkTree) -> Result<[u8; 32], ChunkStoreError> {
    let mut hasher = Hasher::new();
    hasher.update(POR_CHUNK_DOMAIN);
    hasher.update(&index.to_le_bytes());
    hasher.update(&chunk.offset.to_le_bytes());
    hasher.update(&chunk.length.to_le_bytes());
    hasher.update(&chunk.chunk_digest);
    let segment_count =
        u64::try_from(chunk.segments.len()).map_err(|_| ChunkStoreError::PorCountOverflow {
            context: "PoR chunk commitment segment count",
        })?;
    hasher.update(&segment_count.to_le_bytes());
    for segment in &chunk.segments {
        hasher.update(&segment.digest);
    }
    Ok(hasher.finalize().into())
}

fn chunk_merkle_depth(chunk_count: usize) -> usize {
    if chunk_count <= 1 {
        0
    } else {
        usize::BITS as usize - (chunk_count - 1).leading_zeros() as usize
    }
}

fn hash_chunk_node(level: u32, parent_index: u64, left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(POR_CHUNK_NODE_DOMAIN);
    hasher.update(&level.to_le_bytes());
    hasher.update(&parent_index.to_le_bytes());
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}

fn build_chunk_merkle_levels(
    chunk_roots: &[[u8; 32]],
) -> Result<Vec<Vec<[u8; 32]>>, ChunkStoreError> {
    if chunk_roots.is_empty() {
        return Ok(Vec::new());
    }
    if chunk_roots.len() > CAR_PLAN_MAX_CHUNKS {
        return Err(ChunkStoreError::PorInvariant {
            context: "PoR chunk Merkle count",
        });
    }
    let mut levels: Vec<Vec<[u8; 32]>> = Vec::new();
    let mut width = chunk_roots.len();
    let mut level = 0_u32;
    while width > 1 {
        let parent_count = width.div_ceil(2);
        let mut parents = Vec::new();
        try_reserve_store(&mut parents, parent_count, "PoR chunk Merkle level")?;
        for parent_index in 0..parent_count {
            let child_index = parent_index * 2;
            let (left, right) = if levels.is_empty() {
                let left = &chunk_roots[child_index];
                let right = chunk_roots.get(child_index + 1).unwrap_or(left);
                (left, right)
            } else {
                let children = levels.last().expect("non-empty chunk Merkle levels");
                let left = &children[child_index];
                let right = children.get(child_index + 1).unwrap_or(left);
                (left, right)
            };
            let parent_index =
                u64::try_from(parent_index).map_err(|_| ChunkStoreError::PorCountOverflow {
                    context: "PoR chunk Merkle parent index",
                })?;
            parents.push(hash_chunk_node(level, parent_index, left, right));
        }
        width = parents.len();
        levels.push(parents);
        level = level
            .checked_add(1)
            .ok_or(ChunkStoreError::PorCountOverflow {
                context: "PoR chunk Merkle level",
            })?;
    }
    if levels.len() > POR_CHUNK_MERKLE_MAX_DEPTH {
        return Err(ChunkStoreError::PorInvariant {
            context: "PoR chunk Merkle depth",
        });
    }
    Ok(levels)
}

fn hash_root(
    total_len: u64,
    chunk_count: usize,
    leaf_count: u64,
    chunk_tree_root: &[u8; 32],
) -> Result<[u8; 32], ChunkStoreError> {
    let minimum_leaf_count = total_len.div_ceil(u64::try_from(POR_LEAF_SIZE).map_err(|_| {
        ChunkStoreError::PorCountOverflow {
            context: "PoR leaf size",
        }
    })?);
    if total_len == 0
        || chunk_count == 0
        || chunk_count > CAR_PLAN_MAX_CHUNKS
        || leaf_count < minimum_leaf_count
        || leaf_count > total_len
    {
        return Err(ChunkStoreError::PorInvariant {
            context: "PoR root chunk count",
        });
    }
    let chunk_count =
        u64::try_from(chunk_count).map_err(|_| ChunkStoreError::PorCountOverflow {
            context: "PoR root chunk count",
        })?;
    if leaf_count < chunk_count {
        return Err(ChunkStoreError::PorInvariant {
            context: "PoR root leaf population",
        });
    }
    let mut hasher = Hasher::new();
    hasher.update(POR_ROOT_DOMAIN);
    hasher.update(&total_len.to_le_bytes());
    hasher.update(&chunk_count.to_le_bytes());
    hasher.update(&leaf_count.to_le_bytes());
    hasher.update(chunk_tree_root);
    Ok(hasher.finalize().into())
}

/// Deterministic, allocation-bounded iterator over unique PoR sample indices.
///
/// Each SplitMix64 candidate is reduced into the authenticated leaf population. Collisions use
/// deterministic linear probing, which guarantees progress and keeps producers and verifiers on
/// the same exact schedule without allocating in proportion to the full payload.
#[derive(Debug)]
pub struct PorSampleIndices {
    leaf_count: u64,
    target: usize,
    emitted: usize,
    rng_state: u64,
    seen: HashSet<u64>,
}

impl PorSampleIndices {
    /// Build a deterministic sample schedule bounded by `min(count, leaf_count)`.
    pub fn new(leaf_count: u64, count: usize, seed: u64) -> Result<Self, ChunkStoreError> {
        let target = usize::try_from(leaf_count.min(u64::try_from(count).map_err(|_| {
            ChunkStoreError::PorCountOverflow {
                context: "PoR requested sample count",
            }
        })?))
        .map_err(|_| ChunkStoreError::PorCountOverflow {
            context: "PoR sample schedule target",
        })?;
        let mut seen = HashSet::new();
        seen.try_reserve(target)
            .map_err(|_| ChunkStoreError::AllocationFailed {
                context: "PoR sample uniqueness set",
                requested: target,
            })?;
        Ok(Self {
            leaf_count,
            target,
            emitted: 0,
            rng_state: seed,
            seen,
        })
    }

    /// Return the exact number of indices this schedule will emit.
    #[must_use]
    pub fn sample_count(&self) -> usize {
        self.target
    }
}

impl Iterator for PorSampleIndices {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.emitted == self.target || self.leaf_count == 0 {
            return None;
        }
        self.rng_state = splitmix64(self.rng_state);
        let mut candidate = self.rng_state % self.leaf_count;
        while !self.seen.insert(candidate) {
            candidate = if candidate + 1 == self.leaf_count {
                0
            } else {
                candidate + 1
            };
        }
        self.emitted += 1;
        Some(candidate)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.target - self.emitted;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for PorSampleIndices {}

/// SplitMix64 round used by the canonical PoR sample schedule.
#[must_use]
pub fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

/// Summary produced by [`ingest_single_file`], combining chunk metadata and a CAR plan.
#[derive(Debug, Clone)]
pub struct IngestSummary {
    pub chunk_store: ChunkStore,
    pub plan: CarBuildPlan,
}

/// Failure while deriving and ingesting a single-file CAR plan.
#[derive(Debug, Error)]
pub enum IngestSingleFileError {
    /// CAR planning failed.
    #[error(transparent)]
    Plan(#[from] CarPlanError),
    /// Chunk-store ingestion failed.
    #[error(transparent)]
    Store(#[from] ChunkStoreError),
}

/// Ingests a single payload using the default registry profile and derives a CAR plan.
pub fn ingest_single_file(bytes: &[u8]) -> Result<IngestSummary, IngestSingleFileError> {
    let plan = CarBuildPlan::single_file(bytes)?;
    let mut chunk_store = ChunkStore::new();
    chunk_store.ingest_bytes(bytes)?;
    Ok(IngestSummary { chunk_store, plan })
}

/// CARv2 writer that produces spec-compliant archives.
pub struct CarWriter<'a> {
    plan: &'a CarBuildPlan,
    payload: &'a [u8],
    expected_roots: Option<Vec<Vec<u8>>>,
}

impl<'a> CarWriter<'a> {
    /// Creates a new writer for the provided plan and payload.
    pub fn new(plan: &'a CarBuildPlan, payload: &'a [u8]) -> Result<Self, CarWriteError> {
        plan.validate()?;
        let payload_len =
            u64::try_from(payload.len()).map_err(|_| CarWriteError::ArithmeticOverflow {
                context: "CAR payload length",
            })?;
        if plan.content_length != payload_len {
            return Err(CarWriteError::PayloadMismatch);
        }
        if blake3::hash(payload) != plan.payload_digest {
            return Err(CarWriteError::PayloadDigestMismatch);
        }
        Ok(Self {
            plan,
            payload,
            expected_roots: None,
        })
    }

    /// Sets an expected root list that must match the computed CAR roots.
    pub fn with_expected_roots(
        plan: &'a CarBuildPlan,
        payload: &'a [u8],
        roots: Vec<Vec<u8>>,
    ) -> Result<Self, CarWriteError> {
        let mut writer = Self::new(plan, payload)?;
        writer.expected_roots = Some(roots);
        Ok(writer)
    }

    /// Writes a CARv2 container (pragma + header + CARv1 payload + optional
    /// MultihashIndexSorted index) to the provided writer.
    pub fn write_to<W: Write>(&self, mut writer: W) -> Result<CarWriteStats, CarWriteError> {
        let layout = CarLayout::new(self.plan)?;
        if self
            .expected_roots
            .as_ref()
            .is_some_and(|expected| expected != &layout.root_cids)
        {
            return Err(CarWriteError::RootMismatch);
        }
        layout.write_car(
            self.plan,
            &mut writer,
            |chunk_index, chunk, writer, file_hasher, payload_hasher| {
                let start = chunk.offset as usize;
                let end = start
                    .checked_add(chunk.length as usize)
                    .ok_or(CarWriteError::ChunkOutOfBounds { chunk_index })?;
                if end > self.payload.len() {
                    return Err(CarWriteError::ChunkOutOfBounds { chunk_index });
                }
                let data = &self.payload[start..end];
                let digest = blake3::hash(data);
                if digest.as_bytes() != &chunk.digest {
                    return Err(CarWriteError::DigestMismatch { chunk_index });
                }
                write_buffer(writer, file_hasher, Some(payload_hasher), data)
            },
        )
    }

    /// Returns the plan used by this writer.
    #[must_use]
    pub fn plan(&self) -> &CarBuildPlan {
        self.plan
    }
}

/// Streaming CAR writer that reads chunk bytes from an arbitrary reader.
pub struct CarStreamingWriter<'a> {
    plan: &'a CarBuildPlan,
    expected_roots: Option<Vec<Vec<u8>>>,
}

impl<'a> CarStreamingWriter<'a> {
    /// Creates a new streaming writer for the provided plan.
    #[must_use]
    pub fn new(plan: &'a CarBuildPlan) -> Self {
        Self {
            plan,
            expected_roots: None,
        }
    }

    /// Same as [`Self::new`] but enforces an expected root list.
    #[must_use]
    pub fn with_expected_roots(plan: &'a CarBuildPlan, roots: Vec<Vec<u8>>) -> Self {
        Self {
            plan,
            expected_roots: Some(roots),
        }
    }

    /// Streams bytes from `reader`, emitting a CARv2 container to `writer`.
    pub fn write_from_reader<W: Write, R: Read>(
        &self,
        reader: &mut R,
        mut writer: W,
    ) -> Result<CarWriteStats, CarWriteError> {
        let validation = self.plan.validate()?;
        let layout = CarLayout::new(self.plan)?;
        if self
            .expected_roots
            .as_ref()
            .is_some_and(|expected| expected != &layout.root_cids)
        {
            return Err(CarWriteError::RootMismatch);
        }
        let mut buffer = Vec::<u8>::new();
        try_reserve_car(
            &mut buffer,
            validation.max_chunk_len(),
            "streaming CAR chunk buffer",
        )?;
        layout.write_car(
            self.plan,
            &mut writer,
            |chunk_index, chunk, writer, file_hasher, payload_hasher| {
                let length = chunk.length as usize;
                buffer.resize(length, 0);
                reader.read_exact(&mut buffer).map_err(CarWriteError::Io)?;
                let digest = blake3::hash(&buffer);
                if digest.as_bytes() != &chunk.digest {
                    return Err(CarWriteError::DigestMismatch { chunk_index });
                }
                write_buffer(writer, file_hasher, Some(payload_hasher), &buffer)
            },
        )
    }

    /// Returns the plan associated with this writer.
    #[must_use]
    pub fn plan(&self) -> &CarBuildPlan {
        self.plan
    }
}

/// Summary statistics returned by the writer once implemented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarWriteStats {
    pub payload_bytes: u64,
    pub chunk_count: usize,
    pub car_size: u64,
    /// BLAKE3-256 digest of the CARv1 payload section.
    pub car_payload_digest: Hash,
    /// BLAKE3-256 digest of the entire CARv2 file (pragma + header + payload + index).
    pub car_archive_digest: Hash,
    pub car_cid: Vec<u8>,
    pub root_cids: Vec<Vec<u8>>,
    pub dag_codec: u64,
    pub chunk_profile: ChunkProfile,
}

fn write_buffer<W: Write>(
    writer: &mut W,
    file_hasher: &mut blake3::Hasher,
    payload_hasher: Option<&mut blake3::Hasher>,
    buf: &[u8],
) -> Result<u64, CarWriteError> {
    writer.write_all(buf)?;
    file_hasher.update(buf);
    if let Some(hasher) = payload_hasher {
        hasher.update(buf);
    }
    car_usize_to_u64(buf.len(), "written CAR byte count")
}

const PRAGMA: [u8; 11] = [
    0x0a, 0xa1, 0x67, 0x76, 0x65, 0x72, 0x73, 0x69, 0x6f, 0x6e, 0x02,
];

const HEADER_LEN: usize = 40;
const RAW_CODEC: u64 = 0x55;
const DAG_CBOR_CODEC: u64 = 0x71;
const BLAKE3_256_MULTIHASH_CODE: u64 = 0x1f;
const MAX_FANOUT: usize = 128;
const DAG_NODE_VERSION: u64 = 1;
const LEAF_NODE_TYPE: &str = "sorafs.file.leaf.v1";
const BRANCH_NODE_TYPE: &str = "sorafs.file.branch.v1";
const DIR_NODE_TYPE: &str = "sorafs.dir.node.v1";

struct CarLayout {
    header_bytes: [u8; 40],
    carv1_header_prefix: Vec<u8>,
    carv1_header_bytes: Vec<u8>,
    sections: Vec<CarSection>,
    index_bytes: Option<Vec<u8>>,
    root_cids: Vec<Vec<u8>>,
}

struct CarSection {
    length_varint: Vec<u8>,
    cid_bytes: Vec<u8>,
    data: SectionData,
    digest: [u8; 32],
    offset: u64,
}

enum SectionData {
    Chunk { chunk_index: usize },
    Node(Vec<u8>),
}

impl CarSection {
    fn data_len(&self, plan: &CarBuildPlan) -> Result<usize, CarWriteError> {
        match &self.data {
            SectionData::Chunk { chunk_index } => plan
                .chunks
                .get(*chunk_index)
                .map(|chunk| chunk.length as usize)
                .ok_or(CarWriteError::DagInvariant {
                    context: "chunk section index",
                }),
            SectionData::Node(bytes) => Ok(bytes.len()),
        }
    }
}

struct ChunkRef<'a> {
    cid: &'a [u8],
    length: u32,
}

#[derive(Clone)]
struct TreeNode {
    cid_bytes: Vec<u8>,
    data: Vec<u8>,
    digest: [u8; 32],
    size: u64,
}

struct FileDag {
    nodes: Vec<TreeNode>,
    root_index: usize,
}

struct FileRootInfo<'a> {
    path: &'a [String],
    cid: Vec<u8>,
    size: u64,
}

struct DirectoryDag {
    nodes: Vec<TreeNode>,
    root_cid: Vec<u8>,
}

#[derive(Default)]
struct DirectoryBuilderNode<'a> {
    entries: BTreeMap<&'a str, DirectoryBuilderEntry<'a>>,
}

enum DirectoryBuilderEntry<'a> {
    File(DirectoryFile),
    Directory(Box<DirectoryBuilderNode<'a>>),
}

#[derive(Clone)]
struct DirectoryFile {
    cid: Vec<u8>,
    size: u64,
}

struct DirectoryEntry<'a> {
    name: &'a str,
    cid: Vec<u8>,
    kind: DirectoryEntryKind,
    size: u64,
}

fn try_reserve_car<T>(
    values: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), CarWriteError> {
    values
        .try_reserve(additional)
        .map_err(|_| CarWriteError::AllocationFailed {
            context,
            requested: additional,
        })
}

fn checked_car_usize_add(
    left: usize,
    right: usize,
    context: &'static str,
) -> Result<usize, CarWriteError> {
    left.checked_add(right)
        .ok_or(CarWriteError::ArithmeticOverflow { context })
}

fn checked_car_usize_mul(
    left: usize,
    right: usize,
    context: &'static str,
) -> Result<usize, CarWriteError> {
    left.checked_mul(right)
        .ok_or(CarWriteError::ArithmeticOverflow { context })
}

fn checked_car_u64_add(left: u64, right: u64, context: &'static str) -> Result<u64, CarWriteError> {
    left.checked_add(right)
        .ok_or(CarWriteError::ArithmeticOverflow { context })
}

fn car_usize_to_u64(value: usize, context: &'static str) -> Result<u64, CarWriteError> {
    u64::try_from(value).map_err(|_| CarWriteError::ArithmeticOverflow { context })
}

fn try_clone_car_bytes(bytes: &[u8], context: &'static str) -> Result<Vec<u8>, CarWriteError> {
    let mut clone = Vec::new();
    try_reserve_car(&mut clone, bytes.len(), context)?;
    clone.extend_from_slice(bytes);
    Ok(clone)
}

fn try_clone_car_byte_vectors(
    values: &[Vec<u8>],
    context: &'static str,
) -> Result<Vec<Vec<u8>>, CarWriteError> {
    let mut clone = Vec::new();
    try_reserve_car(&mut clone, values.len(), context)?;
    for value in values {
        clone.push(try_clone_car_bytes(value, context)?);
    }
    Ok(clone)
}

#[cfg(unix)]
fn format_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(unix)]
fn directory_plan_io(path: &Path, source: io::Error) -> CarPlanError {
    CarPlanError::DirectoryIo {
        path: format_path(path),
        source,
    }
}

fn try_reserve_plan<T>(
    values: &mut Vec<T>,
    additional: usize,
    context: &'static str,
) -> Result<(), CarPlanError> {
    values
        .try_reserve(additional)
        .map_err(|_| CarPlanError::AllocationFailed {
            context,
            requested: additional,
        })
}

fn try_owned_plan_string(value: &str, context: &'static str) -> Result<String, CarPlanError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| CarPlanError::AllocationFailed {
            context,
            requested: value.len(),
        })?;
    owned.push_str(value);
    Ok(owned)
}

fn try_clone_logical_path(path: &[String]) -> Result<Vec<String>, CarPlanError> {
    let mut clone = Vec::new();
    try_reserve_plan(&mut clone, path.len(), "logical file path")?;
    for component in path {
        clone.push(try_owned_plan_string(component, "logical path component")?);
    }
    Ok(clone)
}

fn try_clone_taikai_hint(hint: &TaikaiSegmentHint) -> Result<TaikaiSegmentHint, CarPlanError> {
    Ok(TaikaiSegmentHint {
        event: try_owned_plan_string(&hint.event, "Taikai event hint")?,
        stream: try_owned_plan_string(&hint.stream, "Taikai stream hint")?,
        rendition: try_owned_plan_string(&hint.rendition, "Taikai rendition hint")?,
        sequence: hint.sequence,
        payload_len: hint.payload_len,
        payload_digest: hint.payload_digest,
    })
}

fn checked_plan_payload_add(total: usize, additional: usize) -> Result<usize, CarPlanError> {
    total
        .checked_add(additional)
        .ok_or(CarPlanError::ContentLengthTooLarge)
}

fn ensure_plan_file_count(count: usize) -> Result<(), CarPlanError> {
    if count > CAR_PLAN_MAX_FILES {
        return Err(CarPlanError::TooManyFiles {
            count,
            maximum: CAR_PLAN_MAX_FILES,
        });
    }
    Ok(())
}

#[cfg(unix)]
struct SecureDirectoryScan {
    canonical_root: PathBuf,
    root_metadata: fs::Metadata,
    files: Vec<FileEntry>,
    entry_count: usize,
    total_bytes: u64,
}

#[cfg(unix)]
impl SecureDirectoryScan {
    fn validate_root(&self) -> Result<(), CarPlanError> {
        let current = fs::symlink_metadata(&self.canonical_root)
            .map_err(|error| directory_plan_io(&self.canonical_root, error))?;
        if current.file_type().is_symlink()
            || !current.is_dir()
            || !metadata_snapshot_matches(&self.root_metadata, &current)
        {
            return Err(CarPlanError::InvalidPath(format!(
                "directory root changed during scan: {}",
                self.canonical_root.display()
            )));
        }
        Ok(())
    }

    fn scan(
        &mut self,
        current: &Path,
        logical_path: &mut Vec<String>,
        on_file_opened: &mut dyn FnMut(&Path) -> io::Result<()>,
    ) -> Result<(), CarPlanError> {
        self.validate_root()?;
        validate_no_symlinks_below_root(&self.canonical_root, current)
            .map_err(|error| directory_plan_io(current, error))?;
        let before =
            fs::symlink_metadata(current).map_err(|error| directory_plan_io(current, error))?;
        if before.file_type().is_symlink() || !before.is_dir() {
            return Err(CarPlanError::InvalidPath(format_path(current)));
        }

        let read_dir = fs::read_dir(current).map_err(|error| directory_plan_io(current, error))?;
        let mut entries = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(|error| directory_plan_io(current, error))?;
            self.entry_count =
                self.entry_count
                    .checked_add(1)
                    .ok_or(CarPlanError::TooManyDirectoryEntries {
                        count: usize::MAX,
                        maximum: CAR_DIRECTORY_MAX_ENTRIES,
                    })?;
            if self.entry_count > CAR_DIRECTORY_MAX_ENTRIES {
                return Err(CarPlanError::TooManyDirectoryEntries {
                    count: self.entry_count,
                    maximum: CAR_DIRECTORY_MAX_ENTRIES,
                });
            }
            try_reserve_plan(&mut entries, 1, "directory entries")?;
            entries.push(entry);
        }
        entries.sort_by_key(|entry| entry.file_name());

        for entry in entries {
            let path = entry.path();
            let name = entry.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| CarPlanError::NonUtf8Path(format_path(&path)))?;
            if !is_portable_normal_component(name)
                || name.len() > CAR_LOGICAL_PATH_COMPONENT_MAX_BYTES
            {
                return Err(CarPlanError::InvalidPath(format_path(&path)));
            }
            if logical_path.len() >= CAR_LOGICAL_PATH_MAX_COMPONENTS {
                return Err(CarPlanError::InvalidPath(format!(
                    "directory tree exceeds {CAR_LOGICAL_PATH_MAX_COMPONENTS} components at {}",
                    path.display()
                )));
            }
            let logical_bytes = logical_path
                .iter()
                .try_fold(name.len(), |total, component| {
                    total
                        .checked_add(component.len())
                        .and_then(|value| value.checked_add(1))
                });
            if logical_bytes.is_none_or(|bytes| bytes > CAR_LOGICAL_PATH_MAX_BYTES) {
                return Err(CarPlanError::InvalidPath(format!(
                    "logical path exceeds {CAR_LOGICAL_PATH_MAX_BYTES} bytes at {}",
                    path.display()
                )));
            }

            let file_type = entry
                .file_type()
                .map_err(|error| directory_plan_io(&path, error))?;
            if file_type.is_symlink() {
                return Err(CarPlanError::InvalidPath(format!(
                    "symbolic links are not allowed in directory inputs: {}",
                    path.display()
                )));
            }
            let linked =
                fs::symlink_metadata(&path).map_err(|error| directory_plan_io(&path, error))?;
            if linked.file_type().is_symlink() {
                return Err(CarPlanError::InvalidPath(format_path(&path)));
            }

            try_reserve_plan(logical_path, 1, "directory logical path")?;
            logical_path.push(try_owned_plan_string(name, "directory path component")?);
            let result = if file_type.is_dir() && linked.is_dir() {
                self.scan(&path, logical_path, on_file_opened)
            } else if file_type.is_file() && linked.is_file() {
                self.read_file(&path, &linked, logical_path, on_file_opened)
            } else {
                Err(CarPlanError::InvalidPath(format!(
                    "directory entry is not a stable regular file or directory: {}",
                    path.display()
                )))
            };
            logical_path.pop();
            result?;
        }

        let after =
            fs::symlink_metadata(current).map_err(|error| directory_plan_io(current, error))?;
        if !metadata_snapshot_matches(&before, &after) {
            return Err(CarPlanError::InvalidPath(format!(
                "directory changed during scan: {}",
                current.display()
            )));
        }
        self.validate_root()
    }

    fn read_file(
        &mut self,
        path: &Path,
        linked: &fs::Metadata,
        logical_path: &[String],
        on_file_opened: &mut dyn FnMut(&Path) -> io::Result<()>,
    ) -> Result<(), CarPlanError> {
        if linked.nlink() != 1 {
            return Err(CarPlanError::InvalidPath(format!(
                "hard-linked files are not allowed in directory inputs: {}",
                path.display()
            )));
        }
        let file_count = self
            .files
            .len()
            .checked_add(1)
            .ok_or(CarPlanError::TooManyFiles {
                count: usize::MAX,
                maximum: CAR_PLAN_MAX_FILES,
            })?;
        if file_count > CAR_PLAN_MAX_FILES {
            return Err(CarPlanError::TooManyFiles {
                count: file_count,
                maximum: CAR_PLAN_MAX_FILES,
            });
        }
        let total_bytes = self.total_bytes.checked_add(linked.len()).ok_or(
            CarPlanError::DirectoryPayloadTooLarge {
                bytes: u64::MAX,
                maximum: CAR_EAGER_DIRECTORY_MAX_BYTES,
            },
        )?;
        if total_bytes > CAR_EAGER_DIRECTORY_MAX_BYTES {
            return Err(CarPlanError::DirectoryPayloadTooLarge {
                bytes: total_bytes,
                maximum: CAR_EAGER_DIRECTORY_MAX_BYTES,
            });
        }

        let mut file =
            open_confined_payload_file(&self.canonical_root, path, linked.len(), Some(linked))
                .map_err(|error| directory_plan_io(path, error))?;
        let opened = file
            .metadata()
            .map_err(|error| directory_plan_io(path, error))?;
        on_file_opened(path).map_err(|error| directory_plan_io(path, error))?;

        let byte_count =
            usize::try_from(linked.len()).map_err(|_| CarPlanError::ContentLengthTooLarge)?;
        let mut data = Vec::new();
        try_reserve_plan(&mut data, byte_count, "directory file payload")?;
        data.resize(byte_count, 0);
        file.read_exact(&mut data)
            .map_err(|error| directory_plan_io(path, error))?;
        let mut trailing = [0u8; 1];
        if file
            .read(&mut trailing)
            .map_err(|error| directory_plan_io(path, error))?
            != 0
        {
            return Err(CarPlanError::InvalidPath(format!(
                "directory file grew during scan: {}",
                path.display()
            )));
        }
        validate_payload_file_handle(path, &file, linked.len(), Some(&opened))
            .map_err(|error| directory_plan_io(path, error))?;
        self.validate_root()?;

        try_reserve_plan(&mut self.files, 1, "directory file inventory")?;
        self.files.push(FileEntry {
            path: try_clone_logical_path(logical_path)?,
            data,
        });
        self.total_bytes = total_bytes;
        Ok(())
    }
}

#[cfg(unix)]
fn gather_files_secure(
    root: &Path,
    on_file_opened: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> Result<Vec<FileEntry>, CarPlanError> {
    let root_metadata =
        fs::symlink_metadata(root).map_err(|error| directory_plan_io(root, error))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(CarPlanError::InvalidPath(format_path(root)));
    }
    let canonical_root = fs::canonicalize(root).map_err(|error| directory_plan_io(root, error))?;
    let canonical_metadata = fs::symlink_metadata(&canonical_root)
        .map_err(|error| directory_plan_io(&canonical_root, error))?;
    if !metadata_snapshot_matches(&root_metadata, &canonical_metadata) {
        return Err(CarPlanError::InvalidPath(
            "directory root changed during canonicalization".to_owned(),
        ));
    }
    let mut scan = SecureDirectoryScan {
        canonical_root: canonical_root.clone(),
        root_metadata: canonical_metadata,
        files: Vec::new(),
        entry_count: 0,
        total_bytes: 0,
    };
    let mut logical_path = Vec::new();
    try_reserve_plan(
        &mut logical_path,
        CAR_LOGICAL_PATH_MAX_COMPONENTS,
        "directory logical path stack",
    )?;
    scan.scan(&canonical_root, &mut logical_path, on_file_opened)?;
    scan.validate_root()?;
    Ok(scan.files)
}

#[cfg(not(unix))]
fn gather_files_secure(
    _root: &Path,
    _on_file_opened: &mut dyn FnMut(&Path) -> io::Result<()>,
) -> Result<Vec<FileEntry>, CarPlanError> {
    Err(CarPlanError::SecureDirectoryUnsupported)
}

#[derive(Debug, Clone, Copy)]
enum DirectoryEntryKind {
    File,
    Directory,
}

impl CarLayout {
    fn new(plan: &CarBuildPlan) -> Result<Self, CarWriteError> {
        plan.validate()?;
        let mut chunk_cids = Vec::new();
        try_reserve_car(&mut chunk_cids, plan.chunks.len(), "chunk CIDs")?;
        for chunk in &plan.chunks {
            chunk_cids.push(encode_cid(RAW_CODEC, &chunk.digest));
        }
        let mut file_root_infos = Vec::new();
        try_reserve_car(
            &mut file_root_infos,
            plan.files.len(),
            "file root inventory",
        )?;
        let mut file_nodes = Vec::new();
        try_reserve_car(&mut file_nodes, plan.files.len(), "file DAG inventory")?;

        for file in &plan.files {
            let start = file.first_chunk;
            let end = checked_car_usize_add(start, file.chunk_count, "file chunk range")?;
            if end > plan.chunks.len() || end > chunk_cids.len() {
                return Err(CarWriteError::DagInvariant {
                    context: "file chunk range",
                });
            }
            let mut chunk_refs = Vec::new();
            try_reserve_car(&mut chunk_refs, file.chunk_count, "file chunk references")?;
            for idx in start..end {
                let cid = chunk_cids.get(idx).ok_or(CarWriteError::DagInvariant {
                    context: "file chunk CID",
                })?;
                let chunk = plan.chunks.get(idx).ok_or(CarWriteError::DagInvariant {
                    context: "file chunk metadata",
                })?;
                chunk_refs.push(ChunkRef {
                    cid: cid.as_slice(),
                    length: chunk.length,
                });
            }
            let file_dag = build_file_dag(&chunk_refs, file.size)?;
            let root_node =
                file_dag
                    .nodes
                    .get(file_dag.root_index)
                    .ok_or(CarWriteError::DagInvariant {
                        context: "file DAG root index",
                    })?;
            file_root_infos.push(FileRootInfo {
                path: &file.path,
                cid: try_clone_car_bytes(&root_node.cid_bytes, "file root CID")?,
                size: file.size,
            });
            file_nodes.push(file_dag.nodes);
        }

        let needs_directory =
            plan.files.len() != 1 || plan.files.first().is_none_or(|file| !file.path.is_empty());
        let directory = if needs_directory {
            Some(build_directory_dag(&file_root_infos)?)
        } else {
            None
        };

        let root_cid = if let Some(dir) = &directory {
            try_clone_car_bytes(&dir.root_cid, "directory root CID")?
        } else {
            let first = file_root_infos.first().ok_or(CarWriteError::DagInvariant {
                context: "CAR root file",
            })?;
            try_clone_car_bytes(&first.cid, "file root CID")?
        };
        let mut root_cids = Vec::new();
        try_reserve_car(&mut root_cids, 1, "CAR root list")?;
        root_cids.push(root_cid);

        let carv1_header_bytes = encode_carv1_header(&root_cids)?;
        let carv1_header_prefix = encode_uleb128_vec(car_usize_to_u64(
            carv1_header_bytes.len(),
            "CARv1 header length",
        )?);
        let header_len_usize = checked_car_usize_add(
            carv1_header_prefix.len(),
            carv1_header_bytes.len(),
            "CARv1 framed header length",
        )?;
        let header_len = car_usize_to_u64(header_len_usize, "CARv1 framed header length")?;

        let mut section_count = plan.chunks.len();
        for nodes in &file_nodes {
            section_count =
                checked_car_usize_add(section_count, nodes.len(), "file DAG section count")?;
        }
        if let Some(directory) = &directory {
            section_count = checked_car_usize_add(
                section_count,
                directory.nodes.len(),
                "directory DAG section count",
            )?;
        }
        let mut sections = Vec::new();
        try_reserve_car(&mut sections, section_count, "CAR sections")?;

        for (idx, chunk) in plan.chunks.iter().enumerate() {
            let cid = chunk_cids.get(idx).ok_or(CarWriteError::DagInvariant {
                context: "chunk section CID",
            })?;
            let cid_bytes = try_clone_car_bytes(cid, "chunk section CID")?;
            let section_length = checked_car_usize_add(
                cid_bytes.len(),
                chunk.length as usize,
                "chunk section length",
            )?;
            let length_varint =
                encode_uleb128_vec(car_usize_to_u64(section_length, "chunk section length")?);
            sections.push(CarSection {
                length_varint,
                cid_bytes,
                data: SectionData::Chunk { chunk_index: idx },
                digest: chunk.digest,
                offset: 0,
            });
        }

        for nodes in file_nodes {
            for node in nodes {
                let section_length = checked_car_usize_add(
                    node.cid_bytes.len(),
                    node.data.len(),
                    "file DAG section length",
                )?;
                let length_varint = encode_uleb128_vec(car_usize_to_u64(
                    section_length,
                    "file DAG section length",
                )?);
                sections.push(CarSection {
                    length_varint,
                    cid_bytes: node.cid_bytes,
                    data: SectionData::Node(node.data),
                    digest: node.digest,
                    offset: 0,
                });
            }
        }

        if let Some(mut dir) = directory {
            for node in dir.nodes.drain(..) {
                let section_length = checked_car_usize_add(
                    node.cid_bytes.len(),
                    node.data.len(),
                    "directory DAG section length",
                )?;
                let length_varint = encode_uleb128_vec(car_usize_to_u64(
                    section_length,
                    "directory DAG section length",
                )?);
                sections.push(CarSection {
                    length_varint,
                    cid_bytes: node.cid_bytes,
                    data: SectionData::Node(node.data),
                    digest: node.digest,
                    offset: 0,
                });
            }
        }

        let mut payload_len = header_len;
        let mut section_offset = header_len;
        for section in sections.iter_mut() {
            section.offset = section_offset;
            let framed_len = checked_car_usize_add(
                section.length_varint.len(),
                section.cid_bytes.len(),
                "framed CAR section length",
            )?;
            let framed_len = checked_car_usize_add(
                framed_len,
                section.data_len(plan)?,
                "framed CAR section length",
            )?;
            let section_size = car_usize_to_u64(framed_len, "framed CAR section length")?;
            section_offset =
                checked_car_u64_add(section_offset, section_size, "CAR section offset")?;
            payload_len = checked_car_u64_add(payload_len, section_size, "CAR payload length")?;
        }

        let data_offset = checked_car_u64_add(
            car_usize_to_u64(PRAGMA.len(), "CAR data offset")?,
            car_usize_to_u64(HEADER_LEN, "CAR data offset")?,
            "CAR data offset",
        )?;
        let mut characteristics = [0u8; 16];

        let index_bytes = build_index(&sections)?;
        let index_offset = if index_bytes.is_some() {
            characteristics[0] |= 0x80;
            Some(checked_car_u64_add(
                data_offset,
                payload_len,
                "CAR index offset",
            )?)
        } else {
            None
        };
        if let (Some(offset), Some(index)) = (index_offset, index_bytes.as_ref()) {
            checked_car_u64_add(
                offset,
                car_usize_to_u64(index.len(), "CAR index length")?,
                "CAR archive length",
            )?;
        }

        let mut header_bytes = [0u8; HEADER_LEN];
        header_bytes[..16].copy_from_slice(&characteristics);
        header_bytes[16..24].copy_from_slice(&data_offset.to_le_bytes());
        header_bytes[24..32].copy_from_slice(&payload_len.to_le_bytes());
        header_bytes[32..40].copy_from_slice(&index_offset.unwrap_or(0).to_le_bytes());

        Ok(Self {
            header_bytes,
            carv1_header_prefix,
            carv1_header_bytes,
            sections,
            index_bytes,
            root_cids,
        })
    }

    fn write_car<W, F>(
        &self,
        plan: &CarBuildPlan,
        writer: &mut W,
        mut chunk_writer: F,
    ) -> Result<CarWriteStats, CarWriteError>
    where
        W: Write,
        F: FnMut(usize, &CarChunk, &mut W, &mut Hasher, &mut Hasher) -> Result<u64, CarWriteError>,
    {
        let mut file_hasher = Hasher::new();
        let mut payload_hasher = Hasher::new();
        let mut total_written = 0u64;

        total_written = checked_car_u64_add(
            total_written,
            write_buffer(writer, &mut file_hasher, None, &PRAGMA)?,
            "CAR archive write length",
        )?;
        total_written = checked_car_u64_add(
            total_written,
            write_buffer(writer, &mut file_hasher, None, &self.header_bytes)?,
            "CAR archive write length",
        )?;
        total_written = checked_car_u64_add(
            total_written,
            write_buffer(
                writer,
                &mut file_hasher,
                Some(&mut payload_hasher),
                &self.carv1_header_prefix,
            )?,
            "CAR archive write length",
        )?;
        total_written = checked_car_u64_add(
            total_written,
            write_buffer(
                writer,
                &mut file_hasher,
                Some(&mut payload_hasher),
                &self.carv1_header_bytes,
            )?,
            "CAR archive write length",
        )?;
        total_written = checked_car_u64_add(
            total_written,
            self.write_sections(
                plan,
                writer,
                &mut file_hasher,
                &mut payload_hasher,
                &mut chunk_writer,
            )?,
            "CAR archive write length",
        )?;
        if let Some(index_bytes) = &self.index_bytes {
            total_written = checked_car_u64_add(
                total_written,
                write_buffer(writer, &mut file_hasher, None, index_bytes)?,
                "CAR archive write length",
            )?;
        }

        let car_archive_digest = file_hasher.finalize();
        let car_payload_digest = payload_hasher.finalize();
        let mut digest_arr = [0u8; 32];
        digest_arr.copy_from_slice(car_archive_digest.as_bytes());
        let car_cid = encode_cid(RAW_CODEC, &digest_arr);

        Ok(CarWriteStats {
            payload_bytes: plan.content_length,
            chunk_count: plan.chunks.len(),
            car_size: total_written,
            car_payload_digest,
            car_archive_digest,
            car_cid,
            root_cids: try_clone_car_byte_vectors(&self.root_cids, "CAR write roots")?,
            dag_codec: DAG_CBOR_CODEC,
            chunk_profile: plan.chunk_profile,
        })
    }

    fn write_sections<W, F>(
        &self,
        plan: &CarBuildPlan,
        writer: &mut W,
        file_hasher: &mut Hasher,
        payload_hasher: &mut Hasher,
        chunk_writer: &mut F,
    ) -> Result<u64, CarWriteError>
    where
        W: Write,
        F: FnMut(usize, &CarChunk, &mut W, &mut Hasher, &mut Hasher) -> Result<u64, CarWriteError>,
    {
        let mut written = 0u64;
        for section in &self.sections {
            written = checked_car_u64_add(
                written,
                write_buffer(
                    writer,
                    file_hasher,
                    Some(payload_hasher),
                    &section.length_varint,
                )?,
                "CAR section write length",
            )?;
            written = checked_car_u64_add(
                written,
                write_buffer(
                    writer,
                    file_hasher,
                    Some(payload_hasher),
                    &section.cid_bytes,
                )?,
                "CAR section write length",
            )?;
            match &section.data {
                SectionData::Chunk { chunk_index } => {
                    let chunk =
                        plan.chunks
                            .get(*chunk_index)
                            .ok_or(CarWriteError::DagInvariant {
                                context: "chunk section index during write",
                            })?;
                    written = checked_car_u64_add(
                        written,
                        chunk_writer(*chunk_index, chunk, writer, file_hasher, payload_hasher)?,
                        "CAR section write length",
                    )?;
                }
                SectionData::Node(bytes) => {
                    written = checked_car_u64_add(
                        written,
                        write_buffer(writer, file_hasher, Some(payload_hasher), bytes)?,
                        "CAR section write length",
                    )?;
                }
            }
        }
        Ok(written)
    }
}

fn build_file_dag(chunks: &[ChunkRef<'_>], expected_size: u64) -> Result<FileDag, CarWriteError> {
    let leaf_count = chunks.len().div_ceil(MAX_FANOUT).max(1);
    let mut node_count = leaf_count;
    let mut level_count = leaf_count;
    while level_count > 1 {
        level_count = level_count.div_ceil(MAX_FANOUT);
        node_count = checked_car_usize_add(node_count, level_count, "file DAG node count")?;
    }
    let mut nodes: Vec<TreeNode> = Vec::new();
    try_reserve_car(&mut nodes, node_count, "file DAG nodes")?;
    let mut current_indices: Vec<usize> = Vec::new();
    try_reserve_car(&mut current_indices, leaf_count, "file DAG leaf indices")?;

    for group in chunks.chunks(MAX_FANOUT) {
        let node = build_leaf_node(group)?;
        nodes.push(node);
        current_indices.push(nodes.len() - 1);
    }

    if current_indices.is_empty() {
        let node = build_leaf_node(&[])?;
        nodes.push(node);
        current_indices.push(nodes.len() - 1);
    }

    while current_indices.len() > 1 {
        let next_count = div_ceil_usize(current_indices.len(), MAX_FANOUT);
        let mut next_indices = Vec::new();
        try_reserve_car(&mut next_indices, next_count, "file DAG branch indices")?;
        for group in current_indices.chunks(MAX_FANOUT) {
            let mut children = Vec::new();
            try_reserve_car(&mut children, group.len(), "file DAG branch children")?;
            for index in group {
                children.push(nodes.get(*index).ok_or(CarWriteError::DagInvariant {
                    context: "file DAG child index",
                })?);
            }
            let node = build_branch_node(&children)?;
            nodes.push(node);
            next_indices.push(nodes.len() - 1);
        }
        current_indices = next_indices;
    }

    let root_index = current_indices
        .first()
        .copied()
        .ok_or(CarWriteError::DagInvariant {
            context: "file DAG root",
        })?;
    let actual_size = nodes
        .get(root_index)
        .ok_or(CarWriteError::DagInvariant {
            context: "file DAG root index",
        })?
        .size;
    if actual_size != expected_size {
        return Err(CarWriteError::DagInvariant {
            context: "file DAG payload size",
        });
    }

    Ok(FileDag { nodes, root_index })
}

fn build_leaf_node(group: &[ChunkRef<'_>]) -> Result<TreeNode, CarWriteError> {
    let mut total_size = 0u64;
    let mut buffer_capacity = 128usize;
    for chunk in group {
        total_size = checked_car_u64_add(
            total_size,
            u64::from(chunk.length),
            "file leaf payload size",
        )?;
        let entry_capacity =
            checked_car_usize_add(chunk.cid.len(), 32, "file leaf encoding capacity")?;
        buffer_capacity = checked_car_usize_add(
            buffer_capacity,
            entry_capacity,
            "file leaf encoding capacity",
        )?;
    }
    let mut buf = Vec::new();
    try_reserve_car(&mut buf, buffer_capacity, "file leaf encoding")?;
    encode_cbor_map(&mut buf, 4);
    encode_cbor_text(&mut buf, "chunks");
    encode_cbor_array(
        &mut buf,
        car_usize_to_u64(group.len(), "file leaf child count")?,
    );
    for chunk in group {
        encode_cbor_array(&mut buf, 2);
        encode_cbor_bytes(
            &mut buf,
            car_usize_to_u64(chunk.cid.len(), "file leaf CID length")?,
        );
        buf.extend_from_slice(chunk.cid);
        encode_cbor_uint(&mut buf, u64::from(chunk.length));
    }
    encode_cbor_text(&mut buf, "size");
    encode_cbor_uint(&mut buf, total_size);
    encode_cbor_text(&mut buf, "type");
    encode_cbor_text(&mut buf, LEAF_NODE_TYPE);
    encode_cbor_text(&mut buf, "version");
    encode_cbor_uint(&mut buf, DAG_NODE_VERSION);

    let digest: [u8; 32] = blake3::hash(&buf).into();
    let cid_bytes = encode_cid(DAG_CBOR_CODEC, &digest);
    Ok(TreeNode {
        cid_bytes,
        data: buf,
        digest,
        size: total_size,
    })
}

fn build_branch_node(group: &[&TreeNode]) -> Result<TreeNode, CarWriteError> {
    let mut total_size = 0u64;
    let mut buffer_capacity = 128usize;
    for node in group {
        total_size = checked_car_u64_add(total_size, node.size, "file branch payload size")?;
        let entry_capacity =
            checked_car_usize_add(node.cid_bytes.len(), 32, "file branch encoding capacity")?;
        buffer_capacity = checked_car_usize_add(
            buffer_capacity,
            entry_capacity,
            "file branch encoding capacity",
        )?;
    }
    let mut buf = Vec::new();
    try_reserve_car(&mut buf, buffer_capacity, "file branch encoding")?;
    encode_cbor_map(&mut buf, 4);
    encode_cbor_text(&mut buf, "children");
    encode_cbor_array(
        &mut buf,
        car_usize_to_u64(group.len(), "file branch child count")?,
    );
    for child in group {
        encode_cbor_array(&mut buf, 2);
        encode_cbor_bytes(
            &mut buf,
            car_usize_to_u64(child.cid_bytes.len(), "file branch CID length")?,
        );
        buf.extend_from_slice(&child.cid_bytes);
        encode_cbor_uint(&mut buf, child.size);
    }
    encode_cbor_text(&mut buf, "size");
    encode_cbor_uint(&mut buf, total_size);
    encode_cbor_text(&mut buf, "type");
    encode_cbor_text(&mut buf, BRANCH_NODE_TYPE);
    encode_cbor_text(&mut buf, "version");
    encode_cbor_uint(&mut buf, DAG_NODE_VERSION);

    let digest: [u8; 32] = blake3::hash(&buf).into();
    let cid_bytes = encode_cid(DAG_CBOR_CODEC, &digest);
    Ok(TreeNode {
        cid_bytes,
        data: buf,
        digest,
        size: total_size,
    })
}

fn build_directory_dag(files: &[FileRootInfo<'_>]) -> Result<DirectoryDag, CarWriteError> {
    let mut directory_node_bound = 1usize;
    for file in files {
        directory_node_bound = checked_car_usize_add(
            directory_node_bound,
            file.path.len().saturating_sub(1),
            "directory DAG node count",
        )?;
    }
    let mut nodes = Vec::new();
    try_reserve_car(&mut nodes, directory_node_bound, "directory DAG nodes")?;

    let mut root = DirectoryBuilderNode::default();
    for file in files {
        insert_directory_entry(
            &mut root,
            file.path,
            DirectoryFile {
                cid: try_clone_car_bytes(&file.cid, "directory file CID")?,
                size: file.size,
            },
        )?;
    }

    let (root_cid, _) = materialize_directory_nodes(&root, &mut nodes)?;

    Ok(DirectoryDag { nodes, root_cid })
}

fn insert_directory_entry<'a>(
    node: &mut DirectoryBuilderNode<'a>,
    path: &'a [String],
    file: DirectoryFile,
) -> Result<(), CarWriteError> {
    let (head, tail) = path.split_first().ok_or(CarWriteError::DagInvariant {
        context: "directory DAG file path",
    })?;
    if tail.is_empty() {
        if node.entries.contains_key(head.as_str()) {
            return Err(CarWriteError::DirectoryPathConflict);
        }
        node.entries
            .insert(head.as_str(), DirectoryBuilderEntry::File(file));
        Ok(())
    } else {
        let child = node.entries.entry(head.as_str()).or_insert_with(|| {
            DirectoryBuilderEntry::Directory(Box::<DirectoryBuilderNode<'a>>::default())
        });
        match child {
            DirectoryBuilderEntry::File(_) => Err(CarWriteError::DirectoryPathConflict),
            DirectoryBuilderEntry::Directory(dir) => insert_directory_entry(dir, tail, file),
        }
    }
}

fn materialize_directory_nodes<'a>(
    node: &DirectoryBuilderNode<'a>,
    nodes: &mut Vec<TreeNode>,
) -> Result<(Vec<u8>, u64), CarWriteError> {
    let mut entries = Vec::new();
    try_reserve_car(&mut entries, node.entries.len(), "directory node entries")?;
    let mut total_size = 0u64;

    for (name, entry) in node.entries.iter() {
        match entry {
            DirectoryBuilderEntry::File(file) => {
                total_size =
                    checked_car_u64_add(total_size, file.size, "directory subtree payload size")?;
                entries.push(DirectoryEntry {
                    name,
                    cid: try_clone_car_bytes(&file.cid, "directory entry CID")?,
                    kind: DirectoryEntryKind::File,
                    size: file.size,
                });
            }
            DirectoryBuilderEntry::Directory(child) => {
                let (child_cid, child_size) = materialize_directory_nodes(child, nodes)?;
                total_size =
                    checked_car_u64_add(total_size, child_size, "directory subtree payload size")?;
                entries.push(DirectoryEntry {
                    name,
                    cid: child_cid,
                    kind: DirectoryEntryKind::Directory,
                    size: child_size,
                });
            }
        }
    }

    let dir_node = build_directory_node(&entries, total_size)?;
    let cid = try_clone_car_bytes(&dir_node.cid_bytes, "materialized directory CID")?;
    nodes.push(dir_node);
    Ok((cid, total_size))
}

fn build_directory_node(
    entries: &[DirectoryEntry<'_>],
    size: u64,
) -> Result<TreeNode, CarWriteError> {
    let mut buffer_capacity = 128usize;
    for entry in entries {
        let entry_capacity = checked_car_usize_add(
            entry.name.len(),
            entry.cid.len(),
            "directory node encoding capacity",
        )?;
        let entry_capacity =
            checked_car_usize_add(entry_capacity, 96, "directory node encoding capacity")?;
        buffer_capacity = checked_car_usize_add(
            buffer_capacity,
            entry_capacity,
            "directory node encoding capacity",
        )?;
    }
    let mut buf = Vec::new();
    try_reserve_car(&mut buf, buffer_capacity, "directory node encoding")?;
    encode_cbor_map(&mut buf, 4);
    encode_cbor_text(&mut buf, "entries");
    encode_cbor_array(
        &mut buf,
        car_usize_to_u64(entries.len(), "directory entry count")?,
    );
    for entry in entries {
        encode_cbor_map(&mut buf, 4);
        encode_cbor_text(&mut buf, "name");
        encode_cbor_text(&mut buf, entry.name);
        encode_cbor_text(&mut buf, "cid");
        encode_cbor_bytes(
            &mut buf,
            car_usize_to_u64(entry.cid.len(), "directory entry CID length")?,
        );
        buf.extend_from_slice(&entry.cid);
        encode_cbor_text(&mut buf, "kind");
        let kind_str = match entry.kind {
            DirectoryEntryKind::File => "file",
            DirectoryEntryKind::Directory => "dir",
        };
        encode_cbor_text(&mut buf, kind_str);
        encode_cbor_text(&mut buf, "size");
        encode_cbor_uint(&mut buf, entry.size);
    }
    encode_cbor_text(&mut buf, "size");
    encode_cbor_uint(&mut buf, size);
    encode_cbor_text(&mut buf, "type");
    encode_cbor_text(&mut buf, DIR_NODE_TYPE);
    encode_cbor_text(&mut buf, "version");
    encode_cbor_uint(&mut buf, DAG_NODE_VERSION);

    let digest: [u8; 32] = blake3::hash(&buf).into();
    let cid_bytes = encode_cid(DAG_CBOR_CODEC, &digest);
    Ok(TreeNode {
        cid_bytes,
        data: buf,
        digest,
        size,
    })
}

fn encode_carv1_header(roots: &[Vec<u8>]) -> Result<Vec<u8>, CarWriteError> {
    let mut capacity = 64usize;
    for root in roots {
        capacity = checked_car_usize_add(capacity, root.len(), "CARv1 header capacity")?;
        capacity = checked_car_usize_add(capacity, 16, "CARv1 header capacity")?;
    }
    let mut buf = Vec::new();
    try_reserve_car(&mut buf, capacity, "CARv1 header")?;
    buf.push(0xa2);
    encode_cbor_text(&mut buf, "roots");
    encode_cbor_array(&mut buf, car_usize_to_u64(roots.len(), "CARv1 root count")?);
    for root in roots {
        let len = u32::try_from(root.len()).map_err(|_| CarWriteError::RootTooLarge)?;
        encode_cbor_bytes(&mut buf, len as u64);
        buf.extend_from_slice(root);
    }
    encode_cbor_text(&mut buf, "version");
    buf.push(0x01);
    Ok(buf)
}

fn encode_cbor_text(buf: &mut Vec<u8>, text: &str) {
    encode_cbor_bytestring(buf, text.as_bytes(), true);
}

fn encode_cbor_bytes(buf: &mut Vec<u8>, len: u64) {
    encode_cbor_major(buf, 2, len);
}

fn encode_cbor_array(buf: &mut Vec<u8>, len: u64) {
    encode_cbor_major(buf, 4, len);
}

fn encode_cbor_map(buf: &mut Vec<u8>, len: u64) {
    encode_cbor_major(buf, 5, len);
}

fn encode_cbor_uint(buf: &mut Vec<u8>, value: u64) {
    encode_cbor_major(buf, 0, value);
}

fn encode_cbor_bytestring(buf: &mut Vec<u8>, bytes: &[u8], text: bool) {
    let major = if text { 3 } else { 2 };
    encode_cbor_major(buf, major, bytes.len() as u64);
    buf.extend_from_slice(bytes);
}

fn encode_cbor_major(buf: &mut Vec<u8>, major: u8, len: u64) {
    if len < 24 {
        buf.push((major << 5) | len as u8);
    } else if len < 256 {
        buf.push((major << 5) | 24);
        buf.push(len as u8);
    } else if len < 65_536 {
        buf.push((major << 5) | 25);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
    } else if len < 4_294_967_296 {
        buf.push((major << 5) | 26);
        buf.extend_from_slice(&(len as u32).to_be_bytes());
    } else {
        buf.push((major << 5) | 27);
        buf.extend_from_slice(&len.to_be_bytes());
    }
}

fn encode_cid(codec: u64, digest: &[u8; 32]) -> Vec<u8> {
    // CIDv1 + two worst-case u64 varints + one-byte fixed BLAKE3-256 length + digest. The
    // production codecs currently use one-byte varints, but the fixed 54-byte bound keeps this
    // helper safe if another u64 codec is introduced without deriving capacity from untrusted data.
    const CID_BLAKE3_256_MAX_BYTES: usize = 1 + 10 + 10 + 1 + 32;
    let mut cid = Vec::with_capacity(CID_BLAKE3_256_MAX_BYTES);
    encode_uleb128(0x01, &mut cid);
    encode_uleb128(codec, &mut cid);
    encode_uleb128(BLAKE3_256_MULTIHASH_CODE, &mut cid);
    encode_uleb128(32, &mut cid);
    cid.extend_from_slice(digest);
    cid
}

fn encode_uleb128_vec(value: u64) -> Vec<u8> {
    let mut buf = Vec::new();
    encode_uleb128(value, &mut buf);
    buf
}

fn encode_uleb128(mut value: u64, buf: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn div_ceil_usize(value: usize, divisor: usize) -> usize {
    value.div_ceil(divisor)
}

fn build_index(sections: &[CarSection]) -> Result<Option<Vec<u8>>, CarWriteError> {
    if sections.is_empty() {
        return Ok(None);
    }
    let mut entries = Vec::new();
    try_reserve_car(&mut entries, sections.len(), "CAR index entries")?;
    for section in sections {
        entries.push((section.digest, section.offset));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let entries_capacity = checked_car_usize_mul(entries.len(), 40, "CAR index capacity")?;
    let capacity = checked_car_usize_add(64, entries_capacity, "CAR index capacity")?;
    let mut buf = Vec::new();
    try_reserve_car(&mut buf, capacity, "CAR index encoding")?;
    encode_uleb128(0x0401, &mut buf);
    buf.extend_from_slice(&BLAKE3_256_MULTIHASH_CODE.to_le_bytes());
    buf.extend_from_slice(&32_u32.to_le_bytes());
    buf.extend_from_slice(&car_usize_to_u64(entries.len(), "CAR index entry count")?.to_le_bytes());
    for (digest, offset) in entries {
        buf.extend_from_slice(&digest);
        buf.extend_from_slice(&offset.to_le_bytes());
    }
    Ok(Some(buf))
}

fn validate_car_plan(plan: &CarBuildPlan) -> Result<CarPlanValidation, CarPlanValidationError> {
    plan.chunk_profile.validate()?;
    if plan.chunk_profile.max_size > CHUNK_STORE_MAX_CHUNK_BYTES as usize {
        return Err(CarPlanValidationError::ChunkProfileTooLarge {
            max_size: plan.chunk_profile.max_size,
            limit: CHUNK_STORE_MAX_CHUNK_BYTES,
        });
    }
    if plan.files.is_empty() {
        return Err(CarPlanValidationError::MissingFiles);
    }
    if plan.files.len() > CAR_PLAN_MAX_FILES {
        return Err(CarPlanValidationError::TooManyFiles {
            count: plan.files.len(),
            maximum: CAR_PLAN_MAX_FILES,
        });
    }
    if plan.chunks.len() > CAR_PLAN_MAX_CHUNKS {
        return Err(CarPlanValidationError::TooManyChunks {
            count: plan.chunks.len(),
            maximum: CAR_PLAN_MAX_CHUNKS,
        });
    }
    if plan.content_length == 0 {
        if !plan.chunks.is_empty() {
            return Err(CarPlanValidationError::EmptyPayloadHasChunks {
                count: plan.chunks.len(),
            });
        }
        if plan.payload_digest != blake3::hash(&[]) {
            return Err(CarPlanValidationError::InvalidEmptyPayloadDigest);
        }
    } else if plan.chunks.is_empty() {
        return Err(CarPlanValidationError::MissingChunks);
    }

    let profile_limit = u32::try_from(plan.chunk_profile.max_size).map_err(|_| {
        CarPlanValidationError::ChunkProfileTooLarge {
            max_size: plan.chunk_profile.max_size,
            limit: CHUNK_STORE_MAX_CHUNK_BYTES,
        }
    })?;
    let mut expected_offset = 0u64;
    let mut max_chunk_len = 0usize;
    let mut por_leaf_count = 0usize;
    let mut por_segment_count = 0usize;
    for (chunk_index, chunk) in plan.chunks.iter().enumerate() {
        if chunk.length == 0 {
            return Err(CarPlanValidationError::ZeroLengthChunk { chunk_index });
        }
        if chunk.length > profile_limit {
            return Err(CarPlanValidationError::ChunkTooLarge {
                chunk_index,
                length: chunk.length,
                limit: profile_limit,
            });
        }
        let chunk_end = chunk
            .offset
            .checked_add(u64::from(chunk.length))
            .ok_or(CarPlanValidationError::ChunkRangeOverflow { chunk_index })?;
        if chunk.offset != expected_offset {
            return Err(CarPlanValidationError::NonContiguousChunk {
                chunk_index,
                expected: expected_offset,
                actual: chunk.offset,
            });
        }
        expected_offset = chunk_end;
        let chunk_len = usize::try_from(chunk.length).map_err(|_| {
            CarPlanValidationError::EstimateOverflow {
                context: "maximum chunk buffer",
            }
        })?;
        max_chunk_len = max_chunk_len.max(chunk_len);
        por_leaf_count = por_leaf_count
            .checked_add(chunk_len.div_ceil(POR_LEAF_SIZE))
            .ok_or(CarPlanValidationError::EstimateOverflow {
                context: "PoR leaf count",
            })?;
        por_segment_count = por_segment_count
            .checked_add(chunk_len.div_ceil(POR_SEGMENT_SIZE))
            .ok_or(CarPlanValidationError::EstimateOverflow {
                context: "PoR segment count",
            })?;
    }
    if expected_offset != plan.content_length {
        return Err(CarPlanValidationError::ContentLengthMismatch {
            expected: plan.content_length,
            actual: expected_offset,
        });
    }

    let mut expected_file_offset = 0u64;
    let mut expected_first_chunk = 0usize;
    let mut previous_path: Option<&[String]> = None;
    let mut total_path_components = 0usize;
    let mut total_path_bytes = 0usize;
    for (file_index, file) in plan.files.iter().enumerate() {
        if file.path.is_empty() {
            if plan.files.len() != 1 {
                return Err(CarPlanValidationError::EmptyPathInMultiFilePlan { file_index });
            }
        } else {
            if file.path.len() > CAR_LOGICAL_PATH_MAX_COMPONENTS {
                return Err(CarPlanValidationError::PathTooDeep {
                    file_index,
                    count: file.path.len(),
                    maximum: CAR_LOGICAL_PATH_MAX_COMPONENTS,
                });
            }
            let mut file_path_bytes = 0usize;
            for (component_index, component) in file.path.iter().enumerate() {
                if !is_portable_normal_component(component) {
                    return Err(CarPlanValidationError::InvalidPathComponent {
                        file_index,
                        component_index,
                    });
                }
                if component.len() > CAR_LOGICAL_PATH_COMPONENT_MAX_BYTES {
                    return Err(CarPlanValidationError::PathComponentTooLong {
                        file_index,
                        component_index,
                        bytes: component.len(),
                        maximum: CAR_LOGICAL_PATH_COMPONENT_MAX_BYTES,
                    });
                }
                file_path_bytes = file_path_bytes.checked_add(component.len()).ok_or(
                    CarPlanValidationError::EstimateOverflow {
                        context: "logical path bytes",
                    },
                )?;
                total_path_components = total_path_components.checked_add(1).ok_or(
                    CarPlanValidationError::EstimateOverflow {
                        context: "logical path components",
                    },
                )?;
            }
            file_path_bytes = file_path_bytes
                .checked_add(file.path.len().saturating_sub(1))
                .ok_or(CarPlanValidationError::EstimateOverflow {
                    context: "logical path separators",
                })?;
            if file_path_bytes > CAR_LOGICAL_PATH_MAX_BYTES {
                return Err(CarPlanValidationError::PathTooLong {
                    file_index,
                    bytes: file_path_bytes,
                    maximum: CAR_LOGICAL_PATH_MAX_BYTES,
                });
            }
            total_path_bytes = total_path_bytes.checked_add(file_path_bytes).ok_or(
                CarPlanValidationError::EstimateOverflow {
                    context: "aggregate logical path bytes",
                },
            )?;
        }
        if let Some(previous) = previous_path {
            if previous >= file.path.as_slice() {
                return Err(CarPlanValidationError::NonCanonicalFileOrder { file_index });
            }
            if file.path.starts_with(previous) {
                return Err(CarPlanValidationError::FilePathConflict { file_index });
            }
        }
        previous_path = Some(&file.path);

        if file.first_chunk != expected_first_chunk {
            return Err(CarPlanValidationError::NonCanonicalFileChunkStart {
                file_index,
                expected: expected_first_chunk,
                actual: file.first_chunk,
            });
        }
        let chunk_end = file
            .first_chunk
            .checked_add(file.chunk_count)
            .ok_or(CarPlanValidationError::FileChunkRangeOverflow { file_index })?;
        if chunk_end > plan.chunks.len() {
            return Err(CarPlanValidationError::FileChunkRangeOutOfBounds {
                file_index,
                end: chunk_end,
                chunk_count: plan.chunks.len(),
            });
        }
        let file_end = expected_file_offset
            .checked_add(file.size)
            .ok_or(CarPlanValidationError::FileByteRangeOverflow { file_index })?;
        if file.size == 0 {
            if file.chunk_count != 0 {
                return Err(CarPlanValidationError::EmptyFileHasChunks { file_index });
            }
        } else {
            if file.chunk_count == 0 {
                return Err(CarPlanValidationError::NonEmptyFileHasNoChunks { file_index });
            }
            let minimum = u64::try_from(plan.chunk_profile.min_size).map_err(|_| {
                CarPlanValidationError::EstimateOverflow {
                    context: "chunk profile minimum",
                }
            })?;
            let maximum_count = usize::try_from(file.size.div_ceil(minimum)).map_err(|_| {
                CarPlanValidationError::EstimateOverflow {
                    context: "canonical per-file chunk count",
                }
            })?;
            if file.chunk_count > maximum_count {
                return Err(CarPlanValidationError::TooManyFileChunks {
                    file_index,
                    maximum: maximum_count,
                    actual: file.chunk_count,
                });
            }
            for chunk_index in file.first_chunk..chunk_end.saturating_sub(1) {
                let length = plan.chunks[chunk_index].length;
                if usize::try_from(length).unwrap_or(usize::MAX) < plan.chunk_profile.min_size {
                    return Err(CarPlanValidationError::ChunkBelowProfileMinimum {
                        chunk_index,
                        length,
                        minimum: plan.chunk_profile.min_size,
                    });
                }
            }
            let first = &plan.chunks[file.first_chunk];
            let last = &plan.chunks[chunk_end - 1];
            let planned_end = last.offset.checked_add(u64::from(last.length)).ok_or(
                CarPlanValidationError::ChunkRangeOverflow {
                    chunk_index: chunk_end - 1,
                },
            )?;
            if first.offset != expected_file_offset || planned_end != file_end {
                return Err(CarPlanValidationError::FileChunkByteRangeMismatch { file_index });
            }
        }
        expected_first_chunk = chunk_end;
        expected_file_offset = file_end;
    }
    if expected_first_chunk != plan.chunks.len() {
        return Err(CarPlanValidationError::ChunkCoverageMismatch {
            expected: plan.chunks.len(),
            actual: expected_first_chunk,
        });
    }
    if expected_file_offset != plan.content_length {
        return Err(CarPlanValidationError::FileCoverageMismatch {
            expected: plan.content_length,
            actual: expected_file_offset,
        });
    }

    let mut estimated_ingest_heap_bytes = max_chunk_len;
    checked_estimate_add_product(
        &mut estimated_ingest_heap_bytes,
        plan.chunks.len(),
        std::mem::size_of::<StoredChunk>()
            + std::mem::size_of::<PorChunkTree>()
            + std::mem::size_of::<[u8; 32]>() * 3
            + std::mem::size_of::<ExpectedSinkChunk>()
            + std::mem::size_of::<PersistedChunkRecord>()
            + 32,
        "chunk metadata, roots, and directory sink records",
    )?;
    checked_estimate_add_product(
        &mut estimated_ingest_heap_bytes,
        por_segment_count,
        std::mem::size_of::<PorSegment>() + std::mem::size_of::<[u8; 32]>(),
        "PoR segments",
    )?;
    checked_estimate_add_product(
        &mut estimated_ingest_heap_bytes,
        por_leaf_count,
        std::mem::size_of::<PorLeaf>() + std::mem::size_of::<[u8; 32]>(),
        "PoR leaves",
    )?;
    checked_estimate_add_product(
        &mut estimated_ingest_heap_bytes,
        plan.files.len(),
        std::mem::size_of::<FilePlan>(),
        "file inventory",
    )?;
    checked_estimate_add_product(
        &mut estimated_ingest_heap_bytes,
        total_path_components,
        std::mem::size_of::<String>(),
        "logical path components",
    )?;
    estimated_ingest_heap_bytes = estimated_ingest_heap_bytes
        .checked_add(total_path_bytes)
        .ok_or(CarPlanValidationError::EstimateOverflow {
            context: "logical path bytes",
        })?;
    #[cfg(feature = "manifest")]
    if plan.content_length != 0 {
        let pdp_retained = estimated_pdp_heap_bytes(plan.content_length).map_err(|_| {
            CarPlanValidationError::EstimateOverflow {
                context: "canonical PDP tree",
            }
        })?;
        let pdp_peak = pdp_retained
            .checked_mul(2)
            .and_then(|value| value.checked_add(256 * 1024))
            .ok_or(CarPlanValidationError::EstimateOverflow {
                context: "canonical PDP construction peak",
            })?;
        estimated_ingest_heap_bytes = estimated_ingest_heap_bytes.checked_add(pdp_peak).ok_or(
            CarPlanValidationError::EstimateOverflow {
                context: "total ingest heap",
            },
        )?;
    }
    if estimated_ingest_heap_bytes > isize::MAX as usize {
        return Err(CarPlanValidationError::EstimateOverflow {
            context: "host allocation limit",
        });
    }

    Ok(CarPlanValidation {
        max_chunk_len,
        estimated_ingest_heap_bytes,
    })
}

fn checked_estimate_add_product(
    total: &mut usize,
    count: usize,
    item_size: usize,
    context: &'static str,
) -> Result<(), CarPlanValidationError> {
    let bytes = count
        .checked_mul(item_size)
        .ok_or(CarPlanValidationError::EstimateOverflow { context })?;
    if bytes > isize::MAX as usize {
        return Err(CarPlanValidationError::EstimateOverflow { context });
    }
    *total = total
        .checked_add(bytes)
        .ok_or(CarPlanValidationError::EstimateOverflow { context })?;
    Ok(())
}

fn estimate_direct_chunk_store_heap(
    payload_len: usize,
    profile: ChunkProfile,
) -> Result<usize, CarPlanValidationError> {
    profile.validate()?;
    if profile.max_size > CHUNK_STORE_MAX_CHUNK_BYTES as usize {
        return Err(CarPlanValidationError::ChunkProfileTooLarge {
            max_size: profile.max_size,
            limit: CHUNK_STORE_MAX_CHUNK_BYTES,
        });
    }
    let chunk_count = if payload_len == 0 {
        0
    } else {
        payload_len.div_ceil(profile.min_size)
    };
    if chunk_count > CAR_PLAN_MAX_CHUNKS {
        return Err(CarPlanValidationError::TooManyChunks {
            count: chunk_count,
            maximum: CAR_PLAN_MAX_CHUNKS,
        });
    }
    let por_leaf_count = if payload_len == 0 {
        0
    } else {
        payload_len
            .div_ceil(POR_LEAF_SIZE)
            .checked_add(chunk_count.saturating_sub(1))
            .ok_or(CarPlanValidationError::EstimateOverflow {
                context: "direct PoR leaf count",
            })?
    };
    let por_segment_count = if payload_len == 0 {
        0
    } else {
        payload_len
            .div_ceil(POR_SEGMENT_SIZE)
            .checked_add(chunk_count.saturating_sub(1))
            .ok_or(CarPlanValidationError::EstimateOverflow {
                context: "direct PoR segment count",
            })?
    };
    let mut total = payload_len.min(profile.max_size);
    checked_estimate_add_product(
        &mut total,
        chunk_count,
        std::mem::size_of::<ChunkDigest>()
            + std::mem::size_of::<StoredChunk>()
            + std::mem::size_of::<PorChunkTree>()
            + std::mem::size_of::<[u8; 32]>() * 3
            + std::mem::size_of::<ExpectedSinkChunk>()
            + std::mem::size_of::<PersistedChunkRecord>()
            + 32,
        "direct chunk metadata, roots, and directory sink records",
    )?;
    checked_estimate_add_product(
        &mut total,
        por_segment_count,
        std::mem::size_of::<PorSegment>() + std::mem::size_of::<[u8; 32]>(),
        "direct PoR segments",
    )?;
    checked_estimate_add_product(
        &mut total,
        por_leaf_count,
        std::mem::size_of::<PorLeaf>() + std::mem::size_of::<[u8; 32]>(),
        "direct PoR leaves",
    )?;
    #[cfg(feature = "manifest")]
    if payload_len != 0 {
        let payload_len_u64 =
            u64::try_from(payload_len).map_err(|_| CarPlanValidationError::EstimateOverflow {
                context: "direct PDP payload length",
            })?;
        let retained = estimated_pdp_heap_bytes(payload_len_u64).map_err(|_| {
            CarPlanValidationError::EstimateOverflow {
                context: "direct canonical PDP tree",
            }
        })?;
        let peak = retained
            .checked_mul(2)
            .and_then(|value| value.checked_add(256 * 1024))
            .ok_or(CarPlanValidationError::EstimateOverflow {
                context: "direct canonical PDP construction peak",
            })?;
        total = total
            .checked_add(peak)
            .ok_or(CarPlanValidationError::EstimateOverflow {
                context: "direct total ingest heap",
            })?;
    }
    if total > isize::MAX as usize {
        return Err(CarPlanValidationError::EstimateOverflow {
            context: "direct host allocation limit",
        });
    }
    Ok(total)
}

fn is_portable_normal_component(component: &str) -> bool {
    if component.is_empty()
        || component == "."
        || component == ".."
        || component.contains('/')
        || component.contains('\\')
        || component.contains(':')
        || component.chars().any(|character| {
            character.is_control() || matches!(character, '<' | '>' | '"' | '|' | '?' | '*')
        })
        || component.ends_with('.')
        || component.ends_with(' ')
    {
        return false;
    }
    let Some(basename) = component.split('.').next() else {
        return false;
    };
    let basename = basename.trim_end_matches(' ');
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "CLOCK$"]
        .iter()
        .any(|reserved| basename.eq_ignore_ascii_case(reserved))
    {
        return false;
    }
    if let (Some(prefix), Some(suffix)) = (basename.get(..3), basename.get(3..)) {
        let reserved_prefix =
            prefix.eq_ignore_ascii_case("COM") || prefix.eq_ignore_ascii_case("LPT");
        let reserved_ascii_digit = suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9');
        if reserved_prefix && (reserved_ascii_digit || matches!(suffix, "¹" | "²" | "³")) {
            return false;
        }
    }
    true
}

fn append_file_chunks(
    profile: ChunkProfile,
    data: &[u8],
    base_offset: u64,
    output: &mut Vec<CarChunk>,
) -> Result<(), CarPlanError> {
    if data.is_empty() {
        return Ok(());
    }
    let maximum_chunks = data.len().div_ceil(profile.min_size);
    let mut boundaries = Vec::new();
    try_reserve_plan(&mut boundaries, maximum_chunks, "file chunk boundaries")?;
    let mut chunker = sorafs_chunker::Chunker::try_with_profile(profile)?;
    let mut emitted_too_many = false;
    chunker.feed(data, |chunk| {
        if boundaries.len() < maximum_chunks {
            boundaries.push(chunk);
        } else {
            emitted_too_many = true;
        }
    });
    chunker.finish(|chunk| {
        if boundaries.len() < maximum_chunks {
            boundaries.push(chunk);
        } else {
            emitted_too_many = true;
        }
    });
    if emitted_too_many {
        return Err(CarPlanError::TooManyChunks {
            count: maximum_chunks.saturating_add(1),
            maximum: maximum_chunks,
        });
    }

    for boundary in boundaries {
        let end =
            boundary
                .checked_end()
                .ok_or(sorafs_chunker::ChunkerError::ChunkRangeOverflow {
                    offset: boundary.offset,
                    length: boundary.length,
                })?;
        if end > data.len() {
            return Err(CarPlanError::Chunking(
                sorafs_chunker::ChunkerError::ChunkRangeOverflow {
                    offset: boundary.offset,
                    length: boundary.length,
                },
            ));
        }
        let local_offset =
            u64::try_from(boundary.offset).map_err(|_| CarPlanError::ChunkOffsetTooLarge {
                offset: boundary.offset,
            })?;
        let offset = base_offset
            .checked_add(local_offset)
            .ok_or(CarPlanError::ContentLengthTooLarge)?;
        let length =
            u32::try_from(boundary.length).map_err(|_| CarPlanError::ChunkLengthTooLarge {
                length: boundary.length,
                limit: CAR_CHUNK_LENGTH_LIMIT,
            })?;
        if output.len() >= output.capacity() {
            return Err(CarPlanError::AllocationFailed {
                context: "pre-reserved CAR chunk inventory",
                requested: output.len().saturating_add(1),
            });
        }
        output.push(CarChunk {
            offset,
            length,
            digest: blake3::hash(&data[boundary.offset..end]).into(),
            taikai_segment_hint: None,
        });
    }
    Ok(())
}

impl CarBuildPlan {
    /// Validate the complete chunk/file plan and return checked allocation geometry.
    ///
    /// This method performs no allocation or I/O. It validates the chunking profile,
    /// canonical per-file chunk bounds, exact chunk and file coverage, portable logical
    /// paths, and every arithmetic operation used by ingest and writer allocation.
    pub fn validate(&self) -> Result<CarPlanValidation, CarPlanValidationError> {
        validate_car_plan(self)
    }

    /// Validate this plan for chunk-store ingestion using the production default heap limit.
    pub fn validate_for_ingest(&self) -> Result<CarPlanValidation, ChunkStoreError> {
        self.validate_for_ingest_with_limit(DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES)
    }

    /// Validate this plan for chunk-store ingestion under an explicit heap estimate limit.
    pub fn validate_for_ingest_with_limit(
        &self,
        max_estimated_heap_bytes: usize,
    ) -> Result<CarPlanValidation, ChunkStoreError> {
        let validation = self.validate()?;
        if validation.estimated_ingest_heap_bytes > max_estimated_heap_bytes {
            return Err(ChunkStoreError::EstimatedHeapLimitExceeded {
                estimated: validation.estimated_ingest_heap_bytes,
                limit: max_estimated_heap_bytes,
            });
        }
        Ok(validation)
    }

    /// Creates a CAR plan by chunking the provided payload with the SoraFS SF-1
    /// profile. Returns `CarPlanError::EmptyInput` if the payload is empty.
    pub fn single_file(payload: &[u8]) -> Result<Self, CarPlanError> {
        Self::single_file_with_profile(payload, ChunkProfile::DEFAULT)
    }

    /// Same as [`single_file`] but uses a custom chunking profile.
    pub fn single_file_with_profile(
        payload: &[u8],
        profile: ChunkProfile,
    ) -> Result<Self, CarPlanError> {
        if payload.is_empty() {
            return Err(CarPlanError::EmptyInput);
        }
        ensure_car_plan_profile(profile)?;
        let maximum_chunk_count = payload.len().div_ceil(profile.min_size);
        if maximum_chunk_count > CAR_PLAN_MAX_CHUNKS {
            return Err(CarPlanError::TooManyChunks {
                count: maximum_chunk_count,
                maximum: CAR_PLAN_MAX_CHUNKS,
            });
        }
        let mut chunks = Vec::new();
        try_reserve_plan(
            &mut chunks,
            maximum_chunk_count,
            "single-file chunk inventory",
        )?;
        append_file_chunks(profile, payload, 0, &mut chunks)?;
        let chunk_count = chunks.len();
        let content_length =
            u64::try_from(payload.len()).map_err(|_| CarPlanError::ContentLengthTooLarge)?;
        let mut files = Vec::new();
        try_reserve_plan(&mut files, 1, "single-file inventory")?;
        files.push(FilePlan {
            path: Vec::new(),
            first_chunk: 0,
            chunk_count,
            size: content_length,
        });
        let plan = Self {
            chunk_profile: profile,
            payload_digest: blake3::hash(payload),
            content_length,
            chunks,
            files,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Builds a CAR plan for every regular file under `root`, preserving
    /// lexicographic order and returning the concatenated payload.
    pub fn from_directory(root: &Path) -> Result<(Self, Vec<u8>), CarPlanError> {
        Self::from_directory_with_profile(root, ChunkProfile::DEFAULT)
    }

    /// Same as [`from_directory`] but uses a custom chunking profile.
    pub fn from_directory_with_profile(
        root: &Path,
        profile: ChunkProfile,
    ) -> Result<(Self, Vec<u8>), CarPlanError> {
        ensure_car_plan_profile(profile)?;
        let mut no_scan_hook = |_path: &Path| Ok(());
        let files = gather_files_secure(root, &mut no_scan_hook)?;
        if files.is_empty() {
            return Err(CarPlanError::EmptyInput);
        }
        Self::from_files_with_profile(files, profile)
    }

    /// Builds a CAR plan for multiple files, returning the plan alongside the
    /// concatenated payload bytes that must be passed to [`CarWriter`]. The
    /// files are addressed by their UTF-8 path components (relative to the
    /// dataset root). Paths are validated as portable normal components and
    /// must be strictly ordered after sorting. Empty files are represented by
    /// a zero-sized file range and zero chunks; synthetic zero-length chunks
    /// are never emitted.
    pub fn from_files(files: Vec<FileEntry>) -> Result<(Self, Vec<u8>), CarPlanError> {
        Self::from_files_with_profile(files, ChunkProfile::DEFAULT)
    }

    pub fn from_files_with_profile(
        mut files: Vec<FileEntry>,
        profile: ChunkProfile,
    ) -> Result<(Self, Vec<u8>), CarPlanError> {
        if files.is_empty() {
            return Err(CarPlanError::EmptyInput);
        }
        ensure_plan_file_count(files.len())?;
        ensure_car_plan_profile(profile)?;

        files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut total_payload_len = 0usize;
        let mut maximum_chunk_count = 0usize;
        for entry in &files {
            validate_path(&entry.path)?;
            total_payload_len = checked_plan_payload_add(total_payload_len, entry.data.len())?;
            if !entry.data.is_empty() {
                let file_maximum = entry.data.len().div_ceil(profile.min_size);
                maximum_chunk_count = maximum_chunk_count.checked_add(file_maximum).ok_or(
                    CarPlanError::TooManyChunks {
                        count: usize::MAX,
                        maximum: CAR_PLAN_MAX_CHUNKS,
                    },
                )?;
                if maximum_chunk_count > CAR_PLAN_MAX_CHUNKS {
                    return Err(CarPlanError::TooManyChunks {
                        count: maximum_chunk_count,
                        maximum: CAR_PLAN_MAX_CHUNKS,
                    });
                }
            }
        }
        for pair in files.windows(2) {
            let previous = &pair[0].path;
            let current = &pair[1].path;
            if previous == current {
                return Err(CarPlanError::DuplicatePath(path_to_string(current)));
            }
            if current.starts_with(previous) {
                return Err(CarPlanError::PathConflict {
                    existing: path_to_string(previous),
                    new: path_to_string(current),
                });
            }
        }
        let content_length =
            u64::try_from(total_payload_len).map_err(|_| CarPlanError::ContentLengthTooLarge)?;

        let mut chunks = Vec::new();
        try_reserve_plan(&mut chunks, maximum_chunk_count, "CAR chunk inventory")?;
        let mut file_plans = Vec::new();
        try_reserve_plan(&mut file_plans, files.len(), "CAR file inventory")?;
        let mut payload = Vec::new();
        try_reserve_plan(&mut payload, total_payload_len, "concatenated CAR payload")?;
        let mut hasher = blake3::Hasher::new();
        let mut base_offset = 0u64;

        for entry in files {
            let start_chunk = chunks.len();
            let data_len =
                u64::try_from(entry.data.len()).map_err(|_| CarPlanError::ContentLengthTooLarge)?;
            hasher.update(&entry.data);
            payload.extend_from_slice(&entry.data);
            append_file_chunks(profile, &entry.data, base_offset, &mut chunks)?;
            let end_chunk = chunks.len();
            file_plans.push(FilePlan {
                path: entry.path,
                first_chunk: start_chunk,
                chunk_count: end_chunk - start_chunk,
                size: data_len,
            });
            base_offset = base_offset
                .checked_add(data_len)
                .ok_or(CarPlanError::ContentLengthTooLarge)?;
        }

        let payload_digest = hasher.finalize();
        if payload.len() != total_payload_len || base_offset != content_length {
            return Err(CarPlanError::ContentLengthTooLarge);
        }
        let plan = Self {
            chunk_profile: profile,
            payload_digest,
            content_length,
            chunks,
            files: file_plans,
        };
        plan.validate()?;
        Ok((plan, payload))
    }

    /// Tries to build the list of chunk fetch specifications derived from this validated plan.
    ///
    /// This helper is convenient for multi-source retrieval orchestrators that
    /// need to schedule chunk downloads while verifying digests and payload
    /// offsets deterministically.
    pub fn try_chunk_fetch_specs(&self) -> Result<Vec<ChunkFetchSpec>, CarPlanError> {
        self.validate()?;
        let mut estimated = self
            .chunks
            .len()
            .checked_mul(std::mem::size_of::<ChunkFetchSpec>())
            .ok_or(CarPlanError::EstimatedHeapLimitExceeded {
                context: "chunk fetch specifications",
                estimated: usize::MAX,
                limit: DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
            })?;
        for chunk in &self.chunks {
            if let Some(hint) = &chunk.taikai_segment_hint {
                estimated = estimated
                    .checked_add(hint.event.len())
                    .and_then(|bytes| bytes.checked_add(hint.stream.len()))
                    .and_then(|bytes| bytes.checked_add(hint.rendition.len()))
                    .ok_or(CarPlanError::EstimatedHeapLimitExceeded {
                        context: "chunk fetch specifications",
                        estimated: usize::MAX,
                        limit: DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
                    })?;
            }
        }
        if estimated > DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES
            || estimated > isize::MAX as usize
        {
            return Err(CarPlanError::EstimatedHeapLimitExceeded {
                context: "chunk fetch specifications",
                estimated,
                limit: DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES,
            });
        }
        let mut specs = Vec::new();
        try_reserve_plan(&mut specs, self.chunks.len(), "chunk fetch specifications")?;
        for (index, chunk) in self.chunks.iter().enumerate() {
            specs.push(ChunkFetchSpec {
                chunk_index: index,
                offset: chunk.offset,
                length: chunk.length,
                digest: chunk.digest,
                taikai_segment_hint: chunk
                    .taikai_segment_hint
                    .as_ref()
                    .map(try_clone_taikai_hint)
                    .transpose()?,
            });
        }
        Ok(specs)
    }
}

fn validate_path(path: &[String]) -> Result<(), CarPlanError> {
    if path.is_empty() {
        return Err(CarPlanError::InvalidPath("".into()));
    }
    if path.len() > CAR_LOGICAL_PATH_MAX_COMPONENTS {
        return Err(CarPlanError::InvalidPath(format!(
            "logical path has {} components; maximum is {CAR_LOGICAL_PATH_MAX_COMPONENTS}",
            path.len()
        )));
    }
    let mut bytes = path.len().saturating_sub(1);
    for (index, component) in path.iter().enumerate() {
        if !is_portable_normal_component(component)
            || component.len() > CAR_LOGICAL_PATH_COMPONENT_MAX_BYTES
        {
            return Err(CarPlanError::InvalidPath(format!(
                "logical path component {index} is not portable or exceeds {CAR_LOGICAL_PATH_COMPONENT_MAX_BYTES} bytes"
            )));
        }
        bytes = bytes
            .checked_add(component.len())
            .ok_or(CarPlanError::ContentLengthTooLarge)?;
    }
    if bytes > CAR_LOGICAL_PATH_MAX_BYTES {
        return Err(CarPlanError::InvalidPath(format!(
            "logical path has {bytes} bytes; maximum is {CAR_LOGICAL_PATH_MAX_BYTES}"
        )));
    }
    Ok(())
}

fn path_to_string(path: &[String]) -> String {
    path.join("/")
}

/// Input file used when constructing multi-file CAR plans.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: Vec<String>,
    pub data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        collections::{BTreeMap, HashSet},
        fs,
        io::Cursor,
        rc::Rc,
    };

    use sorafs_chunker::fixtures::FixtureProfile;
    use tempfile::tempdir;

    use super::*;

    #[derive(Debug)]
    struct StoreSnapshot {
        profile: ChunkProfile,
        chunks: Vec<StoredChunk>,
        por_root: [u8; 32],
        payload_digest: [u8; 32],
        payload_len: u64,
        #[cfg(feature = "manifest")]
        pdp_hot_root: Option<[u8; 32]>,
        #[cfg(feature = "manifest")]
        pdp_segment_root: Option<[u8; 32]>,
    }

    impl StoreSnapshot {
        fn capture(store: &ChunkStore) -> Self {
            Self {
                profile: store.profile(),
                chunks: store.chunks().to_vec(),
                por_root: *store.por_tree().root(),
                payload_digest: *store.payload_digest().as_bytes(),
                payload_len: store.payload_len(),
                #[cfg(feature = "manifest")]
                pdp_hot_root: store.pdp_hot_root(),
                #[cfg(feature = "manifest")]
                pdp_segment_root: store.pdp_segment_root(),
            }
        }

        fn assert_unchanged(&self, store: &ChunkStore) {
            assert_eq!(store.profile(), self.profile);
            assert_eq!(store.chunks(), self.chunks);
            assert_eq!(store.por_tree().root(), &self.por_root);
            assert_eq!(store.payload_digest().as_bytes(), &self.payload_digest);
            assert_eq!(store.payload_len(), self.payload_len);
            #[cfg(feature = "manifest")]
            {
                assert_eq!(store.pdp_hot_root(), self.pdp_hot_root);
                assert_eq!(store.pdp_segment_root(), self.pdp_segment_root);
            }
        }
    }

    #[derive(Clone)]
    struct ProbeSource {
        reads: Rc<Cell<usize>>,
    }

    impl PayloadSource for ProbeSource {
        fn read_exact(&mut self, _offset: u64, _buf: &mut [u8]) -> Result<(), ChunkStoreError> {
            self.reads.set(self.reads.get() + 1);
            Err(ChunkStoreError::Io(io::Error::other(
                "probe source must not be read",
            )))
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum ProbeSinkFailure {
        Never,
        Write(usize),
        Finish,
    }

    #[derive(Clone)]
    struct ProbeSink {
        prepares: Rc<Cell<usize>>,
        writes: Rc<Cell<usize>>,
        finishes: Rc<Cell<usize>>,
        failure: ProbeSinkFailure,
    }

    impl ChunkSink for ProbeSink {
        type Output = ();

        fn prepare(&mut self, _plan: &CarBuildPlan) -> Result<(), ChunkStoreError> {
            self.prepares.set(self.prepares.get() + 1);
            Ok(())
        }

        fn write_chunk(
            &mut self,
            index: usize,
            _chunk: &CarChunk,
            _data: &[u8],
        ) -> Result<(), ChunkStoreError> {
            self.writes.set(self.writes.get() + 1);
            if matches!(self.failure, ProbeSinkFailure::Write(failed) if failed == index) {
                return Err(ChunkStoreError::Io(io::Error::other(
                    "injected sink write failure",
                )));
            }
            Ok(())
        }

        fn finish(self) -> Result<Self::Output, ChunkStoreError> {
            self.finishes.set(self.finishes.get() + 1);
            if matches!(self.failure, ProbeSinkFailure::Finish) {
                return Err(ChunkStoreError::Io(io::Error::other(
                    "injected sink finish failure",
                )));
            }
            Ok(())
        }
    }

    fn probe_sink(failure: ProbeSinkFailure) -> (ProbeSink, [Rc<Cell<usize>>; 3]) {
        let prepares = Rc::new(Cell::new(0));
        let writes = Rc::new(Cell::new(0));
        let finishes = Rc::new(Cell::new(0));
        (
            ProbeSink {
                prepares: Rc::clone(&prepares),
                writes: Rc::clone(&writes),
                finishes: Rc::clone(&finishes),
                failure,
            },
            [prepares, writes, finishes],
        )
    }

    fn reject_before_io(plan: &CarBuildPlan) -> ChunkStoreError {
        let mut store = ChunkStore::new();
        store
            .ingest_bytes(b"pre-existing store state")
            .expect("seed store");
        let before = StoreSnapshot::capture(&store);
        let reads = Rc::new(Cell::new(0));
        let mut source = ProbeSource {
            reads: Rc::clone(&reads),
        };
        let (sink, counters) = probe_sink(ProbeSinkFailure::Never);
        let error = store
            .ingest_plan_source_with_sink(plan, &mut source, sink)
            .expect_err("malicious plan must fail preflight");
        assert_eq!(reads.get(), 0, "preflight failure read from source");
        assert_eq!(counters[0].get(), 0, "preflight failure prepared sink");
        assert_eq!(counters[1].get(), 0, "preflight failure wrote sink");
        assert_eq!(counters[2].get(), 0, "preflight failure finished sink");
        before.assert_unchanged(&store);
        error
    }

    #[test]
    fn chunk_plan_digest_depends_on_ordered_chunk_metadata() {
        let first = CarChunk {
            offset: 0,
            length: 4,
            digest: [1; 32],
            taikai_segment_hint: None,
        };
        let second = CarChunk {
            offset: 4,
            length: 4,
            digest: [2; 32],
            taikai_segment_hint: None,
        };

        let digest = compute_chunk_plan_digest_sha3(&[first.clone(), second.clone()]);
        let repeated = compute_chunk_plan_digest_sha3(&[first.clone(), second.clone()]);
        let reordered = compute_chunk_plan_digest_sha3(&[second, first.clone()]);

        let mut content_changed = first;
        content_changed.digest[0] ^= 1;
        let content_changed = compute_chunk_plan_digest_sha3(&[
            content_changed,
            CarChunk {
                offset: 4,
                length: 4,
                digest: [2; 32],
                taikai_segment_hint: None,
            },
        ]);

        assert_eq!(digest, repeated);
        assert_ne!(digest, reordered);
        assert_ne!(digest, content_changed);
    }

    #[cfg(feature = "manifest")]
    fn sample_manifest() -> DaManifestV1 {
        use iroha_data_model::{
            da::{
                manifest::{ChunkCommitment, ChunkRole},
                types::{
                    BlobClass, BlobCodec, BlobDigest, ChunkDigest, DaRentQuote, ErasureProfile,
                    ExtraMetadata, MetadataEntry, MetadataVisibility, RetentionPolicy,
                    StorageTicketId,
                },
            },
            nexus::LaneId,
        };

        let chunk_digest = ChunkDigest::new([0xAA; 32]);
        let chunk = ChunkCommitment::new_with_role(0, 0, 8, chunk_digest, ChunkRole::Data, 0);
        let metadata = ExtraMetadata {
            items: vec![
                MetadataEntry::new(
                    "taikai.event_id",
                    b"demo-event".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.stream_id",
                    b"primary-stream".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.rendition_id",
                    b"main-1080p".to_vec(),
                    MetadataVisibility::Public,
                ),
                MetadataEntry::new(
                    "taikai.segment.sequence",
                    b"42".to_vec(),
                    MetadataVisibility::Public,
                ),
            ],
        };
        DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: BlobDigest::new([0x11; 32]),
            lane_id: LaneId::new(7),
            epoch: 1,
            blob_class: BlobClass::TaikaiSegment,
            codec: BlobCodec::new(String::from("custom.binary")),
            blob_hash: BlobDigest::new([0x22; 32]),
            chunk_root: BlobDigest::new([0x33; 32]),
            storage_ticket: StorageTicketId::new([0x44; 32]),
            total_size: 8,
            chunk_size: 8,
            total_stripes: 1,
            shards_per_stripe: 3,
            erasure_profile: ErasureProfile {
                data_shards: 2,
                parity_shards: 1,
                row_parity_stripes: 0,
                chunk_alignment: 1,
                fec_scheme: iroha_data_model::da::types::FecScheme::Rs12_10,
            },
            retention_policy: RetentionPolicy {
                hot_retention_secs: 10,
                cold_retention_secs: 20,
                required_replicas: 3,
                storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
                governance_tag: iroha_data_model::da::types::GovernanceTag::new(String::from(
                    "da.test",
                )),
            },
            rent_quote: DaRentQuote::default(),
            chunks: vec![chunk],
            ipa_commitment: BlobDigest::new([0x33; 32]),
            metadata,
            issued_at_unix: 123,
        }
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn build_plan_from_da_manifest_matches_manifest() {
        use blake3::Hash as BlakeHash;

        let manifest = sample_manifest();
        let plan = build_plan_from_da_manifest(&manifest).expect("plan from manifest");
        assert_eq!(plan.content_length, manifest.total_size);
        assert_eq!(
            plan.payload_digest.as_bytes(),
            BlakeHash::from(*manifest.blob_hash.as_ref()).as_bytes()
        );
        assert_eq!(plan.chunks.len(), manifest.chunks.len());
        assert_eq!(plan.chunks[0].offset, manifest.chunks[0].offset);
        assert_eq!(plan.chunks[0].length, manifest.chunks[0].length);
        assert_eq!(
            plan.chunks[0].digest,
            *manifest.chunks[0].commitment.as_ref()
        );
        let hint = plan.chunks[0]
            .taikai_segment_hint
            .as_ref()
            .expect("taikai hint present");
        assert_eq!(hint.event, "demo-event");
        assert_eq!(hint.stream, "primary-stream");
        assert_eq!(hint.rendition, "main-1080p");
        assert_eq!(hint.sequence, 42);
        assert_eq!(plan.files.len(), 1);
        assert_eq!(plan.files[0].chunk_count, manifest.chunks.len());
        assert_eq!(plan.files[0].size, manifest.total_size);
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn build_plan_from_da_manifest_uses_cache_hint_metadata() {
        use iroha_data_model::da::types::MetadataVisibility;

        let mut manifest = sample_manifest();
        let payload_digest = [0xAB; 32];
        let cache_hint = format!(
            "{{\"event\":\"demo-event\",\"stream\":\"primary-stream\",\"rendition\":\"main-1080p\",\"sequence\":42,\"payload_len\":4096,\"payload_blake3_hex\":\"{}\"}}",
            "ab".repeat(32)
        );
        manifest
            .metadata
            .items
            .push(iroha_data_model::da::types::MetadataEntry::new(
                META_TAIKAI_CACHE_HINT,
                cache_hint.into_bytes(),
                MetadataVisibility::Public,
            ));

        let plan = build_plan_from_da_manifest(&manifest).expect("plan from manifest");
        let hint = plan.chunks[0]
            .taikai_segment_hint
            .as_ref()
            .expect("hint present");
        assert_eq!(hint.payload_len, Some(4096));
        assert_eq!(hint.payload_digest, Some(payload_digest));
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn build_plan_from_da_manifest_errors_on_missing_taikai_metadata() {
        let mut manifest = sample_manifest();
        manifest
            .metadata
            .items
            .retain(|entry| entry.key != "taikai.segment.sequence");

        let err = build_plan_from_da_manifest(&manifest).expect_err("missing metadata rejected");
        assert_eq!(
            err,
            PlanFromManifestError::MissingTaikaiMetadata("taikai.segment.sequence")
        );
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn build_plan_from_da_manifest_errors_on_encrypted_taikai_metadata() {
        use iroha_data_model::da::types::MetadataEncryption;

        let mut manifest = sample_manifest();
        if let Some(entry) = manifest
            .metadata
            .items
            .iter_mut()
            .find(|entry| entry.key == META_TAIKAI_EVENT_ID)
        {
            entry.encryption = MetadataEncryption::ChaCha20Poly1305(Default::default());
        } else {
            panic!("taikai event id metadata entry missing");
        }

        let err = build_plan_from_da_manifest(&manifest).expect_err("encrypted metadata rejected");
        assert!(matches!(
            err,
            PlanFromManifestError::InvalidTaikaiMetadata { field, .. }
            if field == META_TAIKAI_EVENT_ID
        ));
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn taikai_sequence_requires_exact_canonical_unsigned_decimal() {
        for invalid in [
            "",
            " 42",
            "42 ",
            "+42",
            "-1",
            "00",
            "042",
            "4_2",
            "18446744073709551616",
        ] {
            assert!(matches!(
                parse_canonical_taikai_sequence(invalid),
                Err(PlanFromManifestError::InvalidTaikaiMetadata {
                    field: META_TAIKAI_SEGMENT_SEQUENCE,
                    ..
                })
            ));
        }
        assert_eq!(parse_canonical_taikai_sequence("0").expect("zero"), 0);
        assert_eq!(
            parse_canonical_taikai_sequence("18446744073709551615").expect("u64 max"),
            u64::MAX
        );

        let mut manifest = sample_manifest();
        let sequence = manifest
            .metadata
            .items
            .iter_mut()
            .find(|entry| entry.key == META_TAIKAI_SEGMENT_SEQUENCE)
            .expect("sequence metadata");
        sequence.value = b" 42".to_vec();
        assert!(matches!(
            build_plan_from_da_manifest(&manifest),
            Err(PlanFromManifestError::InvalidTaikaiMetadata {
                field: META_TAIKAI_SEGMENT_SEQUENCE,
                ..
            })
        ));
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn taikai_metadata_rejects_duplicate_and_oversized_fields() {
        let mut duplicate = sample_manifest();
        let event = duplicate
            .metadata
            .items
            .iter()
            .find(|entry| entry.key == META_TAIKAI_EVENT_ID)
            .expect("event metadata")
            .clone();
        duplicate.metadata.items.push(event);
        assert!(matches!(
            build_plan_from_da_manifest(&duplicate),
            Err(PlanFromManifestError::InvalidTaikaiMetadata {
                field: META_TAIKAI_EVENT_ID,
                ..
            })
        ));

        let mut oversized = sample_manifest();
        let event = oversized
            .metadata
            .items
            .iter_mut()
            .find(|entry| entry.key == META_TAIKAI_EVENT_ID)
            .expect("event metadata");
        event.value = vec![b'a'; TAIKAI_NAME_MAX_BYTES + 1];
        assert!(matches!(
            build_plan_from_da_manifest(&oversized),
            Err(PlanFromManifestError::InvalidTaikaiMetadata {
                field: META_TAIKAI_EVENT_ID,
                ..
            })
        ));
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn manifest_plan_preflight_rejects_geometry_before_construction() {
        let mut manifest = sample_manifest();
        manifest.chunks[0].offset = 1;
        assert!(matches!(
            build_plan_from_da_manifest(&manifest),
            Err(PlanFromManifestError::InvalidPlan(
                CarPlanValidationError::NonContiguousChunk { chunk_index: 0, .. }
            ))
        ));

        let mut manifest = sample_manifest();
        manifest.total_size += 1;
        assert!(matches!(
            build_plan_from_da_manifest(&manifest),
            Err(PlanFromManifestError::InvalidPlan(
                CarPlanValidationError::ContentLengthMismatch { .. }
            ))
        ));

        let mut manifest = sample_manifest();
        manifest.chunk_size = CHUNK_STORE_MAX_CHUNK_BYTES + 1;
        assert!(matches!(
            build_plan_from_da_manifest(&manifest),
            Err(PlanFromManifestError::InvalidPlan(
                CarPlanValidationError::ChunkProfileTooLarge { .. }
            ))
        ));
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn manifest_plan_heap_estimate_rejects_multiplicative_hint_bombs() {
        let hint = TaikaiSegmentHint {
            event: "e".repeat(1024),
            stream: "s".repeat(1024),
            rendition: "r".repeat(1024),
            ..TaikaiSegmentHint::default()
        };
        assert!(matches!(
            preflight_manifest_plan_heap(CAR_PLAN_MAX_CHUNKS, Some(&hint)),
            Err(PlanFromManifestError::EstimatedPlanHeapLimitExceeded { .. })
        ));
        assert!(matches!(
            preflight_manifest_plan_heap(usize::MAX, None),
            Err(PlanFromManifestError::EstimatedPlanHeapLimitExceeded { .. })
        ));
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn build_plan_from_da_manifest_errors_on_invalid_chunks() {
        use iroha_data_model::da::types::{BlobDigest, RetentionPolicy};

        let mut manifest = sample_manifest();
        manifest.chunks.clear();
        assert_eq!(
            build_plan_from_da_manifest(&manifest),
            Err(PlanFromManifestError::EmptyChunks)
        );

        manifest.chunks = sample_manifest().chunks;
        manifest.chunk_size = 0;
        assert_eq!(
            build_plan_from_da_manifest(&manifest),
            Err(PlanFromManifestError::ZeroChunkSize)
        );

        // ensure other fields don't affect validation by restoring chunk size
        manifest.chunk_size = 8;
        manifest.total_size = 8;
        manifest.retention_policy = RetentionPolicy {
            hot_retention_secs: 1,
            cold_retention_secs: 2,
            required_replicas: 1,
            storage_class: iroha_data_model::sorafs::pin_registry::StorageClass::Hot,
            governance_tag: iroha_data_model::da::types::GovernanceTag::new(String::from("da.alt")),
        };
        manifest.client_blob_id = BlobDigest::new([0x55; 32]);
        let plan = build_plan_from_da_manifest(&manifest).expect("plan");
        assert_eq!(plan.chunks.len(), manifest.chunks.len());
    }

    #[test]
    fn chunk_store_ingest_matches_fixture() {
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        let mut store = ChunkStore::new();
        store.ingest_bytes(&vectors.input).expect("ingest fixture");
        assert_eq!(
            store.payload_digest().as_bytes(),
            blake3::hash(&vectors.input).as_bytes()
        );
        let chunks = store.chunks();
        assert_eq!(chunks.len(), vectors.chunk_lengths.len());
        for (idx, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.offset as usize, vectors.chunk_offsets[idx]);
            assert_eq!(chunk.length as usize, vectors.chunk_lengths[idx]);
            assert_eq!(chunk.blake3, vectors.chunk_digests_blake3[idx]);
        }

        let por_tree = store.por_tree();
        assert!(!por_tree.is_empty(), "expected PoR tree for fixture data");
        assert_eq!(por_tree.chunks().len(), chunks.len());
        assert_eq!(por_tree.payload_len(), store.payload_len());

        let mut expected_chunk_roots = Vec::new();
        for (idx, chunk) in por_tree.chunks().iter().enumerate() {
            assert_eq!(chunk.chunk_index, idx);
            assert_eq!(chunk.offset, chunks[idx].offset);
            assert_eq!(chunk.length, chunks[idx].length);
            assert_eq!(chunk.chunk_digest, chunks[idx].blake3);

            let mut chunk_total = 0u64;
            let mut segment_roots = Vec::new();
            for segment in &chunk.segments {
                assert!(
                    segment.length as usize <= POR_SEGMENT_SIZE,
                    "segment exceeds 64 KiB window"
                );
                let mut segment_total = 0u64;
                let mut leaf_roots = Vec::new();
                for leaf in &segment.leaves {
                    assert!(
                        leaf.length as usize <= POR_LEAF_SIZE,
                        "leaf exceeds 4 KiB window"
                    );
                    let start = leaf.offset as usize;
                    let end = start + leaf.length as usize;
                    let expected_leaf =
                        hash_leaf(leaf.flat_index, leaf.offset, &vectors.input[start..end]);
                    assert_eq!(leaf.digest, expected_leaf);
                    segment_total += leaf.length as u64;
                    leaf_roots.push(leaf.digest);
                }
                assert_eq!(segment_total as u32, segment.length);
                let expected_segment = hash_segment(segment.offset, segment.length, &leaf_roots);
                assert_eq!(segment.digest, expected_segment);
                chunk_total += segment.length as u64;
                segment_roots.push(segment.digest);
            }
            assert_eq!(chunk_total as u32, chunk.length);
            let expected_chunk = hash_chunk(
                chunk.chunk_index as u64,
                chunk.offset,
                chunk.length,
                &chunk.chunk_digest,
                &segment_roots,
            );
            assert_eq!(chunk.root, expected_chunk);
            expected_chunk_roots.push(chunk.root);
        }

        let expected_levels =
            build_chunk_merkle_levels(&expected_chunk_roots).expect("build expected chunk tree");
        let expected_chunk_tree_root = expected_levels
            .last()
            .and_then(|level| level.first())
            .copied()
            .unwrap_or(expected_chunk_roots[0]);
        let expected_root = hash_root(
            store.payload_len(),
            expected_chunk_roots.len(),
            por_tree.leaf_count_u64(),
            &expected_chunk_tree_root,
        )
        .expect("hash expected PoR root");
        assert_eq!(por_tree.root(), &expected_root);
    }

    #[test]
    fn chunk_store_persists_chunks_to_directory() {
        let payload = b"deterministic chunk sink payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut store = ChunkStore::with_profile(plan.chunk_profile);
        let base = tempdir().expect("temp dir");
        let dir = fs::canonicalize(base.path())
            .expect("canonical base")
            .join("chunks");

        let mut source = InMemoryPayload::new(payload);
        let output = store
            .ingest_plan_to_directory(&plan, &mut source, &dir)
            .expect("ingest directory");

        assert_eq!(output.total_bytes, plan.content_length);
        assert_eq!(output.records.len(), plan.chunks.len());
        assert_eq!(output.publication, DirectoryPublicationStatus::Durable);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            assert_eq!(
                fs::metadata(&dir)
                    .expect("sink directory metadata")
                    .permissions()
                    .mode()
                    & 0o077,
                0,
                "published sink directory must remain private"
            );
        }

        for record in &output.records {
            let chunk_path = dir.join(&record.file_name);
            let bytes = fs::read(&chunk_path).expect("chunk file");
            assert_eq!(bytes.len(), record.length as usize);
            assert_eq!(blake3::hash(&bytes).as_bytes(), &record.digest);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;

                assert_eq!(
                    fs::metadata(&chunk_path)
                        .expect("chunk metadata")
                        .permissions()
                        .mode()
                        & 0o077,
                    0,
                    "persisted chunk must remain private"
                );
            }
        }

        assert_eq!(
            store.payload_digest().as_bytes(),
            blake3::hash(payload).as_bytes()
        );
    }

    #[test]
    fn chunk_store_stream_persist_consumes_reader() {
        let payload = b"streamable payload for chunk sink".repeat(4);
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let mut store = ChunkStore::with_profile(plan.chunk_profile);
        let base = tempdir().expect("dir");
        let dir = fs::canonicalize(base.path())
            .expect("canonical base")
            .join("chunks");
        let mut reader = Cursor::new(payload.clone());

        let output = store
            .ingest_plan_stream_to_directory(&plan, &mut reader, &dir)
            .expect("stream ingest");

        assert_eq!(output.total_bytes, plan.content_length);
        assert_eq!(output.records.len(), plan.chunks.len());
        assert_eq!(output.publication, DirectoryPublicationStatus::Durable);
        assert_eq!(reader.position(), payload.len() as u64);
    }

    #[test]
    fn ingest_preflight_rejects_u32_max_chunk_before_allocation_or_io() {
        let payload = b"x";
        let mut plan = CarBuildPlan::single_file(payload).expect("plan");
        plan.chunks[0].length = u32::MAX;
        plan.content_length = u64::from(u32::MAX);

        let error = reject_before_io(&plan);
        assert!(matches!(
            error,
            ChunkStoreError::InvalidPlan(CarPlanValidationError::ChunkTooLarge {
                chunk_index: 0,
                length: u32::MAX,
                limit,
            }) if limit == ChunkProfile::DEFAULT.max_size as u32
        ));
    }

    #[test]
    fn ingest_preflight_rejects_chunk_longer_than_declared_profile() {
        let payload = b"x";
        let mut plan = CarBuildPlan::single_file(payload).expect("plan");
        plan.chunk_profile = ChunkProfile {
            min_size: 1,
            target_size: 1,
            max_size: 1,
            break_mask: 1,
        };
        plan.chunks[0].length = 2;
        plan.content_length = 2;

        let error = reject_before_io(&plan);
        assert!(matches!(
            error,
            ChunkStoreError::InvalidPlan(CarPlanValidationError::ChunkTooLarge {
                chunk_index: 0,
                length: 2,
                limit: 1,
            })
        ));
    }

    #[test]
    fn ingest_preflight_rejects_noncontiguous_offsets_before_io() {
        let payload = b"ab";
        let mut plan = CarBuildPlan::single_file(payload).expect("plan");
        plan.chunks[0].offset = 1;
        plan.content_length = 3;

        let error = reject_before_io(&plan);
        assert!(matches!(
            error,
            ChunkStoreError::InvalidPlan(CarPlanValidationError::NonContiguousChunk {
                chunk_index: 0,
                expected: 0,
                actual: 1
            })
        ));
    }

    #[test]
    fn ingest_preflight_rejects_overflowing_chunk_range_before_io() {
        let payload = b"ab";
        let mut plan = CarBuildPlan::single_file(payload).expect("plan");
        plan.chunks.push(CarChunk {
            offset: u64::MAX - 1,
            length: 4,
            digest: [0; 32],
            taikai_segment_hint: None,
        });
        plan.content_length = u64::MAX;

        let error = reject_before_io(&plan);
        assert!(matches!(
            error,
            ChunkStoreError::InvalidPlan(CarPlanValidationError::ChunkRangeOverflow {
                chunk_index: 1,
            })
        ));
    }

    #[test]
    fn ingest_preflight_rejects_zero_length_and_empty_plans_before_io() {
        let payload = b"x";
        let mut zero = CarBuildPlan::single_file(payload).expect("plan");
        zero.chunks[0].length = 0;
        assert!(matches!(
            reject_before_io(&zero),
            ChunkStoreError::InvalidPlan(CarPlanValidationError::ZeroLengthChunk {
                chunk_index: 0
            })
        ));

        let mut empty = CarBuildPlan::single_file(payload).expect("plan");
        empty.chunks.clear();
        assert!(matches!(
            reject_before_io(&empty),
            ChunkStoreError::InvalidPlan(CarPlanValidationError::MissingChunks)
        ));
    }

    #[test]
    fn ingest_preflight_rejects_content_length_mismatch_before_io() {
        let payload = b"payload";
        let mut plan = CarBuildPlan::single_file(payload).expect("plan");
        plan.content_length += 1;
        let error = reject_before_io(&plan);
        assert!(matches!(
            error,
            ChunkStoreError::InvalidPlan(CarPlanValidationError::ContentLengthMismatch { .. })
        ));
    }

    #[test]
    fn short_read_preserves_preexisting_chunk_store_state() {
        let payload = b"replacement payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut store = ChunkStore::new();
        store
            .ingest_bytes(b"pre-existing store state")
            .expect("seed store");
        let before = StoreSnapshot::capture(&store);
        let mut reader = Cursor::new(&payload[..payload.len() - 1]);

        let error = store
            .ingest_plan_stream(&plan, &mut reader)
            .expect_err("short read must fail");
        assert!(matches!(
            error,
            ChunkStoreError::UnexpectedEof { chunk_index: 0, .. }
        ));
        before.assert_unchanged(&store);
    }

    #[test]
    fn trailing_stream_data_preserves_preexisting_chunk_store_state() {
        let payload = b"replacement payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut store = ChunkStore::new();
        store
            .ingest_bytes(b"pre-existing store state")
            .expect("seed store");
        let before = StoreSnapshot::capture(&store);
        let mut with_trailer = payload.to_vec();
        with_trailer.extend_from_slice(b"attacker trailer");
        let mut reader = Cursor::new(with_trailer);

        let error = store
            .ingest_plan_stream(&plan, &mut reader)
            .expect_err("trailing data must fail");
        assert!(matches!(
            error,
            ChunkStoreError::LengthMismatch {
                expected,
                actual,
            } if expected == plan.content_length && actual == plan.content_length + 1
        ));
        before.assert_unchanged(&store);
    }

    #[test]
    fn chunk_and_payload_digest_failures_preserve_preexisting_store_state() {
        let payload = b"replacement payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut store = ChunkStore::new();
        store
            .ingest_bytes(b"pre-existing store state")
            .expect("seed store");
        let before = StoreSnapshot::capture(&store);

        let mut corrupted = payload.to_vec();
        corrupted[0] ^= 0xff;
        let mut source = InMemoryPayload::new(&corrupted);
        assert!(matches!(
            store.ingest_plan_source(&plan, &mut source),
            Err(ChunkStoreError::DigestMismatch { chunk_index: 0 })
        ));
        before.assert_unchanged(&store);

        let mut wrong_payload_digest = plan.clone();
        wrong_payload_digest.payload_digest = blake3::hash(b"wrong payload digest");
        let mut source = InMemoryPayload::new(payload);
        assert!(matches!(
            store.ingest_plan_source(&wrong_payload_digest, &mut source),
            Err(ChunkStoreError::PayloadDigestMismatch)
        ));
        before.assert_unchanged(&store);
    }

    #[test]
    fn sink_write_and_finish_failures_preserve_preexisting_store_state() {
        let payload = b"replacement payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut store = ChunkStore::new();
        store
            .ingest_bytes(b"pre-existing store state")
            .expect("seed store");
        let before = StoreSnapshot::capture(&store);

        let (sink, counters) = probe_sink(ProbeSinkFailure::Write(0));
        let mut source = InMemoryPayload::new(payload);
        assert!(matches!(
            store.ingest_plan_source_with_sink(&plan, &mut source, sink),
            Err(ChunkStoreError::Io(_))
        ));
        assert_eq!(counters[0].get(), 1);
        assert_eq!(counters[1].get(), 1);
        assert_eq!(counters[2].get(), 0);
        before.assert_unchanged(&store);

        let (sink, counters) = probe_sink(ProbeSinkFailure::Finish);
        let mut source = InMemoryPayload::new(payload);
        assert!(matches!(
            store.ingest_plan_source_with_sink(&plan, &mut source, sink),
            Err(ChunkStoreError::Io(_))
        ));
        assert_eq!(counters[0].get(), 1);
        assert_eq!(counters[1].get(), plan.chunks.len());
        assert_eq!(counters[2].get(), 1);
        before.assert_unchanged(&store);
    }

    #[test]
    fn directory_sink_rejects_existing_directory_without_touching_it() {
        let base = tempdir().expect("base");
        let root = fs::canonicalize(base.path())
            .expect("canonical base")
            .join("chunks");
        fs::create_dir(&root).expect("old root");
        fs::write(root.join("sentinel"), b"old state").expect("old sentinel");

        let payload = b"abc";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut source = InMemoryPayload::new(payload);
        let mut store = ChunkStore::new();
        store
            .ingest_bytes(b"pre-existing store state")
            .expect("seed store");
        let before = StoreSnapshot::capture(&store);

        assert!(matches!(
            store.ingest_plan_to_directory(&plan, &mut source, &root),
            Err(ChunkStoreError::Io(_))
        ));
        before.assert_unchanged(&store);
        assert_eq!(
            fs::read(root.join("sentinel")).expect("sentinel"),
            b"old state"
        );
        let staged = fs::read_dir(base.path())
            .expect("base listing")
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.starts_with(".sorafs-chunks."))
            .collect::<Vec<_>>();
        assert!(staged.is_empty(), "orphan staging paths: {staged:?}");
    }

    #[test]
    fn directory_sink_detects_destination_collision_before_commit() {
        let base = tempdir().expect("base");
        let root = fs::canonicalize(base.path())
            .expect("canonical base")
            .join("chunks");
        let payload = b"payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut sink = DirectoryChunkSink::new(&root);
        sink.prepare(&plan).expect("prepare");
        sink.write_chunk(0, &plan.chunks[0], payload)
            .expect("staged chunk");

        fs::create_dir(&root).expect("attacker collision root");
        fs::write(root.join("sentinel"), b"attacker state").expect("sentinel");
        assert!(matches!(sink.finish(), Err(ChunkStoreError::Io(_))));
        assert_eq!(
            fs::read(root.join("sentinel")).expect("sentinel"),
            b"attacker state"
        );
    }

    #[test]
    fn directory_sink_rejects_incomplete_and_malformed_direct_writes() {
        let base = tempdir().expect("base");
        let payload = b"validated payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");

        let canonical_base = fs::canonicalize(base.path()).expect("canonical base");
        let incomplete_root = canonical_base.join("incomplete");
        let mut incomplete = DirectoryChunkSink::new(&incomplete_root);
        incomplete.prepare(&plan).expect("prepare incomplete sink");
        assert!(matches!(
            incomplete.finish(),
            Err(ChunkStoreError::SinkIncomplete { .. })
        ));
        assert!(!incomplete_root.exists());

        let root = canonical_base.join("malformed");
        let mut sink = DirectoryChunkSink::new(&root);
        sink.prepare(&plan).expect("prepare sink");
        assert!(matches!(
            sink.write_chunk(1, &plan.chunks[0], payload),
            Err(ChunkStoreError::SinkChunkOrder {
                expected: 0,
                actual: 1
            })
        ));

        let mut wrong_metadata = plan.chunks[0].clone();
        wrong_metadata.offset = 1;
        assert!(matches!(
            sink.write_chunk(0, &wrong_metadata, payload),
            Err(ChunkStoreError::SinkChunkMetadataMismatch { chunk_index: 0 })
        ));
        assert!(matches!(
            sink.write_chunk(0, &plan.chunks[0], &payload[..payload.len() - 1]),
            Err(ChunkStoreError::SinkChunkLengthMismatch { chunk_index: 0, .. })
        ));
        let mut wrong_bytes = payload.to_vec();
        wrong_bytes[0] ^= 0xff;
        assert!(matches!(
            sink.write_chunk(0, &plan.chunks[0], &wrong_bytes),
            Err(ChunkStoreError::SinkChunkDigestMismatch { chunk_index: 0 })
        ));
        sink.write_chunk(0, &plan.chunks[0], payload)
            .expect("valid chunk after rejected attempts");
        sink.finish().expect("complete sink");
        assert_eq!(
            fs::read(root.join("chunk_00000.bin")).expect("published chunk"),
            payload
        );
    }

    #[test]
    fn directory_sink_rechecks_staged_chunk_before_publication() {
        let base = tempdir().expect("base");
        let root = fs::canonicalize(base.path())
            .expect("canonical base")
            .join("chunks");
        let payload = b"payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut sink = DirectoryChunkSink::new(&root);
        sink.prepare(&plan).expect("prepare");
        sink.write_chunk(0, &plan.chunks[0], payload)
            .expect("write chunk");
        let staging = sink.staging_root.clone().expect("staging");
        fs::write(staging.join("chunk_00000.bin"), b"tamper!").expect("tamper staged chunk");
        assert!(matches!(
            sink.finish(),
            Err(ChunkStoreError::SinkChunkDigestMismatch { chunk_index: 0 })
        ));
        assert!(!root.exists());
    }

    #[test]
    fn post_rename_failures_return_non_retryable_published_outcome() {
        for (name, fault) in [
            ("identity", DirectoryCommitFault::PostRenameIdentity),
            ("parent-sync", DirectoryCommitFault::ParentSync),
        ] {
            let base = tempdir().expect("base");
            let root = fs::canonicalize(base.path())
                .expect("canonical base")
                .join(name);
            let payload = b"published payload";
            let plan = CarBuildPlan::single_file(payload).expect("plan");
            let mut sink = DirectoryChunkSink::new(&root);
            sink.commit_fault = Some(fault);
            let mut source = InMemoryPayload::new(payload);
            let mut store = ChunkStore::new();
            store.ingest_bytes(b"old").expect("seed store");

            let output = store
                .ingest_plan_source_with_sink(&plan, &mut source, sink)
                .expect("published outcome is successful");
            assert_eq!(
                output.publication,
                DirectoryPublicationStatus::PublishedButDurabilityUncertain
            );
            assert_eq!(store.payload_digest(), &blake3::hash(payload));
            assert_eq!(
                fs::read(root.join("chunk_00000.bin")).expect("published chunk"),
                payload
            );

            let after = StoreSnapshot::capture(&store);
            let mut source = InMemoryPayload::new(payload);
            assert!(matches!(
                store.ingest_plan_to_directory(&plan, &mut source, &root),
                Err(ChunkStoreError::Io(_))
            ));
            after.assert_unchanged(&store);
        }
    }

    #[test]
    fn atomic_chunk_write_ignores_predictable_partial_collision() {
        let dir = tempdir().expect("dir");
        let output = dir.path().join("chunk.bin");
        let stale_partial = dir.path().join("chunk.bin.partial");
        let victim = dir.path().join("victim");
        fs::write(&victim, b"victim").expect("victim");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&victim, &stale_partial).expect("partial symlink");
        #[cfg(not(unix))]
        fs::write(&stale_partial, b"collision").expect("partial collision");

        DirectoryChunkSink::write_atomic(&output, b"new chunk").expect("atomic write");
        assert_eq!(fs::read(&output).expect("output"), b"new chunk");
        assert_eq!(fs::read(&victim).expect("victim"), b"victim");
        assert!(fs::symlink_metadata(&stale_partial).is_ok());
        let random_partials = fs::read_dir(dir.path())
            .expect("listing")
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| name.starts_with(".chunk.bin.") && name.ends_with(".partial"))
            .collect::<Vec<_>>();
        assert!(
            random_partials.is_empty(),
            "orphan random partials: {random_partials:?}"
        );
    }

    #[test]
    fn atomic_chunk_write_rejects_existing_file_collision() {
        let dir = tempdir().expect("dir");
        let output = dir.path().join("chunk.bin");
        fs::write(&output, b"old chunk").expect("old chunk");
        assert!(matches!(
            DirectoryChunkSink::write_atomic(&output, b"new chunk"),
            Err(ChunkStoreError::Io(_))
        ));
        assert_eq!(fs::read(&output).expect("output"), b"old chunk");
    }

    #[cfg(unix)]
    #[test]
    fn atomic_chunk_write_rejects_symlink_and_hardlink_destinations() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().expect("dir");
        let victim = dir.path().join("victim");
        fs::write(&victim, b"victim").expect("victim");

        let symlink_path = dir.path().join("symlink-chunk");
        symlink(&victim, &symlink_path).expect("symlink");
        assert!(matches!(
            DirectoryChunkSink::write_atomic(&symlink_path, b"attack"),
            Err(ChunkStoreError::Io(_))
        ));
        assert_eq!(fs::read(&victim).expect("victim"), b"victim");

        let hardlink_path = dir.path().join("hardlink-chunk");
        fs::hard_link(&victim, &hardlink_path).expect("hard link");
        assert!(matches!(
            DirectoryChunkSink::write_atomic(&hardlink_path, b"attack"),
            Err(ChunkStoreError::Io(_))
        ));
        assert_eq!(fs::read(&victim).expect("victim"), b"victim");
    }

    #[cfg(unix)]
    #[test]
    fn directory_sink_rejects_symlink_root_without_touching_target() {
        use std::os::unix::fs::symlink;

        let base = tempdir().expect("base");
        let canonical_base = fs::canonicalize(base.path()).expect("canonical base");
        let victim = canonical_base.join("victim");
        fs::create_dir(&victim).expect("victim directory");
        fs::write(victim.join("sentinel"), b"victim").expect("sentinel");
        let root = canonical_base.join("chunks");
        symlink(&victim, &root).expect("root symlink");

        let payload = b"payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut source = InMemoryPayload::new(payload);
        let mut store = ChunkStore::new();
        store
            .ingest_bytes(b"pre-existing store state")
            .expect("seed store");
        let before = StoreSnapshot::capture(&store);
        assert!(matches!(
            store.ingest_plan_to_directory(&plan, &mut source, &root),
            Err(ChunkStoreError::Io(_))
        ));
        before.assert_unchanged(&store);
        assert_eq!(
            fs::read(victim.join("sentinel")).expect("sentinel"),
            b"victim"
        );
    }

    #[cfg(unix)]
    #[test]
    fn directory_sink_detects_staging_symlink_replacement_without_following_it() {
        use std::os::unix::fs::symlink;

        let base = tempdir().expect("base");
        let canonical_base = fs::canonicalize(base.path()).expect("canonical base");
        let root = canonical_base.join("chunks");
        let victim = canonical_base.join("victim");
        fs::create_dir(&victim).expect("victim directory");
        fs::write(victim.join("sentinel"), b"victim").expect("sentinel");
        let payload = b"payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut sink = DirectoryChunkSink::new(&root);
        sink.prepare(&plan).expect("prepare");
        let staging = sink.staging_root.clone().expect("staging path");
        let displaced = canonical_base.join("displaced-staging");
        fs::rename(&staging, &displaced).expect("displace staging");
        symlink(&victim, &staging).expect("replace staging with symlink");

        assert!(matches!(
            sink.write_chunk(0, &plan.chunks[0], payload),
            Err(ChunkStoreError::Io(_))
        ));
        drop(sink);
        assert_eq!(
            fs::read(victim.join("sentinel")).expect("sentinel"),
            b"victim"
        );
        assert!(
            fs::symlink_metadata(&staging)
                .expect("replacement remains")
                .file_type()
                .is_symlink()
        );
    }

    #[cfg(unix)]
    #[test]
    fn directory_sink_rejects_symlinked_intermediate_parent() {
        use std::os::unix::fs::symlink;

        let base = tempdir().expect("base");
        let canonical_base = fs::canonicalize(base.path()).expect("canonical base");
        let outside = tempdir().expect("outside");
        let outside_path = fs::canonicalize(outside.path()).expect("canonical outside");
        symlink(&outside_path, canonical_base.join("linked-parent")).expect("parent symlink");
        let root = canonical_base.join("linked-parent").join("chunks");
        let plan = CarBuildPlan::single_file(b"payload").expect("plan");
        let mut sink = DirectoryChunkSink::new(&root);
        assert!(matches!(sink.prepare(&plan), Err(ChunkStoreError::Io(_))));
        assert!(!outside_path.join("chunks").exists());
    }

    #[cfg(unix)]
    #[test]
    fn directory_sink_detects_concurrent_parent_swap_before_write() {
        let base = tempdir().expect("base");
        let canonical_base = fs::canonicalize(base.path()).expect("canonical base");
        let parent = canonical_base.join("parent");
        fs::create_dir(&parent).expect("parent");
        let root = parent.join("chunks");
        let payload = b"payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut sink = DirectoryChunkSink::new(&root);
        sink.prepare(&plan).expect("prepare");

        let displaced = canonical_base.join("displaced-parent");
        fs::rename(&parent, &displaced).expect("displace parent");
        fs::create_dir(&parent).expect("replacement parent");
        fs::write(parent.join("sentinel"), b"replacement").expect("sentinel");
        assert!(matches!(
            sink.write_chunk(0, &plan.chunks[0], payload),
            Err(ChunkStoreError::Io(_))
        ));
        assert_eq!(
            fs::read(parent.join("sentinel")).expect("sentinel"),
            b"replacement"
        );

        fs::remove_dir_all(&parent).expect("remove replacement");
        fs::rename(&displaced, &parent).expect("restore parent");
        drop(sink);
        assert!(!root.exists());
    }

    #[test]
    fn por_proof_verification_succeeds() {
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        let mut store = ChunkStore::new();
        store.ingest_bytes(&vectors.input).expect("ingest fixture");
        let tree = store.por_tree();
        let proof = tree
            .try_prove_leaf(0, 0, 0, &vectors.input)
            .expect("proof construction")
            .expect("proof for first leaf");
        assert!(proof.verify(tree.root()));

        let mut tampered = proof.clone();
        tampered.leaf_bytes[0] ^= 0xFF;
        assert!(!tampered.verify(tree.root()));

        let mut tampered = proof.clone();
        tampered.leaf_length = tampered.leaf_length.saturating_add(1);
        assert!(!tampered.verify(tree.root()));

        let mut tampered = proof.clone();
        tampered.leaf_offset = tampered.leaf_offset.saturating_add(1);
        assert!(!tampered.verify(tree.root()));

        let mut tampered = proof.clone();
        tampered.segment_length = tampered.segment_length.saturating_add(1);
        assert!(!tampered.verify(tree.root()));

        let mut tampered = proof.clone();
        tampered.segment_leaves.push([0x55; 32]);
        assert!(!tampered.verify(tree.root()));

        let mut tampered = proof.clone();
        tampered.leaf_count = tampered.leaf_count.saturating_add(1);
        assert!(!tampered.verify(tree.root()));

        let mut tampered = proof.clone();
        tampered.leaf_index_flat = if tampered.leaf_count > 1 {
            (tampered.leaf_index_flat + 1) % tampered.leaf_count
        } else {
            tampered.leaf_count
        };
        assert!(!tampered.verify(tree.root()));

        let mut tampered = proof;
        tampered.chunk_count = tampered.chunk_count.saturating_add(1);
        assert!(!tampered.verify(tree.root()));
    }

    #[test]
    fn por_proof_above_legacy_chunk_cap_is_logarithmic_and_roundtrips() {
        const CHUNK_COUNT: usize = 2_049;
        let payload = (0..CHUNK_COUNT)
            .map(|index| u8::try_from(index % 251).expect("fixture byte"))
            .collect::<Vec<_>>();
        let chunks = payload
            .iter()
            .enumerate()
            .map(|(index, byte)| StoredChunk {
                offset: u64::try_from(index).expect("fixture offset"),
                length: 1,
                blake3: blake3::hash(core::slice::from_ref(byte)).into(),
            })
            .collect::<Vec<_>>();
        let tree = PorMerkleTree::try_from_payload(&payload, &chunks)
            .expect("build PoR tree above the retired 2,048-chunk cap");
        let proof = tree
            .try_prove_leaf(CHUNK_COUNT - 1, 0, 0, &payload)
            .expect("construct proof")
            .expect("last chunk proof");
        assert_eq!(proof.chunk_count, CHUNK_COUNT as u64);
        assert_eq!(proof.chunk_merkle_path.len(), 12);
        assert!(proof.verify(tree.root()));

        let encoded = por_json::proof_to_value(&proof);
        let decoded = por_json::proof_from_value(&encoded).expect("roundtrip canonical proof JSON");
        assert_eq!(decoded, proof);
        assert!(decoded.verify(tree.root()));
    }

    #[test]
    fn por_direct_construction_enforces_canonical_chunk_bounds_before_work() {
        assert!(matches!(
            ensure_por_chunk_count(CAR_PLAN_MAX_CHUNKS + 1),
            Err(ChunkStoreError::InvalidPlan(
                CarPlanValidationError::TooManyChunks {
                    count,
                    maximum: CAR_PLAN_MAX_CHUNKS,
                }
            )) if count == CAR_PLAN_MAX_CHUNKS + 1
        ));

        let mut payload = vec![0x5a; CHUNK_STORE_MAX_CHUNK_BYTES as usize];
        let canonical_chunk = StoredChunk {
            offset: 0,
            length: CHUNK_STORE_MAX_CHUNK_BYTES,
            blake3: blake3::hash(&payload).into(),
        };
        let tree =
            PorMerkleTree::try_from_payload(&payload, core::slice::from_ref(&canonical_chunk))
                .expect("the exact canonical chunk ceiling is accepted");
        let proof = tree
            .try_prove_leaf(0, 0, 0, &payload)
            .expect("construct proof at chunk ceiling")
            .expect("one proof");
        assert!(proof.verify(tree.root()));

        payload.push(0x5b);
        let oversized_chunk = StoredChunk {
            offset: 0,
            length: CHUNK_STORE_MAX_CHUNK_BYTES + 1,
            blake3: [0; 32],
        };
        assert!(matches!(
            PorMerkleTree::try_from_payload(&payload, &[oversized_chunk]),
            Err(ChunkStoreError::ChunkLengthTooLarge {
                length,
                limit: CHUNK_STORE_MAX_CHUNK_BYTES,
            }) if length == CHUNK_STORE_MAX_CHUNK_BYTES as usize + 1
        ));

        let mut oversized_proof = proof;
        oversized_proof.chunk_length = CHUNK_STORE_MAX_CHUNK_BYTES + 1;
        assert!(!oversized_proof.verify(tree.root()));
    }

    #[test]
    fn por_sampling_is_deterministic_and_unique() {
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        let mut store = ChunkStore::new();
        store.ingest_bytes(&vectors.input).expect("ingest fixture");
        let samples_a = store
            .sample_leaves(8, 12345, &vectors.input)
            .expect("sample leaves");
        let samples_b = store
            .sample_leaves(8, 12345, &vectors.input)
            .expect("sample leaves");
        assert_eq!(samples_a, samples_b, "sampling should be deterministic");
        let total_leaves = store.por_leaf_count();
        assert_eq!(samples_a.len(), 8.min(total_leaves));
        let mut seen = HashSet::new();
        for (flat, proof) in samples_a {
            assert!(seen.insert(flat), "duplicate flat leaf index {flat}");
            assert!(proof.verify(store.por_tree().root()));
            let (chunk_idx, segment_idx, leaf_idx) = store
                .por_tree()
                .leaf_path(flat)
                .expect("flat index resolves");
            assert_eq!(proof.chunk_index, chunk_idx);
            assert_eq!(proof.segment_index, segment_idx);
            assert_eq!(proof.leaf_index, leaf_idx);
        }
    }

    #[test]
    fn por_sampling_collision_resolution_is_deterministic_and_bounded() {
        let leaf_count = 5u64;
        let collision_seed = (0u64..)
            .find(|seed| {
                let first = splitmix64(*seed);
                let second = splitmix64(first);
                first % leaf_count == second % leaf_count
            })
            .expect("small population has a reduced SplitMix collision");
        let first_candidate = splitmix64(collision_seed) % leaf_count;
        let expected_second = (first_candidate + 1) % leaf_count;
        let samples = PorSampleIndices::new(leaf_count, 5, collision_seed)
            .expect("build bounded sample schedule")
            .collect::<Vec<_>>();
        assert_eq!(samples[0], first_candidate);
        assert_eq!(samples[1], expected_second);
        assert_eq!(samples.len(), 5);
        assert_eq!(samples.iter().copied().collect::<HashSet<_>>().len(), 5);
        assert_eq!(
            PorSampleIndices::new(leaf_count, 500, collision_seed)
                .expect("request truncates to population")
                .count(),
            5
        );
    }

    #[test]
    fn sampling_truncates_to_leaf_count() {
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        let mut store = ChunkStore::new();
        store.ingest_bytes(&vectors.input).expect("ingest fixture");
        let total = store.por_leaf_count();
        let samples = store
            .sample_leaves(total + 10, 7, &vectors.input)
            .expect("sample leaves");
        assert_eq!(samples.len(), total);
    }

    #[test]
    fn por_tree_empty_payload() {
        let mut store = ChunkStore::new();
        store.ingest_bytes(&[]).expect("ingest empty payload");
        let tree = store.por_tree();
        assert!(tree.is_empty());
        assert_eq!(tree.root(), &[0u8; 32]);
    }

    #[test]
    fn por_try_from_payload_rejects_gap_overlap_and_digest_mismatch() {
        let payload = b"abcdefgh";
        let first_digest: [u8; 32] = blake3::hash(&payload[..4]).into();
        let second_digest: [u8; 32] = blake3::hash(&payload[4..]).into();
        let first = StoredChunk {
            offset: 0,
            length: 4,
            blake3: first_digest,
        };
        let second = StoredChunk {
            offset: 4,
            length: 4,
            blake3: second_digest,
        };
        PorMerkleTree::try_from_payload(payload, &[first.clone(), second.clone()])
            .expect("canonical two-chunk tree");

        let mut gap = second.clone();
        gap.offset = 5;
        assert!(matches!(
            PorMerkleTree::try_from_payload(payload, &[first.clone(), gap]),
            Err(ChunkStoreError::PorInvariant { .. })
        ));

        let mut overlap = second.clone();
        overlap.offset = 3;
        assert!(matches!(
            PorMerkleTree::try_from_payload(payload, &[first.clone(), overlap]),
            Err(ChunkStoreError::PorInvariant { .. })
        ));

        let mut wrong_digest = second;
        wrong_digest.blake3[0] ^= 0xff;
        assert!(matches!(
            PorMerkleTree::try_from_payload(payload, &[first, wrong_digest]),
            Err(ChunkStoreError::DigestMismatch { chunk_index: 1 })
        ));
    }

    #[test]
    fn por_try_from_payload_rejects_empty_inventory_mismatches() {
        assert_eq!(
            PorMerkleTree::try_from_payload(&[], &[]).expect("canonical empty tree"),
            PorMerkleTree::empty()
        );
        assert!(matches!(
            PorMerkleTree::try_from_payload(b"x", &[]),
            Err(ChunkStoreError::PorInvariant { .. })
        ));
        let chunk = StoredChunk {
            offset: 0,
            length: 1,
            blake3: blake3::hash(b"x").into(),
        };
        assert!(matches!(
            PorMerkleTree::try_from_payload(&[], &[chunk]),
            Err(ChunkStoreError::PorInvariant { .. })
        ));
    }

    #[test]
    fn por_try_from_chunks_rejects_malformed_geometry_and_root_vectors() {
        let payload = vec![0x5a; POR_SEGMENT_SIZE + POR_LEAF_SIZE + 1];
        let chunk = StoredChunk {
            offset: 0,
            length: u32::try_from(payload.len()).expect("test payload fits u32"),
            blake3: blake3::hash(&payload).into(),
        };
        let tree = PorMerkleTree::try_from_payload(&payload, &[chunk]).expect("canonical tree");
        let chunks = tree.chunks.clone();
        let roots = chunks.iter().map(|entry| entry.root).collect::<Vec<_>>();

        let mut mismatched_roots = roots.clone();
        mismatched_roots[0][0] ^= 0xff;
        assert!(matches!(
            PorMerkleTree::try_from_chunks(chunks.clone(), mismatched_roots, tree.payload_len),
            Err(ChunkStoreError::PorInvariant { .. })
        ));
        assert!(matches!(
            PorMerkleTree::try_from_chunks(chunks.clone(), Vec::new(), tree.payload_len),
            Err(ChunkStoreError::PorInvariant { .. })
        ));

        let mut segment_gap = chunks.clone();
        segment_gap[0].segments[1].offset += 1;
        assert!(matches!(
            PorMerkleTree::try_from_chunks(segment_gap, roots.clone(), tree.payload_len),
            Err(ChunkStoreError::PorInvariant { .. })
        ));

        let mut leaf_overlap = chunks.clone();
        leaf_overlap[0].segments[0].leaves[1].offset -= 1;
        assert!(matches!(
            PorMerkleTree::try_from_chunks(leaf_overlap, roots.clone(), tree.payload_len),
            Err(ChunkStoreError::PorInvariant { .. })
        ));

        let mut reordered_flat_index = chunks.clone();
        reordered_flat_index[0].segments[0].leaves[0].flat_index += 1;
        assert!(matches!(
            PorMerkleTree::try_from_chunks(reordered_flat_index, roots.clone(), tree.payload_len),
            Err(ChunkStoreError::PorInvariant { .. })
        ));

        let mut forged_segment = chunks;
        forged_segment[0].segments[0].digest[0] ^= 0xff;
        assert!(matches!(
            PorMerkleTree::try_from_chunks(forged_segment, roots, tree.payload_len),
            Err(ChunkStoreError::PorInvariant { .. })
        ));
    }

    #[test]
    fn por_chunk_builder_rejects_absolute_offset_overflow() {
        let payload = vec![0x11; POR_LEAF_SIZE + 1];
        let length = u32::try_from(payload.len()).expect("test payload fits u32");
        assert!(matches!(
            PorMerkleTree::build_chunk_tree_from_bytes(
                0,
                u64::MAX,
                length,
                blake3::hash(&payload).into(),
                0,
                &payload,
            ),
            Err(ChunkStoreError::PorInvariant { .. })
        ));
    }

    #[test]
    fn por_sampling_propagates_payload_read_failures() {
        let payload = b"non-empty PoR sampling payload";
        let mut store = ChunkStore::new();
        store.ingest_bytes(payload).expect("ingest payload");
        assert!(matches!(
            store.sample_leaves(1, 7, &[]),
            Err(ChunkStoreError::OffsetOutOfRange { .. })
        ));

        let mut tampered = payload.to_vec();
        tampered[0] ^= 0xff;
        assert!(matches!(
            store.sample_leaves(1, 7, &tampered),
            Err(ChunkStoreError::PorProofLeafDigestMismatch { .. })
        ));
    }

    #[test]
    fn por_sampling_rejects_aggregate_proof_heap_before_payload_io() {
        let payload = b"bounded PoR sampling payload";
        let mut store = ChunkStore::new();
        store.ingest_bytes(payload).expect("ingest payload");
        store
            .set_max_estimated_heap_bytes(1)
            .expect("set non-zero sampling limit");
        let reads = Rc::new(Cell::new(0));
        let mut source = ProbeSource {
            reads: Rc::clone(&reads),
        };
        assert!(matches!(
            store.sample_leaves_with(1, 7, &mut source),
            Err(ChunkStoreError::EstimatedHeapLimitExceeded { limit: 1, .. })
        ));
        assert_eq!(reads.get(), 0, "heap rejection must precede payload I/O");
    }

    #[test]
    fn por_tree_can_be_moved_out_without_cloning_or_discarding_chunks() {
        let payload = b"PoR ownership transfer";
        let mut store = ChunkStore::new();
        store.ingest_bytes(payload).expect("ingest payload");
        let expected_root = *store.por_tree().root();
        let chunk_count = store.chunks().len();

        let tree = store.take_por_tree();
        assert_eq!(tree.root(), &expected_root);
        assert!(store.por_tree().is_empty());
        assert_eq!(store.chunks().len(), chunk_count);
        assert!(store.take_por_tree().is_empty());
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn canonical_pdp_roots_match_manifest_tree_across_boundaries() {
        let sizes = [
            1usize,
            4 * 1024 - 1,
            4 * 1024,
            4 * 1024 + 1,
            256 * 1024 - 1,
            256 * 1024,
            256 * 1024 + 1,
            512 * 1024 + 73,
        ];
        for size in sizes {
            let payload = (0..size)
                .map(|index| (index.wrapping_mul(37) & 0xff) as u8)
                .collect::<Vec<_>>();
            let reference = PdpMerkleTreeV1::from_bytes(&payload).expect("reference PDP tree");
            let mut store = ChunkStore::new();
            store
                .try_ingest_bytes(&payload)
                .expect("chunk-store ingest");
            assert_eq!(
                store.pdp_hot_root(),
                Some(reference.hot_root()),
                "size {size}"
            );
            assert_eq!(
                store.pdp_segment_root(),
                Some(reference.segment_root()),
                "size {size}"
            );
            assert_eq!(store.pdp_hot_leaf_count(), reference.hot_leaf_count());
            assert_eq!(store.pdp_segment_count(), reference.segment_count());
        }
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn canonical_pdp_tree_can_be_moved_out_without_disturbing_por_state() {
        let payload = b"canonical PDP ownership transfer";
        let mut store = ChunkStore::new();
        store.ingest_bytes(payload).expect("ingest payload");
        let por_root = *store.por_tree().root();
        let expected_hot_root = store.pdp_hot_root().expect("PDP hot root");

        let tree = store.take_pdp_tree().expect("move PDP tree");
        assert_eq!(tree.hot_root(), expected_hot_root);
        assert!(store.pdp_tree().is_none());
        assert_eq!(store.pdp_hot_root(), None);
        assert_eq!(store.pdp_hot_leaf_count(), 0);
        assert_eq!(store.por_tree().root(), &por_root);
        assert!(store.take_pdp_tree().is_none());
    }

    #[cfg(feature = "manifest")]
    #[test]
    fn streamed_pdp_tree_is_transactional_and_supports_random_access_proofs() {
        let profile = ChunkProfile {
            min_size: 4 * 1024,
            target_size: 4 * 1024,
            max_size: 8 * 1024,
            break_mask: 0xff,
        };
        let payload = (0usize..(300 * 1024 + 17))
            .map(|index| (index.wrapping_mul(19) & 0xff) as u8)
            .collect::<Vec<_>>();
        let plan =
            CarBuildPlan::single_file_with_profile(&payload, profile).expect("streaming plan");
        assert!(plan.chunks.len() > 1);
        let reference = PdpMerkleTreeV1::from_bytes(&payload).expect("reference tree");
        let mut store = ChunkStore::new();
        store.ingest_bytes(b"old state").expect("seed store");
        let before = StoreSnapshot::capture(&store);

        let mut corrupted = payload.clone();
        let second_offset = plan.chunks[1].offset as usize;
        corrupted[second_offset] ^= 0xff;
        let mut source = InMemoryPayload::new(&corrupted);
        assert!(matches!(
            store.ingest_plan_source(&plan, &mut source),
            Err(ChunkStoreError::DigestMismatch { chunk_index: 1 })
        ));
        before.assert_unchanged(&store);

        let mut source = InMemoryPayload::new(&payload);
        store
            .ingest_plan_source(&plan, &mut source)
            .expect("streamed PDP ingest");
        assert_eq!(store.pdp_hot_root(), Some(reference.hot_root()));
        assert_eq!(store.pdp_segment_root(), Some(reference.segment_root()));

        let samples = [PdpSampleV1 {
            segment_index: 1,
            hot_leaf_indices: vec![0, 1],
        }];
        let mut source = InMemoryPayload::new(&payload);
        let proofs = store
            .prove_pdp_samples_with(&samples, &mut source)
            .expect("random-access PDP witnesses");
        let reference_proofs = reference
            .prove_samples(&samples, &payload)
            .expect("reference witnesses");
        assert_eq!(proofs, reference_proofs);

        let mut tampered = payload.clone();
        tampered[256 * 1024] ^= 0xff;
        let mut source = InMemoryPayload::new(&tampered);
        assert!(matches!(
            store.prove_pdp_samples_with(&samples, &mut source),
            Err(PdpMerkleReadError::Tree(
                PdpMerkleTreeError::PayloadDigestMismatch {
                    segment_index: 1,
                    leaf_index: 0
                }
            ))
        ));
    }

    #[test]
    fn ingest_single_file_produces_matching_plan() {
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        let summary = ingest_single_file(&vectors.input).expect("ingest summary");
        assert_eq!(
            summary.chunk_store.payload_digest().as_bytes(),
            blake3::hash(&vectors.input).as_bytes()
        );
        assert_eq!(summary.plan.chunks.len(), vectors.chunk_lengths.len());
        assert_eq!(
            summary.plan.payload_digest.as_bytes(),
            blake3::hash(&vectors.input).as_bytes()
        );
    }

    fn sample_input() -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 << 20);
        let mut state: u64 = 0xDEC0DED;
        for _ in 0..buf.capacity() {
            state = state
                .wrapping_mul(2862933555777941757)
                .wrapping_add(3037000493);
            buf.push((state >> 32) as u8);
        }
        buf
    }

    #[test]
    fn plan_matches_chunker_fixture() {
        let input = sample_input();
        let plan = CarBuildPlan::single_file(&input).expect("plan");
        assert_eq!(plan.content_length, input.len() as u64);
        assert_eq!(plan.chunks.len(), 5);
        assert_eq!(plan.files.len(), 1);
        assert!(plan.files[0].path.is_empty());
        assert_eq!(plan.files[0].chunk_count, plan.chunks.len());
        let lengths: Vec<u32> = plan.chunks.iter().map(|c| c.length).collect();
        assert_eq!(
            lengths,
            vec![177_082, 210_377, 403_145, 187_169, 70_803],
            "chunk lengths drifted"
        );
    }

    #[test]
    fn plan_from_files_orders_and_offsets() {
        let files = vec![
            FileEntry {
                path: vec!["docs".to_owned(), "a.txt".to_owned()],
                data: vec![1u8; 10],
            },
            FileEntry {
                path: vec!["docs".to_owned(), "b.txt".to_owned()],
                data: vec![2u8; 20],
            },
        ];
        let (plan, payload) = CarBuildPlan::from_files(files).expect("plan");
        assert_eq!(plan.files.len(), 2);
        assert_eq!(
            plan.files[0].path,
            vec!["docs".to_owned(), "a.txt".to_owned()]
        );
        assert_eq!(
            plan.files[1].path,
            vec!["docs".to_owned(), "b.txt".to_owned()]
        );
        assert_eq!(plan.files[0].first_chunk, 0);
        assert!(plan.files[1].first_chunk >= plan.files[0].chunk_count);
        assert_eq!(plan.content_length as usize, payload.len());
        assert!(plan.payload_digest.as_bytes().len() == 32);
    }

    #[test]
    fn empty_files_emit_no_chunks_and_zero_content_plan_ingests() {
        let files = vec![
            FileEntry {
                path: vec!["a.empty".to_owned()],
                data: Vec::new(),
            },
            FileEntry {
                path: vec!["b.bin".to_owned()],
                data: b"payload".to_vec(),
            },
            FileEntry {
                path: vec!["c.empty".to_owned()],
                data: Vec::new(),
            },
        ];
        let (plan, payload) = CarBuildPlan::from_files(files).expect("mixed empty plan");
        assert_eq!(plan.chunks.len(), 1);
        assert_eq!(plan.files[0].chunk_count, 0);
        assert_eq!(plan.files[0].first_chunk, 0);
        assert_eq!(plan.files[1].chunk_count, 1);
        assert_eq!(plan.files[1].first_chunk, 0);
        assert_eq!(plan.files[2].chunk_count, 0);
        assert_eq!(plan.files[2].first_chunk, 1);
        let mut store = ChunkStore::new();
        let mut source = InMemoryPayload::new(&payload);
        store
            .ingest_plan_source(&plan, &mut source)
            .expect("mixed empty ingest");

        let all_empty = vec![
            FileEntry {
                path: vec!["a.empty".to_owned()],
                data: Vec::new(),
            },
            FileEntry {
                path: vec!["b.empty".to_owned()],
                data: Vec::new(),
            },
        ];
        let (empty_plan, empty_payload) =
            CarBuildPlan::from_files(all_empty).expect("all-empty plan");
        assert_eq!(empty_plan.content_length, 0);
        assert!(empty_plan.chunks.is_empty());
        empty_plan.validate().expect("valid zero-content plan");
        let mut source = InMemoryPayload::new(&empty_payload);
        store
            .ingest_plan_source(&empty_plan, &mut source)
            .expect("zero-content ingest");
        assert!(store.chunks().is_empty());
        assert!(store.por_tree().is_empty());
        #[cfg(feature = "manifest")]
        assert!(store.pdp_tree().is_none());

        let writer = CarWriter::new(&empty_plan, &empty_payload).expect("empty CAR writer");
        let mut output = Cursor::new(Vec::new());
        writer.write_to(&mut output).expect("empty CAR");
    }

    #[test]
    fn plan_rejects_nonportable_paths_and_malformed_file_coverage() {
        for component in [
            ".",
            "..",
            "a/b",
            "a\\b",
            "C:",
            "name.",
            "name ",
            "CON",
            "con.txt",
            "PrN.log",
            "AUX",
            "nul.bin",
            "COM1",
            "com9.dat",
            "COM¹",
            "LPT1",
            "lpt9.txt",
            "LPT².log",
            "CONIN$",
            "clock$.txt",
            "angle<name",
            "angle>name",
            "quote\"name",
            "pipe|name",
            "question?name",
            "star*name",
            "control\n",
        ] {
            let error = CarBuildPlan::from_files(vec![FileEntry {
                path: vec![component.to_owned()],
                data: b"x".to_vec(),
            }])
            .expect_err("nonportable component must fail");
            assert!(matches!(error, CarPlanError::InvalidPath(_)));
        }
        for component in ["console", "COM0", "COM10", "LPT0", "report.txt", "文件"] {
            assert!(is_portable_normal_component(component), "{component}");
        }

        let (mut plan, _) = CarBuildPlan::from_files(vec![FileEntry {
            path: vec!["payload".to_owned()],
            data: b"payload".to_vec(),
        }])
        .expect("plan");
        plan.files[0].first_chunk = 1;
        assert!(matches!(
            plan.validate(),
            Err(CarPlanValidationError::NonCanonicalFileChunkStart {
                file_index: 0,
                expected: 0,
                actual: 1
            })
        ));

        let (mut plan, _) = CarBuildPlan::from_files(vec![FileEntry {
            path: vec!["payload".to_owned()],
            data: b"payload".to_vec(),
        }])
        .expect("plan");
        plan.files[0].size += 1;
        assert!(matches!(
            plan.validate(),
            Err(CarPlanValidationError::FileChunkByteRangeMismatch { file_index: 0 })
        ));
        assert!(matches!(
            CarWriter::new(&plan, b"payload"),
            Err(CarWriteError::InvalidPlan(_))
        ));
    }

    #[test]
    fn plan_rejects_below_minimum_chunk_bomb_and_heap_limit() {
        let profile = ChunkProfile {
            min_size: 4,
            target_size: 4,
            max_size: 8,
            break_mask: 1,
        };
        let mut plan = CarBuildPlan::single_file_with_profile(b"abcdefgh", profile).expect("plan");
        if plan.chunks.len() == 1 {
            let digest = plan.chunks[0].digest;
            plan.chunks = vec![
                CarChunk {
                    offset: 0,
                    length: 1,
                    digest,
                    taikai_segment_hint: None,
                },
                CarChunk {
                    offset: 1,
                    length: 7,
                    digest,
                    taikai_segment_hint: None,
                },
            ];
            plan.files[0].chunk_count = 2;
        } else {
            plan.chunks[0].length = 1;
            plan.chunks[1].offset = 1;
            plan.chunks[1].length = 7;
        }
        assert!(matches!(
            plan.validate(),
            Err(CarPlanValidationError::TooManyFileChunks { .. })
                | Err(CarPlanValidationError::ChunkBelowProfileMinimum { .. })
        ));

        let valid = CarBuildPlan::single_file(b"payload").expect("valid plan");
        let estimate = valid
            .validate()
            .expect("validation")
            .estimated_ingest_heap_bytes();
        assert!(estimate > 0);
        assert!(matches!(
            valid.validate_for_ingest_with_limit(estimate - 1),
            Err(ChunkStoreError::EstimatedHeapLimitExceeded {
                estimated,
                limit
            }) if estimated == estimate && limit == estimate - 1
        ));

        let mut total = 0usize;
        assert!(matches!(
            checked_estimate_add_product(&mut total, usize::MAX, 2, "adversarial test"),
            Err(CarPlanValidationError::EstimateOverflow {
                context: "adversarial test"
            })
        ));
        assert!(matches!(
            ensure_plan_file_count(CAR_PLAN_MAX_FILES + 1),
            Err(CarPlanError::TooManyFiles {
                count,
                maximum: CAR_PLAN_MAX_FILES
            }) if count == CAR_PLAN_MAX_FILES + 1
        ));
        assert!(matches!(
            checked_plan_payload_add(usize::MAX, 1),
            Err(CarPlanError::ContentLengthTooLarge)
        ));
        let mut allocation = Vec::<u8>::new();
        assert!(matches!(
            try_reserve_plan(&mut allocation, usize::MAX, "adversarial plan allocation"),
            Err(CarPlanError::AllocationFailed {
                context: "adversarial plan allocation",
                requested: usize::MAX
            })
        ));
    }

    #[test]
    fn plan_from_files_rejects_conflicting_paths() {
        let files = vec![
            FileEntry {
                path: vec!["docs".to_owned()],
                data: vec![1u8],
            },
            FileEntry {
                path: vec!["docs".to_owned(), "nested.txt".to_owned()],
                data: vec![2u8],
            },
        ];
        let err = CarBuildPlan::from_files(files).unwrap_err();
        assert!(matches!(err, CarPlanError::PathConflict { .. }));
    }

    #[test]
    fn plan_from_directory_matches_from_files() {
        use std::fs;

        use tempfile::tempdir;

        let tempdir = tempdir().expect("tempdir");
        let root = tempdir.path();
        fs::create_dir(root.join("docs")).expect("create docs dir");
        fs::write(root.join("docs").join("a.txt"), b"AAA").expect("write a");
        fs::write(root.join("docs").join("b.txt"), b"BBBB").expect("write b");

        let (plan_dir, payload_dir) =
            CarBuildPlan::from_directory(root).expect("plan from directory");

        let files = vec![
            FileEntry {
                path: vec!["docs".to_owned(), "a.txt".to_owned()],
                data: b"AAA".to_vec(),
            },
            FileEntry {
                path: vec!["docs".to_owned(), "b.txt".to_owned()],
                data: b"BBBB".to_vec(),
            },
        ];
        let (plan_files, payload_files) = CarBuildPlan::from_files(files).expect("plan from files");

        assert_eq!(plan_dir.files, plan_files.files);
        assert_eq!(plan_dir.content_length, plan_files.content_length);
        assert_eq!(plan_dir.chunks.len(), plan_files.chunks.len());
        assert_eq!(payload_dir, payload_files);
    }

    #[cfg(unix)]
    #[test]
    fn from_directory_rejects_symlink_files_directories_roots_and_hardlinks() {
        use std::os::unix::fs::symlink;

        let base = tempdir().expect("base");
        let root = base.path().join("root");
        let outside = base.path().join("outside");
        fs::create_dir(&root).expect("root");
        fs::create_dir(&outside).expect("outside");
        fs::write(root.join("payload"), b"payload").expect("payload");
        fs::write(outside.join("outside"), b"outside").expect("outside payload");

        symlink(outside.join("outside"), root.join("file-link")).expect("file symlink");
        assert!(CarBuildPlan::from_directory(&root).is_err());
        fs::remove_file(root.join("file-link")).expect("remove file symlink");

        symlink(&outside, root.join("directory-link")).expect("directory symlink");
        assert!(CarBuildPlan::from_directory(&root).is_err());
        fs::remove_file(root.join("directory-link")).expect("remove directory symlink");

        fs::hard_link(root.join("payload"), root.join("hardlink")).expect("hardlink");
        assert!(CarBuildPlan::from_directory(&root).is_err());
        fs::remove_file(root.join("hardlink")).expect("remove hardlink");

        let linked_root = base.path().join("linked-root");
        symlink(&root, &linked_root).expect("root symlink");
        assert!(CarBuildPlan::from_directory(&linked_root).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn secure_directory_scan_rejects_mid_scan_file_and_root_mutations() {
        #[derive(Clone, Copy)]
        enum Mutation {
            Replace,
            Grow,
            Truncate,
            RenameRoot,
        }

        for (label, mutation) in [
            ("replace", Mutation::Replace),
            ("grow", Mutation::Grow),
            ("truncate", Mutation::Truncate),
            ("rename-root", Mutation::RenameRoot),
        ] {
            let base = tempdir().expect("base");
            let canonical_base = fs::canonicalize(base.path()).expect("canonical base");
            let root = canonical_base.join("root");
            fs::create_dir(&root).expect("root");
            fs::write(root.join("payload"), b"payload").expect("payload");
            let displaced = canonical_base.join("displaced");
            let mut mutated = false;
            let mut hook = |path: &Path| -> io::Result<()> {
                if mutated {
                    return Ok(());
                }
                mutated = true;
                match mutation {
                    Mutation::Replace => {
                        fs::remove_file(path)?;
                        fs::write(path, b"changed")?;
                    }
                    Mutation::Grow => {
                        let mut file = OpenOptions::new().append(true).open(path)?;
                        file.write_all(b"!")?;
                        file.sync_all()?;
                    }
                    Mutation::Truncate => {
                        OpenOptions::new().write(true).open(path)?.set_len(1)?;
                    }
                    Mutation::RenameRoot => {
                        fs::rename(&root, &displaced)?;
                        fs::create_dir(&root)?;
                    }
                }
                Ok(())
            };
            let error = gather_files_secure(&root, &mut hook)
                .expect_err("mid-scan mutation must be rejected");
            assert!(mutated, "mutation hook did not run for {label}: {error}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn from_directory_rejects_deep_trees_and_oversized_sparse_inventory() {
        let deep = tempdir().expect("deep root");
        let mut current = deep.path().to_path_buf();
        for _ in 0..=CAR_LOGICAL_PATH_MAX_COMPONENTS {
            current.push("d");
            fs::create_dir(&current).expect("deep directory");
        }
        fs::write(current.join("payload"), b"payload").expect("deep payload");
        assert!(matches!(
            CarBuildPlan::from_directory(deep.path()),
            Err(CarPlanError::InvalidPath(_))
        ));

        let oversized = tempdir().expect("oversized root");
        let sparse = File::create(oversized.path().join("sparse")).expect("sparse file");
        sparse
            .set_len(CAR_EAGER_DIRECTORY_MAX_BYTES + 1)
            .expect("sparse length");
        assert!(matches!(
            CarBuildPlan::from_directory(oversized.path()),
            Err(CarPlanError::DirectoryPayloadTooLarge {
                bytes,
                maximum: CAR_EAGER_DIRECTORY_MAX_BYTES
            }) if bytes == CAR_EAGER_DIRECTORY_MAX_BYTES + 1
        ));
    }

    #[cfg(not(unix))]
    #[test]
    fn from_directory_fails_closed_without_secure_unix_file_identity() {
        assert!(matches!(
            CarBuildPlan::from_directory(Path::new("payload")),
            Err(CarPlanError::SecureDirectoryUnsupported)
        ));
    }

    #[test]
    fn empty_input_rejected() {
        let err = CarBuildPlan::single_file(&[]).unwrap_err();
        assert!(matches!(err, CarPlanError::EmptyInput));
    }

    #[test]
    fn plan_building_rejects_invalid_chunk_profile_without_panicking() {
        let invalid_profile = ChunkProfile {
            min_size: 4096,
            target_size: 2048,
            max_size: 8192,
            break_mask: ChunkProfile::DEFAULT.break_mask,
        };
        let err = CarBuildPlan::single_file_with_profile(b"payload", invalid_profile).unwrap_err();
        assert!(matches!(
            err,
            CarPlanError::Chunking(sorafs_chunker::ChunkerError::TargetBeforeMin { .. })
        ));

        let files = vec![FileEntry {
            path: vec!["payload.bin".to_owned()],
            data: b"payload".to_vec(),
        }];
        let err = CarBuildPlan::from_files_with_profile(files, invalid_profile).unwrap_err();
        assert!(matches!(
            err,
            CarPlanError::Chunking(sorafs_chunker::ChunkerError::TargetBeforeMin { .. })
        ));
    }

    #[test]
    fn chunk_store_try_ingest_rejects_invalid_profile_without_mutation() {
        let invalid_profile = ChunkProfile {
            min_size: 1,
            target_size: 1,
            max_size: 1,
            break_mask: 0,
        };
        let mut store = ChunkStore::with_profile(invalid_profile);
        let err = store.try_ingest_bytes(b"payload").unwrap_err();
        assert!(matches!(
            err,
            ChunkStoreError::Chunking(sorafs_chunker::ChunkerError::BreakMaskZero)
        ));
        assert!(store.chunks().is_empty());
        assert_eq!(store.payload_len(), 0);
        assert_eq!(
            store.payload_digest().as_bytes(),
            blake3::hash(&[]).as_bytes()
        );
    }

    #[test]
    fn chunk_store_heap_limit_is_checked_before_every_ingest() {
        assert!(matches!(
            ChunkStore::with_profile_and_heap_limit(ChunkProfile::DEFAULT, 0),
            Err(ChunkStoreError::InvalidEstimatedHeapLimit)
        ));
        assert!(matches!(
            DirectoryChunkSink::new("chunks").with_max_estimated_heap_bytes(0),
            Err(ChunkStoreError::InvalidEstimatedHeapLimit)
        ));
        let sink = DirectoryChunkSink::new("chunks")
            .with_max_estimated_heap_bytes(123)
            .expect("non-zero sink limit");
        assert_eq!(sink.max_estimated_heap_bytes, 123);
        assert_eq!(sink.clone().max_estimated_heap_bytes, 123);

        let mut store = ChunkStore::new();
        store
            .ingest_bytes(b"pre-existing state")
            .expect("seed store");
        let before = StoreSnapshot::capture(&store);
        store
            .set_max_estimated_heap_bytes(1)
            .expect("non-zero limit");
        assert_eq!(store.max_estimated_heap_bytes(), 1);
        assert!(matches!(
            store.try_ingest_bytes(b"replacement"),
            Err(ChunkStoreError::EstimatedHeapLimitExceeded { limit: 1, .. })
        ));
        before.assert_unchanged(&store);

        let plan = CarBuildPlan::single_file(b"replacement").expect("plan");
        let reads = Rc::new(Cell::new(0));
        let mut source = ProbeSource {
            reads: Rc::clone(&reads),
        };
        assert!(matches!(
            store.ingest_plan_source(&plan, &mut source),
            Err(ChunkStoreError::EstimatedHeapLimitExceeded { limit: 1, .. })
        ));
        assert_eq!(reads.get(), 0, "heap rejection must precede source I/O");
        before.assert_unchanged(&store);

        assert!(matches!(
            store.set_max_estimated_heap_bytes(0),
            Err(ChunkStoreError::InvalidEstimatedHeapLimit)
        ));
        assert_eq!(store.max_estimated_heap_bytes(), 1);
    }

    #[test]
    fn plan_heap_estimate_accounts_for_directory_sink_state() {
        let plan = CarBuildPlan::single_file(b"payload").expect("plan");
        let validation = plan.validate().expect("validation");
        let sink_floor = plan.chunks.len()
            * (std::mem::size_of::<ExpectedSinkChunk>()
                + std::mem::size_of::<PersistedChunkRecord>()
                + 32);
        assert!(validation.estimated_ingest_heap_bytes() >= sink_floor);
        assert!(matches!(
            plan.validate_for_ingest_with_limit(sink_floor.saturating_sub(1)),
            Err(ChunkStoreError::EstimatedHeapLimitExceeded { .. })
        ));
    }

    #[test]
    fn production_chunk_ceiling_covers_100_gib_high_density_geometry() {
        let t2_max_bytes = 100_u64 * 1024 * 1024 * 1024;
        let minimum_chunk_bytes = sorafs_chunker::HIGH_DENSITY_PROFILE.min_size as u64;
        let worst_case_chunks = t2_max_bytes.div_ceil(minimum_chunk_bytes);

        assert_eq!(worst_case_chunks, 3_276_800);
        assert!(
            worst_case_chunks <= CAR_PLAN_MAX_CHUNKS as u64,
            "the production plan ceiling must admit the documented 100 GiB T2 boundary"
        );
        #[cfg(target_pointer_width = "64")]
        {
            let estimate = estimate_direct_chunk_store_heap(
                t2_max_bytes as usize,
                sorafs_chunker::HIGH_DENSITY_PROFILE,
            )
            .expect("100 GiB high-density geometry must be estimable without allocation");
            assert!(estimate > DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES);
        }
    }

    #[test]
    fn car_plan_and_store_reject_profiles_wider_than_production_limit() {
        let max_size = CHUNK_STORE_MAX_CHUNK_BYTES as usize + 1;
        let profile = ChunkProfile {
            min_size: 1,
            target_size: 1,
            max_size,
            break_mask: 1,
        };

        let err = CarBuildPlan::single_file_with_profile(b"payload", profile).unwrap_err();
        assert!(matches!(
            err,
            CarPlanError::ChunkProfileMaxSizeTooLarge {
                max_size: observed,
                limit: CHUNK_STORE_MAX_CHUNK_BYTES
            } if observed == max_size
        ));

        let files = vec![FileEntry {
            path: vec!["payload.bin".to_owned()],
            data: b"payload".to_vec(),
        }];
        let err = CarBuildPlan::from_files_with_profile(files, profile).unwrap_err();
        assert!(matches!(
            err,
            CarPlanError::ChunkProfileMaxSizeTooLarge {
                max_size: observed,
                limit: CHUNK_STORE_MAX_CHUNK_BYTES
            } if observed == max_size
        ));

        let mut store = ChunkStore::with_profile(profile);
        let err = store.try_ingest_bytes(b"payload").unwrap_err();
        assert!(matches!(
            err,
            ChunkStoreError::ChunkProfileMaxSizeTooLarge {
                max_size: observed,
                limit: CHUNK_STORE_MAX_CHUNK_BYTES
            } if observed == max_size
        ));
        assert!(store.chunks().is_empty());
        assert_eq!(store.payload_len(), 0);
    }

    #[test]
    fn writer_emits_spec_compliant_carv2() {
        use std::io::Cursor;

        let input = sample_input();
        let plan = CarBuildPlan::single_file(&input).expect("plan");
        let writer = CarWriter::new(&plan, &input).expect("writer");

        let mut buffer = Cursor::new(Vec::new());
        let stats = writer.write_to(&mut buffer).expect("write carv2");
        let bytes = buffer.into_inner();

        assert_eq!(stats.payload_bytes, input.len() as u64);
        assert_eq!(stats.chunk_count, plan.chunks.len());
        assert_eq!(stats.car_size as usize, bytes.len());

        assert_eq!(&bytes[..PRAGMA.len()], &PRAGMA);

        let characteristics = &bytes[PRAGMA.len()..PRAGMA.len() + 16];
        assert_eq!(characteristics[0] & 0x80, 0x80);

        let header_offset = PRAGMA.len() + HEADER_LEN;
        let data_offset = u64::from_le_bytes(
            bytes[PRAGMA.len() + 16..PRAGMA.len() + 24]
                .try_into()
                .unwrap(),
        );
        assert_eq!(data_offset as usize, header_offset);

        let data_size = u64::from_le_bytes(
            bytes[PRAGMA.len() + 24..PRAGMA.len() + 32]
                .try_into()
                .unwrap(),
        );
        let index_offset = u64::from_le_bytes(
            bytes[PRAGMA.len() + 32..PRAGMA.len() + 40]
                .try_into()
                .unwrap(),
        );
        assert_eq!(index_offset, data_offset + data_size);
        assert!(index_offset as usize <= bytes.len());

        let payload_slice = &bytes[data_offset as usize..(data_offset + data_size) as usize];
        assert_eq!(
            stats.car_payload_digest.as_bytes(),
            blake3::hash(payload_slice).as_bytes()
        );
        assert_eq!(
            stats.car_archive_digest.as_bytes(),
            blake3::hash(&bytes).as_bytes()
        );
        let mut digest_arr = [0u8; 32];
        digest_arr.copy_from_slice(stats.car_archive_digest.as_bytes());
        assert_eq!(stats.car_cid, encode_cid(RAW_CODEC, &digest_arr));
        assert_eq!(stats.root_cids.len(), 1, "expected single root cid");
        assert_eq!(stats.dag_codec, DAG_CBOR_CODEC);

        let mut cursor = header_offset;
        let (header_len, header_len_bytes) = decode_uleb128(&bytes[cursor..]);
        cursor += header_len_bytes;
        let header_bytes = &bytes[cursor..cursor + header_len as usize];
        let mut header_idx = 0usize;
        let (map_len, consumed) = decode_cbor_map_len(&header_bytes[header_idx..]);
        assert_eq!(map_len, 2);
        header_idx += consumed;
        let (key_roots, consumed) = decode_cbor_text(&header_bytes[header_idx..]);
        assert_eq!(key_roots, "roots");
        header_idx += consumed;
        let (root_count, consumed) = decode_cbor_array_len(&header_bytes[header_idx..]);
        assert_eq!(root_count, 1);
        header_idx += consumed;
        let (root_bytes, consumed) = decode_cbor_bytes(&header_bytes[header_idx..]);
        assert_eq!(root_bytes.as_slice(), stats.root_cids[0].as_slice());
        header_idx += consumed;
        let (key_version, consumed) = decode_cbor_text(&header_bytes[header_idx..]);
        assert_eq!(key_version, "version");
        header_idx += consumed;
        let (version_value, consumed) = decode_cbor_uint(&header_bytes[header_idx..]);
        assert_eq!(version_value, 1);
        header_idx += consumed;
        assert_eq!(header_idx, header_bytes.len());
        cursor += header_len as usize;

        let mut observed_entries: Vec<([u8; 32], u64)> = Vec::new();
        let data_end = data_offset + data_size;
        let chunk_count = plan.chunks.len();
        while (cursor as u64) < data_end {
            let section_start = cursor;
            let (section_len, len_bytes) = decode_uleb128(&bytes[cursor..]);
            cursor += len_bytes;
            let (cid_len, codec) = decode_cid(&bytes[cursor..]);
            let cid_bytes = &bytes[cursor..cursor + cid_len];
            cursor += cid_len;
            let data_len = section_len as usize - cid_len;
            let data_slice = &bytes[cursor..cursor + data_len];
            cursor += data_len;

            let offset = (section_start - header_offset) as u64;

            let digest: [u8; 32] = if observed_entries.len() < chunk_count {
                let chunk = &plan.chunks[observed_entries.len()];
                assert_eq!(codec, RAW_CODEC);
                assert_eq!(cid_bytes, encode_cid(RAW_CODEC, &chunk.digest).as_slice());
                let chunk_start = chunk.offset as usize;
                let chunk_end = chunk_start + chunk.length as usize;
                assert_eq!(data_slice, &input[chunk_start..chunk_end]);
                chunk.digest
            } else {
                assert_eq!(codec, DAG_CBOR_CODEC);
                assert!(matches!(data_slice.first().map(|b| b & 0xe0), Some(0xa0)));
                blake3::hash(data_slice).into()
            };

            observed_entries.push((digest, offset));
        }

        assert_eq!(cursor as u64, data_end);

        let (index_codec, index_codec_len) = decode_uleb128(&bytes[index_offset as usize..]);
        assert_eq!(index_codec, 0x0401);
        let mut idx_cursor = index_offset as usize + index_codec_len;

        let mut indexed_entries: Vec<([u8; 32], u64)> = Vec::new();
        while idx_cursor < bytes.len() {
            let mh_code = u64::from_le_bytes(bytes[idx_cursor..idx_cursor + 8].try_into().unwrap());
            assert_eq!(mh_code, BLAKE3_256_MULTIHASH_CODE);
            idx_cursor += 8;

            let digest_size =
                u32::from_le_bytes(bytes[idx_cursor..idx_cursor + 4].try_into().unwrap());
            assert_eq!(digest_size, 32);
            idx_cursor += 4;

            let count = u64::from_le_bytes(bytes[idx_cursor..idx_cursor + 8].try_into().unwrap());
            idx_cursor += 8;

            for _ in 0..count {
                let mut digest = [0u8; 32];
                digest.copy_from_slice(&bytes[idx_cursor..idx_cursor + digest_size as usize]);
                idx_cursor += digest_size as usize;

                let offset =
                    u64::from_le_bytes(bytes[idx_cursor..idx_cursor + 8].try_into().unwrap());
                idx_cursor += 8;

                indexed_entries.push((digest, offset));
            }
        }

        assert_eq!(idx_cursor, bytes.len());

        assert_eq!(indexed_entries.len(), observed_entries.len());

        let mut expected_entries = observed_entries.clone();
        expected_entries.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(indexed_entries, expected_entries);
    }

    #[test]
    fn car_layout_helpers_fail_closed_on_invariants_overflow_and_capacity() {
        assert!(matches!(
            build_file_dag(&[], 1),
            Err(CarWriteError::DagInvariant {
                context: "file DAG payload size"
            })
        ));

        let first_path = vec!["docs".to_owned()];
        let descendant_path = vec!["docs".to_owned(), "payload.bin".to_owned()];
        let files = [
            FileRootInfo {
                path: &first_path,
                cid: vec![1],
                size: 1,
            },
            FileRootInfo {
                path: &descendant_path,
                cid: vec![2],
                size: 1,
            },
        ];
        assert!(matches!(
            build_directory_dag(&files),
            Err(CarWriteError::DirectoryPathConflict)
        ));

        let empty_path: Vec<String> = Vec::new();
        let empty_path_file = [FileRootInfo {
            path: &empty_path,
            cid: vec![1],
            size: 0,
        }];
        assert!(matches!(
            build_directory_dag(&empty_path_file),
            Err(CarWriteError::DagInvariant {
                context: "directory DAG file path"
            })
        ));

        let first = TreeNode {
            cid_bytes: vec![1],
            data: Vec::new(),
            digest: [0; 32],
            size: u64::MAX,
        };
        let second = TreeNode {
            cid_bytes: vec![2],
            data: Vec::new(),
            digest: [0; 32],
            size: 1,
        };
        assert!(matches!(
            build_branch_node(&[&first, &second]),
            Err(CarWriteError::ArithmeticOverflow {
                context: "file branch payload size"
            })
        ));
        assert!(matches!(
            checked_car_usize_add(usize::MAX, 1, "test arithmetic"),
            Err(CarWriteError::ArithmeticOverflow {
                context: "test arithmetic"
            })
        ));

        let mut allocation = Vec::<u8>::new();
        assert!(matches!(
            try_reserve_car(&mut allocation, usize::MAX, "test allocation"),
            Err(CarWriteError::AllocationFailed {
                context: "test allocation",
                requested: usize::MAX
            })
        ));
        let section = CarSection {
            length_varint: Vec::new(),
            cid_bytes: Vec::new(),
            data: SectionData::Chunk { chunk_index: 1 },
            digest: [0; 32],
            offset: 0,
        };
        let plan = CarBuildPlan::single_file(b"x").expect("plan");
        assert!(matches!(
            section.data_len(&plan),
            Err(CarWriteError::DagInvariant {
                context: "chunk section index"
            })
        ));
    }

    #[test]
    fn streaming_writer_matches_buffered_output() {
        use std::io::Cursor;

        let mut payload = Vec::new();
        for i in 0..(512 * 3 + 123) {
            payload.push((i % 251) as u8);
        }
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let writer = CarWriter::new(&plan, &payload).expect("writer");
        let mut buffered_bytes = Vec::new();
        let stats_buffered = writer
            .write_to(&mut buffered_bytes)
            .expect("buffered write");

        let streaming_writer = CarStreamingWriter::new(&plan);
        let mut reader = Cursor::new(payload.clone());
        let mut streaming_bytes = Vec::new();
        let stats_streaming = streaming_writer
            .write_from_reader(&mut reader, &mut streaming_bytes)
            .expect("streaming write");

        assert_eq!(buffered_bytes, streaming_bytes);
        assert_eq!(stats_buffered, stats_streaming);
    }

    #[test]
    fn streaming_writer_detects_digest_mismatch() {
        use std::io::Cursor;

        let payload = vec![0u8; 600_000];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");

        let mut corrupted = payload.clone();
        corrupted[123] ^= 0xff;

        let streaming_writer = CarStreamingWriter::new(&plan);
        let mut reader = Cursor::new(corrupted);
        let result = streaming_writer.write_from_reader(&mut reader, &mut Vec::new());
        assert!(matches!(
            result,
            Err(CarWriteError::DigestMismatch { chunk_index: 0 })
        ));
    }

    #[test]
    fn writer_rejects_payload_mismatch() {
        let input = sample_input();
        let plan = CarBuildPlan::single_file(&input).expect("plan");
        match CarWriter::new(&plan, &input[..input.len() - 1]) {
            Err(CarWriteError::PayloadMismatch) => {}
            Err(other) => panic!("expected PayloadMismatch, got {other:?}"),
            Ok(_) => panic!("expected PayloadMismatch, got Ok"),
        }
    }

    #[test]
    fn writer_rejects_digest_mismatch() {
        let mut input = sample_input();
        let plan = CarBuildPlan::single_file(&input).expect("plan");
        input[1000] ^= 0xFF;
        assert!(matches!(
            CarWriter::new(&plan, &input),
            Err(CarWriteError::PayloadDigestMismatch)
        ));
    }

    #[test]
    fn writer_respects_expected_roots() {
        use std::io::Cursor;

        let input = sample_input();
        let plan = CarBuildPlan::single_file(&input).expect("plan");
        let mut baseline = Cursor::new(Vec::new());
        let baseline_stats = CarWriter::new(&plan, &input)
            .expect("baseline writer")
            .write_to(&mut baseline)
            .expect("baseline write");
        let expected_root = baseline_stats.root_cids[0].clone();

        let writer = CarWriter::with_expected_roots(&plan, &input, vec![expected_root.clone()])
            .expect("writer with expected roots");

        let mut buffer = Cursor::new(Vec::new());
        let stats = writer.write_to(&mut buffer).expect("write");
        assert_eq!(stats.root_cids, vec![expected_root.clone()]);
        let data = buffer.into_inner();

        let header_offset = PRAGMA.len() + HEADER_LEN;
        let (header_len, len_bytes) = decode_uleb128(&data[header_offset..]);
        let header_start = header_offset + len_bytes;
        let header = &data[header_start..header_start + header_len as usize];

        let (map_len, consumed) = decode_cbor_map_len(header);
        assert_eq!(map_len, 2);
        let (key_roots, delta) = decode_cbor_text(&header[consumed..]);
        assert_eq!(key_roots, "roots");
        let offset = consumed + delta;
        let (root_count, delta) = decode_cbor_array_len(&header[offset..]);
        assert_eq!(root_count, 1);
        let offset = offset + delta;
        let (root_bytes, delta) = decode_cbor_bytes(&header[offset..]);
        assert_eq!(root_bytes.as_slice(), expected_root.as_slice());
        let offset = offset + delta;
        let (key_version, delta) = decode_cbor_text(&header[offset..]);
        assert_eq!(key_version, "version");
        let offset = offset + delta;
        let (version_value, delta) = decode_cbor_uint(&header[offset..]);
        assert_eq!(version_value, 1);
        let offset = offset + delta;
        assert_eq!(offset, header.len());
    }

    #[test]
    fn ingest_plan_stream_matches_in_memory() {
        let input = sample_input();
        let plan = CarBuildPlan::single_file(&input).expect("plan");

        let mut store_mem = ChunkStore::with_profile(plan.chunk_profile);
        store_mem.ingest_plan(&input, &plan).expect("memory ingest");

        let mut store_stream = ChunkStore::with_profile(plan.chunk_profile);
        let mut cursor = Cursor::new(&input);
        store_stream
            .ingest_plan_stream(&plan, &mut cursor)
            .expect("stream ingest");

        assert_eq!(store_mem.payload_len(), store_stream.payload_len());
        assert_eq!(store_mem.payload_digest(), store_stream.payload_digest());
        assert_eq!(store_mem.por_tree().root(), store_stream.por_tree().root());
    }

    #[test]
    fn directory_payload_sampling_matches_in_memory() {
        let temp = tempdir().expect("tempdir");
        let root = temp.path();
        let file_a = root.join("a.bin");
        let file_b = root.join("b.bin");
        fs::write(&file_a, b"hello world").expect("write a");
        fs::write(&file_b, vec![7u8; 8192]).expect("write b");

        let (plan, payload) = CarBuildPlan::from_directory(root).expect("plan directory");

        let mut store_mem = ChunkStore::with_profile(plan.chunk_profile);
        store_mem
            .ingest_plan(&payload, &plan)
            .expect("memory ingest");

        let mut dir_source_ingest = DirectoryPayload::new(root, &plan.files).expect("dir payload");
        let mut store_dir = ChunkStore::with_profile(plan.chunk_profile);
        store_dir
            .ingest_plan_source(&plan, &mut dir_source_ingest)
            .expect("directory ingest");

        assert_eq!(store_mem.por_tree().root(), store_dir.por_tree().root());

        let mut mem_source = InMemoryPayload::new(&payload);
        let mem_samples = store_dir
            .sample_leaves_with(5, 0x1234_5678, &mut mem_source)
            .expect("mem samples");

        let mut dir_source = DirectoryPayload::new(root, &plan.files).expect("dir payload");
        let dir_samples = store_dir
            .sample_leaves_with(5, 0x1234_5678, &mut dir_source)
            .expect("dir samples");

        assert_eq!(mem_samples.len(), dir_samples.len());
        for (mem, dir) in mem_samples.iter().zip(dir_samples.iter()) {
            assert_eq!(mem.0, dir.0);
            assert_eq!(mem.1.leaf_bytes, dir.1.leaf_bytes);
        }
    }

    #[test]
    fn directory_payload_rejects_file_span_length_overflow() {
        let files = vec![
            FilePlan {
                path: vec!["first".to_owned()],
                first_chunk: 0,
                chunk_count: 0,
                size: u64::MAX,
            },
            FilePlan {
                path: vec!["second".to_owned()],
                first_chunk: 0,
                chunk_count: 0,
                size: 1,
            },
        ];
        let error = match DirectoryPayload::new(Path::new("."), &files) {
            Ok(_) => panic!("overflowing file spans must fail"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    #[test]
    fn file_payload_rejects_symlink_nonregular_swap_and_resize() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("payload");
        fs::write(&path, b"payload").expect("payload");
        let link = temp.path().join("link");
        symlink(&path, &link).expect("symlink");
        assert!(FilePayload::open(&link).is_err());
        assert!(FilePayload::open(temp.path()).is_err());
        let hardlink = temp.path().join("hardlink");
        fs::hard_link(&path, &hardlink).expect("hardlink");
        assert!(FilePayload::open(&path).is_err());
        fs::remove_file(&hardlink).expect("remove hardlink");

        let mut source = FilePayload::open(&path).expect("file payload");
        let mut bytes = [0u8; 7];
        source.read_exact(0, &mut bytes).expect("initial read");
        fs::remove_file(&path).expect("remove original");
        fs::write(&path, b"changed").expect("replacement");
        assert!(matches!(
            source.read_exact(0, &mut bytes),
            Err(ChunkStoreError::Io(_))
        ));

        let resize_path = temp.path().join("resize");
        fs::write(&resize_path, b"payload").expect("resize payload");
        let mut source = FilePayload::open(&resize_path).expect("file payload");
        let mut file = OpenOptions::new()
            .append(true)
            .open(&resize_path)
            .expect("append");
        file.write_all(b"!").expect("grow");
        file.sync_all().expect("sync");
        assert!(matches!(
            source.ensure_exhausted(7),
            Err(ChunkStoreError::Io(_))
        ));

        let truncate_path = temp.path().join("truncate");
        fs::write(&truncate_path, b"payload").expect("truncate payload");
        let mut source = FilePayload::open(&truncate_path).expect("file payload");
        OpenOptions::new()
            .write(true)
            .open(&truncate_path)
            .expect("open for truncation")
            .set_len(1)
            .expect("truncate");
        assert!(matches!(
            source.ensure_exhausted(7),
            Err(ChunkStoreError::Io(_))
        ));
    }

    #[test]
    fn directory_payload_rejects_nonportable_paths_and_actual_size_mismatch() {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join("payload"), b"payload").expect("payload");
        let traversal = [FilePlan {
            path: vec!["..".to_owned(), "victim".to_owned()],
            first_chunk: 0,
            chunk_count: 0,
            size: 0,
        }];
        let error = DirectoryPayload::new(temp.path(), &traversal)
            .err()
            .expect("traversal rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        let wrong_size = [FilePlan {
            path: vec!["payload".to_owned()],
            first_chunk: 0,
            chunk_count: 1,
            size: 1,
        }];
        let error = DirectoryPayload::new(temp.path(), &wrong_size)
            .err()
            .expect("size mismatch rejected");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    #[test]
    fn directory_payload_rejects_symlink_and_hardlink_sources() {
        use std::os::unix::fs::symlink;

        let temp = tempdir().expect("tempdir");
        let victim = temp.path().join("victim");
        fs::write(&victim, b"payload").expect("victim");
        let symlink_path = temp.path().join("symlink");
        symlink(&victim, &symlink_path).expect("symlink");
        let symlink_plan = [FilePlan {
            path: vec!["symlink".to_owned()],
            first_chunk: 0,
            chunk_count: 1,
            size: 7,
        }];
        assert!(DirectoryPayload::new(temp.path(), &symlink_plan).is_err());

        let hardlink_path = temp.path().join("hardlink");
        fs::hard_link(&victim, &hardlink_path).expect("hardlink");
        let hardlink_plan = [FilePlan {
            path: vec!["hardlink".to_owned()],
            first_chunk: 0,
            chunk_count: 1,
            size: 7,
        }];
        assert!(DirectoryPayload::new(temp.path(), &hardlink_plan).is_err());
        assert_eq!(fs::read(&victim).expect("victim unchanged"), b"payload");

        let outside = tempdir().expect("outside");
        fs::write(outside.path().join("escaped"), b"payload").expect("outside payload");
        symlink(outside.path(), temp.path().join("escape-dir")).expect("ancestor symlink");
        let escaped_plan = [FilePlan {
            path: vec!["escape-dir".to_owned(), "escaped".to_owned()],
            first_chunk: 0,
            chunk_count: 1,
            size: 7,
        }];
        let error = DirectoryPayload::new(temp.path(), &escaped_plan)
            .err()
            .expect("ancestor escape rejected");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn directory_payload_detects_mutation_before_read_and_final_recheck() {
        let temp = tempdir().expect("tempdir");
        let path = temp.path().join("payload");
        fs::write(&path, b"payload").expect("payload");
        let files = [FilePlan {
            path: vec!["payload".to_owned()],
            first_chunk: 0,
            chunk_count: 1,
            size: 7,
        }];

        let mut source = DirectoryPayload::new(temp.path(), &files).expect("source");
        let replacement = temp.path().join("replacement");
        fs::write(&replacement, b"changed").expect("replacement");
        fs::remove_file(&path).expect("remove original");
        fs::rename(&replacement, &path).expect("replace source");
        let mut bytes = [0u8; 7];
        assert!(matches!(
            source.read_exact(0, &mut bytes),
            Err(ChunkStoreError::Io(_))
        ));

        fs::write(&path, b"payload").expect("restore payload");
        let mut source = DirectoryPayload::new(temp.path(), &files).expect("fresh source");
        source.read_exact(0, &mut bytes).expect("initial read");
        let mut file = OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("append handle");
        file.write_all(b"!").expect("append");
        file.sync_all().expect("sync mutation");
        assert!(matches!(
            source.ensure_exhausted(7),
            Err(ChunkStoreError::Io(_))
        ));
    }

    #[cfg(not(unix))]
    #[test]
    fn secure_file_backed_apis_fail_closed_on_unsupported_platforms() {
        let file_error = FilePayload::open(Path::new("payload"))
            .err()
            .expect("file payload unsupported");
        assert_eq!(file_error.kind(), io::ErrorKind::Unsupported);
        let directory_error = DirectoryPayload::new(Path::new("."), &[])
            .err()
            .expect("directory payload unsupported");
        assert_eq!(directory_error.kind(), io::ErrorKind::Unsupported);

        let plan = CarBuildPlan::single_file(b"payload").expect("plan");
        let mut sink = DirectoryChunkSink::new("chunks");
        assert!(matches!(
            sink.prepare(&plan),
            Err(ChunkStoreError::Io(error)) if error.kind() == io::ErrorKind::Unsupported
        ));
    }

    #[test]
    fn chunk_fetch_specs_reflect_plan() {
        let input = sample_input();
        let plan = CarBuildPlan::single_file(&input).expect("plan");
        let specs = plan.try_chunk_fetch_specs().expect("fetch specs");
        assert_eq!(specs.len(), plan.chunks.len());
        for (idx, spec) in specs.iter().enumerate() {
            let chunk = &plan.chunks[idx];
            assert_eq!(spec.chunk_index, idx);
            assert_eq!(spec.offset, chunk.offset);
            assert_eq!(spec.length, chunk.length);
            assert_eq!(spec.digest, chunk.digest);
        }
    }

    #[test]
    fn try_chunk_fetch_specs_rejects_invalid_plan_instead_of_returning_empty() {
        let input = sample_input();
        let mut plan = CarBuildPlan::single_file(&input).expect("plan");
        plan.chunks[0].offset = 1;
        assert!(matches!(
            plan.try_chunk_fetch_specs(),
            Err(CarPlanError::InvalidPlan(
                CarPlanValidationError::NonContiguousChunk { chunk_index: 0, .. }
            ))
        ));
    }

    #[test]
    fn directory_car_emits_directory_root() {
        use std::io::Cursor;

        let files = vec![
            FileEntry {
                path: vec!["docs".to_string(), "index.html".to_string()],
                data: b"<html></html>".to_vec(),
            },
            FileEntry {
                path: vec!["docs".to_string(), "style.css".to_string()],
                data: b"body { background: #fff; }".to_vec(),
            },
        ];

        let (plan, payload) = CarBuildPlan::from_files(files).expect("plan");
        assert_eq!(plan.files.len(), 2);
        assert_eq!(
            plan.files[0].path,
            vec!["docs".to_string(), "index.html".to_string()]
        );
        assert_eq!(
            plan.files[1].path,
            vec!["docs".to_string(), "style.css".to_string()]
        );

        let writer = CarWriter::new(&plan, &payload).expect("writer");
        let mut buffer = Cursor::new(Vec::new());
        let stats = writer.write_to(&mut buffer).expect("write directory car");
        assert_eq!(stats.root_cids.len(), 1);

        let bytes = buffer.into_inner();

        let header_offset = PRAGMA.len() + HEADER_LEN;
        let data_offset = u64::from_le_bytes(
            bytes[PRAGMA.len() + 16..PRAGMA.len() + 24]
                .try_into()
                .unwrap(),
        );
        let data_size = u64::from_le_bytes(
            bytes[PRAGMA.len() + 24..PRAGMA.len() + 32]
                .try_into()
                .unwrap(),
        );
        assert_eq!(data_offset as usize, header_offset);

        let mut section_cid_map: BTreeMap<Vec<u8>, Vec<u8>> = BTreeMap::new();
        let mut cursor = header_offset;
        let (header_len, header_len_bytes) = decode_uleb128(&bytes[cursor..]);
        cursor += header_len_bytes + header_len as usize;
        let data_end = data_offset + data_size;
        while (cursor as u64) < data_end {
            let (section_len, len_bytes) = decode_uleb128(&bytes[cursor..]);
            cursor += len_bytes;
            let (cid_len, _) = decode_cid(&bytes[cursor..]);
            let cid_bytes = bytes[cursor..cursor + cid_len].to_vec();
            cursor += cid_len;
            let data_len = section_len as usize - cid_len;
            let data_slice = bytes[cursor..cursor + data_len].to_vec();
            cursor += data_len;
            section_cid_map.insert(cid_bytes, data_slice);
        }
        assert_eq!(cursor as u64, data_end);

        let root_cid = stats.root_cids[0].clone();
        let root_data = section_cid_map
            .get(&root_cid)
            .expect("root directory node present");

        let (root_entries, root_size) = parse_directory_node(root_data);
        assert_eq!(root_entries.len(), 1);
        assert_eq!(root_size as usize, payload.len());
        let docs_entry = &root_entries[0];
        assert_eq!(docs_entry.name, "docs");
        assert!(matches!(docs_entry.kind, DirectoryEntryKind::Directory));

        let docs_data = section_cid_map
            .get(&docs_entry.cid)
            .expect("docs directory node present");
        let (docs_entries, docs_size) = parse_directory_node(docs_data);
        assert_eq!(docs_entries.len(), 2);
        assert_eq!(
            docs_size as usize,
            plan.files.iter().map(|f| f.size as usize).sum::<usize>()
        );

        let mut names: Vec<_> = docs_entries.iter().map(|e| e.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["index.html", "style.css"]);
        for entry in docs_entries {
            assert!(matches!(entry.kind, DirectoryEntryKind::File));
            let size = plan
                .files
                .iter()
                .find(|f| f.path.last().unwrap() == &entry.name)
                .map(|f| f.size)
                .expect("matching file plan");
            assert_eq!(entry.size, size);
        }
    }

    fn decode_cid(data: &[u8]) -> (usize, u64) {
        let (version, consumed_version) = decode_uleb128(data);
        assert_eq!(version, 1, "expected CIDv1");
        let (codec, consumed_codec) = decode_uleb128(&data[consumed_version..]);
        let (mh_code, consumed_mh) = decode_uleb128(&data[consumed_version + consumed_codec..]);
        assert_eq!(mh_code, BLAKE3_256_MULTIHASH_CODE);
        let (digest_len, consumed_len) =
            decode_uleb128(&data[consumed_version + consumed_codec + consumed_mh..]);
        let total_len =
            consumed_version + consumed_codec + consumed_mh + consumed_len + digest_len as usize;
        (total_len, codec)
    }

    fn decode_cbor_len(expected_major: u8, data: &[u8]) -> (u64, usize) {
        assert!(!data.is_empty(), "insufficient CBOR data");
        let first = data[0];
        assert_eq!(first >> 5, expected_major, "unexpected CBOR major type");
        let additional = first & 0x1f;
        match additional {
            v @ 0..=23 => (v as u64, 1),
            24 => (data[1] as u64, 2),
            25 => (u16::from_be_bytes([data[1], data[2]]) as u64, 3),
            26 => (
                u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as u64,
                5,
            ),
            27 => (
                u64::from_be_bytes([
                    data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
                ]),
                9,
            ),
            31 => panic!("indefinite length CBOR not supported in tests"),
            _ => unreachable!("invalid CBOR additional info"),
        }
    }

    fn decode_cbor_map_len(data: &[u8]) -> (u64, usize) {
        decode_cbor_len(5, data)
    }

    fn decode_cbor_array_len(data: &[u8]) -> (u64, usize) {
        decode_cbor_len(4, data)
    }

    fn decode_cbor_uint(data: &[u8]) -> (u64, usize) {
        decode_cbor_len(0, data)
    }

    fn decode_cbor_text(data: &[u8]) -> (String, usize) {
        let (len, consumed) = decode_cbor_len(3, data);
        let start = consumed;
        let end = start + len as usize;
        let text = String::from_utf8(data[start..end].to_vec()).expect("valid UTF-8");
        (text, consumed + len as usize)
    }

    fn decode_cbor_bytes(data: &[u8]) -> (Vec<u8>, usize) {
        let (len, consumed) = decode_cbor_len(2, data);
        let start = consumed;
        let end = start + len as usize;
        (data[start..end].to_vec(), consumed + len as usize)
    }

    #[derive(Debug, Clone)]
    struct DirEntryView {
        name: String,
        kind: DirectoryEntryKind,
        cid: Vec<u8>,
        size: u64,
    }

    fn parse_directory_node(data: &[u8]) -> (Vec<DirEntryView>, u64) {
        let mut idx = 0usize;
        let (map_len, consumed) = decode_cbor_map_len(&data[idx..]);
        assert_eq!(map_len, 4);
        idx += consumed;

        let (key_entries, consumed) = decode_cbor_text(&data[idx..]);
        assert_eq!(key_entries, "entries");
        idx += consumed;
        let (entries_len, consumed) = decode_cbor_array_len(&data[idx..]);
        idx += consumed;

        let mut entries = Vec::with_capacity(entries_len as usize);
        for _ in 0..entries_len {
            let (entry_map_len, consumed) = decode_cbor_map_len(&data[idx..]);
            assert_eq!(entry_map_len, 4);
            idx += consumed;

            let (name_key, consumed) = decode_cbor_text(&data[idx..]);
            assert_eq!(name_key, "name");
            idx += consumed;
            let (name_value, consumed) = decode_cbor_text(&data[idx..]);
            idx += consumed;

            let (cid_key, consumed) = decode_cbor_text(&data[idx..]);
            assert_eq!(cid_key, "cid");
            idx += consumed;
            let (cid_value, consumed) = decode_cbor_bytes(&data[idx..]);
            idx += consumed;

            let (kind_key, consumed) = decode_cbor_text(&data[idx..]);
            assert_eq!(kind_key, "kind");
            idx += consumed;
            let (kind_value, consumed) = decode_cbor_text(&data[idx..]);
            idx += consumed;
            let kind = match kind_value.as_str() {
                "file" => DirectoryEntryKind::File,
                "dir" => DirectoryEntryKind::Directory,
                other => panic!("unexpected directory entry kind {other}"),
            };

            let (size_key, consumed) = decode_cbor_text(&data[idx..]);
            assert_eq!(size_key, "size");
            idx += consumed;
            let (size_value, consumed) = decode_cbor_uint(&data[idx..]);
            idx += consumed;

            entries.push(DirEntryView {
                name: name_value,
                kind,
                cid: cid_value,
                size: size_value,
            });
        }

        let (key_size, consumed) = decode_cbor_text(&data[idx..]);
        assert_eq!(key_size, "size");
        idx += consumed;
        let (dir_size, consumed) = decode_cbor_uint(&data[idx..]);
        idx += consumed;

        let (key_type, consumed) = decode_cbor_text(&data[idx..]);
        assert_eq!(key_type, "type");
        idx += consumed;
        let (type_value, consumed) = decode_cbor_text(&data[idx..]);
        assert_eq!(type_value, DIR_NODE_TYPE);
        idx += consumed;

        let (key_version, consumed) = decode_cbor_text(&data[idx..]);
        assert_eq!(key_version, "version");
        idx += consumed;
        let (version_value, consumed) = decode_cbor_uint(&data[idx..]);
        assert_eq!(version_value, DAG_NODE_VERSION);
        idx += consumed;

        assert_eq!(idx, data.len());

        (entries, dir_size)
    }

    fn decode_uleb128(data: &[u8]) -> (u64, usize) {
        let mut value = 0u64;
        let mut shift = 0;
        for (idx, byte) in data.iter().enumerate() {
            let slice = (byte & 0x7F) as u64;
            value |= slice << shift;
            if byte & 0x80 == 0 {
                return (value, idx + 1);
            }
            shift += 7;
        }
        (value, data.len())
    }
}
