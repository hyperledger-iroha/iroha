//! Public upper bounds on distinct changed registers in a completed instruction.
//!
//! Values, privacy tags, taken conditions and acceleration backends never choose
//! a span. Each row contains at most 255 changed architectural registers after
//! the initial complete 256-register image; r0 is immutable. Invalid register
//! geometry remains the ordinary interpreter's error, not a new trace fault.

use crate::{VMError, instruction::wide};

fn single(register: usize) -> usize {
    usize::from(register != 0 && register < 256)
}
fn pair(left: usize, right: usize) -> usize {
    single(left) + usize::from(right != left) * single(right)
}

/// Count the public maximum, independently of source and destination values.
pub(super) fn instruction(word: u32, vector_length: usize) -> Result<usize, VMError> {
    use wide::{arithmetic as a, control as c, crypto as v, memory as m, system as s, zk as z};
    let rd = wide::rd(word);
    let lanes = vector_length.max(1);
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
        | z::FADD
        | z::FSUB
        | z::FMUL
        | z::FINV
        | m::LOAD64
        | m::LDLIT
        | m::LDI64
        | s::GETGAS
        | c::JAL
        | c::JALR
        | v::POSEIDON2
        | v::POSEIDON6
        | v::ED25519VERIFY
        | v::ECDSAVERIFY
        | v::DILITHIUMVERIFY => single(rd),
        m::LOAD128 | v::ED25519BATCHVERIFY => pair(rd, wide::rs2(word)),
        v::AESENC | v::AESDEC | v::BLAKE2S => pair(rd, rd + 1),
        c::JALS => 1,
        v::VADD32 | v::VADD64 | v::VAND | v::VXOR | v::VOR | v::VROT32 => lanes.min(255),
        v::SHA256BLOCK => 2 * lanes.min(4),
        s::SCALL | s::SYSTEM => 255,
        m::STORE64
        | m::STORE128
        | c::BEQ
        | c::BNE
        | c::BLT
        | c::BGE
        | c::BLTU
        | c::BGEU
        | c::JR
        | c::HALT
        | c::JMP
        | v::SETVL
        | v::PARBEGIN
        | v::PAREND
        | v::SHA3BLOCK
        | z::ASSERT
        | z::ASSERT_EQ
        | z::ASSERT_RANGE => 0,
        _ => return Err(VMError::InvalidOpcode((word & 0xffff) as u16)),
    })
}

#[cfg(test)]
mod tests;
