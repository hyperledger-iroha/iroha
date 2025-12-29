//! Helpers for encoding and decoding the canonical wide IVM instruction format.
//! Each instruction is a 32-bit little-endian word composed of an 8-bit primary
//! opcode followed by three 8-bit operand slots.  The helpers below expose a
//! consistent interface for assembling and disassembling this layout.

#[inline]
pub const fn encode_halt() -> u32 {
    wide::encode_halt()
}

/// Helpers for the wide 8-bit opcode layout (three 8-bit operand fields).
pub mod wide {
    #[inline]
    pub const fn encode_rr(op: u8, rd: u8, rs1: u8, rs2: u8) -> u32 {
        ((op as u32) << 24) | ((rd as u32) << 16) | ((rs1 as u32) << 8) | (rs2 as u32)
    }

    #[inline]
    pub fn decode_rr(word: u32) -> (u8, u8, u8, u8) {
        (
            (word >> 24) as u8,
            ((word >> 16) & 0xFF) as u8,
            ((word >> 8) & 0xFF) as u8,
            (word & 0xFF) as u8,
        )
    }

    #[inline]
    pub const fn encode_ri(op: u8, rd: u8, rs1: u8, imm: i8) -> u32 {
        ((op as u32) << 24) | ((rd as u32) << 16) | ((rs1 as u32) << 8) | (imm as u8 as u32)
    }

    #[inline]
    pub fn decode_ri(word: u32) -> (u8, u8, u8, i8) {
        (
            (word >> 24) as u8,
            ((word >> 16) & 0xFF) as u8,
            ((word >> 8) & 0xFF) as u8,
            word as u8 as i8,
        )
    }

    #[inline]
    pub const fn encode_load(op: u8, rd: u8, base: u8, imm: i8) -> u32 {
        ((op as u32) << 24) | ((rd as u32) << 16) | ((base as u32) << 8) | (imm as u8 as u32)
    }

    #[inline]
    pub const fn encode_store(op: u8, base: u8, rs: u8, imm: i8) -> u32 {
        ((op as u32) << 24) | ((base as u32) << 16) | ((rs as u32) << 8) | (imm as u8 as u32)
    }

    #[inline]
    pub const fn encode_load128(op: u8, rd_lo: u8, base: u8, rd_hi: u8) -> u32 {
        ((op as u32) << 24) | ((rd_lo as u32) << 16) | ((base as u32) << 8) | (rd_hi as u32)
    }

    #[inline]
    pub const fn encode_store128(op: u8, base: u8, rs_lo: u8, rs_hi: u8) -> u32 {
        ((op as u32) << 24) | ((base as u32) << 16) | ((rs_lo as u32) << 8) | (rs_hi as u32)
    }

    #[inline]
    pub fn decode_mem(word: u32) -> (u8, u8, u8, i8) {
        (
            (word >> 24) as u8,
            ((word >> 16) & 0xFF) as u8,
            ((word >> 8) & 0xFF) as u8,
            word as u8 as i8,
        )
    }

    #[inline]
    pub fn decode_load128(word: u32) -> (u8, u8, u8, u8) {
        (
            (word >> 24) as u8,
            ((word >> 16) & 0xFF) as u8,
            ((word >> 8) & 0xFF) as u8,
            (word & 0xFF) as u8,
        )
    }

    #[inline]
    pub fn decode_store128(word: u32) -> (u8, u8, u8, u8) {
        (
            (word >> 24) as u8,
            ((word >> 16) & 0xFF) as u8,
            ((word >> 8) & 0xFF) as u8,
            (word & 0xFF) as u8,
        )
    }

    #[inline]
    pub const fn encode_branch(op: u8, rs1: u8, rs2: u8, offset_words: i8) -> u32 {
        ((op as u32) << 24)
            | ((rs1 as u32) << 16)
            | ((rs2 as u32) << 8)
            | (offset_words as u8 as u32)
    }

    #[inline]
    pub const fn encode_jump(op: u8, rd: u8, offset_words: i16) -> u32 {
        ((op as u32) << 24) | ((rd as u32) << 16) | (offset_words as u16 as u32)
    }

    #[inline]
    pub fn decode_jump(word: u32) -> (u8, u8, i16) {
        (
            (word >> 24) as u8,
            ((word >> 16) & 0xFF) as u8,
            (word & 0xFFFF) as u16 as i16,
        )
    }

    #[inline]
    pub const fn encode_sys(op: u8, imm8: u8) -> u32 {
        ((op as u32) << 24) | (imm8 as u32)
    }

    #[inline]
    pub fn decode_sys(word: u32) -> (u8, u8) {
        ((word >> 24) as u8, (word & 0xFF) as u8)
    }

    #[inline]
    pub const fn encode_halt() -> u32 {
        (crate::instruction::wide::control::HALT as u32) << 24
    }
}

#[cfg(test)]
mod tests {
    use super::wide;
    use crate::instruction;

    #[test]
    fn wide_encode_load128_matches_field_order() {
        let word = wide::encode_load128(instruction::wide::memory::LOAD128, 9, 4, 10);
        assert_eq!(
            instruction::wide::opcode(word),
            instruction::wide::memory::LOAD128
        );
        assert_eq!(instruction::wide::rd(word), 9);
        assert_eq!(instruction::wide::rs1(word), 4);
        assert_eq!(instruction::wide::rs2(word), 10);

        let (op, rd_lo, base, rd_hi) = wide::decode_load128(word);
        assert_eq!(op, instruction::wide::memory::LOAD128);
        assert_eq!(rd_lo, 9);
        assert_eq!(base, 4);
        assert_eq!(rd_hi, 10);
    }

    #[test]
    fn wide_encode_store128_matches_field_order() {
        let word = wide::encode_store128(instruction::wide::memory::STORE128, 3, 5, 6);
        assert_eq!(
            instruction::wide::opcode(word),
            instruction::wide::memory::STORE128
        );
        assert_eq!(instruction::wide::rd(word), 3);
        assert_eq!(instruction::wide::rs1(word), 5);
        assert_eq!(instruction::wide::rs2(word), 6);

        let (op, base, rs_lo, rs_hi) = wide::decode_store128(word);
        assert_eq!(op, instruction::wide::memory::STORE128);
        assert_eq!(base, 3);
        assert_eq!(rs_lo, 5);
        assert_eq!(rs_hi, 6);
    }
}
