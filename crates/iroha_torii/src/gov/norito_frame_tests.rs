// Explicit governance frame identity through the real extractor and ballot policy.
fn plain_ballot_frame_fixture(state: &State) -> PlainBallotDto {
    let authority = canonical_literal(ACCOUNT_AUTHORITY);
    PlainBallotDto {
        authority: authority.clone(),
        network_id: *state.network_id_ref(),
        referendum_id: "r1".to_owned(),
        owner: authority,
        amount: 100_u64.into(),
        duration_blocks: "600".to_owned(),
        direction: "Aye".to_owned(),
    }
}
async fn extract_plain_ballot_frame(
    bytes: Vec<u8>,
) -> Result<NoritoJson<PlainBallotDto>, axum::response::Response> {
    use axum::extract::FromRequest as _;
    let request = axum::http::Request::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/x-norito")
        .body(axum::body::Body::from(bytes))
        .expect("framed ballot request");
    NoritoJson::<PlainBallotDto>::from_request(request, &()).await
}
#[tokio::test]
async fn plain_ballot_declared_frame_reaches_exact_network_and_authority_policy() {
    use norito::NoritoSchema as _;
    let (state, _queue, _chain_id) = mk_basic_context();
    let dto = plain_ballot_frame_fixture(&state);
    let name = "iroha_torii::gov::PlainBallotDto";
    assert_eq!(PlainBallotDto::nominal_name(), name);
    assert_eq!(PlainBallotDto::frame_name(), name);
    assert_eq!(
        norito::schema::identity::frame_hash::<PlainBallotDto>(),
        norito::core::schema_hash_for_name(name),
    );
    let frame = norito::encode_canonical(&dto).expect("valid ballot frame");
    let header = norito::core::Header::read(frame.as_slice()).unwrap();
    assert_eq!(header.schema, norito::core::schema_hash_for_name(name));
    let expected = norito::json::to_value(&dto).unwrap();
    let mut layouts = 0;
    for flags in 0..=u8::MAX {
        if norito::core::validate_header_flags(flags).is_err() {
            continue;
        }
        layouts += 1;
        let _layout = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::encode_canonical(&dto).unwrap(), frame);
        let replay: PlainBallotDto = norito::decode_canonical(&frame).unwrap();
        assert_eq!(norito::json::to_value(&replay).unwrap(), expected);
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
    assert_eq!(layouts, 10);
    let parsed = extract_plain_ballot_frame(frame.clone()).await.unwrap();
    assert_eq!(norito::json::to_value(&parsed.0).unwrap(), expected);
    let authenticated = canonical_account(ACCOUNT_AUTHORITY);
    let response = handle_gov_ballot_plain_with_policy(
        state.clone(),
        &authenticated,
        parsed,
        MaybeTelemetry::disabled(),
    )
    .await
    .expect("real extractor result must produce a ballot draft");
    assert!(response.0.drafted);
    assert_eq!(response.0.tx_instructions.len(), 1);

    let wrong_authenticated = AccountId::new(KeyPair::random().public_key().clone());
    assert_ne!(wrong_authenticated, authenticated);
    let error = handle_gov_ballot_plain_with_policy(
        state.clone(),
        &wrong_authenticated,
        extract_plain_ballot_frame(frame).await.unwrap(),
        MaybeTelemetry::disabled(),
    )
    .await
    .expect_err("a valid frame cannot substitute its authenticated caller");
    assert!(matches!(
        error,
        crate::Error::Query(iroha_data_model::ValidationFail::NotPermitted(message))
            if message == "authenticated account must equal the governance ballot authority"
    ));

    let mut foreign = plain_ballot_frame_fixture(&state);
    foreign.network_id = foreign_network_id();
    assert_ne!(foreign.network_id, *state.network_id_ref());
    let parsed = extract_plain_ballot_frame(norito::encode_canonical(&foreign).unwrap())
        .await
        .unwrap();
    let error = handle_gov_ballot_plain_with_policy(
        state,
        &authenticated,
        parsed,
        MaybeTelemetry::disabled(),
    )
    .await
    .expect_err("a well-framed foreign network must still reject");
    assert!(matches!(
        error,
        crate::Error::Query(iroha_data_model::ValidationFail::NotPermitted(message))
            if message == "governance ballot targets a different network"
    ));
}
#[tokio::test]
async fn plain_ballot_frame_extractor_rejects_foreign_truncated_and_forbidden_payloads() {
    let (state, _queue, _chain_id) = mk_basic_context();
    let dto = plain_ballot_frame_fixture(&state);
    let frame = norito::encode_canonical(&dto).unwrap();
    extract_plain_ballot_frame(frame.clone())
        .await
        .expect("each negative has an admitted positive control");
    let value = norito::json::to_value(&dto).unwrap();
    let text = norito::json::to_string(&value).unwrap();
    let foreign = norito::encode_canonical(&text).unwrap();
    assert!(matches!(
        norito::decode_from_bytes::<PlainBallotDto>(&foreign),
        Err(norito::Error::SchemaMismatch)
    ));
    for bytes in [foreign, frame[..frame.len() - 1].to_vec()] {
        let error = extract_plain_ballot_frame(bytes)
            .await
            .expect_err("malformed or foreign frame must reject before ballot policy");
        assert_eq!(error.status(), axum::http::StatusCode::BAD_REQUEST);
    }
    for field in ["unexpected", "private_key"] {
        let mut bad = value.clone();
        bad.as_object_mut().expect("ballot JSON object").insert(
            field.to_owned(),
            norito::json::Value::String("forbidden".into()),
        );
        let text = norito::json::to_string(&bad).unwrap();
        let (payload, flags) = norito::codec::encode_with_header_flags(&text);
        let bytes = norito::core::frame_bare_with_header_flags::<PlainBallotDto>(&payload, flags)
            .expect("well-framed negative payload");
        let error = extract_plain_ballot_frame(bytes)
            .await
            .expect_err("typed framing cannot bypass strict ballot JSON fields");
        assert_eq!(error.status(), axum::http::StatusCode::BAD_REQUEST);
    }
}
