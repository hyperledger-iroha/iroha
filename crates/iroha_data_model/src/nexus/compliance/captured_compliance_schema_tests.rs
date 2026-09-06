//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::LaneCompliancePolicyId>(
        "iroha_data_model::nexus::compliance::LaneCompliancePolicyId",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::LaneCompliancePolicy>(
        "iroha_data_model::nexus::compliance::LaneCompliancePolicy",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::LaneComplianceRule>(
        "iroha_data_model::nexus::compliance::LaneComplianceRule",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ParticipantSelector>(
        "iroha_data_model::nexus::compliance::ParticipantSelector",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::TransferLimit>(
        "iroha_data_model::nexus::compliance::TransferLimit",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::AuditControls>(
        "iroha_data_model::nexus::compliance::AuditControls",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::JurisdictionSet>(
        "iroha_data_model::nexus::compliance::JurisdictionSet",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::JurisdictionFlag>(
        "iroha_data_model::nexus::compliance::JurisdictionFlag",
    );
}
