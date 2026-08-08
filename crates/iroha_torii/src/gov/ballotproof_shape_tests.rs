// Governance ballot-proof canonical-shape regressions.

#[tokio::test]
async fn ballot_zk_v1_ballotproof_rejects_noncanonical_owner_hint_in_raw_json() {
    use iroha_data_model::isi::governance::BallotProof;

    let (state, _queue, chain_id) = mk_basic_context();
    let chain_id_str = chain_id.as_str().to_string();
    let envelope_b64 = base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]);
    let owner_canonical = canonical_literal(ACCOUNT_AUTHORITY);
    let owner_noncanonical = noncanonical_literal(ACCOUNT_AUTHORITY);
    let ballot = BallotProof {
        backend: "halo2/ipa".into(),
        envelope_bytes: vec![1u8, 2, 3, 4],
        root_hint: None,
        owner: Some(
            AccountId::parse_encoded(&owner_canonical)
                .expect("valid account id")
                .into_account_id(),
        ),
        nullifier: None,
        amount: Some(200_u64.into()),
        duration_blocks: Some(256),
        direction: None,
    };
    let dto = super::ZkBallotV1BallotProofDto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        chain_id: chain_id_str.clone(),
        election_id: "ref-1".to_string(),
        ballot,
    };
    let raw = Bytes::from(
        norito::json::to_vec(&norito::json!({
            "authority": ACCOUNT_AUTHORITY,
            "chain_id": chain_id_str,
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
        chain_id,
        state,
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

    let (state, _queue, chain_id) = mk_basic_context();
    let chain_id_str = chain_id.as_str().to_string();
    let ballot = BallotProof {
        backend: "halo2/ipa".into(),
        envelope_bytes: vec![1u8, 2, 3, 4],
        root_hint: None,
        owner: Some(
            AccountId::parse_encoded(ACCOUNT_AUTHORITY)
                .expect("valid account id")
                .into_account_id(),
        ),
        nullifier: None,
        amount: None,
        duration_blocks: None,
        direction: None,
    };
    let dto = super::ZkBallotV1BallotProofDto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        chain_id: chain_id_str,
        election_id: "ref-1".to_string(),
        ballot,
    };
    let raw = Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
    let res = super::handle_gov_ballot_zk_v1_ballotproof(
        chain_id,
        state,
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
