//! Helpers for encoding and decoding the canonical IVM instruction format.
//!
//! Each instruction is a 32-bit little-endian word: an 8-bit primary opcode in bits 31..24 followed
//! by 24 operand bits laid out by the opcode's [`crate::instruction::wide::OperandFormat`]. The
//! encoders reject register numbers outside the 32-register file and immediates outside their
//! field instead of truncating them, so every word they return is canonical.
#[inline]
pub const fn encode_halt() -> u32 {
    wide::encode_halt()
}
/// Encoders and decoders for each operand format.
pub mod wide {
    use crate::instruction::wide as isa;
    #[inline]
    const fn register(index: u8) -> u32 {
        assert!(
            (index as usize) < isa::REGISTER_COUNT,
            "IVM register index out of range"
        );
        index as u32
    }
    #[inline]
    const fn imm14_bits(value: i16) -> u32 {
        assert!(
            value >= isa::IMM14_MIN && value <= isa::IMM14_MAX,
            "IVM 14-bit immediate out of range"
        );
        (value as u16 as u32) & 0x3fff
    }
    #[inline]
    const fn fields(op: u8, first: u8, second: u8) -> u32 {
        ((op as u32) << 24)
            | (register(first) << isa::RD_SHIFT)
            | (register(second) << isa::RS1_SHIFT)
    }
    /// Encode a three-register instruction. Unary operations pass `rs2 = 0`.
    #[inline]
    pub const fn encode_rr(op: u8, rd: u8, rs1: u8, rs2: u8) -> u32 {
        fields(op, rd, rs1) | (register(rs2) << isa::RS2_SHIFT)
    }
    /// Decode the opcode and three register fields.
    #[inline]
    pub const fn decode_rr(word: u32) -> (u8, u8, u8, u8) {
        (
            isa::opcode(word),
            isa::rd(word) as u8,
            isa::rs1(word) as u8,
            isa::rs2(word) as u8,
        )
    }
    /// Encode `rd rs1 imm14`; rotate-immediate amounts must lie in `0..=63`.
    #[inline]
    pub const fn encode_ri(op: u8, rd: u8, rs1: u8, imm: i16) -> u32 {
        fields(op, rd, rs1) | imm14_bits(imm)
    }
    /// Decode `rd rs1 imm14`.
    #[inline]
    pub const fn decode_ri(word: u32) -> (u8, u8, u8, i16) {
        (
            isa::opcode(word),
            isa::rd(word) as u8,
            isa::rs1(word) as u8,
            isa::imm14(word),
        )
    }
    /// Encode `LOAD64 rd, [base + imm]`.
    #[inline]
    pub const fn encode_load(op: u8, rd: u8, base: u8, imm: i16) -> u32 {
        fields(op, rd, base) | imm14_bits(imm)
    }
    /// Decode `(op, rd, base, imm)` of a load.
    #[inline]
    pub const fn decode_load(word: u32) -> (u8, u8, u8, i16) {
        decode_ri(word)
    }
    /// Encode `STORE64 [base + imm], rs`: the value register occupies bits 23..19 and the base
    /// register bits 18..14.
    #[inline]
    pub const fn encode_store(op: u8, base: u8, rs: u8, imm: i16) -> u32 {
        fields(op, rs, base) | imm14_bits(imm)
    }
    /// Decode `(op, base, value, imm)` of a store.
    #[inline]
    pub const fn decode_store(word: u32) -> (u8, u8, u8, i16) {
        (
            isa::opcode(word),
            isa::store_base(word) as u8,
            isa::store_value(word) as u8,
            isa::imm14(word),
        )
    }
    /// Encode an indexed literal-table load with an unsigned 16-bit index.
    #[inline]
    pub const fn encode_literal(op: u8, rd: u8, index: u16) -> u32 {
        assert!(op == isa::memory::LDLIT || op == isa::memory::LDI64);
        ((op as u32) << 24) | (register(rd) << isa::RD_SHIFT) | index as u32
    }
    /// Decode an indexed literal-table load.
    #[inline]
    pub const fn decode_literal(word: u32) -> (u8, u8, u16) {
        (
            isa::opcode(word),
            isa::rd(word) as u8,
            isa::literal_index(word) as u16,
        )
    }
    /// Encode a conditional branch comparing `rs1` (bits 23..19) with `rs2` (bits 18..14) and
    /// jumping by a signed 14-bit word offset.
    #[inline]
    pub const fn encode_branch(op: u8, rs1: u8, rs2: u8, offset_words: i16) -> u32 {
        fields(op, rs1, rs2) | imm14_bits(offset_words)
    }
    /// Decode `(op, lhs, rhs, offset_words)` of a conditional branch.
    #[inline]
    pub const fn decode_branch(word: u32) -> (u8, u8, u8, i16) {
        (
            isa::opcode(word),
            isa::branch_lhs(word) as u8,
            isa::branch_rhs(word) as u8,
            isa::imm14(word),
        )
    }
    /// Encode `JAL` with an explicit link register and signed 19-bit word offset.
    /// Use [`encode_offset24`] for `JMP` and `JALS`.
    #[inline]
    pub const fn encode_jump(op: u8, rd: u8, offset_words: i32) -> u32 {
        assert!(op == isa::control::JAL);
        assert!(
            offset_words >= isa::JAL_OFFSET_MIN && offset_words <= isa::JAL_OFFSET_MAX,
            "IVM JAL offset out of range"
        );
        ((op as u32) << 24)
            | (register(rd) << isa::RD_SHIFT)
            | ((offset_words as u32) & 0x0007_ffff)
    }
    /// Decode the explicit link register and signed 19-bit word offset of `JAL`.
    #[inline]
    pub const fn decode_jump(word: u32) -> (u8, u8, i32) {
        (
            isa::opcode(word),
            isa::rd(word) as u8,
            isa::jal_offset(word),
        )
    }
    /// Encode a signed 24-bit word-relative transfer used by `JMP` and `JALS`.
    #[inline]
    pub const fn encode_offset24(op: u8, offset_words: i32) -> u32 {
        assert!(op == isa::control::JMP || op == isa::control::JALS);
        assert!(
            offset_words >= isa::OFFSET24_MIN && offset_words <= isa::OFFSET24_MAX,
            "IVM 24-bit offset out of range"
        );
        ((op as u32) << 24) | ((offset_words as u32) & 0x00ff_ffff)
    }
    /// Decode a signed 24-bit word-relative transfer offset.
    #[inline]
    pub const fn decode_offset24(word: u32) -> (u8, i32) {
        (isa::opcode(word), isa::imm24(word))
    }
    /// Encode `SCALL` with its 24-bit syscall number.
    #[inline]
    pub const fn encode_scall(syscall: u32) -> u32 {
        assert!(
            syscall <= isa::SYSCALL_NUMBER_MAX,
            "IVM syscall number out of range"
        );
        ((isa::system::SCALL as u32) << 24) | syscall
    }
    /// Decode the 24-bit syscall number carried by `SCALL`.
    #[inline]
    pub const fn decode_scall(word: u32) -> u32 {
        isa::syscall_number(word)
    }
    #[inline]
    pub const fn encode_halt() -> u32 {
        (isa::control::HALT as u32) << 24
    }
}
#[cfg(test)]
mod tests {
    use super::wide;
    use crate::instruction::wide as isa;
    #[test]
    fn literal_index_roundtrips_full_u16_range() {
        for opcode in [isa::memory::LDLIT, isa::memory::LDI64] {
            for index in [0, 1, 255, 256, u16::MAX] {
                let word = wide::encode_literal(opcode, 23, index);
                let (op, rd, decoded) = wide::decode_literal(word);
                assert_eq!(op, opcode);
                assert_eq!(rd, 23);
                assert_eq!(decoded, index);
                assert_eq!(isa::literal_index(word), usize::from(index));
                assert!(isa::validate_word(word).is_ok());
            }
        }
    }
    #[test]
    #[should_panic]
    fn literal_encoder_rejects_non_literal_opcode() {
        let _ = wide::encode_literal(isa::memory::LOAD64, 23, 0);
    }
    #[test]
    fn signed_offset24_roundtrips_boundaries() {
        for offset in [isa::OFFSET24_MIN, -1, 0, 1, isa::OFFSET24_MAX] {
            let word = wide::encode_offset24(isa::control::JMP, offset);
            let (op, decoded) = wide::decode_offset24(word);
            assert_eq!(op, isa::control::JMP);
            assert_eq!(decoded, offset);
            assert_eq!(isa::imm24(word), offset);
        }
    }
    #[test]
    #[should_panic]
    fn jal_encoder_rejects_long_transfer_opcode() {
        let _ = wide::encode_jump(isa::control::JMP, 0, -1);
    }
    #[test]
    fn jal_offset19_roundtrips_boundaries() {
        for offset in [isa::JAL_OFFSET_MIN, -1, 0, 1, isa::JAL_OFFSET_MAX] {
            let word = wide::encode_jump(isa::control::JAL, 31, offset);
            assert_eq!(wide::decode_jump(word), (isa::control::JAL, 31, offset));
            assert!(isa::validate_word(word).is_ok());
        }
    }
    #[test]
    #[should_panic]
    fn jal_encoder_rejects_offset_beyond_19_bits() {
        let _ = wide::encode_jump(isa::control::JAL, 1, isa::JAL_OFFSET_MAX + 1);
    }
    #[test]
    fn scall_roundtrips_24_bit_number() {
        let number = 0x00ab_cdef;
        let word = wide::encode_scall(number);
        assert_eq!(isa::opcode(word), isa::system::SCALL);
        assert_eq!(wide::decode_scall(word), number);
    }
    #[test]
    fn immediate_store_and_branch_fields_roundtrip_boundaries() {
        for imm in [isa::IMM14_MIN, -1, 0, 1, isa::IMM14_MAX] {
            let ri = wide::encode_ri(isa::arithmetic::ADDI, 31, 30, imm);
            assert_eq!(wide::decode_ri(ri), (isa::arithmetic::ADDI, 31, 30, imm));
            let store = wide::encode_store(isa::memory::STORE64, 29, 7, imm);
            assert_eq!(isa::store_base(store), 29);
            assert_eq!(isa::store_value(store), 7);
            assert_eq!(
                wide::decode_store(store),
                (isa::memory::STORE64, 29, 7, imm)
            );
            let branch = wide::encode_branch(isa::control::BLT, 3, 4, imm);
            assert_eq!(wide::decode_branch(branch), (isa::control::BLT, 3, 4, imm));
            for word in [ri, store, branch] {
                assert!(isa::validate_word(word).is_ok());
            }
        }
        let rr = wide::encode_rr(isa::arithmetic::ADD, 31, 1, 2);
        assert_eq!(wide::decode_rr(rr), (isa::arithmetic::ADD, 31, 1, 2));
    }
    #[test]
    #[should_panic]
    fn encoders_reject_registers_outside_the_register_file() {
        let _ = wide::encode_rr(isa::arithmetic::ADD, 32, 1, 2);
    }
    #[test]
    #[should_panic]
    fn encoders_reject_immediates_outside_14_bits() {
        let _ = wide::encode_ri(isa::arithmetic::ADDI, 1, 1, isa::IMM14_MAX + 1);
    }
}
