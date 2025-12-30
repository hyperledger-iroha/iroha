//! Static analysis helpers for compiled IVM programs.
//!
//! This module exposes lightweight inspection utilities that decode `.to`
//! artifacts, walk the canonical instruction stream, and derive register,
//! memory, and syscall usage summaries. The output feeds the AMX/Nexus
//! pipelines (see roadmap item NX-17) by providing a deterministic read/write
//! fingerprint that can be compared against the declared UAID manifests.
//!
//! Typical usage:
//! ```no_run
//! # use ivm::analysis;
//! # fn inspect(bytes: &[u8]) {
//! let report = analysis::analyze_program(bytes).expect("valid program");
//! println!("{} instructions", report.instruction_count);
//! println!("load64 count {}", report.memory.load64);
//! for syscall in &report.syscalls {
//!     println!(
//!         "syscall 0x{:02x} used {} times",
//!         syscall.number, syscall.count
//!     );
//! }
//! # }
//! ```

use core::convert::TryFrom as _;
use std::{collections::BTreeMap, error::Error, fmt, num::NonZeroUsize};

use crate::{
    VMError, encoding,
    instruction::wide,
    ivm_cache::{DecodedOp, IvmCache},
    metadata::ProgramMetadata,
};

/// Aggregate register usage counters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterUsage {
    /// Number of times each register was read.
    pub reads: [u64; 256],
    /// Number of times each register was written.
    pub writes: [u64; 256],
}

impl Default for RegisterUsage {
    fn default() -> Self {
        Self {
            reads: [0; 256],
            writes: [0; 256],
        }
    }
}

/// Memory instruction statistics captured during analysis.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MemoryAccesses {
    pub load64: u64,
    pub store64: u64,
    pub load128: u64,
    pub store128: u64,
}

/// Syscall usage summary sorted by syscall number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyscallUsage {
    pub number: u8,
    pub count: u64,
}

/// Result of analysing a compiled program.
#[derive(Debug, Clone)]
pub struct ProgramAnalysis {
    pub metadata: ProgramMetadata,
    pub instruction_count: usize,
    pub registers: RegisterUsage,
    pub memory: MemoryAccesses,
    pub syscalls: Vec<SyscallUsage>,
}

/// Errors emitted by [`analyze_program`].
#[derive(Debug)]
pub enum ProgramAnalysisError {
    Metadata(VMError),
    Decode(VMError),
}

impl fmt::Display for ProgramAnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProgramAnalysisError::Metadata(err) => {
                write!(f, "failed to parse program metadata: {err}")
            }
            ProgramAnalysisError::Decode(err) => {
                write!(f, "failed to decode instruction stream: {err}")
            }
        }
    }
}

impl Error for ProgramAnalysisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ProgramAnalysisError::Metadata(err) | ProgramAnalysisError::Decode(err) => Some(err),
        }
    }
}

/// Decode the program contained in `bytes` and return aggregate read/write and
/// syscall usage information.
pub fn analyze_program(bytes: &[u8]) -> Result<ProgramAnalysis, ProgramAnalysisError> {
    let parsed = ProgramMetadata::parse(bytes).map_err(ProgramAnalysisError::Metadata)?;
    let code = &bytes[parsed.code_offset..];
    let decoded = IvmCache::decode_stream(code).map_err(ProgramAnalysisError::Decode)?;
    let mut builder = ProgramAnalysisBuilder::new(parsed.metadata.clone());
    for op in decoded.iter() {
        builder.visit(op);
    }
    Ok(builder.finish())
}

/// Default execution budgets for atomic multi-dataspace execution (NX-17).
#[derive(Debug, Clone)]
pub struct AmxLimits {
    /// Per-dataspace budget in milliseconds (defaults to 30 ms).
    pub per_dataspace_budget_ms: u64,
    /// Group budget across all dataspaces in milliseconds (defaults to 140 ms).
    pub group_budget_ms: u64,
    /// Per-instruction cost in nanoseconds used for coarse estimation.
    pub per_instruction_ns: u64,
    /// Cost in nanoseconds for each memory access.
    pub per_memory_access_ns: u64,
    /// Cost in nanoseconds per syscall invocation.
    pub per_syscall_ns: u64,
}

impl Default for AmxLimits {
    fn default() -> Self {
        Self {
            per_dataspace_budget_ms: 30,
            group_budget_ms: 140,
            per_instruction_ns: 50,
            per_memory_access_ns: 80,
            per_syscall_ns: 120,
        }
    }
}

/// Estimated cost summary for AMX budgeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmxCost {
    /// Estimated per-dataspace execution time (nanoseconds).
    pub estimated_ns_per_dataspace: u64,
    /// Estimated group execution time across all dataspaces (nanoseconds).
    pub estimated_group_ns: u64,
}

/// Budget violation raised when AMX estimates exceed configured limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmxBudgetError {
    /// The per-dataspace budget was exceeded.
    PerDataspaceBudgetExceeded {
        /// Estimated cost in nanoseconds.
        estimated_ns: u64,
        /// Allowed budget in nanoseconds.
        limit_ns: u64,
    },
    /// The group budget across dataspaces was exceeded.
    GroupBudgetExceeded {
        /// Estimated group cost in nanoseconds.
        estimated_ns: u64,
        /// Allowed group budget in nanoseconds.
        limit_ns: u64,
    },
}

/// Estimate the AMX execution cost and enforce per-dataspace/group budgets.
pub fn enforce_amx_budget(
    analysis: &ProgramAnalysis,
    dataspace_count: NonZeroUsize,
    limits: &AmxLimits,
) -> Result<AmxCost, AmxBudgetError> {
    let per_ds_limit = limits.per_dataspace_budget_ms.saturating_mul(1_000_000);
    let group_limit = limits.group_budget_ms.saturating_mul(1_000_000);

    let memory_accesses = analysis.memory.load64
        + analysis.memory.store64
        + analysis.memory.load128
        + analysis.memory.store128;
    let syscall_calls: u64 = analysis.syscalls.iter().map(|entry| entry.count).sum();

    let estimated_ns = analysis
        .instruction_count
        .saturating_mul(limits.per_instruction_ns as usize) as u64
        + memory_accesses.saturating_mul(limits.per_memory_access_ns)
        + syscall_calls.saturating_mul(limits.per_syscall_ns);

    if estimated_ns > per_ds_limit {
        return Err(AmxBudgetError::PerDataspaceBudgetExceeded {
            estimated_ns,
            limit_ns: per_ds_limit,
        });
    }

    let group_estimated_ns = estimated_ns.saturating_mul(dataspace_count.get() as u64);
    if group_estimated_ns > group_limit {
        return Err(AmxBudgetError::GroupBudgetExceeded {
            estimated_ns: group_estimated_ns,
            limit_ns: group_limit,
        });
    }

    Ok(AmxCost {
        estimated_ns_per_dataspace: estimated_ns,
        estimated_group_ns: group_estimated_ns,
    })
}

struct ProgramAnalysisBuilder {
    metadata: ProgramMetadata,
    registers: RegisterUsage,
    memory: MemoryAccesses,
    instruction_count: usize,
    syscall_table: BTreeMap<u8, u64>,
}

impl ProgramAnalysisBuilder {
    fn new(metadata: ProgramMetadata) -> Self {
        Self {
            metadata,
            registers: RegisterUsage::default(),
            memory: MemoryAccesses::default(),
            instruction_count: 0,
            syscall_table: BTreeMap::new(),
        }
    }

    fn finish(self) -> ProgramAnalysis {
        let syscalls = self
            .syscall_table
            .into_iter()
            .map(|(number, count)| SyscallUsage { number, count })
            .collect();
        ProgramAnalysis {
            metadata: self.metadata,
            instruction_count: self.instruction_count,
            registers: self.registers,
            memory: self.memory,
            syscalls,
        }
    }

    fn visit(&mut self, op: &DecodedOp) {
        self.instruction_count += 1;
        let opcode = wide::opcode(op.inst);
        match opcode {
            // ALU operations with two explicit sources.
            wide::arithmetic::ADD
            | wide::arithmetic::SUB
            | wide::arithmetic::AND
            | wide::arithmetic::OR
            | wide::arithmetic::XOR
            | wide::arithmetic::SLL
            | wide::arithmetic::SRL
            | wide::arithmetic::SRA
            | wide::arithmetic::SLT
            | wide::arithmetic::SLTU
            | wide::arithmetic::CMOV
            | wide::arithmetic::SEQ
            | wide::arithmetic::SNE
            | wide::arithmetic::MUL
            | wide::arithmetic::MULH
            | wide::arithmetic::MULHU
            | wide::arithmetic::MULHSU
            | wide::arithmetic::DIV
            | wide::arithmetic::DIVU
            | wide::arithmetic::REM
            | wide::arithmetic::REMU
            | wide::arithmetic::ROTL
            | wide::arithmetic::ROTR
            | wide::arithmetic::MIN
            | wide::arithmetic::MAX
            | wide::arithmetic::DIV_CEIL
            | wide::arithmetic::GCD
            | wide::arithmetic::MEAN => {
                self.two_src_one_dst(op.inst);
            }
            // Unary ALU operations.
            wide::arithmetic::NOT
            | wide::arithmetic::NEG
            | wide::arithmetic::POPCNT
            | wide::arithmetic::CLZ
            | wide::arithmetic::CTZ
            | wide::arithmetic::ABS
            | wide::arithmetic::ISQRT => {
                self.one_src_one_dst(op.inst);
            }
            // Immediate ALU operations.
            wide::arithmetic::ADDI
            | wide::arithmetic::ANDI
            | wide::arithmetic::ORI
            | wide::arithmetic::XORI
            | wide::arithmetic::CMOVI
            | wide::arithmetic::ROTL_IMM
            | wide::arithmetic::ROTR_IMM => {
                self.one_src_one_dst(op.inst);
            }
            // Memory access instructions.
            wide::memory::LOAD64 => {
                self.memory.load64 += 1;
                let (_, dest, base, _) = encoding::wide::decode_mem(op.inst);
                self.write(dest);
                self.read(base);
            }
            wide::memory::STORE64 => {
                self.memory.store64 += 1;
                let (_, base, value, _) = encoding::wide::decode_mem(op.inst);
                self.read(base);
                self.read(value);
            }
            wide::memory::LOAD128 => {
                self.memory.load128 += 1;
                let (_, rd_lo, base, rd_hi) = encoding::wide::decode_load128(op.inst);
                self.write(rd_lo);
                self.write(rd_hi);
                self.read(base);
            }
            wide::memory::STORE128 => {
                self.memory.store128 += 1;
                let (_, base, rs_lo, rs_hi) = encoding::wide::decode_store128(op.inst);
                self.read(base);
                self.read(rs_lo);
                self.read(rs_hi);
            }
            // Control flow.
            wide::control::BEQ
            | wide::control::BNE
            | wide::control::BLT
            | wide::control::BGE
            | wide::control::BLTU
            | wide::control::BGEU => {
                let rs1 = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                let rs2 = u8::try_from(wide::rs1(op.inst)).expect("register index fits in u8");
                self.read(rs1);
                self.read(rs2);
            }
            wide::control::JR => {
                let rs = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                self.read(rs);
            }
            wide::control::JALR => {
                let rd = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                let rs = u8::try_from(wide::rs1(op.inst)).expect("register index fits in u8");
                self.write(rd);
                self.read(rs);
            }
            wide::control::JAL | wide::control::JALS => {
                let rd = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                self.write(rd);
            }
            wide::control::JMP | wide::control::HALT => {}
            // System helpers.
            wide::system::GETGAS => {
                let rd = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                self.write(rd);
            }
            wide::system::SCALL => {
                let number = wide::imm8(op.inst) as u8;
                *self.syscall_table.entry(number).or_default() += 1;
            }
            wide::system::SYSTEM => {}
            // Vector configuration.
            wide::crypto::SETVL => {
                let reg = u8::try_from(wide::rs2(op.inst)).expect("register index fits in u8");
                if reg != 0 {
                    self.read(reg);
                }
            }
            wide::crypto::PARBEGIN | wide::crypto::PAREND => {}
            // All remaining opcodes (crypto, ISO20022, ZK, vector ALU, etc.)
            // follow the canonical rd/rs1/rs2 layout.
            _ => {
                self.two_src_one_dst(op.inst);
            }
        }
    }

    fn two_src_one_dst(&mut self, inst: u32) {
        let rd = Self::reg(wide::rd(inst));
        let rs1 = Self::reg(wide::rs1(inst));
        let rs2 = Self::reg(wide::rs2(inst));
        self.write(rd);
        self.read(rs1);
        self.read(rs2);
    }

    fn one_src_one_dst(&mut self, inst: u32) {
        let rd = Self::reg(wide::rd(inst));
        let rs = Self::reg(wide::rs1(inst));
        self.write(rd);
        self.read(rs);
    }

    fn read<R>(&mut self, reg: R)
    where
        R: Into<usize>,
    {
        let idx = reg.into();
        debug_assert!(idx < self.registers.reads.len());
        self.registers.reads[idx] = self.registers.reads[idx].saturating_add(1);
    }

    fn write<R>(&mut self, reg: R)
    where
        R: Into<usize>,
    {
        let idx = reg.into();
        debug_assert!(idx < self.registers.writes.len());
        self.registers.writes[idx] = self.registers.writes[idx].saturating_add(1);
    }

    fn reg(index: usize) -> u8 {
        u8::try_from(index).expect("register index fits in u8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encoding::wide as wide_enc, instruction::wide};

    fn base_analysis(instruction_count: usize) -> ProgramAnalysis {
        ProgramAnalysis {
            metadata: ProgramMetadata::default(),
            instruction_count,
            registers: RegisterUsage::default(),
            memory: MemoryAccesses::default(),
            syscalls: Vec::new(),
        }
    }

    fn build_program(words: &[u32]) -> Vec<u8> {
        let mut bytes = ProgramMetadata::default_for(1, 0, 1).encode();
        for word in words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn analysis_reports_registers_memory_and_syscalls() {
        let words = [
            wide_enc::encode_ri(wide::arithmetic::ADDI, 1, 0, 5),
            wide_enc::encode_load(wide::memory::LOAD64, 2, 1, 0),
            wide_enc::encode_store(wide::memory::STORE64, 3, 2, 8),
            wide_enc::encode_branch(wide::control::BEQ, 2, 1, 1),
            wide_enc::encode_sys(wide::system::SCALL, 0x22),
            wide_enc::encode_halt(),
        ];
        let program = build_program(&words);
        let report = analyze_program(&program).expect("analysis succeeds");
        assert_eq!(report.instruction_count, words.len());
        assert_eq!(report.memory.load64, 1);
        assert_eq!(report.memory.store64, 1);
        assert_eq!(report.syscalls.len(), 1);
        assert_eq!(
            report.syscalls[0],
            SyscallUsage {
                number: 0x22,
                count: 1
            }
        );
        assert_eq!(report.registers.reads[0], 1);
        assert!(report.registers.writes[1] >= 1);
        assert!(report.registers.reads[1] >= 2);
        assert!(report.registers.reads[2] >= 1);
    }

    #[test]
    fn analysis_rejects_truncated_program() {
        let bytes = vec![0u8; 4];
        let err = analyze_program(&bytes).expect_err("metadata parse should fail");
        assert!(matches!(err, ProgramAnalysisError::Metadata(_)));
    }

    #[test]
    fn amx_budget_accepts_small_program() {
        let analysis = base_analysis(10_000);
        let limits = AmxLimits::default();
        let result = enforce_amx_budget(&analysis, NonZeroUsize::new(1).unwrap(), &limits).unwrap();
        assert!(result.estimated_ns_per_dataspace < limits.per_dataspace_budget_ms * 1_000_000);
    }

    #[test]
    fn amx_budget_rejects_large_program() {
        // 700k instructions * 50 ns = 35 ms > 30 ms per-dataspace budget.
        let analysis = base_analysis(700_000);
        let limits = AmxLimits::default();
        let err =
            enforce_amx_budget(&analysis, NonZeroUsize::new(1).unwrap(), &limits).unwrap_err();
        assert!(matches!(
            err,
            AmxBudgetError::PerDataspaceBudgetExceeded { .. }
        ));
    }

    #[test]
    fn amx_budget_rejects_group_overflow() {
        // Fits per-dataspace budget (~27.5 ms) but violates group budget when scaled to 6 lanes.
        let analysis = base_analysis(550_000);
        let limits = AmxLimits::default();
        let err =
            enforce_amx_budget(&analysis, NonZeroUsize::new(6).unwrap(), &limits).unwrap_err();
        assert!(matches!(err, AmxBudgetError::GroupBudgetExceeded { .. }));
    }
}
