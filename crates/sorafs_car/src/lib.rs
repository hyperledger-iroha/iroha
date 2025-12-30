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
    fs::{self, File},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
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
use sorafs_chunker::{ChunkDigest, ChunkProfile, chunk_bytes_with_digests_profile};
#[cfg(feature = "manifest")]
use sorafs_manifest::ManifestV1 as SorafsManifestV1;
use thiserror::Error;

pub mod chunker_registry;
mod chunker_registry_data;
pub mod fetch_plan;
pub mod fixtures;
#[cfg(feature = "manifest")]
pub mod gateway;
pub mod local_fetch;
#[cfg(feature = "manifest")]
pub mod moderation;
pub mod multi_fetch;
pub mod policy;
#[path = "proof_stream.rs"]
pub mod proof_stream;
#[path = "proof_stream_transport.rs"]
pub mod proof_stream_transport;
pub mod scoreboard;
pub mod streaming_verifier;
#[cfg(feature = "cli")]
pub mod taikai;
#[cfg(feature = "manifest")]
pub mod trustless;
#[cfg(feature = "manifest")]
pub mod verifier;

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
}

/// Errors surfaced by the future CAR writer.
#[derive(Debug, Error)]
pub enum CarWriteError {
    #[error("writer failed: {0}")]
    Io(#[from] io::Error),
    #[error("payload length does not match plan content length")]
    PayloadMismatch,
    #[error("chunk {chunk_index} extends beyond payload length")]
    ChunkOutOfBounds { chunk_index: usize },
    #[error("chunk {chunk_index} digest does not match payload bytes")]
    DigestMismatch { chunk_index: usize },
    #[error("root length exceeds supported bounds")]
    RootTooLarge,
    #[error("expected roots do not match computed roots")]
    RootMismatch,
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
}

/// Abstraction over payload sources that support random-access reads.
pub trait PayloadSource {
    /// Reads exactly `buf.len()` bytes starting at `offset` into `buf`.
    fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), ChunkStoreError>;
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
        self.consumed += buf.len() as u64;
        Ok(())
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
        let start = offset as usize;
        let end = start
            .checked_add(buf.len())
            .ok_or(ChunkStoreError::OffsetOutOfRange {
                offset,
                len: self.data.len() as u64,
            })?;
        if end > self.data.len() {
            return Err(ChunkStoreError::OffsetOutOfRange {
                offset,
                len: self.data.len() as u64,
            });
        }
        buf.copy_from_slice(&self.data[start..end]);
        Ok(())
    }
}

/// Payload source backed by a single file on disk.
pub struct FilePayload {
    file: File,
}

impl FilePayload {
    pub fn open(path: &Path) -> Result<Self, io::Error> {
        Ok(Self {
            file: File::open(path)?,
        })
    }
}

impl PayloadSource for FilePayload {
    fn read_exact(&mut self, offset: u64, buf: &mut [u8]) -> Result<(), ChunkStoreError> {
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(ChunkStoreError::Io)?;
        self.file.read_exact(buf).map_err(ChunkStoreError::Io)?;
        Ok(())
    }
}

struct FileSpan {
    start: u64,
    end: u64,
    path: PathBuf,
}

/// Payload source backed by multiple files described by a [`CarBuildPlan`].
pub struct DirectoryPayload {
    spans: Vec<FileSpan>,
    total_len: u64,
    cached_index: Option<usize>,
    cached_file: Option<File>,
}

impl DirectoryPayload {
    pub fn new(root: &Path, files: &[FilePlan]) -> Result<Self, io::Error> {
        let mut spans = Vec::with_capacity(files.len());
        let mut offset = 0u64;
        for file in files {
            let mut path = root.to_path_buf();
            for component in &file.path {
                path.push(component);
            }
            let end = offset + file.size;
            spans.push(FileSpan {
                start: offset,
                end,
                path,
            });
            offset = end;
        }
        Ok(Self {
            total_len: offset,
            spans,
            cached_index: None,
            cached_file: None,
        })
    }

    fn open_file(&mut self, span_index: usize) -> Result<&mut File, ChunkStoreError> {
        if self.cached_index != Some(span_index) {
            let file = File::open(&self.spans[span_index].path).map_err(ChunkStoreError::Io)?;
            self.cached_file = Some(file);
            self.cached_index = Some(span_index);
        }
        self.cached_file.as_mut().ok_or_else(|| {
            ChunkStoreError::Io(io::Error::other("failed to cache directory file handle"))
        })
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
            let span_remaining = (span.end - current_offset) as usize;
            let to_read = span_remaining.min(remaining);

            let file = self.open_file(span_index)?;
            file.seek(SeekFrom::Start(span_offset))
                .map_err(ChunkStoreError::Io)?;
            file.read_exact(&mut buf[buf_cursor..buf_cursor + to_read])
                .map_err(ChunkStoreError::Io)?;

            remaining -= to_read;
            buf_cursor += to_read;
            current_offset += to_read as u64;
        }

        Ok(())
    }
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
    let chunk_size = usize::try_from(manifest.chunk_size)
        .map_err(|_| PlanFromManifestError::ChunkSizeTooLarge(manifest.chunk_size))?;
    let chunk_profile = ChunkProfile {
        min_size: chunk_size,
        target_size: chunk_size,
        max_size: chunk_size,
        break_mask: 1,
    };
    let payload_digest = Hash::from(*manifest.blob_hash.as_ref());
    let taikai_hint = taikai_segment_hint_from_manifest(manifest)?;
    let chunks = manifest
        .chunks
        .iter()
        .map(|chunk| CarChunk {
            offset: chunk.offset,
            length: chunk.length,
            digest: *chunk.commitment.as_ref(),
            taikai_segment_hint: taikai_hint.clone(),
        })
        .collect::<Vec<_>>();
    let files = vec![FilePlan {
        path: vec!["payload.bin".to_owned()],
        first_chunk: 0,
        chunk_count: chunks.len(),
        size: manifest.total_size,
    }];
    Ok(CarBuildPlan {
        chunk_profile,
        payload_digest,
        content_length: manifest.total_size,
        chunks,
        files,
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
    let sequence = sequence_raw.trim().parse::<u64>().map_err(|err| {
        PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_SEGMENT_SEQUENCE,
            reason: err.to_string(),
        }
    })?;
    let cache_hint = decode_taikai_cache_hint(metadata)?;

    let mut hint = TaikaiSegmentHint {
        event: event.as_ref().to_owned(),
        stream: stream.as_ref().to_owned(),
        rendition: rendition.as_ref().to_owned(),
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
    fn lookup<'a>(manifest: &'a SorafsManifestV1, key: &'static str) -> Option<&'a str> {
        manifest
            .metadata
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.value.as_str())
    }

    let Some(event_raw) = lookup(manifest, META_TAIKAI_EVENT_ID) else {
        return Ok(None);
    };
    let stream_raw = lookup(manifest, META_TAIKAI_STREAM_ID).ok_or(
        PlanFromManifestError::MissingTaikaiMetadata(META_TAIKAI_STREAM_ID),
    )?;
    let rendition_raw = lookup(manifest, META_TAIKAI_RENDITION_ID).ok_or(
        PlanFromManifestError::MissingTaikaiMetadata(META_TAIKAI_RENDITION_ID),
    )?;
    let sequence_raw = lookup(manifest, META_TAIKAI_SEGMENT_SEQUENCE).ok_or(
        PlanFromManifestError::MissingTaikaiMetadata(META_TAIKAI_SEGMENT_SEQUENCE),
    )?;

    let event =
        Name::from_str(event_raw).map_err(|err| PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_EVENT_ID,
            reason: err.to_string(),
        })?;
    let stream =
        Name::from_str(stream_raw).map_err(|err| PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_STREAM_ID,
            reason: err.to_string(),
        })?;
    let rendition = Name::from_str(rendition_raw).map_err(|err| {
        PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_RENDITION_ID,
            reason: err.to_string(),
        }
    })?;
    let sequence = sequence_raw.trim().parse::<u64>().map_err(|err| {
        PlanFromManifestError::InvalidTaikaiMetadata {
            field: META_TAIKAI_SEGMENT_SEQUENCE,
            reason: err.to_string(),
        }
    })?;
    let cache_hint = decode_taikai_cache_hint_from_sorafs_manifest(manifest)?;

    let mut hint = TaikaiSegmentHint {
        event: event.as_ref().to_owned(),
        stream: stream.as_ref().to_owned(),
        rendition: rendition.as_ref().to_owned(),
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
    let trimmed = raw.trim();
    Name::from_str(trimmed).map_err(|err| PlanFromManifestError::InvalidTaikaiMetadata {
        field: key,
        reason: err.to_string(),
    })
}

#[cfg(feature = "manifest")]
fn read_taikai_metadata_field<'a>(
    metadata: &'a ExtraMetadata,
    key: &'static str,
) -> Result<&'a str, PlanFromManifestError> {
    let entry = metadata
        .items
        .iter()
        .find(|entry| entry.key == key)
        .ok_or(PlanFromManifestError::MissingTaikaiMetadata(key))?;
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
    let Some(entry) = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_TAIKAI_CACHE_HINT)
    else {
        return Ok(None);
    };
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
    let Some(entry) = manifest
        .metadata
        .iter()
        .find(|entry| entry.key == META_TAIKAI_CACHE_HINT)
    else {
        return Ok(None);
    };
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

/// Prototype chunk store used during SoraFS node ingestion.
///
/// Captures chunk metadata, payload digests, and the two-level (64 KiB / 4 KiB) PoR sampling tree.
#[derive(Debug, Clone)]
pub struct ChunkStore {
    profile: sorafs_chunker::ChunkProfile,
    chunks: Vec<StoredChunk>,
    por_tree: PorMerkleTree,
    payload_digest: Hash,
    payload_len: u64,
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
}

/// Writes each chunk into a deterministic directory layout (`chunk_{idx:05}.bin`).
#[derive(Debug, Clone)]
pub struct DirectoryChunkSink {
    root: PathBuf,
    records: Vec<PersistedChunkRecord>,
    total_bytes: u64,
}

impl DirectoryChunkSink {
    /// Construct a sink that writes chunks into `root`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            records: Vec::new(),
            total_bytes: 0,
        }
    }

    fn write_atomic(path: &Path, data: &[u8]) -> Result<(), ChunkStoreError> {
        let parent = path
            .parent()
            .ok_or_else(|| ChunkStoreError::Io(io::Error::other("missing parent directory")))?;
        fs::create_dir_all(parent).map_err(ChunkStoreError::Io)?;
        let mut tmp = path.to_path_buf();
        tmp.add_extension("partial");
        {
            let mut file = File::create(&tmp).map_err(ChunkStoreError::Io)?;
            file.write_all(data).map_err(ChunkStoreError::Io)?;
            file.sync_all().map_err(ChunkStoreError::Io)?;
        }
        fs::rename(&tmp, path).map_err(ChunkStoreError::Io)?;
        Ok(())
    }
}

impl ChunkSink for DirectoryChunkSink {
    type Output = DirectoryChunkSinkOutput;

    fn prepare(&mut self, _plan: &CarBuildPlan) -> Result<(), ChunkStoreError> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root).map_err(ChunkStoreError::Io)?;
        }
        fs::create_dir_all(&self.root).map_err(ChunkStoreError::Io)?;
        self.records.clear();
        self.total_bytes = 0;
        Ok(())
    }

    fn write_chunk(
        &mut self,
        index: usize,
        chunk: &CarChunk,
        data: &[u8],
    ) -> Result<(), ChunkStoreError> {
        let file_name = format!("chunk_{index:05}.bin");
        let path = self.root.join(&file_name);
        Self::write_atomic(&path, data)?;
        self.records.push(PersistedChunkRecord {
            file_name,
            offset: chunk.offset,
            length: chunk.length,
            digest: chunk.digest,
        });
        self.total_bytes = self.total_bytes.checked_add(chunk.length as u64).ok_or(
            ChunkStoreError::LengthMismatch {
                expected: u64::MAX,
                actual: u64::MAX,
            },
        )?;
        Ok(())
    }

    fn finish(self) -> Result<Self::Output, ChunkStoreError> {
        Ok(DirectoryChunkSinkOutput {
            records: self.records,
            total_bytes: self.total_bytes,
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
        }
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

    /// Returns the PDP hot-leaf commitment root (4 KiB granularity) if available.
    #[must_use]
    pub fn pdp_hot_root(&self) -> Option<[u8; 32]> {
        self.por_tree.hot_root()
    }

    /// Returns the PDP segment commitment root (256 KiB granularity) if available.
    #[must_use]
    pub fn pdp_segment_root(&self) -> Option<[u8; 32]> {
        self.por_tree.segment_root()
    }

    /// Returns the total number of PDP hot leaves tracked by the current tree.
    #[must_use]
    pub fn pdp_hot_leaf_count(&self) -> usize {
        self.por_tree.leaf_count()
    }

    /// Returns the total number of PDP segments tracked by the current tree.
    #[must_use]
    pub fn pdp_segment_count(&self) -> usize {
        self.por_tree.segment_count()
    }

    /// Returns the total number of PoR leaves tracked by the current tree.
    #[must_use]
    pub fn por_leaf_count(&self) -> usize {
        self.por_tree.leaf_count()
    }

    /// Samples PoR leaves deterministically using `splitmix64` seeded with `seed`.
    #[must_use]
    pub fn sample_leaves(&self, count: usize, seed: u64, payload: &[u8]) -> Vec<(usize, PorProof)> {
        let mut source = InMemoryPayload::new(payload);
        self.sample_leaves_with(count, seed, &mut source)
            .unwrap_or_default()
    }

    pub fn sample_leaves_with<P: PayloadSource>(
        &self,
        count: usize,
        seed: u64,
        source: &mut P,
    ) -> Result<Vec<(usize, PorProof)>, ChunkStoreError> {
        let total = self.por_tree.leaf_count();
        if total == 0 || count == 0 {
            return Ok(Vec::new());
        }
        let target = count.min(total);
        let mut rng_state = seed;
        let mut seen = HashSet::new();
        let mut samples = Vec::with_capacity(target);
        while samples.len() < target {
            rng_state = splitmix64(rng_state);
            let idx = (rng_state as usize) % total;
            if !seen.insert(idx) {
                continue;
            }
            let (chunk_idx, segment_idx, leaf_idx) = match self.por_tree.leaf_path(idx) {
                Some(path) => path,
                None => continue,
            };
            if let Some(proof) =
                self.por_tree
                    .prove_leaf_with(chunk_idx, segment_idx, leaf_idx, source)?
            {
                samples.push((idx, proof));
            }
        }
        Ok(samples)
    }

    /// Clears the store and ingests the provided payload.
    pub fn ingest_bytes(&mut self, payload: &[u8]) {
        let vectors = sorafs_chunker::chunk_bytes_with_digests_profile(self.profile, payload);
        self.chunks.clear();
        self.chunks.reserve(vectors.len());
        for digest in vectors {
            self.chunks.push(StoredChunk {
                offset: digest.offset as u64,
                length: digest.length as u32,
                blake3: digest.digest,
            });
        }
        self.por_tree = PorMerkleTree::from_payload(payload, &self.chunks);
        self.payload_digest = blake3::hash(payload);
        self.payload_len = payload.len() as u64;
    }

    /// Ingests a payload using chunk boundaries supplied by an existing plan.
    pub fn ingest_plan(&mut self, payload: &[u8], plan: &CarBuildPlan) {
        let mut source = InMemoryPayload::new(payload);
        self.ingest_plan_source(plan, &mut source)
            .expect("ingest_plan_source should not fail for in-memory payloads");
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
        self.profile = plan.chunk_profile;
        self.chunks.clear();
        self.chunks.reserve(plan.chunks.len());

        sink.prepare(plan)?;

        let mut chunk_nodes = Vec::with_capacity(plan.chunks.len());
        let mut chunk_roots = Vec::with_capacity(plan.chunks.len());
        let mut payload_hasher = blake3::Hasher::new();
        let mut buffer = Vec::new();
        let mut last_chunk_end = 0u64;

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

            if idx == 0 {
                if chunk_plan.offset != 0 {
                    return Err(ChunkStoreError::OffsetOutOfRange {
                        offset: chunk_plan.offset,
                        len: 0,
                    });
                }
            } else if chunk_plan.offset != last_chunk_end {
                return Err(ChunkStoreError::OffsetOutOfRange {
                    offset: chunk_plan.offset,
                    len: last_chunk_end,
                });
            }

            self.chunks.push(StoredChunk {
                offset: chunk_plan.offset,
                length: chunk_plan.length,
                blake3: chunk_plan.digest,
            });

            let (chunk_tree, chunk_root) = PorMerkleTree::build_chunk_tree_from_bytes(
                idx,
                chunk_plan.offset,
                chunk_plan.length,
                chunk_plan.digest,
                &buffer,
            );
            chunk_roots.push(chunk_root);
            chunk_nodes.push(chunk_tree);

            sink.write_chunk(idx, chunk_plan, &buffer)?;
            last_chunk_end = chunk_plan
                .offset
                .checked_add(chunk_plan.length as u64)
                .ok_or(ChunkStoreError::LengthMismatch {
                    expected: plan.content_length,
                    actual: u64::MAX,
                })?;
        }

        if last_chunk_end != plan.content_length {
            return Err(ChunkStoreError::LengthMismatch {
                expected: plan.content_length,
                actual: last_chunk_end,
            });
        }

        self.por_tree = PorMerkleTree::from_chunks(chunk_nodes, chunk_roots, plan.content_length);
        self.payload_digest = payload_hasher.finalize();
        self.payload_len = plan.content_length;
        sink.finish()
    }

    /// Ingests chunk metadata while persisting chunk bytes to `directory`.
    pub fn ingest_plan_to_directory<P: PayloadSource>(
        &mut self,
        plan: &CarBuildPlan,
        source: &mut P,
        directory: &Path,
    ) -> Result<DirectoryChunkSinkOutput, ChunkStoreError> {
        let sink = DirectoryChunkSink::new(directory);
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
        let sink = DirectoryChunkSink::new(directory);
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
const POR_ROOT_DOMAIN: &[u8] = b"sorafs:por:root:v1";
const PDP_HOT_DOMAIN: &[u8] = b"sorafs:pdp:hot_root:v1";
const PDP_SEGMENT_DOMAIN: &[u8] = b"sorafs:pdp:segment_root:v1";

/// Two-level Merkle tree used for Proof-of-Retrievability sampling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorMerkleTree {
    root: [u8; 32],
    chunks: Vec<PorChunkTree>,
    payload_len: u64,
}

impl PorMerkleTree {
    /// Returns an empty PoR tree.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            root: [0u8; 32],
            chunks: Vec::new(),
            payload_len: 0,
        }
    }

    /// Builds a PoR tree from the provided payload and chunk metadata.
    #[must_use]
    pub fn from_payload(payload: &[u8], chunks: &[StoredChunk]) -> Self {
        if payload.is_empty() || chunks.is_empty() {
            return Self::empty();
        }

        let mut chunk_nodes = Vec::with_capacity(chunks.len());
        let mut chunk_roots = Vec::with_capacity(chunks.len());
        for (index, chunk) in chunks.iter().enumerate() {
            let chunk_start = chunk.offset as usize;
            let chunk_len = chunk.length as usize;
            let chunk_end = chunk_start.saturating_add(chunk_len).min(payload.len());
            let slice = &payload[chunk_start..chunk_end];
            let (chunk_tree, chunk_root) = Self::build_chunk_tree_from_bytes(
                index,
                chunk.offset,
                chunk.length,
                chunk.blake3,
                slice,
            );
            chunk_roots.push(chunk_root);
            chunk_nodes.push(chunk_tree);
        }

        Self::from_chunks(chunk_nodes, chunk_roots, payload.len() as u64)
    }

    /// Builds a PoR tree from precomputed chunk subtrees and their roots.
    #[must_use]
    pub fn from_chunks(
        chunks: Vec<PorChunkTree>,
        chunk_roots: Vec<[u8; 32]>,
        payload_len: u64,
    ) -> Self {
        debug_assert_eq!(chunks.len(), chunk_roots.len());
        let root = hash_root(payload_len, &chunk_roots);
        Self {
            root,
            chunks,
            payload_len,
        }
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

    /// Returns true if the tree is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }

    /// Returns the total number of PoR leaves tracked by this tree.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        self.chunks
            .iter()
            .flat_map(|chunk| chunk.segments.iter())
            .map(|segment| segment.leaves.len())
            .sum()
    }

    /// Returns the total number of segments tracked by this tree.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.chunks.iter().map(|chunk| chunk.segments.len()).sum()
    }

    /// Computes the PDP hot-leaf commitment root spanning the entire payload.
    #[must_use]
    pub fn hot_root(&self) -> Option<[u8; 32]> {
        if self.is_empty() {
            return None;
        }
        let mut leaves = Vec::new();
        for chunk in &self.chunks {
            for segment in &chunk.segments {
                for leaf in &segment.leaves {
                    leaves.push(leaf.digest);
                }
            }
        }
        Some(hash_pdp_root(PDP_HOT_DOMAIN, self.payload_len, &leaves))
    }

    /// Computes the PDP segment commitment root spanning the entire payload.
    #[must_use]
    pub fn segment_root(&self) -> Option<[u8; 32]> {
        if self.is_empty() {
            return None;
        }
        let mut segments = Vec::new();
        for chunk in &self.chunks {
            for segment in &chunk.segments {
                segments.push(segment.digest);
            }
        }
        Some(hash_pdp_root(
            PDP_SEGMENT_DOMAIN,
            self.payload_len,
            &segments,
        ))
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
                leaf_index = leaf_index.saturating_sub(leaf_len);
            }
        }
        None
    }

    /// Constructs a PoR proof for the specified chunk/segment/leaf tuple.
    pub fn prove_leaf(
        &self,
        chunk_index: usize,
        segment_index: usize,
        leaf_index: usize,
        payload: &[u8],
    ) -> Option<PorProof> {
        let mut source = InMemoryPayload::new(payload);
        self.prove_leaf_with(chunk_index, segment_index, leaf_index, &mut source)
            .ok()
            .flatten()
    }

    pub fn prove_leaf_with<P: PayloadSource>(
        &self,
        chunk_index: usize,
        segment_index: usize,
        leaf_index: usize,
        source: &mut P,
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
        let mut leaf_bytes = vec![0u8; leaf.length as usize];
        source.read_exact(leaf.offset, &mut leaf_bytes)?;

        let segment_leaves = segment
            .leaves
            .iter()
            .map(|entry| entry.digest)
            .collect::<Vec<_>>();
        let chunk_segments = chunk
            .segments
            .iter()
            .map(|entry| entry.digest)
            .collect::<Vec<_>>();
        let chunk_roots = self
            .chunks
            .iter()
            .map(|entry| entry.root)
            .collect::<Vec<_>>();

        Ok(Some(PorProof {
            payload_len: self.payload_len,
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
            chunk_roots,
        }))
    }

    fn build_chunk_tree_from_bytes(
        chunk_index: usize,
        chunk_offset: u64,
        chunk_length: u32,
        chunk_digest: [u8; 32],
        bytes: &[u8],
    ) -> (PorChunkTree, [u8; 32]) {
        debug_assert_eq!(bytes.len(), chunk_length as usize);

        let mut segments = Vec::new();
        let mut segment_hashes = Vec::new();
        let mut segment_start = 0usize;
        while segment_start < bytes.len() {
            let segment_end = (segment_start + POR_SEGMENT_SIZE).min(bytes.len());
            let mut leaves = Vec::new();
            let mut leaf_hashes = Vec::new();
            let mut leaf_start = segment_start;
            while leaf_start < segment_end {
                let leaf_end = (leaf_start + POR_LEAF_SIZE).min(segment_end);
                let absolute_offset = chunk_offset + leaf_start as u64;
                let digest = hash_leaf(absolute_offset, &bytes[leaf_start..leaf_end]);
                leaves.push(PorLeaf {
                    offset: absolute_offset,
                    length: (leaf_end - leaf_start) as u32,
                    digest,
                });
                leaf_hashes.push(digest);
                leaf_start = leaf_end;
            }

            let segment_digest = hash_segment(
                chunk_offset + segment_start as u64,
                (segment_end - segment_start) as u32,
                &leaf_hashes,
            );
            segments.push(PorSegment {
                offset: chunk_offset + segment_start as u64,
                length: (segment_end - segment_start) as u32,
                digest: segment_digest,
                leaves,
            });
            segment_hashes.push(segment_digest);
            segment_start = segment_end;
        }

        let chunk_root = hash_chunk(
            chunk_index as u64,
            chunk_offset,
            chunk_length,
            &chunk_digest,
            &segment_hashes,
        );

        (
            PorChunkTree {
                chunk_index,
                offset: chunk_offset,
                length: chunk_length,
                chunk_digest,
                root: chunk_root,
                segments,
            },
            chunk_root,
        )
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
    pub offset: u64,
    pub length: u32,
    pub digest: [u8; 32],
}

/// Proof-of-Retrievability witness for a single leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorProof {
    pub payload_len: u64,
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
    pub chunk_roots: Vec<[u8; 32]>,
}

impl PorProof {
    /// Verifies the proof against the expected PoR root.
    #[must_use]
    pub fn verify(&self, expected_root: &[u8; 32]) -> bool {
        if self.segment_leaves.get(self.leaf_index) != Some(&self.leaf_digest) {
            return false;
        }
        if self.chunk_segments.get(self.segment_index) != Some(&self.segment_digest) {
            return false;
        }
        if self.chunk_roots.get(self.chunk_index) != Some(&self.chunk_root) {
            return false;
        }

        let recomputed_leaf = hash_leaf(self.leaf_offset, &self.leaf_bytes);
        if recomputed_leaf != self.leaf_digest {
            return false;
        }

        let mut segment_leaves = self.segment_leaves.clone();
        segment_leaves[self.leaf_index] = recomputed_leaf;
        let recomputed_segment =
            hash_segment(self.segment_offset, self.segment_length, &segment_leaves);
        if recomputed_segment != self.segment_digest {
            return false;
        }

        let mut chunk_segments = self.chunk_segments.clone();
        chunk_segments[self.segment_index] = recomputed_segment;
        let recomputed_chunk = hash_chunk(
            self.chunk_index as u64,
            self.chunk_offset,
            self.chunk_length,
            &self.chunk_digest,
            &chunk_segments,
        );
        if recomputed_chunk != self.chunk_root {
            return false;
        }

        let mut chunk_roots = self.chunk_roots.clone();
        chunk_roots[self.chunk_index] = recomputed_chunk;
        let recomputed_root = hash_root(self.payload_len, &chunk_roots);
        &recomputed_root == expected_root
    }
}

fn hash_leaf(offset: u64, bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(POR_LEAF_DOMAIN);
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

fn hash_pdp_root(domain: &[u8], payload_len: u64, digests: &[[u8; 32]]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(domain);
    hasher.update(&payload_len.to_le_bytes());
    hasher.update(&(digests.len() as u64).to_le_bytes());
    for digest in digests {
        hasher.update(digest);
    }
    hasher.finalize().into()
}

fn hash_root(total_len: u64, chunk_roots: &[[u8; 32]]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(POR_ROOT_DOMAIN);
    hasher.update(&total_len.to_le_bytes());
    hasher.update(&(chunk_roots.len() as u64).to_le_bytes());
    for root in chunk_roots {
        hasher.update(root);
    }
    hasher.finalize().into()
}

fn splitmix64(mut state: u64) -> u64 {
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

/// Ingests a single payload using the default registry profile and derives a CAR plan.
pub fn ingest_single_file(bytes: &[u8]) -> Result<IngestSummary, CarPlanError> {
    let mut chunk_store = ChunkStore::new();
    chunk_store.ingest_bytes(bytes);
    let plan = CarBuildPlan::single_file(bytes)?;
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
        if plan.content_length != payload.len() as u64 {
            return Err(CarWriteError::PayloadMismatch);
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
        let layout = CarLayout::new(self.plan)?;
        if self
            .expected_roots
            .as_ref()
            .is_some_and(|expected| expected != &layout.root_cids)
        {
            return Err(CarWriteError::RootMismatch);
        }
        let mut buffer = Vec::<u8>::new();
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
    Ok(buf.len() as u64)
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
    fn data_len(&self, plan: &CarBuildPlan) -> usize {
        match &self.data {
            SectionData::Chunk { chunk_index } => plan.chunks[*chunk_index].length as usize,
            SectionData::Node(bytes) => bytes.len(),
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

#[derive(Clone)]
struct FileRootInfo {
    path: Vec<String>,
    cid: Vec<u8>,
    size: u64,
}

struct DirectoryDag {
    nodes: Vec<TreeNode>,
    root_cid: Vec<u8>,
}

#[derive(Default)]
struct DirectoryBuilderNode {
    entries: BTreeMap<String, DirectoryBuilderEntry>,
}

enum DirectoryBuilderEntry {
    File(DirectoryFile),
    Directory(Box<DirectoryBuilderNode>),
}

#[derive(Clone)]
struct DirectoryFile {
    cid: Vec<u8>,
    size: u64,
}

#[derive(Clone)]
struct DirectoryEntry {
    name: String,
    cid: Vec<u8>,
    kind: DirectoryEntryKind,
    size: u64,
}

fn format_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn gather_files(root: &Path, current: &Path, acc: &mut Vec<FileEntry>) -> Result<(), CarPlanError> {
    let mut entries = fs::read_dir(current)
        .map_err(|err| CarPlanError::InvalidPath(format!("{}: {err}", format_path(current))))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| CarPlanError::InvalidPath(format!("{}: {err}", format_path(current))))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|err| CarPlanError::InvalidPath(format!("{}: {err}", format_path(&path))))?;
        if metadata.is_dir() {
            gather_files(root, &path, acc)?;
        } else if metadata.is_file() {
            let rel = path.strip_prefix(root).map_err(|err| {
                CarPlanError::InvalidPath(format!("{}: {err}", format_path(&path)))
            })?;
            let mut components = Vec::new();
            for component in rel.components() {
                match component {
                    Component::Normal(os) => {
                        let value = os
                            .to_str()
                            .ok_or_else(|| CarPlanError::NonUtf8Path(format_path(&path)))?;
                        if value.is_empty() || value.contains('/') {
                            return Err(CarPlanError::InvalidPath(format_path(&path)));
                        }
                        components.push(value.to_string());
                    }
                    Component::CurDir => {}
                    _ => return Err(CarPlanError::InvalidPath(format_path(&path))),
                }
            }
            let data = fs::read(&path).map_err(|err| {
                CarPlanError::InvalidPath(format!("{}: {err}", format_path(&path)))
            })?;
            acc.push(FileEntry {
                path: components,
                data,
            });
        } else {
            return Err(CarPlanError::InvalidPath(format_path(&path)));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum DirectoryEntryKind {
    File,
    Directory,
}

impl CarLayout {
    fn new(plan: &CarBuildPlan) -> Result<Self, CarWriteError> {
        let chunk_cids: Vec<Vec<u8>> = plan
            .chunks
            .iter()
            .map(|chunk| encode_cid(RAW_CODEC, &chunk.digest))
            .collect();
        let mut file_root_infos = Vec::with_capacity(plan.files.len());
        let mut file_nodes = Vec::with_capacity(plan.files.len());

        for file in &plan.files {
            let start = file.first_chunk;
            let end = start + file.chunk_count;
            let chunk_refs: Vec<ChunkRef<'_>> = (start..end)
                .map(|idx| ChunkRef {
                    cid: chunk_cids[idx].as_slice(),
                    length: plan.chunks[idx].length,
                })
                .collect();
            let file_dag = build_file_dag(&chunk_refs, file.size);
            let root_node = file_dag
                .nodes
                .get(file_dag.root_index)
                .expect("valid file root")
                .clone();
            file_root_infos.push(FileRootInfo {
                path: file.path.clone(),
                cid: root_node.cid_bytes.clone(),
                size: file.size,
            });
            file_nodes.push(file_dag.nodes);
        }

        let needs_directory = plan.files.len() != 1 || !plan.files[0].path.is_empty();
        let directory = if needs_directory {
            Some(build_directory_dag(&file_root_infos))
        } else {
            None
        };

        let root_cids = if let Some(dir) = &directory {
            vec![dir.root_cid.clone()]
        } else {
            vec![
                file_root_infos
                    .first()
                    .expect("at least one file")
                    .cid
                    .clone(),
            ]
        };

        let carv1_header_bytes = encode_carv1_header(&root_cids)?;
        let carv1_header_prefix = encode_uleb128_vec(carv1_header_bytes.len() as u64);
        let header_len = (carv1_header_prefix.len() + carv1_header_bytes.len()) as u64;

        let mut sections = Vec::with_capacity(
            plan.chunks.len()
                + file_nodes.iter().map(|nodes| nodes.len()).sum::<usize>()
                + directory.as_ref().map(|dir| dir.nodes.len()).unwrap_or(0),
        );

        for (idx, chunk) in plan.chunks.iter().enumerate() {
            let cid_bytes = chunk_cids[idx].clone();
            let section_length = cid_bytes.len() + chunk.length as usize;
            let length_varint = encode_uleb128_vec(section_length as u64);
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
                let section_length = node.cid_bytes.len() + node.data.len();
                let length_varint = encode_uleb128_vec(section_length as u64);
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
                let section_length = node.cid_bytes.len() + node.data.len();
                let length_varint = encode_uleb128_vec(section_length as u64);
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
            let section_size = section.length_varint.len() as u64
                + section.cid_bytes.len() as u64
                + section.data_len(plan) as u64;
            section_offset += section_size;
            payload_len += section_size;
        }

        let data_offset = PRAGMA.len() as u64 + HEADER_LEN as u64;
        let mut characteristics = [0u8; 16];

        let index_bytes = build_index(&sections);
        let index_offset = if index_bytes.is_some() {
            characteristics[0] |= 0x80;
            Some(data_offset + payload_len)
        } else {
            None
        };

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

        total_written += write_buffer(writer, &mut file_hasher, None, &PRAGMA)?;
        total_written += write_buffer(writer, &mut file_hasher, None, &self.header_bytes)?;
        total_written += write_buffer(
            writer,
            &mut file_hasher,
            Some(&mut payload_hasher),
            &self.carv1_header_prefix,
        )?;
        total_written += write_buffer(
            writer,
            &mut file_hasher,
            Some(&mut payload_hasher),
            &self.carv1_header_bytes,
        )?;
        total_written += self.write_sections(
            plan,
            writer,
            &mut file_hasher,
            &mut payload_hasher,
            &mut chunk_writer,
        )?;
        if let Some(index_bytes) = &self.index_bytes {
            total_written += write_buffer(writer, &mut file_hasher, None, index_bytes)?;
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
            root_cids: self.root_cids.clone(),
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
            written += write_buffer(
                writer,
                file_hasher,
                Some(payload_hasher),
                &section.length_varint,
            )?;
            written += write_buffer(
                writer,
                file_hasher,
                Some(payload_hasher),
                &section.cid_bytes,
            )?;
            match &section.data {
                SectionData::Chunk { chunk_index } => {
                    written += chunk_writer(
                        *chunk_index,
                        &plan.chunks[*chunk_index],
                        writer,
                        file_hasher,
                        payload_hasher,
                    )?;
                }
                SectionData::Node(bytes) => {
                    written += write_buffer(writer, file_hasher, Some(payload_hasher), bytes)?;
                }
            }
        }
        Ok(written)
    }
}

fn build_file_dag(chunks: &[ChunkRef<'_>], expected_size: u64) -> FileDag {
    let mut nodes: Vec<TreeNode> = Vec::new();
    let mut current_indices: Vec<usize> = Vec::new();

    for group in chunks.chunks(MAX_FANOUT) {
        let node = build_leaf_node(group);
        nodes.push(node);
        current_indices.push(nodes.len() - 1);
    }

    if current_indices.is_empty() {
        let node = build_leaf_node(&[]);
        nodes.push(node);
        current_indices.push(nodes.len() - 1);
    }

    while current_indices.len() > 1 {
        let mut next_indices =
            Vec::with_capacity(div_ceil_usize(current_indices.len(), MAX_FANOUT));
        for group in current_indices.chunks(MAX_FANOUT) {
            let children: Vec<&TreeNode> = group.iter().map(|idx| &nodes[*idx]).collect();
            let node = build_branch_node(&children);
            nodes.push(node);
            next_indices.push(nodes.len() - 1);
        }
        current_indices = next_indices;
    }

    let root_index = *current_indices
        .first()
        .expect("file DAG must contain at least one node");

    debug_assert_eq!(nodes[root_index].size, expected_size);

    FileDag { nodes, root_index }
}

fn build_leaf_node(group: &[ChunkRef<'_>]) -> TreeNode {
    let total_size: u64 = group.iter().map(|chunk| u64::from(chunk.length)).sum();
    let mut buf = Vec::new();
    encode_cbor_map(&mut buf, 4);
    encode_cbor_text(&mut buf, "chunks");
    encode_cbor_array(&mut buf, group.len() as u64);
    for chunk in group {
        encode_cbor_array(&mut buf, 2);
        encode_cbor_bytes(&mut buf, chunk.cid.len() as u64);
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
    TreeNode {
        cid_bytes,
        data: buf,
        digest,
        size: total_size,
    }
}

fn build_branch_node(group: &[&TreeNode]) -> TreeNode {
    let total_size: u64 = group.iter().map(|node| node.size).sum();
    let mut buf = Vec::new();
    encode_cbor_map(&mut buf, 4);
    encode_cbor_text(&mut buf, "children");
    encode_cbor_array(&mut buf, group.len() as u64);
    for child in group {
        encode_cbor_array(&mut buf, 2);
        encode_cbor_bytes(&mut buf, child.cid_bytes.len() as u64);
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
    TreeNode {
        cid_bytes,
        data: buf,
        digest,
        size: total_size,
    }
}

fn build_directory_dag(files: &[FileRootInfo]) -> DirectoryDag {
    let mut root = DirectoryBuilderNode::default();
    for file in files {
        insert_directory_entry(
            &mut root,
            &file.path,
            DirectoryFile {
                cid: file.cid.clone(),
                size: file.size,
            },
        );
    }

    let mut nodes = Vec::new();
    let (root_cid, _) = materialize_directory_nodes(&root, &mut nodes);

    DirectoryDag { nodes, root_cid }
}

fn insert_directory_entry(node: &mut DirectoryBuilderNode, path: &[String], file: DirectoryFile) {
    let (head, tail) = path
        .split_first()
        .expect("directory insertion requires non-empty path");
    if tail.is_empty() {
        node.entries
            .insert(head.clone(), DirectoryBuilderEntry::File(file));
    } else {
        let child = node.entries.entry(head.clone()).or_insert_with(|| {
            DirectoryBuilderEntry::Directory(Box::<DirectoryBuilderNode>::default())
        });
        match child {
            DirectoryBuilderEntry::File(_) => {
                panic!("path conflict detected during directory build")
            }
            DirectoryBuilderEntry::Directory(dir) => insert_directory_entry(dir, tail, file),
        }
    }
}

fn materialize_directory_nodes(
    node: &DirectoryBuilderNode,
    nodes: &mut Vec<TreeNode>,
) -> (Vec<u8>, u64) {
    let mut entries = Vec::with_capacity(node.entries.len());
    let mut total_size = 0u64;

    for (name, entry) in node.entries.iter() {
        match entry {
            DirectoryBuilderEntry::File(file) => {
                total_size += file.size;
                entries.push(DirectoryEntry {
                    name: name.clone(),
                    cid: file.cid.clone(),
                    kind: DirectoryEntryKind::File,
                    size: file.size,
                });
            }
            DirectoryBuilderEntry::Directory(child) => {
                let (child_cid, child_size) = materialize_directory_nodes(child, nodes);
                total_size += child_size;
                entries.push(DirectoryEntry {
                    name: name.clone(),
                    cid: child_cid,
                    kind: DirectoryEntryKind::Directory,
                    size: child_size,
                });
            }
        }
    }

    let dir_node = build_directory_node(&entries, total_size);
    let cid = dir_node.cid_bytes.clone();
    nodes.push(dir_node);
    (cid, total_size)
}

fn build_directory_node(entries: &[DirectoryEntry], size: u64) -> TreeNode {
    let mut buf = Vec::new();
    encode_cbor_map(&mut buf, 4);
    encode_cbor_text(&mut buf, "entries");
    encode_cbor_array(&mut buf, entries.len() as u64);
    for entry in entries {
        encode_cbor_map(&mut buf, 4);
        encode_cbor_text(&mut buf, "name");
        encode_cbor_text(&mut buf, &entry.name);
        encode_cbor_text(&mut buf, "cid");
        encode_cbor_bytes(&mut buf, entry.cid.len() as u64);
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
    TreeNode {
        cid_bytes,
        data: buf,
        digest,
        size,
    }
}

fn encode_carv1_header(roots: &[Vec<u8>]) -> Result<Vec<u8>, CarWriteError> {
    let mut buf = Vec::new();
    buf.push(0xa2);
    encode_cbor_text(&mut buf, "roots");
    encode_cbor_array(&mut buf, roots.len() as u64);
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

fn encode_cid(codec: u64, digest: &[u8]) -> Vec<u8> {
    let mut cid = Vec::with_capacity(16 + digest.len());
    encode_uleb128(0x01, &mut cid);
    encode_uleb128(codec, &mut cid);
    encode_uleb128(BLAKE3_256_MULTIHASH_CODE, &mut cid);
    encode_uleb128(digest.len() as u64, &mut cid);
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

type DigestBucket = Vec<([u8; 32], u64)>;
type DigestBuckets = BTreeMap<(u64, u32), DigestBucket>;

fn build_index(sections: &[CarSection]) -> Option<Vec<u8>> {
    if sections.is_empty() {
        return None;
    }
    let mut buckets: DigestBuckets = BTreeMap::new();
    for section in sections {
        let digest = section.digest;
        let offset = section.offset;
        let key = (BLAKE3_256_MULTIHASH_CODE, digest.len() as u32);
        buckets.entry(key).or_default().push((digest, offset));
    }
    for entries in buckets.values_mut() {
        entries.sort_by(|a, b| a.0.cmp(&b.0));
    }

    let total_entries: usize = buckets.values().map(Vec::len).sum();
    let mut buf = Vec::with_capacity(16 + total_entries * 40);
    encode_uleb128(0x0401, &mut buf);
    for ((mh_code, digest_size), entries) in buckets {
        buf.extend_from_slice(&mh_code.to_le_bytes());
        buf.extend_from_slice(&digest_size.to_le_bytes());
        buf.extend_from_slice(&(entries.len() as u64).to_le_bytes());
        for (digest, offset) in entries {
            buf.extend_from_slice(&digest[..digest_size as usize]);
            buf.extend_from_slice(&offset.to_le_bytes());
        }
    }
    Some(buf)
}

impl CarBuildPlan {
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
        let chunks = chunk_bytes_with_digests_profile(profile, payload);
        let chunk_count = chunks.len();
        Ok(Self {
            chunk_profile: profile,
            payload_digest: blake3::hash(payload),
            content_length: payload.len() as u64,
            chunks: chunks.into_iter().map(CarChunk::from).collect(),
            files: vec![FilePlan {
                path: Vec::new(),
                first_chunk: 0,
                chunk_count,
                size: payload.len() as u64,
            }],
        })
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
        if !root.is_dir() {
            return Err(CarPlanError::InvalidPath(format_path(root)));
        }
        let mut files = Vec::new();
        gather_files(root, root, &mut files)?;
        if files.is_empty() {
            return Err(CarPlanError::EmptyInput);
        }
        Self::from_files_with_profile(files, profile)
    }

    /// Builds a CAR plan for multiple files, returning the plan alongside the
    /// concatenated payload bytes that must be passed to [`CarWriter`]. The
    /// files are addressed by their UTF-8 path components (relative to the
    /// dataset root). Paths are validated to be non-empty, free of separators,
    /// and unique.
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

        files.sort_by(|a, b| a.path.cmp(&b.path));
        let mut prev_path: Option<Vec<String>> = None;
        let mut chunks = Vec::new();
        let mut file_plans = Vec::with_capacity(files.len());
        let mut payload = Vec::new();
        let mut hasher = blake3::Hasher::new();
        let mut base_offset = 0u64;

        for entry in files.into_iter() {
            validate_path(&entry.path)?;
            if let Some(prev) = &prev_path {
                if prev == &entry.path {
                    return Err(CarPlanError::DuplicatePath(path_to_string(&entry.path)));
                }
                if entry.path.starts_with(prev) {
                    return Err(CarPlanError::PathConflict {
                        existing: path_to_string(prev),
                        new: path_to_string(&entry.path),
                    });
                }
            }
            prev_path = Some(entry.path.clone());

            let start_chunk = chunks.len();
            let data_len = entry.data.len() as u64;
            hasher.update(&entry.data);
            payload.extend_from_slice(&entry.data);

            let chunk_digests = chunk_bytes_with_digests_profile(profile, &entry.data);
            for digest in chunk_digests {
                chunks.push(CarChunk {
                    offset: base_offset + digest.offset as u64,
                    length: digest.length as u32,
                    digest: digest.digest,
                    taikai_segment_hint: None,
                });
            }
            let end_chunk = chunks.len();
            file_plans.push(FilePlan {
                path: entry.path,
                first_chunk: start_chunk,
                chunk_count: end_chunk - start_chunk,
                size: data_len,
            });
            base_offset += data_len;
        }

        let payload_digest = hasher.finalize();
        Ok((
            Self {
                chunk_profile: profile,
                payload_digest,
                content_length: payload.len() as u64,
                chunks,
                files: file_plans,
            },
            payload,
        ))
    }

    /// Returns the list of chunk fetch specifications derived from the plan.
    ///
    /// This helper is convenient for multi-source retrieval orchestrators that
    /// need to schedule chunk downloads while verifying digests and payload
    /// offsets deterministically.
    #[must_use]
    pub fn chunk_fetch_specs(&self) -> Vec<ChunkFetchSpec> {
        self.chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| ChunkFetchSpec {
                chunk_index: index,
                offset: chunk.offset,
                length: chunk.length,
                digest: chunk.digest,
                taikai_segment_hint: chunk.taikai_segment_hint.clone(),
            })
            .collect()
    }
}

fn validate_path(path: &[String]) -> Result<(), CarPlanError> {
    if path.is_empty() {
        return Err(CarPlanError::InvalidPath("".into()));
    }
    for component in path {
        if component.is_empty() || component.contains('/') {
            return Err(CarPlanError::InvalidPath(path_to_string(path)));
        }
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

impl From<ChunkDigest> for CarChunk {
    fn from(chunk: ChunkDigest) -> Self {
        Self {
            offset: chunk.offset as u64,
            length: chunk.length as u32,
            digest: chunk.digest,
            taikai_segment_hint: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, HashSet},
        fs,
        io::Cursor,
    };

    use sorafs_chunker::fixtures::FixtureProfile;
    use tempfile::tempdir;

    use super::*;

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
        store.ingest_bytes(&vectors.input);
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
                    let expected_leaf = hash_leaf(leaf.offset, &vectors.input[start..end]);
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

        let expected_root = hash_root(store.payload_len(), &expected_chunk_roots);
        assert_eq!(por_tree.root(), &expected_root);
    }

    #[test]
    fn chunk_store_persists_chunks_to_directory() {
        let payload = b"deterministic chunk sink payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut store = ChunkStore::with_profile(plan.chunk_profile);
        let dir = tempdir().expect("temp dir");

        let mut source = InMemoryPayload::new(payload);
        let output = store
            .ingest_plan_to_directory(&plan, &mut source, dir.path())
            .expect("ingest directory");

        assert_eq!(output.total_bytes, plan.content_length);
        assert_eq!(output.records.len(), plan.chunks.len());

        for record in &output.records {
            let chunk_path = dir.path().join(&record.file_name);
            let bytes = fs::read(&chunk_path).expect("chunk file");
            assert_eq!(bytes.len(), record.length as usize);
            assert_eq!(blake3::hash(&bytes).as_bytes(), &record.digest);
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
        let dir = tempdir().expect("dir");
        let mut reader = Cursor::new(payload.clone());

        let output = store
            .ingest_plan_stream_to_directory(&plan, &mut reader, dir.path())
            .expect("stream ingest");

        assert_eq!(output.total_bytes, plan.content_length);
        assert_eq!(output.records.len(), plan.chunks.len());
        assert_eq!(reader.position(), payload.len() as u64);
    }

    #[test]
    fn por_proof_verification_succeeds() {
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        let mut store = ChunkStore::new();
        store.ingest_bytes(&vectors.input);
        let tree = store.por_tree();
        let proof = tree
            .prove_leaf(0, 0, 0, &vectors.input)
            .expect("proof for first leaf");
        assert!(proof.verify(tree.root()));

        let mut tampered = proof.clone();
        tampered.leaf_bytes[0] ^= 0xFF;
        assert!(!tampered.verify(tree.root()));
    }

    #[test]
    fn por_sampling_is_deterministic_and_unique() {
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        let mut store = ChunkStore::new();
        store.ingest_bytes(&vectors.input);
        let samples_a = store.sample_leaves(8, 12345, &vectors.input);
        let samples_b = store.sample_leaves(8, 12345, &vectors.input);
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
    fn sampling_truncates_to_leaf_count() {
        let vectors = FixtureProfile::SF1_V1.generate_vectors();
        let mut store = ChunkStore::new();
        store.ingest_bytes(&vectors.input);
        let total = store.por_leaf_count();
        let samples = store.sample_leaves(total + 10, 7, &vectors.input);
        assert_eq!(samples.len(), total);
    }

    #[test]
    fn por_tree_empty_payload() {
        let mut store = ChunkStore::new();
        store.ingest_bytes(&[]);
        let tree = store.por_tree();
        assert!(tree.is_empty());
        assert_eq!(tree.root(), &[0u8; 32]);
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

    #[test]
    fn empty_input_rejected() {
        let err = CarBuildPlan::single_file(&[]).unwrap_err();
        assert!(matches!(err, CarPlanError::EmptyInput));
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
        let writer = CarWriter::new(&plan, &input).expect("writer");
        let mut sink = Vec::new();
        let err = writer.write_to(&mut sink).expect_err("digest mismatch");
        assert!(matches!(err, CarWriteError::DigestMismatch { .. }));
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
        store_mem.ingest_plan(&input, &plan);

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
        store_mem.ingest_plan(&payload, &plan);

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
    fn chunk_fetch_specs_reflect_plan() {
        let input = sample_input();
        let plan = CarBuildPlan::single_file(&input).expect("plan");
        let specs = plan.chunk_fetch_specs();
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
