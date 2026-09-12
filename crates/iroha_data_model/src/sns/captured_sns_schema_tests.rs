//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::NameSelectorV1>(
        "iroha_data_model::sns::NameSelectorV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::NameRecordV1>(
        "iroha_data_model::sns::NameRecordV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::NameStatus>(
        "iroha_data_model::sns::NameStatus",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::NameFrozenStateV1>(
        "iroha_data_model::sns::NameFrozenStateV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::NameTombstoneStateV1>(
        "iroha_data_model::sns::NameTombstoneStateV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::TokenValue>(
        "iroha_data_model::sns::TokenValue",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::NameAuctionStateV1>(
        "iroha_data_model::sns::NameAuctionStateV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AuctionKind>(
        "iroha_data_model::sns::AuctionKind",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::NameControllerV1>(
        "iroha_data_model::sns::NameControllerV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ControllerType>(
        "iroha_data_model::sns::ControllerType",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PriceTierV1>(
        "iroha_data_model::sns::PriceTierV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ReservedNameV1>(
        "iroha_data_model::sns::ReservedNameV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SuffixFeeSplitV1>(
        "iroha_data_model::sns::SuffixFeeSplitV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SuffixStatus>(
        "iroha_data_model::sns::SuffixStatus",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SuffixPolicyV1>(
        "iroha_data_model::sns::SuffixPolicyV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
