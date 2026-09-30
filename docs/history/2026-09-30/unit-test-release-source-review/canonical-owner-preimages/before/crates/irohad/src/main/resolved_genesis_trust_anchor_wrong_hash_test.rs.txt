#[test]
fn resolved_genesis_trust_anchor_rejects_wrong_hash() {
    let keypair = KeyPair::random();
    let genesis = prepared_genesis_proposal(&keypair);
    let wrong_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; 32]));
    assert_ne!(genesis.0.hash(), wrong_hash);
    let anchor = ResolvedGenesisTrustAnchor {
        public_key: keypair.public_key().clone(),
        consensus_header_hash: wrong_hash,
    };
    let error = anchor
        .verify(&genesis)
        .expect_err("configured genesis hash mismatch must reject genesis");
    assert!(matches!(error.current_context(), StartError::InitKura));
    assert!(
        format!("{error:?}").contains("does not match the resolved genesis trust-anchor hash"),
        "unexpected mismatch diagnostic: {error:?}"
    );
}
