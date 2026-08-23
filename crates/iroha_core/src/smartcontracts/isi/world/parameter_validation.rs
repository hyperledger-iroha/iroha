//! Focused validation for world-level runtime parameters.

use iroha_data_model::{
    isi::error::{InstructionExecutionError, InvalidParameterError},
    parameter::{Parameter, SmartContractParameter},
};

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
            InvalidParameterError::SmartContract(
                format!(
                    "{name} exceeds the ABI V1 heap window: {limit} > {} bytes",
                    iroha_data_model::parameter::system::IVM_HEAP_MAX_BYTES
                )
                .into(),
            ),
        ));
    }
    Ok(())
}
