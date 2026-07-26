//! Validate CRC64 agreement between encoder and streaming decoder paths.

use norito::{
    NoritoDeserialize, NoritoSerialize, core as norito_core, crc64_fallback, decode_from_bytes,
    hardware_crc64, to_bytes,
};

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize)]
struct Sample {
    a: u64,
    b: String,
}

#[test]
fn decode_from_bytes_accepts_hardware_checksum() {
    let value = Sample {
        a: 7,
        b: "bench crc".to_owned(),
    };
    let bytes = to_bytes(&value).expect("encode with header");

    let view = norito_core::from_bytes_view(&bytes).expect("core header validation");
    let payload = view.as_bytes();

    assert_eq!(
        hardware_crc64(payload),
        crc64_fallback(payload),
        "CRC64 implementations must match for payload verification"
    );

    let decoded: Sample = decode_from_bytes(&bytes).expect("stream decode");
    assert_eq!(decoded, value);
}
