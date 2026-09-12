//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::LaneCompliancePolicyId>(
        "iroha_data_model::nexus::compliance::LaneCompliancePolicyId",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneCompliancePolicy>(
        "iroha_data_model::nexus::compliance::LaneCompliancePolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneComplianceRule>(
        "iroha_data_model::nexus::compliance::LaneComplianceRule",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ParticipantSelector>(
        "iroha_data_model::nexus::compliance::ParticipantSelector",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::TransferLimit>(
        "iroha_data_model::nexus::compliance::TransferLimit",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::AuditControls>(
        "iroha_data_model::nexus::compliance::AuditControls",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JurisdictionSet>(
        "iroha_data_model::nexus::compliance::JurisdictionSet",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JurisdictionFlag>(
        "iroha_data_model::nexus::compliance::JurisdictionFlag",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
