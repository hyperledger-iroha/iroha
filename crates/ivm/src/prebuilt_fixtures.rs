//! Canonical builders for checked-in and staged IVM executor fixtures.
//!
//! Keeping these builders in the `ivm` library gives every exporter and test
//! one host-independent implementation. In particular, serialized length
//! prefixes are fixed-width `u64` values and never depend on `usize`.
//!
//! Executor programs run under the interface profile, the frozen baseline engine
//! ([`crate::IvmProfileId::BASELINE_ABI_V1`]): they use the baseline header (with the anchor's
//! default cycle limit) and the baseline instruction encoding, as the anchor's builders did.
use crate::{
    IvmProfileId, ProgramMetadata,
    metadata::{LITERAL_SECTION_MAGIC, LiteralKindV1, encode_literal_descriptor},
};
use iroha_data_model::ValidationFail;
use ivm_abi::baseline::{encoding, instruction};
use norito::codec::Encode;
/// The anchor's `DEFAULT_MAX_CYCLES`, declared by every executor fixture header.
const DEFAULT_MAX_CYCLES: u64 = 1_000_000;
const WIDE_IMM_MIN: i32 = -128;
const WIDE_IMM_MAX: i32 = 127;
const RESULT_LENGTH_PREFIX_BYTES: u64 = 8;
/// The profile executor fixtures are admitted and run under.
pub const EXECUTOR_FIXTURE_PROFILE: IvmProfileId = IvmProfileId::BASELINE_ABI_V1;
fn chunk_immediate(value: i32) -> i8 {
    i8::try_from(value.clamp(WIDE_IMM_MIN, WIDE_IMM_MAX)).expect("clamped to the i8 range")
}
fn emit_addi_inplace(code: &mut Vec<u32>, register: u8, mut value: i32) {
    while value != 0 {
        let chunk = chunk_immediate(value);
        code.push(encoding::wide::encode_ri(
            instruction::wide::arithmetic::ADDI,
            register,
            register,
            chunk,
        ));
        value -= i32::from(chunk);
    }
}
fn set_register(code: &mut Vec<u32>, register: u8, value: i32) {
    code.push(encoding::wide::encode_ri(
        instruction::wide::arithmetic::ADDI,
        register,
        0,
        0,
    ));
    emit_addi_inplace(code, register, value);
}
fn build_copy_program(data_offset: i32, chunks: usize) -> Vec<u32> {
    const SOURCE_POINTER: u8 = 12;
    const DESTINATION_POINTER: u8 = 13;
    const TEMPORARY: u8 = 14;
    let mut code = Vec::new();
    set_register(&mut code, SOURCE_POINTER, data_offset);
    code.push(encoding::wide::encode_ri(
        instruction::wide::arithmetic::ADDI,
        DESTINATION_POINTER,
        10,
        0,
    ));
    for _ in 0..chunks {
        code.push(encoding::wide::encode_load(
            instruction::wide::memory::LOAD64,
            TEMPORARY,
            SOURCE_POINTER,
            0,
        ));
        code.push(encoding::wide::encode_store(
            instruction::wide::memory::STORE64,
            DESTINATION_POINTER,
            TEMPORARY,
            0,
        ));
        emit_addi_inplace(&mut code, SOURCE_POINTER, 8);
        emit_addi_inplace(&mut code, DESTINATION_POINTER, 8);
    }
    code.push(encoding::wide::encode_halt());
    code
}
/// Build the canonical default executor program.
///
/// The program copies a Norito-encoded `Ok(())` validation verdict into the
/// host-provided output buffer and halts. Its data prefix is always an eight
/// byte little-endian `u64`, regardless of the build host's pointer width. The
/// bytes live in authenticated `LTLB` scalar entries before the executable
/// region, so strict admission never interprets verdict data as instructions.
#[must_use]
pub fn build_default_executor_program() -> Vec<u8> {
    let verdict: Result<(), ValidationFail> = Ok(());
    build_encoded_result_program(&verdict.encode())
}
/// Build an executor program that copies one encoded result payload to output.
///
/// The returned payload starts with a fixed-width little-endian `u64` length
/// that includes the prefix itself. Payload chunks are authenticated typed
/// literals, leaving the executable region instruction-only under strict V1
/// admission.
#[must_use]
pub fn build_encoded_result_program(result_bytes: &[u8]) -> Vec<u8> {
    let result_len = u64::try_from(result_bytes.len()).expect("bounded result length fits u64");
    let total_len = RESULT_LENGTH_PREFIX_BYTES
        .checked_add(result_len)
        .expect("bounded result length addition cannot overflow");
    let mut data = Vec::new();
    data.extend_from_slice(&total_len.to_le_bytes());
    data.extend_from_slice(result_bytes);
    while data.len() % 8 != 0 {
        data.push(0);
    }
    let chunk_count = data.len() / 8;
    let descriptor_bytes = chunk_count
        .checked_mul(8)
        .expect("bounded literal descriptor bytes");
    let data_offset = 16_usize
        .checked_add(descriptor_bytes)
        .expect("bounded literal data offset");
    let data_end = data_offset
        .checked_add(data.len())
        .expect("bounded literal data end");
    let post_pad = (4 - (data_end % 4)) % 4;
    let code = build_copy_program(
        i32::try_from(data_offset).expect("bounded literal data offset fits i32"),
        chunk_count,
    );
    let mut program = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        max_cycles: DEFAULT_MAX_CYCLES,
        abi_version: 1,
    }
    .encode_for_profile(EXECUTOR_FIXTURE_PROFILE, 0);
    program.extend_from_slice(&LITERAL_SECTION_MAGIC);
    program.extend_from_slice(
        &u32::try_from(chunk_count)
            .expect("bounded verdict chunk count fits u32")
            .to_le_bytes(),
    );
    program.extend_from_slice(
        &u32::try_from(post_pad)
            .expect("literal alignment padding fits u32")
            .to_le_bytes(),
    );
    program.extend_from_slice(
        &u32::try_from(data.len())
            .expect("bounded literal data length fits u32")
            .to_le_bytes(),
    );
    for index in 0..chunk_count {
        let relative = data_offset
            .checked_add(index.checked_mul(8).expect("bounded chunk offset"))
            .and_then(|offset| u64::try_from(offset).ok())
            .expect("bounded literal offset fits u64");
        let descriptor = encode_literal_descriptor(LiteralKindV1::I64, relative)
            .expect("bounded literal offset fits the descriptor domain");
        program.extend_from_slice(&descriptor.to_le_bytes());
    }
    program.extend_from_slice(&data);
    program.extend(std::iter::repeat_n(0, post_pad));
    for instruction in code {
        program.extend_from_slice(&instruction.to_le_bytes());
    }
    program
}
/// Build the raw (non-contract) program load tools deploy as an IVM trigger.
///
/// Triggers run under the interface profile, so this is a baseline program: the anchor's mixed
/// predecoder artifact (ADD, SUB, XOR, a taken BEQ over one filler instruction, then HALT) under a
/// ZK and VECTOR header with vector length 8 and a 1,000-cycle limit, byte for byte.
#[must_use]
pub fn build_raw_trigger_program() -> Vec<u8> {
    const MODE_ZK_VECTOR: u8 = ivm_abi::baseline::mode::ZK | ivm_abi::baseline::mode::VECTOR;
    const VECTOR_LENGTH: u8 = 8;
    let mut program = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: MODE_ZK_VECTOR,
        max_cycles: 1_000,
        abi_version: 1,
    }
    .encode_for_profile(EXECUTOR_FIXTURE_PROFILE, VECTOR_LENGTH);
    for instruction in [
        encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 3, 1, 2),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SUB, 6, 4, 5),
        encoding::wide::encode_rr(instruction::wide::arithmetic::XOR, 9, 7, 8),
        encoding::wide::encode_branch(instruction::wide::control::BEQ, 1, 1, 2),
        encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 15, 15, 0),
        encoding::wide::encode_halt(),
    ] {
        program.extend_from_slice(&instruction.to_le_bytes());
    }
    program
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::IVM;
    const TEST_GAS_LIMIT: u64 = 1_000_000;
    #[test]
    fn default_executor_uses_authenticated_generic_header_and_loads() {
        let program = build_default_executor_program();
        let parsed = ProgramMetadata::parse_for_profile(&program, EXECUTOR_FIXTURE_PROFILE)
            .expect("default executor metadata parses");
        assert_eq!(parsed.header_len, ivm_abi::baseline::HEADER_SIZE);
        assert_eq!(parsed.metadata.max_cycles, DEFAULT_MAX_CYCLES);
        assert_eq!(parsed.metadata.version_minor, 1);
        assert!(parsed.literal_section.is_some());
        assert!(parsed.code_offset > parsed.header_len);
        let mut vm = IVM::new(TEST_GAS_LIMIT);
        vm.load_program_for_profile(&program, EXECUTOR_FIXTURE_PROFILE)
            .expect("default executor passes admission");
        vm.set_register(10, crate::Memory::OUTPUT_START);
        vm.run().expect("default executor runs");
        let expected_verdict: Result<(), ValidationFail> = Ok(());
        let verdict_bytes = expected_verdict.encode();
        let output = vm.read_output_used();
        assert_eq!(output.len() % 8, 0);
        assert_eq!(
            u64::from_le_bytes(
                output[..8]
                    .try_into()
                    .expect("length prefix is eight bytes")
            ),
            RESULT_LENGTH_PREFIX_BYTES
                + u64::try_from(verdict_bytes.len()).expect("bounded verdict length fits u64")
        );
        assert_eq!(&output[8..8 + verdict_bytes.len()], verdict_bytes);
        assert!(
            output[8 + verdict_bytes.len()..]
                .iter()
                .all(|byte| *byte == 0)
        );
    }
    #[test]
    fn default_executor_is_the_checked_in_baseline_artifact() {
        // `defaults/executor.to` is the anchor's executor; its builder reproduces it exactly.
        assert_eq!(
            build_default_executor_program(),
            include_bytes!("../../../defaults/executor.to")
        );
    }
    #[test]
    fn encoded_result_program_uses_fixed_width_authenticated_literals() {
        let result = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99];
        let program = build_encoded_result_program(&result);
        let parsed = ProgramMetadata::parse_for_profile(&program, EXECUTOR_FIXTURE_PROFILE)
            .expect("encoded-result metadata parses");
        let literals = parsed
            .literal_section
            .expect("encoded-result program has typed literals");
        assert!(literals.count > 0);
        for index in 0..literals.count {
            let start = literals.entries_start + index * 8;
            let descriptor = u64::from_le_bytes(
                program[start..start + 8]
                    .try_into()
                    .expect("literal descriptor is eight bytes"),
            );
            let (kind, _) = crate::metadata::decode_literal_descriptor(descriptor)
                .expect("literal descriptor decodes");
            assert_eq!(kind, LiteralKindV1::I64);
        }
        let mut vm = IVM::new(TEST_GAS_LIMIT);
        vm.load_program_for_profile(&program, EXECUTOR_FIXTURE_PROFILE)
            .expect("encoded-result program passes admission");
        vm.set_register(10, crate::Memory::OUTPUT_START);
        vm.run().expect("encoded-result program runs");
        let output = vm.read_output_used();
        assert_eq!(
            u64::from_le_bytes(output[..8].try_into().expect("fixed u64 prefix")),
            RESULT_LENGTH_PREFIX_BYTES + u64::try_from(result.len()).expect("bounded result")
        );
        assert_eq!(&output[8..8 + result.len()], result);
        assert!(output[8 + result.len()..].iter().all(|byte| *byte == 0));
    }
    #[test]
    fn raw_trigger_program_is_the_anchor_trigger_artifact_and_runs() {
        let program = build_raw_trigger_program();
        // The anchor's `artifact_v1_1_mode03_vlen8_cycles1000_abi1.to`, byte for byte.
        let anchor = [
            "49564d0001010308e803000000000000017531b45dd9d79aabc3a0f64ab30a05",
            "3af847dfe248078cff839ab2e14e3261a7020103010504060208070905020101",
            "40000f0f0100000049",
        ]
        .concat();
        assert_eq!(hex::encode(&program), anchor);
        let parsed = ProgramMetadata::parse_for_profile(&program, EXECUTOR_FIXTURE_PROFILE)
            .expect("trigger metadata parses under the interface profile");
        assert_eq!(parsed.metadata.max_cycles, 1_000);
        let mut vm = IVM::new(TEST_GAS_LIMIT);
        vm.load_program_for_profile(&program, EXECUTOR_FIXTURE_PROFILE)
            .expect("trigger program loads");
        vm.run().expect("trigger program runs to HALT");
    }
}
