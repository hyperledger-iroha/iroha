//! Asset transfer control records used for account-scoped on-chain asset policy.
use std::{format, string::String, vec::Vec};
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use crate::asset::AssetDefinitionId;
/// Account metadata key storing the v1 asset-transfer control store.
pub const ASSET_TRANSFER_CONTROL_METADATA_KEY: &str = "asset_transfer_controls";
/// Maximum UTF-8 byte length of an asset-transfer availability reason.
pub const ASSET_TRANSFER_AVAILABILITY_MAX_REASON_BYTES_V1: usize = 512;
/// Validate an optional operator reason for an availability transition.
///
/// Reasons are persisted in account metadata and therefore must be canonical,
/// bounded text: non-empty, unpadded, and free of control characters.
///
/// # Errors
/// Returns [`crate::error::ParseError`] when a provided reason is not canonical.
pub fn validate_asset_transfer_availability_reason(
    reason: Option<&str>,
) -> Result<(), crate::error::ParseError> {
    let Some(reason) = reason else {
        return Ok(());
    };
    if reason.is_empty() {
        return Err(crate::error::ParseError::new(
            "asset-transfer availability reason must not be empty",
        ));
    }
    if reason.trim() != reason {
        return Err(crate::error::ParseError::new(
            "asset-transfer availability reason must be unpadded",
        ));
    }
    if reason.len() > ASSET_TRANSFER_AVAILABILITY_MAX_REASON_BYTES_V1 {
        return Err(crate::error::ParseError::new(
            "asset-transfer availability reason exceeds maximum byte length",
        ));
    }
    if reason.chars().any(char::is_control) {
        return Err(crate::error::ParseError::new(
            "asset-transfer availability reason must not contain control characters",
        ));
    }
    Ok(())
}
/// Whether an account may participate in one direction of account-to-account transfers.
///
/// This policy does not govern supply operations such as mint or burn. Holding
/// limits independently govern every native credit, including mint.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "state", content = "value")]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
pub enum AssetTransferAvailability {
    /// Asset movement in this direction is permitted.
    #[default]
    Enabled,
    /// Asset movement in this direction is rejected.
    Disabled,
}
impl AssetTransferAvailability {
    /// Returns `true` when movement in this direction is enabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}
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
    pub cap_amount: Option<Quantity>,
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
    pub spent_amount: Quantity,
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
    /// Monotonic revision of the directional availability state.
    ///
    /// Revision zero represents the implicit initial state where both directions
    /// are enabled. Every successful availability transition increments it.
    #[norito(default)]
    pub availability_revision: u64,
    /// Whether incoming asset movement is available.
    #[norito(default)]
    pub incoming_availability: AssetTransferAvailability,
    /// Whether outgoing asset movement is available.
    #[norito(default)]
    pub outgoing_availability: AssetTransferAvailability,
    /// Operator reason associated with the latest availability transition.
    #[norito(default)]
    pub availability_reason: Option<String>,
    /// Whether outbound transfers are blacklisted.
    #[norito(default)]
    pub blacklisted: bool,
    /// Maximum post-credit balance for every future native credit.
    ///
    /// `None` means that no native holding limit is configured. A limit below the
    /// current balance leaves existing funds untouched and rejects further credit
    /// until the balance is no greater than the limit. The configured limit is
    /// evaluated independently for each concrete routed
    /// [`crate::asset::AssetId`] balance bucket.
    #[norito(default)]
    pub holding_limit: Option<Quantity>,
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
    /// Construct the implicit initial control state for one asset definition.
    #[must_use]
    pub fn new(asset_definition_id: AssetDefinitionId) -> Self {
        Self {
            asset_definition_id,
            availability_revision: 0,
            incoming_availability: AssetTransferAvailability::Enabled,
            outgoing_availability: AssetTransferAvailability::Enabled,
            availability_reason: None,
            blacklisted: false,
            holding_limit: None,
            limits: Vec::new(),
            usages: Vec::new(),
            updated_at_ms: None,
        }
    }
    /// Returns `true` when the record has no active controls and can be dropped from storage.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.availability_revision == 0
            && self.incoming_availability.is_enabled()
            && self.outgoing_availability.is_enabled()
            && self.availability_reason.is_none()
            && !self.blacklisted
            && self.holding_limit.is_none()
            && self.limits.iter().all(|limit| limit.cap_amount.is_none())
    }
}
#[cfg(test)]
mod availability_tests {
    use super::*;
    use crate::domain::DomainId;
    fn definition_id() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "rose".parse().expect("asset name"),
        )
    }
    #[test]
    fn availability_defaults_to_enabled() {
        assert!(AssetTransferAvailability::default().is_enabled());
        let record = AssetTransferControlRecord::new(definition_id());
        assert_eq!(record.availability_revision, 0);
        assert!(record.incoming_availability.is_enabled());
        assert!(record.outgoing_availability.is_enabled());
        assert!(record.is_empty());
    }
    #[test]
    fn availability_revision_keeps_reopened_record() {
        let record = AssetTransferControlRecord {
            asset_definition_id: definition_id(),
            availability_revision: 2,
            incoming_availability: AssetTransferAvailability::Enabled,
            outgoing_availability: AssetTransferAvailability::Enabled,
            availability_reason: Some("wallet reopened".to_owned()),
            blacklisted: false,
            holding_limit: None,
            limits: Vec::new(),
            usages: Vec::new(),
            updated_at_ms: Some(2),
        };
        assert!(!record.is_empty());
    }
    #[test]
    fn availability_reason_is_canonical_and_byte_bounded() {
        let exact_limit = "é".repeat(ASSET_TRANSFER_AVAILABILITY_MAX_REASON_BYTES_V1 / 2);
        assert!(validate_asset_transfer_availability_reason(Some(&exact_limit)).is_ok());
        assert!(validate_asset_transfer_availability_reason(None).is_ok());
        let over_limit = format!("{exact_limit}a");
        assert!(validate_asset_transfer_availability_reason(Some(&over_limit)).is_err());
        for invalid in ["", " padded", "padded ", "line\nbreak"] {
            assert!(
                validate_asset_transfer_availability_reason(Some(invalid)).is_err(),
                "{invalid:?} must be rejected"
            );
        }
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
#[cfg(test)]
mod tests {
    use iroha_primitives::numeric::Numeric;
    use super::*;
    #[derive(Encode)]
    struct ForgedAssetTransferLimit {
        window: AssetTransferControlWindow,
        cap_amount: Option<Numeric>,
    }
    #[derive(Encode)]
    struct ForgedAssetTransferUsageBucket {
        window: AssetTransferControlWindow,
        bucket_start_ms: u64,
        spent_amount: Numeric,
    }
    #[test]
    fn negative_numeric_payloads_cannot_decode_as_transfer_control_quantities() {
        let forged_limit = ForgedAssetTransferLimit {
            window: AssetTransferControlWindow::Day,
            cap_amount: Some(Numeric::new(-1_i32, 0)),
        };
        let encoded = forged_limit.encode();
        assert!(
            AssetTransferLimit::decode(&mut encoded.as_slice()).is_err(),
            "a signed negative payload must not cross the transfer-cap quantity boundary"
        );
        let forged_usage = ForgedAssetTransferUsageBucket {
            window: AssetTransferControlWindow::Day,
            bucket_start_ms: 0,
            spent_amount: Numeric::new(-1_i32, 0),
        };
        let encoded = forged_usage.encode();
        assert!(
            AssetTransferUsageBucket::decode(&mut encoded.as_slice()).is_err(),
            "a signed negative payload must not cross the transfer-usage quantity boundary"
        );
    }
}
