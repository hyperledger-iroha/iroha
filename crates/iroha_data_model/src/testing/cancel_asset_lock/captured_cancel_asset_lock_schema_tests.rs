//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::serialize::<super::LegacyCancelAssetLock>(
        "iroha_data_model::testing::cancel_asset_lock::LegacyCancelAssetLock",
    ),
    crate::captured_schema_tests::Case::serialize::<super::RetiredNestedEscrowId>(
        "iroha_data_model::testing::cancel_asset_lock::RetiredNestedEscrowId",
    ),
    crate::captured_schema_tests::Case::serialize::<super::RetiredNestedCancelAssetLock>(
        "iroha_data_model::testing::cancel_asset_lock::RetiredNestedCancelAssetLock",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
