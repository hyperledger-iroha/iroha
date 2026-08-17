#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn signed_query_proxy_tries_next_candidate_only_before_dispatch() {
    let first_peer_id = checked_torii_test_peer_id(0x93, "derive hedged first proxy peer fixture key");
    let second_peer_id = checked_torii_test_peer_id(0x94, "derive hedged second proxy peer fixture key");
    let route = RoutingDecision::new(LaneId::new(2), DataSpaceId::new(2));
    let request = signed_query_proxy_request_for_test(
        Hash::new(b"signed-query-pre-dispatch-fallback"),
        route,
    );
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
    let body = torii_body_bytes(response, "response body should be readable").await;
    assert_eq!(body.as_ref(), b"pre-dispatch-fallback-ok");
}
