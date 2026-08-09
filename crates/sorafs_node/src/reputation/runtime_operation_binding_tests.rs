// Reputation operation binding and publisher-key regressions.

#[test]
fn operation_keys_bind_phase_material_and_signed_result() {
    let signing = publication_idempotency_key(
        b"sorafs-reputation-threshold-signing-operation-v1",
        1,
        [0xC1; 32],
        None,
    )
    .expect("signing key");
    let governance = publication_idempotency_key(
        b"sorafs-reputation-governance-publication-operation-v1",
        1,
        [0xC1; 32],
        Some([0xC2; 32]),
    )
    .expect("governance key");
    let substituted = publication_idempotency_key(
        b"sorafs-reputation-governance-publication-operation-v1",
        1,
        [0xC1; 32],
        Some([0xC3; 32]),
    )
    .expect("substituted key");
    assert_ne!(signing, governance);
    assert_ne!(governance, substituted);
}

#[test]
fn governance_publisher_key_rejects_weak_and_noncanonical_points() {
    let mut weak_identity = [0_u8; 32];
    weak_identity[0] = 1;
    assert!(!valid_ed25519_verifying_key(weak_identity));
    assert!(!valid_ed25519_verifying_key([0xFF; 32]));
    assert!(valid_ed25519_verifying_key(
        SigningKey::from_bytes(&[0xD1; 32])
            .verifying_key()
            .to_bytes()
    ));
}
