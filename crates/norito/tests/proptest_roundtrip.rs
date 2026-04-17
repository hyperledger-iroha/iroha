//! Deterministic roundtrip tests for Norito payload shapes.

use norito::{
    NoritoDeserialize,
    core::{NoritoSerialize, decode_from_bytes, to_bytes},
    from_bytes,
};

#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema,
)]
struct TuplePayload {
    number: u32,
    text: String,
}

#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema,
)]
struct StructPayload {
    flag: bool,
    values: Vec<u8>,
}

#[derive(
    Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema,
)]
enum TestEnum {
    Unit,
    Tuple(TuplePayload),
    Struct(StructPayload),
}

#[test]
fn roundtrip_primitives() {
    let cases = [
        (false, 0_i64, 0_u64, String::new()),
        (true, -1, 1, "iroha".to_owned()),
        (false, i64::MIN, u64::MAX, "unicode π".to_owned()),
        (true, i64::MAX, 42, "line\nbreak".to_owned()),
    ];

    for value in cases {
        let bytes = to_bytes(&value).unwrap();
        let decoded: (bool, i64, u64, String) = decode_from_bytes(&bytes).unwrap();
        assert_eq!(value, decoded);
    }
}

#[test]
fn roundtrip_nested_collections() {
    let cases: Vec<Vec<Vec<u32>>> = vec![
        Vec::new(),
        vec![Vec::new()],
        vec![vec![0, 1, 2], Vec::new(), vec![u32::MAX]],
        vec![vec![10, 20], vec![30, 40, 50]],
    ];

    for value in cases {
        let bytes = to_bytes(&value).unwrap();
        let decoded: Vec<Vec<u32>> = decode_from_bytes(&bytes).unwrap();
        assert_eq!(value, decoded);
    }
}

#[test]
fn roundtrip_enums() {
    let cases = [
        TestEnum::Unit,
        TestEnum::Tuple(TuplePayload {
            number: 0,
            text: String::new(),
        }),
        TestEnum::Tuple(TuplePayload {
            number: u32::MAX,
            text: "tuple".to_owned(),
        }),
        TestEnum::Struct(StructPayload {
            flag: true,
            values: vec![0, 1, 2, 255],
        }),
    ];

    for value in cases {
        let bytes = to_bytes(&value).unwrap();
        let archived = from_bytes::<TestEnum>(&bytes).unwrap();
        let decoded = <TestEnum as NoritoDeserialize>::deserialize(archived);
        assert_eq!(value, decoded);
    }
}
