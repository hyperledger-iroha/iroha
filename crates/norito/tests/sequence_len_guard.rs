use norito::core::{DecodeFromSlice, Error, compact_seq_length_enabled};

fn encode_sequence_len(len: usize) -> Vec<u8> {
    if compact_seq_length_enabled() {
        encode_varint(len as u64)
    } else {
        (len as u64).to_le_bytes().to_vec()
    }
}

fn encode_varint(mut value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        } else {
            out.push(byte | 0x80);
        }
    }
    out
}

#[test]
fn vec_decode_rejects_unbounded_len() {
    let payload = encode_sequence_len(usize::MAX);
    // No element bytes follow the header, so any non-zero len is impossible.
    let err =
        <Vec<u8> as DecodeFromSlice>::decode_from_slice(&payload).expect_err("len guard failed");
    assert!(matches!(err, Error::LengthMismatch));
}
