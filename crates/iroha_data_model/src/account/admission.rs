//! Global account admission policy.
//!
//! Iroha accounts are explicit on-chain entities identified by canonical I105 account IDs.
//! This module defines a chain-level policy that controls *implicit account creation on receipt*
//! for Ethereum/Bitcoin-like UX: sending/minting assets (or transferring NFTs) to a
//! never-before-seen `AccountId` can auto-create the corresponding account object when the
//! global policy allows it.

use std::collections::BTreeMap;

use iroha_primitives::{json::Json, numeric::Numeric};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    parameter::{CustomParameter, CustomParameterId},
    prelude::{AccountId, AssetDefinitionId, RoleId},
};

/// Legacy domain metadata key previously used for account admission policy payloads.
///
/// Account admission is now configured globally via
/// [`AccountAdmissionPolicy::PARAMETER_ID_STR`], and this key is retained only for compatibility
/// with external tooling that may still reference the literal.
pub const ACCOUNT_ADMISSION_POLICY_METADATA_KEY: &str = "iroha:account_admission_policy";

/// Default cap for the number of implicit accounts that may be created in a single transaction.
pub const DEFAULT_MAX_IMPLICIT_ACCOUNT_CREATIONS_PER_TX: u32 = 16;

/// Admission mode for implicit account creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "mode", content = "value", rename_all = "snake_case")]
pub enum AccountAdmissionMode {
    /// Only explicit `Register<Account>` may create accounts.
    ExplicitOnly,
    /// Receipt-like operations may implicitly create destination accounts.
    ImplicitReceive,
}

/// Destination for implicit account creation fees.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "destination", content = "value", rename_all = "snake_case")]
pub enum ImplicitAccountFeeDestination {
    /// Burn the fee.
    Burn,
    /// Deposit the fee into the provided account.
    Account(AccountId),
}

/// Fee charged when creating an account implicitly.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ImplicitAccountCreationFee {
    /// Asset definition used to pay the fee.
    pub asset_definition_id: AssetDefinitionId,
    /// Amount of the fee.
    pub amount: Numeric,
    /// Where the fee should be routed.
    pub destination: ImplicitAccountFeeDestination,
}

/// Chain-level policy controlling whether receipt operations may create accounts implicitly.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct AccountAdmissionPolicy {
    /// Whether implicit account creation is enabled.
    pub mode: AccountAdmissionMode,
    /// Optional per-transaction cap for implicit account creations.
    #[norito(default)]
    pub max_implicit_creations_per_tx: Option<u32>,
    /// Optional per-block cap for implicit account creations.
    #[norito(default)]
    pub max_implicit_creations_per_block: Option<u32>,
    /// Optional fee to charge when creating an account implicitly.
    #[norito(default)]
    pub implicit_creation_fee: Option<ImplicitAccountCreationFee>,
    /// Optional minimum amounts required for the first receipt per asset definition.
    #[norito(default)]
    pub min_initial_amounts: BTreeMap<AssetDefinitionId, Numeric>,
    /// Optional role to assign automatically when an account is created implicitly.
    #[norito(default)]
    pub default_role_on_create: Option<RoleId>,
}

impl Default for AccountAdmissionPolicy {
    fn default() -> Self {
        Self {
            mode: AccountAdmissionMode::ImplicitReceive,
            max_implicit_creations_per_tx: None,
            max_implicit_creations_per_block: None,
            implicit_creation_fee: None,
            min_initial_amounts: BTreeMap::new(),
            default_role_on_create: None,
        }
    }
}

impl AccountAdmissionPolicy {
    /// Identifier of the chain-level custom parameter that provides a default policy.
    pub const PARAMETER_ID_STR: &'static str = "iroha:default_account_admission_policy";

    /// Construct the [`CustomParameterId`] associated with this payload.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid default account admission policy parameter identifier")
    }

    /// Convert the policy into a [`CustomParameter`] (chain parameter).
    #[must_use]
    pub fn into_custom_parameter(self) -> CustomParameter {
        CustomParameter::new(Self::parameter_id(), Json::new(self))
    }

    /// Attempt to decode this policy from a [`CustomParameter`].
    #[must_use]
    pub fn from_custom_parameter(custom: &CustomParameter) -> Option<Self> {
        if custom.id != Self::parameter_id() {
            return None;
        }
        custom.payload().try_into_any_norito::<Self>().ok()
    }

    /// Return the effective per-transaction cap for implicit account creations.
    #[must_use]
    pub fn max_implicit_creations_per_tx(&self) -> u32 {
        self.max_implicit_creations_per_tx
            .unwrap_or(DEFAULT_MAX_IMPLICIT_ACCOUNT_CREATIONS_PER_TX)
    }

    /// Return the per-block cap for implicit account creations, if configured.
    #[must_use]
    pub fn max_implicit_creations_per_block(&self) -> Option<u32> {
        self.max_implicit_creations_per_block
    }

    /// Return the configured fee charged for implicit account creation, if any.
    #[must_use]
    pub fn implicit_creation_fee(&self) -> Option<&ImplicitAccountCreationFee> {
        self.implicit_creation_fee.as_ref()
    }

    /// Return the minimum initial amount required for the given asset definition, if configured.
    #[must_use]
    pub fn min_initial_amount_for(&self, id: &AssetDefinitionId) -> Option<&Numeric> {
        self.min_initial_amounts.get(id)
    }

    /// Return the role to be assigned on implicit creation, if configured.
    #[must_use]
    pub fn default_role_on_create(&self) -> Option<&RoleId> {
        self.default_role_on_create.as_ref()
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use norito::json;

    use super::*;

    #[test]
    fn policy_json_roundtrips_with_tagged_mode() {
        let policy = AccountAdmissionPolicy {
            mode: AccountAdmissionMode::ImplicitReceive,
            max_implicit_creations_per_tx: Some(3),
            max_implicit_creations_per_block: Some(9),
            implicit_creation_fee: None,
            min_initial_amounts: BTreeMap::new(),
            default_role_on_create: None,
        };
        let serialized = json::to_json(&policy).expect("serialize policy to JSON");
        assert!(
            serialized.contains("\"mode\":{\"mode\":\"ImplicitReceive\""),
            "serialized policy must carry tagged enum value: {serialized}"
        );

        let decoded: AccountAdmissionPolicy =
            json::from_str(&serialized).expect("decode policy from JSON");
        assert_eq!(decoded.mode, AccountAdmissionMode::ImplicitReceive);
        assert_eq!(decoded.max_implicit_creations_per_tx, Some(3));
        assert_eq!(decoded.max_implicit_creations_per_block, Some(9));
    }

    #[test]
    fn default_cap_applies_when_unspecified() {
        let policy = AccountAdmissionPolicy {
            mode: AccountAdmissionMode::ImplicitReceive,
            max_implicit_creations_per_tx: None,
            max_implicit_creations_per_block: None,
            implicit_creation_fee: None,
            min_initial_amounts: BTreeMap::new(),
            default_role_on_create: None,
        };
        assert_eq!(
            policy.max_implicit_creations_per_tx(),
            DEFAULT_MAX_IMPLICIT_ACCOUNT_CREATIONS_PER_TX
        );
    }

    #[test]
    fn policy_custom_parameter_roundtrips() {
        let policy = AccountAdmissionPolicy {
            mode: AccountAdmissionMode::ImplicitReceive,
            max_implicit_creations_per_tx: Some(7),
            max_implicit_creations_per_block: Some(11),
            implicit_creation_fee: None,
            min_initial_amounts: BTreeMap::new(),
            default_role_on_create: None,
        };
        let custom = policy.clone().into_custom_parameter();
        assert_eq!(custom.id, AccountAdmissionPolicy::parameter_id());
        let decoded =
            AccountAdmissionPolicy::from_custom_parameter(&custom).expect("decode policy");
        assert_eq!(decoded, policy);
    }

    #[test]
    fn fee_and_minimums_roundtrip() {
        let mut minimums = BTreeMap::new();
        minimums.insert(
            iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            ),
            Numeric::new(5, 0),
        );
        let policy = AccountAdmissionPolicy {
            mode: AccountAdmissionMode::ImplicitReceive,
            max_implicit_creations_per_tx: None,
            max_implicit_creations_per_block: None,
            implicit_creation_fee: Some(ImplicitAccountCreationFee {
                asset_definition_id: iroha_data_model::asset::AssetDefinitionId::new(
                    DomainId::try_new("wonderland", "universal").unwrap(),
                    "rose".parse().unwrap(),
                ),
                amount: Numeric::new(2, 0),
                destination: ImplicitAccountFeeDestination::Burn,
            }),
            min_initial_amounts: minimums,
            default_role_on_create: Some("basic_user".parse().expect("role id")),
        };

        let serialized = json::to_json(&policy).expect("serialize policy");
        let decoded: AccountAdmissionPolicy =
            json::from_str(&serialized).expect("decode policy with fee/minimums");
        assert_eq!(
            decoded.implicit_creation_fee(),
            policy.implicit_creation_fee()
        );
        assert_eq!(
            decoded.min_initial_amount_for(&iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap()
            )),
            policy.min_initial_amount_for(&iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap()
            ))
        );
        assert_eq!(
            decoded.default_role_on_create(),
            policy.default_role_on_create()
        );
    }
}
