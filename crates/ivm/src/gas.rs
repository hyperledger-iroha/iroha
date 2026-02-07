//! Gas cost table and utilities for instruction gas accounting.
//!
//! Canonical schedule
//! - This table is the canonical source for gas costs used by the interpreter.
//!   Tests compare runtime accounting against this table to ensure conformance.
//! - Costs follow the IVM spec. Variants (e.g., DIV vs MUL) are distinguished by
//!   decoding full instruction words (funct fields), not just opcodes.
//!
//! Scope
//! - Includes extended vector/parallel and cryptographic instructions.
//! - Vector length scaling and HTM retry penalties are supported; current VM uses
//!   a fixed 128‑bit vector width and rarely incurs HTM retries.

use iroha_crypto::Hash;

use crate::instruction::wide;

/// Gas accounting treats two lanes as the baseline for vector operations.
pub const VECTOR_BASE_LANES: usize = 2;

/// Canonical opcode set covered by the gas schedule.
///
/// Keep this list in opcode order so `schedule_hash` remains deterministic
/// across platforms.
pub const SCHEDULE_OPCODES: &[u8] = &[
    // Arithmetic + logic
    wide::arithmetic::ADD,
    wide::arithmetic::SUB,
    wide::arithmetic::AND,
    wide::arithmetic::OR,
    wide::arithmetic::XOR,
    wide::arithmetic::SLL,
    wide::arithmetic::SRL,
    wide::arithmetic::SRA,
    wide::arithmetic::NEG,
    wide::arithmetic::NOT,
    wide::arithmetic::ADDI,
    wide::arithmetic::ANDI,
    wide::arithmetic::ORI,
    wide::arithmetic::XORI,
    wide::arithmetic::MUL,
    wide::arithmetic::MULH,
    wide::arithmetic::MULHU,
    wide::arithmetic::MULHSU,
    wide::arithmetic::DIV,
    wide::arithmetic::DIVU,
    wide::arithmetic::REM,
    wide::arithmetic::REMU,
    wide::arithmetic::ROTL,
    wide::arithmetic::ROTR,
    wide::arithmetic::ROTL_IMM,
    wide::arithmetic::ROTR_IMM,
    wide::arithmetic::POPCNT,
    wide::arithmetic::CLZ,
    wide::arithmetic::CTZ,
    wide::arithmetic::ISQRT,
    wide::arithmetic::MIN,
    wide::arithmetic::MAX,
    wide::arithmetic::ABS,
    wide::arithmetic::DIV_CEIL,
    wide::arithmetic::GCD,
    wide::arithmetic::MEAN,
    wide::arithmetic::SLT,
    wide::arithmetic::SLTU,
    wide::arithmetic::SEQ,
    wide::arithmetic::SNE,
    wide::arithmetic::CMOV,
    wide::arithmetic::CMOVI,
    // Memory
    wide::memory::LOAD64,
    wide::memory::STORE64,
    wide::memory::LOAD128,
    wide::memory::STORE128,
    // Control flow
    wide::control::BEQ,
    wide::control::BNE,
    wide::control::BLT,
    wide::control::BGE,
    wide::control::BLTU,
    wide::control::BGEU,
    wide::control::JAL,
    wide::control::JALR,
    wide::control::JR,
    wide::control::JMP,
    wide::control::JALS,
    wide::control::HALT,
    // System
    wide::system::SCALL,
    wide::system::GETGAS,
    // Crypto/vector
    wide::crypto::VADD32,
    wide::crypto::VADD64,
    wide::crypto::VAND,
    wide::crypto::VXOR,
    wide::crypto::VOR,
    wide::crypto::VROT32,
    wide::crypto::SETVL,
    wide::crypto::PARBEGIN,
    wide::crypto::PAREND,
    wide::crypto::SHA256BLOCK,
    wide::crypto::SHA3BLOCK,
    wide::crypto::POSEIDON2,
    wide::crypto::POSEIDON6,
    wide::crypto::PUBKGEN,
    wide::crypto::VALCOM,
    wide::crypto::ECADD,
    wide::crypto::ECMUL_VAR,
    wide::crypto::PAIRING,
    wide::crypto::AESENC,
    wide::crypto::AESDEC,
    wide::crypto::BLAKE2S,
    wide::crypto::ED25519VERIFY,
    wide::crypto::ED25519BATCHVERIFY,
    wide::crypto::ECDSAVERIFY,
    wide::crypto::DILITHIUMVERIFY,
    // ZK helpers
    wide::zk::ASSERT,
    wide::zk::ASSERT_EQ,
    wide::zk::FADD,
    wide::zk::FSUB,
    wide::zk::FMUL,
    wide::zk::FINV,
    wide::zk::ASSERT_RANGE,
];

/// Return the gas cost for the given 32-bit instruction word.
///
/// Property tests in `crates/ivm/tests/gas_property.rs` exercise representative
/// instruction sequences to ensure runtime accounting matches this schedule.
// See roadmap.md → Spec → Implementation Plan (Folded) → Opcode + Gas Reference is normative.
pub fn cost_of(instr: u32) -> Option<u64> {
    let wide_op = wide::opcode(instr);

    match wide_op {
        wide::arithmetic::ADD
        | wide::arithmetic::SUB
        | wide::arithmetic::AND
        | wide::arithmetic::OR
        | wide::arithmetic::XOR
        | wide::arithmetic::SLL
        | wide::arithmetic::SRL
        | wide::arithmetic::SRA
        | wide::arithmetic::NEG
        | wide::arithmetic::NOT
        | wide::arithmetic::ADDI
        | wide::arithmetic::ANDI
        | wide::arithmetic::ORI
        | wide::arithmetic::XORI => Some(1),
        wide::arithmetic::MUL
        | wide::arithmetic::MULH
        | wide::arithmetic::MULHU
        | wide::arithmetic::MULHSU => Some(3),
        wide::arithmetic::DIV
        | wide::arithmetic::DIVU
        | wide::arithmetic::REM
        | wide::arithmetic::REMU => Some(10),
        wide::arithmetic::ROTL
        | wide::arithmetic::ROTR
        | wide::arithmetic::ROTL_IMM
        | wide::arithmetic::ROTR_IMM => Some(2),
        wide::arithmetic::POPCNT
        | wide::arithmetic::CLZ
        | wide::arithmetic::CTZ
        | wide::arithmetic::ISQRT => Some(6),
        wide::arithmetic::MIN | wide::arithmetic::MAX | wide::arithmetic::ABS => Some(1),
        wide::arithmetic::DIV_CEIL | wide::arithmetic::GCD => Some(12),
        wide::arithmetic::MEAN => Some(2),
        wide::arithmetic::SLT | wide::arithmetic::SLTU => Some(2),
        wide::arithmetic::SEQ | wide::arithmetic::SNE => Some(2),
        wide::arithmetic::CMOV | wide::arithmetic::CMOVI => Some(3),
        wide::memory::LOAD64 | wide::memory::STORE64 => Some(3),
        wide::memory::LOAD128 | wide::memory::STORE128 => Some(5),
        wide::control::BEQ
        | wide::control::BNE
        | wide::control::BLT
        | wide::control::BGE
        | wide::control::BLTU
        | wide::control::BGEU => Some(1),
        wide::control::JAL
        | wide::control::JALR
        | wide::control::JR
        | wide::control::JMP
        | wide::control::JALS => Some(2),
        wide::control::HALT => Some(0),
        wide::system::SCALL => Some(5),
        wide::system::GETGAS => Some(0),
        wide::crypto::VADD32 | wide::crypto::VADD64 => Some(2),
        wide::crypto::VAND | wide::crypto::VXOR | wide::crypto::VOR | wide::crypto::VROT32 => {
            Some(1)
        }
        wide::crypto::SETVL => Some(1),
        wide::crypto::PARBEGIN | wide::crypto::PAREND => Some(0),
        wide::crypto::SHA256BLOCK | wide::crypto::SHA3BLOCK => Some(50),
        wide::crypto::POSEIDON2 | wide::crypto::POSEIDON6 => Some(10),
        wide::crypto::PUBKGEN | wide::crypto::VALCOM => Some(50),
        wide::crypto::ECADD => Some(20),
        wide::crypto::ECMUL_VAR => Some(100),
        wide::crypto::PAIRING => Some(500),
        wide::crypto::AESENC | wide::crypto::AESDEC => Some(30),
        wide::crypto::BLAKE2S => Some(40),
        wide::crypto::ED25519VERIFY => Some(1000),
        wide::crypto::ED25519BATCHVERIFY => Some(500),
        wide::crypto::ECDSAVERIFY => Some(1500),
        wide::crypto::DILITHIUMVERIFY => Some(5000),
        wide::zk::ASSERT | wide::zk::ASSERT_EQ | wide::zk::ASSERT_RANGE => Some(1),
        wide::zk::FADD | wide::zk::FSUB => Some(1),
        wide::zk::FMUL => Some(3),
        wide::zk::FINV => Some(5),
        _ => None,
    }
}

/// Maximum base cost across all scheduled opcodes.
#[must_use]
pub fn max_instruction_cost() -> u64 {
    SCHEDULE_OPCODES
        .iter()
        .map(|op| cost_of((*op as u32) << 24).expect("scheduled opcode must have gas cost"))
        .max()
        .unwrap_or(0)
}

/// Compute gas cost considering vector length and HTM retries.
#[allow(dead_code)]
pub fn cost_of_with_params(instr: u32, vector_len: usize, htm_retries: u32) -> Option<u64> {
    let mut cost = cost_of(instr)?;
    let wide_op = wide::opcode(instr);
    if matches!(
        wide_op,
        wide::crypto::VADD32
            | wide::crypto::VADD64
            | wide::crypto::VAND
            | wide::crypto::VXOR
            | wide::crypto::VOR
            | wide::crypto::VROT32
    ) {
        let lanes = vector_len.clamp(1, VECTOR_BASE_LANES);
        cost = (cost * lanes as u64).div_ceil(VECTOR_BASE_LANES as u64);
    }
    Some(cost.saturating_mul(htm_retries as u64 + 1))
}

/// Deterministic digest of the canonical gas schedule.
///
/// The digest is derived from the opcode → cost table used by the interpreter so
/// validators can assert the active schedule matches consensus configuration.
#[must_use]
pub fn schedule_hash() -> Hash {
    let mut buf: Vec<u8> =
        Vec::with_capacity(SCHEDULE_OPCODES.len() * (1 + core::mem::size_of::<u64>()));
    for &op in SCHEDULE_OPCODES {
        let instr = u32::from(op) << 24;
        let cost = cost_of(instr).expect("scheduled opcode must have gas cost");
        buf.push(op);
        buf.extend_from_slice(&cost.to_le_bytes());
    }
    Hash::new(buf)
}
