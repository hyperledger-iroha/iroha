//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    #[cfg(test)]
    crate::captured_schema_tests::Case::serialize::<super::ForgedTokenValue>(
        "iroha_data_model::sns::tests::ForgedTokenValue",
    )
    .check();
}
