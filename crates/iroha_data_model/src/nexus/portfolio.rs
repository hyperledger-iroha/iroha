//! Portfolio snapshot structures for UAID aggregation.

use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::{AccountId, rekey::AccountAlias},
    asset::{AssetDefinitionId, AssetId},
    nexus::{DataSpaceId, UniversalAccountId},
};

/// Aggregated holdings for a universal account identifier.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct UniversalPortfolio {
    /// Universal account identifier the snapshot describes.
    pub uaid: UniversalAccountId,
    /// Grouped portfolio entries per dataspace.
    #[norito(default)]
    pub dataspaces: Vec<DataspacePortfolio>,
    /// Summary counters for the snapshot.
    pub totals: PortfolioTotals,
}

/// Summary counters for a UAID portfolio snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct PortfolioTotals {
    /// Number of accounts bound to the UAID.
    pub accounts: u64,
    /// Number of asset positions across all accounts.
    pub positions: u64,
}

/// Holdings scoped to a single dataspace.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct DataspacePortfolio {
    /// Dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Optional human-readable alias for the dataspace.
    #[norito(default)]
    pub dataspace_alias: Option<String>,
    /// Accounts observed within this dataspace.
    #[norito(default)]
    pub accounts: Vec<AccountPortfolio>,
}

/// Holdings for a single account mapped to a UAID.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AccountPortfolio {
    /// Canonical account identifier.
    pub account_id: AccountId,
    /// Stable alias assigned to the account, if present.
    #[norito(default)]
    pub label: Option<AccountAlias>,
    /// Asset positions for the account.
    #[norito(default)]
    pub assets: Vec<AssetPosition>,
}

/// Individual asset position captured in a portfolio snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AssetPosition {
    /// Fully qualified asset identifier (definition + account).
    pub asset_id: AssetId,
    /// Asset definition identifier for the position.
    pub asset_definition_id: AssetDefinitionId,
    /// Quantity held; serialized as Norito numeric value.
    pub quantity: Numeric,
}
