use norito::{
    NoritoDeserialize,
    core::{NoritoSerialize, decode_from_bytes, to_bytes},
    from_bytes,
};
use proptest::prelude::*;

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

fn arb_test_enum() -> impl Strategy<Value = TestEnum> {
    prop_oneof![
        Just(TestEnum::Unit),
        (any::<u32>(), any::<String>())
            .prop_map(|(number, text)| TestEnum::Tuple(TuplePayload { number, text })),
        (any::<bool>(), prop::collection::vec(any::<u8>(), 0..5))
            .prop_map(|(flag, values)| { TestEnum::Struct(StructPayload { flag, values }) }),
    ]
}

proptest! {
    #[test]
    fn roundtrip_primitives(value in any::<(bool, i64, u64, String)>()) {
        let bytes = to_bytes(&value).unwrap();
        let decoded: (bool, i64, u64, String) = decode_from_bytes(&bytes).unwrap();
        prop_assert_eq!(value, decoded);
    }

    #[test]
    fn roundtrip_nested_collections(value in prop::collection::vec(prop::collection::vec(any::<u32>(), 0..5), 0..5)) {
        let bytes = to_bytes(&value).unwrap();
        let decoded: Vec<Vec<u32>> = decode_from_bytes(&bytes).unwrap();
        prop_assert_eq!(value, decoded);
    }

    #[test]
    fn roundtrip_enums(value in arb_test_enum()) {
        let bytes = to_bytes(&value).unwrap();
        let archived = from_bytes::<TestEnum>(&bytes).unwrap();
        let decoded = <TestEnum as NoritoDeserialize>::deserialize(archived);
        prop_assert_eq!(value, decoded);
    }
}
