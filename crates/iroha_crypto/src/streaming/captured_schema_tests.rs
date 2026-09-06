//! Compiler-captured identities for this module’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<
        super::TransportCapabilityResolutionSnapshot,
    >("iroha_crypto::streaming::TransportCapabilityResolutionSnapshot");
    crate::captured_schema_tests::assert_bidirectional::<super::StreamingSessionSnapshot>(
        "iroha_crypto::streaming::StreamingSessionSnapshot",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SessionCadenceSnapshot>(
        "iroha_crypto::streaming::SessionCadenceSnapshot",
    );
    crate::captured_schema_tests::assert_serialize::<super::KeyUpdateTranscript>(
        "iroha_crypto::streaming::KeyUpdateTranscript",
    );
}
