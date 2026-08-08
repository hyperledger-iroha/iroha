// Governance ballot-v1 hard-cut and canonical-shape regressions.

#[tokio::test]
async fn ballot_zk_v1_rejects_alias_keys_in_raw_json() {
    let (state, _queue, chain_id) = mk_basic_context();
    let chain_id_str = chain_id.as_str().to_string();
    let envelope_b64 = base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]);
    let root_hint = hex::encode([0u8; 32]);
    let dto = super::ZkBallotV1Dto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        chain_id: chain_id_str.clone(),
        election_id: "ref-1".to_string(),
        backend: "halo2/ipa".to_string(),
        envelope_b64: envelope_b64.clone(),
        root_hint: Some(root_hint.clone()),
        owner: None,
        amount: None,
        duration_blocks: None,
        direction: None,
        nullifier: None,
    };
    let root_hint_alias = root_hint.clone();
    let raw = Bytes::from(
        norito::json::to_vec(&norito::json!({
            "authority": ACCOUNT_AUTHORITY,
            "chain_id": chain_id_str,
            "election_id": "ref-1",
            "backend": "halo2/ipa",
            "envelope_b64": envelope_b64,
            "root_hint": root_hint_alias,
            "rootHintHex": root_hint,
        }))
        .unwrap(),
    );
    let res = super::handle_gov_ballot_zk_v1(
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
        Some("public inputs must use root_hint (unsupported key rootHintHex)")
    );
}

#[tokio::test]
async fn ballot_zk_v1_rejects_noncanonical_owner_hint() {
    let (state, _queue, chain_id) = mk_basic_context();
    let chain_id_str = chain_id.as_str().to_string();
    let owner = noncanonical_literal(ACCOUNT_AUTHORITY);
    let dto = super::ZkBallotV1Dto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        chain_id: chain_id_str,
        election_id: "ref-1".to_string(),
        backend: "halo2/ipa".to_string(),
        envelope_b64: base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]),
        root_hint: None,
        owner: Some(owner),
        amount: Some(100_u64.into()),
        duration_blocks: Some(200),
        direction: None,
        nullifier: None,
    };
    let raw = Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
    let res = super::handle_gov_ballot_zk_v1(
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
async fn zk_v1_handlers_reject_noncanonical_direction() {
    use iroha_data_model::isi::governance::BallotProof;

    let (state, _queue, chain_id) = mk_basic_context();
    let chain_id_str = chain_id.as_str().to_string();
    let envelope_b64 = base64::engine::general_purpose::STANDARD.encode([1_u8, 2, 3, 4]);
    let dto = super::ZkBallotV1Dto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        chain_id: chain_id_str.clone(),
        election_id: "ref-1".to_string(),
        backend: "halo2/ipa".to_string(),
        envelope_b64,
        root_hint: None,
        owner: None,
        amount: None,
        duration_blocks: None,
        direction: Some("aye".to_string()),
        nullifier: None,
    };
    let raw = Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
    let response = super::handle_gov_ballot_zk_v1(
        chain_id.clone(),
        state.clone(),
        MaybeTelemetry::disabled(),
        crate::NoritoJsonWithBytes { value: dto, raw },
    )
    .await
    .expect("handler response");
    assert!(!response.0.accepted);
    assert_eq!(
        response.0.reason.as_deref(),
        Some("direction must be Aye, Nay, or Abstain")
    );

    let dto = super::ZkBallotV1BallotProofDto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        chain_id: chain_id_str,
        election_id: "ref-1".to_string(),
        ballot: BallotProof {
            backend: "halo2/ipa".into(),
            envelope_bytes: vec![1, 2, 3, 4],
            root_hint: None,
            owner: None,
            nullifier: None,
            amount: None,
            duration_blocks: None,
            direction: Some("Approve".to_string()),
        },
    };
    let raw = Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
    let response = super::handle_gov_ballot_zk_v1_ballotproof(
        chain_id,
        state,
        MaybeTelemetry::disabled(),
        crate::NoritoJsonWithBytes { value: dto, raw },
    )
    .await
    .expect("handler response");
    assert!(!response.0.accepted);
    assert_eq!(
        response.0.reason.as_deref(),
        Some("direction must be Aye, Nay, or Abstain")
    );
}

#[tokio::test]
async fn zk_v1_handlers_reject_non_token_backends_before_context_lookup() {
    use iroha_data_model::isi::governance::BallotProof;

    let (state, _queue, chain_id) = mk_basic_context();
    for backend in ["", " halo2/ipa", "halo2/ipa ", "halo2 ipa", "halo2\nipa"] {
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: "deliberately-wrong-chain".to_owned(),
            election_id: "ref-1".to_owned(),
            backend: backend.to_owned(),
            envelope_b64: base64::engine::general_purpose::STANDARD.encode([1_u8, 2, 3, 4]),
            root_hint: None,
            owner: None,
            amount: None,
            duration_blocks: None,
            direction: None,
            nullifier: None,
        };
        let raw =
            Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
        let response = super::handle_gov_ballot_zk_v1(
            chain_id.clone(),
            state.clone(),
            MaybeTelemetry::disabled(),
            crate::NoritoJsonWithBytes { value: dto, raw },
        )
        .await
        .expect("backend rejection must precede chain lookup");
        assert!(!response.0.accepted, "backend `{backend:?}`");
        assert!(
            response.0.tx_instructions.is_empty(),
            "backend `{backend:?}`"
        );
        assert!(
            response
                .0
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("backend")),
            "backend `{backend:?}`"
        );

        let dto = super::ZkBallotV1BallotProofDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: "deliberately-wrong-chain".to_owned(),
            election_id: "ref-1".to_owned(),
            ballot: BallotProof {
                backend: backend.into(),
                envelope_bytes: vec![1, 2, 3, 4],
                root_hint: None,
                owner: None,
                nullifier: None,
                amount: None,
                duration_blocks: None,
                direction: None,
            },
        };
        let raw =
            Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
        let response = super::handle_gov_ballot_zk_v1_ballotproof(
            chain_id.clone(),
            state.clone(),
            MaybeTelemetry::disabled(),
            crate::NoritoJsonWithBytes { value: dto, raw },
        )
        .await
        .expect("backend rejection must precede chain lookup");
        assert!(!response.0.accepted, "backend `{backend:?}`");
        assert!(
            response.0.tx_instructions.is_empty(),
            "backend `{backend:?}`"
        );
        assert!(
            response
                .0
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("backend")),
            "backend `{backend:?}`"
        );
    }
}

#[tokio::test]
async fn ballot_zk_v1_ballotproof_builds_instruction_skeleton() {
    use axum::{Router, routing::post};
    use http_body_util::BodyExt as _;
    use iroha_data_model::isi::governance::BallotProof;
    use tower::ServiceExt as _;

    let (state, _queue, chain_id) = mk_basic_context();
    let chain_id_str = chain_id.as_str().to_string();
    // Route for zk-v1/ballot-proof
    let app = Router::new().route(
        "/v1/gov/ballots/zk-v1/ballot-proof",
        post({
            let state = state.clone();
            let chain_id = chain_id.clone();
            move |body: crate::NoritoJsonWithBytes<super::ZkBallotV1BallotProofDto>| {
                let telemetry = MaybeTelemetry::disabled();
                async move {
                    super::handle_gov_ballot_zk_v1_ballotproof(chain_id, state, telemetry, body)
                        .await
                }
            }
        }),
    );

    // Build DTO
    let owner = canonical_literal(ACCOUNT_AUTHORITY);
    let ballot = BallotProof {
        backend: "halo2/ipa".into(),
        envelope_bytes: vec![1u8, 2, 3, 4],
        root_hint: Some([0xAA; 32]),
        owner: Some(
            AccountId::parse_encoded(&owner)
                .expect("valid account id")
                .into_account_id(),
        ),
        nullifier: Some([0x11; 32]),
        amount: Some(200_u64.into()),
        duration_blocks: Some(256),
        direction: Some("Nay".to_string()),
    };
    let dto = super::ZkBallotV1BallotProofDto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        chain_id: chain_id_str,
        election_id: "ref-1".to_string(),
        ballot,
    };
    let req = http::Request::builder()
        .method("POST")
        .uri("/v1/gov/ballots/zk-v1/ballot-proof")
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(
            norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap(),
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), http::StatusCode::OK);
    let b = resp.into_body().collect().await.unwrap().to_bytes();
    let v: norito::json::Value = norito::json::from_slice(&b).unwrap();
    assert_eq!(
        v.get("ok").and_then(norito::json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        v.get("accepted").and_then(norito::json::Value::as_bool),
        Some(true)
    );
    assert!(
        v.get("tx_instructions")
            .and_then(|x| x.as_array())
            .is_some()
    );
}

#[tokio::test]
async fn ballot_zk_v1_ballotproof_rejects_alias_keys_in_raw_json() {
    use iroha_data_model::isi::governance::BallotProof;

    let (state, _queue, chain_id) = mk_basic_context();
    let chain_id_str = chain_id.as_str().to_string();
    let envelope_b64 = base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]);
    let root_hint = hex::encode([0xAAu8; 32]);
    let ballot = BallotProof {
        backend: "halo2/ipa".into(),
        envelope_bytes: vec![1u8, 2, 3, 4],
        root_hint: Some([0xAAu8; 32]),
        owner: None,
        nullifier: None,
        amount: None,
        duration_blocks: None,
        direction: None,
    };
    let dto = super::ZkBallotV1BallotProofDto {
        authority: ACCOUNT_AUTHORITY.to_string(),
        chain_id: chain_id_str.clone(),
        election_id: "ref-1".to_string(),
        ballot,
    };
    let root_hint_alias = root_hint.clone();
    let raw = Bytes::from(
        norito::json::to_vec(&norito::json!({
            "authority": ACCOUNT_AUTHORITY,
            "chain_id": chain_id_str,
            "election_id": "ref-1",
            "ballot": {
                "backend": "halo2/ipa",
                "envelope_bytes": envelope_b64,
                "root_hint": root_hint_alias,
                "rootHintHex": root_hint,
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
        Some("public inputs must use root_hint (unsupported key rootHintHex)")
    );
}
