//! Bridge proof ingestion instructions.
use super::*;

isi! {
    /// Submit a bridge proof artifact for verification and registry retention.
    #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::bridge::SubmitBridgeProof")]
    pub struct SubmitBridgeProof {
        /// Typed bridge proof payload and its payload-owned verifier binding.
        pub proof: crate::bridge::BridgeProof,
    }
}
impl crate::seal::Instruction for SubmitBridgeProof {}
impl SubmitBridgeProof {
    /// Construct a new submission wrapping the provided proof.
    pub fn new(proof: crate::bridge::BridgeProof) -> Self {
        Self { proof }
    }
}
isi! {
    /// Record a bridge receipt and emit a typed bridge event.
    #[derive (crate :: DeriveJsonSerialize , crate :: DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::bridge::RecordBridgeReceipt")]
    pub struct RecordBridgeReceipt {
        /// Bridge receipt payload to record.
        pub receipt: crate::bridge::BridgeReceipt,
    }
}
impl crate::seal::Instruction for RecordBridgeReceipt {}
impl RecordBridgeReceipt {
    /// Construct a new record instruction for the provided receipt.
    pub fn new(receipt: crate::bridge::BridgeReceipt) -> Self {
        Self { receipt }
    }
}
fn bridge_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}
impl<'a> norito::core::DecodeFromSlice<'a> for SubmitBridgeProof {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = bridge_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let proof = super::decode_aos_canonical_field::<crate::bridge::BridgeProof>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { proof }, offset))
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for RecordBridgeReceipt {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = bridge_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }
        let mut offset = 0usize;
        let receipt = super::decode_aos_canonical_field::<crate::bridge::BridgeReceipt>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { receipt }, offset))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::isi::test_support::{
        assert_registry_decodes_registered_type as assert_registry_decodes, assert_slice_roundtrip,
    };
    use crate::{
        bridge::{
            BridgeProof, BridgeProofPayload, BridgeProofRange, BridgeReceipt,
            BridgeTransparentProof,
        },
        proof::ProofBox,
    };
    use iroha_model_base::topology::LaneId;
    fn proof() -> BridgeProof {
        BridgeProof {
            range: BridgeProofRange {
                start_height: 7,
                end_height: 9,
            },
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                verifier_manifest_hash: [0xAB; 32],
                proof: ProofBox::new("halo2/mock".into(), vec![0xDE, 0xAD, 0xBE, 0xEF]),
                recursion_depth: Some(2),
            }),
        }
    }
    fn receipt() -> BridgeReceipt {
        BridgeReceipt {
            lane: LaneId::from(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: Some([0x22; 32]),
            proof_hash: [0x33; 32],
            amount: 42_u64.into(),
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
        }
    }
    #[test]
    fn bridge_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(SubmitBridgeProof::new(proof()));
        assert_slice_roundtrip(RecordBridgeReceipt::new(receipt()));
    }
    #[test]
    fn bridge_registry_decodes_canonical_wire_ids() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<SubmitBridgeProof>(
                "iroha.instruction.v1::bridge::SubmitBridgeProof",
            )
            .register_with_id_slice::<RecordBridgeReceipt>(
                "iroha.instruction.v1::bridge::RecordBridgeReceipt",
            );
        assert_registry_decodes(&registry, SubmitBridgeProof::new(proof()));
        assert_registry_decodes(&registry, RecordBridgeReceipt::new(receipt()));
    }

    #[test]
    fn submit_bridge_proof_json_rejects_retired_replay_witness_field() {
        let instruction = SubmitBridgeProof::new(proof());
        let canonical =
            norito::json::to_json(&instruction).expect("serialize bridge proof instruction JSON");
        assert_eq!(
            norito::json::from_json::<SubmitBridgeProof>(&canonical)
                .expect("canonical bridge proof instruction JSON decodes"),
            instruction
        );
        let retired = canonical.replacen('{', "{\"replay_witness\":null,", 1);
        assert_ne!(retired, canonical);
        assert!(
            norito::json::from_json::<SubmitBridgeProof>(&retired).is_err(),
            "the retired SCCP replay witness must not decode"
        );
    }

    #[test]
    fn record_bridge_receipt_json_rejects_unknown_instruction_fields() {
        let instruction = RecordBridgeReceipt::new(receipt());
        let canonical =
            norito::json::to_json(&instruction).expect("serialize bridge receipt instruction JSON");
        assert_eq!(
            norito::json::from_json::<RecordBridgeReceipt>(&canonical)
                .expect("canonical bridge receipt instruction JSON decodes"),
            instruction
        );
        let hostile = canonical.replacen('{', "{\"adversarial_extension\":null,", 1);
        assert_ne!(hostile, canonical);
        assert!(
            norito::json::from_json::<RecordBridgeReceipt>(&hostile).is_err(),
            "signed receipt instruction JSON must reject unknown fields"
        );
    }
}
