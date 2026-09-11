//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::WrappedAssetDef>(
        "iroha_data_model::bridge::WrappedAssetDef",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeReceipt>(
        "iroha_data_model::bridge::BridgeReceipt",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeHashFunction>(
        "iroha_data_model::bridge::BridgeHashFunction",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeProofRange>(
        "iroha_data_model::bridge::BridgeProofRange",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeIcsProof>(
        "iroha_data_model::bridge::BridgeIcsProof",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeTransparentProof>(
        "iroha_data_model::bridge::BridgeTransparentProof",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeNativeProofBackendV1>(
        "iroha_data_model::bridge::BridgeNativeProofBackendV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SccpNativeTrustAnchorV1>(
        "iroha_data_model::bridge::SccpNativeTrustAnchorV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeNativeProtocolProofV1>(
        "iroha_data_model::bridge::BridgeNativeProtocolProofV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeSccpDestinationProofBackendV1>(
        "iroha_data_model::bridge::BridgeSccpDestinationProofBackendV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeSccpDestinationProofV1>(
        "iroha_data_model::bridge::BridgeSccpDestinationProofV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeProofPayload>(
        "iroha_data_model::bridge::BridgeProofPayload",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeProof>(
        "iroha_data_model::bridge::BridgeProof",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeProofRecord>(
        "iroha_data_model::bridge::BridgeProofRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeFinalityProof>(
        "iroha_data_model::bridge::BridgeFinalityProof",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeFinalityAttestationBodyV1>(
        "iroha_data_model::bridge::BridgeFinalityAttestationBodyV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeFinalityAttestationV1>(
        "iroha_data_model::bridge::BridgeFinalityAttestationV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeCommitment>(
        "iroha_data_model::bridge::BridgeCommitment",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeFinalityBundle>(
        "iroha_data_model::bridge::BridgeFinalityBundle",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
