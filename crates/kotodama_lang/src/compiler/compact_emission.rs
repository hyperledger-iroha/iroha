//! Compact existing nominal-abort and authenticated numeric operand emission.
//!
//! No source check, typed consumer, reserved register, callable, frame or
//! metadata field is omitted. Nominal abort's terminal body stays inside one
//! original function range; normal source edges cannot fall through into it.

use super::*;

#[cfg(test)]
std::thread_local! {
    static RETAIN_SCALAR_COMPACT_EMISSION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
pub(super) fn with_scalar_emission<R>(body: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            RETAIN_SCALAR_COMPACT_EMISSION.set(self.0);
        }
    }
    let _restore = Restore(RETAIN_SCALAR_COMPACT_EMISSION.replace(true));
    body()
}
pub(super) fn retain_scalar() -> bool {
    #[cfg(test)]
    if RETAIN_SCALAR_COMPACT_EMISSION.get() {
        return true;
    }
    false
}
pub(super) fn share_nominal_abort(program: &ir::Program) -> bool {
    !retain_scalar()
        && program
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instrs)
            .filter(|instruction| matches!(instruction, Instr::AbortIf { .. }))
            .take(2)
            .count()
            == 2
}
pub(super) fn emit_nominal_abort_tail(code: &mut Vec<u8>) -> Result<(), String> {
    // Keep the original publication and exact reserved-register protocol.
    // request_contract_abort marks the canonical VM halted synchronously; no
    // stack adjustment or caller result publication belongs to a failed call.
    push_syscall(code, syscalls::SYSCALL_INPUT_PUBLISH_TLV);
    for register in 12..=15 {
        push_word(code, encode_addi(register, 0, 0)?);
    }
    push_syscall(code, syscalls::SYSCALL_CONTRACT_ABORT);
    Ok(())
}

#[cfg(test)]
mod native_pairs;
#[cfg(test)]
mod tests;
