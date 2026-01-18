use std::string::String;

use iroha_crypto::Hash;
use iroha_primitives::numeric::Numeric;

use super::*;
use crate::{
    account::AccountId,
    asset::AssetId,
    metadata::Metadata,
    nexus::{LaneId, PublicLaneRewardShare},
};

isi! {
    /// Activate a pending validator for a public Nexus lane.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct ActivatePublicLaneValidator {
        /// Lane that the validator targets.
        pub lane_id: LaneId,
        /// Account that signs consensus messages for the lane.
        pub validator: AccountId,
    }
}

impl ActivatePublicLaneValidator {
    /// Build a public-lane validator activation instruction.
    #[must_use]
    pub fn new(lane_id: LaneId, validator: AccountId) -> Self {
        Self { lane_id, validator }
    }
}

isi! {
    /// Request graceful exit for a validator and release its slot.
    pub struct ExitPublicLaneValidator {
        /// Lane that the validator targets.
        pub lane_id: LaneId,
        /// Account that signs consensus messages for the lane.
        pub validator: AccountId,
        /// Timestamp (unix ms) after which the validator is considered fully exited.
        pub release_at_ms: u64,
    }
}

isi! {
    /// Register a validator for a public Nexus lane and bond initial stake.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RegisterPublicLaneValidator {
        /// Lane that the validator targets.
        pub lane_id: LaneId,
        /// Account that signs consensus messages for the lane.
        pub validator: AccountId,
        /// Account whose funds are locked as stake.
        pub stake_account: AccountId,
        /// Amount of stake bonded during registration.
        pub initial_stake: Numeric,
        /// Metadata documenting commission, jurisdiction flags, telemetry ids, etc.
        pub metadata: Metadata,
    }
}

impl RegisterPublicLaneValidator {
    /// Build a public-lane validator registration instruction.
    #[must_use]
    pub fn new(
        lane_id: LaneId,
        validator: AccountId,
        stake_account: AccountId,
        initial_stake: Numeric,
        metadata: Metadata,
    ) -> Self {
        Self {
            lane_id,
            validator,
            stake_account,
            initial_stake,
            metadata,
        }
    }
}

isi! {
    /// Bond additional stake for an existing validator (self or delegator supplied).
    pub struct BondPublicLaneStake {
        /// Lane identifier.
        pub lane_id: LaneId,
        /// Target validator account.
        pub validator: AccountId,
        /// Account supplying the stake (delegator or validator).
        pub staker: AccountId,
        /// Stake amount to lock.
        pub amount: Numeric,
        /// Optional metadata captured for dashboards/audit trails.
        pub metadata: Metadata,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{AccountId, Algorithm, DomainId, KeyPair};

    fn sample_account() -> AccountId {
        let domain: DomainId = "wonderland".parse().expect("domain id");
        let key_pair = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
        AccountId::new(domain, key_pair.public_key().clone())
    }

    #[test]
    fn activate_public_lane_validator_new_sets_fields() {
        let validator = sample_account();
        let instruction = ActivatePublicLaneValidator::new(LaneId::SINGLE, validator.clone());

        assert_eq!(*instruction.lane_id(), LaneId::SINGLE);
        assert_eq!(instruction.validator(), &validator);
    }

    #[test]
    fn register_public_lane_validator_new_sets_fields() {
        let validator = sample_account();
        let metadata = Metadata::default();
        let instruction = RegisterPublicLaneValidator::new(
            LaneId::SINGLE,
            validator.clone(),
            validator.clone(),
            Numeric::from(10_u64),
            metadata.clone(),
        );

        assert_eq!(*instruction.lane_id(), LaneId::SINGLE);
        assert_eq!(instruction.validator(), &validator);
        assert_eq!(instruction.stake_account(), &validator);
        assert_eq!(instruction.initial_stake(), &Numeric::from(10_u64));
        assert_eq!(instruction.metadata(), &metadata);
    }
}

isi! {
    /// Schedule stake withdrawal for a validator or delegator.
    pub struct SchedulePublicLaneUnbond {
        /// Lane identifier.
        pub lane_id: LaneId,
        /// Target validator account.
        pub validator: AccountId,
        /// Account withdrawing stake.
        pub staker: AccountId,
        /// Deterministic identifier supplied by the caller to track the withdrawal.
        pub request_id: Hash,
        /// Amount scheduled for withdrawal.
        pub amount: Numeric,
        /// Timestamp (unix ms) when the withdrawal becomes eligible for finalisation.
        pub release_at_ms: u64,
    }
}

isi! {
    /// Finalise a previously scheduled stake withdrawal once the unlock timer expires.
    pub struct FinalizePublicLaneUnbond {
        /// Lane identifier.
        pub lane_id: LaneId,
        /// Target validator account.
        pub validator: AccountId,
        /// Account receiving the unlocked stake.
        pub staker: AccountId,
        /// Identifier of the withdrawal request that is being completed.
        pub request_id: Hash,
    }
}

isi! {
    /// Slash a validator for misbehaviour and emit an audit trail entry.
    pub struct SlashPublicLaneValidator {
        /// Lane identifier.
        pub lane_id: LaneId,
        /// Misbehaving validator account.
        pub validator: AccountId,
        /// Unique identifier for the slash event.
        pub slash_id: Hash,
        /// Amount of stake to burn or seize.
        pub amount: Numeric,
        /// Canonical reason code (e.g., `double_sign`, `downtime`).
        pub reason_code: String,
        /// Metadata documenting evidence digests, governance proposal ids, etc.
        pub metadata: Metadata,
    }
}

isi! {
    /// Record a reward distribution for a public lane epoch.
    pub struct RecordPublicLaneRewards {
        /// Lane identifier.
        pub lane_id: LaneId,
        /// Epoch identifier generated by consensus.
        pub epoch: u64,
        /// Asset used for payouts.
        pub reward_asset: AssetId,
        /// Total reward minted or transferred into the pool.
        pub total_reward: Numeric,
        /// Individual reward shares per validator/delegator.
        pub shares: Vec<PublicLaneRewardShare>,
        /// Optional metadata for audit reports (tx hashes, ceremony notes).
        pub metadata: Metadata,
    }
}

isi! {
    /// Claim pending public-lane rewards for an account up to an optional epoch (inclusive).
    pub struct ClaimPublicLaneRewards {
        /// Lane identifier.
        pub lane_id: LaneId,
        /// Account receiving the rewards.
        pub account: AccountId,
        /// Upper bound for epochs to claim (inclusive). If omitted, claims all available epochs.
        pub upto_epoch: Option<u64>,
    }
}

impl crate::seal::Instruction for RegisterPublicLaneValidator {}
impl crate::seal::Instruction for ActivatePublicLaneValidator {}
impl crate::seal::Instruction for ExitPublicLaneValidator {}
impl crate::seal::Instruction for BondPublicLaneStake {}
impl crate::seal::Instruction for SchedulePublicLaneUnbond {}
impl crate::seal::Instruction for FinalizePublicLaneUnbond {}
impl crate::seal::Instruction for SlashPublicLaneValidator {}
impl crate::seal::Instruction for RecordPublicLaneRewards {}
impl crate::seal::Instruction for ClaimPublicLaneRewards {}

#[cfg(test)]
mod json_tests {
    use super::{ActivatePublicLaneValidator, RegisterPublicLaneValidator};
    use crate::{account::AccountId, metadata::Metadata, nexus::LaneId};
    use iroha_primitives::numeric::Numeric;
    use norito::json::value::{from_value, to_value};

    #[test]
    fn register_public_lane_validator_json_roundtrip() {
        let validator: AccountId = "validator@wonderland"
            .parse()
            .expect("validator account id");
        let isi = RegisterPublicLaneValidator::new(
            LaneId::new(1),
            validator.clone(),
            "stake@wonderland".parse().expect("stake account id"),
            Numeric::from(42u32),
            Metadata::default(),
        );

        let encoded = to_value(&isi).expect("encode RegisterPublicLaneValidator");
        let decoded: RegisterPublicLaneValidator =
            from_value(encoded).expect("decode RegisterPublicLaneValidator");

        assert_eq!(decoded, isi);
    }

    #[test]
    fn activate_public_lane_validator_json_roundtrip() {
        let validator: AccountId = "validator@wonderland"
            .parse()
            .expect("validator account id");
        let isi = ActivatePublicLaneValidator::new(LaneId::new(2), validator);

        let encoded = to_value(&isi).expect("encode ActivatePublicLaneValidator");
        let decoded: ActivatePublicLaneValidator =
            from_value(encoded).expect("decode ActivatePublicLaneValidator");

        assert_eq!(decoded, isi);
    }
}
