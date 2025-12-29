//! Negative tests for malformed varint length headers under compact-len.

use norito::decode_bare_from_bytes;

#[test]
fn overlong_varint_in_bare_string_errors() {
    // Construct an overlong varint: 11 bytes with continuation bits set
    // This should fail under compact-len decoders.
    let bytes = vec![0xFFu8; 11];
    // No payload follows; decoder must reject the varint header
    // Without `strict-safe`, this path may panic inside `deserialize`.
    let res = std::panic::catch_unwind(|| decode_bare_from_bytes::<String>(&bytes));
    if let Ok(Ok(_)) = res {
        panic!("expected error or panic on malformed varint");
    }
}
