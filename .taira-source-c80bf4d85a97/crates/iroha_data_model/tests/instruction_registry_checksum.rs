//! Ensure instruction framing used by the registry preserves Norito headers.
use iroha_data_model::{
    isi::{Instruction, InstructionRegistry},
    prelude::{Encode, Level, Log},
};
use norito::decode_from_bytes;

#[test]
fn frame_helper_produces_valid_header() {
    let instruction = Log::new(Level::INFO, "bench".to_owned());
    let type_name = Instruction::id(&instruction);

    // Encode without a Norito header, then frame with the helper used by the registry.
    let payload = instruction.encode();
    let framed = norito::core::frame_bare_with_header_flags::<Log>(
        &payload,
        norito::core::default_encode_flags(),
    )
    .expect("frame payload");

    // Header checksum should match the payload bytes.
    let header_size = norito::core::Header::SIZE;
    assert!(
        framed.len() >= header_size,
        "framed bytes must include Norito header"
    );
    assert_eq!(&framed[..4], b"NRT0");
    let len_offset = 4 + 1 + 1 + 16 + 1;
    let header_len = u64::from_le_bytes(
        framed[len_offset..len_offset + 8]
            .try_into()
            .expect("payload length bytes"),
    );
    let checksum_offset = len_offset + 8;
    let header_checksum = u64::from_le_bytes(
        framed[checksum_offset..checksum_offset + 8]
            .try_into()
            .expect("checksum bytes"),
    );
    let payload_bytes = {
        let slice = &framed[header_size..];
        let payload_len = usize::try_from(header_len).expect("payload length fits in usize");
        let padding = slice.len().saturating_sub(payload_len);
        &slice[padding..]
    };
    assert_eq!(
        header_checksum,
        norito::crc64_fallback(payload_bytes),
        "header checksum must match payload"
    );

    // And the registry should accept the framed bytes.
    let registry = InstructionRegistry::new().register::<Log>();
    let decoded = registry
        .decode(type_name, &framed)
        .expect("registered")
        .expect("decode framed payload");
    assert_eq!(Instruction::id(&*decoded), type_name);

    // Direct Norito decode should also succeed.
    let _ = decode_from_bytes::<Log>(&framed).expect("decode framed payload directly");
}
