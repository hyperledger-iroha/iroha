//! Compiler-captured identities for this module’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::ReplayLedgerSnapshotV1>(
        "iroha_crypto::soranet::replay::ReplayLedgerSnapshotV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ReplayLedgerSnapshotEntryV1>(
        "iroha_crypto::soranet::replay::ReplayLedgerSnapshotEntryV1",
    );
}
