//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::RiskOperation>(
        "iroha_data_model::fraud::types::RiskOperation",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RiskContext>(
        "iroha_data_model::fraud::types::RiskContext",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::FeatureInput>(
        "iroha_data_model::fraud::types::FeatureInput",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RiskQuery>(
        "iroha_data_model::fraud::types::RiskQuery",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AssessmentDecision>(
        "iroha_data_model::fraud::types::AssessmentDecision",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RuleOutcome>(
        "iroha_data_model::fraud::types::RuleOutcome",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::FraudAssessment>(
        "iroha_data_model::fraud::types::FraudAssessment",
    );
    #[cfg(feature = "governance")]
    crate::captured_schema_tests::assert_bidirectional::<super::GovernanceExport>(
        "iroha_data_model::fraud::types::GovernanceExport",
    );
    #[cfg(feature = "governance")]
    crate::captured_schema_tests::assert_bidirectional::<super::DecisionAggregate>(
        "iroha_data_model::fraud::types::DecisionAggregate",
    );
}
