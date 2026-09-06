//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::DomainMetadataKey>(
        "iroha_data_model::state::DomainMetadataKey",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AccountMetadataKey>(
        "iroha_data_model::state::AccountMetadataKey",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetDefinitionMetadataKey>(
        "iroha_data_model::state::AssetDefinitionMetadataKey",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetMetadataKey>(
        "iroha_data_model::state::AssetMetadataKey",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::NftMetadataKey>(
        "iroha_data_model::state::NftMetadataKey",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RwaMetadataKey>(
        "iroha_data_model::state::RwaMetadataKey",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::TriggerMetadataKey>(
        "iroha_data_model::state::TriggerMetadataKey",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AccountRoleKey>(
        "iroha_data_model::state::AccountRoleKey",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::TxQueueKey>(
        "iroha_data_model::state::TxQueueKey",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::CanonicalStateKey>(
        "iroha_data_model::state::CanonicalStateKey",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::StateAccessSetAdvisory>(
        "iroha_data_model::state::StateAccessSetAdvisory",
    );
}
