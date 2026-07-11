/// IVM integration helpers.
///
/// This module currently only exposes a runtime cache used by other
/// components for the Iroha Virtual Machine (IVM).
pub mod cache;
/// Host adapter for IVM. See module docs for design and current limitations.
pub mod host;
/// Exact, privacy-safe public return decoding.
pub mod return_value;

use std::num::NonZeroU64;

use iroha_data_model::{
    ValidationFail,
    executor::{IvmAdmissionError, MaxCyclesExceedsUpperBoundInfo},
};

/// Validate and return an artifact's positive cycle limit under node policy.
///
/// This is shared by executable admission and every path that persists an IVM
/// artifact, so zero cannot mean "unlimited" and an artifact cannot be stored
/// for later execution with a header above the configured ceiling.
pub(crate) fn validate_cycle_ceiling(
    meta: &ivm::ProgramMetadata,
    upper_bound: NonZeroU64,
) -> Result<NonZeroU64, IvmAdmissionError> {
    let cycles = NonZeroU64::new(meta.max_cycles).ok_or(IvmAdmissionError::MissingMaxCycles)?;
    if cycles > upper_bound {
        return Err(IvmAdmissionError::MaxCyclesExceedsUpperBound(
            MaxCyclesExceedsUpperBoundInfo {
                max_cycles: cycles.get(),
                upper_bound: upper_bound.get(),
            },
        ));
    }
    Ok(cycles)
}

/// Compute a conservative gas limit for a given cycle budget.
///
/// The interpreter pads traces to exactly `max_cycles` when cycle limits are
/// enabled, charging one unit of gas per padded cycle in addition to the
/// per‑instruction gas schedule. To ensure padding cannot exhaust gas after
/// executing costlier instructions, use the worst-case instruction cost as the
/// multiplier. V1 requires a positive cycle limit, represented by
/// [`NonZeroU64`], so this helper cannot manufacture an unbounded budget.
#[must_use]
pub fn gas_limit_for_cycles(cycles: NonZeroU64) -> u64 {
    cycles
        .get()
        .saturating_mul(ivm::gas::max_instruction_cost())
}

/// Convenience helper to derive a gas limit from program metadata.
///
/// # Errors
/// Returns [`IvmAdmissionError::MissingMaxCycles`] when the artifact encodes
/// the forbidden zero cycle limit.
pub fn gas_limit_for_meta(meta: &ivm::ProgramMetadata) -> Result<u64, IvmAdmissionError> {
    let cycles = NonZeroU64::new(meta.max_cycles).ok_or(IvmAdmissionError::MissingMaxCycles)?;
    Ok(gas_limit_for_cycles(cycles))
}

/// Map a VM execution error into a user-facing validation failure.
#[must_use]
pub fn map_vm_error_to_validation(err: &ivm::VMError) -> ValidationFail {
    ValidationFail::NotPermitted(err.to_string())
}

fn format_vm_diagnostic(diag: &ivm::VmExecutionDiagnostic) -> String {
    let mut message = diag.message.clone();
    use std::fmt::Write as _;
    let _ = write!(&mut message, " at pc=0x{:x}", diag.pc);
    if let Some(function) = diag
        .source
        .as_ref()
        .and_then(|source| source.function.as_deref())
        .or(diag.context.current_function.as_deref())
    {
        let _ = write!(&mut message, " fn={function}");
    }
    if let Some(source) = diag.source.as_ref()
        && let (Some(line), Some(column)) = (source.line, source.column)
    {
        if let Some(path) = source.path.as_deref() {
            let _ = write!(&mut message, " src={path}:{line}:{column}");
        } else {
            let _ = write!(&mut message, " src={line}:{column}");
        }
    }
    if let Some(opcode) = diag.context.opcode {
        let _ = write!(&mut message, " opcode=0x{opcode:02x}");
    }
    if let Some(syscall) = diag.context.syscall {
        let _ = write!(&mut message, " syscall=0x{syscall:02x}");
    }
    message
}

/// Map a VM execution error into a validation failure enriched with VM context.
#[must_use]
pub fn map_vm_error_with_context_to_validation(
    vm: &ivm::IVM,
    err: &ivm::VMError,
) -> ValidationFail {
    if let Some(diag) = vm.last_diagnostic() {
        ValidationFail::NotPermitted(format_vm_diagnostic(diag))
    } else {
        map_vm_error_to_validation(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gas_limit_for_cycles_scales_by_max_instruction_cost() {
        let cost = ivm::gas::max_instruction_cost();
        assert_eq!(gas_limit_for_cycles(NonZeroU64::new(1).unwrap()), cost);
        assert_eq!(
            gas_limit_for_cycles(NonZeroU64::new(2).unwrap()),
            cost.saturating_mul(2)
        );
    }

    #[test]
    fn gas_limit_for_meta_rejects_zero_cycle_budget() {
        let zero = ivm::ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 0,
            abi_version: 1,
        };
        assert!(matches!(
            gas_limit_for_meta(&zero),
            Err(IvmAdmissionError::MissingMaxCycles)
        ));

        let positive = ivm::ProgramMetadata {
            max_cycles: 2,
            ..zero
        };
        assert_eq!(
            gas_limit_for_meta(&positive).unwrap(),
            ivm::gas::max_instruction_cost().saturating_mul(2)
        );
    }

    #[test]
    fn cycle_ceiling_validation_rejects_zero_and_over_bound() {
        let upper_bound = NonZeroU64::new(42).expect("test ceiling is non-zero");
        let mut metadata = ivm::ProgramMetadata {
            max_cycles: 0,
            ..ivm::ProgramMetadata::default()
        };
        assert!(matches!(
            validate_cycle_ceiling(&metadata, upper_bound),
            Err(IvmAdmissionError::MissingMaxCycles)
        ));

        metadata.max_cycles = 42;
        assert_eq!(
            validate_cycle_ceiling(&metadata, upper_bound),
            Ok(upper_bound)
        );

        metadata.max_cycles = 43;
        assert!(matches!(
            validate_cycle_ceiling(&metadata, upper_bound),
            Err(IvmAdmissionError::MaxCyclesExceedsUpperBound(info))
                if info.max_cycles == 43 && info.upper_bound == 42
        ));
    }

    #[test]
    fn compiler_and_node_release_cycle_defaults_match() {
        let compiler_default = ivm::kotodama::compiler::CompilerOptions::default().max_cycles;
        let node_default = iroha_config::parameters::defaults::pipeline::IVM_MAX_CYCLES_UPPER_BOUND;

        assert_eq!(compiler_default, node_default.get());
        assert_eq!(compiler_default, 1_000_000);
    }

    #[test]
    fn vm_error_maps_to_not_permitted() {
        let err = map_vm_error_to_validation(&ivm::VMError::OutOfGas);
        assert!(matches!(err, ValidationFail::NotPermitted(msg) if msg.contains("out of gas")));
    }
}
