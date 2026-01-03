use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    da::types::{
        BlobClass, BlobCodec, BlobDigest, ChunkDigest, DaRentQuote, ErasureProfile, ExtraMetadata,
        RetentionPolicy, StorageTicketId,
    },
    nexus::LaneId,
};

/// Role for a chunk within an erasure-coded stripe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "role", content = "value"))]
pub enum ChunkRole {
    /// Data chunk.
    #[default]
    Data,
    /// Local parity within the row/stripe.
    LocalParity,
    /// Global parity within the row/stripe.
    GlobalParity,
    /// Parity across rows/stripes (2D column parity).
    StripeParity,
}

/// Chunk commitment record produced during manifest generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct ChunkCommitment {
    /// Zero-based chunk index.
    pub index: u32,
    /// Byte offset of the chunk within the original payload.
    pub offset: u64,
    /// Chunk length in bytes (last chunk may be shorter than the nominal chunk size).
    pub length: u32,
    /// Blake3 commitment of the chunk contents.
    pub commitment: ChunkDigest,
    /// Whether this chunk is a parity shard (true) or data shard (false).
    pub parity: bool,
    /// Role of the chunk within the erasure layout.
    pub role: ChunkRole,
    /// Zero-based group identifier for the chunk (stripe index for row/column parity).
    pub group_id: u32,
}

impl ChunkCommitment {
    /// Convenience constructor.
    #[must_use]
    pub fn new(
        index: u32,
        offset: u64,
        length: u32,
        commitment: ChunkDigest,
        parity: bool,
    ) -> Self {
        Self::new_with_role(
            index,
            offset,
            length,
            commitment,
            if parity {
                ChunkRole::GlobalParity
            } else {
                ChunkRole::Data
            },
            0,
        )
    }

    /// Constructor that accepts an explicit role and group identifier.
    #[must_use]
    pub fn new_with_role(
        index: u32,
        offset: u64,
        length: u32,
        commitment: ChunkDigest,
        role: ChunkRole,
        group_id: u32,
    ) -> Self {
        Self {
            index,
            offset,
            length,
            commitment,
            parity: matches!(
                role,
                ChunkRole::GlobalParity | ChunkRole::LocalParity | ChunkRole::StripeParity
            ),
            role,
            group_id,
        }
    }
}

/// Canonical Norito manifest emitted after chunking a DA blob.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
pub struct DaManifestV1 {
    /// Manifest format version. Currently always 1.
    pub version: u16,
    /// Caller-supplied blob identifier.
    pub client_blob_id: BlobDigest,
    /// Nexus lane the blob belongs to.
    pub lane_id: LaneId,
    /// Epoch assigned to the blob.
    pub epoch: u64,
    /// Semantic classification of the blob.
    pub blob_class: BlobClass,
    /// Codec label describing the payload.
    pub codec: BlobCodec,
    /// Hash of the original payload.
    pub blob_hash: BlobDigest,
    /// Merkle root of the data chunk commitments.
    pub chunk_root: BlobDigest,
    /// Storage ticket issued for this blob.
    pub storage_ticket: StorageTicketId,
    /// Total payload size in bytes.
    pub total_size: u64,
    /// Chunk size in bytes used during encoding.
    pub chunk_size: u32,
    /// Total stripes in the 2D matrix (data stripes + column parity stripes).
    #[norito(default)]
    pub total_stripes: u32,
    /// Total shards per stripe (data + row parity).
    #[norito(default)]
    pub shards_per_stripe: u32,
    /// Erasure coding profile applied to the chunks.
    pub erasure_profile: ErasureProfile,
    /// Retention policy negotiated for the blob.
    pub retention_policy: RetentionPolicy,
    /// Rent and incentive breakdown derived from the configured policy.
    #[norito(default)]
    pub rent_quote: DaRentQuote,
    /// Chunk commitments ordered by chunk index (row-major over all stripes).
    pub chunks: Vec<ChunkCommitment>,
    /// IPA commitment for the full matrix (row+column parity).
    #[norito(default)]
    #[norito(skip_serializing_if = "BlobDigest::is_zero")]
    pub ipa_commitment: BlobDigest,
    /// Additional metadata entries carried over from ingest.
    pub metadata: ExtraMetadata,
    /// Unix timestamp (seconds) when the manifest was generated.
    pub issued_at_unix: u64,
}

impl DaManifestV1 {
    /// Current manifest version number.
    pub const VERSION: u16 = 1;
}
