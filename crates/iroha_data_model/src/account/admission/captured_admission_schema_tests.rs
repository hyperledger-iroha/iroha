//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::AccountAdmissionMode>(
        "iroha_data_model::account::admission::AccountAdmissionMode",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ImplicitAccountFeeDestination>(
        "iroha_data_model::account::admission::ImplicitAccountFeeDestination",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ImplicitAccountCreationFee>(
        "iroha_data_model::account::admission::ImplicitAccountCreationFee",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AccountAdmissionPolicy>(
        "iroha_data_model::account::admission::AccountAdmissionPolicy",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
