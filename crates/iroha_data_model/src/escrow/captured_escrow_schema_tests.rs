//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::AssetEscrowStatus>(
        "iroha_data_model::escrow::AssetEscrowStatus",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetEscrowKind>(
        "iroha_data_model::escrow::AssetEscrowKind",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConditionalEscrowValue>(
        "iroha_data_model::escrow::ConditionalEscrowValue",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConditionalEscrowPredicate>(
        "iroha_data_model::escrow::ConditionalEscrowPredicate",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConditionalEscrowOracleCondition>(
        "iroha_data_model::escrow::ConditionalEscrowOracleCondition",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConditionalEscrowWithinCondition>(
        "iroha_data_model::escrow::ConditionalEscrowWithinCondition",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConditionalEscrowCondition>(
        "iroha_data_model::escrow::ConditionalEscrowCondition",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConditionalEscrowAttestation>(
        "iroha_data_model::escrow::ConditionalEscrowAttestation",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConditionalEscrowConditionState>(
        "iroha_data_model::escrow::ConditionalEscrowConditionState",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetEscrowResolution>(
        "iroha_data_model::escrow::AssetEscrowResolution",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetEscrowRecord>(
        "iroha_data_model::escrow::AssetEscrowRecord",
    );
}
