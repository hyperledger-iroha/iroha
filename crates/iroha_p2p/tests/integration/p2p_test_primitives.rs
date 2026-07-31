fn setup_logger() {
    test_logger();
}

fn default_soranet_handshake() -> ActualSoranetHandshake {
    // Admission-puzzle behavior has dedicated integration coverage. General
    // network timing tests disable it so they measure the behavior they name.
    let pow = SoranetPow {
        required: false,
        puzzle: None,
        ..SoranetPow::default()
    };
    ActualSoranetHandshake {
        descriptor_commit: WithOrigin::inline(DEFAULT_DESCRIPTOR_COMMIT.to_vec()),
        client_capabilities: WithOrigin::inline(DEFAULT_CLIENT_CAPABILITIES.to_vec()),
        relay_capabilities: WithOrigin::inline(DEFAULT_RELAY_CAPABILITIES.to_vec()),
        trust_gossip: true,
        kem_id: 1,
        sig_id: 1,
        resume_hash: None,
        pow,
    }
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
    let aad = b"Iroha2 AAD";
    let ciphertext = encryptor
        .encrypt_easy(aad.as_ref(), message.as_ref())
        .unwrap();
    let decrypted = encryptor
        .decrypt_easy(aad.as_ref(), ciphertext.as_slice())
        .unwrap();
    assert_eq!(decrypted.as_slice(), message);
}
