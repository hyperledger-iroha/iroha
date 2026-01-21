//! Kotodama helpers for emitting the canonical IVM wide-opcode encodings.
//!
//! These routines provide strongly typed builders for the 8-bit opcode layout
//! and are used throughout the compiler to assemble final bytecode. All code
//! generation now flows through these helpers; alternate instruction formats no
//! longer participate in the pipeline.
//!
//! 128-bit loads/stores dedicate the third operand slot to the high
//! destination/source register. They therefore do not offer an inline
//! displacement; callers must pre-adjust the base register (e.g., chunked frame
//! updates via [`encode_addi`]) before issuing the wide memory operation.

use crate::{encoding::wide, instruction};

/// Errors produced when constructing wide-opcode encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WideEncodingError {
    /// Immediate value does not fit in the target bit-width.
    ImmediateOutOfRange { value: i16, min: i16, max: i16 },
}

impl core::fmt::Display for WideEncodingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            WideEncodingError::ImmediateOutOfRange { value, min, max } => {
                write!(f, "immediate {value} out of range [{min}, {max}]")
            }
        }
    }
}

impl std::error::Error for WideEncodingError {}

/// Encode a wide register-register arithmetic instruction.
#[allow(dead_code)]
pub fn encode_rr(op: u8, rd: u8, rs1: u8, rs2: u8) -> u32 {
    wide::encode_rr(op, rd, rs1, rs2)
}

/// Encode `rd = rs1 + rs2` using the wide opcode space.
#[allow(dead_code)]
pub fn encode_add(rd: u8, rs1: u8, rs2: u8) -> u32 {
    encode_rr(instruction::wide::arithmetic::ADD, rd, rs1, rs2)
}

/// Encode `rd = rs` using the wide opcode space.
#[allow(dead_code)]
pub fn encode_move(rd: u8, rs: u8) -> u32 {
    encode_addi(rd, rs, 0)
}

/// Encode `rd = rs1 - rs2` using the wide opcode space.
#[allow(dead_code)]
pub fn encode_sub(rd: u8, rs1: u8, rs2: u8) -> u32 {
    encode_rr(instruction::wide::arithmetic::SUB, rd, rs1, rs2)
}

/// Encode a wide register-immediate arithmetic instruction.
#[allow(dead_code)]
pub fn encode_ri(op: u8, rd: u8, rs1: u8, imm: i8) -> u32 {
    wide::encode_ri(op, rd, rs1, imm)
}

/// Encode `rd = rs1 + imm` using the wide opcode space.
///
/// Callers must ensure `imm` fits in 8 bits; larger immediates need to be
/// materialised via additional instructions (e.g., chunked adds).
#[allow(dead_code)]
pub fn encode_addi(rd: u8, rs1: u8, imm: i8) -> u32 {
    encode_ri(instruction::wide::arithmetic::ADDI, rd, rs1, imm)
}

/// Encode `rd = rs1 + imm` and report when the immediate is not representable.
#[allow(dead_code)]
pub fn encode_addi_checked(rd: u8, rs1: u8, imm: i16) -> Result<u32, WideEncodingError> {
    if !(i8::MIN as i16..=i8::MAX as i16).contains(&imm) {
        return Err(WideEncodingError::ImmediateOutOfRange {
            value: imm,
            min: i8::MIN as i16,
            max: i8::MAX as i16,
        });
    }
    Ok(encode_addi(rd, rs1, imm as i8))
}

/// Encode a wide bitwise AND: `rd = rs1 & rs2`.
#[allow(dead_code)]
pub fn encode_and(rd: u8, rs1: u8, rs2: u8) -> u32 {
    encode_rr(instruction::wide::arithmetic::AND, rd, rs1, rs2)
}

/// Encode a wide bitwise OR: `rd = rs1 | rs2`.
#[allow(dead_code)]
pub fn encode_or(rd: u8, rs1: u8, rs2: u8) -> u32 {
    encode_rr(instruction::wide::arithmetic::OR, rd, rs1, rs2)
}

/// Encode a wide bitwise XOR: `rd = rs1 ^ rs2`.
#[allow(dead_code)]
pub fn encode_xor(rd: u8, rs1: u8, rs2: u8) -> u32 {
    encode_rr(instruction::wide::arithmetic::XOR, rd, rs1, rs2)
}

/// Encode a logical left shift.
#[allow(dead_code)]
pub fn encode_sll(rd: u8, rs1: u8, rs2: u8) -> u32 {
    encode_rr(instruction::wide::arithmetic::SLL, rd, rs1, rs2)
}

/// Encode a logical right shift.
#[allow(dead_code)]
pub fn encode_srl(rd: u8, rs1: u8, rs2: u8) -> u32 {
    encode_rr(instruction::wide::arithmetic::SRL, rd, rs1, rs2)
}

/// Encode an arithmetic right shift.
#[allow(dead_code)]
pub fn encode_sra(rd: u8, rs1: u8, rs2: u8) -> u32 {
    encode_rr(instruction::wide::arithmetic::SRA, rd, rs1, rs2)
}

/// Encode a wide 64-bit load: `rd <- [base + imm]`.
#[allow(dead_code)]
pub fn encode_load64(base: u8, rd: u8, imm: i8) -> u32 {
    wide::encode_load(instruction::wide::memory::LOAD64, rd, base, imm)
}

/// Encode a wide 128-bit load: `{rd_lo, rd_hi} <- [base]`.
///
/// The base register must already contain the final address. Adjustments should
/// be materialised via `encode_addi` before issuing the load.
#[allow(dead_code)]
pub fn encode_load128(base: u8, rd_lo: u8, rd_hi: u8) -> u32 {
    wide::encode_load128(instruction::wide::memory::LOAD128, rd_lo, base, rd_hi)
}

/// Encode a wide 64-bit store: `[base + imm] <- rs`.
#[allow(dead_code)]
pub fn encode_store64(base: u8, rs: u8, imm: i8) -> u32 {
    wide::encode_store(instruction::wide::memory::STORE64, base, rs, imm)
}

/// Encode a wide 128-bit store: `[base] <- {rs_lo, rs_hi}`.
///
/// The base register must already contain the final address. Adjustments should
/// be materialised via `encode_addi` before issuing the store.
#[allow(dead_code)]
pub fn encode_store128(base: u8, rs_lo: u8, rs_hi: u8) -> u32 {
    wide::encode_store128(instruction::wide::memory::STORE128, base, rs_lo, rs_hi)
}

/// Encode a wide unconditional jump (`JAL`), returning to `rd`.
#[allow(dead_code)]
pub fn encode_jal(rd: u8, imm_words: i16) -> u32 {
    wide::encode_jump(instruction::wide::control::JAL, rd, imm_words)
}

/// Encode a wide register jump (`JR`) with no link.
#[allow(dead_code)]
pub fn encode_jr(rs: u8) -> u32 {
    wide::encode_rr(instruction::wide::control::JR, 0, rs, 0)
}

/// Encode a compare-equal branch with range checking.
#[allow(dead_code)]
pub fn encode_beq_checked(rs1: u8, rs2: u8, offset_words: i16) -> Result<u32, WideEncodingError> {
    encode_branch_checked(instruction::wide::control::BEQ, rs1, rs2, offset_words)
}

/// Encode a compare-not-equal branch with range checking.
#[allow(dead_code)]
pub fn encode_bne_checked(rs1: u8, rs2: u8, offset_words: i16) -> Result<u32, WideEncodingError> {
    encode_branch_checked(instruction::wide::control::BNE, rs1, rs2, offset_words)
}

/// Encode a generic branch with the given opcode and offset.
#[allow(dead_code)]
pub fn encode_branch_checked(
    op: u8,
    rs1: u8,
    rs2: u8,
    offset_words: i16,
) -> Result<u32, WideEncodingError> {
    if !(i8::MIN as i16..=i8::MAX as i16).contains(&offset_words) {
        return Err(WideEncodingError::ImmediateOutOfRange {
            value: offset_words,
            min: i8::MIN as i16,
            max: i8::MAX as i16,
        });
    }
    Ok(wide::encode_branch(op, rs1, rs2, offset_words as i8))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding;

    #[test]
    fn wide_add_roundtrip() {
        let word = encode_add(5, 1, 2);
        let (op, rd, rs1, rs2) = encoding::wide::decode_rr(word);
        assert_eq!(op, instruction::wide::arithmetic::ADD);
        assert_eq!(rd, 5);
        assert_eq!(rs1, 1);
        assert_eq!(rs2, 2);
    }

    #[test]
    fn move_uses_addi_encoding() {
        let word = encode_move(5, 1);
        assert_eq!(
            crate::instruction::wide::opcode(word),
            crate::instruction::wide::arithmetic::ADDI
        );
    }

    #[test]
    fn addi_checked_accepts_small_immediates() {
        let word = encode_addi_checked(3, 1, -7).expect("encode");
        let (op, rd, rs1, imm) = encoding::wide::decode_ri(word);
        assert_eq!(op, instruction::wide::arithmetic::ADDI);
        assert_eq!(rd, 3);
        assert_eq!(rs1, 1);
        assert_eq!(imm, -7);
    }

    #[test]
    fn addi_checked_rejects_large_immediates() {
        let err = encode_addi_checked(3, 1, 256).expect_err("should reject");
        assert_eq!(
            err,
            WideEncodingError::ImmediateOutOfRange {
                value: 256,
                min: -128,
                max: 127
            }
        );
    }

    #[test]
    fn branch_checked_validates_range() {
        let word =
            encode_beq_checked(1, 2, -4).expect("encode branch within supported displacement");
        let mut mem = Memory::new(16);
        mem.load_code(&word.to_le_bytes());
        let (decoded, _) = decode(&mem, 0).expect("decode");
        assert_eq!(decoded, word);

        let err = encode_beq_checked(1, 2, 200).expect_err("offset too large");
        assert!(matches!(
            err,
            WideEncodingError::ImmediateOutOfRange { value: 200, .. }
        ));
    }

    #[test]
    fn load_store128_helpers_encode_expected_fields() {
        let load = encode_load128(7, 9, 10);
        assert_eq!(instruction::wide::rd(load), 9);
        assert_eq!(instruction::wide::rs1(load), 7);
        assert_eq!(instruction::wide::rs2(load), 10);

        let store = encode_store128(3, 4, 5);
        assert_eq!(instruction::wide::rd(store), 3);
        assert_eq!(instruction::wide::rs1(store), 4);
        assert_eq!(instruction::wide::rs2(store), 5);

        let mut mem = Memory::new(32);
        mem.load_code(&load.to_le_bytes());
        let (decoded, _) = decode(&mem, 0).expect("decode");
        assert_eq!(decoded, load);
    }
}
