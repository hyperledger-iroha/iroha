//! Program metadata parsing tests for literal sections.

use ivm::ProgramMetadata;

const LITERAL_SECTION_MAGIC: [u8; 4] = *b"LTLB";

fn build_header(
    version_major: u8,
    mode: u8,
    vector_length: u8,
    max_cycles: u64,
    abi_version: u8,
) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"IVM\0");
    v.push(version_major);
    v.push(1); // version_minor
    v.push(mode);
    v.push(vector_length);
    v.extend_from_slice(&max_cycles.to_le_bytes());
    v.push(abi_version);
    v
}

#[test]
fn parse_accepts_version_1_and_abi_v1() {
    let hdr = build_header(1, 0, 0, 0, 1);
    let bytes = [hdr.as_slice(), &[1, 2, 3, 4]].concat();
    let parsed = ProgramMetadata::parse(&bytes).expect("parse ok");

    let meta = parsed.metadata;

    let off = parsed.code_offset;
    assert_eq!(meta.version_major, 1);
    assert_eq!(meta.version_minor, 1);
    assert_eq!(meta.mode, 0);
    assert_eq!(meta.vector_length, 0);
    assert_eq!(meta.max_cycles, 0);
    assert_eq!(meta.abi_version, 1);
    assert_eq!(off, 17);
}

#[test]
fn parse_rejects_unknown_major_and_mode_bits() {
    // Unsupported majors
    let mut hdr = build_header(0, 0, 0, 0, 1);
    let bytes = [hdr.as_slice(), &[0u8; 4]].concat();
    assert!(ProgramMetadata::parse(&bytes).is_err());
    // Unknown major
    hdr = build_header(2, 0, 0, 0, 1);
    let bytes = [hdr.as_slice(), &[0u8; 4]].concat();
    assert!(ProgramMetadata::parse(&bytes).is_err());
    // Unknown mode bit (0x80) should be rejected
    hdr = build_header(1, 0x80, 0, 0, 1);
    let bytes2 = [hdr.as_slice(), &[0u8; 4]].concat();
    assert!(ProgramMetadata::parse(&bytes2).is_err());
}

#[test]
fn parse_rejects_short_or_bad_magic() {
    // Too short
    assert!(ProgramMetadata::parse(&[0u8; 8]).is_err());
    // Bad magic
    let mut bad = build_header(1, 0, 0, 0, 1);
    bad[0..4].copy_from_slice(b"BAD\0");
    let bytes = [bad.as_slice(), &[0u8; 4]].concat();
    assert!(ProgramMetadata::parse(&bytes).is_err());
}

#[test]
fn parse_skips_literal_section_when_present() {
    let mut bytes = build_header(1, 0, 0, 0, 1);
    // Literal table header: magic + count (2 entries)
    bytes.extend_from_slice(&LITERAL_SECTION_MAGIC);
    bytes.extend_from_slice(&(2u32).to_le_bytes());
    bytes.extend_from_slice(&(1u32).to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    // Two dummy literal entries (16 bytes)
    bytes.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    bytes.extend_from_slice(&0x99AA_BBCC_DDEE_F010u64.to_le_bytes());
    bytes.push(0); // post-pad to keep code offset aligned
    // Append a couple of code bytes so the parser sees a non-empty code region
    bytes.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
    let parsed = ProgramMetadata::parse(&bytes).expect("parse ok with literals");

    let meta = parsed.metadata;

    let off = parsed.code_offset;
    assert_eq!(meta.version_major, 1);
    assert_eq!(off, 17 + 16 + 16 + 1);
    assert!(off < bytes.len());
}

#[test]
fn parse_rejects_literal_padding() {
    let mut bytes = build_header(1, 0, 0, 0, 1);
    bytes.extend_from_slice(&[0u8; 4]);
    bytes.extend_from_slice(&LITERAL_SECTION_MAGIC);
    bytes.extend_from_slice(&(1u32).to_le_bytes());
    bytes.extend_from_slice(&(0u32).to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    bytes.extend_from_slice(&[0xAA, 0xBB]);
    assert!(ProgramMetadata::parse(&bytes).is_err());
}
