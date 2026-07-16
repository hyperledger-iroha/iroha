//! On-chain Nexus fee sponsor policy model.

use std::{collections::BTreeSet, fmt, str::FromStr};

use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::{AccountId, ParsedAccountId},
    asset::AssetDefinitionId,
    name::Name,
    smart_contract::{ContractAddress, ContractAlias},
};

use super::DataSpaceId;

/// Error returned while parsing [`FeeSponsorPolicyId`] literals.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum FeeSponsorPolicyIdParseError {
    /// The policy literal must use `sponsor/policy`.
    #[error("fee sponsor policy literal must use `sponsor/policy`")]
    InvalidFormat,
    /// Sponsor account literal is invalid.
    #[error("invalid sponsor account: {0}")]
    InvalidSponsor(String),
    /// Policy name is invalid.
    #[error("invalid policy name: {0}")]
    InvalidName(String),
}

/// Stable on-chain identifier for one fee sponsor policy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct FeeSponsorPolicyId {
    /// Sponsor account that owns and pays through the policy.
    pub sponsor: AccountId,
    /// Sponsor-local policy name.
    pub name: Name,
}

impl FeeSponsorPolicyId {
    /// Construct a new sponsor policy identifier.
    #[must_use]
    pub const fn new(sponsor: AccountId, name: Name) -> Self {
        Self { sponsor, name }
    }
}

impl fmt::Display for FeeSponsorPolicyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.sponsor, self.name)
    }
}

impl FromStr for FeeSponsorPolicyId {
    type Err = FeeSponsorPolicyIdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        let (sponsor, name) = trimmed
            .rsplit_once('/')
            .ok_or(FeeSponsorPolicyIdParseError::InvalidFormat)?;
        let sponsor = AccountId::parse_encoded(sponsor.trim())
            .map(ParsedAccountId::into_account_id)
            .map_err(|err| FeeSponsorPolicyIdParseError::InvalidSponsor(err.to_string()))?;
        let name = Name::from_str(name.trim())
            .map_err(|err| FeeSponsorPolicyIdParseError::InvalidName(err.to_string()))?;
        Ok(Self::new(sponsor, name))
    }
}

/// Executable class selectable by a fee sponsor policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FeeSponsorExecutableKind {
    /// Ordered native instruction batch.
    Instructions,
    /// Deployed contract invocation.
    ContractCall,
    /// Raw IVM bytecode transaction.
    Ivm,
    /// Proved IVM bytecode with deterministic native overlay.
    IvmProved,
}

/// Effect of a policy rule when it matches a transaction operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(
    tag = "effect",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FeeSponsorRuleEffect {
    /// Permit the matched operation if no deny rule also matches.
    Allow,
    /// Reject the matched operation even if an allow rule also matches.
    Deny,
}

/// Contract target selector for sponsored contract calls.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct FeeSponsorContractSelector {
    /// Optional stable contract alias that must match the target.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub contract_alias: Option<ContractAlias>,
    /// Optional concrete contract address that must match the target.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub contract_address: Option<ContractAddress>,
    /// Optional entrypoint names. Empty means all entrypoints for the selected target.
    #[norito(default)]
    pub entrypoints: BTreeSet<String>,
}

/// One ordered policy rule. Empty selector sets act as wildcards.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct FeeSponsorRule {
    /// Allow or deny effect.
    pub effect: FeeSponsorRuleEffect,
    /// Optional maximum total transaction fee for this allow rule.
    ///
    /// This lets one policy safely cover operations with materially different
    /// fee envelopes (for example, one native retail transfer versus a bounded
    /// contract call). It is ignored for deny rules; deny selectors always
    /// override matching allow rules.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub max_fee: Option<Quantity>,
    /// Optional data-space selector. Empty means all data spaces.
    #[norito(default)]
    pub dataspaces: BTreeSet<DataSpaceId>,
    /// Optional executable-kind selector. Empty means all executable kinds.
    #[norito(default)]
    pub executable_kinds: BTreeSet<FeeSponsorExecutableKind>,
    /// Optional native instruction wire IDs. Empty means all native instruction IDs.
    #[norito(default)]
    pub instruction_wire_ids: BTreeSet<String>,
    /// Optional asset-definition IDs for native asset transfers.
    ///
    /// A non-empty set matches only `iroha.transfer` operations whose concrete
    /// transfer variant is an asset-quantity transfer for one of these
    /// definitions. Domain, asset-definition, and NFT ownership transfers do
    /// not match this selector even though they share the same wire ID.
    #[norito(default)]
    pub asset_transfer_definition_ids: BTreeSet<AssetDefinitionId>,
    /// Optional contract-call selectors. Empty means all contract calls.
    #[norito(default)]
    pub contract_selectors: Vec<FeeSponsorContractSelector>,
}

impl FeeSponsorRule {
    /// Construct a new rule with no selectors.
    #[must_use]
    pub fn new(effect: FeeSponsorRuleEffect) -> Self {
        Self {
            effect,
            max_fee: None,
            dataspaces: BTreeSet::new(),
            executable_kinds: BTreeSet::new(),
            instruction_wire_ids: BTreeSet::new(),
            asset_transfer_definition_ids: BTreeSet::new(),
            contract_selectors: Vec::new(),
        }
    }
}

/// Sponsor-owned policy that decides which transactions the sponsor will pay for.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct FeeSponsorPolicy {
    /// Policy identifier.
    pub id: FeeSponsorPolicyId,
    /// Whether the policy may currently authorize sponsorship.
    pub enabled: bool,
    /// Optional policy-local maximum fee. The global sponsor cap still applies.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub max_fee: Option<Quantity>,
    /// Ordered rules. Deny rules override allow rules during evaluation.
    #[norito(default)]
    pub rules: Vec<FeeSponsorRule>,
}

impl FeeSponsorPolicy {
    /// Construct a disabled policy with no rules.
    #[must_use]
    pub const fn new(id: FeeSponsorPolicyId) -> Self {
        Self {
            id,
            enabled: false,
            max_fee: None,
            rules: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_primitives::numeric::Numeric;
    use norito::codec::{Decode as _, Encode as _};

    use super::*;

    #[derive(Encode)]
    struct ForgedFeeSponsorPolicy {
        id: FeeSponsorPolicyId,
        enabled: bool,
        max_fee: Option<Numeric>,
        rules: Vec<FeeSponsorRule>,
    }

    #[derive(Encode)]
    struct ForgedFeeSponsorRule {
        effect: FeeSponsorRuleEffect,
        max_fee: Option<Numeric>,
        dataspaces: BTreeSet<DataSpaceId>,
        executable_kinds: BTreeSet<FeeSponsorExecutableKind>,
        instruction_wire_ids: BTreeSet<String>,
        asset_transfer_definition_ids: BTreeSet<AssetDefinitionId>,
        contract_selectors: Vec<FeeSponsorContractSelector>,
    }

    fn sponsor_account() -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![0x53; 32], Algorithm::Ed25519)
            .expect("fixture seed derives Ed25519 keypair");
        AccountId::new(keypair.public_key().clone())
    }

    #[test]
    fn policy_id_display_roundtrips() {
        let sponsor = sponsor_account();
        let id = FeeSponsorPolicyId::new(
            sponsor,
            "retail_transfers"
                .parse::<Name>()
                .expect("valid policy name"),
        );

        let literal = id.to_string();
        assert_eq!(
            literal
                .parse::<FeeSponsorPolicyId>()
                .expect("display literal parses"),
            id
        );
        assert_eq!(
            format!("  {literal}  ")
                .parse::<FeeSponsorPolicyId>()
                .expect("trimmed literal parses"),
            id
        );
    }

    #[test]
    fn policy_id_parse_rejects_invalid_literals() {
        assert_eq!(
            "missing-separator".parse::<FeeSponsorPolicyId>(),
            Err(FeeSponsorPolicyIdParseError::InvalidFormat)
        );
        assert!(matches!(
            "not-an-account/default".parse::<FeeSponsorPolicyId>(),
            Err(FeeSponsorPolicyIdParseError::InvalidSponsor(_))
        ));

        let sponsor = sponsor_account();
        assert!(matches!(
            format!("{sponsor}/").parse::<FeeSponsorPolicyId>(),
            Err(FeeSponsorPolicyIdParseError::InvalidName(_))
        ));
    }

    #[test]
    fn constructors_default_to_locked_down_policy_shapes() {
        let rule = FeeSponsorRule::new(FeeSponsorRuleEffect::Deny);
        assert_eq!(rule.effect, FeeSponsorRuleEffect::Deny);
        assert_eq!(rule.max_fee, None);
        assert!(rule.dataspaces.is_empty());
        assert!(rule.executable_kinds.is_empty());
        assert!(rule.instruction_wire_ids.is_empty());
        assert!(rule.contract_selectors.is_empty());

        let id = FeeSponsorPolicyId::new(
            sponsor_account(),
            "default".parse::<Name>().expect("valid policy name"),
        );
        let policy = FeeSponsorPolicy::new(id.clone());
        assert_eq!(policy.id, id);
        assert!(!policy.enabled);
        assert_eq!(policy.max_fee, None);
        assert!(policy.rules.is_empty());
    }

    #[test]
    fn negative_numeric_payload_cannot_decode_as_sponsor_fee_cap() {
        let forged = ForgedFeeSponsorPolicy {
            id: FeeSponsorPolicyId::new(
                sponsor_account(),
                "negative_cap".parse().expect("valid policy name"),
            ),
            enabled: true,
            max_fee: Some(Numeric::new(-1_i32, 0)),
            rules: Vec::new(),
        };
        let encoded = forged.encode();

        assert!(
            FeeSponsorPolicy::decode(&mut encoded.as_slice()).is_err(),
            "a negative signed payload must not decode as a sponsor fee cap"
        );
    }

    #[test]
    fn negative_numeric_payload_cannot_decode_as_rule_fee_cap() {
        let forged = ForgedFeeSponsorRule {
            effect: FeeSponsorRuleEffect::Allow,
            max_fee: Some(Numeric::new(-1_i32, 2)),
            dataspaces: BTreeSet::new(),
            executable_kinds: BTreeSet::new(),
            instruction_wire_ids: BTreeSet::new(),
            asset_transfer_definition_ids: BTreeSet::new(),
            contract_selectors: Vec::new(),
        };
        let encoded = forged.encode();

        assert!(
            FeeSponsorRule::decode(&mut encoded.as_slice()).is_err(),
            "a negative signed payload must not decode as a rule-level sponsor fee cap"
        );
    }

    #[test]
    fn rule_fee_cap_json_roundtrips_and_rejects_negative_or_unknown_fields() {
        let mut rule = FeeSponsorRule::new(FeeSponsorRuleEffect::Allow);
        rule.max_fee = Some("0.01".parse().expect("valid rule fee cap"));
        rule.dataspaces.insert(DataSpaceId::new(10));
        rule.executable_kinds
            .insert(FeeSponsorExecutableKind::Instructions);
        rule.instruction_wire_ids
            .insert("iroha.transfer".to_owned());
        rule.asset_transfer_definition_ids.insert(
            "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
                .parse()
                .expect("canonical asset definition id"),
        );

        let json = norito::json::to_json(&rule).expect("serialize rule fee cap");
        let decoded: FeeSponsorRule =
            norito::json::from_str(&json).expect("deserialize rule fee cap");
        assert_eq!(decoded, rule);
        assert!(json.contains("\"max_fee\":\"0.01\""));
        assert!(json.contains("\"asset_transfer_definition_ids\""));

        let negative = json.replacen("\"0.01\"", "\"-0.01\"", 1);
        assert!(
            norito::json::from_str::<FeeSponsorRule>(&negative).is_err(),
            "negative JSON rule fee caps must fail closed"
        );

        let unknown = json.replacen('{', "{\"unknown_cap\":\"5\",", 1);
        assert!(
            norito::json::from_str::<FeeSponsorRule>(&unknown).is_err(),
            "unknown rule fields must fail closed"
        );
    }
}
