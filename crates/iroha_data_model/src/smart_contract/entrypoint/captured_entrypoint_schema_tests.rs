//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_serialize::<super::EntrypointValueTypeV1>(
        "iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1",
    );
    crate::captured_schema_tests::assert_deserialize::<super::DecodedEntrypointValueTypeV1>(
        "iroha_data_model::smart_contract::entrypoint::DecodedEntrypointValueTypeV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::EntrypointValueWordKindV1>(
        "iroha_data_model::smart_contract::entrypoint::EntrypointValueWordKindV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::EntrypointValueAtomV1>(
        "iroha_data_model::smart_contract::entrypoint::EntrypointValueAtomV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::EntrypointArgumentRecordV1>(
        "iroha_data_model::smart_contract::entrypoint::EntrypointArgumentRecordV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::EntrypointReturnRecordV1>(
        "iroha_data_model::smart_contract::entrypoint::EntrypointReturnRecordV1",
    );
}
