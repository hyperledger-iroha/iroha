//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::UniversalPortfolio>(
        "iroha_data_model::nexus::portfolio::UniversalPortfolio",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PortfolioTotals>(
        "iroha_data_model::nexus::portfolio::PortfolioTotals",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DataspacePortfolio>(
        "iroha_data_model::nexus::portfolio::DataspacePortfolio",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AccountPortfolio>(
        "iroha_data_model::nexus::portfolio::AccountPortfolio",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssetPosition>(
        "iroha_data_model::nexus::portfolio::AssetPosition",
    );
}
