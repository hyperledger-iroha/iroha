//! Public lane staking records and reward metadata.
use crate::{account::AccountId, asset::AssetId};
use iroha_crypto::Hash;
use iroha_model_base::metadata::Metadata;
use iroha_model_base::peer::PeerId;
use iroha_model_base::topology::LaneId;
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::collections::BTreeMap;
/// Maximum reward records processed by one signed claim.
pub const MAX_PUBLIC_LANE_REWARD_CLAIM_RECORDS: usize = 64;
/// Maximum exact custody sources read or changed by one signed reward claim.
pub const MAX_PUBLIC_LANE_REWARD_CLAIM_SOURCES: usize = 64;

/// Signature scope for exact staking effects, including network-independent genesis templates.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneMonetaryScopeV1")]
#[norito(tag = "scope", content = "value", deny_unknown_fields)]
pub enum PublicLaneMonetaryScopeV1 {
    /// Only the authenticated genesis bootstrap may execute this template.
    Genesis,
    /// Ordinary execution on exactly this genesis-derived network.
    Network(crate::NetworkId),
}

/// Exact eligibility boundary for a new validator registration.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneMonetaryRegistrationV1")]
#[norito(deny_unknown_fields)]
pub struct PublicLaneMonetaryRegistrationV1 {
    /// Inclusive first height of the new validator tenure.
    pub activation_height: u64,
}

/// Exact validator tenure and peer binding authorized for additional stake.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneMonetaryBondV1")]
#[norito(deny_unknown_fields)]
pub struct PublicLaneMonetaryBondV1 {
    /// Inclusive first height of the validator tenure.
    pub activation_height: u64,
    /// Peer binding observed by the staker.
    pub peer_id: PeerId,
}

/// Exact retained withdrawal request authorized for release.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneMonetaryUnbondV1")]
#[norito(deny_unknown_fields)]
pub struct PublicLaneMonetaryUnbondV1 {
    /// Inclusive first height of the validator tenure.
    pub activation_height: u64,
    /// Domain-separated commitment to the exact pending withdrawal record.
    pub request_hash: Hash,
}

/// Exact validator tenure and eligible exposure authorized for a privileged slash.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneMonetarySlashV1")]
#[norito(deny_unknown_fields)]
pub struct PublicLaneMonetarySlashV1 {
    /// Inclusive first height of the validator tenure.
    pub activation_height: u64,
    /// Complete eligible custody exposure before this slash.
    pub slashable_exposure: Quantity,
}

/// Exact state identity to which a monetary staking instruction is restricted.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneMonetaryPreconditionV1")]
#[norito(tag = "operation", content = "value", deny_unknown_fields)]
pub enum PublicLaneMonetaryPreconditionV1 {
    /// A new registration at this exact eligibility boundary.
    Registration(PublicLaneMonetaryRegistrationV1),
    /// Additional stake for the currently bound validator tenure.
    Bond(PublicLaneMonetaryBondV1),
    /// One retained withdrawal request, including its amount and liability window.
    Unbond(PublicLaneMonetaryUnbondV1),
    /// One privileged slash against an exact tenure and slashable exposure.
    Slash(PublicLaneMonetarySlashV1),
}

/// Signed exact transfer and operation-specific custody change for a staking action.
///
/// Registration and bonding reserve `amount` at `destination_asset`; withdrawal
/// and slashing release `amount` at `source_asset`. The instruction determines
/// this direction, so the caller cannot select additional reserve effects.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneMonetaryPlanV1")]
#[norito(deny_unknown_fields)]
pub struct PublicLaneMonetaryPlanV1 {
    /// Genesis-derived network identity covered by the submitting signature.
    pub network_scope: PublicLaneMonetaryScopeV1,
    /// Last block height at which this plan may execute, inclusive.
    pub valid_until_height: u64,
    /// Exact source asset, including its dataspace scope.
    pub source_asset: AssetId,
    /// Exact destination asset, including its dataspace scope.
    pub destination_asset: AssetId,
    /// Exact quantity moved and added to or removed from protocol custody.
    pub amount: Quantity,
    /// Exact operation and retained state being authorized.
    pub precondition: PublicLaneMonetaryPreconditionV1,
}
impl PublicLaneMonetaryPlanV1 {
    /// Build the exact prefunded registration template authenticated by genesis.
    ///
    /// Both assets and the amount remain explicit. Core accepts this scope only
    /// while applying initial genesis, where the first eligibility height is one.
    #[must_use]
    pub fn genesis_registration(
        source_asset: AssetId,
        destination_asset: AssetId,
        amount: Quantity,
    ) -> Self {
        Self {
            network_scope: PublicLaneMonetaryScopeV1::Genesis,
            valid_until_height: 1,
            source_asset,
            destination_asset,
            amount,
            precondition: PublicLaneMonetaryPreconditionV1::Registration(
                PublicLaneMonetaryRegistrationV1 {
                    activation_height: 1,
                },
            ),
        }
    }

    /// Check state-independent invariants before collecting signed monetary effects.
    #[must_use]
    pub fn has_canonical_shape(&self) -> bool {
        self.valid_until_height > 0
            && !self.amount.is_zero()
            && self.source_asset.definition() == self.destination_asset.definition()
            && self.source_asset.scope() == self.destination_asset.scope()
            && match &self.precondition {
                PublicLaneMonetaryPreconditionV1::Registration(value) => {
                    value.activation_height > 0
                }
                PublicLaneMonetaryPreconditionV1::Bond(value) => value.activation_height > 0,
                PublicLaneMonetaryPreconditionV1::Unbond(value) => value.activation_height > 0,
                PublicLaneMonetaryPreconditionV1::Slash(value) => {
                    value.activation_height > 0 && value.slashable_exposure >= self.amount
                }
            }
    }
}

/// Per-recipient chronological processing progress through a lane's reward records.
///
/// Unpaid quantities live in separate exact-source accrual rows, so historical
/// dust across many custody sources never requires an unbounded claim operation.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneRewardClaimStateV1")]
#[norito(deny_unknown_fields)]
pub struct PublicLaneRewardClaimStateV1 {
    /// Last processed reward epoch, including epochs with no recipient entitlement.
    #[norito(required)]
    pub through_epoch: Option<u64>,
}

/// Immutable reward record covered by the recipient's signed processing plan.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneRewardRecordRefV1")]
#[norito(deny_unknown_fields)]
pub struct PublicLaneRewardRecordRefV1 {
    /// Reward epoch within the instruction's lane.
    pub epoch: u64,
    /// Domain-separated commitment to the complete canonical reward record.
    pub record_hash: Hash,
}

/// Exact prior accrual and payment from one retained reward custody source.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneRewardClaimSourceV1")]
#[norito(deny_unknown_fields)]
pub struct PublicLaneRewardClaimSourceV1 {
    /// Immutable custody asset retained by reward records and accrual rows.
    pub source_asset: AssetId,
    /// Recipient asset with the identical definition and dataspace scope.
    pub destination_asset: AssetId,
    /// Exact previous unpaid accrual; absence denotes no retained source row.
    #[norito(required)]
    pub expected_accrued: Option<Quantity>,
    /// Exact payment and reserve release; zero retains all accrued funds as dust.
    pub payout: Quantity,
}

/// Bounded signed reward processing and payment plan.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneRewardClaimPlanV1")]
#[norito(deny_unknown_fields)]
pub struct PublicLaneRewardClaimPlanV1 {
    /// Exact network or authenticated genesis template scope.
    pub network_scope: PublicLaneMonetaryScopeV1,
    /// Last block height at which this plan may execute, inclusive.
    pub valid_until_height: u64,
    /// Exact retained chronological processing cursor observed by the recipient.
    #[norito(required)]
    pub expected_state: Option<PublicLaneRewardClaimStateV1>,
    /// At most 64 consecutive reward records after the retained processing cursor.
    pub records: Vec<PublicLaneRewardRecordRefV1>,
    /// At most 64 touched or previously accrued sources, in exact asset order.
    pub sources: Vec<PublicLaneRewardClaimSourceV1>,
}
impl PublicLaneRewardClaimPlanV1 {
    /// Check bounded canonical ordering and exact payment invariants.
    #[must_use]
    pub fn has_canonical_shape(&self, recipient: &AccountId) -> bool {
        self.valid_until_height > 0
            && self.records.len() <= MAX_PUBLIC_LANE_REWARD_CLAIM_RECORDS
            && self.sources.len() <= MAX_PUBLIC_LANE_REWARD_CLAIM_SOURCES
            && self
                .records
                .windows(2)
                .all(|pair| pair[0].epoch < pair[1].epoch)
            && self.records.first().is_none_or(|record| {
                self.expected_state
                    .as_ref()
                    .and_then(|state| state.through_epoch)
                    .is_none_or(|epoch| epoch < record.epoch)
            })
            && self
                .sources
                .windows(2)
                .all(|pair| pair[0].source_asset < pair[1].source_asset)
            && self.sources.iter().all(|source| {
                source.source_asset.definition() == source.destination_asset.definition()
                    && source.source_asset.scope() == source.destination_asset.scope()
                    && source.destination_asset.account() == recipient
                    && source
                        .expected_accrued
                        .as_ref()
                        .is_none_or(|amount| !amount.is_zero())
            })
    }
}

/// Commit to an immutable reward record in a distinct staking protocol domain.
///
/// # Errors
/// Returns an error if canonical Norito encoding cannot be produced.
pub fn public_lane_reward_record_commitment(
    record: &PublicLaneRewardRecord,
) -> Result<Hash, norito::Error> {
    let mut bytes = b"iroha.staking.reward_record.v1\0".to_vec();
    bytes.extend_from_slice(&norito::encode_canonical(record)?);
    Ok(Hash::new(bytes))
}

/// Commit to every field of a retained withdrawal request in a distinct protocol domain.
///
/// # Errors
/// Returns an error if the canonical Norito encoding cannot be produced.
pub fn public_lane_unbonding_commitment(
    request: &PublicLaneUnbonding,
) -> Result<Hash, norito::Error> {
    let mut bytes = b"iroha.staking.pending_unbond.v1\0".to_vec();
    bytes.extend_from_slice(&norito::encode_canonical(request)?);
    Ok(Hash::new(bytes))
}

/// Snapshot of a validator registered for a public Nexus lane.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneValidatorRecord")]
pub struct PublicLaneValidatorRecord {
    /// Lane that the validator services.
    pub lane_id: LaneId,
    /// Account that owns validator authority for the lane.
    pub validator: AccountId,
    /// Peer identity that participates in consensus and receives routed traffic.
    pub peer_id: PeerId,
    /// Canonical self-stake account; must equal `validator`.
    pub stake_account: AccountId,
    /// Total bonded stake attributed to the validator (self + nominators).
    pub total_stake: Quantity,
    /// Portion of stake supplied by the validator.
    pub self_stake: Quantity,
    /// Optional slot for metadata (commission, endpoints, jurisdiction flags, etc.).
    pub metadata: Metadata,
    /// Current lifecycle state of the validator.
    pub status: PublicLaneValidatorStatus,
    /// Inclusive first height at which this validator may be elected.
    ///
    /// Pending validators carry this scheduled boundary before their lifecycle
    /// status is promoted, so a finalized boundary snapshot can project them
    /// without depending on block-local execution order.
    pub activation_height: u64,
    /// Exclusive first height no longer covered by this validator binding.
    ///
    /// Retained custody records preserve this boundary after exit or slash so
    /// evidence can be matched to the exact historical tenure.
    pub deactivation_height: Option<u64>,
    /// Epoch identifier that last produced a reward payout.
    pub last_reward_epoch: Option<u64>,
}
/// Lifecycle state for a validator entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneValidatorStatus")]
pub enum PublicLaneValidatorStatus {
    /// Validator is scheduled for election eligibility at the exact payload height.
    PendingActivation(u64),
    /// Validator participates in consensus for the target lane.
    Active,
    /// Validator is exiting and the bonded stake is being unlocked.
    Exiting(u64),
    /// Validator exit processing is complete.
    ///
    /// Historical authority and stake custody remain governed by the exact
    /// retained `[activation_height, deactivation_height)` tenure and their
    /// independent release boundaries.
    Exited,
    /// Validator was slashed; slash ids help correlate telemetry/audits.
    Slashed(Hash),
}
/// Per-staker bonded stake record.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneStakeShare")]
pub struct PublicLaneStakeShare {
    /// Lane serviced by the validator.
    pub lane_id: LaneId,
    /// Validator the stake is delegated to.
    pub validator: AccountId,
    /// Account that provided the stake.
    pub staker: AccountId,
    /// Amount of stake currently bonded.
    pub bonded: Quantity,
    /// Pending unbonding requests keyed by client-supplied identifier.
    pub pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>,
    /// Optional metadata for dashboards or wallet hints.
    pub metadata: Metadata,
}
/// Pending unbond request tracked on-ledger.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneUnbonding")]
pub struct PublicLaneUnbonding {
    /// Deterministic identifier supplied by the submitter.
    pub request_id: Hash,
    /// Amount scheduled for release.
    pub amount: Quantity,
    /// Unix timestamp (ms) when the withdrawal can be finalised.
    pub release_at_ms: u64,
    /// Inclusive final offence height underwritten by this retained custody.
    pub slashable_through_height: u64,
    /// Earliest block whose post-finality transaction phase may release the funds.
    ///
    /// Consensus penalties run before ordinary transactions at this height, so
    /// evidence for `slashable_through_height` admitted at the end of the
    /// configured horizon still has its complete slashing-delay window.
    pub liability_release_height: u64,
}
/// Aggregated reward share emitted for a validator or delegator.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneRewardShare")]
pub struct PublicLaneRewardShare {
    /// Account that receives the payout.
    pub account: AccountId,
    /// Role applied when allocating the reward (validator vs nominee).
    pub role: PublicLaneRewardRole,
    /// Amount of rewards allocated to the account.
    pub amount: Quantity,
}
/// Role marker for a reward share.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneRewardRole")]
pub enum PublicLaneRewardRole {
    /// Validator portion of the reward.
    Validator,
    /// Nominator/delegator portion of the reward.
    Nominator,
}
impl PublicLaneRewardRole {
    fn json_name(self) -> &'static str {
        match self {
            Self::Validator => "Validator",
            Self::Nominator => "Nominator",
        }
    }
}
impl norito::json::FastJsonWrite for PublicLaneRewardRole {
    fn write_json(&self, out: &mut String) {
        norito::json::write_json_string(self.json_name(), out);
    }

    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(self.json_name(), out)
    }
}
impl norito::json::JsonDeserialize for PublicLaneRewardRole {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        match parser.parse_string()?.as_str() {
            "Validator" => Ok(Self::Validator),
            "Nominator" => Ok(Self::Nominator),
            _ => Err(norito::json::Error::Message(
                "reward role must be Validator or Nominator".to_owned(),
            )),
        }
    }
}
/// Ledger entry capturing the outcome of a reward distribution for auditing.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLaneRewardRecord")]
pub struct PublicLaneRewardRecord {
    /// Lane that produced the reward.
    pub lane_id: LaneId,
    /// Epoch or slot identifier recorded by consensus.
    pub epoch: u64,
    /// Asset identifier used for payouts.
    pub asset: AssetId,
    /// Total reward minted or transferred into the pool.
    pub total_reward: Quantity,
    /// Individual reward shares emitted in this payout.
    pub shares: Vec<PublicLaneRewardShare>,
    /// Optional metadata for auditors (tx hashes, ceremony notes, etc.).
    pub metadata: Metadata,
}
/// Pending reward summary for an account and lane.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::staking::PublicLanePendingReward")]
pub struct PublicLanePendingReward {
    /// Lane identifier.
    pub lane_id: LaneId,
    /// Account that will receive the payout.
    pub account: AccountId,
    /// Exact custody source, including its owner and balance scope.
    pub asset: AssetId,
    /// Last processed reward epoch, independent of payment; absence includes epoch zero.
    pub processed_through_epoch: Option<u64>,
    /// Latest processed or newly included epoch covered by this unpaid projection.
    pub pending_through_epoch: u64,
    /// Total unpaid entitlement, including retained accrual, before the dust threshold.
    pub amount: Quantity,
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_primitives::numeric::Numeric;
    #[derive(Encode)]
    struct ForgedPublicLaneStakeShare {
        lane_id: LaneId,
        validator: AccountId,
        staker: AccountId,
        bonded: Numeric,
        pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>,
        metadata: Metadata,
    }
    #[derive(Encode)]
    struct ForgedPublicLaneRewardShare {
        account: AccountId,
        role: PublicLaneRewardRole,
        amount: Numeric,
    }
    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked durable staking fixture account keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    #[test]
    fn reward_shares_roundtrip_norito_json_with_exact_quantities_and_roles() {
        for (role, literal) in [
            (PublicLaneRewardRole::Validator, "\"Validator\""),
            (PublicLaneRewardRole::Nominator, "\"Nominator\""),
        ] {
            let role_json = norito::json::to_json(&role).expect("serialize reward role");
            assert_eq!(role_json, literal);
            assert_eq!(
                norito::json::from_str::<PublicLaneRewardRole>(&role_json)
                    .expect("deserialize reward role"),
                role
            );
            let share = PublicLaneRewardShare {
                account: account(0x44),
                role,
                amount: "9007199254740993.000000001".parse().expect("exact reward"),
            };
            let json = norito::json::to_json(&share).expect("serialize reward share");
            let decoded: PublicLaneRewardShare =
                norito::json::from_str(&json).expect("deserialize reward share");
            assert_eq!(decoded, share);
            assert_eq!(decoded.encode(), share.encode());
        }
    }
    #[test]
    fn reward_share_json_rejects_negative_quantities() {
        let share = PublicLaneRewardShare {
            account: account(0x45),
            role: PublicLaneRewardRole::Validator,
            amount: Quantity::from(17_u64),
        };
        let mut value = norito::json::to_value(&share).expect("serialize reward share");
        value
            .as_object_mut()
            .expect("reward share object")
            .insert("amount".into(), norito::json::Value::String("-1".into()));
        assert!(norito::json::from_value::<PublicLaneRewardShare>(value).is_err());
    }
    #[test]
    fn reward_role_json_rejects_unknown_and_non_string_values() {
        for value in ["\"validator\"", "\"Delegator\"", "null", "0", "{}"] {
            assert!(norito::json::from_str::<PublicLaneRewardRole>(value).is_err());
        }
    }
    #[test]
    fn negative_numeric_payloads_cannot_decode_as_durable_staking_quantities() {
        let stake = ForgedPublicLaneStakeShare {
            lane_id: LaneId::SINGLE,
            validator: account(0x41),
            staker: account(0x42),
            bonded: Numeric::new(-1_i32, 0),
            pending_unbonds: BTreeMap::new(),
            metadata: Metadata::default(),
        };
        let encoded = stake.encode();
        assert!(
            PublicLaneStakeShare::decode(&mut encoded.as_slice()).is_err(),
            "a negative signed payload must not decode as durable bonded stake"
        );
        let reward = ForgedPublicLaneRewardShare {
            account: account(0x43),
            role: PublicLaneRewardRole::Validator,
            amount: Numeric::new(-1_i32, 0),
        };
        let encoded = reward.encode();
        assert!(
            PublicLaneRewardShare::decode(&mut encoded.as_slice()).is_err(),
            "a negative signed payload must not decode as a durable staking reward"
        );
    }
}

#[cfg(test)]
mod captured_staking_schema_tests;

#[cfg(test)]
mod monetary_codec_tests;
