// Canonical CAR decoding wrappers shared by unit-test components.
fn decode_uleb128(data: &[u8]) -> (u64, usize) {
    verifier::decode_uleb128(data).expect("canonical CAR ULEB128")
}
fn decode_cid(data: &[u8]) -> (usize, u64) {
    let (cid, consumed) = verifier::decode_cid(data, 0).expect("canonical CAR CID");
    (consumed, cid.codec)
}
fn decode_cbor_map_len(data: &[u8]) -> (u64, usize) {
    verifier::decode_cbor_map_len(data).expect("canonical CAR CBOR map")
}
fn decode_cbor_array_len(data: &[u8]) -> (u64, usize) {
    verifier::decode_cbor_array_len(data).expect("canonical CAR CBOR array")
}
fn decode_cbor_uint(data: &[u8]) -> (u64, usize) {
    verifier::decode_cbor_uint(data).expect("canonical CAR CBOR integer")
}
fn decode_cbor_text(data: &[u8]) -> (&str, usize) {
    verifier::decode_cbor_text(data).expect("canonical CAR CBOR text")
}
fn decode_cbor_bytes(data: &[u8]) -> (Vec<u8>, usize) {
    verifier::decode_cbor_bytes(data).expect("canonical CAR CBOR bytes")
}
