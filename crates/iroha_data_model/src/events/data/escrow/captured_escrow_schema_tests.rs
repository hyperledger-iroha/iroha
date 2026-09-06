//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::EscrowEvent>(
        "iroha_data_model::events::data::escrow::EscrowEvent",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ConditionalEscrowAttested>(
        "iroha_data_model::events::data::escrow::ConditionalEscrowAttested",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetEscrowDisputed>(
        "iroha_data_model::events::data::escrow::AssetEscrowDisputed",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetEscrowResolved>(
        "iroha_data_model::events::data::escrow::AssetEscrowResolved",
    );
}
