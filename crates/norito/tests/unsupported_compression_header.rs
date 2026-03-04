//! `from_bytes` rejects payloads with non-None compression via UnsupportedCompression.

use byteorder::{ByteOrder, LittleEndian};

#[test]
fn from_bytes_rejects_non_none_compression() {
    // Build a minimal header with Compression::Zstd (1) and zero-length payload
    let mut bytes = vec![0u8; norito::core::Header::SIZE];
    // Magic
    bytes[0..4].copy_from_slice(b"NRT0");
    bytes[4] = norito::core::VERSION_MAJOR;
    bytes[5] = norito::core::VERSION_MINOR;
    // Schema: type u8
    let schema = <u8 as norito::NoritoSerialize>::schema_hash();
    bytes[6..22].copy_from_slice(&schema);
    // Compression = 1 (Zstd)
    bytes[22] = 1u8;
    // Length = 0
    LittleEndian::write_u64(&mut bytes[23..31], 0);
    // Checksum over empty payload = 0
    LittleEndian::write_u64(&mut bytes[31..39], 0);
    // Flags = 0
    bytes[39] = 0;

    let res = norito::core::from_bytes::<u8>(&bytes);
    assert!(matches!(
        res,
        Err(norito::Error::UnsupportedCompression { found: 1, .. })
    ));
}
