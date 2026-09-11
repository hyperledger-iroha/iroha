//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::JurisdictionId>(
        "iroha_data_model::jurisdiction::JurisdictionId",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgBlockRange>(
        "iroha_data_model::jurisdiction::JdgBlockRange",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgAttestationScope>(
        "iroha_data_model::jurisdiction::JdgAttestationScope",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgStateAccessSet>(
        "iroha_data_model::jurisdiction::JdgStateAccessSet",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgVerdict>(
        "iroha_data_model::jurisdiction::JdgVerdict",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgSignatureScheme>(
        "iroha_data_model::jurisdiction::JdgSignatureScheme",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgThresholdSignature>(
        "iroha_data_model::jurisdiction::JdgThresholdSignature",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgSdnCommitment>(
        "iroha_data_model::jurisdiction::JdgSdnCommitment",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgSdnCommitmentSignable>(
        "iroha_data_model::jurisdiction::JdgSdnCommitmentSignable",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgSdnRotationPolicy>(
        "iroha_data_model::jurisdiction::JdgSdnRotationPolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgSdnPolicy>(
        "iroha_data_model::jurisdiction::JdgSdnPolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgSdnKeyRecord>(
        "iroha_data_model::jurisdiction::JdgSdnKeyRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgCommitteeId>(
        "iroha_data_model::jurisdiction::JdgCommitteeId",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgAttestation>(
        "iroha_data_model::jurisdiction::JdgAttestation",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::JdgAttestationSignable>(
        "iroha_data_model::jurisdiction::JdgAttestationSignable",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
