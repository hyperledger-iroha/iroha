#[cfg(feature = "json")]
use norito::json::{FastJsonWrite, JsonSerialize};

use super::*;
use crate::runtime::RuntimeUpgradeId;

isi! {
    /// Propose a runtime upgrade by submitting a manifest.
    pub struct ProposeRuntimeUpgrade {
        /// Canonical V1 manifest bytes encoded as Norito JSON/binary at admission time.
        pub manifest_bytes: Vec<u8>,
    }
}

isi! {
    /// Activate a previously proposed runtime upgrade at the window start height.
    pub struct ActivateRuntimeUpgrade {
        /// Content-address (blake2b32) of the canonical manifest bytes.
        pub id: RuntimeUpgradeId,
    }
}

isi! {
    /// Cancel a proposed runtime upgrade prior to its start height.
    pub struct CancelRuntimeUpgrade {
        /// Content-address (blake2b32) of the canonical manifest bytes.
        pub id: RuntimeUpgradeId,
    }
}

// Seal implementations so these types participate as instructions
impl crate::seal::Instruction for ProposeRuntimeUpgrade {}
impl crate::seal::Instruction for ActivateRuntimeUpgrade {}
impl crate::seal::Instruction for CancelRuntimeUpgrade {}

impl<'a> norito::core::DecodeFromSlice<'a> for ProposeRuntimeUpgrade {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let manifest_bytes = super::decode_aos_slice_field::<Vec<u8>>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { manifest_bytes }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for ActivateRuntimeUpgrade {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let id = super::decode_aos_slice_field::<RuntimeUpgradeId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { id }, offset))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for CancelRuntimeUpgrade {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = norito::core::effective_decode_flags()
            .unwrap_or_else(norito::core::default_encode_flags);
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let id = super::decode_aos_slice_field::<RuntimeUpgradeId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { id }, offset))
    }
}

// Stable wire IDs for runtime-upgrade instructions
impl ProposeRuntimeUpgrade {
    /// Norito wire identifier for proposing a runtime upgrade.
    pub const WIRE_ID: &'static str = "iroha.runtime_upgrade.propose";
}
impl ActivateRuntimeUpgrade {
    /// Norito wire identifier for activating a runtime upgrade.
    pub const WIRE_ID: &'static str = "iroha.runtime_upgrade.activate";
}
impl CancelRuntimeUpgrade {
    /// Norito wire identifier for canceling a runtime upgrade.
    pub const WIRE_ID: &'static str = "iroha.runtime_upgrade.cancel";
}

#[cfg(feature = "json")]
impl FastJsonWrite for ProposeRuntimeUpgrade {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"manifest_bytes\":");
        JsonSerialize::json_serialize(&self.manifest_bytes, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for ActivateRuntimeUpgrade {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"id\":");
        JsonSerialize::json_serialize(&self.id, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for CancelRuntimeUpgrade {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"id\":");
        JsonSerialize::json_serialize(&self.id, out);
        out.push('}');
    }
}

#[cfg(test)]
mod tests {
    use norito::core::DecodeFromSlice;

    use super::*;

    #[test]
    fn encode_decode_runtime_upgrade_isis() {
        let pid = RuntimeUpgradeId([0xAA; 32]);
        let propose = ProposeRuntimeUpgrade {
            manifest_bytes: vec![1, 2, 3],
        };
        let act = ActivateRuntimeUpgrade { id: pid };
        let can = CancelRuntimeUpgrade { id: pid };
        let b1 = norito::to_bytes(&propose).unwrap();
        let b2 = norito::to_bytes(&act).unwrap();
        let b3 = norito::to_bytes(&can).unwrap();
        let r1: ProposeRuntimeUpgrade = norito::decode_from_bytes(&b1).unwrap();
        let r2: ActivateRuntimeUpgrade = norito::decode_from_bytes(&b2).unwrap();
        let r3: CancelRuntimeUpgrade = norito::decode_from_bytes(&b3).unwrap();
        assert_eq!(propose, r1);
        assert_eq!(act, r2);
        assert_eq!(can, r3);
    }

    #[test]
    fn runtime_upgrade_isi_decode_from_slice_roundtrips() {
        let pid = RuntimeUpgradeId([0xBB; 32]);
        let propose = ProposeRuntimeUpgrade {
            manifest_bytes: vec![1, 2, 3, 4],
        };
        let activate = ActivateRuntimeUpgrade { id: pid };
        let cancel = CancelRuntimeUpgrade { id: pid };

        let propose_bytes = propose.encode();
        let (decoded, used) =
            ProposeRuntimeUpgrade::decode_from_slice(&propose_bytes).expect("decode propose");
        assert_eq!(used, propose_bytes.len());
        assert_eq!(decoded, propose);

        let activate_bytes = activate.encode();
        let (decoded, used) =
            ActivateRuntimeUpgrade::decode_from_slice(&activate_bytes).expect("decode activate");
        assert_eq!(used, activate_bytes.len());
        assert_eq!(decoded, activate);

        let cancel_bytes = cancel.encode();
        let (decoded, used) =
            CancelRuntimeUpgrade::decode_from_slice(&cancel_bytes).expect("decode cancel");
        assert_eq!(used, cancel_bytes.len());
        assert_eq!(decoded, cancel);
    }

    #[test]
    fn runtime_upgrade_registry_decodes_stable_ids() {
        let pid = RuntimeUpgradeId([0xCC; 32]);
        let registry = crate::isi::InstructionRegistry::new()
            .register_with_id_slice::<ProposeRuntimeUpgrade>(ProposeRuntimeUpgrade::WIRE_ID)
            .register_with_id_slice::<ActivateRuntimeUpgrade>(ActivateRuntimeUpgrade::WIRE_ID)
            .register_with_id_slice::<CancelRuntimeUpgrade>(CancelRuntimeUpgrade::WIRE_ID);

        let propose = ProposeRuntimeUpgrade {
            manifest_bytes: vec![5, 6, 7, 8],
        };
        let (payload, flags) = norito::codec::encode_with_header_flags(&propose);
        let framed =
            norito::core::frame_bare_with_header_flags::<ProposeRuntimeUpgrade>(&payload, flags)
                .expect("frame propose");
        let decoded = crate::isi::InstructionRegistry::decode(
            &registry,
            ProposeRuntimeUpgrade::WIRE_ID,
            &framed,
        )
        .expect("registered propose")
        .expect("decode propose");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);

        let activate = ActivateRuntimeUpgrade { id: pid };
        let (payload, flags) = norito::codec::encode_with_header_flags(&activate);
        let framed =
            norito::core::frame_bare_with_header_flags::<ActivateRuntimeUpgrade>(&payload, flags)
                .expect("frame activate");
        let decoded = crate::isi::InstructionRegistry::decode(
            &registry,
            ActivateRuntimeUpgrade::WIRE_ID,
            &framed,
        )
        .expect("registered activate")
        .expect("decode activate");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);

        let cancel = CancelRuntimeUpgrade { id: pid };
        let (payload, flags) = norito::codec::encode_with_header_flags(&cancel);
        let framed =
            norito::core::frame_bare_with_header_flags::<CancelRuntimeUpgrade>(&payload, flags)
                .expect("frame cancel");
        let decoded = crate::isi::InstructionRegistry::decode(
            &registry,
            CancelRuntimeUpgrade::WIRE_ID,
            &framed,
        )
        .expect("registered cancel")
        .expect("decode cancel");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }
}
