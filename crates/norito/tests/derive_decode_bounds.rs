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

#[derive(Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
struct EnumField(u32);

#[derive(Debug, PartialEq, Eq, norito::derive::Encode, norito::derive::Decode)]
enum DerivedTupleEnum {
    Pair(EnumField, u32),
    Boundary(EnumField, #[norito(skip)] ()),
}

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

#[test]
fn derived_multi_field_tuple_enum_canonical_roundtrips() {
    let value = DerivedTupleEnum::Pair(EnumField(0x1122_3344), 0x5566_7788);
    let frame = norito::encode_canonical(&value).expect("encode canonical tuple enum");
    let decoded: DerivedTupleEnum =
        norito::decode_canonical(&frame).expect("decode canonical tuple enum");

    assert_eq!(decoded, value);
}

#[test]
fn derived_tuple_enum_rejects_understated_first_field_length() {
    let value = DerivedTupleEnum::Boundary(EnumField(0x1122_3344), ());
    let frame = norito::encode_canonical(&value).expect("encode canonical tuple enum");
    let view = norito::core::from_bytes_view(&frame).expect("inspect canonical frame");
    let flags = view.flags();
    let mut payload = view.as_bytes().to_vec();

    let first_field_prefix = 4;
    let (declared, prefix_len) =
        norito::core::read_len_from_slice_with_flags(&payload[first_field_prefix..], flags)
            .expect("read first field length");
    assert_eq!(declared, 4);
    assert_eq!(prefix_len, 1);
    payload[first_field_prefix] = u8::try_from(declared - 1).expect("shortened field length");

    let forged = norito::core::frame_bare_with_header_flags::<DerivedTupleEnum>(&payload, flags)
        .expect("frame corrupted tuple enum");
    assert!(matches!(
        norito::decode_canonical::<DerivedTupleEnum>(&forged),
        Err(Error::LengthMismatch)
    ));
}
