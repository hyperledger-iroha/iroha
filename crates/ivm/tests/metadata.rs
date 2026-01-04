//! Tests for IVM bytecode header (ProgramMetadata) validation.

use ivm::{ProgramMetadata, VMError, ivm_mode};

fn encode_with(mut meta: ProgramMetadata, f: impl FnOnce(&mut ProgramMetadata)) -> Vec<u8> {
    f(&mut meta);
    meta.encode()
}

#[test]
fn parse_accepts_valid_default_header() {
    let meta = ProgramMetadata::default();
    let bytes = meta.encode();
    let parsed = ProgramMetadata::parse(&bytes).expect("parse valid header");
    assert_eq!(parsed.code_offset, ivm::METADATA_MAGIC.len() + 13); // 17 total
    assert_eq!(parsed.metadata.version_major, 2);
    assert_eq!(parsed.metadata.version_minor, 0);
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
fn parse_accepts_future_abi_versions() {
    // Parser should accept unknown/forward ABI versions; hosts map them to policy.
    let bytes = encode_with(ProgramMetadata::default(), |m| {
        m.abi_version = 2;
    });
    let parsed = ProgramMetadata::parse(&bytes).expect("parse ok for future abi_version");
    assert_eq!(parsed.code_offset, ivm::METADATA_MAGIC.len() + 13);
    assert_eq!(parsed.metadata.abi_version, 2);
}

#[test]
fn parse_rejects_wrong_major_version() {
    let bytes = encode_with(ProgramMetadata::default(), |m| {
        m.version_major = 3;
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
