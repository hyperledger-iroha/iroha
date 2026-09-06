//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::RepoCashLeg>(
        "iroha_data_model::repo::RepoCashLeg",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RepoCollateralLeg>(
        "iroha_data_model::repo::RepoCollateralLeg",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RepoGovernance>(
        "iroha_data_model::repo::RepoGovernance",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RepoAgreement>(
        "iroha_data_model::repo::RepoAgreement",
    );
}
