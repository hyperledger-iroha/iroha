//! Compiler-captured identities for this module’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::GuardDirectorySnapshotV2>(
        "iroha_crypto::soranet::directory::GuardDirectorySnapshotV2",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::GuardDirectoryIssuerV1>(
        "iroha_crypto::soranet::directory::GuardDirectoryIssuerV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::GuardDirectoryRelayEntryV2>(
        "iroha_crypto::soranet::directory::GuardDirectoryRelayEntryV2",
    );
}
