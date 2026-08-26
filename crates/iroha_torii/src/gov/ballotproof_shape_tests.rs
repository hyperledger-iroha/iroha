// Governance ballot-proof canonical-shape regressions.
#[tokio::test]
async fn ballot_zk_v1_ballotproof_rejects_noncanonical_owner_hint_in_raw_json() {
    use iroha_data_model::isi::governance::BallotProof;
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let network_id = *state.network_id_ref();
    let envelope_b64 = base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]);
    let owner_canonical = canonical_literal(ACCOUNT_AUTHORITY);
    let owner_noncanonical = noncanonical_literal(ACCOUNT_AUTHORITY);
    let ballot = BallotProof {
        backend: "halo2/ipa".into(),
        envelope_bytes: vec![1u8, 2, 3, 4],
        root_hint: None,
        owner: Some(AccountId::parse_encoded(&owner_canonical).expect("valid account id")),
        nullifier: None,
        amount: Some(200_u64.into()),
        duration_blocks: Some(256),
        direction: None,
    };
    let dto = super::ZkBallotV1BallotProofDto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        network_id,
        election_id: "ref-1".to_string(),
        ballot,
    };
    let raw = Bytes::from(
        norito::json::to_vec(&norito::json!({
            "authority": ACCOUNT_AUTHORITY,
            "network_id": network_id,
            "election_id": "ref-1",
            "ballot": {
                "backend": "halo2/ipa",
                "envelope_bytes": envelope_b64,
                "owner": owner_noncanonical,
                "amount": "200",
                "duration_blocks": 256,
            },
        }))
        .unwrap(),
    );
    let res = super::handle_gov_ballot_zk_v1_ballotproof(
        state,
        &authenticated,
        MaybeTelemetry::disabled(),
        crate::NoritoJsonWithBytes { value: dto, raw },
    )
    .await
    .expect("handler ok");
    let body = res.0;
    assert!(!body.ok);
    assert!(!body.accepted);
    assert_eq!(
        body.reason.as_deref(),
        Some("owner must use canonical I105 account id form")
    );
}
#[tokio::test]
async fn ballot_zk_v1_ballotproof_rejects_partial_lock_hints() {
    use iroha_data_model::isi::governance::BallotProof;
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let ballot = BallotProof {
        backend: "halo2/ipa".into(),
        envelope_bytes: vec![1u8, 2, 3, 4],
        root_hint: None,
        owner: Some(AccountId::parse_encoded(ACCOUNT_AUTHORITY).expect("valid account id")),
        nullifier: None,
        amount: None,
        duration_blocks: None,
        direction: None,
    };
    let dto = super::ZkBallotV1BallotProofDto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        network_id: *state.network_id_ref(),
        election_id: "ref-1".to_string(),
        ballot,
    };
    let raw = Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
    let res = super::handle_gov_ballot_zk_v1_ballotproof(
        state,
        &authenticated,
        MaybeTelemetry::disabled(),
        crate::NoritoJsonWithBytes { value: dto, raw },
    )
    .await
    .expect("handler ok");
    let body = res.0;
    assert!(!body.ok);
    assert!(!body.accepted);
    assert_eq!(
        body.reason.as_deref(),
        Some("lock hints must include owner, amount, duration_blocks")
    );
}
#[tokio::test]
async fn ballot_zk_v1_ballotproof_rejects_owner_hint_different_from_authority() {
    use iroha_data_model::isi::governance::BallotProof;
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let ballot = BallotProof {
        backend: "halo2/ipa".into(),
        envelope_bytes: vec![1_u8, 2, 3, 4],
        root_hint: None,
        owner: Some(canonical_account(ACCOUNT_OWNER_ALT)),
        nullifier: None,
        amount: Some(100_u64.into()),
        duration_blocks: Some(200),
        direction: None,
    };
    let dto = super::ZkBallotV1BallotProofDto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        network_id: *state.network_id_ref(),
        election_id: "ref-1".to_string(),
        ballot,
    };
    let raw = Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
    let response = super::handle_gov_ballot_zk_v1_ballotproof(
        state,
        &authenticated,
        MaybeTelemetry::disabled(),
        crate::NoritoJsonWithBytes { value: dto, raw },
    )
    .await
    .expect("handler response");
    assert!(!response.0.ok);
    assert!(!response.0.accepted);
    assert_eq!(
        response.0.reason.as_deref(),
        Some("owner must equal authority")
    );
    assert!(response.0.tx_instructions.is_empty());
}
