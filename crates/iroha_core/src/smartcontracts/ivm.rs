/// IVM integration helpers.
///
/// This module currently only exposes a runtime cache used by other
/// components for the Iroha Virtual Machine (IVM).
pub mod cache;
/// Host adapter for IVM. See module docs for design and current limitations.
pub mod host;

use iroha_data_model::ValidationFail;

/// Compute a conservative gas limit for a given cycle budget.
///
/// The interpreter pads traces to exactly `max_cycles` when cycle limits are
/// enabled, charging one unit of gas per padded cycle in addition to the
/// per‑instruction gas schedule. To ensure padding cannot exhaust gas after
/// executing costlier instructions, use the worst-case instruction cost as the
/// multiplier. A zero cycle limit means "uncapped", so return an effectively
/// unbounded gas budget.
#[must_use]
pub fn gas_limit_for_cycles(cycles: u64) -> u64 {
    if cycles == 0 {
        u64::MAX
    } else {
        cycles.saturating_mul(ivm::gas::max_instruction_cost())
    }
}

/// Convenience helper to derive a gas limit from program metadata.
#[must_use]
pub fn gas_limit_for_meta(meta: &ivm::ProgramMetadata) -> u64 {
    gas_limit_for_cycles(meta.max_cycles)
}

/// Map a VM execution error into a user-facing validation failure.
#[must_use]
pub fn map_vm_error_to_validation(err: &ivm::VMError) -> ValidationFail {
    ValidationFail::NotPermitted(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gas_limit_for_cycles_zero_is_unbounded() {
        assert_eq!(gas_limit_for_cycles(0), u64::MAX);
    }

    #[test]
    fn gas_limit_for_cycles_scales_by_max_instruction_cost() {
        let cost = ivm::gas::max_instruction_cost();
        assert_eq!(gas_limit_for_cycles(1), cost);
        assert_eq!(gas_limit_for_cycles(2), cost.saturating_mul(2));
    }

    #[test]
    fn gas_limit_for_meta_uses_cycle_budget() {
        let meta = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 0,
            abi_version: 1,
        };
        assert_eq!(gas_limit_for_meta(&meta), u64::MAX);
    }

    #[test]
    fn vm_error_maps_to_not_permitted() {
        let err = map_vm_error_to_validation(&ivm::VMError::OutOfGas);
        assert!(matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("out of gas")));
    }
}
