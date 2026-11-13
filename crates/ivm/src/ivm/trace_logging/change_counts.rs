//! Public upper bounds on distinct changed registers in a completed instruction.
//!
//! Values, privacy tags, taken conditions and acceleration backends never choose
//! a span. Each row contains at most 31 changed architectural registers after
//! the initial complete 32-register image; r0 is immutable. Invalid register
//! geometry remains the ordinary interpreter's error, not a new trace fault.

use crate::{VMError, instruction::wide};

fn single(register: usize) -> usize {
    usize::from(register != 0 && register < crate::parallel::REGISTER_COUNT)
}

/// Count the public maximum, independently of source and destination values.
pub(super) fn instruction(word: u32) -> Result<usize, VMError> {
    use wide::{arithmetic as a, control as c, memory as m, system as s};
    let rd = wide::rd(word);
    Ok(match wide::opcode(word) {
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
        | a::MEAN
        | a::ADDI
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
        | m::LOAD64
        | m::LDLIT
        | m::LDI64
        | c::JAL
        | c::JALR => single(rd),
        c::JALS => 1,
        s::SCALL => crate::parallel::REGISTER_COUNT - 1,
        m::STORE64 | c::BEQ | c::BNE | c::BLT | c::BGE | c::BLTU | c::BGEU | c::HALT | c::JMP => 0,
        opcode => return Err(VMError::InvalidOpcode(opcode)),
    })
}

#[cfg(test)]
mod tests;
