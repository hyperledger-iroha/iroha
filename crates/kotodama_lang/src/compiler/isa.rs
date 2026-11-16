//! Instruction encoding of one code-generation target.
//!
//! Code generation plans the same instruction sequence for every engine. What differs is the word
//! layout and the immediate, branch and jump ranges that decide when a value is materialised by
//! chunked additions or the literal table and when a transfer is relaxed:
//!
//! - [`Isa::C10`] is the consolidated encoding (`ivm_abi::{encoding, instruction}`): 32 registers,
//!   signed 14-bit immediates, displacements and branch word offsets, a signed 19-bit `JAL` and a
//!   single 24-bit `SCALL`.
//! - [`Isa::Baseline`] is the frozen encoding of the installed baseline selection
//!   (`ivm_abi::baseline::{encoding, instruction}`), the anchor `77600a4840`: three 8-bit operand
//!   slots, signed 8-bit immediates, displacements and branch word offsets, a signed 16-bit
//!   `JAL`, an 8-bit `SCALL` for syscall numbers up to `0xFF` and the 24-bit `SYSTEM` form above.
//!
//! Opcode numbers are shared: every opcode the compiler emits has the same value in both tables.
//! The checked encoders keep the error texts of the compiler that defined each layout.

use ivm_abi::{
    IvmEngine,
    baseline::{encoding::wide as baseline_encoding, instruction::wide as baseline_isa},
    encoding::wide as c10_encoding,
    instruction::wide as c10_isa,
};

/// Instruction encoding emitted by one compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Isa {
    /// The consolidated C10 encoding.
    C10,
    /// The frozen baseline (anchor) encoding.
    Baseline,
}

impl Isa {
    /// The encoding executed by `engine`.
    pub(crate) const fn of(engine: IvmEngine) -> Self {
        match engine {
            IvmEngine::C10 => Self::C10,
            IvmEngine::Baseline => Self::Baseline,
        }
    }

    /// Smallest signed immediate, displacement and conditional-branch word offset.
    pub(crate) const fn imm_min(self) -> i16 {
        match self {
            Self::C10 => c10_isa::IMM14_MIN,
            Self::Baseline => i8::MIN as i16,
        }
    }

    /// Largest signed immediate, displacement and conditional-branch word offset.
    pub(crate) const fn imm_max(self) -> i16 {
        match self {
            Self::C10 => c10_isa::IMM14_MAX,
            Self::Baseline => i8::MAX as i16,
        }
    }

    /// Narrow `value` to the immediate field, if it fits.
    pub(crate) fn fit_imm(self, value: i64) -> Option<i16> {
        i16::try_from(value)
            .ok()
            .filter(|value| (self.imm_min()..=self.imm_max()).contains(value))
    }

    /// Clamp `value` to the immediate field (one step of a chunked addition).
    pub(crate) fn chunk_immediate(self, value: i64) -> i16 {
        self.fit_imm(value.clamp(i64::from(self.imm_min()), i64::from(self.imm_max())))
            .expect("a clamped immediate fits the immediate field")
    }

    /// Largest `JAL` word offset.
    pub(crate) const fn jal_offset_max(self) -> i32 {
        match self {
            Self::C10 => c10_isa::JAL_OFFSET_MAX,
            Self::Baseline => i16::MAX as i32,
        }
    }

    /// Smallest `JAL` word offset.
    const fn jal_offset_min(self) -> i32 {
        match self {
            Self::C10 => c10_isa::JAL_OFFSET_MIN,
            Self::Baseline => i16::MIN as i32,
        }
    }

    fn narrow(self, imm: i16) -> i8 {
        debug_assert_eq!(self, Self::Baseline);
        i8::try_from(imm).expect("the caller checked the baseline 8-bit immediate field")
    }

    /// Encode a three-register instruction.
    pub(crate) fn encode_rr(self, op: u8, rd: u8, rs1: u8, rs2: u8) -> u32 {
        match self {
            Self::C10 => c10_encoding::encode_rr(op, rd, rs1, rs2),
            Self::Baseline => baseline_encoding::encode_rr(op, rd, rs1, rs2),
        }
    }

    /// Encode a register-immediate instruction; the immediate must fit the field.
    pub(crate) fn encode_ri(self, op: u8, rd: u8, rs1: u8, imm: i16) -> u32 {
        match self {
            Self::C10 => c10_encoding::encode_ri(op, rd, rs1, imm),
            Self::Baseline => baseline_encoding::encode_ri(op, rd, rs1, self.narrow(imm)),
        }
    }

    /// Encode an indexed literal-table load.
    pub(crate) fn encode_literal(self, op: u8, rd: u8, index: u16) -> u32 {
        match self {
            Self::C10 => c10_encoding::encode_literal(op, rd, index),
            Self::Baseline => baseline_encoding::encode_literal(op, rd, index),
        }
    }

    /// Encode a signed 24-bit word-relative `JMP` or `JALS`.
    pub(crate) fn encode_offset24(self, op: u8, offset_words: i32) -> u32 {
        match self {
            Self::C10 => c10_encoding::encode_offset24(op, offset_words),
            Self::Baseline => baseline_encoding::encode_offset24(op, offset_words),
        }
    }

    /// Encode a system call: the C10 `SCALL`, or the baseline 8-bit `SCALL` up to `0xFF` and
    /// the 24-bit `SYSTEM` form above.
    pub(crate) fn encode_scall(self, number: u32) -> u32 {
        match self {
            Self::C10 => c10_encoding::encode_scall(number),
            Self::Baseline => match u8::try_from(number) {
                Ok(imm8) => baseline_encoding::encode_sys(baseline_isa::system::SCALL, imm8),
                Err(_) => baseline_encoding::encode_syscallx(number),
            },
        }
    }

    /// Encode `HALT`.
    pub(crate) fn encode_halt(self) -> u32 {
        match self {
            Self::C10 => c10_encoding::encode_halt(),
            Self::Baseline => baseline_encoding::encode_halt(),
        }
    }

    /// Encode the function return `JALR x0, x1, 0`.
    pub(crate) fn encode_return(self) -> u32 {
        match self {
            Self::C10 => c10_encoding::encode_ri(c10_isa::control::JALR, 0, 1, 0),
            Self::Baseline => baseline_encoding::encode_rr(baseline_isa::control::JALR, 0, 1, 0),
        }
    }

    /// Encode `rd = rs1 + imm`, refusing an immediate outside the field.
    pub(crate) fn encode_addi(self, rd: u8, rs1: u8, imm: i16) -> Result<u32, String> {
        if self.fit_imm(i64::from(imm)).is_none() {
            return Err(match self {
                Self::C10 => format!(
                    "encode_addi immediate {imm} exceeds the signed 14-bit field; use emit_addi for chunked emission"
                ),
                Self::Baseline => format!(
                    "encode_addi immediate {imm} out of range; use emit_addi for chunked emission"
                ),
            });
        }
        Ok(self.encode_ri(c10_isa::arithmetic::ADDI, rd, rs1, imm))
    }

    /// Encode a 64-bit load `rd <- [rs1 + imm]`, refusing a displacement outside the field.
    pub(crate) fn encode_load64(self, rd: u8, rs1: u8, imm: i16) -> Result<u32, String> {
        if self.fit_imm(i64::from(imm)).is_none() {
            return Err(match self {
                Self::C10 => format!(
                    "encode_load64_rv displacement {imm} exceeds the signed 14-bit field; use emit_load64"
                ),
                Self::Baseline => {
                    format!("encode_load64_rv offset {imm} out of wide range; use emit_load64")
                }
            });
        }
        Ok(match self {
            Self::C10 => c10_encoding::encode_load(c10_isa::memory::LOAD64, rd, rs1, imm),
            Self::Baseline => baseline_encoding::encode_load(
                baseline_isa::memory::LOAD64,
                rd,
                rs1,
                self.narrow(imm),
            ),
        })
    }

    /// Encode a 64-bit store `[rs1 + imm] <- rs2`, refusing a displacement outside the field.
    pub(crate) fn encode_store64(self, rs1: u8, rs2: u8, imm: i16) -> Result<u32, String> {
        if self.fit_imm(i64::from(imm)).is_none() {
            return Err(match self {
                Self::C10 => format!(
                    "encode_store64_rv displacement {imm} exceeds the signed 14-bit field; use emit_store64"
                ),
                Self::Baseline => {
                    format!("encode_store64_rv offset {imm} out of wide range; use emit_store64")
                }
            });
        }
        Ok(match self {
            Self::C10 => c10_encoding::encode_store(c10_isa::memory::STORE64, rs1, rs2, imm),
            Self::Baseline => baseline_encoding::encode_store(
                baseline_isa::memory::STORE64,
                rs1,
                rs2,
                self.narrow(imm),
            ),
        })
    }

    /// Encode a conditional branch whose `funct3` selects the condition and whose `imm` is a
    /// word-aligned byte offset.
    pub(crate) fn encode_branch_rv(
        self,
        funct3: u8,
        rs1: u8,
        rs2: u8,
        imm: i16,
    ) -> Result<u32, String> {
        if (imm & 0x3) != 0 {
            return Err(format!(
                "encode_branch_rv requires word-aligned offset, got {imm}"
            ));
        }
        let offset_words = imm / 4;
        if self == Self::Baseline && self.fit_imm(i64::from(offset_words)).is_none() {
            return Err(format!(
                "encode_branch_rv offset {imm} out of wide range; use emit_branch"
            ));
        }
        let op = match funct3 {
            0x0 => c10_isa::control::BEQ,
            0x1 => c10_isa::control::BNE,
            0x4 => c10_isa::control::BLT,
            0x5 => c10_isa::control::BGE,
            0x6 => c10_isa::control::BLTU,
            0x7 => c10_isa::control::BGEU,
            other => {
                return Err(format!("unsupported branch funct3 {other}"));
            }
        };
        Ok(self.encode_branch(op, rs1, rs2, offset_words))
    }

    /// Encode a conditional branch with a word offset that fits the field.
    pub(crate) fn encode_branch(self, op: u8, rs1: u8, rs2: u8, offset_words: i16) -> u32 {
        match self {
            Self::C10 => c10_encoding::encode_branch(op, rs1, rs2, offset_words),
            Self::Baseline => {
                baseline_encoding::encode_branch(op, rs1, rs2, self.narrow(offset_words))
            }
        }
    }

    /// Encode `JAL` with link register `rd` and a word-aligned byte offset `imm`.
    pub(crate) fn encode_jal(self, rd: u8, imm: i32) -> Result<u32, String> {
        if (imm % 4) != 0 {
            return Err(format!(
                "encode_jal requires word-aligned offset, got {imm}"
            ));
        }
        let offset_words = imm / 4;
        if !(self.jal_offset_min()..=self.jal_offset_max()).contains(&offset_words) {
            return Err(match self {
                Self::C10 => {
                    format!("encode_jal offset {imm} exceeds the signed 19-bit word range")
                }
                Self::Baseline => format!("encode_jal offset {imm} exceeds 16-bit word range"),
            });
        }
        Ok(match self {
            Self::C10 => c10_encoding::encode_jump(c10_isa::control::JAL, rd, offset_words),
            Self::Baseline => baseline_encoding::encode_jump(
                baseline_isa::control::JAL,
                rd,
                i16::try_from(offset_words).expect("checked against the 16-bit JAL range"),
            ),
        })
    }

    /// Destination register of a register, register-immediate, load or literal word.
    pub(crate) fn rd(self, word: u32) -> usize {
        match self {
            Self::C10 => c10_isa::rd(word),
            Self::Baseline => baseline_isa::rd(word),
        }
    }

    /// Word offset of a conditional branch.
    pub(crate) fn branch_offset(self, word: u32) -> i16 {
        match self {
            Self::C10 => c10_isa::imm14(word),
            Self::Baseline => i16::from(baseline_isa::imm8(word)),
        }
    }

    /// Compared registers of a conditional branch.
    pub(crate) fn branch_registers(self, word: u32) -> (u8, u8) {
        let (lhs, rhs) = match self {
            Self::C10 => (c10_isa::branch_lhs(word), c10_isa::branch_rhs(word)),
            Self::Baseline => (baseline_isa::rd(word), baseline_isa::rs1(word)),
        };
        (
            u8::try_from(lhs).expect("decoded branch register fits u8"),
            u8::try_from(rhs).expect("decoded branch register fits u8"),
        )
    }

    /// Word offset of `JAL`.
    pub(crate) fn jal_offset(self, word: u32) -> i32 {
        match self {
            Self::C10 => c10_isa::jal_offset(word),
            Self::Baseline => i32::from(baseline_isa::imm16(word)),
        }
    }

    /// Word offset of `JMP` or `JALS`.
    pub(crate) fn imm24(self, word: u32) -> i32 {
        match self {
            Self::C10 => c10_isa::imm24(word),
            Self::Baseline => baseline_isa::imm24(word),
        }
    }

    /// Syscall number of a system-call word, or `None` for any other word.
    pub(crate) fn syscall_number(self, word: u32) -> Option<u32> {
        let opcode = c10_isa::opcode(word);
        match self {
            Self::C10 => (opcode == c10_isa::system::SCALL).then(|| c10_isa::syscall_number(word)),
            Self::Baseline => match opcode {
                baseline_isa::system::SCALL => Some(word & 0xFF),
                baseline_isa::system::SYSTEM => Some(baseline_encoding::decode_syscallx(word)),
                _ => None,
            },
        }
    }
}

/// Whether a baseline code image uses an operation of the vector extension, which sets the
/// header's `VECTOR` mode bit (the anchor's opcode scan, kept verbatim).
pub(crate) fn baseline_uses_vector(code: &[u8]) -> bool {
    const VECTOR_OPS: [u8; 14] = [
        baseline_isa::crypto::VADD32,
        baseline_isa::crypto::VADD64,
        baseline_isa::crypto::VAND,
        baseline_isa::crypto::VXOR,
        baseline_isa::crypto::VOR,
        baseline_isa::crypto::VROT32,
        baseline_isa::crypto::SHA256BLOCK,
        baseline_isa::crypto::AESENC,
        baseline_isa::crypto::AESDEC,
        baseline_isa::crypto::SETVL,
        baseline_isa::crypto::PARBEGIN,
        baseline_isa::crypto::PAREND,
        baseline_isa::memory::LOAD128,
        baseline_isa::memory::STORE128,
    ];
    code.as_chunks::<4>().0.iter().any(|chunk| {
        let word = u32::from_le_bytes(*chunk);
        VECTOR_OPS.contains(&baseline_isa::opcode(word))
    })
}

/// Whether a baseline code image uses a ZK-helper operation, which sets the header's `ZK` mode
/// bit (the anchor's opcode scan, kept verbatim).
pub(crate) fn baseline_uses_zk(code: &[u8]) -> bool {
    const ZK_OPS: [u8; 7] = [
        baseline_isa::zk::ASSERT,
        baseline_isa::zk::ASSERT_EQ,
        baseline_isa::zk::FADD,
        baseline_isa::zk::FSUB,
        baseline_isa::zk::FMUL,
        baseline_isa::zk::FINV,
        baseline_isa::zk::ASSERT_RANGE,
    ];
    code.as_chunks::<4>().0.iter().any(|chunk| {
        let word = u32::from_le_bytes(*chunk);
        ZK_OPS.contains(&baseline_isa::opcode(word))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_follow_each_layout() {
        assert_eq!((Isa::C10.imm_min(), Isa::C10.imm_max()), (-8192, 8191));
        assert_eq!(
            (Isa::Baseline.imm_min(), Isa::Baseline.imm_max()),
            (-128, 127)
        );
        assert_eq!(Isa::Baseline.fit_imm(127), Some(127));
        assert_eq!(Isa::Baseline.fit_imm(128), None);
        assert_eq!(Isa::C10.fit_imm(128), Some(128));
        assert_eq!(Isa::Baseline.chunk_immediate(1_000), 127);
        assert_eq!(Isa::Baseline.chunk_immediate(-1_000), -128);
        assert_eq!(Isa::C10.chunk_immediate(1_000), 1_000);
        assert_eq!(Isa::Baseline.jal_offset_max(), 32_767);
        assert_eq!(Isa::C10.jal_offset_max(), c10_isa::JAL_OFFSET_MAX);
        assert_eq!(Isa::of(IvmEngine::Baseline), Isa::Baseline);
        assert_eq!(Isa::of(IvmEngine::C10), Isa::C10);
    }

    #[test]
    fn baseline_words_are_the_anchor_words() {
        let isa = Isa::Baseline;
        // The anchor's documented `addi x1, x1, 7`.
        assert_eq!(isa.encode_addi(1, 1, 7).unwrap(), 0x2001_0107);
        assert!(isa.encode_addi(1, 1, 128).is_err());
        assert_eq!(
            isa.encode_load64(3, 2, -8).unwrap(),
            baseline_encoding::encode_load(baseline_isa::memory::LOAD64, 3, 2, -8)
        );
        assert_eq!(
            isa.encode_store64(2, 3, 16).unwrap(),
            baseline_encoding::encode_store(baseline_isa::memory::STORE64, 2, 3, 16)
        );
        assert_eq!(
            isa.encode_branch_rv(0x1, 4, 5, -12).unwrap(),
            baseline_encoding::encode_branch(baseline_isa::control::BNE, 4, 5, -3)
        );
        assert!(isa.encode_branch_rv(0x0, 1, 2, 4 * 128).is_err());
        assert_eq!(
            isa.encode_jal(1, 4 * 32_767).unwrap(),
            baseline_encoding::encode_jump(baseline_isa::control::JAL, 1, 32_767)
        );
        assert!(isa.encode_jal(0, 4 * 32_768).is_err());
        assert_eq!(
            isa.encode_scall(0x20),
            baseline_encoding::encode_sys(baseline_isa::system::SCALL, 0x20)
        );
        assert_eq!(
            isa.encode_scall(0x1_0000),
            baseline_encoding::encode_syscallx(0x1_0000)
        );
        assert_eq!(isa.syscall_number(isa.encode_scall(0x20)), Some(0x20));
        assert_eq!(
            isa.syscall_number(isa.encode_scall(0x1_0000)),
            Some(0x1_0000)
        );
        assert_eq!(isa.syscall_number(isa.encode_halt()), None);
        assert_eq!(
            isa.encode_return(),
            baseline_encoding::encode_rr(baseline_isa::control::JALR, 0, 1, 0)
        );
    }

    #[test]
    fn baseline_decoders_invert_the_encoders() {
        let isa = Isa::Baseline;
        let branch = isa.encode_branch(c10_isa::control::BLT, 9, 17, -128);
        assert_eq!(isa.branch_offset(branch), -128);
        assert_eq!(isa.branch_registers(branch), (9, 17));
        let jal = isa.encode_jal(1, -4 * 32_768).unwrap();
        assert_eq!(isa.jal_offset(jal), -32_768);
        assert_eq!(isa.rd(jal), 1);
        let jmp = isa.encode_offset24(c10_isa::control::JMP, -5);
        assert_eq!(isa.imm24(jmp), -5);
        assert_eq!(
            isa.rd(isa.encode_literal(c10_isa::memory::LDI64, 7, 300)),
            7
        );
    }

    #[test]
    fn c10_words_are_the_consolidated_encoders() {
        let isa = Isa::C10;
        assert_eq!(
            isa.encode_addi(1, 1, 7).unwrap(),
            c10_encoding::encode_ri(c10_isa::arithmetic::ADDI, 1, 1, 7)
        );
        assert_eq!(isa.encode_scall(0x20), c10_encoding::encode_scall(0x20));
        assert_eq!(isa.syscall_number(isa.encode_scall(0x123)), Some(0x123));
        assert_eq!(
            isa.encode_return(),
            c10_encoding::encode_ri(c10_isa::control::JALR, 0, 1, 0)
        );
        let branch = isa.encode_branch_rv(0x4, 3, 4, 4 * 2_000).unwrap();
        assert_eq!(isa.branch_offset(branch), 2_000);
        assert_eq!(isa.branch_registers(branch), (3, 4));
    }

    #[test]
    fn the_nop_placeholder_is_one_word_in_both_layouts() {
        // Reserved words are written before the target encoding is known to every caller.
        assert_eq!(
            Isa::C10.encode_addi(0, 0, 0).unwrap(),
            Isa::Baseline.encode_addi(0, 0, 0).unwrap()
        );
    }

    #[test]
    fn baseline_mode_scans_recognise_their_opcodes() {
        let vector = baseline_encoding::encode_rr(baseline_isa::crypto::SETVL, 0, 0, 4);
        let assert = baseline_encoding::encode_rr(baseline_isa::zk::ASSERT, 0, 1, 0);
        let halt = baseline_encoding::encode_halt();
        assert!(baseline_uses_vector(&vector.to_le_bytes()));
        assert!(!baseline_uses_vector(&halt.to_le_bytes()));
        assert!(baseline_uses_zk(&assert.to_le_bytes()));
        assert!(!baseline_uses_zk(&halt.to_le_bytes()));
    }
}
