use std::io::Cursor;

use norito::{Error, codec::Decode as NoritoDecode};

#[derive(norito::derive::Encode, norito::derive::Decode)]
struct Wrapper {
    value: String,
}

#[derive(norito::derive::Encode, norito::derive::Decode)]
struct Dual {
    first: String,
    second: String,
}

#[derive(norito::derive::Encode, norito::derive::Decode)]
struct TupleDual(String, String);

#[test]
fn derive_decode_rejects_overlong_field() {
    // Craft a payload where the field length claims more bytes than are available.
    let mut payload = Vec::new();
    payload.extend_from_slice(&u64::MAX.to_le_bytes());
    payload.extend_from_slice(b"abc");

    let mut cursor = Cursor::new(payload);
    let decoded = Wrapper::decode(&mut cursor);
    match decoded {
        Err(Error::LengthMismatch) => {}
        Err(err) => panic!("unexpected decode error: {err:?}"),
        Ok(_) => panic!("expected decode failure for truncated payload"),
    }
}

#[test]
fn derive_decode_rejects_truncated_second_field() {
    // Encoded layout for `Dual { first, second }` with default len headers (8-byte LE).
    // first = "abc" (len=3), second declares len=10 but only provides 2 bytes.
    let mut payload = Vec::new();
    payload.extend_from_slice(&(3u64).to_le_bytes());
    payload.extend_from_slice(b"abc");
    payload.extend_from_slice(&(10u64).to_le_bytes());
    payload.extend_from_slice(b"xy"); // insufficient bytes for declared length

    let mut cursor = Cursor::new(payload);
    let decoded = Dual::decode(&mut cursor);
    match decoded {
        Err(Error::LengthMismatch) => {}
        Err(err) => panic!("unexpected decode error: {err:?}"),
        Ok(_) => panic!("expected decode failure for truncated second field"),
    }
}

#[test]
fn tuple_decode_rejects_truncated_second_field() {
    // Same scenario as above but exercising the tuple-field decode path.
    let mut payload = Vec::new();
    payload.extend_from_slice(&(1u64).to_le_bytes());
    payload.extend_from_slice(b"x");
    payload.extend_from_slice(&(5u64).to_le_bytes());
    payload.extend_from_slice(b"yz"); // missing 3 bytes

    let mut cursor = Cursor::new(payload);
    let decoded = TupleDual::decode(&mut cursor);
    match decoded {
        Err(Error::LengthMismatch) => {}
        Err(err) => panic!("unexpected decode error: {err:?}"),
        Ok(_) => panic!("expected decode failure for tuple second field"),
    }
}
