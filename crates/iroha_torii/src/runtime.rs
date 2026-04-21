//! Runtime upgrade app API handlers.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{extract::Path, response::IntoResponse};
use iroha_core::{
    query::projection_checkpoint::{
        QUERY_PROJECTION_DA_BLOB_CLASS_CUSTOM_ID, QUERY_PROJECTION_DA_CODEC,
        QUERY_PROJECTION_DA_COMPRESSION, QUERY_PROJECTION_DEFAULT_PARTITION_COUNT,
        QUERY_PROJECTION_SCHEMA_VERSION, QueryProjectionCheckpoint,
        QueryProjectionCheckpointPlanError, QueryProjectionResourceKind,
        QueryProjectionUploadedShardArchive, query_projection_default_partition_for_account,
    },
    query::projection_rowset::{
        QueryProjectionAccountRow, QueryProjectionAccountsShardRowSet,
        QueryProjectionAssetHolderRow, QueryProjectionAssetHoldersShardRowSet,
        QueryProjectionShardRowSet,
    },
    query::projection_shard::{
        QUERY_PROJECTION_METADATA_ASSET_KEY, QUERY_PROJECTION_METADATA_BLOCK_HASH_KEY,
        QUERY_PROJECTION_METADATA_EMITTED_AT_KEY, QUERY_PROJECTION_METADATA_HEIGHT_KEY,
        QUERY_PROJECTION_METADATA_LOCATOR_KEY, QUERY_PROJECTION_METADATA_PARTITION_KEY,
        QUERY_PROJECTION_METADATA_RESOURCE_KEY, QUERY_PROJECTION_METADATA_ROW_COUNT_KEY,
        QUERY_PROJECTION_METADATA_ROWSET_CODEC_KEY, QUERY_PROJECTION_METADATA_ROWSET_HASH_KEY,
        QUERY_PROJECTION_SHARD_ARCHIVE_VERSION, QUERY_PROJECTION_SHARD_ROWSET_CODEC,
        QueryProjectionShardArchive,
    },
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_crypto::Algorithm;
use iroha_data_model::{
    Identifiable,
    account::curve::CurveId,
    da::types::{BlobClass, BlobDigest, Compression, StorageTicketId},
    transaction::SignedTransaction,
};
use iroha_logger::warn;
use mv::storage::StorageReadOnly;
use norito::derive::{NoritoDeserialize, NoritoSerialize};

use crate::{
    NoritoJson,
    json_macros::{JsonDeserialize, JsonSerialize},
};

const CURVE_REGISTRY_VERSION: u32 = 1;
const QUERY_PROJECTION_SHARD_CATALOG_VERSION: u16 = 1;
const QUERY_PROJECTION_SHARD_CATALOG_DEFAULT_LIMIT: u32 = 1024;
const QUERY_PROJECTION_SHARD_CATALOG_MAX_LIMIT: u32 = 8192;

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Node capabilities advert (subset)
pub struct NodeCapabilitiesResponse {
    /// ABI version accepted by this node.
    pub abi_version: u16,
    /// Data model compatibility version for SDK handshakes.
    pub data_model_version: u32,
    /// Norito schema hash for `SignedTransaction` submit payloads.
    pub signed_transaction_schema_hash_hex: String,
    /// Cryptography capabilities (SM, default hashes, allow-lists)
    pub crypto: NodeCryptoCapabilities,
    /// Query DSL and projection-index capabilities.
    pub query: NodeQueryCapabilities,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Crypto capability advert (currently SM-focused).
pub struct NodeCryptoCapabilities {
    /// SM cryptography capability manifest.
    pub sm: NodeSmCapabilities,
    /// Curve capability advert anchored to the registry.
    pub curves: NodeCurveCapabilities,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// SM capability manifest exported by the node.
pub struct NodeSmCapabilities {
    /// Whether SM helpers are enabled in this node.
    pub enabled: bool,
    /// Default transaction hash algorithm.
    pub default_hash: String,
    /// Admission allow-list for signing algorithms.
    pub allowed_signing: Vec<String>,
    /// Default SM2 distinguishing identifier.
    pub sm2_distid_default: String,
    /// Whether the OpenSSL/Tongsuo preview backend is toggled on.
    pub openssl_preview: bool,
    /// Acceleration advert (scalar/NEON policy).
    pub acceleration: NodeSmAcceleration,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Hardware/software acceleration advert for SM algorithms.
pub struct NodeSmAcceleration {
    /// Scalar implementation availability (always true).
    pub scalar: bool,
    /// NEON accelerated SM3 hashing available.
    pub neon_sm3: bool,
    /// NEON accelerated SM4 block operations available.
    pub neon_sm4: bool,
    /// Dispatch policy string (`auto`, `force-enable`, `force-disable`, `scalar-only`).
    pub policy: String,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Curve capability advert emitted by `/v1/node/capabilities`.
pub struct NodeCurveCapabilities {
    /// Registry version referenced by this advert.
    pub registry_version: u32,
    /// Allowed curve identifiers (as published in the registry).
    pub allowed_curve_ids: Vec<u8>,
    /// Bitmap of allowed curve identifiers (bit `i` ⇒ curve id `i`).
    #[norito(default)]
    #[norito(skip_serializing_if = "Vec::is_empty")]
    pub allowed_curve_bitmap: Vec<u64>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Query capability advert emitted by `/v1/node/capabilities`.
pub struct NodeQueryCapabilities {
    /// Aggregate query support exposed today.
    pub aggregate: NodeAggregateQueryCapabilities,
    /// Whether aggregate responses report a durable indexed snapshot marker.
    pub indexed_snapshot_marker: bool,
    /// Additional alias-aware fields injected into aggregate-capable row responses.
    pub row_enrichment_fields: Vec<String>,
    /// Reserved DA-backed projection checkpoint contract.
    pub projection: NodeProjectionCapabilities,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Aggregate DSL capability advert emitted by `/v1/node/capabilities`.
pub struct NodeAggregateQueryCapabilities {
    /// Whether aggregate DSL v1 is available.
    pub v1: bool,
    /// Whether the current aggregate implementation is exact rather than approximate.
    pub exact_results: bool,
    /// Resource families that currently accept aggregate mode.
    pub supported_resources: Vec<String>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Reserved DA query projection capability advert.
pub struct NodeProjectionCapabilities {
    /// Whether the checkpoint descriptor contract is defined and stable.
    pub checkpoint_contract_v1: bool,
    /// Whether the DA-backed cold projection worker is enabled.
    pub da_v1_enabled: bool,
    /// Whether callers can preflight checkpoint publication directly through Torii.
    pub checkpoint_plan_v1: bool,
    /// Whether callers can persist a rebuilt projection checkpoint directly through Torii.
    pub checkpoint_publish_v1: bool,
    /// Whether the node can enumerate the canonical live shard set directly.
    pub shard_catalog_v1: bool,
    /// Whether the node can export canonical projection shard archives directly.
    pub archive_export_v1: bool,
    /// Version of the immutable shard archive payload carried inside DA blobs.
    pub archive_version: u16,
    /// Schema version of the reserved checkpoint shard payload.
    pub schema_version: u32,
    /// Reserved `BlobClass::Custom(..)` identifier for query projection shards.
    pub blob_class_custom_id: u16,
    /// Reserved codec label for query projection shards.
    pub codec: String,
    /// Codec label for the logical rowset bytes inside the shard archive.
    pub rowset_codec: String,
    /// Compression used for reserved query projection shards.
    pub compression: String,
    /// Default partition count for account-scoped projection shards.
    pub default_partition_count: u32,
    /// Canonical public metadata keys expected on DA payloads for query projection shards.
    pub metadata_keys: Vec<String>,
    /// Resource families the shard export contract currently supports.
    pub export_supported_resources: Vec<String>,
    /// Latest indexed height covered by a persisted projection checkpoint, if any.
    pub latest_checkpoint_indexed_height: Option<u64>,
    /// Latest indexed block hash covered by a persisted projection checkpoint, if any.
    pub latest_checkpoint_block_hash_hex: Option<String>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Response for the latest persisted query projection checkpoint descriptor.
pub struct NodeProjectionCheckpointResponse {
    /// Descriptor payload version.
    pub version: u16,
    /// Projection shard schema version covered by this checkpoint.
    pub schema_version: u32,
    /// Semantic blob class label used by the referenced shard payloads.
    pub blob_class: String,
    /// Custom blob class id when `blob_class` is `custom`.
    pub blob_class_custom_id: Option<u16>,
    /// Codec label used by the referenced shard payloads.
    pub codec: String,
    /// Compression used by the referenced shard payloads.
    pub compression: String,
    /// Latest block height covered by this checkpoint.
    pub indexed_height: u64,
    /// Latest block hash covered by this checkpoint, hex-encoded when present.
    pub indexed_block_hash_hex: Option<String>,
    /// Unix timestamp when the checkpoint descriptor was emitted.
    pub emitted_at_unix: u64,
    /// Immutable shard references that make up this checkpoint.
    pub shards: Vec<NodeProjectionCheckpointShardRef>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// JSON/Norito-friendly representation of one shard reference inside a checkpoint.
pub struct NodeProjectionCheckpointShardRef {
    /// Stable resource family identifier.
    pub resource: String,
    /// Stable partition identifier inside the resource family.
    pub partition_id: u32,
    /// Optional asset definition discriminator for holder shards.
    pub asset_definition_id: Option<String>,
    /// Manifest digest, lowercase hex.
    pub manifest_digest_hex: String,
    /// Storage ticket id, lowercase hex.
    pub storage_ticket_hex: String,
    /// Compressed blob hash, lowercase hex.
    pub blob_hash_hex: String,
}

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Request body for planning or publishing a projection checkpoint from uploaded shard refs.
pub struct NodeProjectionCheckpointPublishRequest {
    /// Unix timestamp recorded on the checkpoint descriptor itself. Defaults to now.
    pub emitted_at_unix: Option<u64>,
    /// Uploaded shard references that must cover the canonical non-empty live shard set.
    pub shards: Vec<NodeProjectionCheckpointPublishShardRef>,
}

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// One uploaded shard reference used to plan or publish a projection checkpoint.
pub struct NodeProjectionCheckpointPublishShardRef {
    /// Stable resource family identifier (`accounts` or `asset_holders`).
    pub resource: String,
    /// Stable partition identifier inside the resource family.
    pub partition_id: u32,
    /// Optional asset-definition discriminator required for `asset_holders`.
    pub asset_definition_id: Option<String>,
    /// Unix timestamp that was embedded in the exported shard archive uploaded to DA.
    pub archive_emitted_at_unix: u64,
    /// Canonical digest of the uploaded DA manifest, hex-encoded.
    pub manifest_digest_hex: String,
    /// Storage ticket resolving the uploaded shard archive in DA, hex-encoded.
    pub storage_ticket_hex: String,
}

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ProjectionCheckpointShardKey {
    resource: QueryProjectionResourceKind,
    partition_id: u32,
    asset_definition_id: Option<String>,
}

#[cfg(feature = "app_api")]
impl ProjectionCheckpointShardKey {
    fn from_archive(archive: &QueryProjectionShardArchive) -> Self {
        Self {
            resource: archive.resource,
            partition_id: archive.partition_id,
            asset_definition_id: archive.asset_definition_id.clone(),
        }
    }

    fn from_catalog_entry(
        resource: QueryProjectionResourceKind,
        entry: NodeProjectionShardCatalogEntry,
    ) -> Self {
        Self {
            resource,
            partition_id: entry.partition_id,
            asset_definition_id: entry.asset_definition_id,
        }
    }

    fn describe(&self) -> String {
        match &self.asset_definition_id {
            Some(asset_definition_id) => format!(
                "{}/partition={}/asset={asset_definition_id}",
                self.resource.as_stable_str(),
                self.partition_id
            ),
            None => format!(
                "{}/partition={}",
                self.resource.as_stable_str(),
                self.partition_id
            ),
        }
    }
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Response for the live projection shard catalog of one resource family.
pub struct NodeProjectionShardCatalogResponse {
    /// Catalog payload version.
    pub version: u16,
    /// Stable resource family identifier.
    pub resource: String,
    /// Projection shard schema version covered by the listed entries.
    pub schema_version: u32,
    /// Latest block height covered by this live catalog snapshot.
    pub indexed_height: u64,
    /// Latest block hash covered by this live catalog snapshot, hex-encoded when present.
    pub indexed_block_hash_hex: Option<String>,
    /// Canonical partition count for account-scoped shard resources.
    pub default_partition_count: u32,
    /// Offset applied to the stable ordered entry set.
    pub offset: u64,
    /// Maximum number of entries returned in this page.
    pub limit: u32,
    /// Total number of matching non-empty shard entries.
    pub total_entries: u64,
    /// Offset to request the next page, when more entries remain.
    pub next_offset: Option<u64>,
    /// Stable ordered non-empty shard entries for the selected resource family.
    pub entries: Vec<NodeProjectionShardCatalogEntry>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// One stable shard entry inside the live projection catalog.
pub struct NodeProjectionShardCatalogEntry {
    /// Stable partition identifier inside the resource family.
    pub partition_id: u32,
    /// Exact logical row count for the shard.
    pub row_count: u64,
    /// Optional asset-definition discriminator for holder shards.
    pub asset_definition_id: Option<String>,
    /// Optional display alias for `asset_definition_id`.
    pub asset_alias: Option<String>,
}

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, JsonDeserialize, NoritoDeserialize)]
/// Query parameters for exporting one canonical query projection shard archive.
pub struct NodeProjectionShardExportQuery {
    /// Canonical or alias asset-definition selector required for `asset_holders`.
    pub asset_definition_id: Option<String>,
}

#[cfg(feature = "app_api")]
#[derive(Debug, Clone, JsonDeserialize, NoritoDeserialize)]
/// Query parameters for enumerating the live projection shard catalog of one resource family.
pub struct NodeProjectionShardCatalogQuery {
    /// Canonical or alias asset-definition selector used to narrow `asset_holders` entries.
    pub asset_definition_id: Option<String>,
    /// Stable entry offset within the canonical ordered catalog.
    pub offset: Option<u64>,
    /// Maximum number of entries to return.
    pub limit: Option<u32>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// JSON summary of runtime-related metrics of interest
pub struct RuntimeMetricsResponse {
    /// ABI version accepted by the runtime.
    pub abi_version: u16,
    /// Upgrade lifecycle event counters
    pub upgrade_events_total: UpgradeEventsCounters,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
pub struct UpgradeEventsCounters {
    pub proposed: u64,
    pub activated: u64,
    pub canceled: u64,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
pub struct RuntimeAbiActiveResponse {
    pub abi_version: u16,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
/// Response with the node's canonical ABI hash for the active policy.
pub struct RuntimeAbiHashResponse {
    /// Policy label (first release: always "V1").
    pub policy: String,
    /// 32-byte lowercase hex digest of the ABI surface.
    pub abi_hash_hex: String,
}

// Tests omitted to keep feature-gating friction low; behavior is trivial (hash compute) and
// exercised by doc-sync tests in the ivm crate.

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
pub struct RuntimeUpgradeListItem {
    pub id_hex: String,
    pub record: iroha_data_model::runtime::RuntimeUpgradeRecord,
}

#[derive(Debug, JsonSerialize, JsonDeserialize, NoritoSerialize, NoritoDeserialize)]
pub struct RuntimeUpgradesListResponse {
    pub items: Vec<RuntimeUpgradeListItem>,
}

/// GET /v1/runtime/abi/active
pub async fn handle_runtime_abi_active(
    state: Arc<iroha_core::state::State>,
) -> Result<RuntimeAbiActiveResponse, crate::Error> {
    let world = state.world_view();
    Ok(RuntimeAbiActiveResponse {
        abi_version: world.abi_version(),
    })
}

/// GET /v1/node/capabilities — advertise the fixed runtime ABI version and defaults
pub async fn handle_node_capabilities(
    state: Arc<iroha_core::state::State>,
) -> Result<NodeCapabilitiesResponse, crate::Error> {
    let world = state.world_view();
    let crypto_cfg = state.crypto();
    let allowed_signing: Vec<String> = crypto_cfg
        .allowed_signing
        .iter()
        .map(|algo| algo.as_static_str().to_string())
        .collect();
    let latest_projection_checkpoint = state.query_projection_checkpoint_snapshot();
    let curve_caps = summarize_curve_capabilities(&crypto_cfg);
    #[cfg(feature = "sm")]
    let (neon_sm3, neon_sm4, policy_string) = {
        let advert = iroha_crypto::sm::acceleration_advert();
        (
            advert.neon_sm3,
            advert.neon_sm4,
            advert.as_policy_str().to_string(),
        )
    };
    #[cfg(not(feature = "sm"))]
    let (neon_sm3, neon_sm4, policy_string) = (false, false, "scalar-only".to_string());
    Ok(NodeCapabilitiesResponse {
        abi_version: world.abi_version(),
        data_model_version: iroha_data_model::DATA_MODEL_VERSION,
        signed_transaction_schema_hash_hex: signed_transaction_schema_hash_hex(),
        crypto: NodeCryptoCapabilities {
            sm: NodeSmCapabilities {
                enabled: crypto_cfg.sm_helpers_enabled(),
                default_hash: crypto_cfg.default_hash.clone(),
                allowed_signing,
                sm2_distid_default: crypto_cfg.sm2_distid_default.clone(),
                openssl_preview: crypto_cfg.enable_sm_openssl_preview,
                acceleration: NodeSmAcceleration {
                    scalar: true,
                    neon_sm3,
                    neon_sm4,
                    policy: policy_string,
                },
            },
            curves: curve_caps,
        },
        query: NodeQueryCapabilities {
            aggregate: NodeAggregateQueryCapabilities {
                v1: true,
                exact_results: true,
                supported_resources: vec!["accounts".to_string(), "asset_holders".to_string()],
            },
            indexed_snapshot_marker: true,
            row_enrichment_fields: vec![
                "primary_alias".to_string(),
                "primary_alias_name".to_string(),
                "primary_alias_dataspace".to_string(),
                "primary_alias_domain".to_string(),
                "has_primary_alias".to_string(),
            ],
            projection: NodeProjectionCapabilities {
                checkpoint_contract_v1: true,
                da_v1_enabled: false,
                checkpoint_plan_v1: cfg!(feature = "app_api"),
                checkpoint_publish_v1: cfg!(feature = "app_api"),
                shard_catalog_v1: cfg!(feature = "app_api"),
                archive_export_v1: cfg!(feature = "app_api"),
                archive_version: QUERY_PROJECTION_SHARD_ARCHIVE_VERSION,
                schema_version: QUERY_PROJECTION_SCHEMA_VERSION,
                blob_class_custom_id: QUERY_PROJECTION_DA_BLOB_CLASS_CUSTOM_ID,
                codec: QUERY_PROJECTION_DA_CODEC.to_string(),
                rowset_codec: QUERY_PROJECTION_SHARD_ROWSET_CODEC.to_string(),
                compression: compression_name(QUERY_PROJECTION_DA_COMPRESSION).to_string(),
                default_partition_count: QUERY_PROJECTION_DEFAULT_PARTITION_COUNT,
                metadata_keys: vec![
                    QUERY_PROJECTION_METADATA_LOCATOR_KEY.to_string(),
                    QUERY_PROJECTION_METADATA_RESOURCE_KEY.to_string(),
                    QUERY_PROJECTION_METADATA_PARTITION_KEY.to_string(),
                    QUERY_PROJECTION_METADATA_ASSET_KEY.to_string(),
                    QUERY_PROJECTION_METADATA_HEIGHT_KEY.to_string(),
                    QUERY_PROJECTION_METADATA_BLOCK_HASH_KEY.to_string(),
                    QUERY_PROJECTION_METADATA_ROW_COUNT_KEY.to_string(),
                    QUERY_PROJECTION_METADATA_ROWSET_CODEC_KEY.to_string(),
                    QUERY_PROJECTION_METADATA_ROWSET_HASH_KEY.to_string(),
                    QUERY_PROJECTION_METADATA_EMITTED_AT_KEY.to_string(),
                ],
                export_supported_resources: if cfg!(feature = "app_api") {
                    vec!["accounts".to_string(), "asset_holders".to_string()]
                } else {
                    Vec::new()
                },
                latest_checkpoint_indexed_height: latest_projection_checkpoint
                    .as_ref()
                    .map(|checkpoint| checkpoint.indexed_height),
                latest_checkpoint_block_hash_hex: latest_projection_checkpoint
                    .as_ref()
                    .and_then(|checkpoint| checkpoint.indexed_block_hash)
                    .map(|hash| hex::encode(hash.as_ref())),
            },
        },
    })
}

/// GET /v1/node/query/projection/checkpoint — return the latest persisted checkpoint descriptor.
#[must_use]
pub async fn handle_node_query_projection_checkpoint(
    state: Arc<iroha_core::state::State>,
) -> Option<NodeProjectionCheckpointResponse> {
    state
        .query_projection_checkpoint_snapshot()
        .map(node_projection_checkpoint_response)
}

#[cfg(feature = "app_api")]
/// POST /v1/node/query/projection/checkpoint/plan — validate uploaded shard refs and build a checkpoint.
pub async fn handle_node_query_projection_checkpoint_plan(
    state: Arc<iroha_core::state::State>,
    request: NodeProjectionCheckpointPublishRequest,
) -> Result<NodeProjectionCheckpointResponse, crate::Error> {
    let emitted_at_unix = request.emitted_at_unix.unwrap_or_else(current_unix_seconds);
    let uploads = build_query_projection_uploaded_archives(state.as_ref(), request.shards)?;
    validate_projection_checkpoint_shard_set(state.as_ref(), &uploads)?;
    let plan = state
        .plan_query_projection_checkpoint_from_archives(emitted_at_unix, uploads)
        .map_err(projection_checkpoint_plan_error)?;
    Ok(node_projection_checkpoint_response(plan.into_checkpoint()))
}

#[cfg(feature = "app_api")]
/// POST /v1/node/query/projection/checkpoint/publish — rebuild uploaded shard refs and persist the checkpoint.
pub async fn handle_node_query_projection_checkpoint_publish(
    state: Arc<iroha_core::state::State>,
    request: NodeProjectionCheckpointPublishRequest,
) -> Result<NodeProjectionCheckpointResponse, crate::Error> {
    let emitted_at_unix = request.emitted_at_unix.unwrap_or_else(current_unix_seconds);
    let uploads = build_query_projection_uploaded_archives(state.as_ref(), request.shards)?;
    validate_projection_checkpoint_shard_set(state.as_ref(), &uploads)?;
    let checkpoint = state
        .publish_query_projection_checkpoint_from_archives(
            emitted_at_unix,
            uploads.into_iter().map(|upload| {
                (
                    upload.archive,
                    upload.manifest_digest,
                    upload.storage_ticket,
                )
            }),
        )
        .map_err(projection_checkpoint_plan_error)?;
    Ok(node_projection_checkpoint_response(checkpoint))
}

fn node_projection_checkpoint_response(
    checkpoint: QueryProjectionCheckpoint,
) -> NodeProjectionCheckpointResponse {
    NodeProjectionCheckpointResponse {
        version: checkpoint.version,
        schema_version: checkpoint.schema_version,
        blob_class: blob_class_name(checkpoint.blob_class).to_string(),
        blob_class_custom_id: blob_class_custom_id(checkpoint.blob_class),
        codec: checkpoint.codec.0,
        compression: compression_name(checkpoint.compression).to_string(),
        indexed_height: checkpoint.indexed_height,
        indexed_block_hash_hex: checkpoint
            .indexed_block_hash
            .map(|hash| hex::encode(hash.as_ref())),
        emitted_at_unix: checkpoint.emitted_at_unix,
        shards: checkpoint
            .shards
            .into_iter()
            .map(|shard| NodeProjectionCheckpointShardRef {
                resource: shard.resource.as_stable_str().to_string(),
                partition_id: shard.partition_id,
                asset_definition_id: shard.asset_definition_id,
                manifest_digest_hex: hex::encode(shard.manifest_digest.as_bytes()),
                storage_ticket_hex: hex::encode(shard.storage_ticket.as_bytes()),
                blob_hash_hex: hex::encode(shard.blob_hash.as_bytes()),
            })
            .collect(),
    }
}

#[cfg(feature = "app_api")]
fn build_query_projection_uploaded_archives(
    state: &iroha_core::state::State,
    shards: Vec<NodeProjectionCheckpointPublishShardRef>,
) -> Result<Vec<QueryProjectionUploadedShardArchive>, crate::Error> {
    let mut uploads = Vec::with_capacity(shards.len());

    for shard in shards {
        validate_projection_partition_id(shard.partition_id)?;
        let manifest_digest =
            parse_blob_digest_hex(&shard.manifest_digest_hex, "manifest_digest_hex")?;
        let storage_ticket =
            parse_storage_ticket_hex(&shard.storage_ticket_hex, "storage_ticket_hex")?;

        let archive = match shard.resource.trim().to_ascii_lowercase().as_str() {
            "accounts" => {
                if let Some(asset_definition_id) = shard
                    .asset_definition_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    return Err(projection_export_conversion_error(format!(
                        "asset_definition_id is not supported for accounts checkpoint shards (got `{asset_definition_id}`)"
                    )));
                }
                build_accounts_projection_shard_archive(
                    state,
                    shard.partition_id,
                    shard.archive_emitted_at_unix,
                )?
            }
            "asset_holders" => {
                let selector = shard
                    .asset_definition_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        projection_export_conversion_error(
                            "asset_definition_id is required for asset_holders checkpoint shards"
                                .to_owned(),
                        )
                    })?;
                build_asset_holders_projection_shard_archive(
                    state,
                    selector,
                    shard.partition_id,
                    shard.archive_emitted_at_unix,
                )?
            }
            other => {
                return Err(projection_export_conversion_error(format!(
                    "unsupported checkpoint shard resource `{other}`; expected `accounts` or `asset_holders`"
                )));
            }
        };

        uploads.push(QueryProjectionUploadedShardArchive::new(
            archive,
            manifest_digest,
            storage_ticket,
        ));
    }

    Ok(uploads)
}

#[cfg(feature = "app_api")]
fn validate_projection_checkpoint_shard_set(
    state: &iroha_core::state::State,
    uploads: &[QueryProjectionUploadedShardArchive],
) -> Result<(), crate::Error> {
    let expected = build_expected_projection_checkpoint_shard_keys(state)?;
    let actual: BTreeSet<_> = uploads
        .iter()
        .map(|upload| ProjectionCheckpointShardKey::from_archive(&upload.archive))
        .collect();

    let missing_count = expected.difference(&actual).count();
    let unexpected_count = actual.difference(&expected).count();
    if missing_count == 0 && unexpected_count == 0 {
        return Ok(());
    }

    let missing_preview = expected
        .difference(&actual)
        .take(8)
        .map(ProjectionCheckpointShardKey::describe)
        .collect::<Vec<_>>();
    let unexpected_preview = actual
        .difference(&expected)
        .take(8)
        .map(ProjectionCheckpointShardKey::describe)
        .collect::<Vec<_>>();

    let mut problems = Vec::new();
    if missing_count > 0 {
        problems.push(format!(
            "missing {missing_count} shard(s): {}{}",
            missing_preview.join(", "),
            preview_suffix(missing_count, missing_preview.len())
        ));
    }
    if unexpected_count > 0 {
        problems.push(format!(
            "unexpected {unexpected_count} shard(s): {}{}",
            unexpected_preview.join(", "),
            preview_suffix(unexpected_count, unexpected_preview.len())
        ));
    }

    Err(projection_export_conversion_error(format!(
        "checkpoint shard set must match the canonical live shard catalog; {}",
        problems.join("; ")
    )))
}

#[cfg(feature = "app_api")]
fn build_expected_projection_checkpoint_shard_keys(
    state: &iroha_core::state::State,
) -> Result<BTreeSet<ProjectionCheckpointShardKey>, crate::Error> {
    let mut expected = BTreeSet::new();
    expected.extend(
        build_accounts_projection_shard_catalog_entries(state)
            .into_iter()
            .map(|entry| {
                ProjectionCheckpointShardKey::from_catalog_entry(
                    QueryProjectionResourceKind::Accounts,
                    entry,
                )
            }),
    );
    expected.extend(
        build_asset_holders_projection_shard_catalog_entries(state, None)?
            .into_iter()
            .map(|entry| {
                ProjectionCheckpointShardKey::from_catalog_entry(
                    QueryProjectionResourceKind::AssetHolders,
                    entry,
                )
            }),
    );
    Ok(expected)
}

#[cfg(feature = "app_api")]
fn preview_suffix(total: usize, shown: usize) -> String {
    let remaining = total.saturating_sub(shown);
    if remaining == 0 {
        String::new()
    } else {
        format!(" (+{remaining} more)")
    }
}

/// GET /v1/node/query/projection/catalog/{resource} — enumerate the canonical live shard set.
#[cfg(feature = "app_api")]
pub async fn handle_node_query_projection_shard_catalog(
    state: Arc<iroha_core::state::State>,
    resource: String,
    query: NodeProjectionShardCatalogQuery,
) -> Result<NodeProjectionShardCatalogResponse, crate::Error> {
    let index_status = state.query_index_status_snapshot();

    match resource.trim().to_ascii_lowercase().as_str() {
        "accounts" => {
            if let Some(asset_definition_id) = query
                .asset_definition_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Err(projection_export_conversion_error(format!(
                    "asset_definition_id is not supported for accounts catalog (got `{asset_definition_id}`)"
                )));
            }
            build_projection_shard_catalog_response(
                QueryProjectionResourceKind::Accounts,
                index_status.indexed_height,
                index_status
                    .indexed_block_hash
                    .map(|hash| hex::encode(hash.as_ref())),
                query.offset,
                query.limit,
                build_accounts_projection_shard_catalog_entries(state.as_ref()),
            )
        }
        "asset_holders" => build_projection_shard_catalog_response(
            QueryProjectionResourceKind::AssetHolders,
            index_status.indexed_height,
            index_status
                .indexed_block_hash
                .map(|hash| hex::encode(hash.as_ref())),
            query.offset,
            query.limit,
            build_asset_holders_projection_shard_catalog_entries(
                state.as_ref(),
                query.asset_definition_id.as_deref(),
            )?,
        ),
        other => Err(projection_export_conversion_error(format!(
            "unsupported projection resource `{other}`; expected `accounts` or `asset_holders`"
        ))),
    }
}

/// GET /v1/node/query/projection/shards/{resource}/{partition_id} — export one canonical shard archive.
#[cfg(feature = "app_api")]
pub async fn handle_node_query_projection_shard_export(
    state: Arc<iroha_core::state::State>,
    resource: String,
    partition_id: u32,
    query: NodeProjectionShardExportQuery,
) -> Result<QueryProjectionShardArchive, crate::Error> {
    validate_projection_partition_id(partition_id)?;

    let emitted_at_unix = current_unix_seconds();
    match resource.trim().to_ascii_lowercase().as_str() {
        "accounts" => {
            if let Some(asset_definition_id) = query
                .asset_definition_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                return Err(projection_export_conversion_error(format!(
                    "asset_definition_id is not supported for accounts export (got `{asset_definition_id}`)"
                )));
            }
            build_accounts_projection_shard_archive(state.as_ref(), partition_id, emitted_at_unix)
        }
        "asset_holders" => {
            let selector = query
                .asset_definition_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    projection_export_conversion_error(
                        "asset_definition_id is required for asset_holders export".to_owned(),
                    )
                })?;
            build_asset_holders_projection_shard_archive(
                state.as_ref(),
                selector,
                partition_id,
                emitted_at_unix,
            )
        }
        other => Err(projection_export_conversion_error(format!(
            "unsupported projection resource `{other}`; expected `accounts` or `asset_holders`"
        ))),
    }
}

#[cfg(feature = "app_api")]
fn build_projection_shard_catalog_response(
    resource: QueryProjectionResourceKind,
    indexed_height: u64,
    indexed_block_hash_hex: Option<String>,
    offset: Option<u64>,
    limit: Option<u32>,
    mut entries: Vec<NodeProjectionShardCatalogEntry>,
) -> Result<NodeProjectionShardCatalogResponse, crate::Error> {
    let offset = offset.unwrap_or(0);
    let limit = match limit.unwrap_or(QUERY_PROJECTION_SHARD_CATALOG_DEFAULT_LIMIT) {
        0 => {
            return Err(projection_export_conversion_error(
                "limit must be greater than zero".to_owned(),
            ));
        }
        limit if limit > QUERY_PROJECTION_SHARD_CATALOG_MAX_LIMIT => {
            return Err(projection_export_conversion_error(format!(
                "limit must be less than or equal to {QUERY_PROJECTION_SHARD_CATALOG_MAX_LIMIT}"
            )));
        }
        limit => limit,
    };

    let total_entries = entries.len() as u64;
    let start = usize::try_from(offset.min(total_entries)).unwrap_or(entries.len());
    let end = start
        .saturating_add(usize::try_from(limit).unwrap_or(usize::MAX))
        .min(entries.len());
    let next_offset = (end < entries.len()).then_some(end as u64);
    entries = entries
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect();

    Ok(NodeProjectionShardCatalogResponse {
        version: QUERY_PROJECTION_SHARD_CATALOG_VERSION,
        resource: resource.as_stable_str().to_string(),
        schema_version: QUERY_PROJECTION_SCHEMA_VERSION,
        indexed_height,
        indexed_block_hash_hex,
        default_partition_count: QUERY_PROJECTION_DEFAULT_PARTITION_COUNT,
        offset,
        limit,
        total_entries,
        next_offset,
        entries,
    })
}

#[cfg(feature = "app_api")]
fn build_accounts_projection_shard_catalog_entries(
    state: &iroha_core::state::State,
) -> Vec<NodeProjectionShardCatalogEntry> {
    let world = state.world_view();
    let accounts = crate::routing::collect_subject_accounts(&world);
    drop(world);

    let mut counts: BTreeMap<u32, u64> = BTreeMap::new();
    for account in accounts {
        let partition_id =
            query_projection_default_partition_for_account(&account.id().to_string());
        *counts.entry(partition_id).or_default() += 1;
    }

    counts
        .into_iter()
        .map(
            |(partition_id, row_count)| NodeProjectionShardCatalogEntry {
                partition_id,
                row_count,
                asset_definition_id: None,
                asset_alias: None,
            },
        )
        .collect()
}

#[cfg(feature = "app_api")]
fn build_accounts_projection_shard_archive(
    state: &iroha_core::state::State,
    partition_id: u32,
    emitted_at_unix: u64,
) -> Result<QueryProjectionShardArchive, crate::Error> {
    let world = state.world_view();
    let accounts = crate::routing::collect_subject_accounts(&world);
    drop(world);

    let mut rows = Vec::new();
    for account in accounts {
        let account_id = account.id().to_string();
        if query_projection_default_partition_for_account(&account_id) != partition_id {
            continue;
        }
        let alias = crate::routing::primary_alias_projection_for_account_id(state, account.id());
        rows.push(QueryProjectionAccountRow {
            account_id,
            primary_alias: alias.literal,
            primary_alias_name: alias.name,
            primary_alias_dataspace: alias.dataspace,
            primary_alias_domain: alias.domain,
            has_primary_alias: alias.has_primary_alias,
        });
    }
    rows.sort_by(|left, right| left.account_id.cmp(&right.account_id));

    let rowset = QueryProjectionShardRowSet::Accounts(QueryProjectionAccountsShardRowSet::new(
        partition_id,
        rows,
    ));
    let row_count = rowset.row_count();
    let payload = rowset.encode_payload().map_err(|err| {
        projection_export_conversion_error(format!(
            "failed to encode accounts projection shard rowset: {err}"
        ))
    })?;

    Ok(QueryProjectionShardArchive::from_index_status(
        state.query_index_status_snapshot(),
        emitted_at_unix,
        QueryProjectionResourceKind::Accounts,
        partition_id,
        None,
        row_count,
        payload,
    ))
}

#[cfg(feature = "app_api")]
fn build_asset_holders_projection_shard_catalog_entries(
    state: &iroha_core::state::State,
    asset_definition_selector: Option<&str>,
) -> Result<Vec<NodeProjectionShardCatalogEntry>, crate::Error> {
    let now_ms = crate::routing::asset_alias_observation_time_ms(state);
    let world = state.world_view();
    let selected_definition_id = asset_definition_selector
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|selector| crate::routing::resolve_asset_definition_selector(&world, selector, now_ms))
        .transpose()?;

    let mut counts: BTreeMap<(iroha_data_model::asset::AssetDefinitionId, u32), u64> =
        BTreeMap::new();
    match selected_definition_id {
        Some(definition_id) => {
            for asset in world.assets_by_definition_iter(&definition_id) {
                let partition_id = query_projection_default_partition_for_account(
                    &asset.id().account().to_string(),
                );
                *counts
                    .entry((definition_id.clone(), partition_id))
                    .or_default() += 1;
            }
        }
        None => {
            for asset in world.assets_iter() {
                let partition_id = query_projection_default_partition_for_account(
                    &asset.id().account().to_string(),
                );
                *counts
                    .entry((asset.id().definition().clone(), partition_id))
                    .or_default() += 1;
            }
        }
    }

    let mut entries = Vec::with_capacity(counts.len());
    for ((definition_id, partition_id), row_count) in counts {
        let asset_alias = world
            .asset_definition(&definition_id)
            .ok()
            .and_then(|definition| definition.alias().as_ref().map(ToString::to_string));
        entries.push(NodeProjectionShardCatalogEntry {
            partition_id,
            row_count,
            asset_definition_id: Some(definition_id.to_string()),
            asset_alias,
        });
    }
    drop(world);

    Ok(entries)
}

#[cfg(feature = "app_api")]
fn build_asset_holders_projection_shard_archive(
    state: &iroha_core::state::State,
    asset_definition_selector: &str,
    partition_id: u32,
    emitted_at_unix: u64,
) -> Result<QueryProjectionShardArchive, crate::Error> {
    let now_ms = crate::routing::asset_alias_observation_time_ms(state);
    let world = state.world_view();
    let definition_id = crate::routing::resolve_asset_definition_selector(
        &world,
        asset_definition_selector,
        now_ms,
    )?;
    let asset_alias = world
        .asset_definition(&definition_id)
        .ok()
        .and_then(|definition| definition.alias().as_ref().map(ToString::to_string));

    let mut aggregated: BTreeMap<
        (
            iroha_data_model::account::AccountId,
            iroha_data_model::asset::AssetBalanceScope,
        ),
        iroha_primitives::numeric::Numeric,
    > = BTreeMap::new();
    for asset in world.assets_by_definition_iter(&definition_id) {
        let account_id = asset.id().account().clone();
        let scope = asset.id().scope().clone();
        let entry = aggregated
            .entry((account_id, scope))
            .or_insert_with(iroha_primitives::numeric::Numeric::zero);
        if let Some(sum) = entry.clone().checked_add(asset.value().clone()) {
            *entry = sum;
        }
    }
    let alias_cache: BTreeMap<_, _> = aggregated
        .keys()
        .map(|(account_id, _)| {
            (
                account_id.clone(),
                crate::routing::primary_alias_projection_for_account_id(state, account_id),
            )
        })
        .collect();
    drop(world);

    let mut rows = Vec::new();
    for ((account_id, scope), quantity) in aggregated {
        let canonical_id = account_id.to_string();
        if query_projection_default_partition_for_account(&canonical_id) != partition_id {
            continue;
        }
        let alias = alias_cache.get(&account_id).cloned().unwrap_or_default();
        rows.push(QueryProjectionAssetHolderRow {
            account_id: canonical_id,
            scope: crate::routing::asset_balance_scope_literal(&scope),
            quantity,
            primary_alias: alias.literal,
            primary_alias_name: alias.name,
            primary_alias_dataspace: alias.dataspace,
            primary_alias_domain: alias.domain,
            has_primary_alias: alias.has_primary_alias,
        });
    }
    rows.sort_by(|left, right| {
        left.account_id
            .cmp(&right.account_id)
            .then(left.scope.cmp(&right.scope))
    });

    let rowset =
        QueryProjectionShardRowSet::AssetHolders(QueryProjectionAssetHoldersShardRowSet::new(
            partition_id,
            definition_id.to_string(),
            asset_alias,
            rows,
        ));
    let row_count = rowset.row_count();
    let payload = rowset.encode_payload().map_err(|err| {
        projection_export_conversion_error(format!(
            "failed to encode asset_holders projection shard rowset: {err}"
        ))
    })?;

    Ok(QueryProjectionShardArchive::from_index_status(
        state.query_index_status_snapshot(),
        emitted_at_unix,
        QueryProjectionResourceKind::AssetHolders,
        partition_id,
        rowset.asset_definition_id().map(ToOwned::to_owned),
        row_count,
        payload,
    ))
}

#[cfg(feature = "app_api")]
fn projection_export_conversion_error(message: impl Into<String>) -> crate::Error {
    crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::Conversion(message.into()),
    ))
}

#[cfg(feature = "app_api")]
fn projection_checkpoint_plan_error(err: QueryProjectionCheckpointPlanError) -> crate::Error {
    projection_export_conversion_error(format!(
        "failed to validate query projection checkpoint publish plan: {err}"
    ))
}

#[cfg(feature = "app_api")]
fn validate_projection_partition_id(partition_id: u32) -> Result<(), crate::Error> {
    if partition_id >= QUERY_PROJECTION_DEFAULT_PARTITION_COUNT {
        return Err(projection_export_conversion_error(format!(
            "partition_id must be less than {}",
            QUERY_PROJECTION_DEFAULT_PARTITION_COUNT
        )));
    }
    Ok(())
}

#[cfg(feature = "app_api")]
fn parse_blob_digest_hex(value: &str, field: &str) -> Result<BlobDigest, crate::Error> {
    parse_fixed_hex_32(value, field).map(BlobDigest::new)
}

#[cfg(feature = "app_api")]
fn parse_storage_ticket_hex(value: &str, field: &str) -> Result<StorageTicketId, crate::Error> {
    parse_fixed_hex_32(value, field).map(StorageTicketId::new)
}

#[cfg(feature = "app_api")]
fn parse_fixed_hex_32(value: &str, field: &str) -> Result<[u8; 32], crate::Error> {
    let trimmed = value.trim_start_matches("0x");
    let bytes = hex::decode(trimmed).map_err(|err| {
        projection_export_conversion_error(format!(
            "invalid hex in `{field}` (expected 32 bytes): {err}"
        ))
    })?;
    let len = bytes.len();
    bytes.try_into().map_err(|_| {
        projection_export_conversion_error(format!("`{field}` must be 32 bytes (got {len})"))
    })
}

#[cfg(feature = "app_api")]
fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn signed_transaction_schema_hash_hex() -> String {
    hex::encode(<SignedTransaction as norito::core::NoritoSerialize>::schema_hash())
}

fn summarize_curve_capabilities(
    crypto: &iroha_config::parameters::actual::Crypto,
) -> NodeCurveCapabilities {
    let mut ids = crypto.allowed_curve_ids.clone();
    if ids.is_empty() {
        ids = iroha_config::parameters::defaults::crypto::derive_curve_ids_from_algorithms(
            &crypto.allowed_signing,
        );
    }
    if ids.is_empty() {
        warn!(
            target: "iroha_torii::runtime",
            "allowed_curve_ids resolved to an empty list; defaulting to ed25519-only advert"
        );
        ids.push(CurveId::ED25519.as_u8());
    }
    ids.sort_unstable();
    ids.dedup();
    let bitmap = curve_bitmap_from_ids(&ids);
    NodeCurveCapabilities {
        registry_version: CURVE_REGISTRY_VERSION,
        allowed_curve_ids: ids,
        allowed_curve_bitmap: bitmap,
    }
}

fn curve_bitmap_from_ids(ids: &[u8]) -> Vec<u64> {
    if ids.is_empty() {
        return Vec::new();
    }
    // 256 identifier slots => four 64-bit lanes; trim trailing zeros later.
    let mut lanes = [0u64; 4];
    for &id in ids {
        let lane = (id / 64) as usize;
        let offset = id % 64;
        if let Some(slot) = lanes.get_mut(lane) {
            *slot |= 1u64 << offset;
        }
    }
    let mut vec = lanes.to_vec();
    while vec.len() > 1 && matches!(vec.last(), Some(&0)) {
        vec.pop();
    }
    if vec.len() == 1 && vec[0] == 0 {
        vec.clear();
    }
    vec
}

fn compression_name(compression: Compression) -> &'static str {
    match compression {
        Compression::Identity => "identity",
        Compression::Gzip => "gzip",
        Compression::Deflate => "deflate",
        Compression::Zstd => "zstd",
    }
}

fn blob_class_name(blob_class: BlobClass) -> &'static str {
    match blob_class {
        BlobClass::TaikaiSegment => "taikai_segment",
        BlobClass::NexusLaneSidecar => "nexus_lane_sidecar",
        BlobClass::GovernanceArtifact => "governance_artifact",
        BlobClass::Custom(_) => "custom",
    }
}

fn blob_class_custom_id(blob_class: BlobClass) -> Option<u16> {
    match blob_class {
        BlobClass::Custom(id) => Some(id),
        _ => None,
    }
}

#[cfg(test)]
mod bitmap_tests {
    use super::curve_bitmap_from_ids;

    #[test]
    fn bitmap_handles_edges() {
        assert!(curve_bitmap_from_ids(&[]).is_empty());
        assert_eq!(curve_bitmap_from_ids(&[0]), vec![1]);
        assert_eq!(
            curve_bitmap_from_ids(&[1, 63]),
            vec![(1u64 << 1) | (1u64 << 63)]
        );
        // Highest identifier should land in the final lane.
        let mut ids = vec![255];
        let bitmap = curve_bitmap_from_ids(&ids);
        assert_eq!(bitmap.len(), 4);
        assert_eq!(bitmap[3], 1u64 << 63);
        assert_eq!(bitmap[0..3], [0, 0, 0]);
        // Multiple lanes retain intermediate zeros.
        ids.extend([0, 128]);
        let bitmap = curve_bitmap_from_ids(&ids);
        assert_eq!(bitmap, vec![1, 0, 1, 1u64 << 63]);
    }
}

/// GET /v1/runtime/metrics — expose runtime metrics summary
pub async fn handle_runtime_metrics(
    state: Arc<iroha_core::state::State>,
) -> Result<RuntimeMetricsResponse, crate::Error> {
    let world = state.world_view();
    let mut proposed: u64 = 0;
    let mut activated: u64 = 0;
    let mut canceled: u64 = 0;
    for (_id, rec) in world.runtime_upgrades().iter() {
        proposed = proposed.saturating_add(1);
        match rec.status {
            iroha_data_model::runtime::RuntimeUpgradeStatus::ActivatedAt(_) => {
                activated = activated.saturating_add(1)
            }
            iroha_data_model::runtime::RuntimeUpgradeStatus::Canceled => {
                canceled = canceled.saturating_add(1)
            }
            _ => {}
        }
    }
    Ok(RuntimeMetricsResponse {
        abi_version: world.abi_version(),
        upgrade_events_total: UpgradeEventsCounters {
            proposed,
            activated,
            canceled,
        },
    })
}

/// GET /v1/runtime/abi/hash — return the canonical ABI hash for the node's active policy.
pub async fn handle_runtime_abi_hash(
    _state: Arc<iroha_core::state::State>,
) -> Result<RuntimeAbiHashResponse, crate::Error> {
    // First release: single policy V1
    let h = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    Ok(RuntimeAbiHashResponse {
        policy: "V1".to_string(),
        abi_hash_hex: hex::encode::<[u8; 32]>(h),
    })
}

/// GET /v1/runtime/upgrades
///
/// # Errors
/// Returns an error if the state view cannot be acquired or serialized.
pub async fn handle_runtime_upgrades_list(
    state: Arc<iroha_core::state::State>,
) -> Result<RuntimeUpgradesListResponse, crate::Error> {
    let world = state.world_view();
    let mut items: Vec<RuntimeUpgradeListItem> = Vec::new();
    for (id, rec) in world.runtime_upgrades().iter() {
        items.push(RuntimeUpgradeListItem {
            id_hex: hex::encode(id.0),
            record: rec.clone(),
        });
    }
    // Stable sort: by start_height then abi_version
    items.sort_by_key(|it| {
        (
            it.record.manifest.start_height,
            it.record.manifest.abi_version,
        )
    });
    Ok(RuntimeUpgradesListResponse { items })
}

#[derive(Debug, JsonDeserialize, NoritoDeserialize)]
pub struct ProposeUpgradeDto(pub iroha_data_model::runtime::RuntimeUpgradeManifest);

#[derive(Debug, JsonSerialize, NoritoSerialize)]
pub struct TxInstr {
    pub wire_id: String,
    pub payload_hex: String,
}

fn instruction_box_to_tx_instr(boxed: iroha_data_model::isi::InstructionBox) -> TxInstr {
    use iroha_data_model::isi::Instruction;

    let wire_id = Instruction::id(&*boxed).to_string();
    let payload = Instruction::dyn_encode(&*boxed);
    TxInstr {
        wire_id,
        payload_hex: hex::encode(payload),
    }
}

#[derive(Debug, JsonSerialize, NoritoSerialize)]
pub struct ProposeUpgradeResponse {
    pub ok: bool,
    pub tx_instructions: Vec<TxInstr>,
}

/// POST /v1/runtime/upgrades/propose
pub async fn handle_runtime_propose_upgrade(
    NoritoJson(ProposeUpgradeDto(manifest)): NoritoJson<ProposeUpgradeDto>,
) -> Result<ProposeUpgradeResponse, crate::Error> {
    let manifest_bytes = norito::to_bytes(&manifest).map_err(|e| {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "encode manifest: {e}"
            )),
        ))
    })?;
    let isi = iroha_data_model::isi::runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes };
    let boxed: iroha_data_model::isi::InstructionBox = isi.into();
    let tx_instructions = vec![instruction_box_to_tx_instr(boxed)];
    Ok(ProposeUpgradeResponse {
        ok: true,
        tx_instructions,
    })
}

#[derive(Debug, JsonSerialize, NoritoSerialize)]
/// Response payload describing the outcome of runtime activation/cancellation helpers.
pub struct ActivateCancelResponse {
    /// Indicates whether the operation succeeded.
    pub ok: bool,
    /// Instructions (if any) that must be signed and submitted by the caller.
    pub tx_instructions: Vec<TxInstr>,
}

impl IntoResponse for ActivateCancelResponse {
    fn into_response(self) -> axum::response::Response {
        crate::JsonBody(self).into_response()
    }
}

/// POST /v1/runtime/upgrades/activate/{id}
///
/// # Errors
/// Returns an error when the provided upgrade identifier is malformed or activation fails.
pub async fn handle_runtime_activate_upgrade(
    Path(id): Path<String>,
) -> Result<ActivateCancelResponse, crate::Error> {
    let s = id.trim_start_matches("0x");
    let bytes = hex::decode(s).map_err(|_| {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion("invalid id".into()),
        ))
    })?;
    if bytes.len() != 32 {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid id length".into(),
                ),
            ),
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    let isi = iroha_data_model::isi::runtime_upgrade::ActivateRuntimeUpgrade {
        id: iroha_data_model::runtime::RuntimeUpgradeId(arr),
    };
    let boxed: iroha_data_model::isi::InstructionBox = isi.into();
    let tx_instructions = vec![instruction_box_to_tx_instr(boxed)];
    Ok(ActivateCancelResponse {
        ok: true,
        tx_instructions,
    })
}

/// POST /v1/runtime/upgrades/cancel/{id}
///
/// # Errors
/// Returns an error when the identifier cannot be decoded or cancellation fails.
pub async fn handle_runtime_cancel_upgrade(
    Path(id): Path<String>,
) -> Result<ActivateCancelResponse, crate::Error> {
    let s = id.trim_start_matches("0x");
    let bytes = hex::decode(s).map_err(|_| {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion("invalid id".into()),
        ))
    })?;
    if bytes.len() != 32 {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid id length".into(),
                ),
            ),
        ));
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    let isi = iroha_data_model::isi::runtime_upgrade::CancelRuntimeUpgrade {
        id: iroha_data_model::runtime::RuntimeUpgradeId(arr),
    };
    let boxed: iroha_data_model::isi::InstructionBox = isi.into();
    let tx_instructions = vec![instruction_box_to_tx_instr(boxed)];
    Ok(ActivateCancelResponse {
        ok: true,
        tx_instructions,
    })
}

#[cfg(test)]
mod tests {
    use iroha_core::{kura::Kura, query::store::LiveQueryStore, state::State};

    use super::*;

    #[cfg(feature = "app_api")]
    fn accounts_checkpoint_request_for_state(
        state: &std::sync::Arc<State>,
        emitted_at_unix: u64,
        archive_emitted_at_unix: u64,
        manifest_seed: u8,
        ticket_seed: u8,
    ) -> NodeProjectionCheckpointPublishRequest {
        let shards = build_accounts_projection_shard_catalog_entries(state.as_ref())
            .into_iter()
            .enumerate()
            .map(|(index, entry)| NodeProjectionCheckpointPublishShardRef {
                resource: "accounts".to_string(),
                partition_id: entry.partition_id,
                asset_definition_id: None,
                archive_emitted_at_unix,
                manifest_digest_hex: hex::encode([manifest_seed.wrapping_add(index as u8); 32]),
                storage_ticket_hex: hex::encode([ticket_seed.wrapping_add(index as u8); 32]),
            })
            .collect();
        NodeProjectionCheckpointPublishRequest {
            emitted_at_unix: Some(emitted_at_unix),
            shards,
        }
    }

    #[tokio::test]
    async fn runtime_abi_hash_matches_ivm() {
        // Build a minimal state (not used by the handler, but required by signature)
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let world = iroha_core::state::World::default();
        let state = State::new_for_testing(world, kura, query_handle);

        let resp = handle_runtime_abi_hash(std::sync::Arc::new(state))
            .await
            .expect("ok");
        assert_eq!(resp.policy, "V1");
        // Expected hex length for 32 bytes
        assert_eq!(resp.abi_hash_hex.len(), 64);
        let expected = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        assert_eq!(resp.abi_hash_hex, hex::encode::<[u8; 32]>(expected));
    }

    #[tokio::test]
    async fn node_capabilities_reports_v1() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let world = iroha_core::state::World::default();
        let state = State::new_for_testing(world, kura, query_handle);

        let resp = handle_node_capabilities(std::sync::Arc::new(state))
            .await
            .expect("ok");
        assert_eq!(resp.abi_version, 1);
        assert_eq!(
            resp.data_model_version,
            iroha_data_model::DATA_MODEL_VERSION
        );
        assert_eq!(resp.signed_transaction_schema_hash_hex.len(), 32);
        assert_eq!(
            resp.signed_transaction_schema_hash_hex,
            signed_transaction_schema_hash_hex()
        );
        assert_eq!(resp.crypto.curves.registry_version, CURVE_REGISTRY_VERSION);
        assert!(resp.query.aggregate.v1);
        assert!(resp.query.aggregate.exact_results);
        assert_eq!(
            resp.query.aggregate.supported_resources,
            vec!["accounts".to_string(), "asset_holders".to_string()]
        );
        assert!(resp.query.indexed_snapshot_marker);
        assert_eq!(
            resp.query.row_enrichment_fields,
            vec![
                "primary_alias".to_string(),
                "primary_alias_name".to_string(),
                "primary_alias_dataspace".to_string(),
                "primary_alias_domain".to_string(),
                "has_primary_alias".to_string()
            ]
        );
        assert!(resp.query.projection.checkpoint_contract_v1);
        assert!(!resp.query.projection.da_v1_enabled);
        assert_eq!(
            resp.query.projection.checkpoint_plan_v1,
            cfg!(feature = "app_api")
        );
        assert_eq!(
            resp.query.projection.checkpoint_publish_v1,
            cfg!(feature = "app_api")
        );
        assert_eq!(
            resp.query.projection.shard_catalog_v1,
            cfg!(feature = "app_api")
        );
        assert_eq!(
            resp.query.projection.archive_export_v1,
            cfg!(feature = "app_api")
        );
        assert_eq!(
            resp.query.projection.archive_version,
            QUERY_PROJECTION_SHARD_ARCHIVE_VERSION
        );
        assert_eq!(
            resp.query.projection.blob_class_custom_id,
            QUERY_PROJECTION_DA_BLOB_CLASS_CUSTOM_ID
        );
        assert_eq!(resp.query.projection.codec, QUERY_PROJECTION_DA_CODEC);
        assert_eq!(
            resp.query.projection.rowset_codec,
            QUERY_PROJECTION_SHARD_ROWSET_CODEC
        );
        assert_eq!(resp.query.projection.compression, "zstd");
        assert_eq!(
            resp.query.projection.default_partition_count,
            QUERY_PROJECTION_DEFAULT_PARTITION_COUNT
        );
        if cfg!(feature = "app_api") {
            assert_eq!(
                resp.query.projection.export_supported_resources,
                vec!["accounts".to_string(), "asset_holders".to_string()]
            );
        } else {
            assert!(resp.query.projection.export_supported_resources.is_empty());
        }
        assert!(
            resp.query
                .projection
                .metadata_keys
                .contains(&QUERY_PROJECTION_METADATA_LOCATOR_KEY.to_string())
        );
        assert!(
            resp.query
                .projection
                .metadata_keys
                .contains(&QUERY_PROJECTION_METADATA_ROWSET_HASH_KEY.to_string())
        );
        assert!(
            resp.query
                .projection
                .latest_checkpoint_indexed_height
                .is_none()
        );
        assert!(
            resp.query
                .projection
                .latest_checkpoint_block_hash_hex
                .is_none()
        );
        assert!(
            resp.crypto
                .curves
                .allowed_curve_ids
                .contains(&CurveId::ED25519.as_u8()),
            "expected ED25519 curve id to be advertised"
        );
    }

    #[tokio::test]
    async fn node_capabilities_reports_projection_checkpoint_snapshot_when_present() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let world = iroha_core::state::World::default();
        let state = State::new_for_testing(world, kura, query_handle);
        let expected_hash =
            iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::new([0x5A; iroha_crypto::Hash::LENGTH]),
            );
        state.persist_query_projection_checkpoint(Some(
            iroha_core::query::projection_checkpoint::QueryProjectionCheckpoint::from_index_status(
                iroha_core::query::index_status::QueryIndexStatus {
                    indexed_height: 44,
                    indexed_block_hash: Some(expected_hash),
                },
                1_714_000_444,
                Vec::new(),
            ),
        ));

        let resp = handle_node_capabilities(std::sync::Arc::new(state))
            .await
            .expect("ok");
        assert_eq!(
            resp.query.projection.archive_version,
            QUERY_PROJECTION_SHARD_ARCHIVE_VERSION
        );
        assert_eq!(
            resp.query.projection.rowset_codec,
            QUERY_PROJECTION_SHARD_ROWSET_CODEC
        );
        assert_eq!(
            resp.query.projection.archive_export_v1,
            cfg!(feature = "app_api")
        );
        assert_eq!(
            resp.query.projection.checkpoint_plan_v1,
            cfg!(feature = "app_api")
        );
        assert_eq!(
            resp.query.projection.checkpoint_publish_v1,
            cfg!(feature = "app_api")
        );
        assert_eq!(
            resp.query.projection.shard_catalog_v1,
            cfg!(feature = "app_api")
        );
        assert_eq!(
            resp.query.projection.latest_checkpoint_indexed_height,
            Some(44)
        );
        assert_eq!(
            resp.query.projection.latest_checkpoint_block_hash_hex,
            Some(hex::encode(expected_hash.as_ref()))
        );
    }

    #[tokio::test]
    async fn node_query_projection_checkpoint_returns_none_when_absent() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let world = iroha_core::state::World::default();
        let state = State::new_for_testing(world, kura, query_handle);

        assert!(
            handle_node_query_projection_checkpoint(std::sync::Arc::new(state))
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn node_query_projection_checkpoint_returns_persisted_descriptor() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let world = iroha_core::state::World::default();
        let state = State::new_for_testing(world, kura, query_handle);
        let expected_hash =
            iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                iroha_crypto::Hash::new([0x5A; iroha_crypto::Hash::LENGTH]),
            );
        state.persist_query_projection_checkpoint(Some(
            iroha_core::query::projection_checkpoint::QueryProjectionCheckpoint::from_index_status(
                iroha_core::query::index_status::QueryIndexStatus {
                    indexed_height: 44,
                    indexed_block_hash: Some(expected_hash),
                },
                1_714_000_444,
                vec![iroha_core::query::projection_checkpoint::QueryProjectionCheckpointShard {
                    resource:
                        iroha_core::query::projection_checkpoint::QueryProjectionResourceKind::AssetHolders,
                    partition_id: 7,
                    asset_definition_id: Some("pkr#sbp".to_string()),
                    manifest_digest: iroha_data_model::da::types::BlobDigest::new([0x11; 32]),
                    storage_ticket: iroha_data_model::da::types::StorageTicketId::new([0x22; 32]),
                    blob_hash: iroha_data_model::da::types::BlobDigest::new([0x33; 32]),
                }],
            ),
        ));

        let resp = handle_node_query_projection_checkpoint(std::sync::Arc::new(state))
            .await
            .expect("checkpoint response");
        assert_eq!(resp.version, 1);
        assert_eq!(resp.schema_version, QUERY_PROJECTION_SCHEMA_VERSION);
        assert_eq!(resp.blob_class, "custom");
        assert_eq!(
            resp.blob_class_custom_id,
            Some(QUERY_PROJECTION_DA_BLOB_CLASS_CUSTOM_ID)
        );
        assert_eq!(resp.codec, QUERY_PROJECTION_DA_CODEC);
        assert_eq!(resp.compression, "zstd");
        assert_eq!(resp.indexed_height, 44);
        assert_eq!(
            resp.indexed_block_hash_hex,
            Some(hex::encode(expected_hash.as_ref()))
        );
        assert_eq!(resp.emitted_at_unix, 1_714_000_444);
        assert_eq!(resp.shards.len(), 1);
        assert_eq!(resp.shards[0].resource, "asset_holders");
        assert_eq!(resp.shards[0].partition_id, 7);
        assert_eq!(
            resp.shards[0].asset_definition_id.as_deref(),
            Some("pkr#sbp")
        );
        assert_eq!(resp.shards[0].manifest_digest_hex, hex::encode([0x11; 32]));
        assert_eq!(resp.shards[0].storage_ticket_hex, hex::encode([0x22; 32]));
        assert_eq!(resp.shards[0].blob_hash_hex, hex::encode([0x33; 32]));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn node_query_projection_checkpoint_plan_rebuilds_uploaded_shards() {
        use iroha_data_model::Registrable;
        use iroha_data_model::prelude::{Account, Domain, DomainId};

        let authority = iroha_crypto::KeyPair::random();
        let alice = iroha_crypto::KeyPair::random();
        let authority_id =
            iroha_data_model::account::AccountId::new(authority.public_key().clone());
        let alice_id = iroha_data_model::account::AccountId::new(alice.public_key().clone());
        let domain_id = DomainId::try_new("projection-plan", "universal").expect("domain");
        let world = iroha_core::state::World::with(
            [Domain::new(domain_id).build(&authority_id)],
            [
                Account::new(authority_id.clone()).build(&authority_id),
                Account::new(alice_id.clone()).build(&authority_id),
            ],
            [],
        );
        let state = std::sync::Arc::new(State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let archive_emitted_at_unix = 1_714_001_111;
        let checkpoint_emitted_at_unix = 1_714_001_222;
        let request = accounts_checkpoint_request_for_state(
            &state,
            checkpoint_emitted_at_unix,
            archive_emitted_at_unix,
            0x11,
            0x21,
        );
        let first_shard = request.shards.first().expect("accounts shard");
        let archive = build_accounts_projection_shard_archive(
            state.as_ref(),
            first_shard.partition_id,
            archive_emitted_at_unix,
        )
        .expect("archive");
        let expected_shard = archive
            .clone()
            .into_checkpoint_shard(
                BlobDigest::new([0x11; 32]),
                StorageTicketId::new([0x21; 32]),
            )
            .expect("checkpoint shard");

        let response = handle_node_query_projection_checkpoint_plan(state.clone(), request)
            .await
            .expect("plan");

        assert_eq!(response.emitted_at_unix, checkpoint_emitted_at_unix);
        assert_eq!(response.indexed_height, 0);
        assert_eq!(
            response.shards.len(),
            build_accounts_projection_shard_catalog_entries(state.as_ref()).len()
        );
        assert_eq!(response.shards[0].resource, "accounts");
        assert_eq!(response.shards[0].partition_id, first_shard.partition_id);
        assert_eq!(
            response.shards[0].manifest_digest_hex,
            hex::encode([0x11; 32])
        );
        assert_eq!(
            response.shards[0].storage_ticket_hex,
            hex::encode([0x21; 32])
        );
        assert_eq!(
            response.shards[0].blob_hash_hex,
            hex::encode(expected_shard.blob_hash.as_bytes())
        );
        assert!(state.query_projection_checkpoint_snapshot().is_none());
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn node_query_projection_checkpoint_plan_rejects_incomplete_shard_set() {
        use iroha_data_model::Registrable;
        use iroha_data_model::prelude::{Account, Domain, DomainId};

        let authority = iroha_crypto::KeyPair::random();
        let authority_id =
            iroha_data_model::account::AccountId::new(authority.public_key().clone());
        let domain_id = DomainId::try_new("projection-plan-gap", "universal").expect("domain");
        let mut accounts = vec![Account::new(authority_id.clone()).build(&authority_id)];
        for _ in 0..32 {
            let key_pair = iroha_crypto::KeyPair::random();
            let account_id =
                iroha_data_model::account::AccountId::new(key_pair.public_key().clone());
            accounts.push(Account::new(account_id).build(&authority_id));
        }
        let world = iroha_core::state::World::with(
            [Domain::new(domain_id).build(&authority_id)],
            accounts,
            [],
        );
        let state = std::sync::Arc::new(State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let mut request =
            accounts_checkpoint_request_for_state(&state, 1_714_001_260, 1_714_001_250, 0x51, 0x61);
        assert!(
            request.shards.len() >= 2,
            "expected more than one live account shard for completeness validation"
        );
        request.shards.pop();

        let err = handle_node_query_projection_checkpoint_plan(state, request)
            .await
            .expect_err("incomplete shard set must fail");

        let message = err.to_string();
        assert!(message.contains("canonical live shard catalog"));
        assert!(message.contains("missing"));
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn node_query_projection_checkpoint_publish_persists_descriptor() {
        use iroha_data_model::Registrable;
        use iroha_data_model::prelude::{Account, Domain, DomainId};

        let authority = iroha_crypto::KeyPair::random();
        let alice = iroha_crypto::KeyPair::random();
        let authority_id =
            iroha_data_model::account::AccountId::new(authority.public_key().clone());
        let alice_id = iroha_data_model::account::AccountId::new(alice.public_key().clone());
        let domain_id = DomainId::try_new("projection-publish", "universal").expect("domain");
        let world = iroha_core::state::World::with(
            [Domain::new(domain_id).build(&authority_id)],
            [
                Account::new(authority_id.clone()).build(&authority_id),
                Account::new(alice_id.clone()).build(&authority_id),
            ],
            [],
        );
        let state = std::sync::Arc::new(State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let checkpoint_emitted_at_unix = 1_714_001_333;
        let request = accounts_checkpoint_request_for_state(
            &state,
            checkpoint_emitted_at_unix,
            1_714_001_300,
            0x31,
            0x41,
        );

        let response = handle_node_query_projection_checkpoint_publish(state.clone(), request)
            .await
            .expect("publish");

        assert_eq!(response.emitted_at_unix, checkpoint_emitted_at_unix);
        let persisted = state
            .query_projection_checkpoint_snapshot()
            .expect("persisted checkpoint");
        assert_eq!(persisted.emitted_at_unix, checkpoint_emitted_at_unix);
        assert_eq!(
            persisted.shards.len(),
            build_accounts_projection_shard_catalog_entries(state.as_ref()).len()
        );
        assert_eq!(
            response.shards[0].blob_hash_hex,
            hex::encode(persisted.shards[0].blob_hash.as_bytes())
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn node_query_projection_shard_catalog_builds_accounts_entries() {
        use iroha_data_model::Registrable;
        use iroha_data_model::prelude::{Account, Domain, DomainId};

        let authority = iroha_crypto::KeyPair::random();
        let alice = iroha_crypto::KeyPair::random();
        let bob = iroha_crypto::KeyPair::random();
        let authority_id =
            iroha_data_model::account::AccountId::new(authority.public_key().clone());
        let alice_id = iroha_data_model::account::AccountId::new(alice.public_key().clone());
        let bob_id = iroha_data_model::account::AccountId::new(bob.public_key().clone());
        let domain_id = DomainId::try_new("projection-catalog", "universal").expect("domain");
        let world = iroha_core::state::World::with(
            [Domain::new(domain_id).build(&authority_id)],
            [
                Account::new(authority_id.clone()).build(&authority_id),
                Account::new(alice_id.clone()).build(&authority_id),
                Account::new(bob_id.clone()).build(&authority_id),
            ],
            [],
        );
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );

        let response = handle_node_query_projection_shard_catalog(
            std::sync::Arc::new(state),
            "accounts".to_string(),
            NodeProjectionShardCatalogQuery {
                asset_definition_id: None,
                offset: None,
                limit: None,
            },
        )
        .await
        .expect("catalog");

        assert_eq!(response.version, QUERY_PROJECTION_SHARD_CATALOG_VERSION);
        assert_eq!(response.resource, "accounts");
        assert_eq!(
            response.default_partition_count,
            QUERY_PROJECTION_DEFAULT_PARTITION_COUNT
        );
        assert!(
            !response.entries.is_empty(),
            "expected at least one non-empty shard"
        );
        assert_eq!(
            response
                .entries
                .iter()
                .map(|entry| entry.row_count)
                .sum::<u64>(),
            3
        );
        assert!(
            response.entries.iter().all(|entry| {
                entry.asset_definition_id.is_none() && entry.asset_alias.is_none()
            })
        );
        assert_eq!(response.total_entries, response.entries.len() as u64);
        assert!(response.next_offset.is_none());
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn node_query_projection_shard_catalog_builds_asset_holder_entries() {
        use iroha_data_model::Registrable;
        use iroha_data_model::prelude::{
            Account, Asset, AssetDefinition, AssetDefinitionId, AssetId, Domain, DomainId,
        };
        use iroha_primitives::numeric::Numeric;

        let authority = iroha_crypto::KeyPair::random();
        let alice = iroha_crypto::KeyPair::random();
        let bob = iroha_crypto::KeyPair::random();
        let authority_id =
            iroha_data_model::account::AccountId::new(authority.public_key().clone());
        let alice_id = iroha_data_model::account::AccountId::new(alice.public_key().clone());
        let bob_id = iroha_data_model::account::AccountId::new(bob.public_key().clone());
        let domain_id =
            DomainId::try_new("projection-catalog-assets", "universal").expect("domain");
        let rose_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("name"));
        let tulip_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "tulip".parse().expect("name"));
        let rose_definition = AssetDefinition::numeric(rose_definition_id.clone())
            .with_name("rose".to_owned())
            .build(&authority_id);
        let tulip_definition = AssetDefinition::numeric(tulip_definition_id.clone())
            .with_name("tulip".to_owned())
            .build(&authority_id);
        let world = iroha_core::state::World::with_assets(
            [Domain::new(domain_id).build(&authority_id)],
            [
                Account::new(authority_id.clone()).build(&authority_id),
                Account::new(alice_id.clone()).build(&authority_id),
                Account::new(bob_id.clone()).build(&authority_id),
            ],
            [rose_definition, tulip_definition],
            [
                Asset::new(
                    AssetId::new(rose_definition_id.clone(), alice_id.clone()),
                    Numeric::from(10_u32),
                ),
                Asset::new(
                    AssetId::new(rose_definition_id.clone(), bob_id.clone()),
                    Numeric::from(25_u32),
                ),
                Asset::new(
                    AssetId::new(tulip_definition_id.clone(), alice_id.clone()),
                    Numeric::from(50_u32),
                ),
            ],
            [],
        );
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );

        let response = handle_node_query_projection_shard_catalog(
            std::sync::Arc::new(state),
            "asset_holders".to_string(),
            NodeProjectionShardCatalogQuery {
                asset_definition_id: None,
                offset: Some(0),
                limit: Some(1),
            },
        )
        .await
        .expect("catalog");

        assert_eq!(response.resource, "asset_holders");
        assert_eq!(response.limit, 1);
        assert!(response.total_entries >= 2);
        assert_eq!(response.entries.len(), 1);
        assert_eq!(response.next_offset, Some(1));
        assert!(
            response.entries[0].asset_definition_id.is_some(),
            "holder catalog must retain the asset discriminator"
        );
        assert!(
            response.entries[0].row_count > 0,
            "holder catalog entries must represent non-empty shards"
        );
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn node_query_projection_shard_export_builds_accounts_archive() {
        use iroha_data_model::Registrable;
        use iroha_data_model::prelude::{Account, Domain, DomainId};

        let authority = iroha_crypto::KeyPair::random();
        let alice = iroha_crypto::KeyPair::random();
        let authority_id =
            iroha_data_model::account::AccountId::new(authority.public_key().clone());
        let alice_id = iroha_data_model::account::AccountId::new(alice.public_key().clone());
        let domain_id = DomainId::try_new("projection", "universal").expect("domain");
        let world = iroha_core::state::World::with(
            [Domain::new(domain_id).build(&authority_id)],
            [
                Account::new(authority_id.clone()).build(&authority_id),
                Account::new(alice_id.clone()).build(&authority_id),
            ],
            [],
        );
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let partition_id = query_projection_default_partition_for_account(&alice_id.to_string());

        let archive = handle_node_query_projection_shard_export(
            std::sync::Arc::new(state),
            "accounts".to_string(),
            partition_id,
            NodeProjectionShardExportQuery {
                asset_definition_id: None,
            },
        )
        .await
        .expect("export accounts shard");

        assert_eq!(archive.resource, QueryProjectionResourceKind::Accounts);
        assert_eq!(archive.partition_id, partition_id);
        let rowset: QueryProjectionShardRowSet =
            norito::decode_from_bytes(&archive.payload).expect("decode rowset");
        match rowset {
            QueryProjectionShardRowSet::Accounts(rowset) => {
                assert_eq!(rowset.partition_id, partition_id);
                assert!(
                    rowset
                        .rows
                        .iter()
                        .any(|row| row.account_id == alice_id.to_string()),
                    "expected exported partition to contain alice"
                );
                assert!(rowset.rows.iter().all(|row| {
                    query_projection_default_partition_for_account(&row.account_id) == partition_id
                }));
            }
            other => panic!("unexpected rowset variant: {other:?}"),
        }
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn node_query_projection_shard_export_builds_asset_holders_archive() {
        use iroha_data_model::Registrable;
        use iroha_data_model::prelude::{
            Account, Asset, AssetDefinition, AssetDefinitionId, AssetId, Domain, DomainId,
        };
        use iroha_primitives::numeric::Numeric;

        let authority = iroha_crypto::KeyPair::random();
        let alice = iroha_crypto::KeyPair::random();
        let bob = iroha_crypto::KeyPair::random();
        let authority_id =
            iroha_data_model::account::AccountId::new(authority.public_key().clone());
        let alice_id = iroha_data_model::account::AccountId::new(alice.public_key().clone());
        let bob_id = iroha_data_model::account::AccountId::new(bob.public_key().clone());
        let domain_id = DomainId::try_new("projection-holders", "universal").expect("domain");
        let definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("name"));
        let definition = AssetDefinition::numeric(definition_id.clone())
            .with_name("rose".to_owned())
            .build(&authority_id);
        let world = iroha_core::state::World::with_assets(
            [Domain::new(domain_id).build(&authority_id)],
            [
                Account::new(authority_id.clone()).build(&authority_id),
                Account::new(alice_id.clone()).build(&authority_id),
                Account::new(bob_id.clone()).build(&authority_id),
            ],
            [definition],
            [
                Asset::new(
                    AssetId::new(definition_id.clone(), alice_id.clone()),
                    Numeric::from(10_u32),
                ),
                Asset::new(
                    AssetId::new(definition_id.clone(), bob_id.clone()),
                    Numeric::from(20_u32),
                ),
            ],
            [],
        );
        let state = State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let partition_id = query_projection_default_partition_for_account(&alice_id.to_string());

        let archive = handle_node_query_projection_shard_export(
            std::sync::Arc::new(state),
            "asset_holders".to_string(),
            partition_id,
            NodeProjectionShardExportQuery {
                asset_definition_id: Some(definition_id.to_string()),
            },
        )
        .await
        .expect("export asset holders shard");

        assert_eq!(archive.resource, QueryProjectionResourceKind::AssetHolders);
        assert_eq!(archive.partition_id, partition_id);
        assert_eq!(archive.asset_definition_id, Some(definition_id.to_string()));
        let rowset: QueryProjectionShardRowSet =
            norito::decode_from_bytes(&archive.payload).expect("decode rowset");
        match rowset {
            QueryProjectionShardRowSet::AssetHolders(rowset) => {
                assert_eq!(rowset.partition_id, partition_id);
                assert_eq!(rowset.asset_definition_id, definition_id.to_string());
                assert!(
                    rowset
                        .rows
                        .iter()
                        .any(|row| row.account_id == alice_id.to_string()),
                    "expected exported partition to contain alice holder row"
                );
                assert!(rowset.rows.iter().all(|row| {
                    query_projection_default_partition_for_account(&row.account_id) == partition_id
                }));
            }
            other => panic!("unexpected rowset variant: {other:?}"),
        }
    }

    #[tokio::test]
    async fn runtime_metrics_defaults() {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let world = iroha_core::state::World::default();
        let state = State::new_for_testing(world, kura, query_handle);

        let resp = handle_runtime_metrics(std::sync::Arc::new(state))
            .await
            .expect("ok");
        assert_eq!(resp.abi_version, 1);
        assert_eq!(resp.upgrade_events_total.proposed, 0);
        assert_eq!(resp.upgrade_events_total.activated, 0);
        assert_eq!(resp.upgrade_events_total.canceled, 0);
    }
}
