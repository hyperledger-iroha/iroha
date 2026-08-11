use super::*;
use crate::{
    account::AccountId,
    asset::AssetDefinitionId,
    metadata::Metadata,
    nexus::{
        FeeSponsorProgram, FeeSponsorProgramId, FeeSponsorProgramRevision, LaneId,
        LaneRelayEnvelope, ProofBlob,
    },
    peer::PeerId,
};
use iroha_primitives::numeric::Quantity;

isi! {
    /// Set or clear emergency validator peers used for lane relay quorum recovery.
    ///
    /// This instruction is disabled by default and requires
    /// `nexus.lane_relay_emergency.enabled = true`. When enabled, the transaction authority
    /// must be a multisig account meeting the configured threshold/member minimums
    /// (defaults to 3-of-5).
    pub struct SetLaneRelayEmergencyValidators {
        /// Lane whose emergency committee fillers are being overridden.
        pub lane_id: LaneId,
        /// Live consensus peers allowed to fill missing committee slots.
        pub peers: Vec<PeerId>,
        /// Optional block height (inclusive) after which the override expires.
        #[norito(skip_serializing_if = "Option::is_none")]
        #[norito(default)]
        pub expires_at_height: Option<u64>,
        /// Optional metadata describing the override decision.
        #[norito(default)]
        pub metadata: Metadata,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Persist a finalized, verified private-source lane relay for contract consumption.
    ///
    /// Any account may transport the instruction, but execution requires the envelope's commit QC
    /// to satisfy the canonical on-chain lane committee and verifies its aggregate BLS signature.
    /// A pending envelope without a QC can never create contract-visible relay state.
    pub struct RegisterVerifiedLaneRelay {
        /// Canonical finalized lane relay envelope being registered.
        pub envelope: LaneRelayEnvelope,
        /// FASTPQ/AXT proof blob used to verify the relay payload.
        pub proof_blob: ProofBlob,
        /// Reserved business-effect proof slot.
        ///
        /// The first-release runtime deterministically rejects `Some` until an
        /// effect-specific statement is derived from a finalized, QC-anchored
        /// settlement ledger entry. Callers must submit `None`.
        #[norito(skip_serializing_if = "Option::is_none")]
        #[norito(default)]
        pub effect_proof_blob: Option<ProofBlob>,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Persist a proof-backed cross-lane sponsor-vault spend allocation.
    ///
    /// Execution requires the sponsor or its delegated program manager, rejects source heights
    /// beyond the executing block, and permits at most one unexpired lease for each
    /// program/revision/asset/source-dataspace route.
    pub struct RegisterVerifiedFeeSponsorVaultAllocation {
        /// Exact sponsor program authorized to spend the allocation.
        pub program_id: FeeSponsorProgramId,
        /// Immutable program revision bound by the source proof.
        pub program_revision: u64,
        /// Canonical fee asset allocated by the source vault.
        pub asset_definition_id: AssetDefinitionId,
        /// Maximum amount authorized by this spend lease.
        pub verified_allocation: Quantity,
        /// Dataspace containing the authoritative source vault.
        pub source_dataspace_id: DataSpaceId,
        /// Monotonic source consensus height bound by the proof.
        pub source_height: u64,
        /// Source state root committing the vault and budget state.
        pub source_state_root: iroha_crypto::Hash,
        /// Consensus height after which the spend lease expires.
        pub expires_at_height: u64,
        /// Globally unique proof-bound spend lease identifier.
        pub lease_id: iroha_crypto::Hash,
        /// Manifest root committed by the balance proof.
        pub manifest_root: [u8; 32],
        /// FASTPQ/AXT proof blob used to verify the allocation claim.
        pub proof_blob: ProofBlob,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Create a staged sponsor-owned fee sponsor program.
    pub struct CreateFeeSponsorProgram {
        /// Initial fail-closed lifecycle record to persist.
        pub program: FeeSponsorProgram,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Stage one immutable fee sponsor program revision.
    pub struct StageFeeSponsorProgramRevision {
        /// Immutable revision to validate and stage.
        pub revision: FeeSponsorProgramRevision,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Schedule a staged sponsor-program revision for activation.
    pub struct ActivateFeeSponsorProgramRevision {
        /// Program whose staged revision will become active.
        pub program_id: FeeSponsorProgramId,
        /// Exact staged revision number.
        pub revision: u64,
        /// Earliest consensus height at which activation may take effect.
        ///
        /// The runtime postpones activation until every spend lease from an older revision has
        /// expired.
        pub activate_at_height: u64,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Pause an active fee sponsor program.
    pub struct PauseFeeSponsorProgram {
        /// Program to pause.
        pub program_id: FeeSponsorProgramId,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Begin the fail-closed drain phase for a fee sponsor program.
    pub struct BeginCloseFeeSponsorProgram {
        /// Program that must stop accepting new sponsorship.
        pub program_id: FeeSponsorProgramId,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Permanently close a fully drained fee sponsor program.
    pub struct CloseFeeSponsorProgram {
        /// Program to convert into a permanent tombstone.
        pub program_id: FeeSponsorProgramId,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Enroll an exact canonical account in a fee sponsor program.
    pub struct EnrollFeeSponsorBeneficiary {
        /// Program granting eligibility.
        pub program_id: FeeSponsorProgramId,
        /// Canonical beneficiary account to enroll.
        pub beneficiary: AccountId,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Remove an exact canonical account from a fee sponsor program.
    pub struct UnenrollFeeSponsorBeneficiary {
        /// Program revoking eligibility.
        pub program_id: FeeSponsorProgramId,
        /// Canonical beneficiary account to remove.
        pub beneficiary: AccountId,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Transfer assets into a program-isolated fee vault allocation.
    pub struct FundFeeSponsorProgram {
        /// Program receiving the allocation.
        pub program_id: FeeSponsorProgramId,
        /// Canonical asset definition to allocate.
        pub asset_definition_id: AssetDefinitionId,
        /// Positive amount transferred into protocol custody.
        pub amount: Quantity,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    #[cfg_attr(feature = "json", norito(deny_unknown_fields))]
    #[getset(get = "pub")]
    /// Withdraw assets from a paused or closing program vault allocation.
    pub struct WithdrawFeeSponsorProgram {
        /// Program whose allocation is reduced.
        pub program_id: FeeSponsorProgramId,
        /// Canonical asset definition to withdraw.
        pub asset_definition_id: AssetDefinitionId,
        /// Positive amount released from protocol custody.
        pub amount: Quantity,
    }
}

impl PartialOrd for RegisterVerifiedLaneRelay {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RegisterVerifiedLaneRelay {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}

impl PartialOrd for RegisterVerifiedFeeSponsorVaultAllocation {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RegisterVerifiedFeeSponsorVaultAllocation {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}

macro_rules! impl_instruction_ord {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl PartialOrd for $ty {
                fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                    Some(self.cmp(other))
                }
            }

            impl Ord for $ty {
                fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                    self.encode().cmp(&other.encode())
                }
            }
        )+
    };
}

impl_instruction_ord!(
    CreateFeeSponsorProgram,
    StageFeeSponsorProgramRevision,
    ActivateFeeSponsorProgramRevision,
    PauseFeeSponsorProgram,
    BeginCloseFeeSponsorProgram,
    CloseFeeSponsorProgram,
    EnrollFeeSponsorBeneficiary,
    UnenrollFeeSponsorBeneficiary,
    FundFeeSponsorProgram,
    WithdrawFeeSponsorProgram,
);

impl crate::seal::Instruction for SetLaneRelayEmergencyValidators {}
impl crate::seal::Instruction for RegisterVerifiedLaneRelay {}
impl crate::seal::Instruction for RegisterVerifiedFeeSponsorVaultAllocation {}
impl crate::seal::Instruction for CreateFeeSponsorProgram {}
impl crate::seal::Instruction for StageFeeSponsorProgramRevision {}
impl crate::seal::Instruction for ActivateFeeSponsorProgramRevision {}
impl crate::seal::Instruction for PauseFeeSponsorProgram {}
impl crate::seal::Instruction for BeginCloseFeeSponsorProgram {}
impl crate::seal::Instruction for CloseFeeSponsorProgram {}
impl crate::seal::Instruction for EnrollFeeSponsorBeneficiary {}
impl crate::seal::Instruction for UnenrollFeeSponsorBeneficiary {}
impl crate::seal::Instruction for FundFeeSponsorProgram {}
impl crate::seal::Instruction for WithdrawFeeSponsorProgram {}

fn nexus_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

impl<'a> norito::core::DecodeFromSlice<'a> for SetLaneRelayEmergencyValidators {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = nexus_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let lane_id = super::decode_aos_canonical_field::<LaneId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let peers = super::decode_aos_canonical_field::<Vec<PeerId>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let expires_at_height = super::decode_aos_canonical_field::<Option<u64>>(
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
                peers,
                expires_at_height,
                metadata,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RegisterVerifiedLaneRelay {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = nexus_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let envelope = super::decode_aos_canonical_field::<LaneRelayEnvelope>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let proof_blob = super::decode_aos_canonical_field::<ProofBlob>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let effect_proof_blob = if offset == bytes.len() {
            None
        } else {
            super::decode_aos_canonical_field::<Option<ProofBlob>>(
                super::read_aos_field(bytes, &mut offset, flags)?,
                flags,
            )?
        };
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                envelope,
                proof_blob,
                effect_proof_blob,
            },
            offset,
        ))
    }
}

macro_rules! impl_decode_fields {
    ($ty:ident { $($field:ident: $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = nexus_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }

                let mut offset = 0usize;
                $(
                    let $field = super::decode_aos_canonical_field::<$field_ty>(
                        super::read_aos_field(bytes, &mut offset, flags)?,
                        flags,
                    )?;
                )+
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field),+ }, offset))
            }
        }
    };
}

impl_decode_fields!(RegisterVerifiedFeeSponsorVaultAllocation {
    program_id: FeeSponsorProgramId,
    program_revision: u64,
    asset_definition_id: AssetDefinitionId,
    verified_allocation: Quantity,
    source_dataspace_id: DataSpaceId,
    source_height: u64,
    source_state_root: iroha_crypto::Hash,
    expires_at_height: u64,
    lease_id: iroha_crypto::Hash,
    manifest_root: [u8; 32],
    proof_blob: ProofBlob,
});
impl_decode_fields!(CreateFeeSponsorProgram {
    program: FeeSponsorProgram
});
impl_decode_fields!(StageFeeSponsorProgramRevision {
    revision: FeeSponsorProgramRevision
});
impl_decode_fields!(ActivateFeeSponsorProgramRevision {
    program_id: FeeSponsorProgramId,
    revision: u64,
    activate_at_height: u64,
});
impl_decode_fields!(PauseFeeSponsorProgram {
    program_id: FeeSponsorProgramId
});
impl_decode_fields!(BeginCloseFeeSponsorProgram {
    program_id: FeeSponsorProgramId
});
impl_decode_fields!(CloseFeeSponsorProgram {
    program_id: FeeSponsorProgramId
});
impl_decode_fields!(EnrollFeeSponsorBeneficiary {
    program_id: FeeSponsorProgramId,
    beneficiary: AccountId,
});
impl_decode_fields!(UnenrollFeeSponsorBeneficiary {
    program_id: FeeSponsorProgramId,
    beneficiary: AccountId,
});
impl_decode_fields!(FundFeeSponsorProgram {
    program_id: FeeSponsorProgramId,
    asset_definition_id: AssetDefinitionId,
    amount: Quantity,
});
impl_decode_fields!(WithdrawFeeSponsorProgram {
    program_id: FeeSponsorProgramId,
    asset_definition_id: AssetDefinitionId,
    amount: Quantity,
});

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_primitives::numeric::{Numeric, Quantity};
    use norito::codec::{Decode, Encode};
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        block::{BlockHeader, consensus::LaneBlockCommitment},
        nexus::{
            FeeSponsorAssetBudget, FeeSponsorEligibility, FeeSponsorProgram, FeeSponsorProgramId,
            FeeSponsorProgramRevision, FeeSponsorRule, FeeSponsorRuleEffect, LaneId,
        },
    };

    #[derive(Encode)]
    struct ForgedRegisterVerifiedFeeSponsorVaultAllocation {
        program_id: FeeSponsorProgramId,
        program_revision: u64,
        asset_definition_id: AssetDefinitionId,
        verified_allocation: Numeric,
        source_dataspace_id: DataSpaceId,
        source_height: u64,
        source_state_root: iroha_crypto::Hash,
        expires_at_height: u64,
        lease_id: iroha_crypto::Hash,
        manifest_root: [u8; 32],
        proof_blob: ProofBlob,
    }

    fn sample_commitment(height: u64) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height: height,
            lane_id: LaneId::new(3),
            lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
            dataspace_id: DataSpaceId::new(2),
            tx_count: 1,
            total_local_amount: "0.00001".parse().expect("valid settlement quantity"),
            total_xor_due: "0.000005".parse().expect("valid settlement quantity"),
            total_xor_after_haircut: "0.000004".parse().expect("valid settlement quantity"),
            total_xor_variance: "0.000001".parse().expect("valid settlement quantity"),
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        }
    }

    fn sample_header(height: u64) -> BlockHeader {
        BlockHeader::new(
            NonZeroU64::new(height).expect("nonzero height"),
            None,
            None,
            None,
            1_700_000_000_000,
            0,
        )
    }

    fn sample_envelope(height: u64) -> LaneRelayEnvelope {
        LaneRelayEnvelope::new(sample_header(height), None, sample_commitment(height), 0)
            .expect("valid lane relay envelope")
    }

    fn sample_proof_blob(seed: u8) -> ProofBlob {
        ProofBlob {
            payload: vec![seed, seed.wrapping_add(1)],
            expiry_slot: None,
        }
    }

    fn sponsor_account_id() -> AccountId {
        let sponsor_keypair = KeyPair::try_from_seed(vec![0xAB; 32], Algorithm::Ed25519)
            .expect("derive checked Nexus sponsor fixture keypair");
        AccountId::new(sponsor_keypair.public_key().clone())
    }

    fn sample_peer_id(seed: u8) -> PeerId {
        let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked Nexus peer fixture keypair");
        PeerId::new(keypair.public_key().clone())
    }

    fn sample_metadata() -> Metadata {
        let mut metadata = Metadata::default();
        metadata.insert("reason".parse().expect("metadata key"), true);
        metadata
    }

    fn sample_emergency_validators_instruction() -> SetLaneRelayEmergencyValidators {
        SetLaneRelayEmergencyValidators {
            lane_id: LaneId::new(9),
            peers: vec![sample_peer_id(0xC1), sample_peer_id(0xC2)],
            expires_at_height: Some(500),
            metadata: sample_metadata(),
        }
    }

    fn sample_lane_relay_instruction() -> RegisterVerifiedLaneRelay {
        RegisterVerifiedLaneRelay {
            envelope: sample_envelope(5),
            proof_blob: sample_proof_blob(0x01),
            effect_proof_blob: None,
        }
    }

    fn sample_fee_budget_instruction() -> RegisterVerifiedFeeSponsorVaultAllocation {
        RegisterVerifiedFeeSponsorVaultAllocation {
            program_id: sample_fee_sponsor_program().id,
            program_revision: 1,
            asset_definition_id: sample_fee_asset_id(),
            verified_allocation: Quantity::from(10_u32),
            source_dataspace_id: DataSpaceId::new(2),
            source_height: 8,
            source_state_root: iroha_crypto::Hash::new(b"source-state-root"),
            expires_at_height: 108,
            lease_id: iroha_crypto::Hash::new(b"spend-lease"),
            manifest_root: [0x11; 32],
            proof_blob: sample_proof_blob(0x02),
        }
    }

    fn sample_fee_asset_id() -> AssetDefinitionId {
        "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
            .parse()
            .expect("canonical asset definition id")
    }

    fn sample_fee_sponsor_program() -> FeeSponsorProgram {
        let program_id = FeeSponsorProgramId::new(
            sponsor_account_id(),
            "default".parse().expect("program name"),
        );
        FeeSponsorProgram::new(program_id, sponsor_account_id())
    }

    fn sample_fee_sponsor_revision(program_id: FeeSponsorProgramId) -> FeeSponsorProgramRevision {
        FeeSponsorProgramRevision {
            program_id,
            revision: 1,
            eligibility: FeeSponsorEligibility::EnrolledOnly,
            rules: vec![FeeSponsorRule::new(
                "allow_transfer".parse().expect("rule name"),
                FeeSponsorRuleEffect::Allow,
            )],
            asset_budgets: vec![FeeSponsorAssetBudget {
                asset_definition_id: sample_fee_asset_id(),
                per_transaction: Quantity::from(1_u32),
                per_block: Quantity::from(10_u32),
                per_program_epoch: Quantity::from(100_u32),
                per_beneficiary_epoch: Quantity::from(5_u32),
                reserve_floor: "0.1".parse().expect("reserve floor"),
                epoch_length_blocks: NonZeroU64::new(100).expect("nonzero epoch"),
            }],
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
        wire_id: &'static str,
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
    fn register_verified_lane_relay_order_uses_canonical_encoding() {
        let left = sample_lane_relay_instruction();
        let right = RegisterVerifiedLaneRelay {
            envelope: sample_envelope(5),
            proof_blob: sample_proof_blob(0x02),
            effect_proof_blob: None,
        };

        assert_eq!(
            left.partial_cmp(&right),
            Some(left.encode().cmp(&right.encode()))
        );
        assert_eq!(left.cmp(&right), left.encode().cmp(&right.encode()));
    }

    #[test]
    fn register_verified_fee_sponsor_allocation_order_uses_canonical_encoding() {
        let left = sample_fee_budget_instruction();
        let right = RegisterVerifiedFeeSponsorVaultAllocation {
            proof_blob: sample_proof_blob(0x03),
            ..left.clone()
        };

        assert_eq!(
            left.partial_cmp(&right),
            Some(left.encode().cmp(&right.encode()))
        );
        assert_eq!(left.cmp(&right), left.encode().cmp(&right.encode()));
    }

    #[test]
    fn nexus_verified_payload_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(sample_emergency_validators_instruction());
        assert_slice_roundtrip(sample_lane_relay_instruction());
        assert_slice_roundtrip(sample_fee_budget_instruction());
        let program = sample_fee_sponsor_program();
        let id = program.id.clone();
        let beneficiary = sponsor_account_id();
        assert_slice_roundtrip(CreateFeeSponsorProgram {
            program: program.clone(),
        });
        assert_slice_roundtrip(StageFeeSponsorProgramRevision {
            revision: sample_fee_sponsor_revision(id.clone()),
        });
        assert_slice_roundtrip(ActivateFeeSponsorProgramRevision {
            program_id: id.clone(),
            revision: 1,
            activate_at_height: 10,
        });
        assert_slice_roundtrip(PauseFeeSponsorProgram {
            program_id: id.clone(),
        });
        assert_slice_roundtrip(BeginCloseFeeSponsorProgram {
            program_id: id.clone(),
        });
        assert_slice_roundtrip(CloseFeeSponsorProgram {
            program_id: id.clone(),
        });
        assert_slice_roundtrip(EnrollFeeSponsorBeneficiary {
            program_id: id.clone(),
            beneficiary: beneficiary.clone(),
        });
        assert_slice_roundtrip(UnenrollFeeSponsorBeneficiary {
            program_id: id.clone(),
            beneficiary: beneficiary.clone(),
        });
        assert_slice_roundtrip(FundFeeSponsorProgram {
            program_id: id.clone(),
            asset_definition_id: sample_fee_asset_id(),
            amount: Quantity::from(10_u32),
        });
        assert_slice_roundtrip(WithdrawFeeSponsorProgram {
            program_id: id,
            asset_definition_id: sample_fee_asset_id(),
            amount: Quantity::from(1_u32),
        });
    }

    #[cfg(feature = "json")]
    #[test]
    fn withdrawal_json_rejects_caller_selected_destination() {
        let program = sample_fee_sponsor_program();
        let mut value = norito::json::to_value(&WithdrawFeeSponsorProgram {
            program_id: program.id,
            asset_definition_id: sample_fee_asset_id(),
            amount: Quantity::from(1_u32),
        })
        .expect("serialize withdrawal");
        value.as_object_mut().expect("withdrawal object").insert(
            "destination".to_owned(),
            norito::json::to_value(&sponsor_account_id()).expect("serialize forged account"),
        );

        assert!(
            norito::json::from_value::<WithdrawFeeSponsorProgram>(value).is_err(),
            "caller-selected withdrawal destinations are not part of the first-release wire"
        );
    }

    #[test]
    fn negative_numeric_payload_cannot_decode_as_verified_nexus_balance() {
        let valid = sample_fee_budget_instruction();
        let forged = ForgedRegisterVerifiedFeeSponsorVaultAllocation {
            program_id: valid.program_id,
            program_revision: valid.program_revision,
            asset_definition_id: valid.asset_definition_id,
            verified_allocation: Numeric::new(-1_i32, 0),
            source_dataspace_id: valid.source_dataspace_id,
            source_height: valid.source_height,
            source_state_root: valid.source_state_root,
            expires_at_height: valid.expires_at_height,
            lease_id: valid.lease_id,
            manifest_root: [0x11; 32],
            proof_blob: sample_proof_blob(0x02),
        };
        let encoded = forged.encode();

        assert!(
            RegisterVerifiedFeeSponsorVaultAllocation::decode(&mut encoded.as_slice()).is_err(),
            "a signed negative payload must not cross the verified-allocation boundary"
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one stable-ID registry vector enumerates and decodes every verified Nexus instruction in canonical order"
    )]
    fn nexus_verified_payload_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<SetLaneRelayEmergencyValidators>(
                "nexus::SetLaneRelayEmergencyValidators",
            )
            .register_with_id_slice::<RegisterVerifiedLaneRelay>("nexus::RegisterVerifiedLaneRelay")
            .register_with_id_slice::<RegisterVerifiedFeeSponsorVaultAllocation>(
                "nexus::RegisterVerifiedFeeSponsorVaultAllocation",
            )
            .register_with_id_slice::<CreateFeeSponsorProgram>("nexus::CreateFeeSponsorProgram")
            .register_with_id_slice::<StageFeeSponsorProgramRevision>(
                "nexus::StageFeeSponsorProgramRevision",
            )
            .register_with_id_slice::<ActivateFeeSponsorProgramRevision>(
                "nexus::ActivateFeeSponsorProgramRevision",
            )
            .register_with_id_slice::<PauseFeeSponsorProgram>("nexus::PauseFeeSponsorProgram")
            .register_with_id_slice::<BeginCloseFeeSponsorProgram>(
                "nexus::BeginCloseFeeSponsorProgram",
            )
            .register_with_id_slice::<CloseFeeSponsorProgram>("nexus::CloseFeeSponsorProgram")
            .register_with_id_slice::<EnrollFeeSponsorBeneficiary>(
                "nexus::EnrollFeeSponsorBeneficiary",
            )
            .register_with_id_slice::<UnenrollFeeSponsorBeneficiary>(
                "nexus::UnenrollFeeSponsorBeneficiary",
            )
            .register_with_id_slice::<FundFeeSponsorProgram>("nexus::FundFeeSponsorProgram")
            .register_with_id_slice::<WithdrawFeeSponsorProgram>(
                "nexus::WithdrawFeeSponsorProgram",
            );

        assert_registry_decodes(
            &registry,
            "nexus::SetLaneRelayEmergencyValidators",
            sample_emergency_validators_instruction(),
        );
        assert_registry_decodes(
            &registry,
            "nexus::RegisterVerifiedLaneRelay",
            sample_lane_relay_instruction(),
        );
        assert_registry_decodes(
            &registry,
            "nexus::RegisterVerifiedFeeSponsorVaultAllocation",
            sample_fee_budget_instruction(),
        );
        let program = sample_fee_sponsor_program();
        let id = program.id.clone();
        let beneficiary = sponsor_account_id();
        assert_registry_decodes(
            &registry,
            "nexus::CreateFeeSponsorProgram",
            CreateFeeSponsorProgram { program },
        );
        assert_registry_decodes(
            &registry,
            "nexus::StageFeeSponsorProgramRevision",
            StageFeeSponsorProgramRevision {
                revision: sample_fee_sponsor_revision(id.clone()),
            },
        );
        assert_registry_decodes(
            &registry,
            "nexus::ActivateFeeSponsorProgramRevision",
            ActivateFeeSponsorProgramRevision {
                program_id: id.clone(),
                revision: 1,
                activate_at_height: 10,
            },
        );
        assert_registry_decodes(
            &registry,
            "nexus::PauseFeeSponsorProgram",
            PauseFeeSponsorProgram {
                program_id: id.clone(),
            },
        );
        assert_registry_decodes(
            &registry,
            "nexus::BeginCloseFeeSponsorProgram",
            BeginCloseFeeSponsorProgram {
                program_id: id.clone(),
            },
        );
        assert_registry_decodes(
            &registry,
            "nexus::CloseFeeSponsorProgram",
            CloseFeeSponsorProgram {
                program_id: id.clone(),
            },
        );
        assert_registry_decodes(
            &registry,
            "nexus::EnrollFeeSponsorBeneficiary",
            EnrollFeeSponsorBeneficiary {
                program_id: id.clone(),
                beneficiary: beneficiary.clone(),
            },
        );
        assert_registry_decodes(
            &registry,
            "nexus::UnenrollFeeSponsorBeneficiary",
            UnenrollFeeSponsorBeneficiary {
                program_id: id.clone(),
                beneficiary: beneficiary.clone(),
            },
        );
        assert_registry_decodes(
            &registry,
            "nexus::FundFeeSponsorProgram",
            FundFeeSponsorProgram {
                program_id: id.clone(),
                asset_definition_id: sample_fee_asset_id(),
                amount: Quantity::from(10_u32),
            },
        );
        assert_registry_decodes(
            &registry,
            "nexus::WithdrawFeeSponsorProgram",
            WithdrawFeeSponsorProgram {
                program_id: id,
                asset_definition_id: sample_fee_asset_id(),
                amount: Quantity::from(1_u32),
            },
        );
    }
}
