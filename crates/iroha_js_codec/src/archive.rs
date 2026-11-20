//! Canonical explicit-layout instruction archives at the browser signing boundary.

use std::panic::{AssertUnwindSafe, catch_unwind};

use iroha_data_model::account::address::ChainDiscriminantGuard;
use iroha_data_model::isi::InstructionBox;
use norito::core::WireLayout;

use crate::{
    CodecError, CodecResult, codec_error, instruction_from_json, instruction_to_json_value,
};

fn encode_archive(instruction: &InstructionBox, layout: WireLayout) -> CodecResult<Vec<u8>> {
    norito::codec::encode_bare_with_layout(instruction, layout).map_err(codec_error)
}

/// Encode strict instruction JSON as the compact archive embedded in a transaction.
///
/// This is a native bare encoding, not a public frame with its header removed.
/// `network_prefix` selects account admission for this operation only. Ambient
/// layout flags and the caller's network scope are restored before returning.
///
/// # Errors
///
/// Returns an error when the JSON is malformed, violates a strict instruction
/// contract or names an account outside `network_prefix`, or when the instruction
/// cannot be Norito-encoded.
pub fn encode_instruction_archive(json_payload: &str, network_prefix: u16) -> CodecResult<Vec<u8>> {
    encode_instruction_archive_with_layout(json_payload, network_prefix, WireLayout::Compact)
}

/// Encode a typed instruction archive with an explicitly selected layout.
///
/// # Errors
/// Returns an error for invalid JSON, account admission or Norito encoding.
pub fn encode_instruction_archive_with_layout(
    json_payload: &str,
    network_prefix: u16,
    layout: WireLayout,
) -> CodecResult<Vec<u8>> {
    let _network = ChainDiscriminantGuard::enter(network_prefix);
    let instruction = instruction_from_json(json_payload)?;
    encode_archive(&instruction, layout)
}

/// Decode one exact compact instruction archive into the strict instruction JSON contract.
///
/// Native resource limits, exact consumption and canonical re-encoding apply before
/// any instruction is returned. Public frames and trailing bytes are rejected.
/// Domainless account identities are rendered with the required `network_prefix`.
///
/// # Errors
///
/// Returns an error when the bytes are not exactly one canonical instruction archive
/// (including trailing bytes, public frames and decoder panics), or when the decoded
/// instruction has no strict JSON rendering.
pub fn decode_instruction_archive(bytes: &[u8], network_prefix: u16) -> CodecResult<String> {
    decode_instruction_archive_with_layout(bytes, network_prefix, WireLayout::Compact)
}

/// Decode exactly one typed instruction archive in its owner's declared layout.
///
/// # Errors
/// Returns an error for malformed, mismatched or noncanonical bytes or invalid JSON rendering.
pub fn decode_instruction_archive_with_layout(
    bytes: &[u8],
    network_prefix: u16,
    layout: WireLayout,
) -> CodecResult<String> {
    let _network = ChainDiscriminantGuard::enter(network_prefix);
    // The canonical bare decoder resets its thread-local decode state. Preserve
    // the caller's selection across that reset, including error paths.
    let _flags = norito::core::WireLayoutGuard::enter(layout);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let instruction: InstructionBox =
            norito::codec::decode_bare_with_layout(bytes, layout).map_err(codec_error)?;
        if encode_archive(&instruction, layout)?.as_slice() != bytes {
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
