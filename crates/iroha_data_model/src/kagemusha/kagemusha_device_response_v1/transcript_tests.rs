//! Exact response transcript and u32 wire-length boundary regressions.

use super::*;

#[test]
fn response_transcript_preserves_every_original_byte() {
    let actual = kagemusha_device_response_signing_bytes_v1(
        1, [7; 32], b"command", b"reply", [8; 32], [9; 32],
    )
    .unwrap();
    let expected = concat!(
        "69726f68613a6b6167656d757368613a6465766963653a76313a726573706f6e",
        "73652d61757468656e74696361746f7200494b474d4a52533101000100070707",
        "0707070707070707070707070707070707070707070707070707070707050000",
        "00400000005782b18687e6cf8a482fc32d2db5b196d8821c458a0c069c6acf39",
        "53446e7bb55d347fd948b66308f502c3f65c8f7e12ff1c5cf8c760bcdfb188ae",
        "1ec7b8b618080808080808080808080808080808080808080808080808080808",
        "0808080808090909090909090909090909090909090909090909090909090909",
        "0909090909",
    );
    assert_eq!(hex::encode(actual), expected);
}

#[test]
fn response_wire_length_accepts_the_full_body_bound_and_rejects_the_next_byte() {
    let maximum = vec![1; KAGEMUSHA_DEVICE_PAYLOAD_MAX_BYTES_V1];
    let oversized = vec![1; KAGEMUSHA_DEVICE_PAYLOAD_MAX_BYTES_V1 + 1];
    let transcript = kagemusha_device_response_signing_bytes_v1(
        1, [7; 32], &maximum, &maximum, [8; 32], [9; 32],
    )
    .unwrap();
    let lengths = RESPONSE_DOMAIN.len() + 1 + 44;
    assert_eq!(
        &transcript[lengths..lengths + 8],
        &[0, 0, 1, 0, 64, 0, 0, 0]
    );
    for (command, reply) in [(&oversized, &maximum), (&maximum, &oversized)] {
        assert_eq!(
            kagemusha_device_response_signing_bytes_v1(
                1, [7; 32], command, reply, [8; 32], [9; 32],
            ),
            Err(KagemushaDeviceResponseErrorV1::Binding),
        );
    }
}
