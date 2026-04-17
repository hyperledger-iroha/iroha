//! Regression coverage for the documented schema sample payload.

use norito::{
    core::{Header, header_flags},
    schema::SamplePayload,
};

#[test]
fn sample_payload_matches_documented_compact_wire_layout() {
    let payload = SamplePayload {
        version: 7,
        enabled: true,
        label: "demo".into(),
        items: vec![1, 2, 3],
    };

    let encoded = norito::to_bytes(&payload).expect("encode sample payload");
    let (header, body) = encoded.split_at(Header::SIZE);

    assert_eq!(header[Header::SIZE - 1], header_flags::COMPACT_LEN);
    assert_eq!(
        body,
        &[
            0x04, 0x07, 0x00, 0x00, 0x00, 0x01, 0x01, 0x05, 0x04, 0x64, 0x65, 0x6D, 0x6F, 0x17,
            0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x01, 0x00, 0x00, 0x00, 0x04,
            0x02, 0x00, 0x00, 0x00, 0x04, 0x03, 0x00, 0x00, 0x00,
        ]
    );
}
