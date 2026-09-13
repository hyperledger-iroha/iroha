//! Canonical compact instruction archives at the browser signing boundary.

use std::panic::{AssertUnwindSafe, catch_unwind};

use iroha_data_model::account::address::ChainDiscriminantGuard;
use iroha_data_model::isi::InstructionBox;

use crate::{
    CodecError, CodecResult, codec_error, instruction_from_json, instruction_to_json_value,
};

fn encode_archive(instruction: &InstructionBox) -> CodecResult<Vec<u8>> {
    let mut bytes = Vec::new();
    norito::codec::encode_adaptive_into(instruction, &mut bytes).map_err(codec_error)?;
    Ok(bytes)
}

/// Encode strict instruction JSON as the compact archive embedded in a transaction.
///
/// This is a native bare encoding, not a public frame with its header removed.
/// `network_prefix` selects account admission for this operation only. Ambient
/// layout flags and the caller's network scope are restored before returning.
pub fn encode_instruction_archive(json_payload: &str, network_prefix: u16) -> CodecResult<Vec<u8>> {
    let _network = ChainDiscriminantGuard::enter(network_prefix);
    let instruction = instruction_from_json(json_payload)?;
    encode_archive(&instruction)
}

/// Decode one exact compact instruction archive into the strict instruction JSON contract.
///
/// Native resource limits, exact consumption and canonical re-encoding apply before
/// any instruction is returned. Public frames and trailing bytes are rejected.
/// Domainless account identities are rendered with the required `network_prefix`.
pub fn decode_instruction_archive(bytes: &[u8], network_prefix: u16) -> CodecResult<String> {
    let _network = ChainDiscriminantGuard::enter(network_prefix);
    // `decode_adaptive` resets its decoder state. Preserve the caller's flags at
    // this public operation boundary before entering that decoder.
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let result = catch_unwind(AssertUnwindSafe(|| {
        let instruction: InstructionBox =
            norito::codec::decode_adaptive(bytes).map_err(codec_error)?;
        if encode_archive(&instruction)?.as_slice() != bytes {
            return Err(CodecError::failure(
                "instruction archive is not canonical Norito",
            ));
        }
        let value = instruction_to_json_value(&instruction)?;
        norito::json::to_json(&value).map_err(codec_error)
    }));
    result.unwrap_or_else(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("unknown panic");
        Err(CodecError::failure(format!(
            "panic during Norito decode: {message}"
        )))
    })
}
