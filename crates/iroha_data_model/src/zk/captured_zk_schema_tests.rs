//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::BackendTag>(
        "iroha_data_model::zk::BackendTag",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::OpenVerifyEnvelope>(
        "iroha_data_model::zk::OpenVerifyEnvelope",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::StarkFriOpenProofV1>(
        "iroha_data_model::zk::StarkFriOpenProofV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ZkAcePrivacyPublicInputsV1>(
        "iroha_data_model::zk::ZkAcePrivacyPublicInputsV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ZkAcePackedBytesV1>(
        "iroha_data_model::zk::ZkAcePackedBytesV1",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
