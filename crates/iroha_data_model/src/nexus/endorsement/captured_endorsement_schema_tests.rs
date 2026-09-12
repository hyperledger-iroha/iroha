//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::DomainEndorsementScope>(
        "iroha_data_model::nexus::endorsement::DomainEndorsementScope",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DomainEndorsementSignature>(
        "iroha_data_model::nexus::endorsement::DomainEndorsementSignature",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DomainEndorsement>(
        "iroha_data_model::nexus::endorsement::DomainEndorsement",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DomainCommittee>(
        "iroha_data_model::nexus::endorsement::DomainCommittee",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DomainEndorsementPolicy>(
        "iroha_data_model::nexus::endorsement::DomainEndorsementPolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DomainEndorsementRecord>(
        "iroha_data_model::nexus::endorsement::DomainEndorsementRecord",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
