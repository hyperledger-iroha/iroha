//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::ConfidentialComputeMechanism>(
        "iroha_data_model::da::confidential_compute::ConfidentialComputeMechanism",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ConfidentialComputePolicy>(
        "iroha_data_model::da::confidential_compute::ConfidentialComputePolicy",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
