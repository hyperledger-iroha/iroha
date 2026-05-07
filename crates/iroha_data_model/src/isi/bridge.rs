//! Bridge proof ingestion instructions.

use super::*;

isi! {
    /// Submit a bridge proof artifact for verification and registry retention.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct SubmitBridgeProof {
        /// Bridge proof payload (ICS or transparent ZK).
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
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
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

isi! {
    /// Record an SCCP message payload for block-level commitment anchoring.
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    pub struct RecordSccpMessage {
        /// Canonical SCCP payload bytes.
        pub payload_bytes: Vec<u8>,
    }
}

impl crate::seal::Instruction for RecordSccpMessage {}

impl RecordSccpMessage {
    /// Construct a new SCCP message record instruction for the provided payload bytes.
    pub fn new(payload_bytes: Vec<u8>) -> Self {
        Self { payload_bytes }
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RecordSccpMessage {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let payload_bytes = super::decode_aos_slice_field::<Vec<u8>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { payload_bytes }, offset))
    }
}

#[cfg(test)]
mod tests {
    use norito::core::DecodeFromSlice;

    use super::*;
    use crate::{
        bridge::{
            BridgeProof, BridgeProofPayload, BridgeProofRange, BridgeReceipt,
            BridgeTransparentProof,
        },
        nexus::LaneId,
        proof::ProofBox,
    };

    fn proof() -> BridgeProof {
        BridgeProof {
            range: BridgeProofRange {
                start_height: 7,
                end_height: 9,
            },
            manifest_hash: [0xAB; 32],
            payload: BridgeProofPayload::TransparentZk(BridgeTransparentProof {
                proof: ProofBox::new("halo2/mock".into(), vec![0xDE, 0xAD, 0xBE, 0xEF]),
                recursion_depth: Some(2),
            }),
            pinned: true,
        }
    }

    fn receipt() -> BridgeReceipt {
        BridgeReceipt {
            lane: LaneId::from(1),
            direction: b"mint".to_vec(),
            source_tx: [0x11; 32],
            dest_tx: Some([0x22; 32]),
            proof_hash: [0x33; 32],
            amount: 42,
            asset_id: b"wBTC#btc".to_vec(),
            recipient: b"alice@main".to_vec(),
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

    fn assert_registry_decodes<T>(registry: &crate::isi::InstructionRegistry, value: T)
    where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let wire_id = std::any::type_name::<T>();
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn bridge_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(SubmitBridgeProof::new(proof()));
        assert_slice_roundtrip(RecordBridgeReceipt::new(receipt()));
        assert_slice_roundtrip(RecordSccpMessage::new(vec![0xCA, 0xFE]));
    }

    #[test]
    fn bridge_registry_decodes_type_names() {
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<SubmitBridgeProof>()
            .register_slice::<RecordBridgeReceipt>()
            .register_slice::<RecordSccpMessage>();

        assert_registry_decodes(&registry, SubmitBridgeProof::new(proof()));
        assert_registry_decodes(&registry, RecordBridgeReceipt::new(receipt()));
        assert_registry_decodes(&registry, RecordSccpMessage::new(vec![0xCA, 0xFE]));
    }
}
