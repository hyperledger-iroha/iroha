//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::AccountAliasDomain>(
        "iroha_data_model::account::rekey::AccountAliasDomain",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AccountRekeyTransitionProvenance>(
        "iroha_data_model::account::rekey::AccountRekeyTransitionProvenance",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AccountRekeyRecord>(
        "iroha_data_model::account::rekey::AccountRekeyRecord",
    );
}
