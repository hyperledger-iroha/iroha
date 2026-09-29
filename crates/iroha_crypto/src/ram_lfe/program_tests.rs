//! Canonical private-program decoding, builder bounds and clearing ownership.

use super::*;

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
