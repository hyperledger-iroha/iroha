//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::RecoveryGuardian>(
        "iroha_data_model::account::recovery::RecoveryGuardian",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AccountRecoveryPolicy>(
        "iroha_data_model::account::recovery::AccountRecoveryPolicy",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AccountRecoveryStatus>(
        "iroha_data_model::account::recovery::AccountRecoveryStatus",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AccountRecoveryRequest>(
        "iroha_data_model::account::recovery::AccountRecoveryRequest",
    );
}
