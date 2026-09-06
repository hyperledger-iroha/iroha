//! Compiler-captured identities for this module’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::TokenStoreEntry>(
        "iroha_crypto::soranet::token::TokenStoreEntry",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::TokenStoreSnapshot>(
        "iroha_crypto::soranet::token::TokenStoreSnapshot",
    );
}
