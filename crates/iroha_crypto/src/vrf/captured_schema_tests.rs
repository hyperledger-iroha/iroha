//! Compiler-captured identities for this module’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::VrfProof>(
        "iroha_crypto::vrf::VrfProof",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::VrfOutput>(
        "iroha_crypto::vrf::VrfOutput",
    );
}
