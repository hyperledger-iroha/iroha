// Keep the streamed public-reserve key identical to its original framed Norito preimage.

#[test]
fn public_reserve_key_streaming_preserves_exact_framed_asset_id_digest() {
    let fixture = orchard_persisted_fixture();
    let state = fixture.state();
    let original = AssetId::with_scope(
        state.asset_definition_id().clone(),
        state.reserve_account().clone(),
        state.public_balance_scope(),
    );
    let other_account = AssetId::with_scope(
        original.definition().clone(),
        account(0xD7),
        *original.scope(),
    );
    let mut previous_digest = None;
    for asset_id in [original, other_account] {
        let original_frame = norito::to_bytes(&asset_id).expect("original Norito frame");
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_PUBLIC_RESERVE_ASSET_DIGEST_DOMAIN_V1);
        hasher.update(&original_frame);
        let expected_digest = *hasher.finalize().as_bytes();
        assert_ne!(previous_digest, Some(expected_digest));
        previous_digest = Some(expected_digest);
        for protocol_id in PRIVACY_PUBLIC_RESERVE_PROTOCOLS_V1 {
            assert_eq!(
                PrivacyCommitmentKeyV1::public_reserve_custody(protocol_id, &asset_id)
                    .expect("streamed reserve key"),
                PrivacyCommitmentKeyV1::PublicReserveCustody {
                    protocol_id,
                    reserve_asset_digest: expected_digest,
                }
            );
        }
    }
}

#[test]
fn public_reserve_key_ignores_ambient_norito_layout_flags() {
    let state = orchard_persisted_fixture().state();
    let asset_id = AssetId::with_scope(
        state.asset_definition_id().clone(),
        state.reserve_account().clone(),
        state.public_balance_scope(),
    );
    let protocol = PrivacyProtocolIdV1::OrchardHalo2ActionsV1;
    let canonical = PrivacyCommitmentKeyV1::public_reserve_custody(protocol, &asset_id)
        .expect("canonical custody key");
    let alternate = norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
    let under_alternate_layout = {
        let _guard = norito::core::DecodeFlagsGuard::enter(alternate);
        PrivacyCommitmentKeyV1::public_reserve_custody(protocol, &asset_id)
            .expect("ambient flags must not change custody key")
    };
    assert_eq!(under_alternate_layout, canonical);
}
