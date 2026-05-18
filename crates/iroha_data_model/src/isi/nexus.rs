use super::*;
use crate::{
    account::AccountId,
    metadata::Metadata,
    nexus::{LaneId, LaneRelayEnvelope, ProofBlob},
    peer::PeerId,
};
use iroha_primitives::numeric::Numeric;

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
    /// Persist a verified private-source lane relay so contracts can consume it by reference.
    pub struct RegisterVerifiedLaneRelay {
        /// Canonical lane relay envelope being registered.
        pub envelope: LaneRelayEnvelope,
        /// FASTPQ/AXT proof blob used to verify the relay payload.
        pub proof_blob: ProofBlob,
    }
}

iroha_data_model_derive::model_single! {
    #[derive(Debug, Clone, PartialEq, Eq)]
    #[derive(getset::Getters)]
    #[derive(Decode, Encode)]
    #[derive(iroha_schema::IntoSchema)]
    #[getset(get = "pub")]
    /// Persist a verified Nexus public XOR fee-budget cache for asynchronous DPN fee admission.
    pub struct RegisterVerifiedNexusFeeBudget {
        /// Sponsor or payer account whose Nexus public XOR balance was verified.
        pub sponsor_account_id: AccountId,
        /// Fee asset selector used for the verified balance, fixed operationally to public XOR.
        pub fee_asset_id: String,
        /// Verified public Nexus balance for the sponsor and fee asset.
        pub verified_balance: Numeric,
        /// Manifest root committed by the balance proof.
        pub manifest_root: [u8; 32],
        /// FASTPQ/AXT proof blob used to verify the public balance claim.
        pub proof_blob: ProofBlob,
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

impl PartialOrd for RegisterVerifiedNexusFeeBudget {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RegisterVerifiedNexusFeeBudget {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}

impl crate::seal::Instruction for SetLaneRelayEmergencyValidators {}
impl crate::seal::Instruction for RegisterVerifiedLaneRelay {}
impl crate::seal::Instruction for RegisterVerifiedNexusFeeBudget {}

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
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                envelope,
                proof_blob,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RegisterVerifiedNexusFeeBudget {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = nexus_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let sponsor_account_id = super::decode_aos_canonical_field::<AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let fee_asset_id = super::decode_aos_canonical_field::<String>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let verified_balance = super::decode_aos_canonical_field::<Numeric>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let manifest_root = super::decode_aos_canonical_field::<[u8; 32]>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let proof_blob = super::decode_aos_canonical_field::<ProofBlob>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                sponsor_account_id,
                fee_asset_id,
                verified_balance,
                manifest_root,
                proof_blob,
            },
            offset,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Algorithm, KeyPair};
    use norito::codec::Encode;
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        block::{BlockHeader, consensus::LaneBlockCommitment},
        nexus::LaneId,
    };

    fn sample_commitment(height: u64) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height: height,
            lane_id: LaneId::new(3),
            dataspace_id: DataSpaceId::new(2),
            tx_count: 1,
            total_local_micro: 10,
            total_xor_due_micro: 5,
            total_xor_after_haircut_micro: 4,
            total_xor_variance_micro: 1,
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
        LaneRelayEnvelope::new(
            sample_header(height),
            None,
            None,
            sample_commitment(height),
            0,
        )
        .expect("valid lane relay envelope")
    }

    fn sample_proof_blob(seed: u8) -> ProofBlob {
        ProofBlob {
            payload: vec![seed, seed.wrapping_add(1)],
            expiry_slot: None,
        }
    }

    fn sponsor_account_id() -> AccountId {
        let sponsor_keypair = KeyPair::from_seed(vec![0xAB; 32], Algorithm::Ed25519);
        AccountId::new(sponsor_keypair.public_key().clone())
    }

    fn sample_peer_id(seed: u8) -> PeerId {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
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
        }
    }

    fn sample_fee_budget_instruction() -> RegisterVerifiedNexusFeeBudget {
        RegisterVerifiedNexusFeeBudget {
            sponsor_account_id: sponsor_account_id(),
            fee_asset_id: "xor#universal".to_owned(),
            verified_balance: Numeric::from(10_u32),
            manifest_root: [0x11; 32],
            proof_blob: sample_proof_blob(0x02),
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
        };

        assert_eq!(
            left.partial_cmp(&right),
            Some(left.encode().cmp(&right.encode()))
        );
        assert_eq!(left.cmp(&right), left.encode().cmp(&right.encode()));
    }

    #[test]
    fn register_verified_nexus_fee_budget_order_uses_canonical_encoding() {
        let left = sample_fee_budget_instruction();
        let right = RegisterVerifiedNexusFeeBudget {
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
    }

    #[test]
    fn nexus_verified_payload_registry_decodes_stable_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<SetLaneRelayEmergencyValidators>(
                "nexus::SetLaneRelayEmergencyValidators",
            )
            .register_with_id_slice::<RegisterVerifiedLaneRelay>("nexus::RegisterVerifiedLaneRelay")
            .register_with_id_slice::<RegisterVerifiedNexusFeeBudget>(
                "nexus::RegisterVerifiedNexusFeeBudget",
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
            "nexus::RegisterVerifiedNexusFeeBudget",
            sample_fee_budget_instruction(),
        );
    }
}
