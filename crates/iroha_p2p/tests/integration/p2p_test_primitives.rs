#[derive(Clone, Debug, Decode, Encode)]
struct ConsensusMessage(u32);
impl iroha_p2p::network::message::ClassifyTopic for ConsensusMessage {
    fn topic(&self) -> iroha_p2p::network::message::Topic {
        iroha_p2p::network::message::Topic::Consensus
    }
}
fn setup_logger() {
    test_logger();
}
fn default_soranet_handshake() -> ActualSoranetHandshake {
    // Admission remains mandatory; the shared fixture uses the minimum valid
    // Argon2 cost so general network timing tests stay focused and bounded.
    super::mandatory_test_soranet_handshake()
}
#[test]
fn test_encryption() {
    use iroha_crypto::encryption::{ChaCha20Poly1305, SymmetricEncryptor};
    const TEST_KEY: [u8; 32] = [
        5, 87, 82, 183, 220, 57, 107, 49, 227, 4, 96, 231, 198, 88, 153, 11, 22, 65, 56, 45, 237,
        35, 231, 165, 122, 153, 14, 68, 13, 84, 5, 24,
    ];
    let encryptor =
        SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(TEST_KEY).expect("valid key length");
    let message = b"Some ciphertext";
    let aad = b"Iroha3 AAD";
    let ciphertext = encryptor
        .encrypt_easy(aad.as_ref(), message.as_ref())
        .unwrap();
    let decrypted = encryptor
        .decrypt_easy(aad.as_ref(), ciphertext.as_slice())
        .unwrap();
    assert_eq!(decrypted.as_slice(), message);
}
