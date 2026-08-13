// Exact NetworkId governance ballot and capability regressions.
fn foreign_network_id() -> iroha_data_model::NetworkId {
    "hash:0000000000000000000000000000000000000000000000000000000000000003#E54C"
        .parse()
        .expect("canonical foreign network id")
}
#[tokio::test]
async fn governance_ballots_require_exact_network_before_semantic_validation() {
    use iroha_data_model::isi::governance::BallotProof;
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let canonical_authority = canonical_literal(ACCOUNT_AUTHORITY);
    let proof_b64 = base64::engine::general_purpose::STANDARD.encode(b"proof");
    let expected_network = *state.network_id_ref();
    let foreign_network = foreign_network_id();
    ensure_network_id_matches(state.as_ref(), &expected_network)
        .expect("exact network id must remain valid");
    let error = ensure_network_id_matches(state.as_ref(), &foreign_network)
        .expect_err("a distinct genesis-derived network must fail closed");
    assert!(format!("{error:?}").contains("different network"));
    let error = handle_gov_ballot_plain_with_policy(
        state.clone(),
        &authenticated,
        NoritoJson(PlainBallotDto {
            authority: canonical_authority.clone(),
            network_id: foreign_network,
            referendum_id: "referendum-1 ".to_owned(),
            owner: canonical_authority.clone(),
            amount: 1_u64.into(),
            duration_blocks: "0".to_owned(),
            direction: "Aye".to_owned(),
        }),
        MaybeTelemetry::disabled(),
    )
    .await
    .expect_err("foreign PLAIN ballot must fail before selector validation");
    assert!(format!("{error:?}").contains("different network"));
    let error = handle_gov_parliament_ballot(
        state.clone(),
        &authenticated,
        MaybeTelemetry::disabled(),
        NoritoJson(ParliamentBallotDto {
            authority: canonical_authority.clone(),
            network_id: foreign_network,
            proposal_id: "not canonical hex".to_owned(),
            body: ParliamentBody::PolicyJury,
            decision: ParliamentDecision::Approve,
        }),
    )
    .await
    .expect_err("foreign Parliament ballot must fail before proposal validation");
    assert!(format!("{error:?}").contains("different network"));
    let dto = ZkBallotV1Dto {
        authority: canonical_authority.clone(),
        network_id: foreign_network,
        election_id: "election-1\n".to_owned(),
        backend: "halo2/ipa".to_owned(),
        envelope_b64: proof_b64,
        root_hint: None,
        owner: None,
        amount: None,
        duration_blocks: None,
        direction: None,
        nullifier: None,
    };
    let raw = Bytes::from(
        norito::json::to_vec(&norito::json::to_value(&dto).expect("serialize dto"))
            .expect("encode dto"),
    );
    let error = handle_gov_ballot_zk_v1(
        state.clone(),
        &authenticated,
        MaybeTelemetry::disabled(),
        crate::NoritoJsonWithBytes { value: dto, raw },
    )
    .await
    .expect_err("foreign ZK envelope ballot must fail before selector validation");
    assert!(format!("{error:?}").contains("different network"));
    let dto = ZkBallotV1BallotProofDto {
        authority: canonical_authority,
        network_id: foreign_network,
        election_id: " election-1".to_owned(),
        ballot: BallotProof {
            backend: "halo2/ipa".into(),
            envelope_bytes: vec![1, 2, 3],
            root_hint: None,
            owner: None,
            nullifier: None,
            amount: None,
            duration_blocks: None,
            direction: None,
        },
    };
    let raw = Bytes::from(
        norito::json::to_vec(&norito::json::to_value(&dto).expect("serialize dto"))
            .expect("encode dto"),
    );
    let error = handle_gov_ballot_zk_v1_ballotproof(
        state,
        &authenticated,
        MaybeTelemetry::disabled(),
        crate::NoritoJsonWithBytes { value: dto, raw },
    )
    .await
    .expect_err("foreign ZK proof ballot must fail before selector validation");
    assert!(format!("{error:?}").contains("different network"));
}
#[tokio::test]
async fn governance_ballots_bind_authenticated_account_before_semantic_validation() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let authenticated = canonical_account(ACCOUNT_OWNER_ALT);
    let error = handle_gov_ballot_plain_with_policy(
        state.clone(),
        &authenticated,
        NoritoJson(PlainBallotDto {
            authority: canonical_literal(ACCOUNT_AUTHORITY),
            network_id: *state.network_id_ref(),
            referendum_id: "referendum-1 ".to_owned(),
            owner: canonical_literal(ACCOUNT_AUTHORITY),
            amount: 1_u64.into(),
            duration_blocks: "0".to_owned(),
            direction: "Aye".to_owned(),
        }),
        MaybeTelemetry::disabled(),
    )
    .await
    .expect_err("a ballot authority distinct from its authenticated account must fail closed");
    assert!(format!("{error:?}").contains("authenticated account"));
}
#[tokio::test]
async fn governance_ballot_dtos_reject_retired_identity_keys() {
    use iroha_data_model::isi::governance::BallotProof;
    let (state, _queue, _chain_id) = mk_basic_context();
    let authority = canonical_literal(ACCOUNT_AUTHORITY);
    let network_id = *state.network_id_ref();
    let plain = norito::json::to_value(&PlainBallotDto {
        authority: authority.clone(),
        network_id,
        referendum_id: "r1".to_owned(),
        owner: authority.clone(),
        amount: 1_u64.into(),
        duration_blocks: "0".to_owned(),
        direction: "Aye".to_owned(),
    })
    .expect("serialize exact PLAIN ballot");
    for retired in ["chain_id", "genesis_hash"] {
        let mut with_retired = plain.clone();
        with_retired
            .as_object_mut()
            .expect("PLAIN ballot object")
            .insert(retired.into(), norito::json::Value::from("retired"));
        assert!(norito::json::from_value::<PlainBallotDto>(with_retired).is_err());
    }
    let parliament = norito::json::to_value(&ParliamentBallotDto {
        authority: authority.clone(),
        network_id,
        proposal_id: "11".repeat(32),
        body: ParliamentBody::PolicyJury,
        decision: ParliamentDecision::Approve,
    })
    .expect("serialize exact Parliament ballot");
    for retired in ["chain_id", "genesis_hash"] {
        let mut with_retired = parliament.clone();
        with_retired
            .as_object_mut()
            .expect("Parliament ballot object")
            .insert(retired.into(), norito::json::Value::from("retired"));
        assert!(norito::json::from_value::<ParliamentBallotDto>(with_retired).is_err());
    }
    let envelope = norito::json::to_value(&ZkBallotV1Dto {
        authority: authority.clone(),
        network_id,
        election_id: "election-1".to_owned(),
        backend: "halo2/ipa".to_owned(),
        envelope_b64: base64::engine::general_purpose::STANDARD.encode(b"proof"),
        root_hint: None,
        owner: None,
        amount: None,
        duration_blocks: None,
        direction: None,
        nullifier: None,
    })
    .expect("serialize exact ZK envelope ballot");
    for retired in ["chain_id", "genesis_hash"] {
        let mut with_retired = envelope.clone();
        with_retired
            .as_object_mut()
            .expect("ZK envelope ballot object")
            .insert(retired.into(), norito::json::Value::from("retired"));
        assert!(norito::json::from_value::<ZkBallotV1Dto>(with_retired).is_err());
    }
    let proof = norito::json::to_value(&ZkBallotV1BallotProofDto {
        authority,
        network_id,
        election_id: "election-1".to_owned(),
        ballot: BallotProof {
            backend: "halo2/ipa".into(),
            envelope_bytes: vec![1, 2, 3],
            root_hint: None,
            owner: None,
            nullifier: None,
            amount: None,
            duration_blocks: None,
            direction: None,
        },
    })
    .expect("serialize exact ZK proof ballot");
    for retired in ["chain_id", "genesis_hash"] {
        let mut with_retired = proof.clone();
        with_retired
            .as_object_mut()
            .expect("ZK proof ballot object")
            .insert(retired.into(), norito::json::Value::from("retired"));
        assert!(norito::json::from_value::<ZkBallotV1BallotProofDto>(with_retired).is_err());
    }
}
#[tokio::test]
async fn governance_capabilities_expose_one_exact_network_identity() {
    let harness = mk_governance_harness(true);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let block = harness.state.block(header);
    block
        .commit()
        .expect("commit genesis-like capabilities fixture");
    let expected_network = *harness.state.network_id_ref();
    let response = handle_gov_capabilities(harness.state)
        .await
        .expect("capabilities after committed genesis")
        .0;
    assert_eq!(response.network_id, expected_network);
    let encoded = norito::json::to_value(&response).expect("serialize capabilities");
    let object = encoded.as_object().expect("capabilities object");
    assert!(object.contains_key("network_id"));
    assert!(!object.contains_key("chain_id"));
    assert!(!object.contains_key("genesis_hash"));
}
