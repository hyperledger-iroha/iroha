//! Typed metadata contract for DA-backed query projection checkpoints.
//!
//! The full async projection worker is not wired yet, but Torii already needs a
//! stable contract for the reserved DA blob class/codec so clients can discover
//! how cold query shards will be published once the worker is enabled.

use iroha_crypto::HashOf;
use iroha_data_model::{
    block::BlockHeader,
    da::types::{BlobClass, BlobCodec, BlobDigest, Compression, StorageTicketId},
};
use norito::codec::{Decode, Encode};

/// Version of the checkpoint descriptor payload itself.
pub const QUERY_PROJECTION_CHECKPOINT_VERSION: u16 = 1;
/// Schema version for DA-backed query projection shard descriptors.
pub const QUERY_PROJECTION_SCHEMA_VERSION: u32 = 1;
/// Reserved custom DA blob-class identifier for query projection shards.
pub const QUERY_PROJECTION_DA_BLOB_CLASS_CUSTOM_ID: u16 = 1001;
/// Canonical codec label for query projection shard payloads.
pub const QUERY_PROJECTION_DA_CODEC: &str = "application/x-iroha-query-shard+norito+zstd";
/// Compression used for reserved query projection shard payloads.
pub const QUERY_PROJECTION_DA_COMPRESSION: Compression = Compression::Zstd;

/// Resource family described by a query projection checkpoint shard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode)]
pub enum QueryProjectionResourceKind {
    /// Account inventory rows and related aggregate cubes.
    Accounts,
    /// Per-account asset inventory rows and related aggregate cubes.
    AccountAssets,
    /// Asset-holder rows keyed by asset definition and account.
    AssetHolders,
    /// Asset definition inventory rows and related aggregate cubes.
    AssetDefinitions,
    /// Domain inventory rows and related aggregate cubes.
    Domains,
}

/// Reference to one immutable DA shard that belongs to a query projection checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionCheckpointShard {
    /// Resource family covered by this shard.
    pub resource: QueryProjectionResourceKind,
    /// Stable partition identifier inside the resource family.
    pub partition_id: u32,
    /// Optional asset-definition discriminator for holder shards.
    pub asset_definition_id: Option<String>,
    /// Canonical digest of the referenced DA manifest.
    pub manifest_digest: BlobDigest,
    /// Storage ticket resolving the shard in SoraFS.
    pub storage_ticket: StorageTicketId,
    /// Digest of the archived shard payload itself.
    pub blob_hash: BlobDigest,
}

/// Top-level checkpoint descriptor tying a set of projection shards to one indexed height.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionCheckpoint {
    /// Version of this descriptor payload.
    pub version: u16,
    /// Schema version for the shard contents referenced by this checkpoint.
    pub schema_version: u32,
    /// DA blob class reserved for the shard payloads.
    pub blob_class: BlobClass,
    /// Codec label reserved for the shard payloads.
    pub codec: BlobCodec,
    /// Compression applied to each shard payload.
    pub compression: Compression,
    /// Latest block height fully covered by this checkpoint.
    pub indexed_height: u64,
    /// Latest block hash fully covered by this checkpoint.
    pub indexed_block_hash: Option<HashOf<BlockHeader>>,
    /// Unix timestamp when this checkpoint was emitted.
    pub emitted_at_unix: u64,
    /// Immutable shard references that make up the checkpoint.
    pub shards: Vec<QueryProjectionCheckpointShard>,
}

impl Default for QueryProjectionCheckpoint {
    fn default() -> Self {
        Self {
            version: QUERY_PROJECTION_CHECKPOINT_VERSION,
            schema_version: QUERY_PROJECTION_SCHEMA_VERSION,
            blob_class: query_projection_da_blob_class(),
            codec: query_projection_da_codec(),
            compression: QUERY_PROJECTION_DA_COMPRESSION,
            indexed_height: 0,
            indexed_block_hash: None,
            emitted_at_unix: 0,
            shards: Vec::new(),
        }
    }
}

/// Reserved DA blob class used by query projection shards.
#[must_use]
pub const fn query_projection_da_blob_class() -> BlobClass {
    BlobClass::Custom(QUERY_PROJECTION_DA_BLOB_CLASS_CUSTOM_ID)
}

/// Reserved codec label used by query projection shards.
#[must_use]
pub fn query_projection_da_codec() -> BlobCodec {
    BlobCodec::new(QUERY_PROJECTION_DA_CODEC)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Hash;
    use norito::{decode_from_bytes, to_bytes};

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
    fn checkpoint_defaults_match_reserved_da_contract() {
        let checkpoint = QueryProjectionCheckpoint::default();
        assert_eq!(
            checkpoint.blob_class,
            BlobClass::Custom(QUERY_PROJECTION_DA_BLOB_CLASS_CUSTOM_ID)
        );
        assert_eq!(checkpoint.codec, BlobCodec::new(QUERY_PROJECTION_DA_CODEC));
        assert_eq!(checkpoint.compression, Compression::Zstd);
        assert_eq!(checkpoint.version, QUERY_PROJECTION_CHECKPOINT_VERSION);
        assert_eq!(checkpoint.schema_version, QUERY_PROJECTION_SCHEMA_VERSION);
        assert!(checkpoint.shards.is_empty());
    }

    #[test]
    fn checkpoint_round_trips_through_norito() {
        let checkpoint = QueryProjectionCheckpoint {
            indexed_height: 77,
            indexed_block_hash: Some(sample_hash(0x5A)),
            emitted_at_unix: 1_714_000_000,
            shards: vec![QueryProjectionCheckpointShard {
                resource: QueryProjectionResourceKind::AssetHolders,
                partition_id: 12,
                asset_definition_id: Some("pkr#sbp".to_string()),
                manifest_digest: sample_digest(0x11),
                storage_ticket: sample_ticket(0x22),
                blob_hash: sample_digest(0x33),
            }],
            ..QueryProjectionCheckpoint::default()
        };

        let bytes = to_bytes(&checkpoint).expect("encode checkpoint");
        let decoded: QueryProjectionCheckpoint =
            decode_from_bytes(&bytes).expect("decode checkpoint");
        assert_eq!(decoded, checkpoint);
    }
}
