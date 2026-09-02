//! Public lane staking records and reward metadata.
use crate::{account::AccountId, asset::AssetId, metadata::Metadata, nexus::LaneId, peer::PeerId};
use iroha_crypto::Hash;
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::collections::BTreeMap;
/// Snapshot of a validator registered for a public Nexus lane.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct PublicLaneRewardShare {
    /// Account that receives the payout.
    pub account: AccountId,
    /// Role applied when allocating the reward (validator vs nominee).
    pub role: PublicLaneRewardRole,
    /// Amount of rewards allocated to the account.
    pub amount: Quantity,
}
/// Role marker for a reward share.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub enum PublicLaneRewardRole {
    /// Validator portion of the reward.
    Validator,
    /// Nominator/delegator portion of the reward.
    Nominator,
}
/// Ledger entry capturing the outcome of a reward distribution for auditing.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct PublicLanePendingReward {
    /// Lane identifier.
    pub lane_id: LaneId,
    /// Account that will receive the payout.
    pub account: AccountId,
    /// Asset identifier used for the payout.
    pub asset: AssetId,
    /// Latest epoch that was already claimed (0 when none).
    pub last_claimed_epoch: u64,
    /// Latest epoch included in `amount`.
    pub pending_through_epoch: u64,
    /// Total amount available to claim up to `pending_through_epoch`.
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
