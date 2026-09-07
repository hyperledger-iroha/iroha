//! Compiler-captured identities for this module’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::LaneCommitmentId>(
        "iroha_crypto::privacy::LaneCommitmentId",
    );
}
