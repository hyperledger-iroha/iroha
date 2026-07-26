use norito::core::{DecodeFromSlice, Error};

fn encode_sequence_len(len: usize) -> Vec<u8> {
    (len as u64).to_le_bytes().to_vec()
}

#[test]
fn vec_decode_rejects_unbounded_len() {
    let payload = encode_sequence_len(usize::MAX);
    // No element bytes follow the header, so any non-zero len is impossible.
    let err =
        <Vec<u8> as DecodeFromSlice>::decode_from_slice(&payload).expect_err("len guard failed");
    assert!(matches!(err, Error::LengthMismatch));
}
