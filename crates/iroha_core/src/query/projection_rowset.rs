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

/// Canonical rowset payload variants supported by the projection archive contract today.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub enum QueryProjectionShardRowSet {
    /// Rowset for the `accounts` resource family.
    Accounts(QueryProjectionAccountsShardRowSet),
    /// Rowset for the `asset_holders` resource family.
    AssetHolders(QueryProjectionAssetHoldersShardRowSet),
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

impl QueryProjectionShardRowSet {
    /// Resource family represented by this rowset.
    #[must_use]
    pub const fn resource(&self) -> QueryProjectionResourceKind {
        match self {
            Self::Accounts(_) => QueryProjectionResourceKind::Accounts,
            Self::AssetHolders(_) => QueryProjectionResourceKind::AssetHolders,
        }
    }

    /// Stable partition identifier represented by this rowset.
    #[must_use]
    pub const fn partition_id(&self) -> u32 {
        match self {
            Self::Accounts(rowset) => rowset.partition_id,
            Self::AssetHolders(rowset) => rowset.partition_id,
        }
    }

    /// Canonical asset-definition discriminator when present.
    #[must_use]
    pub fn asset_definition_id(&self) -> Option<&str> {
        match self {
            Self::Accounts(_) => None,
            Self::AssetHolders(rowset) => Some(rowset.asset_definition_id.as_str()),
        }
    }

    /// Number of logical rows carried inside this rowset.
    #[must_use]
    pub fn row_count(&self) -> u64 {
        match self {
            Self::Accounts(rowset) => rowset.rows.len() as u64,
            Self::AssetHolders(rowset) => rowset.rows.len() as u64,
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
}
