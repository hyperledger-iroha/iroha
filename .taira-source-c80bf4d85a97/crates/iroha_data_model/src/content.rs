//! Content bundle metadata and chunk records used by the on-chain content lane.
//!
//! A content bundle is a hashed tar archive with a precomputed file index. The
//! hash of the raw tar bytes serves as the bundle identifier and is used as the
//! HTTP `ETag` when serving files through Torii.

use std::collections::BTreeMap;

use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::AccountId,
    da::{
        prelude::DaStripeLayout,
        types::{BlobClass, BlobDigest, RetentionPolicy},
    },
    nexus::{DataSpaceId, LaneId, UniversalAccountId},
    role::RoleId,
};

/// Identifier for a content bundle (`Hash` of the tar archive).
pub type ContentBundleId = Hash;

/// Entry in a content bundle file index.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ContentFileEntry {
    /// Normalised POSIX path inside the tar archive.
    pub path: String,
    /// Byte offset of the file payload inside the tar archive.
    pub offset: u64,
    /// Length of the file payload in bytes.
    pub length: u64,
    /// Blake2b-256 hash of the file payload.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub file_hash: [u8; 32],
}

impl ContentFileEntry {
    /// Returns `true` when the entry represents an empty file.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }
}

/// Cache policy applied when serving bundle files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ContentCachePolicy {
    /// Maximum cache lifetime in seconds (used for Cache-Control max-age).
    pub max_age_seconds: u32,
    /// Whether the bundle is immutable (adds the `immutable` directive).
    pub immutable: bool,
}

impl ContentCachePolicy {
    /// Build the Cache-Control directive string.
    #[must_use]
    pub fn cache_control_value(&self) -> String {
        if self.immutable {
            format!("public, max-age={}, immutable", self.max_age_seconds)
        } else {
            format!("public, max-age={}", self.max_age_seconds)
        }
    }
}

/// Authentication/authorisation guard for bundle reads.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "mode", content = "value")]
pub enum ContentAuthMode {
    /// Bundle is publicly readable.
    Public,
    /// Readers must hold the specified role.
    RoleGate(RoleId),
    /// Readers must present the bound UAID.
    Sponsor(UniversalAccountId),
}

/// Bundle-level manifest describing cache/auth/placement metadata.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ContentBundleManifest {
    /// Stable identifier of the tar archive (BLAKE2b-256).
    pub bundle_id: ContentBundleId,
    /// Deterministic hash of the file index (Norito encoding of [`ContentFileEntry`] list).
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub index_hash: [u8; 32],
    /// Dataspace the bundle is scoped to.
    pub dataspace: DataSpaceId,
    /// Lane that owns the bundle.
    pub lane: LaneId,
    /// Semantic blob class used for DA policy selection.
    pub blob_class: BlobClass,
    /// Retention policy negotiated for the bundle.
    pub retention: RetentionPolicy,
    /// Cache policy applied by the gateway.
    pub cache: ContentCachePolicy,
    /// Authentication/authorisation policy for reads.
    pub auth: ContentAuthMode,
    /// Erasure layout negotiated for the bundle (used for DA proof sampling).
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,
    /// Optional MIME overrides per file path.
    pub mime_overrides: BTreeMap<String, String>,
}

/// Metadata and chunk layout for a published content bundle.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ContentBundleRecord {
    /// Stable identifier derived from the tar bytes.
    pub bundle_id: ContentBundleId,
    /// Web manifest attached to the bundle.
    pub manifest: ContentBundleManifest,
    /// Total length of the tar archive in bytes.
    pub total_bytes: u64,
    /// Chunk size used when ingesting the tar archive.
    pub chunk_size: u32,
    /// Ordered list of chunk hashes (BLAKE3-256).
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::fixed_bytes::vec")
    )]
    pub chunk_hashes: Vec<[u8; 32]>,
    /// Merkle-style root derived from the ordered chunk hashes.
    #[norito(default)]
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub chunk_root: [u8; 32],
    /// Erasure layout used when chunking the tarball.
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,
    /// Optional PDP/IPA commitment tied to the bundle payload.
    #[norito(default)]
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::base64_vec::option")
    )]
    pub pdp_commitment: Option<Vec<u8>>,
    /// File index entries (offsets relative to the tar archive start).
    pub files: Vec<ContentFileEntry>,
    /// Account that submitted the bundle.
    pub created_by: AccountId,
    /// Block height recorded at creation.
    pub created_height: u64,
    /// Optional block height when the bundle expires.
    pub expires_at_height: Option<u64>,
}

/// Chunk payload stored in the content lane store with a reference counter.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ContentChunk {
    /// Raw chunk bytes (at most `chunk_size` bytes).
    pub data: Vec<u8>,
    /// Number of bundles referencing this chunk.
    pub refcount: u32,
}

impl ContentChunk {
    /// Create a new chunk with a refcount of 1.
    #[must_use]
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, refcount: 1 }
    }

    /// Increment the refcount safely.
    pub fn inc(&mut self) {
        self.refcount = self.refcount.saturating_add(1);
    }

    /// Decrement the refcount and return `true` when the chunk should be removed.
    #[must_use]
    pub fn dec_and_should_prune(&mut self) -> bool {
        if self.refcount == 0 {
            return true;
        }
        self.refcount -= 1;
        self.refcount == 0
    }
}

/// Range of bytes served for a content file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ContentRange {
    /// Inclusive start offset (bytes) within the file.
    pub start: u64,
    /// Inclusive end offset (bytes) within the file.
    pub end: u64,
}

/// Receipt attached to content responses carrying DA evidence and served range metadata.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ContentDaReceipt {
    /// Identifier of the bundle that contained the served file.
    pub bundle_id: ContentBundleId,
    /// File path inside the bundle.
    pub path: String,
    /// Blake2b-256 hash of the served file payload.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub file_hash: [u8; 32],
    /// Bytes returned to the caller (range-aware).
    pub served_bytes: u64,
    /// Optional byte range served to the caller.
    #[norito(default)]
    pub range: Option<ContentRange>,
    /// Merkle-style root of the bundle chunk hashes (PoR/IPA input).
    pub chunk_root: BlobDigest,
    /// Erasure layout used for the bundle.
    pub stripe_layout: DaStripeLayout,
    /// Optional PDP/IPA commitment derived from the bundle payload.
    #[norito(default)]
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::base64_vec::option")
    )]
    pub pdp_commitment: Option<Vec<u8>>,
    /// Unix timestamp when the receipt was produced.
    pub served_at_unix: u64,
}

/// Re-export commonly used content types.
pub mod prelude {
    pub use super::{
        ContentAuthMode, ContentBundleId, ContentBundleManifest, ContentBundleRecord,
        ContentCachePolicy, ContentChunk, ContentDaReceipt, ContentFileEntry, ContentRange,
    };
}
