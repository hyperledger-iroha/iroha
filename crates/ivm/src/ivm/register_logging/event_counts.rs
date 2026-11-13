//! Public-geometry upper bounds for complete register-event batches.
//!
//! These bounds count actual ordinary value reads and separate value/tag writes,
//! not native proof packet slots. No register value, privacy tag, host outcome or
//! private witness selects a reservation. Traps/untaken operations may use fewer
//! rows; their unused quota returns only to the enclosing batch.

use crate::{VMError, instruction::wide, syscalls};
use iroha_allocation::AllocationRefusal;

/// Public argument route selected before root preparation effects.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RootArguments {
    Empty,
    Prepared,
    DefaultHost,
}

/// Root tables write four descriptors, read them for the frame, set/tag SP and
/// sentinel, and read four descriptors again for typed validation: sixteen rows.
/// The default input route additionally writes its name and reads its result.
pub(super) fn root(arguments: RootArguments, native: bool) -> usize {
    let native = if native { 6 } else { 0 };
    match arguments {
        RootArguments::Empty | RootArguments::Prepared => 16 + native,
        RootArguments::DefaultHost => 18 + syscall(syscalls::SYSCALL_GET_PUBLIC_INPUT) + native,
    }
}

/// Whole reserved/staged syscall subtree, including net host writes on failure.
/// Metadata/quote/body callbacks are masked; resumed VM net changes own at most
/// one row per writable register (31). Ordinary privacy entry, sanitization, restoration and final tags log.
pub(super) fn syscall(number: u32) -> usize {
    let inputs = crate::ivm::syscall_public_input_registers(number);
    let outputs = crate::ivm::syscall_public_output_registers(number);
    let read_inputs = if number == syscalls::SYSCALL_PRIVATE_NUMERIC_VALCOM {
        // This boundary validates two tags directly, without value reads.
        0
    } else {
        inputs.len()
    };
    let output_only = outputs
        .iter()
        .filter(|register| !inputs.contains(register))
        .count();
    let final_tag = usize::from(matches!(
        number,
        syscalls::SYSCALL_GET_PRIVATE_INPUT | syscalls::SYSCALL_PRIVATE_NUMERIC_VALCOM
    ));
    // Reserved prepare/OOG restores two writes per saved output; a host return
    // instead publishes its net changes and possibly one explicit final tag.
    read_inputs
        + 3 * output_only
        + (2 * output_only).max(crate::parallel::REGISTER_COUNT - 1 + final_tag)
}

fn writes(register: usize) -> usize {
    2 * usize::from(register != 0)
}
fn overflow() -> VMError {
    VMError::AllocationDeferred(AllocationRefusal::DemandOverflow)
}

/// Count one entire instruction including any delayed native finish observations.
///
/// The caller reserves after ordinary opcode/base-gas validation but before gas
/// debit, native preflight, or guest effects. Keep this batch live through the
/// next loop head's `native_finish_step`; padding and root snapshots read no GPRs.
pub(super) fn instruction(word: u32, strict_return: bool, native: bool) -> Result<usize, VMError> {
    use wide::{arithmetic as a, control as c, memory as m, system as s};
    let destination = writes(wide::rd(word));
    let opcode = wide::opcode(word);
    let ordinary = match opcode {
        a::ADD
        | a::SUB
        | a::AND
        | a::OR
        | a::XOR
        | a::SLL
        | a::SRL
        | a::SRA
        | a::MUL
        | a::MULH
        | a::MULHU
        | a::MULHSU
        | a::DIV
        | a::DIVU
        | a::REM
        | a::REMU
        | a::SLT
        | a::SLTU
        | a::SEQ
        | a::SNE
        | a::CMOV
        | a::ROTL
        | a::ROTR
        | a::MIN
        | a::MAX
        | a::DIV_CEIL
        | a::GCD
        | a::MEAN => 2 + destination,
        a::ADDI
        | a::ANDI
        | a::ORI
        | a::XORI
        | a::NEG
        | a::NOT
        | a::ROTL_IMM
        | a::ROTR_IMM
        | a::POPCNT
        | a::CLZ
        | a::CTZ
        | a::ISQRT
        | a::ABS
        | a::CMOVI
        | m::LOAD64 => 1 + destination,
        m::LDLIT | m::LDI64 => destination,
        m::STORE64 => 2,
        c::BEQ | c::BNE | c::BLT | c::BGE | c::BLTU | c::BGEU => 2,
        c::JALR => {
            if strict_return {
                6
            } else {
                1 + destination
            }
        }
        c::JAL => {
            destination
                + if strict_return && wide::rd(word) == 1 {
                    9
                } else {
                    0
                }
        }
        c::JALS => 2 + if strict_return { 9 } else { 0 },
        c::HALT | c::JMP => 0,
        s::SCALL => syscall(wide::syscall_number(word)),
        _ => return Err(VMError::InvalidOpcode(opcode)),
    };
    ordinary
        .checked_add(if native { native_observations(word) } else { 0 })
        .ok_or_else(overflow)
}

fn native_observations(word: u32) -> usize {
    if let Some((_, right)) = crate::execution_packets::public_scalar_operands(word) {
        // Original destination before/after observations occur even for r0;
        // only the native packet publisher suppresses its hardwired-zero write.
        return 3 + usize::from(right.is_some());
    }
    match wide::opcode(word) {
        wide::memory::LDI64 => 2,
        wide::memory::LOAD64 => 5,
        wide::memory::STORE64 => 4,
        wide::control::JALR => 4,
        _ => 0,
    }
}

#[cfg(test)]
mod tests;
