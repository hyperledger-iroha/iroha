//! Instructions of the global chain's AMX two-phase commit (`specs/sumeragi.md` §11).
//!
//! The global chain executes them natively over its AMX World state
//! ([`crate::sumeragi_amx::SumeragiAmxState`]); every record they create is a World write of the
//! executing block, so participants can prove it against the block's certified result.

use super::*;
use crate::sumeragi_amx::{AmxHandoffProofV1, AmxRecordProofV1, AmxTransactionV1};
use iroha_model_base::topology::DataSpaceId;

isi! {
    /// Register a participant dataspace and anchor its foreign-committee tracker at an
    /// independently authenticated epoch context of its consensus instance (§11.7).
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::sumeragi_amx::RegisterAmxDataspaceV1")]
    pub struct RegisterAmxDataspaceV1 {
        /// The dataspace.
        pub dataspace: DataSpaceId,
        /// Consensus instance id `I_J` of the dataspace.
        #[norito(
            with = "crate::json_helpers::fixed_bytes_hex",
            bounded_with = "crate::json_helpers::fixed_bytes_hex::serialize_bounded"
        )]
        pub instance: [u8; 32],
        /// Canonical Norito frame of the trusted `ValidatorEpochContextV1` (its signed genesis
        /// context, or a context its governance certified).
        #[norito(
            with = "crate::json_helpers::base64_vec",
            bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
        )]
        pub anchor: Vec<u8>,
    }
}

isi! {
    /// Record `Begin{x, participants, d}` of an AMX transaction (§11.2).
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::sumeragi_amx::BeginAmxV1")]
    pub struct BeginAmxV1 {
        /// The transaction `X`.
        pub transaction: AmxTransactionV1,
    }
}

isi! {
    /// Relay a participant's `Prepared` record proof (§11.4).
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::sumeragi_amx::RelayAmxPreparedV1")]
    pub struct RelayAmxPreparedV1 {
        /// The record proof.
        pub proof: AmxRecordProofV1,
    }
}

isi! {
    /// Relay a handoff proof of a participant's consensus instance (§11.7).
    #[derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)]
    #[norito_schema(name = "iroha_data_model::isi::sumeragi_amx::RelayAmxHandoffV1")]
    pub struct RelayAmxHandoffV1 {
        /// The participant whose instance hands off.
        pub dataspace: DataSpaceId,
        /// The handoff proof.
        pub proof: AmxHandoffProofV1,
    }
}

impl RegisterAmxDataspaceV1 {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.sumeragi.amx.register_dataspace.v1";
}
impl BeginAmxV1 {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.sumeragi.amx.begin.v1";
}
impl RelayAmxPreparedV1 {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.sumeragi.amx.relay_prepared.v1";
}
impl RelayAmxHandoffV1 {
    /// Stable wire identifier.
    pub const WIRE_ID: &'static str = "iroha.sumeragi.amx.relay_handoff.v1";
}

impl crate::seal::Instruction for RegisterAmxDataspaceV1 {}
impl crate::seal::Instruction for BeginAmxV1 {}
impl crate::seal::Instruction for RelayAmxPreparedV1 {}
impl crate::seal::Instruction for RelayAmxHandoffV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi_amx::{AmxLegV1, AmxTransactionV1};

    #[test]
    fn sumeragi_amx_instructions_decode_through_the_default_registry() {
        let registry = crate::instruction_registry::default();
        let begin = BeginAmxV1 {
            transaction: AmxTransactionV1 {
                legs: vec![
                    AmxLegV1 {
                        dataspace: DataSpaceId::new(1),
                        payload: vec![1],
                    },
                    AmxLegV1 {
                        dataspace: DataSpaceId::new(2),
                        payload: vec![2],
                    },
                ],
                deadline: 9,
                nonce: [3; 32],
            },
        };
        let register = RegisterAmxDataspaceV1 {
            dataspace: DataSpaceId::new(1),
            instance: [4; 32],
            anchor: vec![5, 6],
        };
        for (wire_id, instruction) in [
            (BeginAmxV1::WIRE_ID, InstructionBox::from(begin.clone())),
            (
                RegisterAmxDataspaceV1::WIRE_ID,
                InstructionBox::from(register.clone()),
            ),
        ] {
            assert!(registry.contains(wire_id), "{wire_id}");
            let bytes = norito::encode_canonical(&instruction).expect("encode");
            let decoded: InstructionBox = norito::decode_canonical(&bytes).expect("decode");
            assert_eq!(decoded, instruction);
        }
        assert!(registry.contains(RelayAmxPreparedV1::WIRE_ID));
        assert!(registry.contains(RelayAmxHandoffV1::WIRE_ID));
        let json = norito::json::to_json(&begin).expect("json");
        assert_eq!(
            norito::json::from_str::<BeginAmxV1>(&json).expect("json decode"),
            begin
        );
    }
}
