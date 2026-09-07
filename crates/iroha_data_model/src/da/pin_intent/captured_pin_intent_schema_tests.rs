//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::DaPinIntent>(
        "iroha_data_model::da::pin_intent::DaPinIntent",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaPinIntentBundle>(
        "iroha_data_model::da::pin_intent::DaPinIntentBundle",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaPinIntentWithLocation>(
        "iroha_data_model::da::pin_intent::DaPinIntentWithLocation",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaPinIntentProof>(
        "iroha_data_model::da::pin_intent::DaPinIntentProof",
    );
}
