//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::RepoCashLeg>(
        "iroha_data_model::repo::RepoCashLeg",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RepoCollateralLeg>(
        "iroha_data_model::repo::RepoCollateralLeg",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RepoGovernance>(
        "iroha_data_model::repo::RepoGovernance",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RepoAgreement>(
        "iroha_data_model::repo::RepoAgreement",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
