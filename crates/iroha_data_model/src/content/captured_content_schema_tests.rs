//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::ContentFileEntry>(
        "iroha_data_model::content::ContentFileEntry",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ContentCachePolicy>(
        "iroha_data_model::content::ContentCachePolicy",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ContentAuthMode>(
        "iroha_data_model::content::ContentAuthMode",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ContentBundleManifest>(
        "iroha_data_model::content::ContentBundleManifest",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ContentBundleRecord>(
        "iroha_data_model::content::ContentBundleRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ContentChunk>(
        "iroha_data_model::content::ContentChunk",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ContentRange>(
        "iroha_data_model::content::ContentRange",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ContentDaReceipt>(
        "iroha_data_model::content::ContentDaReceipt",
    );
}
