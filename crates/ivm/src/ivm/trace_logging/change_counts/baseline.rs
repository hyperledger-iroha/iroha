//! Public upper bounds on distinct changed registers in a completed baseline instruction.
//!
//! The anchor `77600a4840`'s table, restored verbatim against the baseline decoder
//! (`ivm_abi::baseline::instruction::wide`). Values, privacy tags, taken conditions and
//! acceleration backends never choose a span. Each row contains at most 255 changed
//! architectural registers after the initial complete 256-register image; r0 is immutable.
//! Invalid register geometry remains the ordinary interpreter's error, not a new trace fault.

use crate::error::VmTrap;
use ivm_abi::baseline::instruction::wide;

use crate::{VMError, registers::BASELINE_REGISTER_COUNT};

fn single(register: usize) -> usize {
    usize::from(register != 0 && register < BASELINE_REGISTER_COUNT)
}

fn pair(left: usize, right: usize) -> usize {
    single(left) + usize::from(right != left) * single(right)
}

/// Count the public maximum of one baseline instruction at the current logical vector length,
/// independently of source and destination values.
pub(in crate::ivm) fn instruction(word: u32, vector_length: usize) -> Result<usize, VMError> {
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
        _ => return Err(VMError::Trap(VmTrap::InvalidOpcode((word & 0xffff) as u16))),
    })
}

#[cfg(test)]
mod tests {
    use ivm_abi::baseline::encoding::wide::encode_rr;

    use super::*;

    #[test]
    fn every_valid_baseline_opcode_has_a_bounded_public_destination_span() {
        for opcode in u8::MIN..=u8::MAX {
            for lanes in [0, 1, 64, usize::MAX] {
                let result = instruction(encode_rr(opcode, 7, 5, 6), lanes);
                assert_eq!(
                    result.is_ok(),
                    wide::is_valid_opcode(opcode),
                    "{opcode:#04x}"
                );
                if let Ok(changes) = result {
                    assert!(
                        changes < BASELINE_REGISTER_COUNT,
                        "{opcode:#04x}: {changes}"
                    );
                }
            }
        }
    }

    #[test]
    fn aliases_zero_and_ignored_operand_bytes_cannot_invent_extra_destinations() {
        use wide::{arithmetic as a, crypto as v, memory as m, system as s};
        for destination in [0, 5, 6, 255] {
            for opcode in [a::ADD, a::CMOV, a::NOT, a::CLZ, s::GETGAS, m::LOAD64] {
                for ignored in [0, 5, 255] {
                    assert_eq!(
                        instruction(encode_rr(opcode, destination, 5, ignored), 64).unwrap(),
                        usize::from(destination != 0)
                    );
                }
            }
            for high in [0, 5, 6, 255] {
                for opcode in [m::LOAD128, v::ED25519BATCHVERIFY] {
                    let expected = usize::from(destination != 0)
                        + usize::from(high != 0 && high != destination);
                    assert_eq!(
                        instruction(encode_rr(opcode, destination, 5, high), 4).unwrap(),
                        expected
                    );
                }
            }
            for opcode in [v::AESENC, v::AESDEC, v::BLAKE2S] {
                assert_eq!(
                    instruction(encode_rr(opcode, destination, 5, 6), 4).unwrap(),
                    usize::from(destination != 0) + usize::from(destination < 255)
                );
            }
        }
    }

    #[test]
    fn vector_and_system_spans_follow_the_public_vector_length() {
        use wide::{crypto as v, system as s};
        for lanes in [0, 1, 6, 64, 300] {
            assert_eq!(
                instruction(encode_rr(v::VADD32, 2, 0, 1), lanes).unwrap(),
                lanes.clamp(1, 255)
            );
            assert_eq!(
                instruction(encode_rr(v::SHA256BLOCK, 2, 0, 1), lanes).unwrap(),
                2 * lanes.clamp(1, 4)
            );
        }
        assert_eq!(instruction(encode_rr(s::SCALL, 0, 0, 0), 1).unwrap(), 255);
        assert_eq!(instruction(encode_rr(s::SYSTEM, 0, 0, 0), 1).unwrap(), 255);
    }
}
