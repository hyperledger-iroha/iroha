//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::AccountAdmissionMode>(
        "iroha_data_model::account::admission::AccountAdmissionMode",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ImplicitAccountFeeDestination>(
        "iroha_data_model::account::admission::ImplicitAccountFeeDestination",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ImplicitAccountCreationFee>(
        "iroha_data_model::account::admission::ImplicitAccountCreationFee",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AccountAdmissionPolicy>(
        "iroha_data_model::account::admission::AccountAdmissionPolicy",
    );
}
