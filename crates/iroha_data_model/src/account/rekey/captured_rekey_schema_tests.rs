//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::AccountAliasDomain>(
        "iroha_data_model::account::rekey::AccountAliasDomain",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AccountRekeyTransitionProvenance>(
        "iroha_data_model::account::rekey::AccountRekeyTransitionProvenance",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AccountRekeyRecord>(
        "iroha_data_model::account::rekey::AccountRekeyRecord",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
