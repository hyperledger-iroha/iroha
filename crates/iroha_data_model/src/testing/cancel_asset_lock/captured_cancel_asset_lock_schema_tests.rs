//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_serialize::<super::LegacyCancelAssetLock>(
        "iroha_data_model::testing::cancel_asset_lock::LegacyCancelAssetLock",
    );
    crate::captured_schema_tests::assert_serialize::<super::RetiredNestedEscrowId>(
        "iroha_data_model::testing::cancel_asset_lock::RetiredNestedEscrowId",
    );
    crate::captured_schema_tests::assert_serialize::<super::RetiredNestedCancelAssetLock>(
        "iroha_data_model::testing::cancel_asset_lock::RetiredNestedCancelAssetLock",
    );
}
