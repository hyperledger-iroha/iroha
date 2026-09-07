//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::AssetTransferAvailability>(
        "iroha_data_model::asset::transfer_control::AssetTransferAvailability",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetTransferControlWindow>(
        "iroha_data_model::asset::transfer_control::AssetTransferControlWindow",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetTransferLimit>(
        "iroha_data_model::asset::transfer_control::AssetTransferLimit",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetTransferUsageBucket>(
        "iroha_data_model::asset::transfer_control::AssetTransferUsageBucket",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetTransferControlRecord>(
        "iroha_data_model::asset::transfer_control::AssetTransferControlRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetTransferControlStoreV1>(
        "iroha_data_model::asset::transfer_control::AssetTransferControlStoreV1",
    );
}
