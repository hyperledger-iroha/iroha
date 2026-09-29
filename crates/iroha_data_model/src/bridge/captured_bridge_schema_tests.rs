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
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeProofPayload>(
        "iroha_data_model::bridge::BridgeProofPayload",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeProof>(
        "iroha_data_model::bridge::BridgeProof",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BridgeProofRecord>(
        "iroha_data_model::bridge::BridgeProofRecord",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

crate::captured_schema_tests::native_capture::owner_printer!(CASES);
