//! Predecoder golden vectors for the wide 32-bit encoding. These assertions
//! ensure the cached decode stream preserves instruction words and lengths
//! across metadata variations.

use ivm::{ProgramMetadata, VMError, encoding, instruction, ivm_cache::IvmCache};

fn push_word(buf: &mut Vec<u8>, word: u32) {
    buf.extend_from_slice(&word.to_le_bytes());
}

fn build_wide_code() -> Vec<u8> {
    let mut code = Vec::new();
    push_word(
        &mut code,
        encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 1, 2, 3),
    );
    push_word(
        &mut code,
        encoding::wide::encode_rr(instruction::wide::arithmetic::SUB, 4, 5, 6),
    );
    push_word(
        &mut code,
        encoding::wide::encode_rr(instruction::wide::arithmetic::XOR, 7, 8, 9),
    );
    push_word(
        &mut code,
        encoding::wide::encode_branch(instruction::wide::control::BEQ, 1, 4, 1),
    );
    // Filler that the branch would skip at runtime.
    push_word(
        &mut code,
        encoding::wide::encode_rr(instruction::wide::arithmetic::AND, 10, 10, 10),
    );
    push_word(&mut code, encoding::wide::encode_halt());
    code
}

#[test]
fn decode_stream_matches_expected_words_and_lengths() {
    let code = build_wide_code();
    let decoded = IvmCache::decode_stream(&code).expect("decode ok");

    assert_eq!(decoded.len(), 6);
    let expected = [
        encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 1, 2, 3),
        encoding::wide::encode_rr(instruction::wide::arithmetic::SUB, 4, 5, 6),
        encoding::wide::encode_rr(instruction::wide::arithmetic::XOR, 7, 8, 9),
        encoding::wide::encode_branch(instruction::wide::control::BEQ, 1, 4, 1),
        encoding::wide::encode_rr(instruction::wide::arithmetic::AND, 10, 10, 10),
        encoding::wide::encode_halt(),
    ];

    for (idx, op) in decoded.iter().enumerate() {
        assert_eq!(op.len, 4, "opcode {idx} should be 4 bytes");
        assert_eq!(op.inst, expected[idx], "opcode {idx} word mismatch");
    }
}

/// Decoding should ignore metadata fields that are orthogonal to the byte stream.
#[test]
fn decode_artifact_invariant_across_metadata_fields() {
    let code = build_wide_code();

    // Baseline header
    let base = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };

    let decode = |m: &ProgramMetadata| {
        let mut a = m.encode();
        a.extend_from_slice(&code);
        let (_meta, d) = IvmCache::decode_artifact(&a).expect("artifact decode ok");
        d
    };

    let golden = decode(&base);

    for mode in 0u8..=0x07 {
        let mut m = base.clone();
        m.mode = mode;
        assert_eq!(&*golden, &*decode(&m), "mode 0x{mode:02x}");
    }

    for vlen in [0u8, 1, 4, 8, 16, 32, 64, 255] {
        let mut m = base.clone();
        m.vector_length = vlen;
        assert_eq!(&*golden, &*decode(&m), "vlen {vlen}");
    }

    for cyc in [0u64, 1, 10, 1_000, u32::MAX as u64, u64::from(u32::MAX) + 1] {
        let mut m = base.clone();
        m.max_cycles = cyc;
        assert_eq!(&*golden, &*decode(&m), "cycles {cyc}");
    }

    let mut m = base.clone();
    m.abi_version = 1;
    assert_eq!(&*golden, &*decode(&m), "abi 1");
}

#[test]
fn decode_artifact_rejects_legacy_or_unknown_minor_version() {
    let code = build_wide_code();
    let base = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };

    for vmin in [0u8, 2, 7, 42] {
        let mut m = base.clone();
        m.version_minor = vmin;
        let mut artifact = m.encode();
        artifact.extend_from_slice(&code);
        let err = IvmCache::decode_artifact(&artifact).expect_err("invalid minor should reject");
        assert_eq!(err, VMError::InvalidMetadata, "minor {vmin}");
    }
}
