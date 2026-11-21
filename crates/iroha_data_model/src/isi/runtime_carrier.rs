//! Immutable raw artifact staging under its separately selected runtime service.

use super::*;

/// Maximum complete artifact bytes, including the exact C10 header and code sections.
pub const MAX_CARRIER_ARTIFACT_BYTES_V1: usize = 1_048_576;

isi! {
    /// Stage exact restricted-profile raw code without granting invocation or activation authority.
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::runtime_carrier::StageRuntimeCarrierArtifactV1")]
    pub struct StageRuntimeCarrierArtifactV1 {
        /// Exact source data space and domain-separated hash of the complete artifact.
        pub artifact_id: crate::smart_contract::ContractArtifactId,
        /// Complete ABI V1 C10 carrier image, raw main entrypoint without CNTR or debug sections.
        pub code: Vec<u8>,
    }
}

impl crate::seal::Instruction for StageRuntimeCarrierArtifactV1 {}
impl StageRuntimeCarrierArtifactV1 {
    /// Immutable wire identity of the independent artifact staging addition.
    pub const WIRE_ID: &'static str = "iroha.runtime.generic_carrier.stage_artifact.v1";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_payload_and_tolerant_instruction_envelope_roundtrip() {
        let instruction = StageRuntimeCarrierArtifactV1 {
            artifact_id: crate::smart_contract::ContractArtifactId::new(
                iroha_model_base::topology::DataSpaceId::new(71),
                iroha_crypto::Hash::new(b"full artifact"),
            ),
            code: vec![0, 1, 2, 3],
        };
        let encoded = norito::encode_canonical(&instruction).unwrap();
        assert_eq!(
            norito::decode_canonical::<StageRuntimeCarrierArtifactV1>(&encoded).unwrap(),
            instruction
        );
        let envelope: InstructionBox = instruction.into();
        assert_eq!(
            instruction_wire_id(&envelope),
            Some(StageRuntimeCarrierArtifactV1::WIRE_ID)
        );
        let encoded = norito::encode_canonical(&envelope).unwrap();
        assert_eq!(
            norito::decode_canonical::<InstructionBox>(&encoded).unwrap(),
            envelope
        );
    }
}
