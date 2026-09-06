//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::DomainEndorsementScope>(
        "iroha_data_model::nexus::endorsement::DomainEndorsementScope",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DomainEndorsementSignature>(
        "iroha_data_model::nexus::endorsement::DomainEndorsementSignature",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DomainEndorsement>(
        "iroha_data_model::nexus::endorsement::DomainEndorsement",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DomainCommittee>(
        "iroha_data_model::nexus::endorsement::DomainCommittee",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DomainEndorsementPolicy>(
        "iroha_data_model::nexus::endorsement::DomainEndorsementPolicy",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DomainEndorsementRecord>(
        "iroha_data_model::nexus::endorsement::DomainEndorsementRecord",
    );
}
