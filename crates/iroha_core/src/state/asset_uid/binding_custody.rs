//! Transaction-owned binding account decoding before a fixed UID may escape.
use super::*;
use crate::execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error};
use iroha_allocation::{AllocatorRefusal, ChargedBufferError, PrepaidSharedError};
use iroha_data_model::asset::uid::{
    AssetUidBindingDecodeError, decode_asset_uid_binding_with_owned_controls,
};
use norito::core::{OwnedPreparedDecodeError, PreparedDecodeError, PreparedDecodeScopeError};

#[path = "binding_custody/retry.rs"]
pub(crate) mod retry;

type DecodeError = OwnedPreparedDecodeError<AssetUidBindingDecodeError>;

/// Preserve each original leaf/control refusal before formatting malformed State.
fn decode_attempt(error: DecodeError) -> ExecutionAttemptError<Error> {
    match error {
        OwnedPreparedDecodeError::Admission(original) => {
            ExecutionAttemptError::Deferred(original.into())
        }
        OwnedPreparedDecodeError::Decode(PreparedDecodeError::Codec(original)) => {
            canonical_decode_attempt_error(original, |error| {
                reject(format!("invalid asset UID state: {error}"))
            })
        }
        OwnedPreparedDecodeError::Decode(PreparedDecodeError::Destination(
            AssetUidBindingDecodeError::Allocation(original),
        )) => match original {
            ChargedBufferError::Admission(original) => {
                ExecutionAttemptError::Deferred(original.into())
            }
            original @ ChargedBufferError::Allocator { .. } => ExecutionAttemptError::Deferred(
                AllocatorRefusal::try_from(original)
                    .expect("matched physical UID binding refusal")
                    .into(),
            ),
        },
        OwnedPreparedDecodeError::Decode(PreparedDecodeError::Scope(
            PreparedDecodeScopeError::Allocation(original @ PrepaidSharedError::Allocator { .. }),
        )) => ExecutionAttemptError::Deferred(
            AllocatorRefusal::try_from(original)
                .expect("matched physical UID binding control refusal")
                .into(),
        ),
        error => {
            ExecutionAttemptError::Rejected(reject(format!("invalid asset UID state: {error}")))
        }
    }
}

pub(super) fn binding_uid_with_owner(
    world: &impl WorldReadOnly,
    definition: &AssetDefinitionId,
    account: &AccountId,
    owner: Option<&WorldTransaction<'_, '_>>,
) -> Result<Option<[u8; 32]>, Error> {
    let Some(owner) = owner else {
        return binding(world, definition, account).map(|value| value.map(|value| value.uid));
    };
    let path = binding_key(definition, account);
    let bytes = world.smart_contract_state().get(&path);
    let budget = owner.sccp_committee_generations.allocation_budget();
    if let Some(original) = owner.uid_binding_execution.as_ref() {
        return original
            .read(&path, bytes.map(Vec::as_slice), definition, account, budget)
            .map_err(|error| owner.attempt_error_to_instruction_error(error));
    }
    // Explicit standalone callers have no retained native execution job. Native
    // validation installs its original bank before opening any World transaction.
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    // SPEC: §13.4 HC975 substitutes the binding's original graph/control pool.
    #[cfg(all(test, sumeragi_core_mutation = "HC975"))]
    let foreign = iroha_allocation::AllocationBudget::new(budget.limit_bytes());
    #[cfg(all(test, sumeragi_core_mutation = "HC975"))]
    let budget = &foreign;
    let binding = decode_asset_uid_binding_with_owned_controls(
        bytes,
        norito::canonical_decode_limits(bytes.len()),
        budget,
    )
    .map_err(|error| owner.attempt_error_to_instruction_error(decode_attempt(error)))?;
    // Only this fixed-size commitment escapes; the actual account drops while its
    // original charges are still retained. Roster/key/map ownership remains open.
    validate_binding(binding.canonical(), definition, account).map(Some)
}

#[cfg(test)]
#[path = "binding_custody/tests.rs"]
mod tests;
