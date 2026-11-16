//! Frozen gas schedule of the baseline engine ([`crate::IvmEngine::Baseline`]).
//!
//! The anchor `77600a4840` priced its instruction set with this table: vector costs scale from a
//! two-lane baseline by the deterministic logical vector length, direct signature opcodes add a
//! byte-linear surcharge, and Ed25519 batch verification adds request-byte and per-entry work. The
//! schedule descriptor binds the same syscall metering registry and named parameters as the C10
//! schedule plus the baseline-only parameters, in the anchor's order; its hash is `af5a27f5…`
//! (`crates/ivm/tests/fixtures/baseline_v1/descriptors/gas_schedule_v1.descriptor.bin`).

use super::{
    GAS_SCHEDULE_DESCRIPTOR_VERSION, GAS_SCHEDULE_DOMAIN, GasParameter, GasScheduleDescriptor,
    SYSCALL_GAS_PER_BYTE, canonical_gas_parameters, canonical_staged_phases,
    canonical_syscall_metering, encode_gas_schedule_descriptor,
};
use iroha_crypto::Hash;
use ivm_abi::baseline::instruction::wide;

/// Gas accounting treats two lanes as the baseline for vector operations.
pub const VECTOR_BASE_LANES: usize = 2;
/// Version of the V1 direct signature-opcode byte gas formula.
pub const SIGNATURE_OPCODE_GAS_FORMULA_VERSION_V1: u64 = 1;
/// Gas charged for each payload byte consumed by a direct signature opcode.
pub const SIGNATURE_OPCODE_GAS_PER_PAYLOAD_BYTE: u64 = SYSCALL_GAS_PER_BYTE;
/// Version of the bounded Ed25519 batch-verification gas formula.
pub const ED25519_BATCH_GAS_FORMULA_VERSION_V1: u64 = 1;
/// Gas charged for each encoded request byte before TLV hashing or Norito decoding.
pub const ED25519_BATCH_GAS_PER_PAYLOAD_BYTE: u64 = 1;
/// Gas charged before strictly verifying one Ed25519 batch entry.
pub const ED25519_BATCH_GAS_PER_ENTRY: u64 = 1_000;
/// Maximum entries accepted by one Ed25519 batch-verification instruction.
pub const ED25519_BATCH_MAX_ENTRIES: usize = 512;
/// Maximum encoded Norito request bytes accepted by batch verification.
pub const ED25519_BATCH_MAX_PAYLOAD_BYTES: usize = 512 * 1024;
/// Maximum elements in one nested sequence, including an entry message.
pub const ED25519_BATCH_MAX_SEQUENCE_ELEMENTS: usize = 64 * 1024;
/// Maximum total decoded elements across one batch request.
pub const ED25519_BATCH_MAX_TOTAL_ELEMENTS: usize = ED25519_BATCH_MAX_PAYLOAD_BYTES;
/// Maximum cumulative allocation permitted while decoding a batch request.
pub const ED25519_BATCH_MAX_DECODE_ALLOCATION_BYTES: usize = 2 * ED25519_BATCH_MAX_PAYLOAD_BYTES;
/// Maximum structural nesting depth accepted by a batch request.
pub const ED25519_BATCH_MAX_DECODE_DEPTH: usize = 8;
/// Decode budget for one attacker-authored Ed25519 batch request.
pub const ED25519_BATCH_DECODE_LIMITS: norito::DecodeLimits = norito::DecodeLimits::new(
    ED25519_BATCH_MAX_SEQUENCE_ELEMENTS,
    ED25519_BATCH_MAX_PAYLOAD_BYTES,
    ED25519_BATCH_MAX_TOTAL_ELEMENTS,
    ED25519_BATCH_MAX_DECODE_ALLOCATION_BYTES,
    ED25519_BATCH_MAX_DECODE_DEPTH,
);
/// Compute gas that must be reserved before Ed25519 batch request work.
///
/// The fixed opcode base is charged by [`cost_of`]. This function covers the
/// complete request bytes and every strict per-entry verification.
#[must_use]
pub const fn ed25519_batch_extra_gas(payload_bytes: u64, entries: u64) -> u64 {
    ED25519_BATCH_GAS_PER_PAYLOAD_BYTE
        .saturating_mul(payload_bytes)
        .saturating_add(ED25519_BATCH_GAS_PER_ENTRY.saturating_mul(entries))
}
/// Compute the byte-linear surcharge for one direct signature opcode.
///
/// Aliased operands are charged independently because the opcode validates
/// each message, signature, and public-key TLV independently.
#[must_use]
pub fn signature_opcode_extra_gas(
    message_bytes: u64,
    signature_bytes: u64,
    public_key_bytes: u64,
) -> Option<u64> {
    message_bytes
        .checked_add(signature_bytes)?
        .checked_add(public_key_bytes)?
        .checked_mul(SIGNATURE_OPCODE_GAS_PER_PAYLOAD_BYTE)
}
/// Scale a vector opcode's base cost by the actual logical lane count.
pub(crate) fn scaled_vector_cost(base_cost: u64, vector_len: usize) -> u64 {
    let lanes = vector_len.max(1) as u64;
    base_cost
        .saturating_mul(lanes)
        .div_ceil(VECTOR_BASE_LANES as u64)
}
/// Canonical opcode set covered by the gas schedule.
///
/// Keep this list in opcode order so `schedule_hash` remains deterministic across platforms.
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
    wide::memory::LDLIT,
    wide::memory::LDI64,
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
    wide::system::SYSTEM,
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
        wide::memory::LDLIT | wide::memory::LDI64 => Some(1),
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
        wide::system::SCALL | wide::system::SYSTEM => Some(5),
        wide::system::GETGAS => Some(0),
        wide::crypto::VADD32 | wide::crypto::VADD64 => Some(2),
        wide::crypto::VAND | wide::crypto::VXOR | wide::crypto::VOR | wide::crypto::VROT32 => {
            Some(1)
        }
        wide::crypto::SETVL => Some(1),
        wide::crypto::PARBEGIN | wide::crypto::PAREND => Some(0),
        wide::crypto::SHA256BLOCK | wide::crypto::SHA3BLOCK => Some(50),
        wide::crypto::POSEIDON2 | wide::crypto::POSEIDON6 => Some(10),
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
/// Compute gas cost when the base cost and opcode were already extracted.
pub(crate) fn cost_from_parts(
    base_cost: Option<u64>,
    wide_op: u8,
    vector_len: usize,
) -> Option<u64> {
    let mut cost = base_cost?;
    if matches!(
        wide_op,
        wide::crypto::VADD32
            | wide::crypto::VADD64
            | wide::crypto::VAND
            | wide::crypto::VXOR
            | wide::crypto::VOR
            | wide::crypto::VROT32
    ) {
        cost = scaled_vector_cost(cost, vector_len);
    }
    Some(cost)
}
/// Compute gas cost using the supplied deterministic logical vector length.
pub fn cost_of_with_vector_len(instr: u32, vector_len: usize) -> Option<u64> {
    let wide_op = wide::opcode(instr);
    cost_from_parts(cost_of(instr), wide_op, vector_len)
}
/// Named parameters of the baseline schedule: the shared parameters with the baseline-only
/// vector, signature-opcode and Ed25519-batch parameters at their original positions.
fn baseline_gas_parameters() -> Vec<GasParameter> {
    fn insert_after(
        parameters: &mut Vec<GasParameter>,
        anchor: &str,
        added: &[(&'static str, u64)],
    ) {
        let position = parameters
            .iter()
            .position(|parameter| parameter.name == anchor)
            .expect("the shared schedule keeps every baseline anchor parameter");
        for (offset, &(name, value)) in added.iter().enumerate() {
            parameters.insert(position + 1 + offset, GasParameter { name, value });
        }
    }
    let mut parameters = canonical_gas_parameters();
    insert_after(
        &mut parameters,
        "call_word_bytes",
        &[("vector_base_lanes", VECTOR_BASE_LANES as u64)],
    );
    insert_after(
        &mut parameters,
        "syscall_per_byte",
        &[
            (
                "signature_opcode_formula_version",
                SIGNATURE_OPCODE_GAS_FORMULA_VERSION_V1,
            ),
            (
                "signature_opcode_per_payload_byte",
                SIGNATURE_OPCODE_GAS_PER_PAYLOAD_BYTE,
            ),
        ],
    );
    insert_after(
        &mut parameters,
        "ledger_query_per_byte",
        &[
            (
                "ed25519_batch_formula_version",
                ED25519_BATCH_GAS_FORMULA_VERSION_V1,
            ),
            (
                "ed25519_batch_per_payload_byte",
                ED25519_BATCH_GAS_PER_PAYLOAD_BYTE,
            ),
            ("ed25519_batch_per_entry", ED25519_BATCH_GAS_PER_ENTRY),
            (
                "ed25519_batch_max_entries",
                ED25519_BATCH_MAX_ENTRIES as u64,
            ),
            (
                "ed25519_batch_max_payload_bytes",
                ED25519_BATCH_MAX_PAYLOAD_BYTES as u64,
            ),
            (
                "ed25519_batch_decode_max_sequence_elements",
                ED25519_BATCH_MAX_SEQUENCE_ELEMENTS as u64,
            ),
            (
                "ed25519_batch_decode_max_field_bytes",
                ED25519_BATCH_MAX_PAYLOAD_BYTES as u64,
            ),
            (
                "ed25519_batch_decode_max_total_elements",
                ED25519_BATCH_MAX_TOTAL_ELEMENTS as u64,
            ),
            (
                "ed25519_batch_decode_max_total_allocated_bytes",
                ED25519_BATCH_MAX_DECODE_ALLOCATION_BYTES as u64,
            ),
            (
                "ed25519_batch_decode_max_nesting_depth",
                ED25519_BATCH_MAX_DECODE_DEPTH as u64,
            ),
        ],
    );
    parameters
}
fn baseline_gas_schedule_descriptor() -> GasScheduleDescriptor {
    let opcodes = SCHEDULE_OPCODES
        .iter()
        .map(|&opcode| {
            let instruction = u32::from(opcode) << 24;
            (
                opcode,
                cost_of(instruction).expect("scheduled opcode must have gas cost"),
            )
        })
        .collect();
    GasScheduleDescriptor {
        domain: GAS_SCHEDULE_DOMAIN,
        version: GAS_SCHEDULE_DESCRIPTOR_VERSION,
        opcodes,
        parameters: baseline_gas_parameters(),
        staged_phases: canonical_staged_phases(),
        syscalls: canonical_syscall_metering(),
    }
}
/// Canonical gas schedule descriptor bytes of the baseline engine.
#[must_use]
pub fn schedule_descriptor() -> Vec<u8> {
    encode_gas_schedule_descriptor(&baseline_gas_schedule_descriptor())
}
/// Hash of the baseline gas schedule (`af5a27f5…`).
#[must_use]
pub fn schedule_hash() -> Hash {
    Hash::new(schedule_descriptor())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_schedule_prices_every_scheduled_opcode_and_rejects_the_rest() {
        assert_eq!(SCHEDULE_OPCODES.len(), 90);
        for opcode in 0..=u8::MAX {
            let priced = cost_of(u32::from(opcode) << 24).is_some();
            assert_eq!(
                priced,
                SCHEDULE_OPCODES.contains(&opcode),
                "opcode {opcode:#04x}"
            );
            assert_eq!(
                priced,
                wide::is_valid_opcode(opcode),
                "opcode {opcode:#04x}"
            );
        }
        assert_eq!(max_instruction_cost(), 5000);
        assert_eq!(cost_from_parts(Some(2), wide::crypto::VADD32, 8), Some(8));
        assert_eq!(cost_from_parts(Some(2), wide::crypto::VADD32, 0), Some(1));
        assert_eq!(cost_from_parts(Some(5), wide::arithmetic::ADD, 64), Some(5));
        assert_eq!(cost_from_parts(None, wide::arithmetic::ADD, 4), None);
        assert_eq!(signature_opcode_extra_gas(1, 2, 3), Some(6));
        assert_eq!(signature_opcode_extra_gas(u64::MAX, 1, 0), None);
        assert_eq!(ed25519_batch_extra_gas(10, 2), 2_010);
    }

    #[test]
    fn baseline_schedule_hash_is_the_installed_metering_identity() {
        let parameters = baseline_gas_parameters();
        assert_eq!(parameters.len(), canonical_gas_parameters().len() + 13);
        assert_eq!(
            schedule_hash(),
            Hash::new(schedule_descriptor()),
            "the hash covers exactly the descriptor bytes"
        );
        assert_eq!(
            hex::encode(schedule_hash().as_ref()),
            "af5a27f5fc57ebfb9eae3c4f8a25091ce78d97ff7797dbc3b304a0091069731f"
        );
        assert_ne!(schedule_hash(), super::super::c10_schedule_hash());
        assert_eq!(schedule_hash(), super::super::schedule_hash());
    }
}
