//! Immutable archive contract for DA-backed query projection partitions.
//!
//! The projection worker itself is still pending, but the shard payload shape,
//! deterministic blob identifiers, and metadata contract can already be fixed.
//! This lets future DA upload code publish cold query snapshots without
//! inventing per-call conventions for compression, digests, or checkpoint
//! references.

use iroha_crypto::HashOf;
use iroha_data_model::{
    block::BlockHeader,
    da::types::{
        BlobCodec, BlobDigest, ExtraMetadata, MetadataEntry, MetadataVisibility, StorageTicketId,
    },
};
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};
use thiserror::Error;

use crate::query::{
    index_status::QueryIndexStatus,
    projection_checkpoint::{QueryProjectionCheckpointShard, QueryProjectionResourceKind},
};

/// Version of the immutable query projection shard archive payload.
pub const QUERY_PROJECTION_SHARD_ARCHIVE_VERSION: u16 = 1;
/// Codec label describing the rowset bytes carried inside a shard archive.
pub const QUERY_PROJECTION_SHARD_ROWSET_CODEC: &str =
    "application/x-iroha-query-shard-rowset+norito";
/// Metadata key recording the shard locator string alongside a DA payload.
pub const QUERY_PROJECTION_METADATA_LOCATOR_KEY: &str = "query_projection.locator";
/// Metadata key recording the projection resource family.
pub const QUERY_PROJECTION_METADATA_RESOURCE_KEY: &str = "query_projection.resource";
/// Metadata key recording the partition identifier.
pub const QUERY_PROJECTION_METADATA_PARTITION_KEY: &str = "query_projection.partition_id";
/// Metadata key recording the asset definition discriminator when present.
pub const QUERY_PROJECTION_METADATA_ASSET_KEY: &str = "query_projection.asset_definition_id";
/// Metadata key recording the indexed block height covered by the shard.
pub const QUERY_PROJECTION_METADATA_HEIGHT_KEY: &str = "query_projection.indexed_height";
/// Metadata key recording the indexed block hash covered by the shard.
pub const QUERY_PROJECTION_METADATA_BLOCK_HASH_KEY: &str =
    "query_projection.indexed_block_hash_hex";
/// Metadata key recording the logical row count inside the shard payload.
pub const QUERY_PROJECTION_METADATA_ROW_COUNT_KEY: &str = "query_projection.row_count";
/// Metadata key recording the logical payload codec for the rowset bytes.
pub const QUERY_PROJECTION_METADATA_ROWSET_CODEC_KEY: &str = "query_projection.rowset_codec";
/// Metadata key recording the digest of the logical rowset payload.
pub const QUERY_PROJECTION_METADATA_ROWSET_HASH_KEY: &str = "query_projection.rowset_hash_hex";
/// Metadata key recording the unix timestamp when the shard archive was emitted.
pub const QUERY_PROJECTION_METADATA_EMITTED_AT_KEY: &str = "query_projection.emitted_at_unix";
/// Default zstd compression level used for archived query projection payloads.
pub const QUERY_PROJECTION_DA_ZSTD_LEVEL: i32 = 3;

/// Errors returned when encoding or compressing query projection shard archives.
#[derive(Debug, Error)]
pub enum QueryProjectionShardArchiveError {
    /// Failed to Norito-encode the shard archive or its locator.
    #[error("failed to encode query projection shard archive: {0}")]
    Encode(#[source] norito::core::Error),
    /// Failed to compress the encoded archive for DA upload.
    #[error("failed to compress query projection shard archive with zstd: {0}")]
    Compress(#[source] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Encode)]
struct QueryProjectionShardLocator {
    version: u16,
    schema_version: u32,
    resource: QueryProjectionResourceKind,
    partition_id: u32,
    asset_definition_id: Option<String>,
    indexed_height: u64,
    indexed_block_hash: Option<HashOf<BlockHeader>>,
}

/// Immutable archive describing one query projection shard snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionShardArchive {
    /// Version of the archive payload itself.
    pub version: u16,
    /// Schema version of the rows/cubes carried inside this archive.
    pub schema_version: u32,
    /// Projection resource family represented by this shard.
    pub resource: QueryProjectionResourceKind,
    /// Stable partition identifier within the resource family.
    pub partition_id: u32,
    /// Optional asset-definition discriminator for holder shards.
    pub asset_definition_id: Option<String>,
    /// Latest block height fully covered by this shard snapshot.
    pub indexed_height: u64,
    /// Latest block hash fully covered by this shard snapshot.
    pub indexed_block_hash: Option<HashOf<BlockHeader>>,
    /// Unix timestamp when the archive was emitted.
    pub emitted_at_unix: u64,
    /// Number of logical rows carried in `payload`.
    pub row_count: u64,
    /// Codec label for the logical rowset bytes.
    pub payload_codec: BlobCodec,
    /// Blake3 digest of the logical rowset payload bytes.
    pub payload_hash: BlobDigest,
    /// Opaque rowset/cube payload bytes for this shard.
    pub payload: Vec<u8>,
}

/// Prepared DA payload derived from a shard archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryProjectionShardDaPayload {
    /// Deterministic client blob identifier derived from the shard locator.
    pub client_blob_id: BlobDigest,
    /// Blake3 digest of the compressed bytes submitted to DA.
    pub payload_hash: BlobDigest,
    /// zstd-compressed Norito bytes of [`QueryProjectionShardArchive`].
    pub payload: Vec<u8>,
    /// Canonical metadata describing the shard for downstream pin/introspection tooling.
    pub metadata: ExtraMetadata,
}

impl QueryProjectionResourceKind {
    /// Stable lowercase identifier for this resource family.
    #[must_use]
    pub const fn as_stable_str(self) -> &'static str {
        match self {
            Self::Accounts => "accounts",
            Self::AccountAssets => "account_assets",
            Self::AssetHolders => "asset_holders",
            Self::AssetDefinitions => "asset_definitions",
            Self::Domains => "domains",
        }
    }
}

impl QueryProjectionShardArchive {
    /// Construct a shard archive from the latest durable query index snapshot.
    #[must_use]
    pub fn from_index_status(
        status: QueryIndexStatus,
        emitted_at_unix: u64,
        resource: QueryProjectionResourceKind,
        partition_id: u32,
        asset_definition_id: Option<String>,
        row_count: u64,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            version: QUERY_PROJECTION_SHARD_ARCHIVE_VERSION,
            schema_version: crate::query::projection_checkpoint::QUERY_PROJECTION_SCHEMA_VERSION,
            resource,
            partition_id,
            asset_definition_id,
            indexed_height: status.indexed_height,
            indexed_block_hash: status.indexed_block_hash,
            emitted_at_unix,
            row_count,
            payload_codec: BlobCodec::new(QUERY_PROJECTION_SHARD_ROWSET_CODEC),
            payload_hash: BlobDigest::from_hash(blake3::hash(&payload)),
            payload,
        }
    }

    fn locator(&self) -> QueryProjectionShardLocator {
        QueryProjectionShardLocator {
            version: self.version,
            schema_version: self.schema_version,
            resource: self.resource,
            partition_id: self.partition_id,
            asset_definition_id: self.asset_definition_id.clone(),
            indexed_height: self.indexed_height,
            indexed_block_hash: self.indexed_block_hash,
        }
    }

    /// Human-readable canonical locator for this shard snapshot.
    #[must_use]
    pub fn locator_label(&self) -> String {
        let mut label = format!(
            "query-projection:{}:partition={}:height={}",
            self.resource.as_stable_str(),
            self.partition_id,
            self.indexed_height
        );
        if let Some(asset_definition_id) = self.asset_definition_id.as_deref() {
            label.push_str(":asset=");
            label.push_str(asset_definition_id);
        }
        if let Some(hash) = self.indexed_block_hash {
            label.push_str(":block=");
            label.push_str(&hex::encode(hash.as_ref()));
        }
        label
    }

    /// Deterministic client blob identifier derived from the shard locator.
    ///
    /// The identifier is stable for a given resource/partition/indexed snapshot, even if
    /// auxiliary metadata such as `row_count` or `emitted_at_unix` changes.
    ///
    /// # Errors
    ///
    /// Returns [`QueryProjectionShardArchiveError::Encode`] if the locator cannot be encoded.
    pub fn client_blob_id(&self) -> Result<BlobDigest, QueryProjectionShardArchiveError> {
        let bytes = to_bytes(&self.locator()).map_err(QueryProjectionShardArchiveError::Encode)?;
        Ok(BlobDigest::from_hash(blake3::hash(&bytes)))
    }

    /// Encode the archive itself as canonical Norito bytes.
    ///
    /// # Errors
    ///
    /// Returns [`QueryProjectionShardArchiveError::Encode`] when serialization fails.
    pub fn encode_archive(&self) -> Result<Vec<u8>, QueryProjectionShardArchiveError> {
        to_bytes(self).map_err(QueryProjectionShardArchiveError::Encode)
    }

    /// Encode the archive as zstd-compressed DA payload bytes.
    ///
    /// # Errors
    ///
    /// Returns [`QueryProjectionShardArchiveError`] when encoding or compression fails.
    pub fn encode_da_payload(&self) -> Result<Vec<u8>, QueryProjectionShardArchiveError> {
        let encoded = self.encode_archive()?;
        zstd::bulk::compress(&encoded, QUERY_PROJECTION_DA_ZSTD_LEVEL)
            .map_err(QueryProjectionShardArchiveError::Compress)
    }

    /// Build canonical DA metadata entries describing this shard archive.
    #[must_use]
    pub fn da_metadata(&self) -> ExtraMetadata {
        let mut items = vec![
            metadata_entry(
                QUERY_PROJECTION_METADATA_LOCATOR_KEY,
                self.locator_label().into_bytes(),
            ),
            metadata_entry(
                QUERY_PROJECTION_METADATA_RESOURCE_KEY,
                self.resource.as_stable_str().as_bytes().to_vec(),
            ),
            metadata_entry(
                QUERY_PROJECTION_METADATA_PARTITION_KEY,
                self.partition_id.to_string().into_bytes(),
            ),
            metadata_entry(
                QUERY_PROJECTION_METADATA_HEIGHT_KEY,
                self.indexed_height.to_string().into_bytes(),
            ),
            metadata_entry(
                QUERY_PROJECTION_METADATA_ROW_COUNT_KEY,
                self.row_count.to_string().into_bytes(),
            ),
            metadata_entry(
                QUERY_PROJECTION_METADATA_ROWSET_CODEC_KEY,
                self.payload_codec.0.as_bytes().to_vec(),
            ),
            metadata_entry(
                QUERY_PROJECTION_METADATA_ROWSET_HASH_KEY,
                hex::encode(self.payload_hash.as_bytes()).into_bytes(),
            ),
            metadata_entry(
                QUERY_PROJECTION_METADATA_EMITTED_AT_KEY,
                self.emitted_at_unix.to_string().into_bytes(),
            ),
        ];
        if let Some(asset_definition_id) = self.asset_definition_id.as_deref() {
            items.push(metadata_entry(
                QUERY_PROJECTION_METADATA_ASSET_KEY,
                asset_definition_id.as_bytes().to_vec(),
            ));
        }
        if let Some(hash) = self.indexed_block_hash {
            items.push(metadata_entry(
                QUERY_PROJECTION_METADATA_BLOCK_HASH_KEY,
                hex::encode(hash.as_ref()).into_bytes(),
            ));
        }
        ExtraMetadata { items }
    }

    /// Build the compressed payload and metadata bundle that a DA worker can ingest.
    ///
    /// # Errors
    ///
    /// Returns [`QueryProjectionShardArchiveError`] when encoding or compression fails.
    pub fn build_da_payload(
        &self,
    ) -> Result<QueryProjectionShardDaPayload, QueryProjectionShardArchiveError> {
        let payload = self.encode_da_payload()?;
        Ok(QueryProjectionShardDaPayload {
            client_blob_id: self.client_blob_id()?,
            payload_hash: BlobDigest::from_hash(blake3::hash(&payload)),
            payload,
            metadata: self.da_metadata(),
        })
    }

    /// Convert an uploaded archive into the checkpoint shard reference persisted in state.
    ///
    /// # Errors
    ///
    /// Returns [`QueryProjectionShardArchiveError`] when the compressed archive cannot be built.
    pub fn into_checkpoint_shard(
        &self,
        manifest_digest: BlobDigest,
        storage_ticket: StorageTicketId,
    ) -> Result<QueryProjectionCheckpointShard, QueryProjectionShardArchiveError> {
        let da_payload = self.build_da_payload()?;
        Ok(QueryProjectionCheckpointShard {
            resource: self.resource,
            partition_id: self.partition_id,
            asset_definition_id: self.asset_definition_id.clone(),
            manifest_digest,
            storage_ticket,
            blob_hash: da_payload.payload_hash,
        })
    }
}

fn metadata_entry(key: impl Into<String>, value: Vec<u8>) -> MetadataEntry {
    MetadataEntry::new(key, value, MetadataVisibility::Public)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Hash;
    use norito::decode_from_bytes;

    fn sample_hash(byte: u8) -> HashOf<BlockHeader> {
        HashOf::from_untyped_unchecked(Hash::new([byte; Hash::LENGTH]))
    }

    fn sample_digest(byte: u8) -> BlobDigest {
        BlobDigest::new([byte; 32])
    }

    fn sample_ticket(byte: u8) -> StorageTicketId {
        StorageTicketId::new([byte; 32])
    }

    #[test]
    fn shard_archive_uses_status_snapshot_and_payload_digest() {
        let archive = QueryProjectionShardArchive::from_index_status(
            QueryIndexStatus {
                indexed_height: 144,
                indexed_block_hash: Some(sample_hash(0x44)),
            },
            1_714_000_777,
            QueryProjectionResourceKind::AssetHolders,
            12,
            Some("pkr#sbp".to_string()),
            3,
            b"rows".to_vec(),
        );

        assert_eq!(archive.version, QUERY_PROJECTION_SHARD_ARCHIVE_VERSION);
        assert_eq!(
            archive.payload_codec,
            BlobCodec::new(QUERY_PROJECTION_SHARD_ROWSET_CODEC)
        );
        assert_eq!(archive.indexed_height, 144);
        assert_eq!(archive.indexed_block_hash, Some(sample_hash(0x44)));
        assert_eq!(
            archive.payload_hash,
            BlobDigest::from_hash(blake3::hash(b"rows"))
        );
        assert_eq!(
            archive.locator_label(),
            format!(
                "query-projection:asset_holders:partition=12:height=144:asset=pkr#sbp:block={}",
                hex::encode(sample_hash(0x44).as_ref())
            )
        );
    }

    #[test]
    fn shard_archive_round_trips_through_norito() {
        let archive = QueryProjectionShardArchive::from_index_status(
            QueryIndexStatus {
                indexed_height: 9,
                indexed_block_hash: Some(sample_hash(0x11)),
            },
            1_714_000_222,
            QueryProjectionResourceKind::Accounts,
            4,
            None,
            2,
            b"payload".to_vec(),
        );

        let bytes = archive.encode_archive().expect("encode archive");
        let decoded: QueryProjectionShardArchive =
            decode_from_bytes(&bytes).expect("decode archive");
        assert_eq!(decoded, archive);
    }

    #[test]
    fn shard_archive_builds_stable_da_payload_and_checkpoint_reference() {
        let archive = QueryProjectionShardArchive::from_index_status(
            QueryIndexStatus {
                indexed_height: 77,
                indexed_block_hash: Some(sample_hash(0x5A)),
            },
            1_714_000_333,
            QueryProjectionResourceKind::AssetHolders,
            31,
            Some("pkr#sbp".to_string()),
            1,
            b"rowset".to_vec(),
        );

        let first_payload = archive.build_da_payload().expect("build payload");
        let second_payload = archive.build_da_payload().expect("build payload again");
        assert_eq!(
            first_payload, second_payload,
            "payload must be deterministic"
        );
        assert_eq!(
            first_payload.client_blob_id,
            archive.client_blob_id().expect("blob id")
        );
        assert!(!first_payload.payload.is_empty());

        let expected_archive = archive.encode_archive().expect("encode archive");
        let decompressed = zstd::bulk::decompress(&first_payload.payload, expected_archive.len())
            .expect("decompress payload");
        let decoded: QueryProjectionShardArchive =
            decode_from_bytes(&decompressed).expect("decode compressed archive");
        assert_eq!(decoded, archive);

        let metadata_keys: Vec<&str> = first_payload
            .metadata
            .items
            .iter()
            .map(|entry| entry.key.as_str())
            .collect();
        assert!(metadata_keys.contains(&QUERY_PROJECTION_METADATA_LOCATOR_KEY));
        assert!(metadata_keys.contains(&QUERY_PROJECTION_METADATA_RESOURCE_KEY));
        assert!(metadata_keys.contains(&QUERY_PROJECTION_METADATA_ROWSET_HASH_KEY));
        assert!(metadata_keys.contains(&QUERY_PROJECTION_METADATA_ASSET_KEY));

        let checkpoint = archive
            .into_checkpoint_shard(sample_digest(0x22), sample_ticket(0x33))
            .expect("checkpoint shard");
        assert_eq!(
            checkpoint.resource,
            QueryProjectionResourceKind::AssetHolders
        );
        assert_eq!(checkpoint.partition_id, 31);
        assert_eq!(checkpoint.asset_definition_id.as_deref(), Some("pkr#sbp"));
        assert_eq!(checkpoint.manifest_digest, sample_digest(0x22));
        assert_eq!(checkpoint.storage_ticket, sample_ticket(0x33));
        assert_eq!(checkpoint.blob_hash, first_payload.payload_hash);
    }
}
