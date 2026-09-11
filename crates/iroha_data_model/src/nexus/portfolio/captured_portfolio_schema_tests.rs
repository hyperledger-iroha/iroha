//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::UniversalPortfolio>(
        "iroha_data_model::nexus::portfolio::UniversalPortfolio",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PortfolioTotals>(
        "iroha_data_model::nexus::portfolio::PortfolioTotals",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DataspacePortfolio>(
        "iroha_data_model::nexus::portfolio::DataspacePortfolio",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AccountPortfolio>(
        "iroha_data_model::nexus::portfolio::AccountPortfolio",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AssetPosition>(
        "iroha_data_model::nexus::portfolio::AssetPosition",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
