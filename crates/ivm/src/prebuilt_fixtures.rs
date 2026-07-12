//! Canonical builders for checked-in and staged IVM executor fixtures.
//!
//! Keeping these builders in the `ivm` library gives every exporter and test
//! one host-independent implementation. In particular, serialized length
//! prefixes are fixed-width `u64` values and never depend on `usize`.

use iroha_data_model::ValidationFail;
use norito::codec::Encode;

use crate::{
    ProgramMetadata, encoding,
    kotodama::wide,
    metadata::{LITERAL_SECTION_MAGIC, LiteralKindV1, encode_literal_descriptor},
};

/// Names and stable discriminator order of the synthetic executor fixtures.
pub const SYNTHETIC_EXECUTOR_FIXTURES: [&str; 8] = [
    "executor_with_admin",
    "executor_with_custom_permission",
    "executor_remove_permission",
    "executor_custom_instructions_simple",
    "executor_custom_instructions_complex",
    "executor_with_migration_fail",
    "executor_with_fuel",
    "executor_with_custom_parameter",
];

const DEFAULT_MAX_CYCLES: u64 = 1_000_000;
const WIDE_IMM_MIN: i32 = -128;
const WIDE_IMM_MAX: i32 = 127;
const RESULT_LENGTH_PREFIX_BYTES: u64 = 8;

fn chunk_immediate(value: i32) -> i8 {
    value.clamp(WIDE_IMM_MIN, WIDE_IMM_MAX) as i8
}

fn emit_addi_inplace(code: &mut Vec<u32>, register: u8, mut value: i32) {
    while value != 0 {
        let chunk = chunk_immediate(value);
        code.push(wide::encode_addi(register, register, chunk));
        value -= i32::from(chunk);
    }
}

fn set_register(code: &mut Vec<u32>, register: u8, value: i32) {
    code.push(wide::encode_move(register, 0));
    emit_addi_inplace(code, register, value);
}

fn build_copy_program(data_offset: i32, chunks: usize) -> Vec<u32> {
    const SOURCE_POINTER: u8 = 12;
    const DESTINATION_POINTER: u8 = 13;
    const TEMPORARY: u8 = 14;

    let mut code = Vec::new();
    set_register(&mut code, SOURCE_POINTER, data_offset);
    code.push(wide::encode_move(DESTINATION_POINTER, 10));

    for _ in 0..chunks {
        code.push(wide::encode_load64(SOURCE_POINTER, TEMPORARY, 0));
        code.push(wide::encode_store64(DESTINATION_POINTER, TEMPORARY, 0));
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
    let verdict_bytes = verdict.encode();
    let verdict_len = u64::try_from(verdict_bytes.len()).expect("bounded verdict length fits u64");
    let total_len = RESULT_LENGTH_PREFIX_BYTES
        .checked_add(verdict_len)
        .expect("bounded verdict length addition cannot overflow");

    let mut data = Vec::new();
    data.extend_from_slice(&total_len.to_le_bytes());
    data.extend_from_slice(&verdict_bytes);
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
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: DEFAULT_MAX_CYCLES,
        abi_version: 1,
    }
    .encode();
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

/// Build one deterministic synthetic executor fixture.
///
/// `tag` is stored as `vector_length = tag + 1`; integration tests use this
/// stable discriminator to select fixture-specific behavior. Only tags in the
/// [`SYNTHETIC_EXECUTOR_FIXTURES`] range are accepted.
///
/// # Panics
///
/// Panics when `tag` does not identify a declared synthetic fixture.
#[must_use]
pub fn build_synthetic_executor_program(tag: u8) -> Vec<u8> {
    assert!(
        usize::from(tag) < SYNTHETIC_EXECUTOR_FIXTURES.len(),
        "synthetic executor tag is outside the declared fixture inventory"
    );
    let mut program = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: tag + 1,
        max_cycles: DEFAULT_MAX_CYCLES,
        abi_version: 1,
    }
    .encode();
    program.extend_from_slice(&encoding::wide::encode_halt().to_le_bytes());
    program
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::IVM;

    #[test]
    fn default_executor_uses_authenticated_generic_header_and_loads() {
        let program = build_default_executor_program();
        let parsed = ProgramMetadata::parse(&program).expect("default executor metadata parses");
        assert_eq!(parsed.header_len, crate::HEADER_SIZE);
        assert_eq!(parsed.metadata.version_minor, 0);
        assert!(parsed.literal_section.is_some());
        assert!(parsed.code_offset > parsed.header_len);
        let mut vm = IVM::new(DEFAULT_MAX_CYCLES);
        vm.load_program(&program)
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
    fn synthetic_executor_inventory_has_unique_loadable_discriminators() {
        let mut programs = std::collections::BTreeSet::new();
        for (tag, name) in SYNTHETIC_EXECUTOR_FIXTURES.iter().enumerate() {
            assert!(!name.is_empty());
            let program = build_synthetic_executor_program(
                u8::try_from(tag).expect("fixture inventory length fits u8"),
            );
            let parsed = ProgramMetadata::parse(&program).expect("fixture metadata parses");
            assert_eq!(parsed.metadata.vector_length, tag as u8 + 1);
            IVM::new(DEFAULT_MAX_CYCLES)
                .load_program(&program)
                .expect("synthetic fixture passes admission");
            assert!(programs.insert(program), "fixture programs must be unique");
        }
    }
}
