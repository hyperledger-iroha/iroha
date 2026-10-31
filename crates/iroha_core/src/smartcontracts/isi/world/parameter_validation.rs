//! Focused validation for world-level runtime parameters.

use iroha_data_model::{
    isi::error::{InstructionExecutionError, InvalidParameterError},
    parameter::{
        Parameter, ParliamentLifecycleParametersV1, SmartContractParameter,
        parliament_lifecycle::ParliamentLifecycleParameterErrorV1,
    },
};

use crate::execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error};
use mv::storage::StorageReadOnly;

/// Reject IVM heap limits that cannot be represented by the ABI V1 address window.
pub(super) fn validate_ivm_heap_parameter(
    parameter: &Parameter,
) -> Result<(), InstructionExecutionError> {
    let (name, limit) = match parameter {
        Parameter::SmartContract(SmartContractParameter::Memory(limit)) => {
            ("smart_contract.memory", limit.get())
        }
        Parameter::Executor(SmartContractParameter::Memory(limit)) => {
            ("executor.memory", limit.get())
        }
        _ => return Ok(()),
    };
    if limit > iroha_data_model::parameter::system::IVM_HEAP_MAX_BYTES {
        return Err(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(format!(
                "{name} exceeds the ABI V1 heap window: {limit} > {} bytes",
                iroha_data_model::parameter::system::IVM_HEAP_MAX_BYTES
            )),
        ));
    }
    Ok(())
}

/// Validate the committed Parliament lifecycle payload before parameter publication.
///
/// The reserved identity always has the same strict schema, including before the
/// anonymous-ballot execution path is installed. Arbitrary custom parameters keep their
/// ordinary admission rules; a malformed lifecycle policy never selects defaults. Local
/// codec refusals remain deferred attempts for the World owner to retain.
pub(super) fn validate_parliament_lifecycle_parameter(
    parameter: &Parameter,
) -> Result<(), ExecutionAttemptError<InstructionExecutionError>> {
    let Parameter::Custom(custom) = parameter else {
        return Ok(());
    };
    if custom.id() != ParliamentLifecycleParametersV1::parameter_id() {
        return Ok(());
    }
    let invalid = |message| {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message))
    };
    ParliamentLifecycleParametersV1::from_custom_parameter(custom).map_err(
        |error| match error {
            ParliamentLifecycleParameterErrorV1::Codec(original) => {
                canonical_decode_attempt_error(original, |error| invalid(error.to_string()))
            }
            error => ExecutionAttemptError::Rejected(invalid(error.to_string())),
        },
    )?;
    Ok(())
}

/// Admit only a new private-settlement schedule rooted in the exact current policy.
/// Captured protocol drain authority is never caller-writable.
pub(super) fn prepare_private_settlement_parameter(
    parameter: &Parameter,
    transaction: &crate::state::StateTransaction<'_, '_>,
) -> Result<Option<Parameter>, ExecutionAttemptError<InstructionExecutionError>> {
    use iroha_data_model::privacy::{
        PrivacyProtocolIdV1, PrivacyProtocolPolicyKeyV1, PrivateSettlementPolicyV1,
    };
    let Parameter::Custom(custom) = parameter else {
        return Ok(None);
    };
    if custom.id() != PrivateSettlementPolicyV1::parameter_id() {
        return Ok(None);
    }
    let invalid = |message| {
        InstructionExecutionError::InvalidParameter(InvalidParameterError::SmartContract(message))
    };
    let convert = |error| crate::private_settlement::policy::attempt(error).map_rejection(invalid);
    let protocol_key =
        PrivacyProtocolPolicyKeyV1::new(PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1);
    let protocol = transaction
        .world
        .privacy_protocol_policies
        .get(&protocol_key);
    let height = transaction.block_height();
    let current = crate::private_settlement::policy::read(
        transaction.world.parameters.get(),
        protocol,
        height,
    )
    .map_err(|error| error.map_rejection(invalid))?;
    let mut proposed = PrivateSettlementPolicyV1::from_custom_parameter(custom).map_err(convert)?;
    if proposed.current_limits != current.current_limits
        || proposed.protocol_closure_drain != current.protocol_closure_drain
        || current.pending_update.is_some()
    {
        return Err(invalid("private-settlement update must preserve current limits and original protocol drain, with no existing pending update".into()).into());
    }
    let Some(pending) = proposed.pending_update.as_mut() else {
        return Err(
            invalid("private-settlement update requires a future pending schedule".into()).into(),
        );
    };
    // The executing root, not the caller, supplies original schedule authority.
    // SPEC: §13.4 HC492 misclassifies an ordinary update as initial genesis policy.
    pending.scheduled_at_height = if cfg!(all(test, sumeragi_core_mutation = "HC492")) {
        1
    } else {
        height
    };
    proposed
        .validate_at_committed_height(height)
        .map_err(|error| invalid(error.to_string()))?;
    proposed
        .validate_protocol_policy(protocol)
        .map_err(convert)?;
    Ok(Some(Parameter::Custom(
        proposed.to_custom_parameter().map_err(convert)?,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::parameter::CustomParameter;
    use iroha_primitives::json::Json;

    #[test]
    fn parliament_lifecycle_validation_reserves_only_its_exact_identity() {
        let malformed = Json::from_str_norito("{}").expect("JSON object");
        let lifecycle = Parameter::Custom(CustomParameter::new(
            ParliamentLifecycleParametersV1::parameter_id().clone(),
            malformed.clone(),
        ));
        assert!(matches!(
            validate_parliament_lifecycle_parameter(&lifecycle),
            Err(ExecutionAttemptError::Rejected(
                InstructionExecutionError::InvalidParameter(_)
            ))
        ));
        let unrelated = Parameter::Custom(CustomParameter::new(
            "application_policy".parse().expect("parameter identity"),
            malformed,
        ));
        assert!(validate_parliament_lifecycle_parameter(&unrelated).is_ok());
        let regular = Parameter::SmartContract(SmartContractParameter::Memory(
            core::num::NonZeroU64::new(1).expect("nonzero"),
        ));
        assert!(validate_parliament_lifecycle_parameter(&regular).is_ok());
    }
}
