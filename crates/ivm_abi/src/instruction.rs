//! Instruction opcode constants, operand formats, field extractors and canonical word validation
//! for the IVM instruction encoding.
//!
//! Every instruction is one 32-bit little-endian word. Bits 31..24 hold the primary opcode and the
//! remaining 24 bits are interpreted by the opcode's [`wide::OperandFormat`]:
//!
//! | Format | Bits 23..19 | Bits 18..14 | Bits 13..0 |
//! |---|---|---|---|
//! | `Register3` | `rd` | `rs1` | `rs2` in 13..9, 8..0 reserved |
//! | `Register2` (unary) | `rd` | `rs1` | reserved |
//! | `Immediate` (ALU, `LOAD64`, `JALR`, `CMOVI`) | `rd` | `rs1`/base | signed `imm14` |
//! | `RotateImmediate` | `rd` | `rs1` | amount in 5..0, 13..6 reserved |
//! | `Store` | value | base | signed byte displacement `disp14` |
//! | `Branch` | left operand | right operand | signed word offset `off14` |
//! | `Jal` | `rd` | signed word offset `off19` in 18..0 | |
//! | `Offset24` (`JMP`, `JALS`) | signed word offset `off24` in 23..0 | | |
//! | `Syscall` (`SCALL`) | syscall number in 23..0 | | |
//! | `Literal` (`LDLIT`, `LDI64`) | `rd` | 18..16 reserved, literal index in 15..0 | |
//! | `NoOperands` (`HALT`) | reserved | reserved | reserved |
//!
//! There are [`wide::REGISTER_COUNT`] (32) general-purpose registers and `r0` is hardwired to
//! zero, so every register field is five bits wide. Every bit that a format does not assign is
//! reserved and must be zero; admission rejects such a word, so each admitted instruction has exactly
//! one encoding. Every other opcode byte is unassigned and rejected as an invalid opcode.
/// Opcode constants, operand formats and field extractors of the canonical encoding.
pub mod wide {
    /// Number of architectural general-purpose registers. `r0` is hardwired to zero.
    pub const REGISTER_COUNT: usize = 32;
    /// Width of every register field.
    pub const REGISTER_FIELD_BITS: u32 = 5;
    /// Bit position of the first register field (`rd`, a store value or a branch's left operand).
    pub const RD_SHIFT: u32 = 19;
    /// Bit position of the second register field (`rs1`, a memory base or a branch's right operand).
    pub const RS1_SHIFT: u32 = 14;
    /// Bit position of the third register field (`rs2`).
    pub const RS2_SHIFT: u32 = 9;
    /// Width of the signed immediate, displacement and branch offset field.
    pub const IMM14_BITS: u32 = 14;
    /// Smallest signed 14-bit immediate.
    pub const IMM14_MIN: i16 = -(1 << 13);
    /// Largest signed 14-bit immediate.
    pub const IMM14_MAX: i16 = (1 << 13) - 1;
    /// Width of the signed `JAL` word offset.
    pub const JAL_OFFSET_BITS: u32 = 19;
    /// Smallest signed 19-bit `JAL` word offset.
    pub const JAL_OFFSET_MIN: i32 = -(1 << 18);
    /// Largest signed 19-bit `JAL` word offset.
    pub const JAL_OFFSET_MAX: i32 = (1 << 18) - 1;
    /// Smallest signed 24-bit `JMP`/`JALS` word offset.
    pub const OFFSET24_MIN: i32 = -(1 << 23);
    /// Largest signed 24-bit `JMP`/`JALS` word offset.
    pub const OFFSET24_MAX: i32 = (1 << 23) - 1;
    /// Largest syscall number carried by `SCALL`.
    pub const SYSCALL_NUMBER_MAX: u32 = 0x00ff_ffff;
    /// Largest rotate-immediate amount.
    pub const ROTATE_AMOUNT_MAX: u8 = 63;
    /// Arithmetic and logic operations.
    pub mod arithmetic {
        pub const ADD: u8 = 0x01;
        pub const SUB: u8 = 0x02;
        pub const AND: u8 = 0x03;
        pub const OR: u8 = 0x04;
        pub const XOR: u8 = 0x05;
        pub const SLL: u8 = 0x06;
        pub const SRL: u8 = 0x07;
        pub const SRA: u8 = 0x08;
        pub const SLT: u8 = 0x09;
        pub const SLTU: u8 = 0x0A;
        pub const CMOV: u8 = 0x0B;
        pub const NOT: u8 = 0x0C;
        pub const NEG: u8 = 0x0D;
        pub const SEQ: u8 = 0x0E;
        pub const SNE: u8 = 0x0F;
        pub const MUL: u8 = 0x10;
        pub const MULH: u8 = 0x11;
        pub const MULHU: u8 = 0x12;
        pub const MULHSU: u8 = 0x13;
        pub const DIV: u8 = 0x14;
        pub const DIVU: u8 = 0x15;
        pub const REM: u8 = 0x16;
        pub const REMU: u8 = 0x17;
        pub const ROTL: u8 = 0x18;
        pub const ROTR: u8 = 0x19;
        pub const POPCNT: u8 = 0x1A;
        pub const CLZ: u8 = 0x1B;
        pub const CTZ: u8 = 0x1C;
        pub const ISQRT: u8 = 0x1D;
        pub const MIN: u8 = 0x1E;
        pub const MAX: u8 = 0x1F;
        pub const ABS: u8 = 0x27;
        pub const DIV_CEIL: u8 = 0x28;
        pub const GCD: u8 = 0x29;
        pub const MEAN: u8 = 0x2A;
        pub const ADDI: u8 = 0x20;
        pub const ANDI: u8 = 0x21;
        pub const ORI: u8 = 0x22;
        pub const XORI: u8 = 0x23;
        pub const CMOVI: u8 = 0x24;
        pub const ROTL_IMM: u8 = 0x25;
        pub const ROTR_IMM: u8 = 0x26;
    }
    /// Memory access.
    pub mod memory {
        pub const LOAD64: u8 = 0x30;
        pub const STORE64: u8 = 0x31;
        /// Load a validated typed literal pointer by its `u16` literal-table index.
        pub const LDLIT: u8 = 0x34;
        /// Load a validated signed 64-bit scalar by its `u16` literal-table index.
        pub const LDI64: u8 = 0x35;
    }
    /// Control flow.
    pub mod control {
        pub const BEQ: u8 = 0x40;
        pub const BNE: u8 = 0x41;
        pub const BLT: u8 = 0x42;
        pub const BGE: u8 = 0x43;
        pub const BLTU: u8 = 0x44;
        pub const BGEU: u8 = 0x45;
        /// Jump by the signed 19-bit word offset in bits 18..0 and link into `rd`.
        pub const JAL: u8 = 0x46;
        pub const JALR: u8 = 0x48;
        pub const HALT: u8 = 0x49;
        /// Jump by the signed 24-bit word offset in bits 23..0.
        pub const JMP: u8 = 0x4A;
        /// Jump by a signed 24-bit word offset and write the link to register 1.
        pub const JALS: u8 = 0x4B;
    }
    /// System call.
    pub mod system {
        /// Invoke the host syscall whose 24-bit number occupies bits 23..0.
        pub const SCALL: u8 = 0x60;
    }
    /// Operand layout of an admitted opcode.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub enum OperandFormat {
        /// `rd[23:19] rs1[18:14] rs2[13:9]`, bits 8..0 reserved.
        Register3,
        /// `rd[23:19] rs1[18:14]`, bits 13..0 reserved.
        Register2,
        /// `rd[23:19] rs1[18:14] imm14[13:0]`, signed.
        Immediate,
        /// `rd[23:19] rs1[18:14] amount[5:0]`, bits 13..6 reserved.
        RotateImmediate,
        /// `value[23:19] base[18:14] disp14[13:0]`, signed byte displacement.
        Store,
        /// `lhs[23:19] rhs[18:14] off14[13:0]`, signed word offset.
        Branch,
        /// `rd[23:19] off19[18:0]`, signed word offset.
        Jal,
        /// `off24[23:0]`, signed word offset.
        Offset24,
        /// `number[23:0]`.
        Syscall,
        /// `rd[23:19] index[15:0]`, bits 18..16 reserved.
        Literal,
        /// Bits 23..0 reserved.
        NoOperands,
    }
    impl OperandFormat {
        /// Every format, in descriptor order.
        pub const ALL: [Self; 11] = [
            Self::Register3,
            Self::Register2,
            Self::Immediate,
            Self::RotateImmediate,
            Self::Store,
            Self::Branch,
            Self::Jal,
            Self::Offset24,
            Self::Syscall,
            Self::Literal,
            Self::NoOperands,
        ];
        /// Operand bits that must be zero.
        #[must_use]
        pub const fn reserved_mask(self) -> u32 {
            match self {
                Self::Register3 => 0x0000_01ff,
                Self::Register2 => 0x0000_3fff,
                Self::RotateImmediate => 0x0000_3fc0,
                Self::Literal => 0x0007_0000,
                Self::NoOperands => 0x00ff_ffff,
                Self::Immediate | Self::Store | Self::Branch | Self::Jal | Self::Offset24 => 0,
                Self::Syscall => 0,
            }
        }
        /// Canonical bit-layout description bound into the ABI descriptor.
        #[must_use]
        pub const fn layout(self) -> &'static str {
            match self {
                Self::Register3 => "rd[23:19]+rs1[18:14]+rs2[13:9]+zero[8:0]",
                Self::Register2 => "rd[23:19]+rs1[18:14]+zero[13:0]",
                Self::Immediate => "rd[23:19]+rs1[18:14]+simm14[13:0]",
                Self::RotateImmediate => "rd[23:19]+rs1[18:14]+zero[13:6]+amount6[5:0]",
                Self::Store => "value[23:19]+base[18:14]+sdisp14-bytes[13:0]",
                Self::Branch => "lhs[23:19]+rhs[18:14]+soff14-words[13:0]",
                Self::Jal => "rd[23:19]+soff19-words[18:0]",
                Self::Offset24 => "soff24-words[23:0]",
                Self::Syscall => "number24[23:0]",
                Self::Literal => "rd[23:19]+zero[18:16]+index16[15:0]",
                Self::NoOperands => "zero[23:0]",
            }
        }
    }
    /// Operand format of `op`, or `None` when the opcode byte is not admitted.
    #[must_use]
    pub const fn operand_format(op: u8) -> Option<OperandFormat> {
        Some(match op {
            arithmetic::ADD
            | arithmetic::SUB
            | arithmetic::AND
            | arithmetic::OR
            | arithmetic::XOR
            | arithmetic::SLL
            | arithmetic::SRL
            | arithmetic::SRA
            | arithmetic::SLT
            | arithmetic::SLTU
            | arithmetic::CMOV
            | arithmetic::SEQ
            | arithmetic::SNE
            | arithmetic::MUL
            | arithmetic::MULH
            | arithmetic::MULHU
            | arithmetic::MULHSU
            | arithmetic::DIV
            | arithmetic::DIVU
            | arithmetic::REM
            | arithmetic::REMU
            | arithmetic::ROTL
            | arithmetic::ROTR
            | arithmetic::MIN
            | arithmetic::MAX
            | arithmetic::DIV_CEIL
            | arithmetic::GCD
            | arithmetic::MEAN => OperandFormat::Register3,
            arithmetic::NOT
            | arithmetic::NEG
            | arithmetic::POPCNT
            | arithmetic::CLZ
            | arithmetic::CTZ
            | arithmetic::ISQRT
            | arithmetic::ABS => OperandFormat::Register2,
            arithmetic::ADDI
            | arithmetic::ANDI
            | arithmetic::ORI
            | arithmetic::XORI
            | arithmetic::CMOVI
            | memory::LOAD64
            | control::JALR => OperandFormat::Immediate,
            arithmetic::ROTL_IMM | arithmetic::ROTR_IMM => OperandFormat::RotateImmediate,
            memory::STORE64 => OperandFormat::Store,
            memory::LDLIT | memory::LDI64 => OperandFormat::Literal,
            control::BEQ
            | control::BNE
            | control::BLT
            | control::BGE
            | control::BLTU
            | control::BGEU => OperandFormat::Branch,
            control::JAL => OperandFormat::Jal,
            control::JMP | control::JALS => OperandFormat::Offset24,
            control::HALT => OperandFormat::NoOperands,
            system::SCALL => OperandFormat::Syscall,
            _ => return None,
        })
    }
    /// Returns true when `op` is one of the admitted opcode values.
    #[must_use]
    pub const fn is_valid_opcode(op: u8) -> bool {
        operand_format(op).is_some()
    }
    /// Reason a raw word is not a canonical admitted instruction.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum WordError {
        /// The primary opcode byte is not admitted.
        InvalidOpcode(u8),
        /// An operand bit reserved by the opcode's format is set.
        ReservedBits,
    }
    /// Validate one instruction word and return its operand format.
    ///
    /// # Errors
    /// Returns [`WordError::InvalidOpcode`] for an unassigned opcode byte and
    /// [`WordError::ReservedBits`] when a reserved operand bit is set.
    pub const fn validate_word(word: u32) -> Result<OperandFormat, WordError> {
        let op = opcode(word);
        let Some(format) = operand_format(op) else {
            return Err(WordError::InvalidOpcode(op));
        };
        if word & format.reserved_mask() != 0 {
            return Err(WordError::ReservedBits);
        }
        Ok(format)
    }
    /// Primary opcode byte (bits 31..24).
    #[inline]
    #[must_use]
    pub const fn opcode(word: u32) -> u8 {
        (word >> 24) as u8
    }
    /// First register field (bits 23..19): `rd`, a store's value or a branch's left operand.
    #[inline]
    #[must_use]
    pub const fn rd(word: u32) -> usize {
        ((word >> RD_SHIFT) & 0x1f) as usize
    }
    /// Second register field (bits 18..14): `rs1`, a memory base or a branch's right operand.
    #[inline]
    #[must_use]
    pub const fn rs1(word: u32) -> usize {
        ((word >> RS1_SHIFT) & 0x1f) as usize
    }
    /// Third register field (bits 13..9) of a three-register instruction.
    #[inline]
    #[must_use]
    pub const fn rs2(word: u32) -> usize {
        ((word >> RS2_SHIFT) & 0x1f) as usize
    }
    /// Register whose value `STORE64` writes (bits 23..19).
    #[inline]
    #[must_use]
    pub const fn store_value(word: u32) -> usize {
        rd(word)
    }
    /// Base register of `STORE64` (bits 18..14).
    #[inline]
    #[must_use]
    pub const fn store_base(word: u32) -> usize {
        rs1(word)
    }
    /// Left operand register of a conditional branch (bits 23..19).
    #[inline]
    #[must_use]
    pub const fn branch_lhs(word: u32) -> usize {
        rd(word)
    }
    /// Right operand register of a conditional branch (bits 18..14).
    #[inline]
    #[must_use]
    pub const fn branch_rhs(word: u32) -> usize {
        rs1(word)
    }
    /// Signed 14-bit immediate, displacement or branch word offset (bits 13..0).
    #[inline]
    #[must_use]
    pub const fn imm14(word: u32) -> i16 {
        (((word << 18) as i32) >> 18) as i16
    }
    /// Rotate-immediate amount (bits 5..0).
    #[inline]
    #[must_use]
    pub const fn rotate_amount(word: u32) -> u32 {
        word & 0x3f
    }
    /// Signed 19-bit `JAL` word offset (bits 18..0).
    #[inline]
    #[must_use]
    pub const fn jal_offset(word: u32) -> i32 {
        ((word << 13) as i32) >> 13
    }
    /// Signed 24-bit word offset carried by `JMP`/`JALS` (bits 23..0).
    #[inline]
    #[must_use]
    pub const fn imm24(word: u32) -> i32 {
        ((word << 8) as i32) >> 8
    }
    /// Syscall number carried by `SCALL` (bits 23..0).
    #[inline]
    #[must_use]
    pub const fn syscall_number(word: u32) -> u32 {
        word & SYSCALL_NUMBER_MAX
    }
    /// Unsigned literal-table index carried by `LDLIT`/`LDI64` (bits 15..0).
    #[inline]
    #[must_use]
    pub const fn literal_index(word: u32) -> usize {
        (word & 0xFFFF) as usize
    }
    /// Canonical ISA layout bound into the ABI descriptor.
    pub const WORD_LAYOUT: &str = "u32le-word;opcode[31:24];registers=32;register-field=5-bits;r0=hardwired-zero-writes-discarded;reserved-bits-must-be-zero;pc-advances-4-bytes;branch-JAL-JMP-JALS-offsets=words-relative-to-instruction;JALR-target=rs1+simm14-bytes-rounded-down-to-instruction-alignment;LOAD64-STORE64-address=base+simm14-bytes;one-word-per-instruction";
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn every_admitted_opcode_has_one_format_and_rejects_reserved_bits() {
            let mut admitted = 0;
            for op in 0..=u8::MAX {
                let Some(format) = operand_format(op) else {
                    assert_eq!(
                        validate_word(u32::from(op) << 24),
                        Err(WordError::InvalidOpcode(op))
                    );
                    continue;
                };
                admitted += 1;
                let base = u32::from(op) << 24;
                assert_eq!(validate_word(base), Ok(format));
                for bit in 0..24 {
                    let word = base | (1 << bit);
                    let reserved = format.reserved_mask() & (1 << bit) != 0;
                    assert_eq!(
                        validate_word(word).is_err(),
                        reserved,
                        "opcode {op:#04x} bit {bit}"
                    );
                }
            }
            assert_eq!(admitted, 58);
        }
        #[test]
        fn field_extractors_sign_extend_and_mask() {
            let word = 0x2000_0000 | (31 << RD_SHIFT) | (17 << RS1_SHIFT) | 0x2000;
            assert_eq!(rd(word), 31);
            assert_eq!(rs1(word), 17);
            assert_eq!(imm14(word), IMM14_MIN);
            assert_eq!(imm14(word | 0x1fff), -1);
            assert_eq!(jal_offset(0x0004_0000), JAL_OFFSET_MIN);
            assert_eq!(jal_offset(0x0003_ffff), JAL_OFFSET_MAX);
            assert_eq!(imm24(0x0080_0000), OFFSET24_MIN);
            assert_eq!(syscall_number(0x60ab_cdef), 0x00ab_cdef);
            assert_eq!(literal_index(0x3400_ffff), 0xffff);
        }
    }
}
