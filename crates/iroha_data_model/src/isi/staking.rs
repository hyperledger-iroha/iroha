use std::string::String;

use iroha_crypto::Hash;
use iroha_primitives::numeric::Numeric;

use super::*;
use crate::{
    account::AccountId,
    asset::AssetId,
    block::consensus::Evidence,
    metadata::Metadata,
    nexus::{LaneId, PublicLaneRewardShare},
    peer::PeerId,
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
        /// Account that owns validator authority for the lane.
        pub validator: AccountId,
        /// Peer identity that signs consensus messages for the lane.
        pub peer_id: PeerId,
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
        peer_id: PeerId,
        stake_account: AccountId,
        initial_stake: Numeric,
        metadata: Metadata,
    ) -> Self {
        Self {
            lane_id,
            validator,
            peer_id,
            stake_account,
            initial_stake,
            metadata,
        }
    }
}

isi! {
    /// Rebind an existing public-lane validator to a new consensus peer identity.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RebindPublicLaneValidatorPeer {
        /// Lane that the validator targets.
        pub lane_id: LaneId,
        /// Account that owns validator authority for the lane.
        pub validator: AccountId,
        /// Replacement peer identity that signs consensus messages for the lane.
        pub peer_id: PeerId,
    }
}

impl RebindPublicLaneValidatorPeer {
    /// Build a public-lane validator peer-rebinding instruction.
    #[must_use]
    pub fn new(lane_id: LaneId, validator: AccountId, peer_id: PeerId) -> Self {
        Self {
            lane_id,
            validator,
            peer_id,
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
    use crate::{
        peer::PeerId,
        prelude::{AccountId, Algorithm, DomainId, KeyPair},
    };

    fn sample_account() -> AccountId {
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let key_pair = KeyPair::from_seed(vec![0x11; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn sample_peer_id() -> PeerId {
        let key_pair = KeyPair::from_seed(vec![0x22; 32], Algorithm::Ed25519);
        PeerId::new(key_pair.public_key().clone())
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
        let peer_id = sample_peer_id();
        let metadata = Metadata::default();
        let instruction = RegisterPublicLaneValidator::new(
            LaneId::SINGLE,
            validator.clone(),
            peer_id.clone(),
            validator.clone(),
            Numeric::from(10_u64),
            metadata.clone(),
        );

        assert_eq!(*instruction.lane_id(), LaneId::SINGLE);
        assert_eq!(instruction.validator(), &validator);
        assert_eq!(instruction.peer_id(), &peer_id);
        assert_eq!(instruction.stake_account(), &validator);
        assert_eq!(instruction.initial_stake(), &Numeric::from(10_u64));
        assert_eq!(instruction.metadata(), &metadata);
    }

    #[test]
    fn rebind_public_lane_validator_peer_new_sets_fields() {
        let validator = sample_account();
        let peer_id = sample_peer_id();
        let instruction =
            RebindPublicLaneValidatorPeer::new(LaneId::SINGLE, validator.clone(), peer_id.clone());

        assert_eq!(*instruction.lane_id(), LaneId::SINGLE);
        assert_eq!(instruction.validator(), &validator);
        assert_eq!(instruction.peer_id(), &peer_id);
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
    /// Cancel a pending consensus evidence penalty before slashing executes.
    pub struct CancelConsensusEvidencePenalty {
        /// Evidence entry to cancel.
        pub evidence: Evidence,
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
impl crate::seal::Instruction for RebindPublicLaneValidatorPeer {}
impl crate::seal::Instruction for ActivatePublicLaneValidator {}
impl crate::seal::Instruction for ExitPublicLaneValidator {}
impl crate::seal::Instruction for BondPublicLaneStake {}
impl crate::seal::Instruction for SchedulePublicLaneUnbond {}
impl crate::seal::Instruction for FinalizePublicLaneUnbond {}
impl crate::seal::Instruction for SlashPublicLaneValidator {}
impl crate::seal::Instruction for CancelConsensusEvidencePenalty {}
impl crate::seal::Instruction for RecordPublicLaneRewards {}
impl crate::seal::Instruction for ClaimPublicLaneRewards {}

fn staking_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

impl<'a> norito::core::DecodeFromSlice<'a> for RegisterPublicLaneValidator {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = staking_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let lane_id = super::decode_aos_canonical_field::<LaneId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let validator = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let peer_id = super::decode_aos_canonical_field::<PeerId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let stake_account = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let initial_stake = super::decode_aos_canonical_field::<Numeric>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let metadata = super::decode_aos_canonical_field::<Metadata>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                lane_id,
                validator,
                peer_id,
                stake_account,
                initial_stake,
                metadata,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RebindPublicLaneValidatorPeer {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = staking_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let lane_id = super::decode_aos_canonical_field::<LaneId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let validator = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let peer_id = super::decode_aos_canonical_field::<PeerId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                lane_id,
                validator,
                peer_id,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for ActivatePublicLaneValidator {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = staking_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let lane_id = super::decode_aos_canonical_field::<LaneId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let validator = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { lane_id, validator }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for ExitPublicLaneValidator {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = staking_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let lane_id = super::decode_aos_canonical_field::<LaneId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let validator = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let release_at_ms = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                lane_id,
                validator,
                release_at_ms,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for CancelConsensusEvidencePenalty {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = staking_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let evidence = super::decode_aos_canonical_field::<Evidence>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { evidence }, offset))
    }
}

#[cfg(test)]
mod slice_tests {
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::block::consensus::{EvidenceKind, EvidencePayload};

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn peer(seed: u8) -> PeerId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        PeerId::new(key_pair.public_key().clone())
    }

    fn sample_evidence() -> Evidence {
        let key_pair = KeyPair::from_seed(vec![0xE1; 32], Algorithm::Ed25519);
        let tx_hash = HashOf::from_untyped_unchecked(Hash::prehashed([0xAA; 32]));
        let payload = crate::transaction::TransactionSubmissionReceiptPayload {
            tx_hash,
            entrypoint_hash: HashOf::from_untyped_unchecked(Hash::prehashed([0xBB; 32])),
            signed_transaction_hash: None,
            submitted_at_ms: 10,
            submitted_at_height: 2,
            signer: key_pair.public_key().clone(),
        };
        let receipt = crate::transaction::TransactionSubmissionReceipt::sign(payload, &key_pair);
        Evidence {
            kind: EvidenceKind::Censorship,
            payload: EvidencePayload::Censorship {
                tx_hash,
                receipts: vec![receipt],
            },
        }
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(
        registry: &crate::isi::InstructionRegistry,
        wire_id: &str,
        value: T,
    ) where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn staking_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: account(0x11),
            peer_id: peer(0x12),
            stake_account: account(0x13),
            initial_stake: Numeric::from(10_u64),
            metadata: Metadata::default(),
        });
        assert_slice_roundtrip(RebindPublicLaneValidatorPeer {
            lane_id: LaneId::SINGLE,
            validator: account(0x11),
            peer_id: peer(0x14),
        });
        assert_slice_roundtrip(ActivatePublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: account(0x11),
        });
        assert_slice_roundtrip(ExitPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: account(0x11),
            release_at_ms: 123_456,
        });
        assert_slice_roundtrip(CancelConsensusEvidencePenalty {
            evidence: sample_evidence(),
        });
    }

    #[test]
    fn staking_registry_decodes_type_names_and_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<RegisterPublicLaneValidator>()
            .register_slice::<RebindPublicLaneValidatorPeer>()
            .register_slice::<ActivatePublicLaneValidator>()
            .register_slice::<ExitPublicLaneValidator>()
            .register_slice::<CancelConsensusEvidencePenalty>()
            .register_with_id_slice::<ActivatePublicLaneValidator>(
                "iroha.staking.activate_public_lane_validator",
            )
            .register_with_id_slice::<RebindPublicLaneValidatorPeer>(
                "iroha.staking.rebind_public_lane_validator_peer",
            )
            .register_with_id_slice::<ExitPublicLaneValidator>(
                "iroha.staking.exit_public_lane_validator",
            );

        assert_registry_decodes(
            &registry,
            std::any::type_name::<RegisterPublicLaneValidator>(),
            RegisterPublicLaneValidator {
                lane_id: LaneId::SINGLE,
                validator: account(0x11),
                peer_id: peer(0x12),
                stake_account: account(0x13),
                initial_stake: Numeric::from(10_u64),
                metadata: Metadata::default(),
            },
        );
        assert_registry_decodes(
            &registry,
            "iroha.staking.rebind_public_lane_validator_peer",
            RebindPublicLaneValidatorPeer {
                lane_id: LaneId::SINGLE,
                validator: account(0x11),
                peer_id: peer(0x14),
            },
        );
        assert_registry_decodes(
            &registry,
            "iroha.staking.activate_public_lane_validator",
            ActivatePublicLaneValidator {
                lane_id: LaneId::SINGLE,
                validator: account(0x11),
            },
        );
        assert_registry_decodes(
            &registry,
            "iroha.staking.exit_public_lane_validator",
            ExitPublicLaneValidator {
                lane_id: LaneId::SINGLE,
                validator: account(0x11),
                release_at_ms: 123_456,
            },
        );
        assert_registry_decodes(
            &registry,
            std::any::type_name::<CancelConsensusEvidencePenalty>(),
            CancelConsensusEvidencePenalty {
                evidence: sample_evidence(),
            },
        );
    }
}

#[cfg(test)]
mod json_tests {
    use super::{
        ActivatePublicLaneValidator, RebindPublicLaneValidatorPeer, RegisterPublicLaneValidator,
    };
    use crate::{
        account::AccountId, domain::DomainId, metadata::Metadata, nexus::LaneId, peer::PeerId,
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_primitives::numeric::Numeric;
    use norito::json::value::{from_value, to_value};

    #[test]
    fn register_public_lane_validator_json_roundtrip() {
        let _domain: crate::domain::DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let validator_key = KeyPair::from_seed(vec![0xA1; 32], Algorithm::Ed25519);
        let stake_key = KeyPair::from_seed(vec![0xA2; 32], Algorithm::Ed25519);
        let peer_key = KeyPair::from_seed(vec![0xA3; 32], Algorithm::Ed25519);
        let validator = AccountId::new(validator_key.public_key().clone());
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let stake_account = AccountId::new(stake_key.public_key().clone());
        let isi = RegisterPublicLaneValidator::new(
            LaneId::new(1),
            validator.clone(),
            peer_id,
            stake_account,
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
        let _domain: crate::domain::DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let validator_key = KeyPair::from_seed(vec![0xB1; 32], Algorithm::Ed25519);
        let validator = AccountId::new(validator_key.public_key().clone());
        let isi = ActivatePublicLaneValidator::new(LaneId::new(2), validator);

        let encoded = to_value(&isi).expect("encode ActivatePublicLaneValidator");
        let decoded: ActivatePublicLaneValidator =
            from_value(encoded).expect("decode ActivatePublicLaneValidator");

        assert_eq!(decoded, isi);
    }

    #[test]
    fn rebind_public_lane_validator_peer_json_roundtrip() {
        let _domain: crate::domain::DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let validator_key = KeyPair::from_seed(vec![0xC1; 32], Algorithm::Ed25519);
        let peer_key = KeyPair::from_seed(vec![0xC2; 32], Algorithm::Ed25519);
        let validator = AccountId::new(validator_key.public_key().clone());
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let isi = RebindPublicLaneValidatorPeer::new(LaneId::new(3), validator, peer_id);

        let encoded = to_value(&isi).expect("encode RebindPublicLaneValidatorPeer");
        let decoded: RebindPublicLaneValidatorPeer =
            from_value(encoded).expect("decode RebindPublicLaneValidatorPeer");

        assert_eq!(decoded, isi);
    }
}
