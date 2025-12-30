#![allow(clippy::manual_div_ceil)]
use norito::{
    Compression,
    core::{Header, NoritoDeserialize, NoritoSerialize},
    deserialize_from, serialize_into,
};

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct TestData {
    a: u32,
    b: bool,
}

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct VecStruct {
    flag: bool,
    values: Vec<u8>,
}

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct OptionStruct {
    value: Option<[u8; 32]>,
}

#[test]
fn roundtrip_no_compression() {
    let data = TestData { a: 42, b: true };
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::None).unwrap();
    let decoded: TestData = deserialize_from(buf.as_slice()).unwrap();
    assert_eq!(data, decoded);
}

#[test]
fn roundtrip_zstd() {
    let data = TestData { a: 7, b: false };
    let mut buf = Vec::new();
    serialize_into(&mut buf, &data, Compression::Zstd).unwrap();
    let decoded: TestData = deserialize_from(buf.as_slice()).unwrap();
    assert_eq!(data, decoded);
}

#[test]
fn empty_vec_roundtrip() {
    let original: Vec<u8> = Vec::new();
    let bytes = norito::core::to_bytes(&original).unwrap();
    let archived = norito::core::from_bytes::<Vec<u8>>(&bytes).unwrap();
    let decoded = <Vec<u8> as NoritoDeserialize>::deserialize(archived);
    assert_eq!(original, decoded);
}

#[test]
fn empty_vec_struct_roundtrip() {
    let value = VecStruct {
        flag: false,
        values: Vec::new(),
    };
    let bytes = norito::core::to_bytes(&value).unwrap();
    let flags = bytes[Header::SIZE - 1];
    assert_eq!(flags, 0, "sequential layout must not set FIELD_BITSET");
    let payload = &bytes[Header::SIZE..];
    assert!(payload.len() >= 8 + 1 + 8 + 8, "payload too short");

    let mut offset = 0usize;
    let read_u64 = |data: &[u8], pos: &mut usize| -> usize {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&data[*pos..*pos + 8]);
        *pos += 8;
        u64::from_le_bytes(buf) as usize
    };

    let flag_len = read_u64(payload, &mut offset);
    assert_eq!(flag_len, 1, "bool field must report length 1");
    assert_eq!(payload[offset], 0);
    offset += flag_len;

    let vec_len = read_u64(payload, &mut offset);
    assert_eq!(
        vec_len, 8,
        "Vec<u8> wrapper should be 8 bytes for empty payload"
    );
    assert!(offset + vec_len <= payload.len());
    let vec_bytes = &payload[offset..offset + vec_len];
    offset += vec_len;
    assert_eq!(
        offset,
        payload.len(),
        "unexpected trailing bytes in payload"
    );

    let mut inner_len_bytes = [0u8; 8];
    inner_len_bytes.copy_from_slice(vec_bytes);
    let inner_len = u64::from_le_bytes(inner_len_bytes);
    assert_eq!(inner_len, 0, "empty Vec<u8> must report zero elements");

    let archived = norito::core::from_bytes::<VecStruct>(&bytes).unwrap();
    let decoded = VecStruct::deserialize(archived);
    assert_eq!(value, decoded);
}

#[test]
fn fixed_option_without_size_header() {
    let value = OptionStruct {
        value: Some([0xAA; 32]),
    };
    let fresh = norito::core::to_bytes(&value).unwrap();
    let header_flags = fresh[Header::SIZE - 1];
    assert_eq!(header_flags, 0);
    let mut payload = fresh[Header::SIZE..].to_vec();
    assert!(payload.len() >= 8 + 1 + 8);

    // Struct field payload length.
    let mut len_bytes = [0u8; 8];
    len_bytes.copy_from_slice(&payload[..8]);
    let field_len = u64::from_le_bytes(len_bytes) as usize;
    assert!(field_len > 0);
    assert_eq!(payload.len(), 8 + field_len);

    let field_start = 8;
    assert_eq!(payload[field_start], 1, "Option tag must be Some");

    let mut inner_len = [0u8; 8];
    inner_len.copy_from_slice(&payload[field_start + 1..field_start + 9]);
    let inner_len = u64::from_le_bytes(inner_len) as usize;
    assert_eq!(
        inner_len,
        field_len - 1 - 8,
        "inner payload length should match Option body"
    );

    // Remove the inner length header to simulate corruption.
    payload.drain(field_start + 1..field_start + 9);

    let fixed_bytes =
        norito::core::frame_bare_with_header_flags::<OptionStruct>(&payload, header_flags).unwrap();
    let err = norito::decode_from_bytes::<OptionStruct>(&fixed_bytes)
        .expect_err("fixed option must be rejected");
    assert!(matches!(
        err,
        norito::core::Error::LengthMismatch | norito::core::Error::DecodePanic { .. }
    ));
}
