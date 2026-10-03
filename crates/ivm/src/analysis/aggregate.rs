//! Aggregate register and memory analysis with one exact syscall scratch owner.

use super::{
    MemoryAccesses, ProgramAnalysis, RegisterUsage, SyscallUsages, syscall_usage::UsageScratch,
};
use crate::{ProgramMetadata, VMError, encoding, instruction::wide, ivm_cache::DecodedOp};
use iroha_allocation::AllocationBudget;

pub(super) fn syscall_number(op: DecodedOp) -> Option<u32> {
    match wide::opcode(op.inst) {
        wide::system::SCALL => Some(u32::from(wide::imm8(op.inst) as u8)),
        wide::system::SYSTEM => Some(crate::encoding::wide::decode_syscallx(op.inst)),
        _ => None,
    }
}

pub(super) fn analyze<I: Iterator<Item = DecodedOp>>(
    metadata: ProgramMetadata,
    instructions: impl Fn() -> I,
    budget: Option<&AllocationBudget>,
) -> Result<ProgramAnalysis, VMError> {
    let occurrences = instructions().filter_map(syscall_number).count();
    let mut scratch = UsageScratch::new(occurrences, budget)?;
    let mut builder = ProgramAnalysisBuilder::new(metadata);
    for op in instructions() {
        builder.visit(&op);
        if let Some(number) = syscall_number(op) {
            scratch.push(number)?;
        }
    }
    Ok(builder.finish(scratch.finish()?))
}

struct ProgramAnalysisBuilder {
    metadata: ProgramMetadata,
    registers: RegisterUsage,
    memory: MemoryAccesses,
    instruction_count: usize,
}
impl ProgramAnalysisBuilder {
    fn new(metadata: ProgramMetadata) -> Self {
        Self {
            metadata,
            registers: RegisterUsage::default(),
            memory: MemoryAccesses::default(),
            instruction_count: 0,
        }
    }
    fn finish(self, syscalls: SyscallUsages) -> ProgramAnalysis {
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
            wide::memory::LDLIT | wide::memory::LDI64 => {
                let rd = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                self.write(rd);
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
            wide::control::JAL => {
                let rd = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                self.write(rd);
            }
            wide::control::JALS => self.write(1u8),
            wide::control::JMP | wide::control::HALT => {}
            // System helpers.
            wide::system::GETGAS => {
                let rd = u8::try_from(wide::rd(op.inst)).expect("register index fits in u8");
                self.write(rd);
            }
            wide::system::SCALL | wide::system::SYSTEM => {}
            // Vector configuration.
            wide::crypto::SETVL => {
                // SETVL carries its lane count in the rs2/immediate field; it
                // does not consume a vector or scalar register operand.
            }
            wide::crypto::PARBEGIN | wide::crypto::PAREND => {}
            wide::crypto::POSEIDON2 => self.two_src_one_dst(op.inst),
            wide::crypto::POSEIDON6 => {
                let rd = Self::reg(wide::rd(op.inst));
                self.write(rd);
                if let Some((_, rs_base)) = crate::encoding::wide::decode_poseidon6(op.inst) {
                    for offset in 0..wide::crypto::POSEIDON6_INPUTS {
                        self.read(usize::from(rs_base) + offset);
                    }
                }
            }
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
mod tests;
