#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn signed_query_proxy_tries_next_candidate_only_before_dispatch() {
    let first_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x93, "derive hedged first proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let second_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x94, "derive hedged second proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let route = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2));
    let request = ToriiProxyRequestV6 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V6,
        request_id: Hash::new(b"signed-query-pre-dispatch-fallback"),
        deadline_unix_ms: super::torii_proxy_test_deadline_unix_ms(),
        hop_count: 1,
        max_hops: 3,
        visited_peer_ids: Vec::new(),
        request: ToriiProxyRequestKindV4::SignedQueryRouteScan {
            query_bytes: Vec::new(),
            expected_route: ToriiRouteHintV1::from(route),
            response_format: ToriiProxyResponseFormatV1::Norito,
        },
    };
    let attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let attempts_ref = attempts.clone();
    let first_peer_id_for_closure = first_peer_id.clone();

    let response = super::execute_torii_proxy_request_across_candidates(
        vec![
            ToriiProxyCandidate::P2p(first_peer_id.clone()),
            ToriiProxyCandidate::P2p(second_peer_id.clone()),
        ],
        route,
        request,
        TORII_PROXY_REQUEST_MAX_ENCODED_BYTES_V1,
        Duration::from_millis(20),
        move |candidate, _request| {
            let attempts = attempts_ref.clone();
            let first_peer_id = first_peer_id_for_closure.clone();
            async move {
                let peer_id = candidate.peer_id().clone();
                attempts
                    .lock()
                    .expect("attempt tracker should lock")
                    .push(peer_id.clone());
                if peer_id == first_peer_id {
                    return Err(ToriiProxyAttemptError::before_dispatch(
                        "request encoding failed before transport dispatch",
                    ));
                }
                Ok(ToriiProxyHttpResponseV1 {
                    status_code: StatusCode::OK.as_u16(),
                    headers: Vec::new(),
                    body: b"pre-dispatch-fallback-ok".to_vec(),
                })
            }
        },
        |_request_id| async move {},
    )
    .await;

    assert_eq!(
        attempts
            .lock()
            .expect("attempt tracker should lock")
            .as_slice(),
        &[first_peer_id, second_peer_id]
    );
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    assert_eq!(body.as_ref(), b"pre-dispatch-fallback-ok");
}
