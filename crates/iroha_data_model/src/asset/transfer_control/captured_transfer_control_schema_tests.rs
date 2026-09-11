//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::AssetTransferAvailability>(
        "iroha_data_model::asset::transfer_control::AssetTransferAvailability",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AssetTransferControlWindow>(
        "iroha_data_model::asset::transfer_control::AssetTransferControlWindow",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AssetTransferLimit>(
        "iroha_data_model::asset::transfer_control::AssetTransferLimit",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AssetTransferUsageBucket>(
        "iroha_data_model::asset::transfer_control::AssetTransferUsageBucket",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AssetTransferControlRecord>(
        "iroha_data_model::asset::transfer_control::AssetTransferControlRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AssetTransferControlStoreV1>(
        "iroha_data_model::asset::transfer_control::AssetTransferControlStoreV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
