use norito::{crc64_fallback, hardware_crc64};

#[test]
fn hardware_matches_fallback_empty() {
    let data = b"";
    assert_eq!(hardware_crc64(data), crc64_fallback(data));
}

#[test]
fn hardware_matches_fallback_known_vector() {
    let data = b"123456789";
    assert_eq!(hardware_crc64(data), crc64_fallback(data));
}
