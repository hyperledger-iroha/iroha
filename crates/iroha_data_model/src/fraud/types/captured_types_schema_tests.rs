//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::Case::bidirectional::<super::RiskOperation>(
        "iroha_data_model::fraud::types::RiskOperation",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::RiskContext>(
        "iroha_data_model::fraud::types::RiskContext",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::FeatureInput>(
        "iroha_data_model::fraud::types::FeatureInput",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::RiskQuery>(
        "iroha_data_model::fraud::types::RiskQuery",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::AssessmentDecision>(
        "iroha_data_model::fraud::types::AssessmentDecision",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::RuleOutcome>(
        "iroha_data_model::fraud::types::RuleOutcome",
    )
    .check();
    crate::captured_schema_tests::Case::bidirectional::<super::FraudAssessment>(
        "iroha_data_model::fraud::types::FraudAssessment",
    )
    .check();
    #[cfg(feature = "governance")]
    crate::captured_schema_tests::Case::bidirectional::<super::GovernanceExport>(
        "iroha_data_model::fraud::types::GovernanceExport",
    )
    .check();
    #[cfg(feature = "governance")]
    crate::captured_schema_tests::Case::bidirectional::<super::DecisionAggregate>(
        "iroha_data_model::fraud::types::DecisionAggregate",
    )
    .check();
}
