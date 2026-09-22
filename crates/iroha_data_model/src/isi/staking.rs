//! Stake-backed public-lane validator admission, custody, and rewards.
use super::*;
use crate::{
    account::AccountId, asset::AssetId, block::consensus::Evidence,
    nexus::{PublicLaneRewardShare, PublicLaneMonetaryPlanV1, PublicLaneRewardClaimPlanV1},
};
use iroha_crypto::{Hash, SignatureOf};
use iroha_model_base::metadata::Metadata;
use iroha_model_base::peer::PeerId;
use iroha_model_base::topology::LaneId;
use iroha_primitives::numeric::Quantity;
use std::string::String;
isi! {
    /// Activate a pending validator for a public Nexus lane.
    #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::staking::ActivatePublicLaneValidator")]
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
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::staking::ExitPublicLaneValidator")]
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
    /// Register a validator for a public Nexus lane and bond validator-owned initial stake.
    #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::staking::RegisterPublicLaneValidator")]
    pub struct RegisterPublicLaneValidator {
        /// Lane that the validator targets.
        pub lane_id: LaneId,
        /// Account that owns validator authority for the lane.
        pub validator: AccountId,
        /// Peer identity that signs consensus messages for the lane.
        pub peer_id: PeerId,
        /// Validator account whose funds are locked as initial self-stake.
        pub stake_account: AccountId,
        /// Amount of stake bonded during registration.
        pub initial_stake: Quantity,
        /// Metadata documenting commission, jurisdiction flags, telemetry ids, etc.
        pub metadata: Metadata,
        /// Signed exact transfer, custody effect, and current-state precondition.
        pub monetary_plan: PublicLaneMonetaryPlanV1,
    }
}
/// Domain-separated consent from a fresh consensus peer to its stake-owning account.
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
#[norito_schema(name = "iroha_data_model::isi::staking::PublicLaneCandidateAuthorization")]
pub struct PublicLaneCandidateAuthorization {
    /// Protocol domain separating this signature from every other signed message.
    domain: String,
    /// Genesis-derived identity of the network admitting the candidate.
    pub network_id: crate::NetworkId,
    /// Exact validator identity, lane, peer, stake, and metadata authorized by the peer.
    pub registration: RegisterPublicLaneValidator,
    /// Exact planned eligibility boundary; consent cannot authorize a later tenure.
    pub activation_height: u64,
}
impl PublicLaneCandidateAuthorization {
    /// Build the exact message that a candidate consensus key must sign.
    #[must_use]
    pub fn new(
        network_id: crate::NetworkId,
        registration: RegisterPublicLaneValidator,
        activation_height: u64,
    ) -> Self {
        Self {
            domain: "iroha.staking.public_lane_candidate.v1".to_owned(),
            network_id,
            registration,
            activation_height,
        }
    }
}
isi! {
    /// Atomically admit a fresh consensus peer and escrow its owner's validator stake.
    ///
    /// Admission schedules eligibility after key activation and the next unfrozen election;
    /// it never grants membership of the current committee.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::staking::RegisterPublicLaneCandidate")]
    pub struct RegisterPublicLaneCandidate {
        /// Exact registration consented to by the transaction account and consensus peer.
        pub registration: RegisterPublicLaneValidator,
        /// Expected election boundary, signed by the peer and recomputed at admission.
        pub activation_height: u64,
        /// BLS-normal proof of possession for the fresh peer identity.
        pub proof_of_possession: Vec<u8>,
        /// Peer signature over the network-bound candidate authorization.
        pub peer_signature: SignatureOf<PublicLaneCandidateAuthorization>,
    }
}
impl crate::seal::Instruction for RegisterPublicLaneCandidate {}
impl RegisterPublicLaneValidator {
    /// Build a public-lane validator registration instruction.
    #[must_use]
    pub fn new(
        lane_id: LaneId,
        validator: AccountId,
        peer_id: PeerId,
        stake_account: AccountId,
        initial_stake: Quantity,
        metadata: Metadata,
        monetary_plan: PublicLaneMonetaryPlanV1,
    ) -> Self {
        Self {
            lane_id,
            validator,
            peer_id,
            stake_account,
            initial_stake,
            metadata,
            monetary_plan,
        }
    }
}
/// Domain-separated peer consent to a pending validator's replacement binding.
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
#[norito_schema(name = "iroha_data_model::isi::staking::PublicLanePeerBindingAuthorization")]
pub struct PublicLanePeerBindingAuthorization {
    /// Protocol domain separating peer rebinding from initial registration.
    domain: String,
    /// Genesis-derived network identity.
    pub network_id: crate::NetworkId,
    /// Lane containing the pending validator.
    pub lane_id: LaneId,
    /// Account that owns validator stake and authority.
    pub validator: AccountId,
    /// Replacement consensus peer consenting to the binding.
    pub peer_id: PeerId,
    /// Pending validator tenure to which this consent is restricted.
    pub activation_height: u64,
    /// Existing peer binding replaced by this consent.
    pub previous_peer_id: PeerId,
}
impl PublicLanePeerBindingAuthorization {
    /// Construct the canonical replacement peer's consent message.
    #[must_use]
    pub fn new(
        network_id: crate::NetworkId,
        lane_id: LaneId,
        validator: AccountId,
        peer_id: PeerId,
        activation_height: u64,
        previous_peer_id: PeerId,
    ) -> Self {
        Self {
            domain: "iroha.staking.public_lane_peer_binding.v1".to_owned(),
            network_id,
            lane_id,
            validator,
            peer_id,
            activation_height,
            previous_peer_id,
        }
    }
}
isi! {
    /// Rebind an existing public-lane validator to a new consensus peer identity.
    #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::staking::RebindPublicLaneValidatorPeer")]
    pub struct RebindPublicLaneValidatorPeer {
        /// Lane that the validator targets.
        pub lane_id: LaneId,
        /// Account that owns validator authority for the lane.
        pub validator: AccountId,
        /// Replacement peer identity that signs consensus messages for the lane.
        pub peer_id: PeerId,
        /// Replacement peer consent; absent only for its own account signatory or peer administrators.
        pub peer_signature: Option<SignatureOf<PublicLanePeerBindingAuthorization>>,
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
            peer_signature: None,
        }
    }
    /// Attach network-bound consent from the replacement consensus peer.
    #[must_use]
    pub fn with_peer_signature(
        mut self,
        signature: SignatureOf<PublicLanePeerBindingAuthorization>,
    ) -> Self {
        self.peer_signature = Some(signature);
        self
    }
}
isi! {
    /// Bond additional stake for an existing validator (self or delegator supplied).
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::staking::BondPublicLaneStake")]
    pub struct BondPublicLaneStake {
        /// Lane identifier.
        pub lane_id: LaneId,
        /// Target validator account.
        pub validator: AccountId,
        /// Account supplying the stake (delegator or validator).
        pub staker: AccountId,
        /// Stake amount to lock.
        pub amount: Quantity,
        /// Optional metadata captured for dashboards/audit trails.
        pub metadata: Metadata,
        /// Signed exact transfer, custody effect, and current-state precondition.
        pub monetary_plan: PublicLaneMonetaryPlanV1,
    }
}

/// Construct a complete exact monetary plan for codec and visitor tests only.
#[cfg(test)]
pub(crate) fn monetary_plan_fixture(
    source: &AccountId,
    destination: &AccountId,
    amount: u64,
    precondition: crate::nexus::PublicLaneMonetaryPreconditionV1,
) -> PublicLaneMonetaryPlanV1 {
    let definition = crate::asset::AssetDefinitionId::from_uuid_bytes([
        1, 2, 3, 4, 5, 6, 0x47, 8, 0x89, 10, 11, 12, 13, 14, 15, 16,
    ])
    .expect("monetary fixture UUIDv4");
    let plan = PublicLaneMonetaryPlanV1 {
        network_scope: crate::nexus::PublicLaneMonetaryScopeV1::Network(
            crate::NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                Hash::new(b"staking-monetary-codec-fixture"),
            )),
        ),
        valid_until_height: 64,
        source_asset: AssetId::new(definition.clone(), source.clone()),
        destination_asset: AssetId::new(definition, destination.clone()),
        amount: Quantity::from(amount),
        precondition,
    };
    assert!(
        plan.has_canonical_shape(),
        "canonical monetary codec fixture"
    );
    plan
}

/// Supply an explicit registration precondition to codec and constructor tests.
#[cfg(test)]
pub(crate) fn registration_plan_fixture(
    staker: &AccountId,
    amount: u64,
) -> PublicLaneMonetaryPlanV1 {
    let custody =
        iroha_crypto::KeyPair::try_from_seed(vec![0x7A; 32], iroha_crypto::Algorithm::Ed25519)
            .expect("monetary fixture custody key");
    monetary_plan_fixture(
        staker,
        &AccountId::new(custody.public_key().clone()),
        amount,
        crate::nexus::PublicLaneMonetaryPreconditionV1::Registration(
            crate::nexus::PublicLaneRegistrationPreconditionV1 {
                activation_height: 1,
            },
        ),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::{AccountId, Algorithm, KeyPair};
    use iroha_model_base::domain::DomainId;
    use iroha_model_base::peer::PeerId;
    fn sample_account() -> AccountId {
        let _domain: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let key_pair = KeyPair::try_from_seed(vec![0x11; 32], Algorithm::Ed25519)
            .expect("derive checked staking fixture account keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    fn sample_peer_id() -> PeerId {
        let key_pair = KeyPair::try_from_seed(vec![0x22; 32], Algorithm::Ed25519)
            .expect("derive checked staking fixture peer keypair");
        PeerId::new(key_pair.public_key().clone())
    }
    fn candidate_fixture() -> (RegisterPublicLaneCandidate, crate::NetworkId) {
        let network_id = crate::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"candidate-network")),
        );
        let key = KeyPair::try_from_seed(vec![0x31; 32], Algorithm::BlsNormal)
            .expect("BLS candidate key");
        let mut monetary_plan = registration_plan_fixture(&sample_account(), 1000);
        monetary_plan.network_scope = crate::nexus::PublicLaneMonetaryScopeV1::Network(network_id);
        monetary_plan.precondition = crate::nexus::PublicLaneMonetaryPreconditionV1::Registration(
            crate::nexus::PublicLaneRegistrationPreconditionV1 {
                activation_height: 13,
            },
        );
        let registration = RegisterPublicLaneValidator::new(
            LaneId::SINGLE,
            sample_account(),
            PeerId::new(key.public_key().clone()),
            sample_account(),
            Quantity::from(1000_u64),
            Metadata::default(),
            monetary_plan,
        );
        let authorization =
            PublicLaneCandidateAuthorization::new(network_id, registration.clone(), 13);
        (
            RegisterPublicLaneCandidate {
                registration,
                activation_height: 13,
                proof_of_possession: iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("PoP"),
                peer_signature: SignatureOf::try_new(key.private_key(), &authorization)
                    .expect("peer consent"),
            },
            network_id,
        )
    }
    #[test]
    fn candidate_consent_binds_network_owner_lane_and_amount() {
        let (candidate, network) = candidate_fixture();
        let payload = PublicLaneCandidateAuthorization::new(
            network,
            candidate.registration.clone(),
            candidate.activation_height,
        );
        candidate
            .peer_signature
            .verify(candidate.registration.peer_id.public_key(), &payload)
            .expect("valid consent");
        let mut alternatives = vec![payload.clone(); 6];
        alternatives[0].network_id = crate::NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::new(b"other-network")),
        );
        alternatives[1].registration.validator =
            AccountId::new(sample_peer_id().public_key().clone());
        alternatives[2].registration.lane_id = LaneId::new(7);
        alternatives[3].registration.initial_stake = Quantity::from(999_u64);
        alternatives[4].activation_height += 1;
        alternatives[5].registration.monetary_plan.amount = Quantity::from(999_u64);
        for changed in alternatives {
            assert!(
                candidate
                    .peer_signature
                    .verify(candidate.registration.peer_id.public_key(), &changed)
                    .is_err()
            );
        }
    }
    #[test]
    fn candidate_instruction_roundtrips_through_canonical_registry() {
        let (candidate, _) = candidate_fixture();
        crate::isi::test_support::assert_slice_roundtrip(candidate.clone());
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<RegisterPublicLaneCandidate>(
                "iroha.staking.register_public_lane_candidate",
            );
        crate::isi::test_support::assert_registry_decodes(
            &registry,
            "iroha.staking.register_public_lane_candidate",
            candidate.clone(),
        );
        let json = norito::json::to_json(&candidate).expect("candidate JSON");
        let decoded: RegisterPublicLaneCandidate =
            norito::json::from_str(&json).expect("candidate decode");
        assert_eq!(candidate, decoded);
    }
    #[test]
    #[ignore = "explicit maintenance command prints canonical changed staking frame fixtures"]
    fn print_staking_admission_record_fixture_rows() {
        let (candidate, network_id) = candidate_fixture();
        let rebind = RebindPublicLaneValidatorPeer::new(
            candidate.registration.lane_id,
            candidate.registration.validator.clone(),
            candidate.registration.peer_id.clone(),
        );
        let peer_key =
            KeyPair::try_from_seed(vec![0x31; 32], Algorithm::BlsNormal).expect("fixture peer");
        let consent = PublicLanePeerBindingAuthorization::new(
            network_id,
            rebind.lane_id,
            rebind.validator.clone(),
            rebind.peer_id.clone(),
            13,
            sample_peer_id(),
        );
        let signed_rebind = rebind.clone().with_peer_signature(
            SignatureOf::try_new(peer_key.private_key(), &consent)
                .expect("fixture rebind signature"),
        );
        let rows = vec![
            crate::isi::generated_record_identity_tests::capture(candidate),
            crate::isi::generated_record_identity_tests::capture(rebind),
            crate::isi::generated_record_identity_tests::capture(signed_rebind),
        ];
        println!(
            "STAKING_ADMISSION_FIXTURE_ROWS={}",
            norito::json::to_json(&rows).expect("fixture rows")
        );
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
            Quantity::from(10_u64),
            metadata.clone(),
            registration_plan_fixture(&validator, 10),
        );
        assert_eq!(*instruction.lane_id(), LaneId::SINGLE);
        assert_eq!(instruction.validator(), &validator);
        assert_eq!(instruction.peer_id(), &peer_id);
        assert_eq!(instruction.stake_account(), &validator);
        assert_eq!(instruction.initial_stake(), &Quantity::from(10_u64));
        assert_eq!(instruction.metadata(), &metadata);
        assert_eq!(
            instruction.monetary_plan(),
            &registration_plan_fixture(&validator, 10)
        );
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
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::staking::SchedulePublicLaneUnbond")]
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
        pub amount: Quantity,
        /// Timestamp (unix ms) when the withdrawal becomes eligible for finalisation.
        pub release_at_ms: u64,
    }
}
isi! {
    /// Finalise a previously scheduled stake withdrawal once the unlock timer expires.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::staking::FinalizePublicLaneUnbond")]
    pub struct FinalizePublicLaneUnbond {
        /// Lane identifier.
        pub lane_id: LaneId,
        /// Target validator account.
        pub validator: AccountId,
        /// Account receiving the unlocked stake.
        pub staker: AccountId,
        /// Identifier of the withdrawal request that is being completed.
        pub request_id: Hash,
        /// Signed exact transfer, custody effect, and current-state precondition.
        pub monetary_plan: PublicLaneMonetaryPlanV1,
    }
}
isi! {
    /// Slash a validator for misbehaviour and emit an audit trail entry.
    #[norito_schema(name = "iroha_data_model::isi::staking::SlashPublicLaneValidator")]
    pub struct SlashPublicLaneValidator {
        /// Lane identifier.
        pub lane_id: LaneId,
        /// Misbehaving validator account.
        pub validator: AccountId,
        /// Exact consensus height at which the slashable offence occurred.
        pub offence_height: u64,
        /// Unique identifier for the slash event.
        pub slash_id: Hash,
        /// Amount of stake to burn or seize.
        pub amount: Quantity,
        /// Canonical reason code (e.g., `double_sign`, `downtime`).
        pub reason_code: String,
        /// Metadata documenting evidence digests, governance proposal ids, etc.
        pub metadata: Metadata,
        /// Signed exact transfer, custody effect, and current-state precondition.
        pub monetary_plan: PublicLaneMonetaryPlanV1,
    }
}
isi! {
    /// Cancel a pending consensus evidence penalty before slashing executes.
    #[norito_schema(name = "iroha_data_model::isi::staking::CancelConsensusEvidencePenalty")]
    pub struct CancelConsensusEvidencePenalty {
        /// Evidence entry to cancel.
        pub evidence: Evidence,
    }
}
isi! {
    /// Record a reward distribution for a public lane epoch.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::staking::RecordPublicLaneRewards")]
    pub struct RecordPublicLaneRewards {
        /// Lane identifier.
        pub lane_id: LaneId,
        /// Epoch identifier generated by consensus.
        pub epoch: u64,
        /// Asset used for payouts.
        pub reward_asset: AssetId,
        /// Total reward minted or transferred into the pool.
        pub total_reward: Quantity,
        /// Individual reward shares per validator/delegator.
        pub shares: Vec<PublicLaneRewardShare>,
        /// Optional metadata for audit reports (tx hashes, ceremony notes).
        pub metadata: Metadata,
    }
}
isi! {
    /// Process a bounded signed reward prefix and pay exact accrued entitlements.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::staking::ClaimPublicLaneRewards")]
    pub struct ClaimPublicLaneRewards {
        /// Lane identifier.
        pub lane_id: LaneId,
        /// Account receiving the rewards.
        pub account: AccountId,
        /// Exact bounded reward records, retained accruals, and payments authorized by the recipient.
        pub claim_plan: PublicLaneRewardClaimPlanV1,
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
impl<'a> norito::core::DecodeFromSlice<'a> for RegisterPublicLaneCandidate {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = staking_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let registration = super::decode_aos_canonical_field::<RegisterPublicLaneValidator>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let activation_height = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let proof_of_possession = super::decode_aos_canonical_field::<Vec<u8>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let peer_signature = super::decode_aos_canonical_field::<
            SignatureOf<PublicLaneCandidateAuthorization>,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                registration,
                activation_height,
                proof_of_possession,
                peer_signature,
            },
            offset,
        ))
    }
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
        let initial_stake = super::decode_aos_canonical_field::<Quantity>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let metadata = super::decode_aos_canonical_field::<Metadata>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let monetary_plan = super::decode_aos_canonical_field::<PublicLaneMonetaryPlanV1>(
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
                monetary_plan,
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
        let peer_signature = super::decode_aos_canonical_field::<
            Option<SignatureOf<PublicLanePeerBindingAuthorization>>,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                lane_id,
                validator,
                peer_id,
                peer_signature,
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
    use super::*;
    use crate::isi::test_support::{assert_registry_decodes, assert_slice_roundtrip};
    use crate::{
        NetworkId,
        block::{
            Header as BlockHeader,
            consensus::SumeragiV2EquivocationEvidence,
            consensus_v2::{
                ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum, HeightContext,
                PROTOCOL_VERSION, PayloadEncoding, SumeragiV2Equivocation, TimeoutVote,
                ValidatorPower,
            },
        },
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_primitives::numeric::Numeric;
    use norito::codec::Decode;
    use norito::core::{DecodeFlagsGuard, DecodeFromSlice, header_flags, read_len_dyn_slice};
    #[derive(norito::codec::Encode)]
    struct ForgedRegisterPublicLaneValidator {
        lane_id: LaneId,
        validator: AccountId,
        peer_id: PeerId,
        stake_account: AccountId,
        initial_stake: Numeric,
        metadata: Metadata,
        monetary_plan: PublicLaneMonetaryPlanV1,
    }
    #[derive(norito::codec::Encode)]
    struct ForgedBondPublicLaneStake {
        lane_id: LaneId,
        validator: AccountId,
        staker: AccountId,
        amount: Numeric,
        metadata: Metadata,
        monetary_plan: PublicLaneMonetaryPlanV1,
    }
    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked staking slice fixture account keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    fn peer(seed: u8) -> PeerId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked staking slice fixture peer keypair");
        PeerId::new(key_pair.public_key().clone())
    }
    fn sample_evidence() -> Evidence {
        let mut peers = (0xE1_u8..=0xE4).map(peer).collect::<Vec<_>>();
        peers.sort();
        let roster = peers
            .into_iter()
            .map(|validator| ValidatorPower {
                validator,
                power: 1,
            })
            .collect::<Vec<_>>();
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA1; 32])),
        );
        let mint_finality_roster = crate::isi::kagemusha_v1::KagemushaMintFinalityAuthorityGenerationV1 {
            version: crate::isi::kagemusha_v1::KAGEMUSHA_CHAIN_VERSION_V1,
            network_id,
            generation: 0,
            validators: roster
                .iter()
                .enumerate()
                .map(|(index, validator)| {
                    crate::isi::kagemusha_v1::KagemushaMintFinalityValidatorKeysV1 {
                        validator: validator.validator.clone(),
                        eq_proof_public_key: [u8::try_from(index + 1)
                            .expect("small fixture roster");
                            32],
                        ep_proof_public_key: [u8::try_from(index + 17)
                            .expect("small fixture roster");
                            32],
                    }
                })
                .collect(),
        };
        let mint_finality_authorization = crate::block::consensus_v2::test_kagemusha_genesis_authorization(&mint_finality_roster, 2);
        let context = HeightContext {
            network_id,
            protocol_version: PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            kagemusha_mint_finality_authorization: mint_finality_authorization,
            kagemusha_mint_finality_authority: mint_finality_roster,
            epoch_end_height: 2,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"staking evidence nexus context"),
            execution_policy_hash: Hash::new(b"staking evidence execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 4,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 1024,
                max_chunk_count: 512,
            },
            leader_seed: [0xA5; 32],
        };
        context.validate().expect("canonical non-boundary evidence fixture");
        let round = ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        Evidence {
            equivocation: SumeragiV2EquivocationEvidence {
                context,
                proofs_of_possession: vec![vec![0xC1; 96]; 4],
                conflict: SumeragiV2Equivocation::TimeoutVote {
                    first: TimeoutVote {
                        round,
                        highest_prepare_qc: None,
                        signer: 0,
                        signature: vec![0xD1; 96],
                    },
                    second: TimeoutVote {
                        round,
                        highest_prepare_qc: None,
                        signer: 0,
                        signature: vec![0xD2; 96],
                    },
                },
            },
        }
    }
    #[test]
    fn staking_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(RegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: account(0x11),
            peer_id: peer(0x12),
            stake_account: account(0x13),
            initial_stake: Quantity::from(10_u64),
            metadata: Metadata::default(),
            monetary_plan: registration_plan_fixture(&account(0x13), 10),
        });
        assert_slice_roundtrip(RebindPublicLaneValidatorPeer {
            lane_id: LaneId::SINGLE,
            validator: account(0x11),
            peer_id: peer(0x14),
            peer_signature: None,
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
    fn staking_registry_decodes_only_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<RegisterPublicLaneValidator>(
                "iroha.instruction.v1::staking::RegisterPublicLaneValidator",
            )
            .register_with_id_slice::<RebindPublicLaneValidatorPeer>(
                "iroha.staking.rebind_public_lane_validator_peer",
            )
            .register_with_id_slice::<ActivatePublicLaneValidator>(
                "iroha.staking.activate_public_lane_validator",
            )
            .register_with_id_slice::<ExitPublicLaneValidator>(
                "iroha.staking.exit_public_lane_validator",
            )
            .register_with_id_slice::<CancelConsensusEvidencePenalty>(
                "iroha.instruction.v1::staking::CancelConsensusEvidencePenalty",
            );
        assert_registry_decodes(
            &registry,
            "iroha.instruction.v1::staking::RegisterPublicLaneValidator",
            RegisterPublicLaneValidator {
                lane_id: LaneId::SINGLE,
                validator: account(0x11),
                peer_id: peer(0x12),
                stake_account: account(0x13),
                initial_stake: Quantity::from(10_u64),
                metadata: Metadata::default(),
                monetary_plan: registration_plan_fixture(&account(0x13), 10),
            },
        );
        assert_registry_decodes(
            &registry,
            "iroha.staking.rebind_public_lane_validator_peer",
            RebindPublicLaneValidatorPeer {
                lane_id: LaneId::SINGLE,
                validator: account(0x11),
                peer_id: peer(0x14),
                peer_signature: None,
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
            "iroha.instruction.v1::staking::CancelConsensusEvidencePenalty",
            CancelConsensusEvidencePenalty {
                evidence: sample_evidence(),
            },
        );
        for type_name in [
            std::any::type_name::<RegisterPublicLaneValidator>(),
            std::any::type_name::<RebindPublicLaneValidatorPeer>(),
            std::any::type_name::<ActivatePublicLaneValidator>(),
            std::any::type_name::<ExitPublicLaneValidator>(),
            std::any::type_name::<CancelConsensusEvidencePenalty>(),
        ] {
            assert!(registry.decode(type_name, &[]).is_none());
        }
    }
    #[test]
    fn negative_numeric_payloads_cannot_decode_as_staking_instructions() {
        let mut forged_registration = ForgedRegisterPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: account(0x31),
            peer_id: peer(0x32),
            stake_account: account(0x33),
            initial_stake: Numeric::new(-1_i32, 0),
            metadata: Metadata::default(),
            monetary_plan: registration_plan_fixture(&account(0x33), 1),
        };
        let encoded = norito::codec::Encode::encode(&forged_registration);
        assert!(
            RegisterPublicLaneValidator::decode_from_slice(&encoded).is_err(),
            "a negative signed payload must not decode as validator initial stake"
        );
        forged_registration.initial_stake = Numeric::new(1_i32, 0);
        let positive = norito::codec::Encode::encode(&forged_registration);
        assert!(
            RegisterPublicLaneValidator::decode_from_slice(&positive).is_ok(),
            "the same complete payload with positive stake must decode"
        );
        let mut forged_bond = ForgedBondPublicLaneStake {
            lane_id: LaneId::SINGLE,
            validator: account(0x34),
            staker: account(0x35),
            amount: Numeric::new(-1_i32, 0),
            metadata: Metadata::default(),
            monetary_plan: monetary_plan_fixture(
                &account(0x35),
                &account(0x34),
                1,
                crate::nexus::PublicLaneMonetaryPreconditionV1::Bond(
                    crate::nexus::PublicLaneBondPreconditionV1 {
                        activation_height: 1,
                        peer_id: peer(0x36),
                    },
                ),
            ),
        };
        let encoded = norito::codec::Encode::encode(&forged_bond);
        assert!(
            BondPublicLaneStake::decode(&mut encoded.as_slice()).is_err(),
            "a negative signed payload must not decode as bonded stake"
        );
        forged_bond.amount = Numeric::new(1_i32, 0);
        let positive = norito::codec::Encode::encode(&forged_bond);
        assert!(
            BondPublicLaneStake::decode(&mut positive.as_slice()).is_ok(),
            "the same complete payload with positive bonded stake must decode"
        );
    }
    #[test]
    fn forged_bond_lane_id_packed_layout_is_rejected_without_unwind() {
        const WIRE_ID: &str = "iroha.instruction.v1::staking::BondPublicLaneStake";

        let bond = BondPublicLaneStake {
            lane_id: LaneId::SINGLE,
            validator: account(0x41),
            staker: account(0x42),
            amount: Quantity::from(10_u64),
            metadata: Metadata::default(),
            monetary_plan: monetary_plan_fixture(
                &account(0x42),
                &account(0x41),
                10,
                crate::nexus::PublicLaneMonetaryPreconditionV1::Bond(
                    crate::nexus::PublicLaneBondPreconditionV1 {
                        activation_height: 1,
                        peer_id: peer(0x43),
                    },
                ),
            ),
        };
        let flags =
            header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET;
        let (mut payload, encoded_flags) = {
            let _guard = DecodeFlagsGuard::enter(flags);
            norito::codec::encode_with_header_flags(&bond)
        };
        assert_eq!(encoded_flags & flags, flags);
        assert_ne!(
            payload[0] & 1,
            0,
            "LaneId must have an explicit packed-field size"
        );
        let mut size_offset = 1_usize;
        let mut lane_header = None;
        let mut lane_len = None;
        {
            let _guard = DecodeFlagsGuard::enter(flags);
            for field in 0..6 {
                if payload[0] & (1 << field) == 0 {
                    continue;
                }
                let (len, header_len) =
                    read_len_dyn_slice(&payload[size_offset..]).expect("packed field size");
                if field == 0 {
                    lane_header = Some((size_offset, header_len));
                    lane_len = Some(len);
                }
                size_offset += header_len;
            }
        }
        let (lane_header_offset, lane_header_len) = lane_header.expect("LaneId packed size header");
        let lane_len = lane_len.expect("LaneId packed payload length");
        assert_eq!(lane_header_len, 1, "small LaneId size must be one varint");
        assert!(
            lane_len > 2,
            "canonical LaneId payload must exceed the forgery"
        );
        payload[lane_header_offset] = 2;
        payload.splice(size_offset..size_offset + lane_len, [0b1, 0x00]);
        let framed = norito::core::frame_bare_with_header_flags::<BondPublicLaneStake>(
            &payload,
            encoded_flags,
        )
        .expect("frame forged bond");
        let registry =
            crate::isi::InstructionRegistry::new().register_with_id::<BondPublicLaneStake>(WIRE_ID);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            registry
                .decode(WIRE_ID, &framed)
                .expect("registered BondPublicLaneStake")
        }));
        let decoded = outcome.expect("forged LaneId must not unwind");
        match decoded {
            Err(norito::Error::LengthMismatch | norito::Error::NonCanonicalEncoding) => {}
            Err(error) => panic!("unexpected forged LaneId decode error: {error:?}"),
            Ok(_) => {
                panic!("a forged LaneId field bitset must not decode through BondPublicLaneStake")
            }
        }
    }
}
#[cfg(test)]
mod json_tests {
    use super::{
        ActivatePublicLaneValidator, RebindPublicLaneValidatorPeer, RegisterPublicLaneValidator,
    };
    use crate::account::AccountId;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_model_base::domain::DomainId;
    use iroha_model_base::metadata::Metadata;
    use iroha_model_base::peer::PeerId;
    use iroha_model_base::topology::LaneId;
    use iroha_primitives::numeric::Quantity;
    use norito::json::value::{from_value, to_value};
    #[test]
    fn register_public_lane_validator_json_roundtrip() {
        let _domain: iroha_model_base::domain::DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let validator_key = KeyPair::try_from_seed(vec![0xA1; 32], Algorithm::Ed25519)
            .expect("derive checked staking JSON validator fixture keypair");
        let stake_key = KeyPair::try_from_seed(vec![0xA2; 32], Algorithm::Ed25519)
            .expect("derive checked staking JSON stake fixture keypair");
        let peer_key = KeyPair::try_from_seed(vec![0xA3; 32], Algorithm::Ed25519)
            .expect("derive checked staking JSON peer fixture keypair");
        let validator = AccountId::new(validator_key.public_key().clone());
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let stake_account = AccountId::new(stake_key.public_key().clone());
        let isi = RegisterPublicLaneValidator::new(
            LaneId::new(1),
            validator.clone(),
            peer_id,
            stake_account.clone(),
            Quantity::from(42u32),
            Metadata::default(),
            super::registration_plan_fixture(&stake_account, 42),
        );
        let encoded = to_value(&isi).expect("encode RegisterPublicLaneValidator");
        let decoded: RegisterPublicLaneValidator =
            from_value(encoded).expect("decode RegisterPublicLaneValidator");
        assert_eq!(decoded, isi);
    }
    #[test]
    fn activate_public_lane_validator_json_roundtrip() {
        let _domain: iroha_model_base::domain::DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let validator_key = KeyPair::try_from_seed(vec![0xB1; 32], Algorithm::Ed25519)
            .expect("derive checked staking JSON active validator fixture keypair");
        let validator = AccountId::new(validator_key.public_key().clone());
        let isi = ActivatePublicLaneValidator::new(LaneId::new(2), validator);
        let encoded = to_value(&isi).expect("encode ActivatePublicLaneValidator");
        let decoded: ActivatePublicLaneValidator =
            from_value(encoded).expect("decode ActivatePublicLaneValidator");
        assert_eq!(decoded, isi);
    }
    #[test]
    fn rebind_public_lane_validator_peer_json_roundtrip() {
        let _domain: iroha_model_base::domain::DomainId =
            DomainId::try_new("wonderland", "universal").expect("domain id");
        let validator_key = KeyPair::try_from_seed(vec![0xC1; 32], Algorithm::Ed25519)
            .expect("derive checked staking JSON rebind validator fixture keypair");
        let peer_key = KeyPair::try_from_seed(vec![0xC2; 32], Algorithm::Ed25519)
            .expect("derive checked staking JSON rebind peer fixture keypair");
        let validator = AccountId::new(validator_key.public_key().clone());
        let peer_id = PeerId::new(peer_key.public_key().clone());
        let isi = RebindPublicLaneValidatorPeer::new(LaneId::new(3), validator, peer_id);
        let encoded = to_value(&isi).expect("encode RebindPublicLaneValidatorPeer");
        let decoded: RebindPublicLaneValidatorPeer =
            from_value(encoded).expect("decode RebindPublicLaneValidatorPeer");
        assert_eq!(decoded, isi);
    }
}
