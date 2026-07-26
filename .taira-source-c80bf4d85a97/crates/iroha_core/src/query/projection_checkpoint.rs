//! Typed metadata contract for DA-backed query projection checkpoints.
//!
//! The full async projection worker is not wired yet, but Torii already needs a
//! stable contract for the reserved DA blob class/codec so clients can discover
//! how cold query shards will be published once the worker is enabled.

use std::collections::HashSet;

use iroha_crypto::HashOf;
use iroha_data_model::{
    block::BlockHeader,
    da::types::{BlobClass, BlobCodec, BlobDigest, Compression, StorageTicketId},
};
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::query::{
    index_status::QueryIndexStatus,
    projection_shard::{
        QUERY_PROJECTION_SHARD_ARCHIVE_VERSION, QUERY_PROJECTION_SHARD_ROWSET_CODEC,
        QueryProjectionShardArchive, QueryProjectionShardArchiveError,
    },
};

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
/// Default partition count for account-scoped query projection shards.
pub const QUERY_PROJECTION_DEFAULT_PARTITION_COUNT: u32 = 4096;

/// Resource family described by a query projection checkpoint shard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Encode, Decode)]
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

/// Uploaded immutable shard archive ready to be referenced from a checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryProjectionUploadedShardArchive {
    /// Immutable shard archive snapshot to publish.
    pub archive: QueryProjectionShardArchive,
    /// Canonical digest of the DA manifest pinning this shard archive.
    pub manifest_digest: BlobDigest,
    /// Storage ticket resolving the shard archive in SoraFS.
    pub storage_ticket: StorageTicketId,
}

/// Errors returned when building a checkpoint publish plan from uploaded archives.
#[derive(Debug, Error)]
pub enum QueryProjectionCheckpointPlanError {
    /// A shard archive uses an unsupported archive-version marker.
    #[error(
        "query projection shard {resource:?}/partition={partition_id}/asset={asset_definition_id:?} uses unsupported archive version {found} (expected {expected})"
    )]
    UnsupportedArchiveVersion {
        /// Expected archive version.
        expected: u16,
        /// Archive version found in the uploaded shard.
        found: u16,
        /// Resource family covered by the shard.
        resource: QueryProjectionResourceKind,
        /// Stable partition identifier inside the resource family.
        partition_id: u32,
        /// Optional asset-definition discriminator.
        asset_definition_id: Option<String>,
    },
    /// A shard archive uses an unexpected logical rowset codec.
    #[error(
        "query projection shard {resource:?}/partition={partition_id}/asset={asset_definition_id:?} uses unexpected rowset codec {found} (expected {expected})"
    )]
    UnexpectedPayloadCodec {
        /// Expected codec label.
        expected: String,
        /// Codec found in the uploaded shard.
        found: String,
        /// Resource family covered by the shard.
        resource: QueryProjectionResourceKind,
        /// Stable partition identifier inside the resource family.
        partition_id: u32,
        /// Optional asset-definition discriminator.
        asset_definition_id: Option<String>,
    },
    /// A shard archive uses an unexpected schema version.
    #[error(
        "query projection shard {resource:?}/partition={partition_id}/asset={asset_definition_id:?} uses unexpected schema version {found} (expected {expected})"
    )]
    UnexpectedSchemaVersion {
        /// Expected schema version.
        expected: u32,
        /// Schema version found in the uploaded shard.
        found: u32,
        /// Resource family covered by the shard.
        resource: QueryProjectionResourceKind,
        /// Stable partition identifier inside the resource family.
        partition_id: u32,
        /// Optional asset-definition discriminator.
        asset_definition_id: Option<String>,
    },
    /// A shard archive payload hash does not match its logical payload bytes.
    #[error(
        "query projection shard {resource:?}/partition={partition_id}/asset={asset_definition_id:?} has payload hash {found} but payload bytes hash to {expected}"
    )]
    PayloadHashMismatch {
        /// Digest computed from the payload bytes.
        expected: String,
        /// Digest advertised by the shard archive.
        found: String,
        /// Resource family covered by the shard.
        resource: QueryProjectionResourceKind,
        /// Stable partition identifier inside the resource family.
        partition_id: u32,
        /// Optional asset-definition discriminator.
        asset_definition_id: Option<String>,
    },
    /// A shard archive covers a different indexed height than the checkpoint snapshot.
    #[error(
        "query projection shard {resource:?}/partition={partition_id}/asset={asset_definition_id:?} covers indexed height {found} but checkpoint snapshot is {expected}"
    )]
    IndexedHeightMismatch {
        /// Indexed height from the checkpoint snapshot.
        expected: u64,
        /// Indexed height found in the shard archive.
        found: u64,
        /// Resource family covered by the shard.
        resource: QueryProjectionResourceKind,
        /// Stable partition identifier inside the resource family.
        partition_id: u32,
        /// Optional asset-definition discriminator.
        asset_definition_id: Option<String>,
    },
    /// A shard archive covers a different indexed block hash than the checkpoint snapshot.
    #[error(
        "query projection shard {resource:?}/partition={partition_id}/asset={asset_definition_id:?} covers indexed block hash {found:?} but checkpoint snapshot is {expected:?}"
    )]
    IndexedBlockHashMismatch {
        /// Indexed block hash from the checkpoint snapshot.
        expected: Option<String>,
        /// Indexed block hash found in the shard archive.
        found: Option<String>,
        /// Resource family covered by the shard.
        resource: QueryProjectionResourceKind,
        /// Stable partition identifier inside the resource family.
        partition_id: u32,
        /// Optional asset-definition discriminator.
        asset_definition_id: Option<String>,
    },
    /// Duplicate shard keys are not allowed inside a single checkpoint.
    #[error(
        "duplicate query projection shard {resource:?}/partition={partition_id}/asset={asset_definition_id:?} in checkpoint publish plan"
    )]
    DuplicateShard {
        /// Resource family covered by the duplicated shard.
        resource: QueryProjectionResourceKind,
        /// Stable partition identifier inside the resource family.
        partition_id: u32,
        /// Optional asset-definition discriminator.
        asset_definition_id: Option<String>,
    },
    /// Building the referenced checkpoint shard failed.
    #[error(transparent)]
    Archive(#[from] QueryProjectionShardArchiveError),
}

/// Validated checkpoint publication plan derived from uploaded shard archives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryProjectionCheckpointPublishPlan {
    checkpoint: QueryProjectionCheckpoint,
}

impl QueryProjectionCheckpoint {
    /// Build a checkpoint descriptor from the latest durable query index snapshot.
    #[must_use]
    pub fn from_index_status(
        status: QueryIndexStatus,
        emitted_at_unix: u64,
        shards: Vec<QueryProjectionCheckpointShard>,
    ) -> Self {
        Self {
            indexed_height: status.indexed_height,
            indexed_block_hash: status.indexed_block_hash,
            emitted_at_unix,
            shards,
            ..Self::default()
        }
    }
}

impl QueryProjectionUploadedShardArchive {
    /// Construct a typed uploaded-shard descriptor.
    #[must_use]
    pub fn new(
        archive: QueryProjectionShardArchive,
        manifest_digest: BlobDigest,
        storage_ticket: StorageTicketId,
    ) -> Self {
        Self {
            archive,
            manifest_digest,
            storage_ticket,
        }
    }
}

impl From<(QueryProjectionShardArchive, BlobDigest, StorageTicketId)>
    for QueryProjectionUploadedShardArchive
{
    fn from(
        (archive, manifest_digest, storage_ticket): (
            QueryProjectionShardArchive,
            BlobDigest,
            StorageTicketId,
        ),
    ) -> Self {
        Self::new(archive, manifest_digest, storage_ticket)
    }
}

impl QueryProjectionCheckpointPublishPlan {
    /// Build a validated checkpoint publication plan from uploaded shard archives.
    ///
    /// # Errors
    ///
    /// Returns [`QueryProjectionCheckpointPlanError`] when the uploaded shards do not
    /// form a coherent immutable checkpoint for the provided query-index snapshot.
    pub fn from_uploaded_archives<I>(
        status: QueryIndexStatus,
        emitted_at_unix: u64,
        uploads: I,
    ) -> Result<Self, QueryProjectionCheckpointPlanError>
    where
        I: IntoIterator<Item = QueryProjectionUploadedShardArchive>,
    {
        let expected_payload_codec = BlobCodec::new(QUERY_PROJECTION_SHARD_ROWSET_CODEC);
        let expected_block_hash_hex = status
            .indexed_block_hash
            .map(|hash| hex::encode(hash.as_ref()));
        let mut seen = HashSet::new();
        let mut shards = Vec::new();

        for upload in uploads {
            let archive = upload.archive;
            let key = (
                archive.resource,
                archive.partition_id,
                archive.asset_definition_id.clone(),
            );
            if !seen.insert(key.clone()) {
                return Err(QueryProjectionCheckpointPlanError::DuplicateShard {
                    resource: key.0,
                    partition_id: key.1,
                    asset_definition_id: key.2,
                });
            }

            if archive.version != QUERY_PROJECTION_SHARD_ARCHIVE_VERSION {
                return Err(
                    QueryProjectionCheckpointPlanError::UnsupportedArchiveVersion {
                        expected: QUERY_PROJECTION_SHARD_ARCHIVE_VERSION,
                        found: archive.version,
                        resource: archive.resource,
                        partition_id: archive.partition_id,
                        asset_definition_id: archive.asset_definition_id.clone(),
                    },
                );
            }

            if archive.schema_version != QUERY_PROJECTION_SCHEMA_VERSION {
                return Err(
                    QueryProjectionCheckpointPlanError::UnexpectedSchemaVersion {
                        expected: QUERY_PROJECTION_SCHEMA_VERSION,
                        found: archive.schema_version,
                        resource: archive.resource,
                        partition_id: archive.partition_id,
                        asset_definition_id: archive.asset_definition_id.clone(),
                    },
                );
            }

            if archive.payload_codec != expected_payload_codec {
                return Err(QueryProjectionCheckpointPlanError::UnexpectedPayloadCodec {
                    expected: expected_payload_codec.0.clone(),
                    found: archive.payload_codec.0.clone(),
                    resource: archive.resource,
                    partition_id: archive.partition_id,
                    asset_definition_id: archive.asset_definition_id.clone(),
                });
            }

            let computed_payload_hash = BlobDigest::from_hash(blake3::hash(&archive.payload));
            if archive.payload_hash != computed_payload_hash {
                return Err(QueryProjectionCheckpointPlanError::PayloadHashMismatch {
                    expected: hex::encode(computed_payload_hash.as_bytes()),
                    found: hex::encode(archive.payload_hash.as_bytes()),
                    resource: archive.resource,
                    partition_id: archive.partition_id,
                    asset_definition_id: archive.asset_definition_id.clone(),
                });
            }

            if archive.indexed_height != status.indexed_height {
                return Err(QueryProjectionCheckpointPlanError::IndexedHeightMismatch {
                    expected: status.indexed_height,
                    found: archive.indexed_height,
                    resource: archive.resource,
                    partition_id: archive.partition_id,
                    asset_definition_id: archive.asset_definition_id.clone(),
                });
            }

            if archive.indexed_block_hash != status.indexed_block_hash {
                return Err(
                    QueryProjectionCheckpointPlanError::IndexedBlockHashMismatch {
                        expected: expected_block_hash_hex.clone(),
                        found: archive
                            .indexed_block_hash
                            .map(|hash| hex::encode(hash.as_ref())),
                        resource: archive.resource,
                        partition_id: archive.partition_id,
                        asset_definition_id: archive.asset_definition_id.clone(),
                    },
                );
            }

            shards.push(
                archive.into_checkpoint_shard(upload.manifest_digest, upload.storage_ticket)?,
            );
        }

        Ok(Self {
            checkpoint: QueryProjectionCheckpoint::from_index_status(
                status,
                emitted_at_unix,
                shards,
            ),
        })
    }

    /// Borrow the validated checkpoint descriptor.
    #[must_use]
    pub fn checkpoint(&self) -> &QueryProjectionCheckpoint {
        &self.checkpoint
    }

    /// Consume the plan and return the validated checkpoint descriptor.
    #[must_use]
    pub fn into_checkpoint(self) -> QueryProjectionCheckpoint {
        self.checkpoint
    }
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

/// Deterministically map a canonical key into one of the configured projection partitions.
#[must_use]
pub fn query_projection_partition_for_key(key: &[u8], partition_count: u32) -> u32 {
    assert!(partition_count > 0, "partition_count must be non-zero");
    let digest = blake3::hash(key);
    let lane = u32::from_le_bytes(digest.as_bytes()[0..4].try_into().expect("four bytes"));
    lane % partition_count
}

/// Deterministically map a canonical account key into the default projection partition set.
#[must_use]
pub fn query_projection_default_partition_for_account(account_key: &str) -> u32 {
    query_projection_partition_for_key(
        account_key.as_bytes(),
        QUERY_PROJECTION_DEFAULT_PARTITION_COUNT,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::query::projection_shard::QueryProjectionShardArchive;
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

    fn sample_upload(
        status: QueryIndexStatus,
        resource: QueryProjectionResourceKind,
        partition_id: u32,
        asset_definition_id: Option<&str>,
    ) -> QueryProjectionUploadedShardArchive {
        QueryProjectionUploadedShardArchive::new(
            QueryProjectionShardArchive::from_index_status(
                status,
                1_714_000_111,
                resource,
                partition_id,
                asset_definition_id.map(str::to_owned),
                2,
                b"rows".to_vec(),
            ),
            sample_digest(0x33),
            sample_ticket(0x44),
        )
    }

    #[test]
    fn partition_helper_is_stable_and_bounded() {
        let first = query_projection_default_partition_for_account("alice@hbl.paynet");
        let second = query_projection_default_partition_for_account("alice@hbl.paynet");
        let other = query_projection_default_partition_for_account("bob@ubl.paynet");

        assert_eq!(first, second, "partition mapping must be stable");
        assert!(first < QUERY_PROJECTION_DEFAULT_PARTITION_COUNT);
        assert!(other < QUERY_PROJECTION_DEFAULT_PARTITION_COUNT);
    }

    #[test]
    fn checkpoint_builds_from_query_index_status() {
        let status = QueryIndexStatus {
            indexed_height: 9,
            indexed_block_hash: Some(sample_hash(0x55)),
        };
        let checkpoint = QueryProjectionCheckpoint::from_index_status(
            status,
            1_714_000_321,
            vec![QueryProjectionCheckpointShard {
                resource: QueryProjectionResourceKind::Accounts,
                partition_id: 1,
                asset_definition_id: None,
                manifest_digest: sample_digest(0x11),
                storage_ticket: sample_ticket(0x22),
                blob_hash: sample_digest(0x33),
            }],
        );

        assert_eq!(checkpoint.indexed_height, 9);
        assert_eq!(checkpoint.indexed_block_hash, status.indexed_block_hash);
        assert_eq!(checkpoint.emitted_at_unix, 1_714_000_321);
        assert_eq!(checkpoint.shards.len(), 1);
        assert_eq!(checkpoint.version, QUERY_PROJECTION_CHECKPOINT_VERSION);
        assert_eq!(checkpoint.schema_version, QUERY_PROJECTION_SCHEMA_VERSION);
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
                asset_definition_id: Some("pkr#paynet".to_string()),
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

    #[test]
    fn checkpoint_publish_plan_builds_checkpoint_from_uploaded_archives() {
        let status = QueryIndexStatus {
            indexed_height: 77,
            indexed_block_hash: Some(sample_hash(0x5A)),
        };
        let plan = QueryProjectionCheckpointPublishPlan::from_uploaded_archives(
            status,
            1_714_000_444,
            vec![
                sample_upload(status, QueryProjectionResourceKind::Accounts, 7, None),
                sample_upload(
                    status,
                    QueryProjectionResourceKind::AssetHolders,
                    11,
                    Some("pkr#paynet"),
                ),
            ],
        )
        .expect("build publish plan");

        assert_eq!(plan.checkpoint().indexed_height, 77);
        assert_eq!(
            plan.checkpoint().indexed_block_hash,
            Some(sample_hash(0x5A))
        );
        assert_eq!(plan.checkpoint().emitted_at_unix, 1_714_000_444);
        assert_eq!(plan.checkpoint().shards.len(), 2);
        assert_eq!(
            plan.checkpoint().shards[1].asset_definition_id.as_deref(),
            Some("pkr#paynet")
        );
    }

    #[test]
    fn checkpoint_publish_plan_rejects_duplicate_shards() {
        let status = QueryIndexStatus {
            indexed_height: 77,
            indexed_block_hash: Some(sample_hash(0x5A)),
        };
        let err = QueryProjectionCheckpointPublishPlan::from_uploaded_archives(
            status,
            1_714_000_444,
            vec![
                sample_upload(status, QueryProjectionResourceKind::Accounts, 7, None),
                sample_upload(status, QueryProjectionResourceKind::Accounts, 7, None),
            ],
        )
        .expect_err("duplicate shard must fail");

        assert!(matches!(
            err,
            QueryProjectionCheckpointPlanError::DuplicateShard {
                resource: QueryProjectionResourceKind::Accounts,
                partition_id: 7,
                asset_definition_id: None,
            }
        ));
    }

    #[test]
    fn checkpoint_publish_plan_rejects_mismatched_index_snapshot() {
        let status = QueryIndexStatus {
            indexed_height: 77,
            indexed_block_hash: Some(sample_hash(0x5A)),
        };
        let other_status = QueryIndexStatus {
            indexed_height: 78,
            indexed_block_hash: Some(sample_hash(0x5B)),
        };
        let err = QueryProjectionCheckpointPublishPlan::from_uploaded_archives(
            status,
            1_714_000_444,
            vec![sample_upload(
                other_status,
                QueryProjectionResourceKind::Accounts,
                7,
                None,
            )],
        )
        .expect_err("mismatched snapshot must fail");

        assert!(matches!(
            err,
            QueryProjectionCheckpointPlanError::IndexedHeightMismatch {
                expected: 77,
                found: 78,
                resource: QueryProjectionResourceKind::Accounts,
                partition_id: 7,
                asset_definition_id: None,
            }
        ));
    }

    #[test]
    fn checkpoint_publish_plan_rejects_tampered_payload_hash() {
        let status = QueryIndexStatus {
            indexed_height: 77,
            indexed_block_hash: Some(sample_hash(0x5A)),
        };
        let mut upload = sample_upload(status, QueryProjectionResourceKind::Accounts, 7, None);
        upload.archive.payload_hash = sample_digest(0x99);

        let err = QueryProjectionCheckpointPublishPlan::from_uploaded_archives(
            status,
            1_714_000_444,
            vec![upload],
        )
        .expect_err("tampered payload hash must fail");

        assert!(matches!(
            err,
            QueryProjectionCheckpointPlanError::PayloadHashMismatch {
                resource: QueryProjectionResourceKind::Accounts,
                partition_id: 7,
                asset_definition_id: None,
                ..
            }
        ));
    }
}
