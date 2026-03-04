//! Data structures describing Taikai live-broadcast artefacts.
//!
//! The Taikai Segment Envelope (TSE) captures the metadata required to map a
//! low-latency broadcast segment to its deterministic data availability
//! manifest, CAR commitments, and playback descriptors. Hosts and clients use
//! the envelope to anchor CMAF ladders, enforce policy, and build viewer
//! dashboards without re-deriving ingest metadata.

use core::{fmt, str::FromStr};
use std::collections::{BTreeMap, BTreeSet};

use derive_more::Display;
use iroha_crypto::{PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    account::AccountId,
    da::types::{BlobDigest, ExtraMetadata, StorageTicketId},
    name::Name,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{ManifestAliasBinding, StorageClass},
    },
};

/// Identifier assigned to a Taikai event (e.g., a live stream or conference day).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[repr(transparent)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct TaikaiEventId(pub Name);

impl TaikaiEventId {
    /// Construct a new Taikai event identifier.
    #[must_use]
    pub fn new(name: Name) -> Self {
        Self(name)
    }

    /// Access the underlying [`Name`].
    #[must_use]
    pub fn as_name(&self) -> &Name {
        &self.0
    }
}

impl fmt::Display for TaikaiEventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifier assigned to a logical stream within an event (e.g., stage feed).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[repr(transparent)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct TaikaiStreamId(pub Name);

impl TaikaiStreamId {
    /// Construct a new stream identifier.
    #[must_use]
    pub fn new(name: Name) -> Self {
        Self(name)
    }

    /// Access the underlying [`Name`].
    #[must_use]
    pub fn as_name(&self) -> &Name {
        &self.0
    }
}

impl fmt::Display for TaikaiStreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifier assigned to a rendition (ladder rung) within a stream.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[repr(transparent)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct TaikaiRenditionId(pub Name);

impl TaikaiRenditionId {
    /// Construct a new rendition identifier.
    #[must_use]
    pub fn new(name: Name) -> Self {
        Self(name)
    }

    /// Access the underlying [`Name`].
    #[must_use]
    pub fn as_name(&self) -> &Name {
        &self.0
    }
}

impl fmt::Display for TaikaiRenditionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Alias binding wrapper reused by Taikai manifests.
pub type TaikaiAliasBinding = ManifestAliasBinding;

/// Presentation timestamp offset for the start of a segment, expressed in microseconds.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash, Default,
)]
#[repr(transparent)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct SegmentTimestamp(pub u64);

impl SegmentTimestamp {
    /// Construct a timestamp in microseconds.
    #[must_use]
    pub const fn new(micros: u64) -> Self {
        Self(micros)
    }

    /// Retrieve the timestamp value in microseconds.
    #[must_use]
    pub const fn as_micros(&self) -> u64 {
        self.0
    }
}

/// Presentation duration for a segment expressed in microseconds.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash, Default,
)]
#[repr(transparent)]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(transparent))]
pub struct SegmentDuration(pub u32);

impl SegmentDuration {
    /// Construct a duration in microseconds.
    #[must_use]
    pub const fn new(micros: u32) -> Self {
        Self(micros)
    }

    /// Retrieve the duration in microseconds.
    #[must_use]
    pub const fn as_micros(&self) -> u32 {
        self.0
    }
}

/// Errors encountered while parsing Taikai metadata fields from textual representations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaikaiParseError {
    /// Track kind string uses an unknown value.
    UnknownTrackKind(String),
    /// Codec string uses an unknown value.
    UnknownCodec(String),
    /// Resolution string has an invalid format (expected `WIDTHxHEIGHT`).
    InvalidResolution(String),
    /// Audio layout string uses an unknown value.
    UnknownAudioLayout(String),
    /// Audio layout `custom:<channels>` payload is invalid.
    InvalidAudioLayoutChannels(String),
}

impl fmt::Display for TaikaiParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTrackKind(value) => {
                write!(f, "unknown Taikai track kind `{value}`")
            }
            Self::UnknownCodec(value) => write!(f, "unknown Taikai codec `{value}`"),
            Self::InvalidResolution(value) => write!(
                f,
                "invalid Taikai resolution `{value}` (expected WIDTHxHEIGHT)"
            ),
            Self::UnknownAudioLayout(value) => {
                write!(f, "unknown Taikai audio layout `{value}`")
            }
            Self::InvalidAudioLayoutChannels(value) => write!(
                f,
                "invalid Taikai audio layout channels `{value}` (expected positive integer)"
            ),
        }
    }
}

impl std::error::Error for TaikaiParseError {}

/// Supported track kinds for Taikai segments.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash, Default,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "kind", content = "value", no_fast_from_json)
)]
pub enum TaikaiTrackKind {
    /// Video track carrying CMAF fragments.
    #[default]
    Video,
    /// Audio track carrying CMAF fragments.
    Audio,
    /// Ancillary data track (subtitles, timed metadata, etc.).
    Data,
}

impl FromStr for TaikaiTrackKind {
    type Err = TaikaiParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.eq_ignore_ascii_case("video") {
            Ok(Self::Video)
        } else if trimmed.eq_ignore_ascii_case("audio") {
            Ok(Self::Audio)
        } else if trimmed.eq_ignore_ascii_case("data") {
            Ok(Self::Data)
        } else {
            Err(TaikaiParseError::UnknownTrackKind(trimmed.to_string()))
        }
    }
}

/// Video resolution in pixels.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash, Default,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiResolution {
    /// Horizontal pixel count.
    pub width: u16,
    /// Vertical pixel count.
    pub height: u16,
}

impl TaikaiResolution {
    /// Construct a resolution descriptor.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

impl FromStr for TaikaiResolution {
    type Err = TaikaiParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        let Some((width_str, height_str)) = trimmed.split_once(['x', 'X']) else {
            return Err(TaikaiParseError::InvalidResolution(trimmed.to_string()));
        };
        let width = width_str
            .trim()
            .parse::<u16>()
            .map_err(|_| TaikaiParseError::InvalidResolution(trimmed.to_string()))?;
        let height = height_str
            .trim()
            .parse::<u16>()
            .map_err(|_| TaikaiParseError::InvalidResolution(trimmed.to_string()))?;
        if width == 0 || height == 0 {
            return Err(TaikaiParseError::InvalidResolution(trimmed.to_string()));
        }
        Ok(Self::new(width, height))
    }
}

/// Audio channel layout descriptor.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash, Default,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "layout", content = "channels", no_fast_from_json)
)]
pub enum TaikaiAudioLayout {
    /// Mono (1.0) layout.
    Mono,
    /// Stereo (2.0) layout.
    #[default]
    Stereo,
    /// 5.1 surround layout.
    FiveOne,
    /// 7.1 surround layout.
    SevenOne,
    /// Custom channel count approved by governance.
    Custom(u8),
}

impl FromStr for TaikaiAudioLayout {
    type Err = TaikaiParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.eq_ignore_ascii_case("mono") {
            Ok(Self::Mono)
        } else if trimmed.eq_ignore_ascii_case("stereo") {
            Ok(Self::Stereo)
        } else if trimmed.eq_ignore_ascii_case("5.1") {
            Ok(Self::FiveOne)
        } else if trimmed.eq_ignore_ascii_case("7.1") {
            Ok(Self::SevenOne)
        } else if trimmed.len() >= 7 && trimmed[..7].eq_ignore_ascii_case("custom:") {
            let channels_str = trimmed[7..].trim();
            if channels_str.is_empty() {
                return Err(TaikaiParseError::InvalidAudioLayoutChannels(
                    channels_str.to_string(),
                ));
            }
            let channels = channels_str.parse::<u8>().map_err(|_| {
                TaikaiParseError::InvalidAudioLayoutChannels(channels_str.to_string())
            })?;
            if channels == 0 {
                return Err(TaikaiParseError::InvalidAudioLayoutChannels(
                    channels_str.to_string(),
                ));
            }
            Ok(Self::Custom(channels))
        } else {
            Err(TaikaiParseError::UnknownAudioLayout(trimmed.to_string()))
        }
    }
}

/// Codec enumeration recognised by the Taikai pipeline.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash, Default,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "codec", content = "profile", no_fast_from_json)
)]
pub enum TaikaiCodec {
    /// H.264 High profile.
    #[default]
    AvcHigh,
    /// H.265 Main10 profile.
    HevcMain10,
    /// AV1 Main profile.
    Av1Main,
    /// AAC-LC audio.
    AacLc,
    /// Opus audio.
    Opus,
    /// Governance-approved custom codec identified by name.
    Custom(String),
}

impl FromStr for TaikaiCodec {
    type Err = TaikaiParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let trimmed = value.trim();
        if trimmed.eq_ignore_ascii_case("avc-high") {
            Ok(Self::AvcHigh)
        } else if trimmed.eq_ignore_ascii_case("hevc-main10") {
            Ok(Self::HevcMain10)
        } else if trimmed.eq_ignore_ascii_case("av1-main") {
            Ok(Self::Av1Main)
        } else if trimmed.eq_ignore_ascii_case("aac-lc") {
            Ok(Self::AacLc)
        } else if trimmed.eq_ignore_ascii_case("opus") {
            Ok(Self::Opus)
        } else if trimmed.len() >= 7 && trimmed[..7].eq_ignore_ascii_case("custom:") {
            let profile = trimmed[7..].trim();
            if profile.is_empty() {
                return Err(TaikaiParseError::UnknownCodec(trimmed.to_string()));
            }
            Ok(Self::Custom(profile.to_string()))
        } else {
            Err(TaikaiParseError::UnknownCodec(trimmed.to_string()))
        }
    }
}

/// Metadata associated with a Taikai track.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiTrackMetadata {
    /// Track kind (video/audio/data).
    pub kind: TaikaiTrackKind,
    /// Codec information.
    pub codec: TaikaiCodec,
    /// Average bitrate in kilobits per second.
    pub average_bitrate_kbps: u32,
    /// Optional video resolution. Only set for video tracks.
    #[norito(default)]
    pub resolution: Option<TaikaiResolution>,
    /// Optional audio channel layout. Only set for audio tracks.
    #[norito(default)]
    pub audio_layout: Option<TaikaiAudioLayout>,
}

impl TaikaiTrackMetadata {
    /// Build metadata for a video track.
    #[must_use]
    pub fn video(codec: TaikaiCodec, bitrate_kbps: u32, resolution: TaikaiResolution) -> Self {
        Self {
            kind: TaikaiTrackKind::Video,
            codec,
            average_bitrate_kbps: bitrate_kbps,
            resolution: Some(resolution),
            audio_layout: None,
        }
    }

    /// Build metadata for an audio track.
    #[must_use]
    pub fn audio(codec: TaikaiCodec, bitrate_kbps: u32, layout: TaikaiAudioLayout) -> Self {
        Self {
            kind: TaikaiTrackKind::Audio,
            codec,
            average_bitrate_kbps: bitrate_kbps,
            resolution: None,
            audio_layout: Some(layout),
        }
    }

    /// Build metadata for a data track.
    #[must_use]
    pub fn data(codec: TaikaiCodec, bitrate_kbps: u32) -> Self {
        Self {
            kind: TaikaiTrackKind::Data,
            codec,
            average_bitrate_kbps: bitrate_kbps,
            resolution: None,
            audio_layout: None,
        }
    }
}

/// CAR commitment pointer linking a segment to its deterministic archive.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiCarPointer {
    /// Multibase-encoded DAG-CBOR CID of the CAR payload.
    pub cid_multibase: String,
    /// BLAKE3 digest of the `CARv2` file (pragma + header + payload + index).
    pub car_digest: BlobDigest,
    /// Total CAR length in bytes.
    pub car_size_bytes: u64,
}

impl TaikaiCarPointer {
    /// Construct a CAR pointer descriptor.
    #[must_use]
    pub fn new(
        cid_multibase: impl Into<String>,
        car_digest: BlobDigest,
        car_size_bytes: u64,
    ) -> Self {
        Self {
            cid_multibase: cid_multibase.into(),
            car_digest,
            car_size_bytes,
        }
    }
}

/// Pointer to the canonical DA manifest generated during ingest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiIngestPointer {
    /// Manifest hash committed during ingest.
    pub manifest_hash: BlobDigest,
    /// Storage ticket assigned by `SoraFS`.
    pub storage_ticket: StorageTicketId,
    /// Chunk Merkle root advertised in the manifest.
    pub chunk_root: BlobDigest,
    /// Number of chunks referenced by the manifest.
    pub chunk_count: u32,
    /// Pointer to the CAR archive containing the segment payload.
    pub car: TaikaiCarPointer,
}

impl TaikaiIngestPointer {
    /// Build an ingest pointer from manifest metadata.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        manifest_hash: BlobDigest,
        storage_ticket: StorageTicketId,
        chunk_root: BlobDigest,
        chunk_count: u32,
        car: TaikaiCarPointer,
    ) -> Self {
        Self {
            manifest_hash,
            storage_ticket,
            chunk_root,
            chunk_count,
            car,
        }
    }
}

/// Optional instrumentation hints recorded at ingest time.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiInstrumentation {
    /// Milliseconds between encoder output and DA ingest.
    #[norito(default)]
    pub encoder_to_ingest_latency_ms: Option<u32>,
    /// Observed drift between live edge and ingest arrival (ms, negative = ahead).
    #[norito(default)]
    pub live_edge_drift_ms: Option<i32>,
    /// Optional identifier for the ingest node handling the segment.
    #[norito(default)]
    pub ingest_node_id: Option<String>,
}

/// Versioned Taikai Segment Envelope binding a broadcast segment to its DA artefacts.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiSegmentEnvelopeV1 {
    /// Envelope format version (currently `1`).
    pub version: u16,
    /// Identifier of the Taikai event.
    pub event_id: TaikaiEventId,
    /// Logical stream identifier within the event.
    pub stream_id: TaikaiStreamId,
    /// Rendition identifier (ladder rung).
    pub rendition_id: TaikaiRenditionId,
    /// Metadata describing the encoded track.
    pub track: TaikaiTrackMetadata,
    /// Monotonic segment sequence number (per rendition).
    pub segment_sequence: u64,
    /// Presentation timestamp (start) in microseconds since stream origin.
    pub segment_start_pts: SegmentTimestamp,
    /// Presentation duration in microseconds.
    pub segment_duration: SegmentDuration,
    /// Wall-clock reference in Unix milliseconds when the segment was finalised.
    pub wallclock_unix_ms: u64,
    /// Pointer to the DA manifest and CAR commitments.
    pub ingest: TaikaiIngestPointer,
    /// Optional instrumentation for telemetry dashboards.
    #[norito(default)]
    pub instrumentation: TaikaiInstrumentation,
    /// Arbitrary metadata carried alongside the envelope.
    #[norito(default)]
    pub metadata: ExtraMetadata,
}

/// Time-ordered index key for Taikai envelopes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiTimeIndexKey {
    /// Event identifier that scopes the stream.
    pub event_id: TaikaiEventId,
    /// Stream identifier within the event.
    pub stream_id: TaikaiStreamId,
    /// Rendition identifier (ladder rung).
    pub rendition_id: TaikaiRenditionId,
    /// Segment presentation timestamp used for ordering.
    pub segment_start_pts: SegmentTimestamp,
}

/// CAR lookup index key for Taikai envelopes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiCidIndexKey {
    /// Event identifier that scopes the stream.
    pub event_id: TaikaiEventId,
    /// Stream identifier within the event.
    pub stream_id: TaikaiStreamId,
    /// Rendition identifier (ladder rung).
    pub rendition_id: TaikaiRenditionId,
    /// Multibase-encoded CID pointing to the CAR archive.
    pub cid_multibase: String,
}

/// Bundle containing both deterministic index keys for a Taikai envelope.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiEnvelopeIndexes {
    /// Time-ordered key derived from the envelope.
    pub time_key: TaikaiTimeIndexKey,
    /// CAR lookup key derived from the envelope.
    pub cid_key: TaikaiCidIndexKey,
}

impl TaikaiSegmentEnvelopeV1 {
    /// Current Taikai Segment Envelope version.
    pub const VERSION: u16 = 1;

    /// Construct a new envelope with the provided metadata.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_id: TaikaiEventId,
        stream_id: TaikaiStreamId,
        rendition_id: TaikaiRenditionId,
        track: TaikaiTrackMetadata,
        segment_sequence: u64,
        segment_start_pts: SegmentTimestamp,
        segment_duration: SegmentDuration,
        wallclock_unix_ms: u64,
        ingest: TaikaiIngestPointer,
    ) -> Self {
        Self {
            version: Self::VERSION,
            event_id,
            stream_id,
            rendition_id,
            track,
            segment_sequence,
            segment_start_pts,
            segment_duration,
            wallclock_unix_ms,
            ingest,
            instrumentation: TaikaiInstrumentation::default(),
            metadata: ExtraMetadata::default(),
        }
    }

    /// Derive deterministic index keys for anchoring and lookups.
    #[must_use]
    pub fn indexes(&self) -> TaikaiEnvelopeIndexes {
        TaikaiEnvelopeIndexes {
            time_key: TaikaiTimeIndexKey {
                event_id: self.event_id.clone(),
                stream_id: self.stream_id.clone(),
                rendition_id: self.rendition_id.clone(),
                segment_start_pts: self.segment_start_pts,
            },
            cid_key: TaikaiCidIndexKey {
                event_id: self.event_id.clone(),
                stream_id: self.stream_id.clone(),
                rendition_id: self.rendition_id.clone(),
                cid_multibase: self.ingest.car.cid_multibase.clone(),
            },
        }
    }
}

/// Schema version for [`CekRotationReceiptV1`].
pub const CEK_ROTATION_RECEIPT_VERSION_V1: u16 = 1;

/// Receipt proving that a Content Encryption Key rotation completed for a stream.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct CekRotationReceiptV1 {
    /// Schema version; must equal [`CEK_ROTATION_RECEIPT_VERSION_V1`].
    pub schema_version: u16,
    /// Event associated with the rotation.
    pub event_id: TaikaiEventId,
    /// Stream associated with the rotation.
    pub stream_id: TaikaiStreamId,
    /// Named KMS profile that issued the wrap key.
    pub kms_profile: String,
    /// Label of the new wrap key provided by the KMS.
    pub new_wrap_key_label: String,
    /// Optional label of the previously active wrap key.
    #[norito(default)]
    pub previous_wrap_key_label: Option<String>,
    /// HKDF salt recorded for the rotation in BLAKE3-256 form.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub hkdf_salt: [u8; 32],
    /// Segment number where the new CEK becomes active.
    pub effective_segment_sequence: u64,
    /// Unix timestamp (seconds) when the rotation occurred.
    pub issued_at_unix: u64,
    /// Optional operator/governance notes.
    #[norito(default)]
    pub notes: Option<String>,
}

/// Schema version for [`ReplicationProofTokenV1`].
pub const REPLICATION_PROOF_TOKEN_VERSION_V1: u16 = 1;

/// Norito envelope linking GAR, CEK receipts, and rollout evidence.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ReplicationProofTokenV1 {
    /// Schema version; must equal [`REPLICATION_PROOF_TOKEN_VERSION_V1`].
    pub schema_version: u16,
    /// Event covered by the attestation.
    pub event_id: TaikaiEventId,
    /// Stream covered by the attestation.
    pub stream_id: TaikaiStreamId,
    /// Rendition covered by the attestation.
    pub rendition_id: TaikaiRenditionId,
    /// Digest of the GAR payload recorded for the rollout.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub gar_digest: [u8; 32],
    /// Digest of the CEK rotation receipt referenced by the rollout.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub cek_receipt_digest: [u8; 32],
    /// Digest of the rollout evidence bundle (archives/logs).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub distribution_bundle_digest: [u8; 32],
    /// Canonical telemetry labels enforced during the attested window.
    #[norito(default)]
    pub policy_labels: Vec<String>,
    /// Unix timestamp (seconds) when the attestation becomes valid.
    pub valid_from_unix: u64,
    /// Unix timestamp (seconds) when the attestation expires.
    pub valid_until_unix: u64,
    /// Optional governance notes or ticket references.
    #[norito(default)]
    pub notes: Option<String>,
}

/// Inclusive sequence window used for Taikai routing manifests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiSegmentWindow {
    /// First sequence value covered by the window (inclusive).
    pub start_sequence: u64,
    /// Last sequence value covered by the window (inclusive).
    pub end_sequence: u64,
}

impl TaikaiSegmentWindow {
    /// Construct a new inclusive window.
    #[must_use]
    pub const fn new(start_sequence: u64, end_sequence: u64) -> Self {
        Self {
            start_sequence,
            end_sequence,
        }
    }

    /// Returns `true` when the supplied sequence lies within the window.
    #[must_use]
    pub const fn contains(&self, sequence: u64) -> bool {
        sequence >= self.start_sequence && sequence <= self.end_sequence
    }

    /// Validate the inclusive range encoded by this window.
    ///
    /// # Errors
    /// Returns [`TaikaiSegmentWindowError::StartExceedsEnd`] when `start_sequence` exceeds
    /// `end_sequence`.
    pub fn validate(&self) -> Result<(), TaikaiSegmentWindowError> {
        if self.start_sequence > self.end_sequence {
            return Err(TaikaiSegmentWindowError::StartExceedsEnd {
                start_sequence: self.start_sequence,
                end_sequence: self.end_sequence,
            });
        }
        Ok(())
    }
}

impl fmt::Display for TaikaiSegmentWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..={}", self.start_sequence, self.end_sequence)
    }
}

/// Errors that can occur while validating a [`TaikaiSegmentWindow`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TaikaiSegmentWindowError {
    /// The start of the window is greater than the end.
    #[error("segment window start {start_sequence} exceeds end {end_sequence}")]
    StartExceedsEnd {
        /// Inclusive lower bound of the invalid window.
        start_sequence: u64,
        /// Inclusive upper bound of the invalid window.
        end_sequence: u64,
    },
}

/// Identifier referencing a `SoraNet` guard directory circuit.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema, Hash, Default,
)]
#[repr(transparent)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct GuardDirectoryId(pub String);

impl GuardDirectoryId {
    /// Construct a new guard directory identifier.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

/// Guard policy describing the `SoraNet` circuit and quorum requirements.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiGuardPolicy {
    /// Canonical guard directory identifier.
    pub directory: GuardDirectoryId,
    /// Version/tag of the guard policy agreed with governance.
    pub version: u32,
    /// Minimum number of relays/guards required for this stream.
    pub min_guard_relays: u16,
    /// Optional lane or quorum labels enforced at runtime.
    #[cfg_attr(feature = "json", norito(default))]
    pub lane_labels: Vec<String>,
}

impl TaikaiGuardPolicy {
    /// Build a guard policy descriptor.
    #[must_use]
    pub fn new(
        directory: GuardDirectoryId,
        version: u32,
        min_guard_relays: u16,
        lane_labels: Vec<String>,
    ) -> Self {
        Self {
            directory,
            version,
            min_guard_relays,
            lane_labels,
        }
    }
}

/// Availability class used for Taikai rendition routing (mirrors `SoraFS` storage tiers).
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Hash, Default, PartialOrd, Ord,
)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "class", content = "value")]
pub enum TaikaiAvailabilityClass {
    /// Hot storage tier.
    #[default]
    Hot,
    /// Warm storage tier.
    Warm,
    /// Cold storage tier.
    Cold,
}

impl From<TaikaiAvailabilityClass> for StorageClass {
    fn from(value: TaikaiAvailabilityClass) -> Self {
        match value {
            TaikaiAvailabilityClass::Hot => StorageClass::Hot,
            TaikaiAvailabilityClass::Warm => StorageClass::Warm,
            TaikaiAvailabilityClass::Cold => StorageClass::Cold,
        }
    }
}

impl From<StorageClass> for TaikaiAvailabilityClass {
    fn from(value: StorageClass) -> Self {
        match value {
            StorageClass::Hot => Self::Hot,
            StorageClass::Warm => Self::Warm,
            StorageClass::Cold => Self::Cold,
        }
    }
}

/// Per-rendition routing record describing replication, CAR commitments, and guard data.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiRenditionRouteV1 {
    /// Rendition identifier (`1080p`, `sign-language`, etc.).
    pub rendition_id: TaikaiRenditionId,
    /// Latest manifest hash advertised by the ingest pipeline.
    pub latest_manifest_hash: BlobDigest,
    /// Latest CAR descriptor holding the payload window.
    pub latest_car: TaikaiCarPointer,
    /// Availability class applied to this rendition window.
    pub availability_class: TaikaiAvailabilityClass,
    /// Ordered list of provider identifiers replicating the CAR window.
    #[cfg_attr(feature = "json", norito(default))]
    pub replication_targets: Vec<ProviderId>,
    /// Guard circuit powering this rendition.
    pub soranet_circuit: GuardDirectoryId,
    /// Sequence range covered by the current signing manifest(s).
    pub ssm_range: TaikaiSegmentWindow,
}

impl TaikaiRenditionRouteV1 {
    /// Returns true when the supplied sequence belongs to this route window.
    #[must_use]
    pub const fn covers_sequence(&self, sequence: u64) -> bool {
        self.ssm_range.contains(sequence)
    }
}

/// Routing manifest tying renditions, guard policy, and alias bindings together.
///
/// Torii persists each manifest alongside the Taikai envelope payloads
/// (`taikai-trm-*.norito` artefacts and the `taikai-trm-state-*` lineage ledgers)
/// so `SoraNS` anchors can ship routing windows with every batch.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiRoutingManifestV1 {
    /// Manifest format version (`1`).
    pub version: u16,
    /// Event identifier covered by this manifest.
    pub event_id: TaikaiEventId,
    /// Stream identifier covered by this manifest.
    pub stream_id: TaikaiStreamId,
    /// Inclusive sequence window for the coverage.
    pub segment_window: TaikaiSegmentWindow,
    /// Routing entries per rendition.
    #[cfg_attr(feature = "json", norito(default))]
    pub renditions: Vec<TaikaiRenditionRouteV1>,
    /// Alias binding anchored in `SoraNS` for this stream.
    pub alias_binding: TaikaiAliasBinding,
    /// Guard policy requirements for the manifest window.
    pub guard_policy: TaikaiGuardPolicy,
    /// Optional metadata (policy directives, rollout annotations).
    pub metadata: ExtraMetadata,
}

impl TaikaiRoutingManifestV1 {
    /// Current routing manifest format version.
    pub const VERSION: u16 = 1;

    /// Check whether the manifest window covers a particular sequence.
    #[must_use]
    pub const fn covers_sequence(&self, sequence: u64) -> bool {
        self.segment_window.contains(sequence)
    }

    /// Validate manifest invariants:
    /// - windows are ordered
    /// - rendition ranges fall within the manifest window
    /// - rendition identifiers are unique
    ///
    /// # Errors
    /// Returns [`TaikaiRoutingManifestValidationError::SegmentWindow`] when the manifest
    /// window is invalid, [`TaikaiRoutingManifestValidationError::RenditionWindowOutOfRange`]
    /// when a rendition window extends outside that range, or
    /// [`TaikaiRoutingManifestValidationError::DuplicateRendition`] when identifiers repeat.
    pub fn validate(&self) -> Result<(), TaikaiRoutingManifestValidationError> {
        self.segment_window.validate()?;
        let mut seen = BTreeSet::new();
        for route in &self.renditions {
            route.ssm_range.validate()?;
            if route.ssm_range.start_sequence < self.segment_window.start_sequence
                || route.ssm_range.end_sequence > self.segment_window.end_sequence
            {
                return Err(
                    TaikaiRoutingManifestValidationError::RenditionWindowOutOfRange {
                        rendition_id: route.rendition_id.clone(),
                        route_window: route.ssm_range,
                        manifest_window: self.segment_window,
                    },
                );
            }
            if !seen.insert(route.rendition_id.clone()) {
                return Err(TaikaiRoutingManifestValidationError::DuplicateRendition {
                    rendition_id: route.rendition_id.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Errors emitted when validating a [`TaikaiRoutingManifestV1`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TaikaiRoutingManifestValidationError {
    /// Wrapper for underlying window errors.
    #[error(transparent)]
    SegmentWindow(#[from] TaikaiSegmentWindowError),
    /// A rendition window extends outside of the manifest range.
    #[error(
        "rendition `{rendition_id}` window {route_window} lies outside manifest range {manifest_window}"
    )]
    RenditionWindowOutOfRange {
        /// Rendition identifier associated with the invalid window.
        rendition_id: TaikaiRenditionId,
        /// The offending rendition window.
        route_window: TaikaiSegmentWindow,
        /// Manifest coverage that the rendition was expected to respect.
        manifest_window: TaikaiSegmentWindow,
    },
    /// Duplicate rendition identifiers were supplied.
    #[error("duplicate rendition `{rendition_id}` detected in manifest")]
    DuplicateRendition {
        /// Identifier that appeared multiple times in the manifest.
        rendition_id: TaikaiRenditionId,
    },
}

/// Payload describing the data signed by publishers for every Taikai segment.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiSegmentSigningBodyV1 {
    /// Payload format version (`1`).
    pub version: u16,
    /// Hash of the Norito-encoded segment envelope.
    pub segment_envelope_hash: BlobDigest,
    /// Manifest hash referenced by the envelope.
    pub manifest_hash: BlobDigest,
    /// Digest of the CAR containing this segment.
    pub car_digest: BlobDigest,
    /// Sequence index of the signed segment.
    pub segment_sequence: u64,
    /// Account responsible for signing this rendition window.
    pub publisher_account: AccountId,
    /// Public key used for the signature.
    pub publisher_key: PublicKey,
    /// Millisecond timestamp when the signature was produced.
    pub signed_unix_ms: u64,
    /// Alias binding proof stapled alongside the manifest.
    pub alias_binding: TaikaiAliasBinding,
    /// Optional policy metadata.
    pub metadata: ExtraMetadata,
}

impl TaikaiSegmentSigningBodyV1 {
    /// Construct a signing body descriptor.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        version: u16,
        segment_envelope_hash: BlobDigest,
        manifest_hash: BlobDigest,
        car_digest: BlobDigest,
        segment_sequence: u64,
        publisher_account: AccountId,
        publisher_key: PublicKey,
        signed_unix_ms: u64,
        alias_binding: TaikaiAliasBinding,
        metadata: ExtraMetadata,
    ) -> Self {
        Self {
            version,
            segment_envelope_hash,
            manifest_hash,
            car_digest,
            segment_sequence,
            publisher_account,
            publisher_key,
            signed_unix_ms,
            alias_binding,
            metadata,
        }
    }
}

/// Signed manifest tying a Taikai segment envelope to its publisher attestation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiSegmentSigningManifestV1 {
    /// Signed body payload.
    pub body: TaikaiSegmentSigningBodyV1,
    /// Signature authenticating the payload.
    pub signature: SignatureOf<TaikaiSegmentSigningBodyV1>,
}

impl TaikaiSegmentSigningManifestV1 {
    /// Construct a signed manifest from the provided payload and signature.
    #[must_use]
    pub fn new(
        body: TaikaiSegmentSigningBodyV1,
        signature: SignatureOf<TaikaiSegmentSigningBodyV1>,
    ) -> Self {
        Self { body, signature }
    }

    /// Borrow the signer account.
    #[must_use]
    pub fn signer(&self) -> &AccountId {
        &self.body.publisher_account
    }
}

/// `QoS` token bucket configuration captured by Taikai cache profiles.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiCacheQosConfigV1 {
    /// Priority stream budget in bits per second.
    pub priority_rate_bps: u64,
    /// Standard stream budget in bits per second.
    pub standard_rate_bps: u64,
    /// Bulk transfer budget in bits per second.
    pub bulk_rate_bps: u64,
    /// Burst multiplier applied to every bucket.
    pub burst_multiplier: u32,
}

impl TaikaiCacheQosConfigV1 {
    fn validate(&self) -> Result<(), TaikaiCacheProfileError> {
        TaikaiCacheProfileError::ensure_positive(self.priority_rate_bps, "qos.priority_rate_bps")?;
        TaikaiCacheProfileError::ensure_positive(self.standard_rate_bps, "qos.standard_rate_bps")?;
        TaikaiCacheProfileError::ensure_positive(self.bulk_rate_bps, "qos.bulk_rate_bps")?;
        if self.burst_multiplier == 0 {
            return Err(TaikaiCacheProfileError::NonPositiveField {
                field: "qos.burst_multiplier",
            });
        }
        Ok(())
    }
}

/// Cache capacity/retention knobs used by SNNet-14.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiCacheConfigV1 {
    /// Hot tier capacity in bytes.
    pub hot_capacity_bytes: u64,
    /// Hot tier retention window in seconds.
    pub hot_retention_secs: u64,
    /// Warm tier capacity in bytes.
    pub warm_capacity_bytes: u64,
    /// Warm tier retention window in seconds.
    pub warm_retention_secs: u64,
    /// Cold tier capacity in bytes.
    pub cold_capacity_bytes: u64,
    /// Cold tier retention window in seconds.
    pub cold_retention_secs: u64,
    /// `QoS` rate/burst configuration.
    pub qos: TaikaiCacheQosConfigV1,
}

impl TaikaiCacheConfigV1 {
    /// Validate the cache configuration to ensure positive capacities and durations.
    ///
    /// # Errors
    /// Returns [`TaikaiCacheProfileError`] when any capacity, retention, or `QoS` knob is zero.
    pub fn validate(&self) -> Result<(), TaikaiCacheProfileError> {
        TaikaiCacheProfileError::ensure_positive(self.hot_capacity_bytes, "hot_capacity_bytes")?;
        TaikaiCacheProfileError::ensure_positive(self.hot_retention_secs, "hot_retention_secs")?;
        TaikaiCacheProfileError::ensure_positive(self.warm_capacity_bytes, "warm_capacity_bytes")?;
        TaikaiCacheProfileError::ensure_positive(self.warm_retention_secs, "warm_retention_secs")?;
        TaikaiCacheProfileError::ensure_positive(self.cold_capacity_bytes, "cold_capacity_bytes")?;
        TaikaiCacheProfileError::ensure_positive(self.cold_retention_secs, "cold_retention_secs")?;
        self.qos.validate()?;
        Ok(())
    }
}

/// Rollout stage describing where a Taikai cache profile is permitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Display)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "stage", content = "value", rename_all = "kebab-case")]
pub enum TaikaiCacheRolloutStage {
    /// Limited canary deployments.
    #[display("canary")]
    Canary,
    /// Region or provider ramp phase.
    #[display("ramp")]
    Ramp,
    /// Default profile for production traffic.
    #[display("default")]
    Default,
    /// Emergency override or rollback profile.
    #[display("emergency")]
    Emergency,
}

impl TaikaiCacheRolloutStage {
    /// Resolve the canonical string label for this rollout stage.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Canary => "canary",
            Self::Ramp => "ramp",
            Self::Default => "default",
            Self::Emergency => "emergency",
        }
    }
}

impl FromStr for TaikaiCacheRolloutStage {
    type Err = TaikaiCacheRolloutStageParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalised = value.trim().to_ascii_lowercase();
        match normalised.as_str() {
            "canary" => Ok(Self::Canary),
            "ramp" => Ok(Self::Ramp),
            "default" => Ok(Self::Default),
            "emergency" => Ok(Self::Emergency),
            _ => Err(TaikaiCacheRolloutStageParseError {
                value: value.trim().to_string(),
            }),
        }
    }
}

/// Error surfaced when parsing a rollout stage label fails.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub struct TaikaiCacheRolloutStageParseError {
    value: String,
}

impl fmt::Display for TaikaiCacheRolloutStageParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown Taikai cache rollout stage `{}`", self.value)
    }
}

/// Governance-approved cache profile stored in the `SoraFS` governance DAG.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct TaikaiCacheProfileV1 {
    /// Unique profile identifier (referenced by rollout manifests).
    pub profile_id: String,
    /// Rollout stage the profile targets.
    pub rollout_stage: TaikaiCacheRolloutStage,
    /// Human-friendly description used in runbooks/dashboards.
    pub description: String,
    /// Free-form annotations mirrored into governance evidence bundles.
    #[cfg_attr(feature = "json", norito(default))]
    pub annotations: BTreeMap<String, String>,
    /// Cache configuration applied to orchestrators and relays.
    #[cfg_attr(feature = "json", norito(rename = "taikai_cache"))]
    pub config: TaikaiCacheConfigV1,
}

impl TaikaiCacheProfileV1 {
    /// Validate the profile metadata and cache configuration.
    ///
    /// # Errors
    /// Returns [`TaikaiCacheProfileError`] when IDs, descriptions, or cache settings are invalid.
    pub fn validate(&self) -> Result<(), TaikaiCacheProfileError> {
        if self.profile_id.trim().is_empty() {
            return Err(TaikaiCacheProfileError::EmptyProfileId);
        }
        if self.description.trim().is_empty() {
            return Err(TaikaiCacheProfileError::EmptyDescription);
        }
        self.config.validate()
    }
}

/// Validation errors surfaced when processing Taikai cache profiles.
#[derive(Copy, Clone, Debug, Error, PartialEq, Eq)]
pub enum TaikaiCacheProfileError {
    /// Profile identifier must not be empty.
    #[error("Taikai cache profile id must not be empty")]
    EmptyProfileId,
    /// Profile description must not be empty.
    #[error("Taikai cache profile description must not be empty")]
    EmptyDescription,
    /// Numeric field must be greater than zero.
    #[error("Taikai cache profile field `{field}` must be greater than zero")]
    NonPositiveField {
        /// Fully-qualified name of the field that failed validation.
        field: &'static str,
    },
}

impl TaikaiCacheProfileError {
    fn ensure_positive(value: u64, field: &'static str) -> Result<(), Self> {
        if value == 0 {
            Err(Self::NonPositiveField { field })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, str::FromStr};

    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::{
        da::types::{
            BlobDigest, ExtraMetadata, MetadataEntry, MetadataVisibility, StorageTicketId,
        },
        domain::DomainId,
    };

    fn digest_from(value: u8) -> BlobDigest {
        let mut bytes = [0u8; 32];
        bytes.fill(value);
        BlobDigest::new(bytes)
    }

    #[test]
    fn event_id_wrapper_round_trip() {
        let name = Name::from_str("launch-stream").expect("valid name");
        let event_id = TaikaiEventId::new(name.clone());
        assert_eq!(event_id.as_name(), &name);
    }

    #[test]
    fn segment_timestamp_helpers() {
        let timestamp = SegmentTimestamp::new(90_000);
        assert_eq!(timestamp.as_micros(), 90_000);
    }

    #[test]
    fn track_metadata_builders_cover_variants() {
        let video_meta = TaikaiTrackMetadata::video(
            TaikaiCodec::HevcMain10,
            5_000,
            TaikaiResolution::new(1920, 1080),
        );
        assert_eq!(video_meta.kind, TaikaiTrackKind::Video);
        assert_eq!(video_meta.resolution.unwrap().width, 1920);

        let audio_meta =
            TaikaiTrackMetadata::audio(TaikaiCodec::AacLc, 192, TaikaiAudioLayout::Stereo);
        assert_eq!(audio_meta.kind, TaikaiTrackKind::Audio);
        assert_eq!(audio_meta.audio_layout.unwrap(), TaikaiAudioLayout::Stereo);

        let data_meta = TaikaiTrackMetadata::data(TaikaiCodec::Custom("id3".into()), 32);
        assert_eq!(data_meta.kind, TaikaiTrackKind::Data);
        assert!(data_meta.resolution.is_none());
    }

    #[test]
    fn envelope_encodes_and_decodes() {
        use std::str::FromStr;

        let event_id = TaikaiEventId::new(Name::from_str("global-keynote").unwrap());
        let stream_id = TaikaiStreamId::new(Name::from_str("stage-a").unwrap());
        let rendition_id = TaikaiRenditionId::new(Name::from_str("1080p").unwrap());
        let track = TaikaiTrackMetadata::video(
            TaikaiCodec::Av1Main,
            8_000,
            TaikaiResolution::new(1920, 1080),
        );
        let manifest_hash = digest_from(0xAA);
        let chunk_root = digest_from(0xBB);
        let storage_ticket = StorageTicketId::new([0xCC; 32]);
        let car = TaikaiCarPointer::new("bagcq", digest_from(0xDD), 65_536);
        let ingest = TaikaiIngestPointer::new(manifest_hash, storage_ticket, chunk_root, 24, car);
        let mut metadata = ExtraMetadata::default();
        metadata.items.push(MetadataEntry::new(
            "language",
            b"en-US".to_vec(),
            MetadataVisibility::Public,
        ));
        let mut envelope = TaikaiSegmentEnvelopeV1::new(
            event_id,
            stream_id,
            rendition_id,
            track,
            42,
            SegmentTimestamp::new(3_600_000),
            SegmentDuration::new(2_000_000),
            1_702_560_000_000,
            ingest,
        );
        envelope.metadata = metadata.clone();
        envelope.instrumentation.encoder_to_ingest_latency_ms = Some(120);

        let encoded = envelope.encode();
        let mut cursor = std::io::Cursor::new(encoded);
        let decoded: TaikaiSegmentEnvelopeV1 =
            Decode::decode(&mut cursor).expect("decode succeeds");
        assert_eq!(decoded.segment_sequence, 42);
        assert_eq!(decoded.metadata, metadata);
        assert_eq!(
            decoded.instrumentation.encoder_to_ingest_latency_ms,
            Some(120)
        );
    }

    #[test]
    fn envelope_indexes_reflect_fields() {
        let event_id = TaikaiEventId::new(Name::from_str("global-keynote").unwrap());
        let stream_id = TaikaiStreamId::new(Name::from_str("stage-a").unwrap());
        let rendition_id = TaikaiRenditionId::new(Name::from_str("1080p").unwrap());
        let track = TaikaiTrackMetadata::video(
            TaikaiCodec::Av1Main,
            8_000,
            TaikaiResolution::new(1920, 1080),
        );
        let car = TaikaiCarPointer::new("zbafyqra", digest_from(0xEA), 65_536);
        let ingest = TaikaiIngestPointer::new(
            digest_from(0xAA),
            StorageTicketId::new([0xBB; 32]),
            digest_from(0xCC),
            24,
            car,
        );
        let envelope = TaikaiSegmentEnvelopeV1::new(
            event_id.clone(),
            stream_id.clone(),
            rendition_id.clone(),
            track,
            42,
            SegmentTimestamp::new(3_600_000),
            SegmentDuration::new(2_000_000),
            1_702_560_000_000,
            ingest,
        );

        let indexes = envelope.indexes();
        assert_eq!(indexes.time_key.event_id, event_id);
        assert_eq!(indexes.time_key.stream_id, stream_id);
        assert_eq!(indexes.time_key.segment_start_pts.as_micros(), 3_600_000);
        assert_eq!(indexes.cid_key.rendition_id, rendition_id);
        assert_eq!(indexes.cid_key.cid_multibase, "zbafyqra");
    }

    #[test]
    fn parsing_helpers_cover_supported_values() {
        use std::str::FromStr;

        assert_eq!(
            TaikaiTrackKind::from_str("Video").expect("track kind"),
            TaikaiTrackKind::Video
        );
        assert_eq!(
            TaikaiCodec::from_str("custom:vp9").expect("codec"),
            TaikaiCodec::Custom("vp9".to_string())
        );
        let resolution = TaikaiResolution::from_str("1920x1080").expect("resolution");
        assert_eq!(resolution.width, 1920);
        assert_eq!(resolution.height, 1080);
        assert_eq!(
            TaikaiAudioLayout::from_str("custom:4").expect("layout"),
            TaikaiAudioLayout::Custom(4)
        );
        assert!(TaikaiAudioLayout::from_str("unknown").is_err());
        assert!(TaikaiCodec::from_str("custom:").is_err());
        assert!(TaikaiResolution::from_str("1920").is_err());
    }

    fn sample_alias_binding() -> TaikaiAliasBinding {
        TaikaiAliasBinding {
            name: "docs".into(),
            namespace: "sora".into(),
            proof: vec![0xAA, 0xBB, 0xCC],
        }
    }

    fn sample_guard_policy() -> TaikaiGuardPolicy {
        TaikaiGuardPolicy::new(
            GuardDirectoryId::new("soranet/demo"),
            2,
            3,
            vec!["lane-a".into(), "lane-b".into()],
        )
    }

    fn sample_routing_manifest() -> TaikaiRoutingManifestV1 {
        let event_id = TaikaiEventId::new(Name::from_str("global-keynote").unwrap());
        let stream_id = TaikaiStreamId::new(Name::from_str("stage-a").unwrap());
        let rendition = TaikaiRenditionId::new(Name::from_str("1080p").unwrap());
        let route = TaikaiRenditionRouteV1 {
            rendition_id: rendition.clone(),
            latest_manifest_hash: digest_from(0x42),
            latest_car: TaikaiCarPointer::new("zbafyqra", digest_from(0x24), 131_072),
            availability_class: TaikaiAvailabilityClass::Hot,
            replication_targets: vec![ProviderId::new([0x11; 32])],
            soranet_circuit: GuardDirectoryId::new("soranet/demo"),
            ssm_range: TaikaiSegmentWindow::new(40, 50),
        };
        TaikaiRoutingManifestV1 {
            version: TaikaiRoutingManifestV1::VERSION,
            event_id,
            stream_id,
            segment_window: TaikaiSegmentWindow::new(40, 64),
            renditions: vec![route],
            alias_binding: sample_alias_binding(),
            guard_policy: sample_guard_policy(),
            metadata: ExtraMetadata::default(),
        }
    }

    fn sample_cek_receipt() -> CekRotationReceiptV1 {
        CekRotationReceiptV1 {
            schema_version: CEK_ROTATION_RECEIPT_VERSION_V1,
            event_id: TaikaiEventId::new(Name::from_str("global-keynote").unwrap()),
            stream_id: TaikaiStreamId::new(Name::from_str("stage-a").unwrap()),
            kms_profile: "kms/default".into(),
            new_wrap_key_label: "wrap-2026q1".into(),
            previous_wrap_key_label: Some("wrap-2025q4".into()),
            hkdf_salt: [0xAB; 32],
            effective_segment_sequence: 64,
            issued_at_unix: 1_734_000_123,
            notes: Some("ticket SN13-E".into()),
        }
    }

    fn sample_replication_proof_token() -> ReplicationProofTokenV1 {
        ReplicationProofTokenV1 {
            schema_version: REPLICATION_PROOF_TOKEN_VERSION_V1,
            event_id: TaikaiEventId::new(Name::from_str("global-keynote").unwrap()),
            stream_id: TaikaiStreamId::new(Name::from_str("stage-a").unwrap()),
            rendition_id: TaikaiRenditionId::new(Name::from_str("1080p").unwrap()),
            gar_digest: [0x11; 32],
            cek_receipt_digest: [0x22; 32],
            distribution_bundle_digest: [0x33; 32],
            policy_labels: vec!["canary".into(), "sn13-evidence".into()],
            valid_from_unix: 1_734_000_000,
            valid_until_unix: 1_734_086_400,
            notes: Some("GAR v2 rollout".into()),
        }
    }

    #[test]
    fn routing_manifest_round_trips() {
        let manifest = sample_routing_manifest();
        let encoded = manifest.encode();
        let mut cursor = std::io::Cursor::new(encoded);
        let decoded: TaikaiRoutingManifestV1 =
            Decode::decode(&mut cursor).expect("routing manifest decodes");
        assert_eq!(decoded.event_id, manifest.event_id);
        assert_eq!(decoded.stream_id, manifest.stream_id);
        assert!(decoded.covers_sequence(42));
        assert_eq!(decoded.renditions.len(), 1);
        assert!(decoded.renditions[0].covers_sequence(45));
        assert_eq!(
            decoded.renditions[0].rendition_id,
            manifest.renditions[0].rendition_id
        );
        assert_eq!(
            decoded.renditions[0].availability_class,
            TaikaiAvailabilityClass::Hot
        );
        decoded.validate().expect("manifest validates");
    }

    #[test]
    fn routing_manifest_covers_sequence_respects_bounds() {
        let manifest = sample_routing_manifest();
        let start = manifest.segment_window.start_sequence;
        let end = manifest.segment_window.end_sequence;
        assert!(manifest.covers_sequence(start));
        assert!(manifest.covers_sequence(end));
        assert!(!manifest.covers_sequence(end + 1));
        if start > 0 {
            assert!(!manifest.covers_sequence(start - 1));
        }

        let route = &manifest.renditions[0];
        let route_start = route.ssm_range.start_sequence;
        let route_end = route.ssm_range.end_sequence;
        assert!(route.covers_sequence(route_start));
        assert!(route.covers_sequence(route_end));
        assert!(!route.covers_sequence(route_end + 1));
        if route_start > 0 {
            assert!(!route.covers_sequence(route_start - 1));
        }
    }

    #[test]
    fn segment_signing_manifest_round_trips() {
        let kp = KeyPair::from_seed(vec![0x41; 32], Algorithm::Ed25519);
        let domain: DomainId = "wonderland".parse().unwrap();
        let publisher_account = AccountId::new(domain.clone(), kp.public_key().clone());
        let alias_binding = sample_alias_binding();
        let body = TaikaiSegmentSigningBodyV1::new(
            1,
            digest_from(0x10),
            digest_from(0x11),
            digest_from(0x12),
            48,
            publisher_account.clone(),
            kp.public_key().clone(),
            1_702_560_123_000,
            alias_binding.clone(),
            ExtraMetadata::default(),
        );
        let signature = SignatureOf::new(kp.private_key(), &body);
        let manifest = TaikaiSegmentSigningManifestV1::new(body.clone(), signature.clone());
        let encoded = manifest.encode();
        let mut cursor = std::io::Cursor::new(encoded);
        let decoded: TaikaiSegmentSigningManifestV1 =
            Decode::decode(&mut cursor).expect("signing manifest decodes");
        assert_eq!(decoded.signer(), &publisher_account);
        assert_eq!(decoded.body, body);
        assert_eq!(decoded.signature, signature);
    }

    fn sample_cache_profile() -> TaikaiCacheProfileV1 {
        TaikaiCacheProfileV1 {
            profile_id: "balanced-canary".to_string(),
            rollout_stage: TaikaiCacheRolloutStage::Canary,
            description: "Balanced cache profile for SNNet-14 pilots".to_string(),
            annotations: BTreeMap::from([
                ("ticket".to_string(), "SNNet-14D".to_string()),
                ("owner".to_string(), "Ops Guild".to_string()),
            ]),
            config: TaikaiCacheConfigV1 {
                hot_capacity_bytes: 8 * 1024 * 1024,
                hot_retention_secs: 45,
                warm_capacity_bytes: 32 * 1024 * 1024,
                warm_retention_secs: 180,
                cold_capacity_bytes: 256 * 1024 * 1024,
                cold_retention_secs: 3_600,
                qos: TaikaiCacheQosConfigV1 {
                    priority_rate_bps: 80 * 1024 * 1024,
                    standard_rate_bps: 40 * 1024 * 1024,
                    bulk_rate_bps: 12 * 1024 * 1024,
                    burst_multiplier: 3,
                },
            },
        }
    }

    #[test]
    fn taikai_cache_profile_round_trips() {
        let profile = sample_cache_profile();
        let encoded = profile.encode();
        let mut cursor = std::io::Cursor::new(encoded);
        let decoded = TaikaiCacheProfileV1::decode(&mut cursor).expect("profile decodes");
        assert_eq!(decoded, profile);
        decoded.validate().expect("profile validates");
    }

    #[test]
    fn taikai_cache_profile_validation_rejects_zero_values() {
        let mut profile = sample_cache_profile();
        profile.config.hot_capacity_bytes = 0;
        let err = profile.validate().expect_err("validation must fail");
        assert!(matches!(
            err,
            TaikaiCacheProfileError::NonPositiveField {
                field: "hot_capacity_bytes"
            }
        ));
    }

    #[test]
    fn taikai_cache_rollout_stage_parses_labels() {
        let stage = TaikaiCacheRolloutStage::from_str("RAMP").expect("stage parsed");
        assert_eq!(stage, TaikaiCacheRolloutStage::Ramp);
        let err = TaikaiCacheRolloutStage::from_str("unknown").expect_err("unknown stage");
        assert_eq!(
            err.to_string(),
            "unknown Taikai cache rollout stage `unknown`"
        );
    }

    #[test]
    fn cek_rotation_receipt_round_trips() {
        let receipt = sample_cek_receipt();
        assert_eq!(receipt.schema_version, CEK_ROTATION_RECEIPT_VERSION_V1);

        let encoded = norito::to_bytes(&receipt).expect("encode receipt");
        let decoded =
            norito::decode_from_bytes::<CekRotationReceiptV1>(&encoded).expect("receipt decodes");
        assert_eq!(decoded, receipt);
    }

    #[test]
    fn replication_proof_token_round_trips() {
        let token = sample_replication_proof_token();
        assert_eq!(token.schema_version, REPLICATION_PROOF_TOKEN_VERSION_V1);

        let encoded = norito::to_bytes(&token).expect("encode token");
        let decoded =
            norito::decode_from_bytes::<ReplicationProofTokenV1>(&encoded).expect("token decodes");
        assert_eq!(decoded, token);
    }

    #[test]
    fn segment_window_validation_rejects_invalid_range() {
        let window = TaikaiSegmentWindow::new(10, 5);
        let err = window.validate().expect_err("validation must fail");
        assert!(matches!(
            err,
            TaikaiSegmentWindowError::StartExceedsEnd {
                start_sequence: 10,
                end_sequence: 5
            }
        ));
    }

    #[test]
    fn routing_manifest_validation_rejects_out_of_range_rendition() {
        let mut manifest = sample_routing_manifest();
        manifest.segment_window = TaikaiSegmentWindow::new(10, 20);
        manifest.renditions[0].ssm_range = TaikaiSegmentWindow::new(5, 15);
        let err = manifest.validate().expect_err("validation must fail");
        assert!(matches!(
            err,
            TaikaiRoutingManifestValidationError::RenditionWindowOutOfRange { .. }
        ));
    }

    #[test]
    fn routing_manifest_validation_rejects_duplicate_renditions() {
        let mut manifest = sample_routing_manifest();
        let duplicate = manifest.renditions[0].clone();
        manifest.renditions.push(duplicate);
        let err = manifest.validate().expect_err("validation must fail");
        assert!(matches!(
            err,
            TaikaiRoutingManifestValidationError::DuplicateRendition { .. }
        ));
    }

    #[test]
    fn routing_manifest_validation_rejects_invalid_segment_window() {
        let mut manifest = sample_routing_manifest();
        manifest.segment_window = TaikaiSegmentWindow::new(100, 50);
        let err = manifest.validate().expect_err("validation must fail");
        assert!(matches!(
            err,
            TaikaiRoutingManifestValidationError::SegmentWindow(
                TaikaiSegmentWindowError::StartExceedsEnd { .. }
            )
        ));
    }
}
