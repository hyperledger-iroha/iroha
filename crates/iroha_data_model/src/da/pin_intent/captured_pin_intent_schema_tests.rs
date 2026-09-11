//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::DaPinIntent>(
        "iroha_data_model::da::pin_intent::DaPinIntent",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaPinIntentBundle>(
        "iroha_data_model::da::pin_intent::DaPinIntentBundle",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaPinIntentWithLocation>(
        "iroha_data_model::da::pin_intent::DaPinIntentWithLocation",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaPinIntentProof>(
        "iroha_data_model::da::pin_intent::DaPinIntentProof",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
