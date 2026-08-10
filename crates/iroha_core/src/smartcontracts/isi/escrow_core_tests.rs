// Same-scope regression coverage extracted to keep the parent source budget bounded.

#[test]
fn court_permission_constant_matches_typed_permission() {
    assert_eq!(
        CanResolveEscrowDispute::name().as_str(),
        CAN_RESOLVE_ESCROW_DISPUTE
    );
}

#[test]
fn custody_account_derivation_is_stable() {
    let network_id = escrow_test_network_id(1);
    let asset_definition: AssetDefinitionId =
        "61CtjvNd9T3THAR65GsMVHr82Bjc".parse().expect("asset");
    let escrow_id = EscrowId::new(Hash::new("escrow"));
    let custody = escrow_custody_account_id(&network_id, &escrow_id, &asset_definition)
        .expect("custody account derivation succeeds");
    assert_eq!(
        custody,
        escrow_custody_account_id(&network_id, &escrow_id, &asset_definition)
            .expect("custody account derivation is repeatable")
    );
    assert_ne!(
        custody,
        escrow_custody_account_id(&escrow_test_network_id(2), &escrow_id, &asset_definition,)
            .expect("different exact network custody derivation succeeds"),
        "same-label deployments with different genesis hashes need disjoint custody",
    );

    let mut public_seed_material = Vec::new();
    public_seed_material.extend_from_slice(ESCROW_CUSTODY_ACCOUNT_DOMAIN.as_bytes());
    public_seed_material.extend_from_slice(network_id.as_bytes());
    public_seed_material.extend_from_slice(escrow_id.as_hash().as_ref());
    public_seed_material.extend_from_slice(asset_definition.to_string().as_bytes());
    let public_seed: [u8; Hash::LENGTH] = Hash::new(public_seed_material).into();
    let public_seed_keypair = KeyPair::try_from_seed(public_seed.to_vec(), Algorithm::Ed25519)
        .expect("public seed derives");
    assert_ne!(
        custody,
        AccountId::new(public_seed_keypair.public_key().clone()),
        "protocol custody must not expose a signing key through public seed derivation"
    );
}

#[test]
fn every_public_escrow_creator_common_gate_rejects_both_orderbook_namespaces() {
    let order_lock = iroha_data_model::sorafs::orderbook::orderbook_order_escrow_id([0x41; 32]);
    let channel_lock =
        iroha_data_model::sorafs::orderbook::orderbook_settlement_escrow_id([0x42; 32]);
    for reserved in [order_lock, channel_lock] {
        let err = reject_reserved_orderbook_escrow_id(&reserved)
            .expect_err("reserved namespace must be native-only");
        assert!(
            err.to_string().contains("reserved SoraFS orderbook"),
            "unexpected error: {err}"
        );
    }
    assert!(
        reject_reserved_orderbook_escrow_id(&fixture_escrow_id("ordinary-public-escrow")).is_ok()
    );
}

#[test]
fn resolution_split_must_equal_escrow_amount() {
    let total = Quantity::from(100_u32);
    assert!(
        ensure_resolution_split(&total, &Quantity::from(40_u32), &Quantity::from(60_u32)).is_ok()
    );
    assert!(
        ensure_resolution_split(&total, &Quantity::from(40_u32), &Quantity::from(59_u32)).is_err()
    );
    assert!(
        Quantity::try_from_numeric(Numeric::new(-1_i32, 0)).is_err(),
        "negative escrow splits must fail at the nominal quantity boundary"
    );
}

#[test]
fn anonymous_escrow_byte_guards_reject_empty_zero_and_duplicate_values() {
    assert!(ensure_unique_non_zero_bytes("test", &[[0x01; 32]]).is_ok());
    assert!(ensure_unique_non_zero_bytes("test", &[]).is_err());
    assert!(ensure_unique_non_zero_bytes("test", &[[0; 32]]).is_err());
    assert!(ensure_unique_non_zero_bytes("test", &[[0x01; 32], [0x01; 32]]).is_err());
    assert!(ensure_single_escrow_nullifier(&[[0x01; 32]]).is_ok());
    assert!(ensure_single_escrow_nullifier(&[[0x01; 32], [0x02; 32]]).is_err());
}

#[test]
fn anonymous_escrow_close_proof_must_bind_stored_commitment() {
    let escrow_commitment = [0x22; 32];

    let matching = anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]);
    ensure_close_proof_spends_escrow_commitment(&matching, escrow_commitment)
        .expect("matching close proof must pass");

    let wrong = anonymous_close_proof_with_input_commitments([[0x44; 32], [0; 32]]);
    let err = ensure_close_proof_spends_escrow_commitment(&wrong, escrow_commitment)
        .expect_err("wrong close proof input commitment must fail");
    assert!(
        err.to_string().contains("input commitment mismatch"),
        "unexpected error: {err}"
    );

    let missing = anonymous_close_proof_with_input_commitments([[0; 32], [0; 32]]);
    let err = ensure_close_proof_spends_escrow_commitment(&missing, escrow_commitment)
        .expect_err("close proof without a non-zero input must fail");
    assert!(
        err.to_string().contains("exactly one escrow commitment"),
        "unexpected error: {err}"
    );

    let extra = anonymous_close_proof_with_input_commitments([escrow_commitment, [0x55; 32]]);
    let err = ensure_close_proof_spends_escrow_commitment(&extra, escrow_commitment)
        .expect_err("close proof with multiple non-zero inputs must fail");
    assert!(
        err.to_string().contains("exactly one escrow commitment"),
        "unexpected error: {err}"
    );
}

#[test]
fn anonymous_escrow_close_proof_rejects_noncanonical_envelope_before_public_input_trust() {
    let escrow_commitment = [0x22; 32];

    for (suffix, proof, expected_msg) in [
        (
            "backend_tag",
            tamper_anonymous_close_proof_envelope(
                anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]),
                |envelope| envelope.backend = BackendTag::Stark,
            ),
            "unexpected OpenVerifyEnvelope backend tag",
        ),
        (
            "circuit_id",
            tamper_anonymous_close_proof_envelope(
                anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]),
                |envelope| envelope.circuit_id = "halo2/pasta/ipa/vote-ballot".to_owned(),
            ),
            "requires confidential transfer v2 circuit",
        ),
        (
            "schema",
            tamper_anonymous_close_proof_envelope(
                anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]),
                |envelope| envelope.public_inputs = b"wrong-schema".to_vec(),
            ),
            "public inputs schema mismatch",
        ),
        (
            "aux",
            tamper_anonymous_close_proof_envelope(
                anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]),
                |envelope| envelope.aux = b"side-channel".to_vec(),
            ),
            "envelope auxiliary bytes must be empty",
        ),
        (
            "zero_vk_hash",
            tamper_anonymous_close_proof_envelope(
                anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]),
                |envelope| envelope.vk_hash = [0u8; 32],
            ),
            "verifier key hash must be non-zero",
        ),
        (
            "vk_commitment",
            {
                let mut proof =
                    anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]);
                proof.vk_commitment = Some([0x55; 32]);
                proof
            },
            "verifier key commitment mismatch",
        ),
        (
            "attachment_backend",
            {
                let mut proof =
                    anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]);
                let bytes = proof.proof.bytes.clone();
                proof.proof =
                    iroha_data_model::proof::ProofBox::new("halo2/ipa/other".into(), bytes);
                proof
            },
            "backend mismatch",
        ),
        (
            "verifier_key_backend",
            {
                let mut proof =
                    anonymous_close_proof_with_input_commitments([escrow_commitment, [0; 32]]);
                proof.vk_ref = iroha_data_model::proof::VerifyingKeyId::new(
                    "stark/fri/sha256-goldilocks",
                    "anonymous_escrow",
                );
                proof
            },
            "verifier-key backend mismatch",
        ),
    ] {
        let err = ensure_close_proof_spends_escrow_commitment(&proof, escrow_commitment)
            .expect_err("noncanonical close proof should fail before public input trust");
        let msg = err.to_string();
        assert!(
            msg.contains(expected_msg),
            "case {suffix}: expected {expected_msg:?}, got {msg:?}"
        );
    }
}
