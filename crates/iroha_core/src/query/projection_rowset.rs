//! Canonical rowset payloads carried inside query projection shard archives.
//!
//! These types define the stable on-wire rows that a DA-backed projection shard
//! archive contains. The aggregate DSL can be evaluated against these rows after
//! retrieval from DA without depending on ad hoc JSON maps or endpoint-specific
//! response wrappers.

use iroha_primitives::numeric::Numeric;
use norito::{
    codec::{Decode, Encode},
    to_bytes,
};

use crate::query::projection_checkpoint::QueryProjectionResourceKind;

/// Version of the logical rowset payload carried inside a shard archive.
pub const QUERY_PROJECTION_ROWSET_VERSION: u16 = 1;

/// Alias-aware projected account row.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionAccountRow {
    /// Canonical account id.
    pub account_id: String,
    /// Primary alias literal when present.
    pub primary_alias: Option<String>,
    /// Primary alias label when present.
    pub primary_alias_name: Option<String>,
    /// Primary alias dataspace when present.
    pub primary_alias_dataspace: Option<String>,
    /// Primary alias domain when present.
    pub primary_alias_domain: Option<String>,
    /// Whether the account has a primary alias projection.
    pub has_primary_alias: bool,
}

/// Alias-aware projected asset-holder row.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionAssetHolderRow {
    /// Canonical account id.
    pub account_id: String,
    /// Stable balance scope literal.
    pub scope: String,
    /// Exact aggregated quantity for `(account_id, scope)`.
    pub quantity: Numeric,
    /// Primary alias literal when present.
    pub primary_alias: Option<String>,
    /// Primary alias label when present.
    pub primary_alias_name: Option<String>,
    /// Primary alias dataspace when present.
    pub primary_alias_dataspace: Option<String>,
    /// Primary alias domain when present.
    pub primary_alias_domain: Option<String>,
    /// Whether the holder account has a primary alias projection.
    pub has_primary_alias: bool,
}

/// Alias-aware projected account asset row.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionAccountAssetRow {
    /// Canonical account id.
    pub account_id: String,
    /// Canonical asset definition id.
    pub asset: String,
    /// Stable display name for the asset definition.
    pub asset_name: String,
    /// Resolved asset alias when present.
    pub asset_alias: Option<String>,
    /// Stable balance scope literal.
    pub scope: String,
    /// Exact quantity held in this `(account_id, asset, scope)` row.
    pub quantity: Numeric,
    /// Primary alias literal when present.
    pub primary_alias: Option<String>,
    /// Primary alias label when present.
    pub primary_alias_name: Option<String>,
    /// Primary alias dataspace when present.
    pub primary_alias_dataspace: Option<String>,
    /// Primary alias domain when present.
    pub primary_alias_domain: Option<String>,
    /// Whether the account has a primary alias projection.
    pub has_primary_alias: bool,
}

/// Stable metadata entry captured in an asset-definition projection row.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionMetadataEntry {
    /// Metadata key.
    pub key: String,
    /// Canonical JSON payload for the metadata value.
    pub value_json: String,
}

/// Projected asset definition row.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionAssetDefinitionRow {
    /// Canonical asset definition id.
    pub id: String,
    /// Stable display name.
    pub name: String,
    /// Resolved asset alias when present.
    pub alias: Option<String>,
    /// Alias binding literal when present.
    pub alias_binding_alias: Option<String>,
    /// Alias binding status when present.
    pub alias_binding_status: Option<String>,
    /// Alias lease expiry in unix milliseconds when present.
    pub alias_binding_lease_expiry_ms: Option<u64>,
    /// Alias grace deadline in unix milliseconds when present.
    pub alias_binding_grace_until_ms: Option<u64>,
    /// Alias binding creation time in unix milliseconds when present.
    pub alias_binding_bound_at_ms: Option<u64>,
    /// Metadata snapshot fields available to `metadata.<key>` DSL paths.
    pub metadata: Vec<QueryProjectionMetadataEntry>,
}

/// Projected domain row.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionDomainRow {
    /// Canonical domain id.
    pub id: String,
}

/// Rowset payload for one `accounts` projection shard.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionAccountsShardRowSet {
    /// Payload version.
    pub version: u16,
    /// Stable partition identifier covered by this rowset.
    pub partition_id: u32,
    /// Projected rows assigned to this partition.
    pub rows: Vec<QueryProjectionAccountRow>,
}

/// Rowset payload for one `asset_holders` projection shard.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionAssetHoldersShardRowSet {
    /// Payload version.
    pub version: u16,
    /// Stable partition identifier covered by this rowset.
    pub partition_id: u32,
    /// Canonical asset definition id covered by this rowset.
    pub asset_definition_id: String,
    /// Resolved asset alias when present.
    pub asset_alias: Option<String>,
    /// Projected rows assigned to this partition and asset definition.
    pub rows: Vec<QueryProjectionAssetHolderRow>,
}

/// Rowset payload for one `account_assets` projection shard.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionAccountAssetsShardRowSet {
    /// Payload version.
    pub version: u16,
    /// Stable partition identifier covered by this rowset.
    pub partition_id: u32,
    /// Projected rows assigned to this account partition.
    pub rows: Vec<QueryProjectionAccountAssetRow>,
}

/// Rowset payload for one `asset_definitions` projection shard.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionAssetDefinitionsShardRowSet {
    /// Payload version.
    pub version: u16,
    /// Stable partition identifier covered by this rowset.
    pub partition_id: u32,
    /// Projected rows assigned to this definition partition.
    pub rows: Vec<QueryProjectionAssetDefinitionRow>,
}

/// Rowset payload for one `domains` projection shard.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct QueryProjectionDomainsShardRowSet {
    /// Payload version.
    pub version: u16,
    /// Stable partition identifier covered by this rowset.
    pub partition_id: u32,
    /// Projected rows assigned to this domain partition.
    pub rows: Vec<QueryProjectionDomainRow>,
}

/// Canonical rowset payload variants supported by the projection archive contract today.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub enum QueryProjectionShardRowSet {
    /// Rowset for the `accounts` resource family.
    Accounts(QueryProjectionAccountsShardRowSet),
    /// Rowset for the `account_assets` resource family.
    AccountAssets(QueryProjectionAccountAssetsShardRowSet),
    /// Rowset for the `asset_holders` resource family.
    AssetHolders(QueryProjectionAssetHoldersShardRowSet),
    /// Rowset for the `asset_definitions` resource family.
    AssetDefinitions(QueryProjectionAssetDefinitionsShardRowSet),
    /// Rowset for the `domains` resource family.
    Domains(QueryProjectionDomainsShardRowSet),
}

impl QueryProjectionAccountsShardRowSet {
    /// Construct a rowset for one `accounts` partition.
    #[must_use]
    pub fn new(partition_id: u32, rows: Vec<QueryProjectionAccountRow>) -> Self {
        Self {
            version: QUERY_PROJECTION_ROWSET_VERSION,
            partition_id,
            rows,
        }
    }
}

impl QueryProjectionAssetHoldersShardRowSet {
    /// Construct a rowset for one `asset_holders` partition.
    #[must_use]
    pub fn new(
        partition_id: u32,
        asset_definition_id: String,
        asset_alias: Option<String>,
        rows: Vec<QueryProjectionAssetHolderRow>,
    ) -> Self {
        Self {
            version: QUERY_PROJECTION_ROWSET_VERSION,
            partition_id,
            asset_definition_id,
            asset_alias,
            rows,
        }
    }
}

impl QueryProjectionAccountAssetsShardRowSet {
    /// Construct a rowset for one `account_assets` partition.
    #[must_use]
    pub fn new(partition_id: u32, rows: Vec<QueryProjectionAccountAssetRow>) -> Self {
        Self {
            version: QUERY_PROJECTION_ROWSET_VERSION,
            partition_id,
            rows,
        }
    }
}

impl QueryProjectionAssetDefinitionsShardRowSet {
    /// Construct a rowset for one `asset_definitions` partition.
    #[must_use]
    pub fn new(partition_id: u32, rows: Vec<QueryProjectionAssetDefinitionRow>) -> Self {
        Self {
            version: QUERY_PROJECTION_ROWSET_VERSION,
            partition_id,
            rows,
        }
    }
}

impl QueryProjectionDomainsShardRowSet {
    /// Construct a rowset for one `domains` partition.
    #[must_use]
    pub fn new(partition_id: u32, rows: Vec<QueryProjectionDomainRow>) -> Self {
        Self {
            version: QUERY_PROJECTION_ROWSET_VERSION,
            partition_id,
            rows,
        }
    }
}

impl QueryProjectionShardRowSet {
    /// Resource family represented by this rowset.
    #[must_use]
    pub const fn resource(&self) -> QueryProjectionResourceKind {
        match self {
            Self::Accounts(_) => QueryProjectionResourceKind::Accounts,
            Self::AccountAssets(_) => QueryProjectionResourceKind::AccountAssets,
            Self::AssetHolders(_) => QueryProjectionResourceKind::AssetHolders,
            Self::AssetDefinitions(_) => QueryProjectionResourceKind::AssetDefinitions,
            Self::Domains(_) => QueryProjectionResourceKind::Domains,
        }
    }

    /// Stable partition identifier represented by this rowset.
    #[must_use]
    pub const fn partition_id(&self) -> u32 {
        match self {
            Self::Accounts(rowset) => rowset.partition_id,
            Self::AccountAssets(rowset) => rowset.partition_id,
            Self::AssetHolders(rowset) => rowset.partition_id,
            Self::AssetDefinitions(rowset) => rowset.partition_id,
            Self::Domains(rowset) => rowset.partition_id,
        }
    }

    /// Canonical asset-definition discriminator when present.
    #[must_use]
    pub fn asset_definition_id(&self) -> Option<&str> {
        match self {
            Self::Accounts(_) => None,
            Self::AccountAssets(_) => None,
            Self::AssetHolders(rowset) => Some(rowset.asset_definition_id.as_str()),
            Self::AssetDefinitions(_) => None,
            Self::Domains(_) => None,
        }
    }

    /// Number of logical rows carried inside this rowset.
    #[must_use]
    pub fn row_count(&self) -> u64 {
        match self {
            Self::Accounts(rowset) => rowset.rows.len() as u64,
            Self::AccountAssets(rowset) => rowset.rows.len() as u64,
            Self::AssetHolders(rowset) => rowset.rows.len() as u64,
            Self::AssetDefinitions(rowset) => rowset.rows.len() as u64,
            Self::Domains(rowset) => rowset.rows.len() as u64,
        }
    }

    /// Encode the rowset as canonical Norito bytes for inclusion in an archive payload.
    ///
    /// # Errors
    ///
    /// Returns the Norito encode error when serialization fails.
    pub fn encode_payload(&self) -> Result<Vec<u8>, norito::core::Error> {
        to_bytes(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::decode_from_bytes;

    #[test]
    fn accounts_rowset_round_trips_through_norito() {
        let rowset = QueryProjectionShardRowSet::Accounts(QueryProjectionAccountsShardRowSet::new(
            7,
            vec![QueryProjectionAccountRow {
                account_id: "alice@wonderland".to_owned(),
                primary_alias: Some("alice@hbl.sbp".to_owned()),
                primary_alias_name: Some("alice".to_owned()),
                primary_alias_dataspace: Some("sbp".to_owned()),
                primary_alias_domain: Some("hbl.sbp".to_owned()),
                has_primary_alias: true,
            }],
        ));

        let encoded = rowset.encode_payload().expect("encode rowset");
        let decoded: QueryProjectionShardRowSet =
            decode_from_bytes(&encoded).expect("decode rowset");
        assert_eq!(decoded, rowset);
        assert_eq!(decoded.resource(), QueryProjectionResourceKind::Accounts);
        assert_eq!(decoded.partition_id(), 7);
        assert_eq!(decoded.asset_definition_id(), None);
        assert_eq!(decoded.row_count(), 1);
    }

    #[test]
    fn asset_holders_rowset_reports_asset_discriminator() {
        let rowset =
            QueryProjectionShardRowSet::AssetHolders(QueryProjectionAssetHoldersShardRowSet::new(
                9,
                "pkr#sbp".to_owned(),
                Some("pkr@sbp".to_owned()),
                vec![QueryProjectionAssetHolderRow {
                    account_id: "alice@wonderland".to_owned(),
                    scope: "global".to_owned(),
                    quantity: Numeric::new(123_45, 2),
                    primary_alias: Some("alice@hbl.sbp".to_owned()),
                    primary_alias_name: Some("alice".to_owned()),
                    primary_alias_dataspace: Some("sbp".to_owned()),
                    primary_alias_domain: Some("hbl.sbp".to_owned()),
                    has_primary_alias: true,
                }],
            ));

        assert_eq!(rowset.resource(), QueryProjectionResourceKind::AssetHolders);
        assert_eq!(rowset.partition_id(), 9);
        assert_eq!(rowset.asset_definition_id(), Some("pkr#sbp"));
        assert_eq!(rowset.row_count(), 1);
    }

    #[test]
    fn account_assets_rowset_round_trips_through_norito() {
        let rowset = QueryProjectionShardRowSet::AccountAssets(
            QueryProjectionAccountAssetsShardRowSet::new(
                3,
                vec![QueryProjectionAccountAssetRow {
                    account_id: "alice@wonderland".to_owned(),
                    asset: "pkr#wonderland".to_owned(),
                    asset_name: "pkr".to_owned(),
                    asset_alias: Some("pkr@sbp".to_owned()),
                    scope: "global".to_owned(),
                    quantity: Numeric::new(42, 0),
                    primary_alias: None,
                    primary_alias_name: None,
                    primary_alias_dataspace: None,
                    primary_alias_domain: None,
                    has_primary_alias: false,
                }],
            ),
        );

        let encoded = rowset.encode_payload().expect("encode rowset");
        let decoded: QueryProjectionShardRowSet =
            decode_from_bytes(&encoded).expect("decode rowset");
        assert_eq!(decoded, rowset);
        assert_eq!(
            decoded.resource(),
            QueryProjectionResourceKind::AccountAssets
        );
        assert_eq!(decoded.partition_id(), 3);
        assert_eq!(decoded.row_count(), 1);
    }

    #[test]
    fn asset_definitions_and_domains_rowsets_report_resources() {
        let definitions = QueryProjectionShardRowSet::AssetDefinitions(
            QueryProjectionAssetDefinitionsShardRowSet::new(
                4,
                vec![QueryProjectionAssetDefinitionRow {
                    id: "pkr#wonderland".to_owned(),
                    name: "pkr".to_owned(),
                    alias: Some("pkr@sbp".to_owned()),
                    alias_binding_alias: Some("pkr@sbp".to_owned()),
                    alias_binding_status: Some("active".to_owned()),
                    alias_binding_lease_expiry_ms: Some(1_800_000_000_000),
                    alias_binding_grace_until_ms: None,
                    alias_binding_bound_at_ms: Some(1_700_000_000_000),
                    metadata: vec![QueryProjectionMetadataEntry {
                        key: "display".to_owned(),
                        value_json: "\"PKR\"".to_owned(),
                    }],
                }],
            ),
        );
        assert_eq!(
            definitions.resource(),
            QueryProjectionResourceKind::AssetDefinitions
        );
        assert_eq!(definitions.partition_id(), 4);
        assert_eq!(definitions.row_count(), 1);

        let domains = QueryProjectionShardRowSet::Domains(QueryProjectionDomainsShardRowSet::new(
            5,
            vec![QueryProjectionDomainRow {
                id: "wonderland".to_owned(),
            }],
        ));
        assert_eq!(domains.resource(), QueryProjectionResourceKind::Domains);
        assert_eq!(domains.partition_id(), 5);
        assert_eq!(domains.row_count(), 1);
    }
}
