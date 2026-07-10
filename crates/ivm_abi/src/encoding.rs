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
    use crate::instruction;

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

    /// Encode `POSEIDON2 rd, rs1, rs2` using two scalar-register inputs.
    #[inline]
    pub const fn encode_poseidon2(rd: u8, rs1: u8, rs2: u8) -> u32 {
        encode_rr(instruction::wide::crypto::POSEIDON2, rd, rs1, rs2)
    }

    /// Encode `POSEIDON6 rd, rs_base`, consuming `rs_base..=rs_base+5`.
    ///
    /// The final operand slot is reserved as zero so malformed encodings are
    /// rejected instead of acquiring an accidental second interpretation.
    #[inline]
    pub const fn encode_poseidon6(rd: u8, rs_base: u8) -> u32 {
        assert!(rs_base <= instruction::wide::crypto::POSEIDON6_MAX_INPUT_BASE);
        encode_rr(instruction::wide::crypto::POSEIDON6, rd, rs_base, 0)
    }

    /// Decode and validate the canonical `POSEIDON6` register-window form.
    #[inline]
    pub fn decode_poseidon6(word: u32) -> Option<(u8, u8)> {
        let (op, rd, rs_base, reserved) = decode_rr(word);
        if op == instruction::wide::crypto::POSEIDON6
            && reserved == 0
            && rs_base <= instruction::wide::crypto::POSEIDON6_MAX_INPUT_BASE
        {
            Some((rd, rs_base))
        } else {
            None
        }
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

    /// Encode a typed literal-table load with an unsigned 16-bit index.
    #[inline]
    pub const fn encode_literal(op: u8, rd: u8, index: u16) -> u32 {
        ((op as u32) << 24) | ((rd as u32) << 16) | index as u32
    }

    /// Decode a typed literal-table load.
    #[inline]
    pub const fn decode_literal(word: u32) -> (u8, u8, u16) {
        (
            (word >> 24) as u8,
            ((word >> 16) & 0xFF) as u8,
            (word & 0xFFFF) as u16,
        )
    }

    #[inline]
    pub const fn encode_branch(op: u8, rs1: u8, rs2: u8, offset_words: i8) -> u32 {
        ((op as u32) << 24)
            | ((rs1 as u32) << 16)
            | ((rs2 as u32) << 8)
            | (offset_words as u8 as u32)
    }

    /// Encode `JAL` with an explicit link register and signed 16-bit word offset.
    /// Use [`encode_offset24`] for `JMP` and `JALS`.
    #[inline]
    pub const fn encode_jump(op: u8, rd: u8, offset_words: i16) -> u32 {
        assert!(op == instruction::wide::control::JAL);
        ((op as u32) << 24) | ((rd as u32) << 16) | (offset_words as u16 as u32)
    }

    /// Decode the explicit link register and signed 16-bit word offset of `JAL`.
    #[inline]
    pub fn decode_jump(word: u32) -> (u8, u8, i16) {
        (
            (word >> 24) as u8,
            ((word >> 16) & 0xFF) as u8,
            (word & 0xFFFF) as u16 as i16,
        )
    }

    /// Encode a signed 24-bit word-relative transfer used by `JMP` and `JALS`.
    #[inline]
    pub const fn encode_offset24(op: u8, offset_words: i32) -> u32 {
        assert!(offset_words >= -0x80_0000 && offset_words <= 0x7f_ffff);
        ((op as u32) << 24) | ((offset_words as u32) & 0x00ff_ffff)
    }

    /// Decode a signed 24-bit word-relative transfer offset.
    #[inline]
    pub const fn decode_offset24(word: u32) -> (u8, i32) {
        ((word >> 24) as u8, ((word << 8) as i32) >> 8)
    }

    #[inline]
    pub const fn encode_sys(op: u8, imm8: u8) -> u32 {
        ((op as u32) << 24) | (imm8 as u32)
    }

    #[inline]
    pub fn decode_sys(word: u32) -> (u8, u8) {
        ((word >> 24) as u8, (word & 0xFF) as u8)
    }

    /// Encode a 24-bit extended syscall id in the `SYSTEM`/SCALLX slot.
    #[inline]
    pub const fn encode_syscallx(syscall: u32) -> u32 {
        assert!(syscall <= 0x00ff_ffff);
        ((crate::instruction::wide::system::SYSTEM as u32) << 24) | syscall
    }

    /// Decode the 24-bit extended syscall id carried by a `SYSTEM`/SCALLX word.
    #[inline]
    pub const fn decode_syscallx(word: u32) -> u32 {
        word & 0x00ff_ffff
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

    #[test]
    fn poseidon_register_encodings_are_canonical() {
        let word = wide::encode_poseidon2(9, 10, 11);
        assert_eq!(
            wide::decode_rr(word),
            (instruction::wide::crypto::POSEIDON2, 9, 10, 11)
        );

        let word = wide::encode_poseidon6(9, 250);
        assert_eq!(
            wide::decode_rr(word),
            (instruction::wide::crypto::POSEIDON6, 9, 250, 0)
        );
        assert_eq!(wide::decode_poseidon6(word), Some((9, 250)));
        assert_eq!(
            wide::decode_poseidon6(wide::encode_rr(
                instruction::wide::crypto::POSEIDON6,
                9,
                10,
                1,
            )),
            None
        );
    }

    #[test]
    #[should_panic]
    fn poseidon6_encoder_rejects_register_window_overflow() {
        let _ = wide::encode_poseidon6(9, 251);
    }

    #[test]
    fn literal_index_roundtrips_full_u16_range() {
        for index in [0, 1, 255, 256, u16::MAX] {
            let word = wide::encode_literal(instruction::wide::memory::LDLIT, 23, index);
            let (op, rd, decoded) = wide::decode_literal(word);
            assert_eq!(op, instruction::wide::memory::LDLIT);
            assert_eq!(rd, 23);
            assert_eq!(decoded, index);
            assert_eq!(instruction::wide::literal_index(word), usize::from(index));
        }
    }

    #[test]
    fn signed_offset24_roundtrips_boundaries() {
        for offset in [-0x80_0000, -1, 0, 1, 0x7f_ffff] {
            let word = wide::encode_offset24(instruction::wide::control::JMP, offset);
            let (op, decoded) = wide::decode_offset24(word);
            assert_eq!(op, instruction::wide::control::JMP);
            assert_eq!(decoded, offset);
            assert_eq!(instruction::wide::imm24(word), offset);
        }
    }

    #[test]
    #[should_panic]
    fn signed16_jump_encoder_rejects_long_transfer_opcode() {
        let _ = wide::encode_jump(instruction::wide::control::JMP, 0, -1);
    }

    #[test]
    fn syscallx_roundtrips_24_bit_number() {
        let number = 0x00ab_cdef;
        let word = wide::encode_syscallx(number);
        assert_eq!(
            instruction::wide::opcode(word),
            instruction::wide::system::SYSTEM
        );
        assert_eq!(wide::decode_syscallx(word), number);
    }
}
