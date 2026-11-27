//! One-use custody of an exact sole instruction in the original external signed transaction.

use iroha_crypto::Hash;
use iroha_data_model::{
    ValidationFail,
    account::AccountId,
    isi::{
        InstructionBox,
        child_runtime::{AttestChildRuntimeSupportV1, AuthorizeChildRuntimeActionV1},
        error::InstructionExecutionError,
    },
    transaction::{Executable, SignedTransaction},
};

use crate::{
    execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error},
    state::{StateReadOnly, StateTransaction},
};

/// This private constructor is called only at the original signed native-vector boundary.
/// The fixed-size token is neither a World value nor a replayable policy flag.
pub(crate) struct DirectInstruction {
    wire: &'static str,
    digest: [u8; 32],
}

fn fingerprint(
    value: &impl norito::NoritoSerialize,
) -> Result<[u8; 32], norito::core::DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| {
        let mut codec = None;
        let hash = Hash::new_from_writer(|writer| {
            writer.write_all(b"iroha:runtime:child-admin:direct-instruction:v1\0")?;
            norito::core::write_canonical_to_writer(value, writer).map_err(|cause| {
                codec = Some(cause);
                std::io::Error::from(std::io::ErrorKind::Other)
            })
        });
        if let Some(error) = codec {
            return Err(error);
        }
        hash.map(Into::into).map_err(norito::Error::Io)
    })
}

/// Capture only one reviewed direct instruction with exact outer signature and source identity.
/// VM, sealed-reveal, batch, substituted and nested instructions never receive this token.
pub(crate) fn capture_direct(
    state: &mut StateTransaction<'_, '_>,
    signed: &SignedTransaction,
    authority: &AccountId,
    instruction: &InstructionBox,
    index: usize,
    direct: bool,
) -> Result<Option<DirectInstruction>, ValidationFail> {
    let any = instruction.as_any();
    let wire = if any.is::<AuthorizeChildRuntimeActionV1>() {
        AuthorizeChildRuntimeActionV1::WIRE_ID
    } else if any.is::<AttestChildRuntimeSupportV1>() {
        AttestChildRuntimeSupportV1::WIRE_ID
    } else {
        return Ok(None);
    };
    let Executable::Instructions(items) = signed.instructions() else {
        return Ok(None);
    };
    if !direct
        || !super::ordinary_context(state)
        || index != 0
        || items.len() != 1
        || items.first() != Some(instruction)
        || signed.authority() != authority
        || signed.network_id() != Some(state.network_id())
        || state.current_network_entrypoint_hash != Some(signed.hash_as_entrypoint())
        || state.tx_call_hash != Some(Hash::from(signed.hash_as_entrypoint()))
        || state.current_tx_hash != Some(signed.hash())
        || state.current_entrypoint_index.is_none()
    {
        return Ok(None);
    }
    match crate::query::signer_check::native_signed_transaction_frame_attempt_v1(signed) {
        Ok(_) => {}
        Err(ExecutionAttemptError::Rejected(_)) => return Ok(None),
        Err(ExecutionAttemptError::Deferred(reason)) => return Err(state.defer_execution(reason)),
    }
    let digest = if let Some(value) = any.downcast_ref::<AuthorizeChildRuntimeActionV1>() {
        fingerprint(value)
    } else {
        fingerprint(
            any.downcast_ref::<AttestChildRuntimeSupportV1>()
                .expect("reviewed typed child instruction"),
        )
    }
    .map_err(|error| {
        state.attempt_error_to_validation_fail(canonical_decode_attempt_error(error, |error| {
            ValidationFail::NotPermitted(error.to_string())
        }))
    })?;
    Ok(Some(DirectInstruction { wire, digest }))
}

pub(super) fn consume(
    state: &mut StateTransaction<'_, '_>,
    wire: &'static str,
    instruction: &impl norito::NoritoSerialize,
) -> Result<(), InstructionExecutionError> {
    // Consume before any signature validation, callback or fallible canonical operation.
    // A caught rejection or unwind cannot reuse the original instruction authority.
    let source = state
        .child_runtime_admin_entrypoint
        .take()
        .ok_or_else(|| super::rejected("requires its sole original signed native instruction"))?;
    if !super::ordinary_context(state) || source.wire != wire {
        return Err(super::rejected(
            "direct child instruction source does not match execution",
        ));
    }
    let digest = fingerprint(instruction).map_err(|error| {
        state.attempt_error_to_instruction_error(canonical_decode_attempt_error(
            error,
            super::rejected,
        ))
    })?;
    if source.digest != digest {
        return Err(super::rejected(
            "child instruction was substituted after source capture",
        ));
    }
    Ok(())
}
