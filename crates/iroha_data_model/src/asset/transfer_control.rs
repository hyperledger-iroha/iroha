//! Asset transfer control records used for CBDC-style on-chain outbound controls.

use std::{format, string::String, vec::Vec};

use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::asset::AssetDefinitionId;

/// Account metadata key storing the v1 asset-transfer control store.
pub const ASSET_TRANSFER_CONTROL_METADATA_KEY: &str = "asset_transfer_controls";

/// Calendar window used for outbound transfer caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "window", content = "value")]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub enum AssetTransferControlWindow {
    /// UTC calendar day.
    Day,
    /// UTC ISO week starting on Monday.
    Week,
    /// UTC calendar month.
    Month,
}

impl AssetTransferControlWindow {
    /// Canonical uppercase label used by Torii app APIs.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Day => "DAY",
            Self::Week => "WEEK",
            Self::Month => "MONTH",
        }
    }
}

impl core::str::FromStr for AssetTransferControlWindow {
    type Err = crate::error::ParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_uppercase().as_str() {
            "DAY" => Ok(Self::Day),
            "WEEK" => Ok(Self::Week),
            "MONTH" => Ok(Self::Month),
            _ => Err(crate::error::ParseError::new(
                "asset transfer control window must be DAY, WEEK, or MONTH",
            )),
        }
    }
}

impl core::fmt::Display for AssetTransferControlWindow {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Configured cap for a specific calendar window.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct AssetTransferLimit {
    /// Controlled window.
    pub window: AssetTransferControlWindow,
    /// Maximum cumulative outbound amount in the window. `None` clears the limit.
    #[norito(default)]
    pub cap_amount: Option<Numeric>,
}

/// Usage bucket tracking actual spent amount in a UTC calendar window.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct AssetTransferUsageBucket {
    /// Controlled window.
    pub window: AssetTransferControlWindow,
    /// UTC bucket start in unix epoch milliseconds.
    pub bucket_start_ms: u64,
    /// Amount already spent in the bucket.
    pub spent_amount: Numeric,
}

/// Control state for one `(account_id, asset_definition_id)` pair.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct AssetTransferControlRecord {
    /// Controlled asset definition.
    pub asset_definition_id: AssetDefinitionId,
    /// Whether outbound transfers are frozen.
    #[norito(default)]
    pub outgoing_frozen: bool,
    /// Whether outbound transfers are blacklisted.
    #[norito(default)]
    pub blacklisted: bool,
    /// Configured transfer caps.
    #[norito(default)]
    pub limits: Vec<AssetTransferLimit>,
    /// Observed usage buckets for active windows.
    #[norito(default)]
    pub usages: Vec<AssetTransferUsageBucket>,
    /// Last mutation timestamp (unix epoch milliseconds).
    #[norito(default)]
    pub updated_at_ms: Option<u64>,
}

impl AssetTransferControlRecord {
    /// Returns `true` when the record has no active controls and can be dropped from storage.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.outgoing_frozen
            && !self.blacklisted
            && self.limits.iter().all(|limit| limit.cap_amount.is_none())
    }
}

/// Account-scoped store of asset-transfer control entries.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub struct AssetTransferControlStoreV1 {
    /// Controlled asset records for the account.
    #[norito(default)]
    pub controls: Vec<AssetTransferControlRecord>,
}

impl AssetTransferControlStoreV1 {
    /// Fetch the control entry for the asset definition when present.
    pub fn find(
        &self,
        asset_definition_id: &AssetDefinitionId,
    ) -> Option<&AssetTransferControlRecord> {
        self.controls
            .iter()
            .find(|entry| &entry.asset_definition_id == asset_definition_id)
    }

    /// Fetch the mutable control entry for the asset definition when present.
    pub fn find_mut(
        &mut self,
        asset_definition_id: &AssetDefinitionId,
    ) -> Option<&mut AssetTransferControlRecord> {
        self.controls
            .iter_mut()
            .find(|entry| &entry.asset_definition_id == asset_definition_id)
    }

    /// Insert or replace a record, dropping empty records from the store.
    pub fn upsert(&mut self, record: AssetTransferControlRecord) {
        if let Some(existing) = self.find_mut(&record.asset_definition_id) {
            *existing = record;
        } else {
            self.controls.push(record);
        }
        self.prune_empty();
        self.sort_canonical();
    }

    /// Remove the entry for an asset definition.
    pub fn remove(&mut self, asset_definition_id: &AssetDefinitionId) {
        self.controls
            .retain(|entry| &entry.asset_definition_id != asset_definition_id);
    }

    fn prune_empty(&mut self) {
        self.controls.retain(|entry| !entry.is_empty());
    }

    fn sort_canonical(&mut self) {
        self.controls.sort_by(|left, right| {
            left.asset_definition_id
                .cmp(&right.asset_definition_id)
                .then_with(|| left.updated_at_ms.cmp(&right.updated_at_ms))
        });
    }
}
