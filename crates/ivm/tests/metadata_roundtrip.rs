use ivm::{METADATA_MAGIC, ProgramMetadata, VMError};

#[test]
fn metadata_encode_parse_roundtrip() {
    let cases = vec![
        ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0x03,
            vector_length: 0,
            max_cycles: 12345,
            abi_version: 1,
        },
        ProgramMetadata {
            version_major: 1,
            version_minor: 1,
            mode: 0x01,
            vector_length: 8,
            max_cycles: u32::MAX as u64 + 1,
            abi_version: 1,
        },
    ];
    for m in cases {
        let bytes = m.encode();
        // magic prefix
        assert_eq!(&bytes[0..4], &METADATA_MAGIC[..]);
        let parsed = ProgramMetadata::parse(&bytes).expect("parse ok");
        assert_eq!(parsed.header_len, bytes.len());
        assert_eq!(parsed.metadata.version_major, m.version_major);
        assert_eq!(parsed.metadata.version_minor, m.version_minor);
        assert_eq!(parsed.metadata.mode, m.mode);
        assert_eq!(parsed.metadata.vector_length, m.vector_length);
        assert_eq!(parsed.metadata.max_cycles, m.max_cycles);
        assert_eq!(parsed.metadata.abi_version, m.abi_version);
        // Parse again from a buffer with padding; parser should ignore trailing bytes
        let mut padded = bytes.clone();
        padded.extend_from_slice(&[0u8; 16]);
        let parsed2 = ProgramMetadata::parse(&padded).expect("parse ok");
        assert_eq!(parsed2.header_len, bytes.len());
        assert_eq!(parsed2.metadata.max_cycles, m.max_cycles);
    }
}

#[test]
fn metadata_parse_rejects_bad_magic_and_short() {
    // Too short
    let short = vec![0u8; 8];
    assert!(matches!(
        ProgramMetadata::parse(&short),
        Err(VMError::InvalidMetadata)
    ));

    // Wrong magic
    let mut m = ProgramMetadata::default().encode();
    let mut bad = m.clone();
    bad[0] = b'X';
    assert!(matches!(
        ProgramMetadata::parse(&bad),
        Err(VMError::InvalidMetadata)
    ));

    // Corrupt only a header byte to ensure parser still consumes fixed size and returns values
    m[7] ^= 0xFF; // vector_length flipped
    let parsed = ProgramMetadata::parse(&m).expect("parse ok despite flip");
    assert_eq!(parsed.header_len, 17);
    // Value should reflect the flip
    assert_ne!(
        parsed.metadata.vector_length,
        ProgramMetadata::default().vector_length
    );
}
