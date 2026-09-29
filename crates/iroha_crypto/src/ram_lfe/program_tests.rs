//! Canonical tape ownership, admission, shared clones and real-buffer cleanup.

use super::*;
use crate::default_bfv_programmed_hidden_program;

fn build(instructions: &[HiddenRamFheInstruction]) -> Result<HiddenRamFheProgram, RamLfeError> {
    let mut builder = HiddenRamFheProgram::builder()?;
    for &instruction in instructions {
        builder.push(instruction)?;
    }
    builder.finish()
}

fn frame(version: u8, registers: u16, lanes: u16, tape: &[u8]) -> Vec<u8> {
    use norito::codec::Encode;
    let payload = ProgramEncoding {
        version,
        register_count: registers,
        memory_lane_count: lanes,
        tape,
    }
    .encode();
    norito::core::frame_bare_with_header_flags::<HiddenRamFheProgram>(
        &payload,
        norito::core::default_encode_flags(),
    )
    .unwrap()
}

fn slot(instruction: HiddenRamFheInstruction) -> Vec<u8> {
    instruction_fields(instruction)
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect()
}

#[test]
fn all_instruction_slots_roundtrip_and_reject_every_unused_word() {
    use HiddenRamFheInstruction::*;
    for instruction in [
        LoadInput(0, 1),
        LoadState(0, 1),
        StoreState(0, 1),
        LoadConst(0, 17),
        Add(0, 1, 2),
        AddPlain(0, 1, 17),
        SubPlain(0, 1, 17),
        MulPlain(0, 1, 17),
        Mul(0, 1, 2),
        SelectEqZero(0, 1, 2, 3),
        Output(0),
    ] {
        let program = build(&[instruction, Output(0)]).unwrap();
        let encoded = program.to_bytes().unwrap();
        let decoded = HiddenRamFheProgram::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, program);
        assert_eq!(decoded.instruction(0), Some(instruction));
        assert!(decoded.instruction(usize::MAX).is_none());
        let mut tape = program.0.tape.initialized().to_vec();
        let used = match instruction {
            SelectEqZero(..) => 5,
            Add(..) | AddPlain(..) | SubPlain(..) | MulPlain(..) | Mul(..) => 4,
            Output(..) => 2,
            _ => 3,
        };
        for index in used..WORDS_PER_INSTRUCTION {
            tape[index * 8] = 1;
            assert!(HiddenRamFheProgram::from_bytes(&frame(1, 4, 32, &tape)).is_err());
            tape[index * 8] = 0;
        }
    }
}

#[test]
fn maximum_tape_has_one_bounded_frame_and_canonical_layout() {
    let program = default_bfv_programmed_hidden_program();
    let encoded = program.to_bytes().unwrap();
    assert_eq!(program.instruction_count(), 256);
    assert_eq!(encoded.len(), RAM_LFE_HIDDEN_PROGRAM_MAX_BYTES);
    assert_eq!(program.0.tape.bytes.len(), MAX_TAPE_BYTES);
    // Independent scalar tree/XOF implementation, checked against all 105
    // BLAKE3 1.8.5 upstream mode/vector controls, fixes this complete tape digest.
    assert_eq!(
        program.digest().unwrap().to_string(),
        "6f4a296ecb6fb66f60c2c696369429de4b08ed1df062ecea706f15b6447d7d0b"
    );
    assert_eq!(HiddenRamFheProgram::from_bytes(&encoded).unwrap(), program);
    let _guard = norito::core::DecodeFlagsGuard::enter(0);
    assert_eq!(program.to_bytes().unwrap(), encoded);
    let alternate = norito::to_bytes(&program).unwrap();
    assert_ne!(&*encoded, &alternate);
    assert!(HiddenRamFheProgram::from_bytes(&alternate).is_err());
}

#[test]
fn sole_decoder_rejects_old_identity_malformed_geometry_tags_and_metadata() {
    use HiddenRamFheInstruction::*;
    let program = build(&[LoadConst(0, 19), Output(0)]).unwrap();
    let encoded = program.to_bytes().unwrap();
    let mut old = encoded.to_vec();
    old[6..22].copy_from_slice(&norito::core::schema_hash_for_name(
        "iroha_crypto::ram_lfe::HiddenRamFheProgram",
    ));
    assert!(HiddenRamFheProgram::from_bytes(&old).is_err());
    for (version, registers, lanes) in [(2, 4, 32), (1, 3, 32), (1, 4, 31)] {
        assert!(
            HiddenRamFheProgram::from_bytes(&frame(
                version,
                registers,
                lanes,
                program.0.tape.initialized()
            ))
            .is_err()
        );
    }
    for length in [0, 47, 49, MAX_TAPE_BYTES + 48] {
        assert!(HiddenRamFheProgram::from_bytes(&frame(1, 4, 32, &vec![0; length])).is_err());
    }
    for (word, value) in [(0, 11_u64), (1, u64::MAX), (1, 4), (2, 257)] {
        let mut tape = program.0.tape.initialized().to_vec();
        tape[word * 8..(word + 1) * 8].copy_from_slice(&value.to_le_bytes());
        assert!(HiddenRamFheProgram::from_bytes(&frame(1, 4, 32, &tape)).is_err());
    }
    for boundary in 0..encoded.len() {
        assert!(HiddenRamFheProgram::from_bytes(&encoded[..boundary]).is_err());
    }
    let mut trailing = encoded.to_vec();
    trailing.push(0);
    assert!(HiddenRamFheProgram::from_bytes(&trailing).is_err());
}

#[test]
fn frame_integrity_and_reserved_fields_fail_before_private_allocation() {
    use HiddenRamFheInstruction::*;
    let program = build(&[LoadConst(0, 205), Output(0)]).unwrap();
    let encoded = program.to_bytes().unwrap();
    CLEARED.with_borrow_mut(|count| *count = Some(0));
    // Magic, major/minor, schema, compression, length, checksum and flags.
    for offset in [0, 4, 5, 6, 22, 23, 31, 39] {
        let mut damaged = encoded.to_vec();
        damaged[offset] ^= 0x80;
        assert!(HiddenRamFheProgram::from_bytes(&damaged).is_err());
    }
    let mut padding = encoded.to_vec();
    padding.insert(norito::core::Header::SIZE, 0);
    assert!(HiddenRamFheProgram::from_bytes(&padding).is_err());
    let mut noncanonical = program.0.tape.initialized().to_vec();
    noncanonical[5 * 8] = 1;
    assert!(HiddenRamFheProgram::from_bytes(&frame(1, 4, 32, &noncanonical)).is_err());
    let overlong = vec![0; RAM_LFE_HIDDEN_PROGRAM_MAX_BYTES + 1];
    assert!(HiddenRamFheProgram::from_bytes(&overlong).is_err());
    assert_eq!(CLEARED.with_borrow_mut(|count| count.take().unwrap()), 0);
}

#[test]
fn explicit_decoder_honors_stricter_outer_field_and_allocation_budgets() {
    use norito::core::{DecodeLimits, with_decode_limits_scope};
    let program = default_bfv_programmed_hidden_program();
    let encoded = program.to_bytes().unwrap();
    CLEARED.with_borrow_mut(|count| *count = Some(0));
    for limits in [
        DecodeLimits::new(usize::MAX, 47, usize::MAX, usize::MAX, 64),
        DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, MAX_TAPE_BYTES - 1, 64),
    ] {
        assert!(
            with_decode_limits_scope(limits, || HiddenRamFheProgram::from_bytes(&encoded)).is_err()
        );
    }
    let text = format!("0x{}", hex::encode(&*encoded));
    let no_allocation = DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
    assert!(
        with_decode_limits_scope(no_allocation, || text.parse::<HiddenRamFheProgram>()).is_err()
    );
    assert_eq!(CLEARED.with_borrow_mut(|count| count.take().unwrap()), 0);
    assert_eq!(HiddenRamFheProgram::from_bytes(&encoded).unwrap(), program);
}

#[test]
fn builder_enforces_indexes_immediates_outputs_depth_and_capacity() {
    use HiddenRamFheInstruction::*;
    for instruction in [
        LoadInput(0, 64),
        LoadState(0, 32),
        StoreState(32, 0),
        LoadConst(4, 1),
        LoadConst(0, 257),
        Add(0, 4, 0),
        Mul(0, 0, 4),
        AddPlain(0, 0, 257),
        SubPlain(0, 0, 257),
        MulPlain(0, 0, 257),
        SelectEqZero(0, 4, 0, 0),
        Output(4),
    ] {
        assert!(build(&[instruction, Output(0)]).is_err());
        let mut tape = slot(instruction);
        tape.extend(slot(Output(0)));
        assert!(HiddenRamFheProgram::from_bytes(&frame(1, 4, 32, &tape)).is_err());
    }
    assert!(build(&[]).is_err());
    assert!(build(&[LoadConst(0, 1)]).is_err());
    assert!(build(&[Output(0); 65]).is_err());
    let mut deep = vec![LoadConst(0, 1)];
    deep.extend([Mul(0, 0, 0); 17]);
    deep.push(Output(0));
    assert!(build(&deep).is_err());
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    assert_eq!(
        format!("{builder:?}"),
        "[REDACTED hidden RAM-FHE program builder]"
    );
    let pointer = builder.tape.bytes.as_ptr();
    for _ in 0..256 {
        builder.push(LoadConst(0, 19)).unwrap();
    }
    assert!(builder.push(Output(0)).is_err());
    assert_eq!(builder.tape.bytes.as_ptr(), pointer);
    assert_eq!(builder.tape.count, 256);
}

#[test]
fn clones_share_and_final_owner_clears_real_private_cells() {
    use HiddenRamFheInstruction::*;
    CLEARED.with_borrow_mut(|count| *count = Some(0));
    let program = build(&[LoadConst(0, 205), Output(0)]).unwrap();
    assert!(program.0.tape.bytes.contains(&205));
    let clone = program.clone();
    assert!(Arc::ptr_eq(&program.0, &clone.0));
    assert_eq!(format!("{program:?}"), "[REDACTED hidden RAM-FHE program]");
    drop(program);
    CLEARED.with_borrow(|count| assert_eq!(*count, Some(0)));
    drop(clone);
    assert_eq!(
        CLEARED.with_borrow_mut(|count| count.take().unwrap()),
        MAX_TAPE_BYTES
    );
}

#[test]
fn incomplete_invalid_and_unwound_tapes_clear_real_cells() {
    use HiddenRamFheInstruction::*;
    CLEARED.with_borrow_mut(|count| *count = Some(0));
    {
        let mut builder = HiddenRamFheProgram::builder().unwrap();
        builder.push(LoadConst(0, 205)).unwrap();
        assert!(builder.tape.bytes.contains(&205));
    }
    assert!(build(&[LoadConst(0, 205)]).is_err());
    let mut invalid = slot(LoadConst(0, 205));
    invalid.extend(slot(Output(4)));
    assert!(HiddenRamFheProgram::from_bytes(&frame(1, 4, 32, &invalid)).is_err());
    assert!(
        std::panic::catch_unwind(|| {
            let mut builder = HiddenRamFheProgram::builder().unwrap();
            builder.push(LoadConst(0, 205)).unwrap();
            panic!("private tape unwind control");
        })
        .is_err()
    );
    assert_eq!(
        CLEARED.with_borrow_mut(|count| count.take().unwrap()),
        4 * MAX_TAPE_BYTES
    );
}

#[cfg(feature = "json")]
#[test]
fn typed_configuration_uses_only_bounded_literal_lowercase_hex() {
    let program = default_bfv_programmed_hidden_program();
    let bytes = program.to_bytes().unwrap();
    let text = format!("0x{}", hex::encode(&*bytes));
    let parsed = text.parse::<HiddenRamFheProgram>().unwrap();
    assert_eq!(parsed, program);
    assert_eq!(
        norito::json::from_str::<HiddenRamFheProgram>(&format!("\"{text}\"")).unwrap(),
        program
    );
    for invalid in [
        String::new(),
        "0x".to_owned(),
        "0xa".to_owned(),
        "0xnot-hex".to_owned(),
        hex::encode(&*bytes),
        format!("0x{}", hex::encode_upper(&*bytes)),
        text.to_uppercase(),
        format!(" {text}"),
        format!("{text}00"),
        format!("0x{}", "aa".repeat(RAM_LFE_HIDDEN_PROGRAM_MAX_BYTES + 1)),
    ] {
        assert!(invalid.parse::<HiddenRamFheProgram>().is_err());
    }
    let escaped = format!("\"\\u0030{}\"", &text[1..]);
    assert!(norito::json::from_str::<HiddenRamFheProgram>(&escaped).is_err());
    for non_string in ["null", "5", "{}"] {
        assert!(norito::json::from_str::<HiddenRamFheProgram>(non_string).is_err());
    }
}

fn program() -> HiddenRamFheProgram {
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    builder
        .push(HiddenRamFheInstruction::LoadInput(0, 0))
        .unwrap();
    builder.push(HiddenRamFheInstruction::Output(0)).unwrap();
    builder.finish().unwrap()
}

#[test]
fn canonical_program_roundtrips_without_exposing_private_instructions() {
    assert_eq!(
        <HiddenRamFheProgram as norito::NoritoSchema>::nominal_name(),
        "iroha_crypto::ram_lfe::HiddenRamFheProgramV1"
    );
    let original = program();
    let bytes = original.to_bytes().unwrap();
    let decoded = HiddenRamFheProgram::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(decoded.version(), 1);
    assert_eq!(decoded.register_count(), BFV_PROGRAM_REGISTER_COUNT_U16);
    assert_eq!(decoded.memory_lane_count(), BFV_PROGRAM_STATE_WIDTH_U16);
    assert_eq!(decoded.instruction_count(), 2);
    assert_eq!(decoded.instructions().len(), 2);
    assert_eq!(
        decoded.instruction(0),
        Some(HiddenRamFheInstruction::LoadInput(0, 0))
    );
    assert_eq!(decoded.instruction(2), None);
    assert_eq!(decoded.instruction(usize::MAX), None);
    assert_eq!(decoded.digest().unwrap(), original.digest().unwrap());
    assert_eq!(format!("{decoded:?}"), "[REDACTED hidden RAM-FHE program]");
    let literal = format!("0x{}", hex::encode(&*bytes));
    assert_eq!(literal.parse::<HiddenRamFheProgram>().unwrap(), original);
    #[cfg(feature = "json")]
    assert_eq!(
        norito::json::from_str::<HiddenRamFheProgram>(&format!("\"{literal}\"")).unwrap(),
        original
    );
}

#[test]
fn decoder_rejects_truncation_extra_bytes_and_wrong_metadata() {
    let bytes = program().to_bytes().unwrap();
    for length in [0, 1, bytes.len() - 1] {
        assert!(HiddenRamFheProgram::from_bytes(&bytes[..length]).is_err());
    }
    let mut extra = bytes.to_vec();
    extra.push(0);
    assert!(HiddenRamFheProgram::from_bytes(&extra).is_err());
    assert!(
        HiddenRamFheProgram::from_bytes(&vec![0; RAM_LFE_HIDDEN_PROGRAM_MAX_BYTES + 1]).is_err()
    );
    for (version, registers, lanes) in [
        (
            2,
            BFV_PROGRAM_REGISTER_COUNT_U16,
            BFV_PROGRAM_STATE_WIDTH_U16,
        ),
        (
            1,
            BFV_PROGRAM_REGISTER_COUNT_U16 + 1,
            BFV_PROGRAM_STATE_WIDTH_U16,
        ),
        (
            1,
            BFV_PROGRAM_REGISTER_COUNT_U16,
            BFV_PROGRAM_STATE_WIDTH_U16 + 1,
        ),
    ] {
        let invalid = from_public_test_parts(
            version,
            registers,
            lanes,
            &program().instructions().collect::<Vec<_>>(),
        );
        assert!(HiddenRamFheProgram::from_bytes(&invalid.to_bytes().unwrap()).is_err());
    }
    for literal in ["", "0x", "00", "0x0", "0xAA", "0xgg"] {
        assert!(literal.parse::<HiddenRamFheProgram>().is_err());
    }
}

#[test]
fn builder_enforces_capacity_and_program_semantics() {
    assert!(HiddenRamFheProgram::builder().unwrap().finish().is_err());
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    builder
        .push(HiddenRamFheInstruction::LoadInput(
            BFV_PROGRAM_REGISTER_COUNT_U16,
            0,
        ))
        .unwrap();
    builder.push(HiddenRamFheInstruction::Output(0)).unwrap();
    assert!(builder.finish().is_err());
    let mut builder = HiddenRamFheProgram::builder().unwrap();
    for _ in 0..BFV_PROGRAM_MAX_INSTRUCTIONS - 1 {
        builder
            .push(HiddenRamFheInstruction::LoadInput(0, 0))
            .unwrap();
    }
    builder.push(HiddenRamFheInstruction::Output(0)).unwrap();
    assert!(builder.push(HiddenRamFheInstruction::Output(0)).is_err());
    assert_eq!(
        builder.finish().unwrap().instruction_count(),
        BFV_PROGRAM_MAX_INSTRUCTIONS
    );
}

#[test]
fn clones_share_storage_and_last_owner_clears_the_whole_allocation() {
    CLEARED.with_borrow_mut(|count| *count = Some(0));
    let original = program();
    let clone = original.clone();
    assert!(Arc::ptr_eq(&original.0, &clone.0));
    drop(original);
    assert_eq!(CLEARED.with_borrow(|count| *count), Some(0));
    drop(clone);
    assert_eq!(CLEARED.with_borrow(|count| *count), Some(MAX_TAPE_BYTES));
    let mut abandoned = HiddenRamFheProgram::builder().unwrap();
    abandoned
        .push(HiddenRamFheInstruction::LoadConst(0, 123))
        .unwrap();
    drop(abandoned);
    assert_eq!(
        CLEARED.with_borrow(|count| *count),
        Some(2 * MAX_TAPE_BYTES)
    );
    let mut invalid = HiddenRamFheProgram::builder().unwrap();
    invalid
        .push(HiddenRamFheInstruction::LoadInput(0, 0))
        .unwrap();
    assert!(invalid.finish().is_err());
    assert_eq!(
        CLEARED.with_borrow(|count| *count),
        Some(3 * MAX_TAPE_BYTES)
    );
    CLEARED.with_borrow_mut(|count| *count = None);
}

#[test]
fn instruction_codec_rejects_unknown_tags_indexes_and_unused_words() {
    use HiddenRamFheInstruction::*;
    for instruction in [
        LoadInput(1, 2),
        LoadState(1, 2),
        StoreState(1, 2),
        LoadConst(1, 2),
        Add(1, 2, 3),
        AddPlain(1, 2, 3),
        SubPlain(1, 2, 3),
        MulPlain(1, 2, 3),
        Mul(1, 2, 3),
        SelectEqZero(1, 2, 3, 4),
        Output(1),
    ] {
        let bytes: Vec<_> = instruction_fields(instruction)
            .into_iter()
            .flat_map(u64::to_le_bytes)
            .collect();
        assert_eq!(decode_instruction(&bytes).unwrap(), instruction);
        let mut noncanonical = bytes;
        noncanonical[BYTES_PER_INSTRUCTION - 1] = 1;
        assert!(decode_instruction(&noncanonical).is_err());
    }
    for words in [
        [11, 0, 0, 0, 0, 0],
        [10, u64::from(u16::MAX) + 1, 0, 0, 0, 0],
    ] {
        let bytes: Vec<_> = words.into_iter().flat_map(u64::to_le_bytes).collect();
        assert!(decode_instruction(&bytes).is_err());
    }
    assert!(decode_instruction(&[0; BYTES_PER_INSTRUCTION - 1]).is_err());
}
