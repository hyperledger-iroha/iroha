//! Repo and reverse-repo agreement data structures.
//!
//! These descriptors provide the Norito layout used by the forthcoming
//! settlement instructions. They focus on deterministic encoding so the
//! runtime and external fixtures can agree on policy and margin parameters.

use derive_more::{Constructor, Display, FromStr};
use getset::{CopyGetters, Getters};
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
#[cfg(feature = "json")]
use mv::json::JsonKeyCodec;
use norito::{
    codec::{Decode, Encode},
    derive::{JsonDeserialize, JsonSerialize},
};

use crate::{
    Identifiable, Name,
    asset::prelude::AssetDefinitionId,
    metadata::Metadata,
    prelude::{AccountId, Numeric},
};

#[model]
mod model {
    use super::*;

    /// Identifier for a repo agreement lifecycle.
    #[derive(
        Debug,
        Display,
        FromStr,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Constructor,
        Getters,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[display("{name}")]
    #[getset(get = "pub")]
    #[repr(transparent)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    pub struct RepoAgreementId {
        /// Logical name assigned by the initiating desk or workflow.
        pub name: Name,
    }
}

pub use self::model::RepoAgreementId;

string_id!(RepoAgreementId);

/// Cash leg definition used by repo instructions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[getset(get = "pub")]
pub struct RepoCashLeg {
    /// Asset definition used for the cash consideration.
    pub asset_definition_id: AssetDefinitionId,
    /// Quantity of the cash asset exchanged at initiation.
    pub quantity: Numeric,
}

/// Collateral leg definition used by repo instructions.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[getset(get = "pub")]
pub struct RepoCollateralLeg {
    /// Asset definition pledged as collateral.
    pub asset_definition_id: AssetDefinitionId,
    /// Quantity of collateral exchanged at initiation.
    pub quantity: Numeric,
    /// Optional metadata (e.g., ISIN/series) attached at admission time.
    pub metadata: Metadata,
}

impl RepoCollateralLeg {
    /// Construct a collateral leg without metadata.
    pub fn new(asset_definition_id: AssetDefinitionId, quantity: Numeric) -> Self {
        Self {
            asset_definition_id,
            quantity,
            metadata: Metadata::default(),
        }
    }
}

/// Governance knobs captured with each agreement.
#[derive(
    Copy, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, CopyGetters, Decode, Encode, IntoSchema,
)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[getset(get_copy = "pub")]
pub struct RepoGovernance {
    /// Haircut applied to the collateral leg, measured in basis points.
    pub haircut_bps: u16,
    /// Cadence (seconds) between mandatory margin checks.
    pub margin_frequency_secs: u64,
}

impl RepoGovernance {
    /// Governance defaults used when configuration omits explicit values.
    pub fn with_defaults(haircut_bps: u16, margin_frequency_secs: u64) -> Self {
        Self {
            haircut_bps,
            margin_frequency_secs,
        }
    }

    /// Return the cadence between margin checks in milliseconds.
    pub fn margin_frequency_millis(&self) -> Option<u64> {
        (self.margin_frequency_secs != 0).then(|| self.margin_frequency_secs.saturating_mul(1_000))
    }
}

/// End-to-end repo agreement envelope recorded on-chain.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Getters, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(JsonSerialize, JsonDeserialize))]
#[getset(get = "pub")]
pub struct RepoAgreement {
    /// Stable identifier assigned to this agreement.
    pub id: RepoAgreementId,
    /// Initiating participant.
    pub initiator: AccountId,
    /// Counterparty accepting the terms.
    pub counterparty: AccountId,
    /// Optional custodian holding pledged collateral for tri-party agreements.
    pub custodian: Option<AccountId>,
    /// Cash leg settled at repo open and unwind.
    pub cash_leg: RepoCashLeg,
    /// Collateral leg pledged for the duration of the repo.
    pub collateral_leg: RepoCollateralLeg,
    /// Fixed rate (in basis points) agreed for the term.
    pub rate_bps: u16,
    /// Unix timestamp (milliseconds) for the agreed maturity.
    pub maturity_timestamp_ms: u64,
    /// Unix timestamp (milliseconds) when the repo was initiated on-ledger.
    pub initiated_timestamp_ms: u64,
    /// Unix timestamp (milliseconds) when the most recent margin check completed.
    pub last_margin_check_timestamp_ms: u64,
    /// Governance parameters applied to the agreement.
    pub governance: RepoGovernance,
}

impl RepoAgreement {
    /// Builder-style helper for constructing a repo agreement.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: RepoAgreementId,
        initiator: AccountId,
        counterparty: AccountId,
        cash_leg: RepoCashLeg,
        collateral_leg: RepoCollateralLeg,
        rate_bps: u16,
        maturity_timestamp_ms: u64,
        initiated_timestamp_ms: u64,
        governance: RepoGovernance,
        custodian: Option<AccountId>,
    ) -> Self {
        Self {
            id,
            initiator,
            counterparty,
            custodian,
            cash_leg,
            collateral_leg,
            rate_bps,
            maturity_timestamp_ms,
            initiated_timestamp_ms,
            last_margin_check_timestamp_ms: initiated_timestamp_ms,
            governance,
        }
    }

    /// Compute the next margin check timestamp after the provided moment.
    ///
    /// Returns `None` when the governance policy disables margin sweeps
    /// (frequency set to zero). Otherwise returns the earliest timestamp
    /// strictly greater than `after_timestamp_ms` that aligns to the
    /// configured cadence starting from the last recorded margin check
    /// (initially the initiation timestamp).
    pub fn next_margin_check_after(&self, after_timestamp_ms: u64) -> Option<u64> {
        let freq_ms = self.governance.margin_frequency_millis()?;

        let start = self.last_margin_check_timestamp_ms;
        if after_timestamp_ms < start {
            return Some(start);
        }

        let elapsed = after_timestamp_ms.saturating_sub(start);
        let periods_elapsed = elapsed / freq_ms;
        let next_period = periods_elapsed.saturating_add(1);
        start.checked_add(freq_ms.saturating_mul(next_period))
    }

    /// Determine whether a margin check is due at the supplied timestamp.
    ///
    /// Returns `false` when margining is disabled. When margining is enabled,
    /// a check is considered due at or after the next scheduled cadence
    /// boundary following the last recorded margin check.
    pub fn is_margin_check_due(&self, at_timestamp_ms: u64) -> bool {
        let Some(next_due) = self.next_margin_check_after(self.last_margin_check_timestamp_ms)
        else {
            return false;
        };
        at_timestamp_ms >= next_due
    }

    /// Record completion of a margin check at the supplied timestamp.
    pub fn record_margin_check(&mut self, timestamp_ms: u64) {
        self.last_margin_check_timestamp_ms = timestamp_ms;
    }
}

impl Identifiable for RepoAgreement {
    type Id = RepoAgreementId;

    fn id(&self) -> &Self::Id {
        &self.id
    }
}

/// Common re-exports for repo-related types.
pub mod prelude {
    pub use super::{
        RepoAgreement, RepoAgreementId, RepoCashLeg, RepoCollateralLeg, RepoGovernance,
    };
}

#[cfg(feature = "json")]
impl JsonKeyCodec for RepoAgreementId {
    fn encode_json_key(&self, out: &mut String) {
        norito::json::write_json_string(&self.to_string(), out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, norito::json::Error> {
        encoded
            .parse()
            .map_err(|err| norito::json::Error::Message(format!("{err}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE_ID_STR: &str =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland";
    const BOB_ID_STR: &str =
        "ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016@wonderland";

    fn sample_agreement(initiated_ms: u64, margin_frequency_secs: u64) -> RepoAgreement {
        let initiator: AccountId = ALICE_ID_STR.parse().unwrap();
        let counterparty: AccountId = BOB_ID_STR.parse().unwrap();
        let cash_leg = RepoCashLeg {
            asset_definition_id: "usd#wonderland".parse().unwrap(),
            quantity: Numeric::from(1_000u32),
        };
        let collateral_leg =
            RepoCollateralLeg::new("bond#wonderland".parse().unwrap(), Numeric::from(1_100u32));
        RepoAgreement::new(
            "daily".parse().unwrap(),
            initiator,
            counterparty,
            cash_leg,
            collateral_leg,
            250,
            initiated_ms + 86_400_000,
            initiated_ms,
            RepoGovernance::with_defaults(1_500, margin_frequency_secs),
            None,
        )
    }

    #[test]
    fn margin_schedule_disabled_when_frequency_zero() {
        let agreement = sample_agreement(1_000, 0);
        assert!(agreement.next_margin_check_after(1_000).is_none());
        assert!(!agreement.is_margin_check_due(1_000));
    }

    #[test]
    fn margin_schedule_returns_start_before_initiation() {
        let agreement = sample_agreement(10_000, 60);
        assert_eq!(
            agreement.next_margin_check_after(5_000),
            Some(10_000),
            "margin checks should start at the initiation timestamp"
        );
    }

    #[test]
    fn margin_schedule_advances_in_cadence() {
        let agreement = sample_agreement(10_000, 60);
        // 60 seconds cadence = 60_000 ms
        assert_eq!(
            agreement.next_margin_check_after(10_000),
            Some(70_000),
            "first check after initiation should be one cadence later"
        );
        assert_eq!(
            agreement.next_margin_check_after(70_000),
            Some(130_000),
            "subsequent checks follow the cadence"
        );
        assert!(agreement.is_margin_check_due(70_000));
        assert!(agreement.is_margin_check_due(70_001));
        assert!(!agreement.is_margin_check_due(69_999));
    }

    #[test]
    fn record_margin_check_updates_schedule() {
        let mut agreement = sample_agreement(10_000, 60);
        agreement.record_margin_check(70_000);
        assert_eq!(agreement.next_margin_check_after(70_000), Some(130_000));
        assert!(!agreement.is_margin_check_due(129_999));
        assert!(agreement.is_margin_check_due(130_000));
        assert!(agreement.is_margin_check_due(130_001));
    }
}
