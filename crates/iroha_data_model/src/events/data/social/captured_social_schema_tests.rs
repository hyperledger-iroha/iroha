//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::SocialEvent>(
        "iroha_data_model::events::data::social::SocialEvent",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ViralRewardApplied>(
        "iroha_data_model::events::data::social::ViralRewardApplied",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ViralEscrowCreated>(
        "iroha_data_model::events::data::social::ViralEscrowCreated",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ViralEscrowReleased>(
        "iroha_data_model::events::data::social::ViralEscrowReleased",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ViralEscrowCancelled>(
        "iroha_data_model::events::data::social::ViralEscrowCancelled",
    );
}
