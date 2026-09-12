//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::RecoveryGuardian>(
        "iroha_data_model::account::recovery::RecoveryGuardian",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AccountRecoveryPolicy>(
        "iroha_data_model::account::recovery::AccountRecoveryPolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AccountRecoveryStatus>(
        "iroha_data_model::account::recovery::AccountRecoveryStatus",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AccountRecoveryRequest>(
        "iroha_data_model::account::recovery::AccountRecoveryRequest",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
