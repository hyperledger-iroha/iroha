//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::ContentFileEntry>(
        "iroha_data_model::content::ContentFileEntry",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ContentCachePolicy>(
        "iroha_data_model::content::ContentCachePolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ContentAuthMode>(
        "iroha_data_model::content::ContentAuthMode",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ContentBundleManifest>(
        "iroha_data_model::content::ContentBundleManifest",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ContentBundleRecord>(
        "iroha_data_model::content::ContentBundleRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ContentChunk>(
        "iroha_data_model::content::ContentChunk",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ContentRange>(
        "iroha_data_model::content::ContentRange",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ContentDaReceipt>(
        "iroha_data_model::content::ContentDaReceipt",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
