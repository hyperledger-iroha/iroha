//! Exact operand-consumption contracts for caller-clobbered register eligibility.

use crate::ir::Instr;

/// These emitters consume all source registers before staging can destroy one.
/// This does not make a register survive the instruction: normal CFG/tuple
/// liveness and split-range checks still exclude every value needed afterward.
pub(super) fn operands_staged_before_clobber(instruction: &Instr) -> bool {
    if matches!(instruction, Instr::Call { .. } | Instr::CallMulti { .. }) {
        return true;
    }
    #[cfg(test)]
    if CONSERVATIVE_HOST_OPERANDS.get() {
        return false;
    }
    matches!(
        instruction,
        // Both sources pass through compiler::emit_parallel_register_moves;
        // literals/spills are loaded only after every register source is consumed.
        Instr::NumericBinary { .. }
            | Instr::NumericCompare { .. }
            | Instr::StateSet { .. }
            | Instr::PathMapKeyNorito { .. }
            // These single-operand emitters read their sole source into r10
            // before any syscall, literal load or result assignment.
            | Instr::PointerToNorito { .. }
            | Instr::StateGet { .. }
    )
}

#[cfg(test)]
std::thread_local! {
    static CONSERVATIVE_HOST_OPERANDS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Measure the same compiler with only the prior operand-register exclusion.
#[cfg(test)]
pub(crate) fn with_conservative_host_operands<R>(body: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            CONSERVATIVE_HOST_OPERANDS.set(self.0);
        }
    }
    let _restore = Restore(CONSERVATIVE_HOST_OPERANDS.replace(true));
    body()
}

#[cfg(test)]
mod tests;
