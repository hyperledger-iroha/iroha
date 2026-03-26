//! Tests for IVM bytecode header (ProgramMetadata) validation.

use ivm::{ProgramMetadata, VMError, ivm_mode};

fn encode_with(mut meta: ProgramMetadata, f: impl FnOnce(&mut ProgramMetadata)) -> Vec<u8> {
    f(&mut meta);
    meta.encode()
}

fn minimal_contract_artifact() -> Vec<u8> {
    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        compiler_fingerprint: "metadata-tests".to_owned(),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "main".to_owned(),
            kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Public,
            params: Vec::new(),
            return_type: None,
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
    };
    let mut bytes = meta.encode();
    bytes.extend_from_slice(&interface.encode_section());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    bytes
}

fn minimal_contract_artifact_with_debug() -> Vec<u8> {
    let meta = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let interface = ivm::EmbeddedContractInterfaceV1 {
        compiler_fingerprint: "metadata-tests".to_owned(),
        features_bitmap: 0,
        access_set_hints: None,
        kotoba: Vec::new(),
        entrypoints: vec![ivm::EmbeddedEntrypointDescriptor {
            name: "main".to_owned(),
            kind: iroha_data_model::smart_contract::manifest::EntryPointKind::Public,
            params: Vec::new(),
            return_type: None,
            permission: None,
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        }],
    };
    let debug = ivm::EmbeddedContractDebugInfoV1 {
        source_map: vec![ivm::EmbeddedSourceMapEntryV1 {
            function_name: "main".to_owned(),
            pc_start: 0,
            pc_end: 4,
            source: ivm::EmbeddedSourceLocation { line: 2, column: 3 },
        }],
        budget_report: vec![ivm::EmbeddedFunctionBudgetReportV1 {
            function_name: "main".to_owned(),
            pc_start: 0,
            pc_end: 4,
            bytecode_bytes: 4,
            bytecode_words: 1,
            frame_bytes: 16,
            jump_span_words: 1,
            jump_range_risk: false,
            source: Some(ivm::EmbeddedSourceLocation { line: 2, column: 3 }),
        }],
    };
    let mut bytes = meta.encode();
    bytes.extend_from_slice(&interface.encode_section());
    bytes.extend_from_slice(&debug.encode_section());
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    bytes
}

#[test]
fn parse_accepts_valid_default_header() {
    let meta = ProgramMetadata::default();
    let bytes = meta.encode();
    let parsed = ProgramMetadata::parse(&bytes).expect("parse valid header");
    assert_eq!(parsed.code_offset, ivm::METADATA_MAGIC.len() + 13); // 17 total
    assert_eq!(parsed.metadata.version_major, 1);
    assert_eq!(parsed.metadata.version_minor, 1);
    assert_eq!(parsed.metadata.mode, 0);
    assert_eq!(parsed.metadata.vector_length, 0);
    // Default ABI version is 1 for the first release. Parser carries the value as-is.
    assert_eq!(
        parsed.metadata.abi_version,
        ProgramMetadata::default().abi_version
    );
}

#[test]
fn parse_rejects_unknown_mode_bits() {
    let bytes = encode_with(ProgramMetadata::default(), |m| {
        m.mode = 0x80; // unknown bit
    });
    let err = ProgramMetadata::parse(&bytes).unwrap_err();
    assert_eq!(err, VMError::InvalidMetadata);
}

#[test]
fn parse_accepts_legacy_minor_zero_without_cntr() {
    let bytes = encode_with(ProgramMetadata::default(), |m| {
        m.version_minor = 0;
    });
    let parsed = ProgramMetadata::parse(&bytes).expect("parse legacy generic header");
    assert_eq!(parsed.metadata.version_major, 1);
    assert_eq!(parsed.metadata.version_minor, 0);
    assert!(parsed.contract_interface.is_none());
    assert_eq!(parsed.code_offset, parsed.header_len);
}

#[test]
fn parse_accepts_vector_len_without_vector_flag() {
    // Vector length is advisory and may be present even if VECTOR bit is off.
    let bytes = encode_with(ProgramMetadata::default(), |m| {
        m.mode = 0; // VECTOR not set
        m.vector_length = 8;
    });
    let parsed = ProgramMetadata::parse(&bytes).expect("parse ok without VECTOR flag");
    assert_eq!(parsed.metadata.mode & ivm_mode::VECTOR, 0);
    assert_eq!(parsed.metadata.vector_length, 8);
}

#[test]
fn parse_accepts_vector_len_with_flag() {
    let bytes = encode_with(ProgramMetadata::default(), |m| {
        m.mode = ivm_mode::VECTOR;
        m.vector_length = 8;
    });
    let parsed = ProgramMetadata::parse(&bytes).expect("parse vector header");
    assert_eq!(parsed.metadata.mode & ivm_mode::VECTOR, ivm_mode::VECTOR);
    assert_eq!(parsed.metadata.vector_length, 8);
}

#[test]
fn parse_rejects_wrong_major_version() {
    let bytes = encode_with(ProgramMetadata::default(), |m| {
        m.version_major = 0;
    });
    let err = ProgramMetadata::parse(&bytes).unwrap_err();
    assert_eq!(err, VMError::InvalidMetadata);

    let bytes = encode_with(ProgramMetadata::default(), |m| {
        m.version_major = 2;
    });
    let err = ProgramMetadata::parse(&bytes).unwrap_err();
    assert_eq!(err, VMError::InvalidMetadata);
}

#[test]
fn parse_accepts_contract_minor_one_with_cntr() {
    let bytes = minimal_contract_artifact();
    let parsed = ProgramMetadata::parse(&bytes).expect("parse contract artifact");
    assert_eq!(parsed.metadata.version_minor, 1);
    assert!(
        parsed.contract_interface.is_some(),
        "CNTR section must decode"
    );
    assert!(
        parsed.code_offset > parsed.header_len,
        "CNTR prefix must advance code offset"
    );
}

#[test]
fn parse_accepts_contract_debug_section() {
    let bytes = minimal_contract_artifact_with_debug();
    let parsed = ProgramMetadata::parse(&bytes).expect("parse contract artifact with debug");
    let debug = parsed.contract_debug.expect("DBG1 section must decode");
    assert_eq!(debug.source_map.len(), 1);
    assert_eq!(debug.source_map[0].function_name, "main");
    assert_eq!(debug.source_map[0].source.line, 2);
    assert_eq!(debug.budget_report.len(), 1);
    assert_eq!(debug.budget_report[0].frame_bytes, 16);
}

#[test]
fn parse_accepts_generic_minor_one_without_cntr() {
    let mut bytes = ProgramMetadata::default().encode();
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let parsed = ProgramMetadata::parse(&bytes).expect("generic 1.1 artifact must parse");
    assert!(parsed.contract_interface.is_none());
    assert_eq!(parsed.code_offset, parsed.header_len);
}

#[test]
fn parse_rejects_unknown_minor_version() {
    let bytes = encode_with(ProgramMetadata::default(), |m| {
        m.version_minor = 2;
    });
    let err = ProgramMetadata::parse(&bytes).unwrap_err();
    assert_eq!(err, VMError::InvalidMetadata);
}

#[test]
fn load_program_honors_zk_mode_bit() {
    let mut bytes = ProgramMetadata {
        mode: ivm_mode::ZK,
        abi_version: 1,
        ..ProgramMetadata::default()
    }
    .encode();
    bytes.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let mut vm = ivm::IVM::new(u64::MAX);
    assert!(!vm.zk_mode_enabled());
    vm.load_program(&bytes).expect("load zk program");
    assert!(vm.zk_mode_enabled());
}
