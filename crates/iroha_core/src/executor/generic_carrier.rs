//! Original signed carrier execution and its existing transaction fee owner.

use super::*;
use crate::execution_attempt::{ExecutionAttemptError, norito_decode_attempt_error};
use crate::runtime::generic_carriers::{admission, execution};

/// Verify the borrowed ordinary signed intent, without importing a query-only frame profile.
fn verify_source_signature(
    signed: &SignedTransaction,
) -> Result<(), ExecutionAttemptError<ValidationFail>> {
    let rejected =
        || ValidationFail::NotPermitted("carrier requires its original ordinary signature".into());
    signed
        .payload()
        .validate_fee_payment_intent()
        .map_err(|_| rejected())?;
    let signatory = signed.authority().try_signatory().ok_or_else(rejected)?;
    if signed.multisig_signatures().is_some() {
        return Err(rejected().into());
    }
    let hash = iroha_crypto::HashOf::try_new(signed.payload())
        .map_err(|original| norito_decode_attempt_error(original, |_| rejected()))?;
    iroha_crypto::verify_signature_borrowed(&signed.signature().0, signatory, hash.as_ref())
        .map_err(|error| match error.unsupported_implementation() {
            Some(original) => ExecutionAttemptError::Deferred(
                ivm::error::ExecutionDeferral::CryptoImplementationUnavailable(original).into(),
            ),
            None => ExecutionAttemptError::Rejected(rejected()),
        })
}

/// Quote VM exposure only for the new kernel's original sole carrier envelope.
/// Native selections retain their existing instruction-metered admission exactly.
pub(super) fn fee_bound(
    world: &impl WorldReadOnly,
    payload: &TransactionPayload,
    zk_gas: &ConfidentialGas,
) -> Result<(usize, usize, u64), ExecutionAttemptError<NexusFeeAdmissionError>> {
    if admission::original_candidate(world.runtime_selection(), &payload.instructions)
        .map_err(|error| NexusFeeAdmissionError::ConfigInvalid(error.to_string()))?
        .is_none()
    {
        return fee_bound_for_admission_payload(payload, zk_gas);
    }
    let gas_limit = payload
        .fee_payment
        .gas_limit()
        .map(core::num::NonZeroU64::get)
        .ok_or_else(|| {
            NexusFeeAdmissionError::rejected(
                FeeRejectionCode::InvalidGasLimit,
                "missing gas limit in fee payment intent",
            )
        })?;
    let tx_bytes_len = norito::canonical_frame_len(payload).map_err(|original| {
        norito_decode_attempt_error(original, |error| {
            NexusFeeAdmissionError::ConfigInvalid(format!(
                "failed to encode carrier transaction for fee metering: {error}"
            ))
        })
    })?;
    Ok((tx_bytes_len, 1, gas_limit))
}

impl Executor {
    /// This private entry borrows its instruction directly from the original signed body.
    /// No callback, replay, reveal or caller-built instruction can acquire its source.
    pub(super) fn execute_generic_carrier_transaction(
        state: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        signed: &SignedTransaction,
        InstructionFeeContext {
            tx_bytes_len,
            gas_limit_md,
            gas_asset_opt,
            fee_sponsor,
            skip_nexus_fee,
            ..
        }: InstructionFeeContext,
    ) -> Result<(), ValidationFail> {
        let instruction = admission::original_candidate(
            state.world.runtime_selection.get(),
            signed.instructions(),
        )
        .map_err(|error| ValidationFail::InternalError(error.to_string()))?
        .ok_or_else(|| {
            ValidationFail::NotPermitted("requires one original generic carrier instruction".into())
        })?;
        if is_initial_genesis_context(state)
            || state.genesis_execution_scope.is_some()
            || state.execution_callback_active()
            || state.completing_obligation.is_some()
            || state.native_execution.is_some()
            || !state.deferred_expectations.is_empty()
            || state.world.runtime_selection.get() != state.world.runtime_selection.canonical()
            || signed.authority() != authority
            || signed.network_id() != Some(state.network_id())
            || state.current_entrypoint_index.is_none()
        {
            return Err(ValidationFail::NotPermitted(
                "carrier requires ordinary original external execution".into(),
            ));
        }
        let outer = signed.try_hash_as_entrypoint().map_err(|error| {
            state.attempt_error_to_validation_fail(norito_decode_attempt_error(error, |_| {
                ValidationFail::NotPermitted("invalid original carrier encoding".into())
            }))
        })?;
        let signed_hash = iroha_crypto::HashOf::from_untyped_unchecked(Hash::from(outer));
        if state.current_network_entrypoint_hash != Some(outer)
            || state.tx_call_hash != Some(Hash::from(outer))
            || state.current_tx_hash != Some(signed_hash)
        {
            return Err(ValidationFail::NotPermitted(
                "carrier source differs from the original signed entrypoint".into(),
            ));
        }
        verify_source_signature(signed)
            .map_err(|attempt| state.attempt_error_to_validation_fail(attempt))?;
        let gas_limit = gas_limit_md.ok_or_else(|| {
            ValidationFail::NotPermitted("missing gas limit in fee payment intent".into())
        })?;
        let native_gas = isi_gas::meter_instruction(instruction, &state.zk.gas)
            .map_err(|original| state.defer_execution(original))?;
        let block_remaining = if state.gas_limit_per_block == 0 {
            u64::MAX
        } else {
            state
                .gas_limit_per_block
                .saturating_sub(state.gas_used_in_block_so_far)
        };
        let effective_limit = gas_limit.min(block_remaining);
        if native_gas > effective_limit {
            return Err(ValidationFail::NotPermitted(
                "carrier envelope exceeds the signed or block gas limit".into(),
            ));
        }
        state.admit_authored_execution_effects(core::iter::once(instruction))?;
        state.last_tx_gas_used = native_gas;
        state.record_execution_fee_instructions(1, native_gas)?;
        let report =
            execution::execute(state, authority, instruction, effective_limit - native_gas)
                .map_err(|original| match original {
                    execution::CarrierExecutionRefusal::Execution(original) => {
                        state.defer_execution(original)
                    }
                    execution::CarrierExecutionRefusal::ExecutionRetained => {
                        // The original is still latched. The enclosing executor exports it
                        // once; taking and re-arming here would destroy that custody chain.
                        ValidationFail::InternalError(
                            "local carrier execution attempt did not complete".into(),
                        )
                    }
                    execution::CarrierExecutionRefusal::Storage(original) => {
                        state.arm_local_storage_refusal(original);
                        ValidationFail::InternalError(
                            "local carrier storage admission did not complete".into(),
                        )
                    }
                })?;
        // Semantic VM/effect rejection retains work. A local deferral above keeps
        // the entire unpublished attempt disposable through its original owner.
        state.last_tx_gas_used = native_gas.checked_add(report.gas_used).ok_or_else(|| {
            ValidationFail::InternalError("carrier execution gas overflows u64".into())
        })?;
        state.record_execution_fee_vm_work(report.gas_used)?;
        report.verdict?;
        if state.last_tx_gas_used > effective_limit {
            return Err(ValidationFail::InternalError(
                "carrier engine exceeded its original execution budget".into(),
            ));
        }
        let gas_used = state.last_tx_gas_used;
        Self::settle_live_transaction_fees(
            state,
            authority,
            signed,
            gas_used,
            1,
            tx_bytes_len,
            gas_asset_opt,
            fee_sponsor.as_ref(),
            skip_nexus_fee,
        )
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod integration_tests;
