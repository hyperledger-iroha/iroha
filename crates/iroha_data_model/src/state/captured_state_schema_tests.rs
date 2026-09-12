//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::DomainMetadataKey>(
        "iroha_data_model::state::DomainMetadataKey",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AccountMetadataKey>(
        "iroha_data_model::state::AccountMetadataKey",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AssetDefinitionMetadataKey>(
        "iroha_data_model::state::AssetDefinitionMetadataKey",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AssetMetadataKey>(
        "iroha_data_model::state::AssetMetadataKey",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::NftMetadataKey>(
        "iroha_data_model::state::NftMetadataKey",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RwaMetadataKey>(
        "iroha_data_model::state::RwaMetadataKey",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::TriggerMetadataKey>(
        "iroha_data_model::state::TriggerMetadataKey",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AccountRoleKey>(
        "iroha_data_model::state::AccountRoleKey",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::TxQueueKey>(
        "iroha_data_model::state::TxQueueKey",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::CanonicalStateKey>(
        "iroha_data_model::state::CanonicalStateKey",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::StateAccessSetAdvisory>(
        "iroha_data_model::state::StateAccessSetAdvisory",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
