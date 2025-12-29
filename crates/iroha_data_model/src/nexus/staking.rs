//! Public lane staking records and reward metadata.

use std::{collections::BTreeMap, string::String};

use iroha_crypto::Hash;
use iroha_primitives::numeric::Numeric;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{account::AccountId, asset::AssetId, metadata::Metadata, nexus::LaneId};

/// Snapshot of a validator registered for a public Nexus lane.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct PublicLaneValidatorRecord {
    /// Lane that the validator services.
    pub lane_id: LaneId,
    /// Account that participates in consensus for the lane.
    pub validator: AccountId,
    /// Account whose balance backs the bonded stake (can differ from `validator`).
    pub stake_account: AccountId,
    /// Total bonded stake attributed to the validator (self + nominators).
    pub total_stake: Numeric,
    /// Portion of stake supplied by the validator.
    pub self_stake: Numeric,
    /// Optional slot for metadata (commission, endpoints, jurisdiction flags, etc.).
    pub metadata: Metadata,
    /// Current lifecycle state of the validator.
    pub status: PublicLaneValidatorStatus,
    /// Epoch index when the validator became active (if activated).
    pub activation_epoch: Option<u64>,
    /// Block height recorded at activation (if activated).
    pub activation_height: Option<u64>,
    /// Epoch identifier that last produced a reward payout.
    pub last_reward_epoch: Option<u64>,
}

/// Lifecycle state for a validator entry.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub enum PublicLaneValidatorStatus {
    /// Validator is waiting for governance approval or activation epoch (payload stores the epoch).
    PendingActivation(u64),
    /// Validator participates in consensus for the target lane.
    Active,
    /// Validator is temporarily jailed and cannot participate until cleared.
    Jailed(String),
    /// Validator is exiting and the bonded stake is being unlocked.
    Exiting(u64),
    /// Validator has fully exited the lane.
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
    pub bonded: Numeric,
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
    pub amount: Numeric,
    /// Unix timestamp (ms) when the withdrawal can be finalised.
    pub release_at_ms: u64,
}

/// Aggregated reward share emitted for a validator or delegator.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct PublicLaneRewardShare {
    /// Account that receives the payout.
    pub account: AccountId,
    /// Role applied when allocating the reward (validator vs nominee).
    pub role: PublicLaneRewardRole,
    /// Amount of rewards allocated to the account.
    pub amount: Numeric,
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
    pub total_reward: Numeric,
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
    pub amount: Numeric,
}
