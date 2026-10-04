//! Exact straight-line reuse of the numeric kernel's zero scale, rounding and trap inputs.
//!
//! Every typed syscall, canonical operand validation and failure mode remains present.
//! Only this emitter's own unconditional numeric setup establishes public-zero facts.
//! Facts belong to one emitted basic block and are invalidated by actual physical writes,
//! literal relocations, calls, control flow and all unrecognized instructions/syscalls.
use super::*;

#[cfg(test)]
std::thread_local! {
    static RETAIN_NUMERIC_ZEROS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
pub(super) fn with_original_zeros<R>(body: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            RETAIN_NUMERIC_ZEROS.set(self.0);
        }
    }
    let _restore = Restore(RETAIN_NUMERIC_ZEROS.replace(true));
    body()
}
fn retain_original() -> bool {
    #[cfg(test)]
    if RETAIN_NUMERIC_ZEROS.get() {
        return true;
    }
    false
}
/// Knowledge about r12–r14 in the current physical emission prefix only.
pub(super) struct Block {
    cursor: usize,
    zero: u8,
}
impl Block {
    pub(super) fn new(start: usize) -> Self {
        Self {
            cursor: start,
            zero: 0,
        }
    }
    fn clear_written(&mut self, register: usize) {
        if (12..=14).contains(&register) {
            self.zero &= !(1 << (register - 12));
        }
    }
    fn observe_word(&mut self, word: u32) {
        use instruction::wide::{arithmetic as a, memory as m, system as s};
        let opcode = instruction::wide::opcode(word);
        match opcode {
            a::ADDI => {
                // Nonliteral NOPs may be patched into calls/branches later. No
                // physical knowledge survives such an unresolved relocation.
                if word == encode_nop() {
                    self.zero = 0;
                    return;
                }
                let destination = instruction::wide::rd(word);
                self.clear_written(destination);
                // Even an exact ADDI from public r0 is only a scanned write:
                // internal conditional branches may skip it. Establish facts
                // exclusively after our own unconditional numeric setup below.
            }
            a::ADD
            | a::SUB
            | a::AND
            | a::OR
            | a::XOR
            | a::SLL
            | a::SRL
            | a::SRA
            | a::SLT
            | a::SLTU
            | a::CMOV
            | a::NOT
            | a::NEG
            | a::SEQ
            | a::SNE
            | a::MUL
            | a::MULH
            | a::MULHU
            | a::MULHSU
            | a::DIV
            | a::DIVU
            | a::REM
            | a::REMU
            | a::ROTL
            | a::ROTR
            | a::POPCNT
            | a::CLZ
            | a::CTZ
            | a::ISQRT
            | a::MIN
            | a::MAX
            | a::ANDI
            | a::ORI
            | a::XORI
            | a::CMOVI
            | a::ROTL_IMM
            | a::ROTR_IMM
            | a::ABS
            | a::DIV_CEIL
            | a::GCD
            | a::MEAN
            | m::LOAD64
            | m::LDLIT
            | m::LDI64 => self.clear_written(instruction::wide::rd(word)),
            m::STORE64 => {} // Stores do not write a scalar register.
            s::SCALL | s::SYSTEM => {
                let number = if opcode == s::SCALL {
                    word & 0xff
                } else {
                    word & 0x00ff_ffff
                };
                // These exact sole-kernel operations write only r10/r11 (the
                // comparisons only r10), including recoverable arithmetic faults.
                // No other host syscall is inferred to preserve r12–r14.
                if !preserves_inputs(number) {
                    self.zero = 0;
                }
            }
            _ => self.zero = 0,
        }
    }
    fn observe(&mut self, code: &[u8], fixups: &LiteralFixups) {
        assert!(self.cursor.is_multiple_of(4) && code.len().is_multiple_of(4));
        let literals = fixups.borrow();
        let cursor = self.cursor;
        // emit_literal_load appends a fixup at the current code.len() only;
        // the original emitter never truncates/reorders code or fixups before
        // this block finishes. Locate this prefix's first pending relocation
        // in logarithmic time, then visit only newly emitted fixups.
        let first = literals.partition_point(|(at, _, _)| *at < cursor);
        debug_assert!(literals.get(first).is_none_or(|(at, _, _)| *at >= cursor));
        let mut pending = literals[first..].iter().peekable();
        for at in (self.cursor..code.len()).step_by(4) {
            if let Some((offset, destination, _)) = pending.peek().copied()
                && *offset == at
            {
                // The current NOP will become LDLIT/LDI64 at its authenticated
                // exact destination. It can clobber an allocatable r12–r14.
                self.clear_written(usize::from(*destination));
                pending.next();
            } else {
                self.observe_word(u32::from_le_bytes(code[at..at + 4].try_into().unwrap()));
            }
        }
        self.cursor = code.len();
    }
    pub(super) fn emit_trap_inputs(
        &mut self,
        code: &mut Vec<u8>,
        fixups: &LiteralFixups,
    ) -> Result<(), String> {
        self.observe(code, fixups);
        // This point follows complete straight-line NumericBinary operand staging.
        // Earlier inline branch targets end within their original IR instruction;
        // incoming SSA block edges target the block start, which starts unknown.
        // Therefore each emitted or already-proven assignment dominates this
        // numeric syscall. Never relearn a fact from a scanned caller instruction.
        for register in 12..=14 {
            let mask = 1 << (register - 12);
            if retain_original() || self.zero & mask == 0 {
                push_word(code, encode_addi(register, 0, 0)?);
            }
            self.zero |= mask;
        }
        self.cursor = code.len();
        Ok(())
    }
}
fn preserves_inputs(number: u32) -> bool {
    matches!(
        number,
        syscalls::SYSCALL_INT_ADD
            | syscalls::SYSCALL_INT_SUB
            | syscalls::SYSCALL_INT_MUL
            | syscalls::SYSCALL_INT_DIV
            | syscalls::SYSCALL_INT_REM
            | syscalls::SYSCALL_DECIMAL_ADD
            | syscalls::SYSCALL_DECIMAL_SUB
            | syscalls::SYSCALL_DECIMAL_MUL
            | syscalls::SYSCALL_DECIMAL_DIV_EXACT
            | syscalls::SYSCALL_QUANTITY_ADD
            | syscalls::SYSCALL_QUANTITY_SUB
            | syscalls::SYSCALL_QUANTITY_MUL_DECIMAL
            | syscalls::SYSCALL_QUANTITY_DIV_DECIMAL_EXACT
            | syscalls::SYSCALL_QUANTITY_RATIO_EXACT
            | syscalls::SYSCALL_INT_EQ
            | syscalls::SYSCALL_INT_NE
            | syscalls::SYSCALL_INT_LT
            | syscalls::SYSCALL_INT_LE
            | syscalls::SYSCALL_INT_GT
            | syscalls::SYSCALL_INT_GE
            | syscalls::SYSCALL_DECIMAL_EQ
            | syscalls::SYSCALL_DECIMAL_NE
            | syscalls::SYSCALL_DECIMAL_LT
            | syscalls::SYSCALL_DECIMAL_LE
            | syscalls::SYSCALL_DECIMAL_GT
            | syscalls::SYSCALL_DECIMAL_GE
            | syscalls::SYSCALL_QUANTITY_EQ
            | syscalls::SYSCALL_QUANTITY_NE
            | syscalls::SYSCALL_QUANTITY_LT
            | syscalls::SYSCALL_QUANTITY_LE
            | syscalls::SYSCALL_QUANTITY_GT
            | syscalls::SYSCALL_QUANTITY_GE
    )
}
#[cfg(test)]
mod native_pairs;
#[cfg(test)]
mod tests;
