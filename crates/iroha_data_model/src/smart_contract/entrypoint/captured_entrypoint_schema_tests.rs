//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::serialize::<super::EntrypointValueTypeV1>(
        "iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1",
    ),
    crate::captured_schema_tests::Case::deserialize::<super::DecodedEntrypointValueTypeV1>(
        "iroha_data_model::smart_contract::entrypoint::DecodedEntrypointValueTypeV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::EntrypointValueWordKindV1>(
        "iroha_data_model::smart_contract::entrypoint::EntrypointValueWordKindV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::EntrypointValueAtomV1>(
        "iroha_data_model::smart_contract::entrypoint::EntrypointValueAtomV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::EntrypointArgumentRecordV1>(
        "iroha_data_model::smart_contract::entrypoint::EntrypointArgumentRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::EntrypointReturnRecordV1>(
        "iroha_data_model::smart_contract::entrypoint::EntrypointReturnRecordV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
