//! Re-export VM error types from `ivm_abi`.
pub use ivm_abi::error::*;

/// Preserve a local operational refusal when mapping a deterministic decode fault.
///
/// The complete original error carries its finite pool/release observation.
/// Converting it to malformed input would make transaction validity depend on
/// local resource pressure. Semantic failures retain the caller's existing map.
pub fn preserve_execution_deferral(error: VMError, malformed: VMError) -> VMError {
    if error.execution_deferral().is_some() {
        error
    } else {
        malformed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_mapping_keeps_complete_operational_errors_and_existing_semantic_faults() {
        let budget = iroha_allocation::AllocationBudget::new(1);
        let occupied = budget.try_reserve_bytes(1).unwrap();
        let refusal = budget.try_reserve_bytes(1).unwrap_err();
        let error = VMError::AllocationDeferred(refusal);
        assert_eq!(
            preserve_execution_deferral(error.clone(), VMError::NoritoInvalid),
            error
        );
        for reason in [
            ExecutionDeferral::AllocationUnavailable,
            ExecutionDeferral::ActiveMemoryCapacity,
        ] {
            let error = VMError::ExecutionDeferred(reason);
            assert_eq!(
                preserve_execution_deferral(error.clone(), VMError::DecodeError),
                error
            );
        }
        let wrapped = VMError::Metered {
            gas: 17,
            source: Box::new(error),
        };
        assert_eq!(
            preserve_execution_deferral(wrapped.clone(), VMError::DecodeError),
            wrapped
        );
        assert_eq!(
            preserve_execution_deferral(VMError::MemoryOutOfBounds, VMError::NoritoInvalid),
            VMError::NoritoInvalid
        );
        drop(occupied);
    }
}
