#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn signed_query_proxy_does_not_resend_after_complete_rejection() {
    let first_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x95, "derive retryable first proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let second_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x96, "derive retryable second proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let route = RoutingDecision::new(LaneId::new(3), DataSpaceId::new(3));
    let request = ToriiProxyRequestV5 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V5,
        request_id: Hash::new(b"signed-query-complete-rejection"),
        hop_count: 1,
        max_hops: 3,
        visited_peer_ids: Vec::new(),
        request: ToriiProxyRequestKindV4::SignedQueryRouteScan {
            query_bytes: Vec::new(),
            expected_route: ToriiRouteHintV1::from(route),
            response_format: ToriiProxyResponseFormatV1::Norito,
        },
    };
    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let attempts_ref = attempts.clone();

    let response = super::execute_torii_proxy_request_across_candidates(
        vec![
            ToriiProxyCandidate::P2p(first_peer_id.clone()),
            ToriiProxyCandidate::P2p(second_peer_id.clone()),
        ],
        route,
        request,
        Duration::from_millis(20),
        move |candidate, _request| {
            let first_peer_id = first_peer_id.clone();
            let attempts = attempts_ref.clone();
            async move {
                attempts.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let peer_id = candidate.peer_id().clone();
                if peer_id == first_peer_id {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    return Ok(ToriiProxyHttpResponseV1 {
                        status_code: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                        headers: vec![iroha_core::torii_proxy::ToriiProxyHeaderV1 {
                            name: "x-iroha-reject-code".to_owned(),
                            value: b"route_unavailable".to_vec(),
                        }],
                        body: b"retry".to_vec(),
                    });
                }
                tokio::time::sleep(Duration::from_millis(30)).await;
                Ok(ToriiProxyHttpResponseV1 {
                    status_code: StatusCode::OK.as_u16(),
                    headers: Vec::new(),
                    body: b"retry-then-ok".to_vec(),
                })
            }
        },
        |_request_id| async move {},
    )
    .await;

    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    assert_eq!(body.as_ref(), b"retry");
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn execute_torii_proxy_request_across_candidates_returns_route_unavailable_without_candidates()
 {
    let route = RoutingDecision::new(LaneId::new(4), DataSpaceId::new(5));
    let request_id = Hash::new(b"torii-proxy-no-candidates");
    let request = ToriiProxyRequestV5 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V5,
        request_id: request_id.clone(),
        hop_count: 1,
        max_hops: 3,
        visited_peer_ids: Vec::new(),
        request: ToriiProxyRequestKindV4::SignedQueryRouteScan {
            query_bytes: Vec::new(),
            expected_route: ToriiRouteHintV1::from(route),
            response_format: ToriiProxyResponseFormatV1::Norito,
        },
    };
    let completed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let completed_ref = completed.clone();

    let response = super::execute_torii_proxy_request_across_candidates(
        Vec::new(),
        route,
        request,
        Duration::from_millis(20),
        |_candidate, _request| async move {
            Err::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(
                ToriiProxyAttemptError::before_dispatch(
                    "execute should not be called without candidates",
                ),
            )
        },
        move |completed_request_id| {
            let completed = completed_ref.clone();
            async move {
                completed
                    .lock()
                    .expect("completion tracker should lock")
                    .push(completed_request_id);
            }
        },
    )
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("route_unavailable")
    );
    assert_eq!(
        completed
            .lock()
            .expect("completion tracker should lock")
            .as_slice(),
        &[request_id]
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn execute_torii_proxy_request_across_candidates_returns_last_retryable_response() {
    let peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x97, "derive last retryable proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let route = RoutingDecision::new(LaneId::new(6), DataSpaceId::new(7));
    let request = ToriiProxyRequestV5 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V5,
        request_id: Hash::new(b"torii-proxy-last-retryable"),
        hop_count: 1,
        max_hops: 3,
        visited_peer_ids: Vec::new(),
        request: ToriiProxyRequestKindV4::SignedQueryRouteScan {
            query_bytes: Vec::new(),
            expected_route: ToriiRouteHintV1::from(route),
            response_format: ToriiProxyResponseFormatV1::Norito,
        },
    };

    let response = super::execute_torii_proxy_request_across_candidates(
        vec![ToriiProxyCandidate::P2p(peer_id)],
        route,
        request,
        Duration::from_millis(20),
        |_candidate, _request| async move {
            Ok(ToriiProxyHttpResponseV1 {
                status_code: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                headers: vec![iroha_core::torii_proxy::ToriiProxyHeaderV1 {
                    name: "x-iroha-reject-code".to_owned(),
                    value: b"route_unavailable".to_vec(),
                }],
                body: b"retry-later".to_vec(),
            })
        },
        |_request_id| async move {},
    )
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-route-transport")
            .and_then(|value| value.to_str().ok()),
        Some("p2p_proxy")
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    assert_eq!(body.as_ref(), b"retry-later");
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_outcome_unknown_survives_both_retryable_completion_orders() {
    let expected_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"outcome-unknown-retryable-order",
    ));
    let expected_hash_literal = expected_hash.to_string();

    for unknown_arrives_first in [true, false] {
        let mut strongest = None;
        let unknown = super::queue_plan_outcome_unknown_response(
            expected_hash.clone(),
            "authoritative cleanup sync outcome is unknown",
        );
        let retryable = super::torii_proxy_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "route_unavailable",
            "definitely not admitted by this candidate",
        );

        if unknown_arrives_first {
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 1, unknown);
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 0, retryable);
        } else {
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 0, retryable);
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 1, unknown);
        }

        let response = strongest.expect("one reducer candidate must remain").2;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN"),
            "ordinary retryable failures must never overwrite an indeterminate admission"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read preserved outcome-unknown response");
        let envelope: ErrorEnvelope =
            norito::decode_from_bytes(&body).expect("decode outcome-unknown error envelope");
        assert_eq!(envelope.code(), "queue_plan_journal_outcome_unknown");
        assert_eq!(
            envelope
                .details
                .expect("outcome-unknown details")
                .tx_hash
                .as_deref(),
            Some(expected_hash_literal.as_str())
        );
    }
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_outcome_unknown_dominates_nonretryable_failure_in_both_completion_orders() {
    let expected_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"outcome-unknown-nonretryable-order",
    ));
    let expected_hash_literal = expected_hash.to_string();

    for unknown_arrives_first in [true, false] {
        let mut strongest = None;
        let unknown = super::queue_plan_outcome_unknown_response(
            expected_hash.clone(),
            "authority may have durably admitted before response loss",
        );
        let nonretryable = super::torii_proxy_error_response(
            StatusCode::CONFLICT,
            "routing_plan_mismatch",
            "stale authority route view",
        );

        if unknown_arrives_first {
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 1, unknown);
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 0, nonretryable);
        } else {
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 0, nonretryable);
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 1, unknown);
        }

        let response = strongest.expect("one reducer candidate must remain").2;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN"),
            "a definite failure from one authority cannot mask another dispatched authority"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read strict proxy outcome-unknown response");
        let envelope: ErrorEnvelope =
            norito::decode_from_bytes(&body).expect("decode outcome-unknown envelope");
        assert_eq!(
            envelope
                .details
                .expect("outcome-unknown details")
                .tx_hash
                .as_deref(),
            Some(expected_hash_literal.as_str())
        );
    }

    for candidate_zero_arrives_first in [true, false] {
        let mut strongest = None;
        let candidate_zero = super::torii_proxy_error_response(
            StatusCode::CONFLICT,
            "candidate_zero",
            "candidate zero failure",
        );
        let candidate_one = super::torii_proxy_error_response(
            StatusCode::CONFLICT,
            "candidate_one",
            "candidate one failure",
        );

        if candidate_zero_arrives_first {
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 0, candidate_zero);
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 1, candidate_one);
        } else {
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 1, candidate_one);
            super::retain_strongest_queue_plan_synced_failure(&mut strongest, 0, candidate_zero);
        }

        let (priority, candidate_index, response) =
            strongest.expect("equal-priority reducer candidate must remain");
        assert_eq!(priority, 1);
        assert_eq!(candidate_index, 0);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("candidate_zero"),
            "candidate index must break equal-priority ties independent of arrival order"
        );
    }
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_outcome_unknown_rejects_forged_reconciliation_hash() {
    let peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(
            0xac,
            "derive forged outcome-unknown proxy peer fixture key",
        )
        .public_key()
        .clone(),
    );
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let (_app, request) =
        incoming_proxy_submit_fixture(0xad, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let expected_hash = super::queue_plan_synced_entrypoint_hash(&request.request)
        .expect("strict request exposes a typed reconciliation identity");
    let forged_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"forged-outcome-unknown-reconciliation-hash",
    ));
    assert_ne!(forged_hash, expected_hash);
    let expected_hash_literal = expected_hash.to_string();
    let forged_snapshot = super::response_to_torii_proxy_snapshot(
        super::queue_plan_outcome_unknown_response(
            forged_hash,
            "forged authoritative reconciliation identity",
        ),
        usize::MAX,
    )
    .await;

    let response = super::execute_torii_proxy_request_across_candidates(
        vec![ToriiProxyCandidate::P2p(peer_id)],
        route,
        request,
        Duration::ZERO,
        move |_candidate, _request| {
            let forged_snapshot = forged_snapshot.clone();
            async move { Ok(forged_snapshot) }
        },
        |_request_id| async move {},
    )
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN")
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read rebuilt outcome-unknown response");
    let envelope: ErrorEnvelope =
        norito::decode_from_bytes(&body).expect("decode rebuilt outcome-unknown envelope");
    assert_eq!(envelope.code(), "queue_plan_journal_outcome_unknown");
    assert_eq!(
        envelope
            .details
            .expect("rebuilt outcome-unknown details")
            .tx_hash
            .as_deref(),
        Some(expected_hash_literal.as_str()),
        "a forged authority hash must be replaced with the submitted transaction identity"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_accepts_a_reforwarded_certificate_from_an_authoritative_peer() {
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let (app, request) =
        incoming_proxy_submit_fixture(0xee, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let final_authority = PeerId::from(app.torii_proxy_bridge_signer.public_key().clone());
    let forwarding_authority = PeerId::from(
        checked_torii_test_ed25519_keypair(
            0xef,
            "derive QueuePlanSynced forwarding-authority fixture key",
        )
        .public_key()
        .clone(),
    );
    assert_ne!(forwarding_authority, final_authority);
    let final_authority_snapshot =
        exact_queue_plan_synced_acceptance_snapshot(&app, &request).await;
    let forwarding_authority_for_closure = forwarding_authority.clone();

    let response = super::execute_torii_proxy_request_across_candidates(
        vec![
            ToriiProxyCandidate::P2p(forwarding_authority.clone()),
            ToriiProxyCandidate::P2p(final_authority.clone()),
        ],
        route,
        request,
        Duration::from_millis(100),
        move |candidate, _request| {
            let final_authority_snapshot = final_authority_snapshot.clone();
            let forwarding_authority = forwarding_authority_for_closure.clone();
            async move {
                if candidate.peer_id() == &forwarding_authority {
                    // The forwarding authority relays the final authority's
                    // signed receipt unchanged.
                    return Ok::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(
                        final_authority_snapshot,
                    );
                }
                Err(ToriiProxyAttemptError::before_dispatch(
                    "the direct final-authority attempt is intentionally unused",
                ))
            }
        },
        |_request_id| async move {},
    )
    .await;

    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "a forwarding peer must not invalidate an exact receipt signed by another authoritative candidate"
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read reforwarded strict receipt");
    let certificate: QueuePlanAdmissionCertificateV2 =
        norito::decode_from_bytes(&body).expect("decode reforwarded strict certificate");
    let attestation = certificate
        .attestations
        .first()
        .expect("reforwarded strict certificate must contain one attestation");
    let coordinator = certificate
        .binding
        .admission_context
        .route_incarnations
        .first()
        .expect("reforwarded certificate coordinator context");
    assert_eq!(
        coordinator.validator_set[usize::from(attestation.validator_index)],
        final_authority
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn proxied_transaction_submission_preserves_public_accept_and_prefer_contracts() {
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let (app, request) =
        incoming_proxy_submit_fixture(0xec, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let ToriiProxyRequestKindV4::SubmitTransaction { transaction, .. } = &request.request else {
        panic!("strict proxy fixture must contain a transaction");
    };
    let entrypoint_hash = transaction.hash();
    let signed_transaction_hash = signed_transaction_hash_for_entrypoint(transaction);
    let mut private_snapshot = exact_queue_plan_synced_acceptance_snapshot(&app, &request).await;
    let _: QueuePlanAdmissionCertificateV2 = norito::decode_from_bytes(&private_snapshot.body)
        .expect("production authority response must contain the private strict certificate");
    private_snapshot
        .headers
        .push(iroha_core::torii_proxy::ToriiProxyHeaderV1 {
            name: "x-iroha-route-transport".to_owned(),
            value: b"p2p_proxy".to_vec(),
        });
    let private_body = private_snapshot.body.clone();

    for (format, minimal_response) in [
        (ResponseFormat::Norito, false),
        (ResponseFormat::Json, false),
        (ResponseFormat::Norito, true),
        (ResponseFormat::Json, true),
    ] {
        let local = transaction_submission_response(
            app.as_ref(),
            entrypoint_hash.clone(),
            signed_transaction_hash.clone(),
            route,
            "local",
            minimal_response,
            format,
        );
        let proxied = super::normalize_proxied_transaction_submission_response(
            app.as_ref(),
            super::torii_proxy_snapshot_to_response(private_snapshot.clone()),
            entrypoint_hash.clone(),
            signed_transaction_hash.clone(),
            route,
            minimal_response,
            format,
        );

        assert_eq!(proxied.status(), local.status());
        assert_eq!(proxied.status(), StatusCode::ACCEPTED);
        for header_name in [
            "content-type",
            "preference-applied",
            "x-iroha-entrypoint-hash",
            "x-iroha-transaction-hash",
            "x-iroha-signed-transaction-hash",
            "x-iroha-route-lane-id",
            "x-iroha-route-dataspace-id",
        ] {
            assert_eq!(
                proxied.headers().get(header_name),
                local.headers().get(header_name),
                "local and proxied public submission responses must agree on `{header_name}`"
            );
        }
        assert_eq!(
            proxied
                .headers()
                .get("x-iroha-route-transport")
                .and_then(|value| value.to_str().ok()),
            Some("p2p_proxy")
        );
        assert_eq!(
            proxied
                .headers()
                .get("x-iroha-routed-by")
                .and_then(|value| value.to_str().ok()),
            Some("proxy")
        );

        let local_body = axum::body::to_bytes(local.into_body(), usize::MAX)
            .await
            .expect("read local public submission response");
        let proxied_body = axum::body::to_bytes(proxied.into_body(), usize::MAX)
            .await
            .expect("read proxied public submission response");
        if minimal_response {
            assert!(local_body.is_empty());
            assert!(proxied_body.is_empty());
            continue;
        }
        assert_ne!(
            proxied_body.as_ref(),
            private_body.as_slice(),
            "the internal QueuePlanSynced proof must not cross the public API boundary"
        );
        let decode_public_receipt = |body: &[u8]| match format {
            ResponseFormat::Norito => {
                norito::decode_from_bytes::<TransactionSubmissionReceipt>(body)
                    .expect("decode public Norito transaction receipt")
            }
            ResponseFormat::Json => norito::json::from_slice::<TransactionSubmissionReceipt>(body)
                .expect("decode public JSON transaction receipt"),
        };
        let local_receipt = decode_public_receipt(&local_body);
        let proxied_receipt = decode_public_receipt(&proxied_body);
        local_receipt
            .verify()
            .expect("local public transaction receipt must verify");
        proxied_receipt
            .verify()
            .expect("proxied public transaction receipt must verify");
        assert_eq!(
            proxied_receipt.payload.entrypoint_hash,
            local_receipt.payload.entrypoint_hash
        );
        assert_eq!(
            proxied_receipt.payload.signed_transaction_hash,
            local_receipt.payload.signed_transaction_hash
        );
        assert_eq!(proxied_receipt.payload.signer, local_receipt.payload.signer);
    }
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_accepts_only_exact_durable_acceptance_evidence() {
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let (app, request) =
        incoming_proxy_submit_fixture(0xaf, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let peer_id = PeerId::from(app.torii_proxy_bridge_signer.public_key().clone());
    let expected_hash_literal = accepted_queue_hash_for_proxy_submit(&app, &request).to_string();
    let valid_snapshot = exact_queue_plan_synced_acceptance_snapshot(&app, &request).await;

    let response =
            super::execute_torii_proxy_request_across_candidates(
                vec![ToriiProxyCandidate::P2p(peer_id.clone())],
                route,
                request.clone(),
                Duration::ZERO,
                {
                    let valid_snapshot = valid_snapshot.clone();
                    move |_candidate, _request| {
                        let valid_snapshot = valid_snapshot.clone();
                        async move {
                            Ok::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(valid_snapshot)
                        }
                    }
                },
                |_request_id| async move {},
            )
            .await;
    assert_eq!(
        response.status(),
        StatusCode::ACCEPTED,
        "the exact signed durable-acceptance response must remain successful"
    );

    let forged_success = ToriiProxyHttpResponseV1 {
        status_code: StatusCode::OK.as_u16(),
        headers: Vec::new(),
        body: b"forged success".to_vec(),
    };
    let mut missing_evidence = valid_snapshot.clone();
    missing_evidence.body.clear();
    let valid_certificate =
        norito::decode_from_bytes::<QueuePlanAdmissionCertificateV2>(&valid_snapshot.body)
            .expect("decode production strict acceptance certificate");
    let rewrite_certificate =
        |mut snapshot: ToriiProxyHttpResponseV1, certificate: QueuePlanAdmissionCertificateV2| {
            snapshot.body = norito::to_bytes(&certificate)
                .expect("encode mutated strict acceptance certificate");
            snapshot
        };

    let mut wrong_hash_certificate = valid_certificate.clone();
    wrong_hash_certificate.binding.entrypoint_hash =
        HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
            b"forged-strict-acceptance-entrypoint-hash",
        ));
    let wrong_hash = rewrite_certificate(valid_snapshot.clone(), wrong_hash_certificate);

    let mut wrong_request_certificate = valid_certificate.clone();
    wrong_request_certificate.binding.request_id =
        Hash::new(b"replayed-queue-plan-synced-request-id");
    let wrong_request = rewrite_certificate(valid_snapshot.clone(), wrong_request_certificate);

    let mut wrong_journal_version_certificate = valid_certificate.clone();
    wrong_journal_version_certificate
        .binding
        .queue_plan_journal_version = queue::QUEUE_PLAN_JOURNAL_VERSION.saturating_add(1);
    let wrong_journal_version =
        rewrite_certificate(valid_snapshot.clone(), wrong_journal_version_certificate);

    let mut wrong_plan_digest_certificate = valid_certificate.clone();
    wrong_plan_digest_certificate.binding.routing_plan_digest =
        Hash::new(b"forged-queue-plan-synced-routing-plan-digest");
    let wrong_plan_digest =
        rewrite_certificate(valid_snapshot.clone(), wrong_plan_digest_certificate);

    let mut wrong_exact_plan_certificate = valid_certificate.clone();
    wrong_exact_plan_certificate
        .binding
        .admission_context
        .route_incarnations[0]
        .leg
        .route = RoutingDecision::new(LaneId::new(9), DataSpaceId::new(12));
    let wrong_exact_plan =
        rewrite_certificate(valid_snapshot.clone(), wrong_exact_plan_certificate);

    let mut wrong_version_certificate = valid_certificate.clone();
    wrong_version_certificate.version = wrong_version_certificate.version.saturating_add(1);
    let wrong_version = rewrite_certificate(valid_snapshot.clone(), wrong_version_certificate);

    let outsider = checked_torii_test_ed25519_keypair(
        0xae,
        "derive forged strict acceptance signer fixture key",
    );
    let outsider_attestation = sign_queue_plan_synced_test_receipt(
        &valid_certificate.binding,
        valid_certificate.attestations[0].validator_index,
        &outsider,
    );
    let outsider_receipt = rewrite_certificate(
        valid_snapshot,
        QueuePlanAdmissionCertificateV2 {
            version: QUEUE_PLAN_ADMISSION_CERTIFICATE_VERSION_V2,
            binding: valid_certificate.binding,
            attestations: vec![outsider_attestation],
        },
    );

    for (label, snapshot) in [
        ("forged arbitrary 2xx", forged_success),
        ("missing receipt evidence", missing_evidence),
        ("wrong signed receipt hash", wrong_hash),
        ("replayed request receipt", wrong_request),
        ("wrong journal-version receipt", wrong_journal_version),
        ("wrong routing-plan digest receipt", wrong_plan_digest),
        ("wrong exact routing-plan receipt", wrong_exact_plan),
        ("wrong certificate version", wrong_version),
        ("self-asserted outsider receipt", outsider_receipt),
    ] {
        let response = super::execute_torii_proxy_request_across_candidates(
            vec![ToriiProxyCandidate::P2p(peer_id.clone())],
            route,
            request.clone(),
            Duration::ZERO,
            move |_candidate, _request| {
                let snapshot = snapshot.clone();
                async move { Ok::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(snapshot) }
            },
            |_request_id| async move {},
        )
        .await;

        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{label} must be classified as indeterminate after dispatch"
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN"),
            "{label} must fail with the stable outcome-unknown code"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read indeterminate strict acceptance response");
        let envelope: ErrorEnvelope =
            norito::decode_from_bytes(&body).expect("decode outcome-unknown response envelope");
        assert_eq!(
            envelope.code(),
            "queue_plan_journal_outcome_unknown",
            "{label}"
        );
        assert_eq!(
            envelope
                .details
                .expect("outcome-unknown details")
                .tx_hash
                .as_deref(),
            Some(expected_hash_literal.as_str()),
            "{label}"
        );
    }
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_post_admission_or_malformed_500_is_indeterminate() {
    let peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(
            0xb0,
            "derive strict post-admission failure proxy peer fixture key",
        )
        .public_key()
        .clone(),
    );
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let (app, request) =
        incoming_proxy_submit_fixture(0xb1, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let expected_hash_literal = accepted_queue_hash_for_proxy_submit(&app, &request).to_string();
    let receipt_signing_failure = super::response_to_torii_proxy_snapshot(
        super::torii_proxy_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "transaction_submission_receipt_signing_failed",
            "queue admission completed before receipt signing failed",
        ),
        app.transaction_max_content_len.max(1),
    )
    .await;
    let malformed_failure = ToriiProxyHttpResponseV1 {
        status_code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
        headers: Vec::new(),
        body: b"malformed post-admission failure".to_vec(),
    };

    for (label, snapshot) in [
        ("receipt-signing failure", receipt_signing_failure),
        ("malformed 500", malformed_failure),
    ] {
        let response = super::execute_torii_proxy_request_across_candidates(
            vec![ToriiProxyCandidate::P2p(peer_id.clone())],
            route,
            request.clone(),
            Duration::ZERO,
            move |_candidate, _request| {
                let snapshot = snapshot.clone();
                async move { Ok::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(snapshot) }
            },
            |_request_id| async move {},
        )
        .await;

        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{label}"
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN"),
            "{label}"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read post-admission outcome-unknown response");
        let envelope: ErrorEnvelope =
            norito::decode_from_bytes(&body).expect("decode post-admission outcome envelope");
        assert_eq!(
            envelope.code(),
            "queue_plan_journal_outcome_unknown",
            "{label}"
        );
        assert_eq!(
            envelope
                .details
                .expect("post-admission outcome details")
                .tx_hash
                .as_deref(),
            Some(expected_hash_literal.as_str()),
            "{label}"
        );
    }
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_post_dispatch_loss_is_exactly_indeterminate_for_each_transport() {
    let p2p_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0xa5, "derive post-dispatch P2P proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let http_peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(
            0xa6,
            "derive post-dispatch HTTP proxy peer fixture key",
        )
        .public_key()
        .clone(),
    );
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    for (seed, candidate, expected_transport) in [
        (0xa7, ToriiProxyCandidate::P2p(p2p_peer_id), "p2p_proxy"),
        (
            0xa8,
            ToriiProxyCandidate::HttpBridge {
                peer_id: http_peer_id,
                torii_url: "https://authority.invalid".to_owned(),
            },
            "http_bridge",
        ),
    ] {
        let (_app, request) =
            incoming_proxy_submit_fixture(seed, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
        let expected_hash = accepted_queue_hash_for_proxy_submit(&_app, &request);
        let expected_hash_literal = expected_hash.to_string();
        let response = super::execute_torii_proxy_request_across_candidates(
            vec![candidate],
            route,
            request,
            Duration::ZERO,
            |_candidate, _request| async move {
                Err::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(
                    ToriiProxyAttemptError::after_dispatch(
                        "authenticated authority response was lost after dispatch",
                    ),
                )
            },
            |_request_id| async move {},
        )
        .await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-route-transport")
                .and_then(|value| value.to_str().ok()),
            Some(expected_transport)
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read post-dispatch outcome-unknown response");
        let envelope: ErrorEnvelope = norito::decode_from_bytes(&body)
            .expect("decode post-dispatch outcome-unknown envelope");
        assert_eq!(envelope.code(), "queue_plan_journal_outcome_unknown");
        assert_eq!(
            envelope
                .details
                .expect("post-dispatch details")
                .tx_hash
                .as_deref(),
            Some(expected_hash_literal.as_str())
        );
    }
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_p2p_missing_network_is_pre_dispatch() {
    let (app, request) =
        incoming_proxy_submit_fixture(0xc1, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0xc2, "derive missing-network proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let error = super::execute_torii_proxy_request_via_peer(&app, peer_id, Arc::new(request))
        .await
        .expect_err("missing P2P network must fail before dispatch");
    assert!(matches!(
        error,
        ToriiProxyAttemptError::DefinitelyNotDispatched(_)
    ));
    assert!(!error.may_have_reached_authority());
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_p2p_post_dispatch_channel_loss_is_indeterminate() {
    let (mut app, request) =
        incoming_proxy_submit_fixture(0xc3, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    Arc::get_mut(&mut app)
        .expect("fixture app must be uniquely owned")
        .p2p = Some(iroha_core::IrohaNetwork::closed_for_tests());
    let peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0xc4, "derive closed-channel proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let pending_key = (request.request_id.clone(), peer_id.clone());
    let after_network_post =
        super::register_torii_proxy_after_network_post_observer(pending_key.clone());
    let app_for_close = app.clone();
    let close_task = tokio::spawn(async move {
        after_network_post.notified().await;
        assert!(
            app_for_close
                .torii_proxy_pending
                .lock()
                .await
                .remove(&pending_key)
                .is_some(),
            "the pending response channel must exist immediately after network.post"
        );
    });

    let error = super::execute_torii_proxy_request_via_peer(&app, peer_id, Arc::new(request))
        .await
        .expect_err("lost response channel after network post must be indeterminate");
    close_task
        .await
        .expect("pending-channel closer must finish");
    assert!(matches!(
        error,
        ToriiProxyAttemptError::DispatchedWithoutResponse(_)
    ));
    assert!(error.may_have_reached_authority());
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_http_connect_loss_is_pre_dispatch_and_body_loss_is_post_dispatch() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let (app, send_request) =
        incoming_proxy_submit_fixture(0xc5, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0xc6, "derive HTTP-loss proxy peer fixture key")
            .public_key()
            .clone(),
    );

    let send_error = super::execute_torii_proxy_request_via_http_bridge(
        &app,
        peer_id.clone(),
        "http://127.0.0.1:0/".to_owned(),
        send_request,
    )
    .await
    .expect_err("port-zero connect failure must occur before dispatch");
    assert!(matches!(
        send_error,
        ToriiProxyAttemptError::DefinitelyNotDispatched(_)
    ));
    assert!(!send_error.may_have_reached_authority());

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind truncated-body HTTP server");
    let address = listener.local_addr().expect("truncated-body address");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept proxy HTTP request");
        let mut request_bytes = vec![0_u8; 4096];
        let _ = socket
            .read(&mut request_bytes)
            .await
            .expect("read proxy HTTP request");
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 32\r\nConnection: close\r\n\r\nshort")
            .await
            .expect("write deliberately truncated response");
        socket.shutdown().await.expect("close truncated response");
    });
    let (_other_app, body_request) =
        incoming_proxy_submit_fixture(0xc7, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let body_error = super::execute_torii_proxy_request_via_http_bridge(
        &app,
        peer_id,
        format!("http://{address}/"),
        body_request,
    )
    .await
    .expect_err("truncated response body must be indeterminate");
    server.await.expect("truncated-body server must finish");
    assert!(matches!(
        body_error,
        ToriiProxyAttemptError::DispatchedWithoutResponse(_)
    ));
    assert!(body_error.may_have_reached_authority());
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_http_redirect_to_connect_loss_is_not_followed() {
    let reached_authority = Arc::new(AtomicUsize::new(0));
    let reached_authority_for_handler = reached_authority.clone();
    let upstream = axum::Router::new().route(
        TORII_INTERNAL_PROXY_HTTP_PATH,
        axum::routing::post(move || {
            let reached_authority = reached_authority_for_handler.clone();
            async move {
                reached_authority.fetch_add(1, Ordering::SeqCst);
                Response::builder()
                    .status(StatusCode::TEMPORARY_REDIRECT)
                    .header(
                        axum::http::header::LOCATION,
                        "http://127.0.0.1:0/redirect-connect-loss",
                    )
                    .body(Body::empty())
                    .expect("build strict proxy redirect response")
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind strict proxy redirect server");
    let address = listener
        .local_addr()
        .expect("strict proxy redirect address");
    let upstream_task = tokio::spawn(async move {
        axum::serve(listener, upstream.into_make_service())
            .await
            .expect("serve strict proxy redirect");
    });
    let (app, request) =
        incoming_proxy_submit_fixture(0xc8, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0xc9, "derive HTTP-redirect proxy peer fixture key")
            .public_key()
            .clone(),
    );

    let snapshot = super::execute_torii_proxy_request_via_http_bridge(
        &app,
        peer_id,
        format!("http://{address}/"),
        request,
    )
    .await
    .expect("strict proxy must return the redirect response without following it");
    upstream_task.abort();

    assert_eq!(reached_authority.load(Ordering::SeqCst), 1);
    assert_eq!(
        snapshot.status_code,
        StatusCode::TEMPORARY_REDIRECT.as_u16(),
        "following this redirect would turn an already-dispatched POST into a connect error"
    );
    assert!(snapshot.headers.iter().any(|header| {
        header.name.eq_ignore_ascii_case("location")
            && header.value.as_slice() == b"http://127.0.0.1:0/redirect-connect-loss"
    }));
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn queue_plan_synced_before_dispatch_failure_remains_definitely_unavailable() {
    let peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(
            0xb5,
            "derive pre-dispatch strict proxy peer fixture key",
        )
        .public_key()
        .clone(),
    );
    let route = RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL);
    let (_app, request) =
        incoming_proxy_submit_fixture(0xb6, ToriiProxyTransactionAdmissionV2::QueuePlanSynced);
    let response = super::execute_torii_proxy_request_across_candidates(
        vec![ToriiProxyCandidate::P2p(peer_id)],
        route,
        request,
        Duration::ZERO,
        |_candidate, _request| async move {
            Err::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(
                ToriiProxyAttemptError::before_dispatch(
                    "request encoding failed before network ownership",
                ),
            )
        },
        |_request_id| async move {},
    )
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("route_unavailable")
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read definite pre-dispatch failure");
    let envelope: ErrorEnvelope =
        norito::decode_from_bytes(&body).expect("decode route-unavailable envelope");
    assert_eq!(envelope.code(), "route_unavailable");
    assert!(
        envelope
            .details
            .is_none_or(|details| details.tx_hash.is_none()),
        "definitely undispatched failures must not publish an indeterminate queue identity"
    );
}

#[cfg(any(feature = "p2p_ws", feature = "connect"))]
#[tokio::test]
async fn execute_torii_proxy_request_across_candidates_returns_route_unavailable_after_transport_errors()
 {
    let peer_id = PeerId::from(
        checked_torii_test_ed25519_keypair(0x98, "derive transport error proxy peer fixture key")
            .public_key()
            .clone(),
    );
    let route = RoutingDecision::new(LaneId::new(8), DataSpaceId::new(9));
    let request_id = Hash::new(b"torii-proxy-all-transport-errors");
    let request = ToriiProxyRequestV5 {
        schema_version: TORII_PROXY_REQUEST_VERSION_V5,
        request_id: request_id.clone(),
        hop_count: 1,
        max_hops: 3,
        visited_peer_ids: Vec::new(),
        request: ToriiProxyRequestKindV4::SignedQueryRouteScan {
            query_bytes: Vec::new(),
            expected_route: ToriiRouteHintV1::from(route),
            response_format: ToriiProxyResponseFormatV1::Norito,
        },
    };
    let completed = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let completed_ref = completed.clone();

    let response = super::execute_torii_proxy_request_across_candidates(
        vec![ToriiProxyCandidate::P2p(peer_id)],
        route,
        request,
        Duration::from_millis(20),
        |_candidate, _request| async move {
            Err::<ToriiProxyHttpResponseV1, ToriiProxyAttemptError>(
                ToriiProxyAttemptError::before_dispatch("transport unavailable"),
            )
        },
        move |completed_request_id| {
            let completed = completed_ref.clone();
            async move {
                completed
                    .lock()
                    .expect("completion tracker should lock")
                    .push(completed_request_id);
            }
        },
    )
    .await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response
            .headers()
            .get("x-iroha-reject-code")
            .and_then(|value| value.to_str().ok()),
        Some("route_unavailable")
    );
    assert_eq!(
        completed
            .lock()
            .expect("completion tracker should lock")
            .as_slice(),
        &[request_id]
    );
}

#[cfg(feature = "telemetry")]
fn sample_privacy_event_dto() -> RecordSoranetPrivacyEventDto {
    RecordSoranetPrivacyEventDto {
        event: SoranetPrivacyEventV1 {
            timestamp_unix: 1_720_000_123,
            mode: SoranetPrivacyModeV1::Entry,
            kind: SoranetPrivacyEventKindV1::HandshakeSuccess(
                SoranetPrivacyEventHandshakeSuccessV1 {
                    rtt_ms: Some(12),
                    active_circuits_after: Some(3),
                },
            ),
        },
        source: None,
    }
}

#[cfg(feature = "telemetry")]
fn sample_privacy_share_dto() -> RecordSoranetPrivacyShareDto {
    let mut share = SoranetPrivacyPrioShareV1::new(1, 1_720_000_020, 60);
    share.mode = SoranetPrivacyModeV1::Entry;
    share.handshake_accept_share = 5;
    share.active_circuits_sum_share = 30;
    share.active_circuits_sample_share = 5;
    share.active_circuits_max_observed = Some(7);
    share.verified_bytes_share = 1_024;
    RecordSoranetPrivacyShareDto {
        share,
        forwarded_by: None,
    }
}

#[cfg(feature = "telemetry")]
fn privacy_operator(
    app: &SharedAppState,
) -> axum::extract::Extension<operator_signatures::AuthenticatedOperatorPublicKey> {
    axum::extract::Extension(operator_signatures::AuthenticatedOperatorPublicKey(
        app.da_receipt_signer.public_key().clone(),
    ))
}

#[tokio::test]
#[cfg(feature = "telemetry")]
async fn privacy_ingest_rejects_when_disabled() {
    let app = mk_app_state_for_tests();
    let dto = sample_privacy_event_dto();

    let response = super::test_handler_post_soranet_privacy_event_with_ingress(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(dto),
    )
    .await
    .expect("handler executes");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let metrics = app.telemetry.metrics().await;
    let disabled = metrics
        .soranet_privacy_ingest_reject_total
        .get_metric_with_label_values(&["event", "disabled"])
        .unwrap()
        .get();
    assert!(disabled >= 1, "disabled counter should increment");
}

#[tokio::test]
#[cfg(feature = "telemetry")]
async fn privacy_ingest_denies_without_allowlist() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut =
            std::sync::Arc::get_mut(&mut app).expect("unique Arc for privacy configuration");
        app_mut.soranet_privacy_ingest.enabled = true;
        app_mut.soranet_privacy_allow_nets = Arc::new(Vec::new());
    }

    let response = super::test_handler_post_soranet_privacy_event_with_ingress(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(sample_privacy_event_dto()),
    )
    .await
    .expect("handler executes");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let metrics = app.telemetry.metrics().await;
    let blocked = metrics
        .soranet_privacy_ingest_reject_total
        .get_metric_with_label_values(&["event", "namespace_blocked"])
        .unwrap()
        .get();
    assert!(blocked >= 1, "namespace block counter should increment");
}

#[tokio::test]
#[cfg(feature = "telemetry")]
async fn privacy_ingest_enforces_operator_namespace_and_rate() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut =
            std::sync::Arc::get_mut(&mut app).expect("unique Arc for privacy configuration");
        app_mut.soranet_privacy_ingest.enabled = true;
        app_mut.soranet_privacy_ingest.allow_cidrs =
            vec!["127.0.0.1/32".to_string(), "::1/128".to_string()];
        app_mut.soranet_privacy_ingest.rate_per_sec =
            Some(std::num::NonZeroU32::new(1).expect("nonzero"));
        app_mut.soranet_privacy_ingest.burst = Some(std::num::NonZeroU32::new(1).expect("nonzero"));
        app_mut.soranet_privacy_allow_nets = Arc::new(crate::limits::parse_cidrs(
            &app_mut.soranet_privacy_ingest.allow_cidrs,
        ));
        app_mut.soranet_privacy_rate_limiter = crate::limits::RateLimiter::new(
            app_mut
                .soranet_privacy_ingest
                .rate_per_sec
                .map(std::num::NonZeroU32::get),
            app_mut
                .soranet_privacy_ingest
                .burst
                .map(std::num::NonZeroU32::get),
        );
    }

    let dto = sample_privacy_event_dto();
    // Retired bearer credentials are rejected even after exact operator authentication.
    let mut retired_headers = HeaderMap::new();
    retired_headers.insert(
        "x-soranet-privacy-token",
        HeaderValue::from_static("retired-secret"),
    );
    let resp = super::test_handler_post_soranet_privacy_event_with_ingress(
        State(app.clone()),
        retired_headers,
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(dto.clone()),
    )
    .await
    .expect("handler executes");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let metrics = app.telemetry.metrics().await;
    let retired = metrics
        .soranet_privacy_ingest_reject_total
        .get_metric_with_label_values(&["event", "retired_token"])
        .unwrap()
        .get();
    assert!(retired >= 1);

    // Wrong namespace
    let resp = super::test_handler_post_soranet_privacy_event_with_ingress(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(dto.clone()),
    )
    .await
    .expect("handler executes");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Happy path
    let ok_resp = super::test_handler_post_soranet_privacy_event_with_ingress(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(dto.clone()),
    )
    .await
    .expect("handler executes");
    assert_eq!(ok_resp.status(), StatusCode::ACCEPTED);

    // Rate limit on second immediate call
    let limited_resp = super::test_handler_post_soranet_privacy_event_with_ingress(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(dto),
    )
    .await
    .expect("handler executes");
    assert_eq!(limited_resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
#[cfg(feature = "telemetry")]
async fn privacy_ingest_denies_without_namespace_allowlist() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut =
            std::sync::Arc::get_mut(&mut app).expect("unique Arc for privacy configuration");
        app_mut.soranet_privacy_ingest.enabled = true;
        app_mut.soranet_privacy_ingest.allow_cidrs.clear();
        app_mut.soranet_privacy_allow_nets = Arc::new(Vec::new());
    }

    let dto = sample_privacy_event_dto();
    let resp = super::test_handler_post_soranet_privacy_event_with_ingress(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(dto),
    )
    .await
    .expect("handler executes");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let metrics = app.telemetry.metrics().await;
    let blocked = metrics
        .soranet_privacy_ingest_reject_total
        .get_metric_with_label_values(&["event", "namespace_blocked"])
        .unwrap()
        .get();
    assert!(
        blocked >= 1,
        "namespace rejection counter should increment for missing allow-list"
    );
}

#[tokio::test]
#[cfg(feature = "telemetry")]
async fn privacy_ingest_authenticates_before_body_decode() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut =
            std::sync::Arc::get_mut(&mut app).expect("unique Arc for privacy configuration");
        app_mut.soranet_privacy_ingest.enabled = true;
        app_mut.soranet_privacy_ingest.allow_cidrs = vec!["127.0.0.1/32".to_owned()];
        app_mut.soranet_privacy_allow_nets = Arc::new(crate::limits::parse_cidrs(
            &app_mut.soranet_privacy_ingest.allow_cidrs,
        ));
    }

    let descriptor = route_catalog::telemetry::SORANET_PRIVACY_EVENT;
    let routes = [descriptor];
    let mut builder = RouterBuilder::new(
        app.clone(),
        RouteCatalog::new(&routes),
        compiled_route_features(),
    )
    .expect("privacy route catalog is valid");
    builder.route(
        &descriptor,
        catalog_post(super::handler_post_soranet_privacy_event)
            .layer(DefaultBodyLimit::max(
                super::SORANET_PRIVACY_INGEST_MAX_BODY_BYTES,
            ))
            .authenticated_soranet_privacy_collector(app.clone(), "event"),
    );
    let (router, _) = builder.finish().expect("privacy route mounts exactly once");

    let mut request = Request::builder()
        .method(HttpMethod::POST)
        .uri(descriptor.path())
        .header("content-type", "application/json")
        .header("x-soranet-privacy-token", "retired-secret")
        .body(Body::from("{"))
        .expect("malformed request");
    request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            0,
        ))));
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("privacy route response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body = b"{";
    let uri = descriptor
        .path()
        .parse::<crate::Uri>()
        .expect("privacy route URI");
    let signed_headers = operator_signatures::signed_request_headers(
        &app.da_receipt_signer,
        app.state.network_id_ref(),
        &crate::Method::POST,
        &uri,
        body,
    )
    .expect("sign malformed privacy request");
    let mut signed_request = Request::builder()
        .method(HttpMethod::POST)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.as_slice()))
        .expect("signed malformed request");
    signed_request.headers_mut().extend(signed_headers);
    signed_request
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(std::net::SocketAddr::from((
            [127, 0, 0, 1],
            0,
        ))));
    let response = router
        .oneshot(signed_request)
        .await
        .expect("signed privacy route response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[cfg(feature = "telemetry")]
async fn privacy_ingest_blocks_without_allowlist() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut =
            std::sync::Arc::get_mut(&mut app).expect("unique Arc for privacy configuration");
        app_mut.soranet_privacy_ingest.enabled = true;
        app_mut.soranet_privacy_allow_nets = Arc::new(Vec::new());
        app_mut.soranet_privacy_rate_limiter = crate::limits::RateLimiter::new(None, None);
    }

    let resp = super::test_handler_post_soranet_privacy_event_with_ingress(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(sample_privacy_event_dto()),
    )
    .await
    .expect("handler executes");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    let metrics = app.telemetry.metrics().await;
    let namespace_blocked = metrics
        .soranet_privacy_ingest_reject_total
        .get_metric_with_label_values(&["event", "namespace_blocked"])
        .unwrap()
        .get();
    assert!(
        namespace_blocked >= 1,
        "namespace reject counter must increment"
    );
}

#[tokio::test]
#[cfg(feature = "telemetry")]
async fn privacy_share_ingest_enforces_policy() {
    let mut app = mk_app_state_for_tests();
    {
        let app_mut =
            std::sync::Arc::get_mut(&mut app).expect("unique Arc for privacy configuration");
        app_mut.soranet_privacy_ingest.enabled = true;
        app_mut.soranet_privacy_ingest.allow_cidrs =
            vec!["127.0.0.1/32".to_string(), "::1/128".to_string()];
        app_mut.soranet_privacy_ingest.rate_per_sec =
            Some(std::num::NonZeroU32::new(1).expect("nonzero"));
        app_mut.soranet_privacy_ingest.burst = Some(std::num::NonZeroU32::new(1).expect("nonzero"));
        app_mut.soranet_privacy_allow_nets = Arc::new(crate::limits::parse_cidrs(
            &app_mut.soranet_privacy_ingest.allow_cidrs,
        ));
        app_mut.soranet_privacy_rate_limiter = crate::limits::RateLimiter::new(
            app_mut
                .soranet_privacy_ingest
                .rate_per_sec
                .map(std::num::NonZeroU32::get),
            app_mut
                .soranet_privacy_ingest
                .burst
                .map(std::num::NonZeroU32::get),
        );
    }

    let share_dto = sample_privacy_share_dto();

    // Retired bearer credential -> 400, even with an authenticated operator.
    let mut retired_headers = HeaderMap::new();
    retired_headers.insert("x-api-token", HeaderValue::from_static("retired-secret"));
    let resp = super::test_handler_post_soranet_privacy_share_with_ingress(
        State(app.clone()),
        retired_headers,
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(share_dto.clone()),
    )
    .await
    .expect("handler executes");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Wrong namespace -> 403
    let resp = super::test_handler_post_soranet_privacy_share_with_ingress(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(share_dto.clone()),
    )
    .await
    .expect("handler executes");
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // Happy path -> 202
    let ok = super::test_handler_post_soranet_privacy_share_with_ingress(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(share_dto.clone()),
    )
    .await
    .expect("handler executes");
    assert_eq!(ok.status(), StatusCode::ACCEPTED);

    // Rate limit -> 429
    let limited = super::test_handler_post_soranet_privacy_share_with_ingress(
        State(app.clone()),
        HeaderMap::new(),
        axum::extract::ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 0))),
        privacy_operator(&app),
        NoritoJson(share_dto),
    )
    .await
    .expect("handler executes");
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);

    let metrics = app.telemetry.metrics().await;
    let retired = metrics
        .soranet_privacy_ingest_reject_total
        .get_metric_with_label_values(&["share", "retired_token"])
        .unwrap()
        .get();
    assert!(retired >= 1);
    let namespace = metrics
        .soranet_privacy_ingest_reject_total
        .get_metric_with_label_values(&["share", "namespace_blocked"])
        .unwrap()
        .get();
    assert!(namespace >= 1);
}

#[tokio::test]
async fn runtime_metrics_and_node_capabilities_ok() {
    let app = mk_app_state_for_tests();
    let headers = HeaderMap::new();

    let metrics_resp = super::handler_runtime_metrics(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
        None,
    )
    .await
    .expect("ok");
    assert_eq!(metrics_resp.status(), axum::http::StatusCode::OK);
    let metrics_bytes = axum::body::to_bytes(metrics_resp.into_body(), usize::MAX)
        .await
        .expect("body");
    let metrics: crate::runtime::RuntimeMetricsResponse =
        norito::json::from_slice(&metrics_bytes).expect("decode json");
    assert_eq!(metrics.abi_version, 1);

    let caps_resp = super::handler_node_capabilities(
        State(app.clone()),
        headers,
        crate::loopback_connect_info(),
        None,
    )
    .await
    .expect("ok");
    assert_eq!(caps_resp.status(), axum::http::StatusCode::OK);
    let caps_bytes = axum::body::to_bytes(caps_resp.into_body(), usize::MAX)
        .await
        .expect("body");
    let caps: crate::runtime::NodeCapabilitiesResponse =
        norito::json::from_slice(&caps_bytes).expect("decode json");
    assert_eq!(caps.abi_version, 1);
    assert_eq!(
        caps.data_model_version,
        iroha_data_model::DATA_MODEL_VERSION
    );
    assert_eq!(caps.signed_transaction_schema_hash_hex.len(), 32);
    assert_eq!(
            caps.signed_transaction_schema_hash_hex,
            hex::encode(<iroha_data_model::transaction::SignedTransaction as norito::core::NoritoSerialize>::schema_hash())
        );
    assert!(caps.crypto.sm.acceleration.scalar);
    assert!(caps.query.aggregate.v1);
    assert!(caps.query.aggregate.exact_results);
    assert_eq!(
        caps.query.aggregate.supported_resources,
        if cfg!(feature = "app_api") {
            crate::generic_query::aggregate_supported_resources()
                .iter()
                .map(|resource| (*resource).to_owned())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        }
    );
    assert!(caps.query.indexed_snapshot_marker);
    assert!(
        caps.query
            .row_enrichment_fields
            .contains(&"primary_alias_domain".to_string())
    );
    assert!(caps.query.projection.checkpoint_contract_v1);
    assert!(!caps.query.projection.da_v1_enabled);
    assert_eq!(
        caps.query.projection.checkpoint_plan_v1,
        cfg!(feature = "app_api")
    );
    assert_eq!(
        caps.query.projection.checkpoint_publish_v1,
        cfg!(feature = "app_api")
    );
    assert_eq!(
        caps.query.projection.shard_catalog_v1,
        cfg!(feature = "app_api")
    );
    assert_eq!(
        caps.query.projection.archive_export_v1,
        cfg!(feature = "app_api")
    );
    assert_eq!(caps.query.projection.archive_version, 1);
    assert_eq!(caps.query.projection.blob_class_custom_id, 1001);
    assert_eq!(
        caps.query.projection.codec,
        "application/x-iroha-query-shard+norito+zstd"
    );
    assert_eq!(
        caps.query.projection.rowset_codec,
        "application/x-iroha-query-shard-rowset+norito"
    );
    assert_eq!(caps.query.projection.compression, "zstd");
    assert_eq!(caps.query.projection.default_partition_count, 4096);
    assert!(
        caps.query
            .projection
            .metadata_keys
            .contains(&"query_projection.locator".to_string())
    );
    if cfg!(feature = "app_api") {
        assert_eq!(
            caps.query.projection.export_supported_resources,
            crate::generic_query::projection_export_supported_resources()
                .iter()
                .map(|resource| (*resource).to_owned())
                .collect::<Vec<_>>()
        );
    } else {
        assert!(caps.query.projection.export_supported_resources.is_empty());
    }
    assert!(
        caps.query
            .projection
            .latest_checkpoint_indexed_height
            .is_none()
    );
    assert!(
        caps.query
            .projection
            .latest_checkpoint_block_hash_hex
            .is_none()
    );
    assert!(
        !caps.crypto.sm.allowed_signing.is_empty(),
        "allowed_signing must advertise at least one algorithm"
    );

    let checkpoint_absent = super::handler_node_query_projection_checkpoint(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
    )
    .await
    .expect("ok");
    assert_eq!(
        checkpoint_absent.status(),
        axum::http::StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn node_query_projection_checkpoint_handler_returns_persisted_payload() {
    let app = mk_app_state_for_tests();
    let expected_hash =
        iroha_crypto::HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
            iroha_crypto::Hash::new([0x6A; iroha_crypto::Hash::LENGTH]),
        );
    app.state.persist_query_projection_checkpoint(Some(
            iroha_core::query::projection_checkpoint::QueryProjectionCheckpoint::from_index_status(
                iroha_core::query::index_status::QueryIndexStatus {
                    indexed_height: 55,
                    indexed_block_hash: Some(expected_hash),
                },
                1_714_000_555,
                vec![iroha_core::query::projection_checkpoint::QueryProjectionCheckpointShard {
                    resource:
                        iroha_core::query::projection_checkpoint::QueryProjectionResourceKind::Accounts,
                    partition_id: 3,
                    asset_definition_id: None,
                    manifest_digest: iroha_data_model::da::types::BlobDigest::new([0x11; 32]),
                    storage_ticket: iroha_data_model::da::types::StorageTicketId::new([0x22; 32]),
                    blob_hash: iroha_data_model::da::types::BlobDigest::new([0x33; 32]),
                }],
            ),
        ));

    let response = super::handler_node_query_projection_checkpoint(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
    )
    .await
    .expect("ok");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(
        response.headers().get(axum::http::header::CONTENT_TYPE),
        Some(&axum::http::HeaderValue::from_static(
            crate::utils::NORITO_MIME_TYPE,
        ))
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let checkpoint: crate::runtime::NodeProjectionCheckpointResponse =
        norito::decode_from_bytes(&body).expect("decode default Norito response");
    assert_eq!(checkpoint.indexed_height, 55);
    assert_eq!(
        checkpoint.indexed_block_hash_hex,
        Some(hex::encode(expected_hash.as_ref()))
    );
    assert_eq!(checkpoint.shards.len(), 1);
    assert_eq!(checkpoint.shards[0].resource, "accounts");
}

#[cfg(feature = "app_api")]
async fn projection_checkpoint_request_for_app(
    app: &SharedAppState,
    emitted_at_unix: u64,
    archive_emitted_at_unix: u64,
    manifest_seed: u8,
    ticket_seed: u8,
) -> crate::runtime::NodeProjectionCheckpointPublishRequest {
    let mut shards = Vec::new();
    let mut next_seed = 0u8;
    for resource in crate::generic_query::projection_export_supported_resources() {
        let catalog = crate::runtime::handle_node_query_projection_shard_catalog(
            app.state.clone(),
            (*resource).to_owned(),
            crate::runtime::NodeProjectionShardCatalogQuery {
                asset_definition_id: None,
                offset: None,
                limit: None,
            },
        )
        .await
        .expect("build projection shard catalog");
        for entry in catalog.entries {
            shards.push(crate::runtime::NodeProjectionCheckpointPublishShardRef {
                resource: (*resource).to_owned(),
                partition_id: entry.partition_id,
                asset_definition_id: entry.asset_definition_id,
                archive_emitted_at_unix,
                manifest_digest_hex: hex::encode([manifest_seed.wrapping_add(next_seed); 32]),
                storage_ticket_hex: hex::encode([ticket_seed.wrapping_add(next_seed); 32]),
            });
            next_seed = next_seed.wrapping_add(1);
        }
    }

    crate::runtime::NodeProjectionCheckpointPublishRequest {
        emitted_at_unix: Some(emitted_at_unix),
        shards,
    }
}

#[cfg(feature = "app_api")]
async fn projection_checkpoint_request_for_app_with_real_manifests(
    app: &SharedAppState,
    emitted_at_unix: u64,
    archive_emitted_at_unix: u64,
    ticket_seed: u8,
) -> crate::runtime::NodeProjectionCheckpointPublishRequest {
    let mut shards = Vec::new();
    let mut next_seed = 0u8;
    for resource in crate::generic_query::projection_export_supported_resources() {
        let catalog = crate::runtime::handle_node_query_projection_shard_catalog(
            app.state.clone(),
            (*resource).to_owned(),
            crate::runtime::NodeProjectionShardCatalogQuery {
                asset_definition_id: None,
                offset: None,
                limit: None,
            },
        )
        .await
        .expect("build projection shard catalog");
        for entry in catalog.entries {
            let archive = match *resource {
                "accounts" => crate::runtime::build_accounts_projection_shard_archive(
                    app.state.as_ref(),
                    entry.partition_id,
                    archive_emitted_at_unix,
                ),
                "account_assets" => crate::runtime::build_account_assets_projection_shard_archive(
                    app.state.as_ref(),
                    entry.partition_id,
                    archive_emitted_at_unix,
                ),
                "asset_holders" => crate::runtime::build_asset_holders_projection_shard_archive(
                    app.state.as_ref(),
                    entry
                        .asset_definition_id
                        .as_deref()
                        .expect("asset_holders catalog entry asset definition"),
                    entry.partition_id,
                    archive_emitted_at_unix,
                ),
                "asset_definitions" => {
                    crate::runtime::build_asset_definitions_projection_shard_archive(
                        app.state.as_ref(),
                        entry.partition_id,
                        archive_emitted_at_unix,
                    )
                }
                "domains" => crate::runtime::build_domains_projection_shard_archive(
                    app.state.as_ref(),
                    entry.partition_id,
                    archive_emitted_at_unix,
                ),
                other => panic!("unsupported projection checkpoint test resource: {other}"),
            }
            .expect("build projection shard archive");
            let (_, _, manifest) =
                crate::routing::query_projection_archive_storage_artifacts(&archive)
                    .expect("build projection archive storage artifacts");
            let manifest_digest_hex = hex::encode(
                manifest
                    .digest()
                    .expect("digest projection archive manifest")
                    .as_bytes(),
            );
            shards.push(crate::runtime::NodeProjectionCheckpointPublishShardRef {
                resource: (*resource).to_owned(),
                partition_id: entry.partition_id,
                asset_definition_id: entry.asset_definition_id,
                archive_emitted_at_unix,
                manifest_digest_hex,
                storage_ticket_hex: hex::encode([ticket_seed.wrapping_add(next_seed); 32]),
            });
            next_seed = next_seed.wrapping_add(1);
        }
    }

    crate::runtime::NodeProjectionCheckpointPublishRequest {
        emitted_at_unix: Some(emitted_at_unix),
        shards,
    }
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn node_query_projection_checkpoint_plan_handler_returns_preview_payload() {
    use iroha_data_model::Registrable;
    use iroha_data_model::prelude::{Account, Domain, DomainId};

    let authority =
        checked_torii_test_ed25519_keypair(0x99, "derive projection plan authority fixture key");
    let alice =
        checked_torii_test_ed25519_keypair(0x9a, "derive projection plan alice fixture key");
    let authority_id = iroha_data_model::account::AccountId::new(authority.public_key().clone());
    let alice_id = iroha_data_model::account::AccountId::new(alice.public_key().clone());
    let domain_id = DomainId::try_new("projection-plan-handler", "universal").expect("domain");
    let world = iroha_core::state::World::with(
        [Domain::new(domain_id).build(&authority_id)],
        [
            Account::new(authority_id.clone()).build(&authority_id),
            Account::new(alice_id.clone()).build(&authority_id),
        ],
        [],
    );
    let app = mk_app_state_for_tests_with_world(world);
    let request =
        projection_checkpoint_request_for_app(&app, 1_714_002_111, 1_714_002_000, 0x21, 0x31).await;

    let response = super::handler_node_query_projection_checkpoint_plan(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        crate::utils::extractors::NoritoJson(request),
    )
    .await
    .expect("ok");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(
        response.headers().get(axum::http::header::CONTENT_TYPE),
        Some(&axum::http::HeaderValue::from_static(
            crate::utils::NORITO_MIME_TYPE,
        ))
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let checkpoint: crate::runtime::NodeProjectionCheckpointResponse =
        norito::decode_from_bytes(&body).expect("decode default Norito response");
    assert_eq!(checkpoint.emitted_at_unix, 1_714_002_111);
    assert!(
        !checkpoint.shards.is_empty(),
        "checkpoint preview must include the canonical live shard set"
    );
    assert!(
        app.state.query_projection_checkpoint_snapshot().is_none(),
        "plan route must not persist checkpoint state"
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn node_query_projection_checkpoint_publish_handler_persists_payload() {
    use iroha_data_model::Registrable;
    use iroha_data_model::prelude::{Account, Domain, DomainId};

    let authority =
        checked_torii_test_ed25519_keypair(0x9b, "derive projection publish authority fixture key");
    let alice =
        checked_torii_test_ed25519_keypair(0x9c, "derive projection publish alice fixture key");
    let authority_id = iroha_data_model::account::AccountId::new(authority.public_key().clone());
    let alice_id = iroha_data_model::account::AccountId::new(alice.public_key().clone());
    let domain_id = DomainId::try_new("projection-publish-handler", "universal").expect("domain");
    let world = iroha_core::state::World::with(
        [Domain::new(domain_id).build(&authority_id)],
        [
            Account::new(authority_id.clone()).build(&authority_id),
            Account::new(alice_id.clone()).build(&authority_id),
        ],
        [],
    );
    let app = mk_app_state_for_tests_with_world(world);
    let request =
        projection_checkpoint_request_for_app(&app, 1_714_002_333, 1_714_002_222, 0x41, 0x51).await;

    let response = super::handler_node_query_projection_checkpoint_publish(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        crate::utils::extractors::NoritoJson(request),
    )
    .await
    .expect("ok");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(
        response.headers().get(axum::http::header::CONTENT_TYPE),
        Some(&axum::http::HeaderValue::from_static(
            crate::utils::NORITO_MIME_TYPE,
        ))
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let checkpoint: crate::runtime::NodeProjectionCheckpointResponse =
        norito::decode_from_bytes(&body).expect("decode default Norito response");
    assert_eq!(checkpoint.emitted_at_unix, 1_714_002_333);
    assert!(
        !checkpoint.shards.is_empty(),
        "checkpoint publish must persist the canonical live shard set"
    );
    assert_eq!(
        app.state
            .query_projection_checkpoint_snapshot()
            .expect("persisted")
            .emitted_at_unix,
        1_714_002_333
    );
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn node_query_projection_checkpoint_publish_handler_seeds_local_projection_store() {
    use iroha_data_model::Registrable;
    use iroha_data_model::prelude::{Account, Domain, DomainId};

    let authority = checked_torii_test_ed25519_keypair(
        0x9d,
        "derive durable projection publish authority fixture key",
    );
    let alice = checked_torii_test_ed25519_keypair(
        0x9e,
        "derive durable projection publish alice fixture key",
    );
    let authority_id = iroha_data_model::account::AccountId::new(authority.public_key().clone());
    let alice_id = iroha_data_model::account::AccountId::new(alice.public_key().clone());
    let domain_id = DomainId::try_new("projection-publish-durable", "universal").expect("domain");
    let world = iroha_core::state::World::with(
        [Domain::new(domain_id).build(&authority_id)],
        [
            Account::new(authority_id.clone()).build(&authority_id),
            Account::new(alice_id.clone()).build(&authority_id),
        ],
        [],
    );
    let app = mk_app_state_for_tests_with_world(world);
    let mut inner = Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state"));
    let storage_dir = tempfile::tempdir().expect("temp storage dir");
    let canonical_storage_root = storage_dir
        .path()
        .canonicalize()
        .expect("canonical temp storage root");
    inner.sorafs_node = sorafs_node::NodeHandle::new(
        sorafs_node::config::StorageConfig::builder()
            .enabled(true)
            .data_dir(canonical_storage_root.join("storage"))
            .build(),
    );
    let app = Arc::new(inner);
    let request = projection_checkpoint_request_for_app_with_real_manifests(
        &app,
        1_714_002_555,
        1_714_002_444,
        0x61,
    )
    .await;

    let response = super::handler_node_query_projection_checkpoint_publish(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        crate::utils::extractors::NoritoJson(request),
    )
    .await
    .expect("ok");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(
        response.headers().get(axum::http::header::CONTENT_TYPE),
        Some(&axum::http::HeaderValue::from_static(
            crate::utils::NORITO_MIME_TYPE,
        ))
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let checkpoint: crate::runtime::NodeProjectionCheckpointResponse =
        norito::decode_from_bytes(&body).expect("decode default Norito response");

    for shard in &checkpoint.shards {
        let manifest_digest_bytes =
            hex::decode(&shard.manifest_digest_hex).expect("decode manifest digest");
        let manifest_digest =
            <[u8; 32]>::try_from(manifest_digest_bytes.as_slice()).expect("manifest digest length");
        assert!(
            app.sorafs_node
                .manifest_metadata_by_digest(&manifest_digest)
                .is_ok(),
            "published checkpoint shard should seed local SoraFS storage"
        );
    }
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn node_query_projection_shard_catalog_handler_returns_catalog_payload() {
    let app = mk_app_state_for_tests();
    let response = super::handler_node_query_projection_shard_catalog(
        State(app),
        AxPath("accounts".to_owned()),
        AxQuery(crate::runtime::NodeProjectionShardCatalogQuery {
            asset_definition_id: None,
            offset: Some(0),
            limit: Some(32),
        }),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
    )
    .await
    .expect("ok");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(
        response.headers().get(axum::http::header::CONTENT_TYPE),
        Some(&axum::http::HeaderValue::from_static(
            crate::utils::NORITO_MIME_TYPE,
        ))
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let catalog: crate::runtime::NodeProjectionShardCatalogResponse =
        norito::decode_from_bytes(&body).expect("decode default Norito response");
    assert_eq!(catalog.resource, "accounts");
    assert_eq!(catalog.limit, 32);
    assert_eq!(catalog.offset, 0);
    assert!(catalog.total_entries >= catalog.entries.len() as u64);
}

#[cfg(feature = "app_api")]
#[tokio::test]
async fn node_query_projection_shard_export_handler_returns_binary_archive() {
    let app = mk_app_state_for_tests();
    let response = super::handler_node_query_projection_shard_export(
        State(app),
        AxPath(("accounts".to_owned(), 0)),
        AxQuery(crate::runtime::NodeProjectionShardExportQuery {
            asset_definition_id: None,
        }),
        HeaderMap::new(),
        crate::loopback_connect_info(),
    )
    .await
    .expect("ok");
    assert_eq!(response.status(), axum::http::StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .map(axum::http::HeaderValue::as_bytes),
        Some(b"application/octet-stream".as_slice())
    );
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let archive: iroha_core::query::projection_shard::QueryProjectionShardArchive =
        norito::decode_from_bytes(&bytes).expect("decode archive");
    assert_eq!(
        archive.resource,
        iroha_core::query::projection_checkpoint::QueryProjectionResourceKind::Accounts
    );
    assert_eq!(archive.partition_id, 0);
}

#[tokio::test]
async fn core_info_handlers_ok() {
    let app = mk_app_state_for_tests();
    let headers = HeaderMap::new();

    // configuration
    let resp = super::handler_get_configuration(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let config_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("config body");
    let config: ConfigGetDTO =
        norito::json::from_slice(&config_bytes).expect("decode config payload");
    assert!(
        !config
            .network
            .soranet_handshake
            .descriptor_commit_hex
            .is_empty(),
        "handshake descriptor should be present in config payload"
    );
    assert!(
        config.network.soranet_handshake.pow.puzzle.is_some(),
        "puzzle gate should be advertised in configuration payload"
    );

    // peers
    let mut peer_headers = headers.clone();
    peer_headers.insert(
        axum::http::header::ACCEPT,
        axum::http::HeaderValue::from_static("application/json"),
    );
    let resp = super::handler_peers(
        State(app.clone()),
        peer_headers,
        crate::loopback_connect_info(),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let peer_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("peers body");
    let peers: HashSet<Peer> = norito::json::from_slice(&peer_bytes).expect("peers JSON");
    assert!(peers.is_empty());

    // A generic test state intentionally has no authenticated ABI-21/V4
    // release, issuer, or escrow catalog. `/health` is readiness (not
    // liveness), so it must fail closed for this fixture.
    // For ConnectInfo we can pass a dummy loopback address by constructing the extractor arg manually is not possible here.
    // Instead, rely on non-allowlist path (headers don't carry the internal x-iroha-remote-addr), which doesn't need ConnectInfo IP.
    let resp = super::handler_health(
        State(app),
        headers,
        axum::extract::ConnectInfo(std::net::SocketAddr::from(([127, 0, 0, 1], 0))),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::SERVICE_UNAVAILABLE);
    let health_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("health body");
    let health: norito::json::Value =
        norito::json::from_slice(&health_bytes).expect("decode health payload");
    assert_eq!(
        health
            .get("cash_handoff_capability")
            .and_then(norito::json::Value::as_str),
        Some("cash_handoff_v1")
    );
    assert_eq!(
        health
            .get("required_bridge_abi_version")
            .and_then(norito::json::Value::as_u64),
        Some(22)
    );
    assert_eq!(
        health.get("ready").and_then(norito::json::Value::as_bool),
        Some(false)
    );
}

#[tokio::test]
async fn time_handlers_ok() {
    let app = mk_app_state_for_tests();
    let headers = HeaderMap::new();

    let resp = super::handler_time_now(
        State(app.clone()),
        headers.clone(),
        crate::loopback_connect_info(),
    )
    .await
    .expect("ok")
    .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    let resp = super::handler_time_status(State(app), headers, crate::loopback_connect_info())
        .await
        .expect("ok")
        .into_response();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
}

#[cfg(all(feature = "app_api", feature = "push"))]
fn push_test_identity(seed: u8) -> (KeyPair, AccountId) {
    let key_pair = checked_torii_test_keypair(
        vec![seed; 32],
        iroha_crypto::Algorithm::Ed25519,
        "derive push fixture key",
    );
    let account_id = AccountId::new(key_pair.public_key().clone());
    (key_pair, account_id)
}

#[cfg(all(feature = "app_api", feature = "push"))]
fn push_test_config() -> iroha_config::parameters::actual::Push {
    iroha_config::parameters::actual::Push {
        enabled: true,
        fcm_project_id: Some("project".to_string()),
        fcm_service_account_path: Some(std::path::PathBuf::from("/tmp/service-account.json")),
        ..Default::default()
    }
}

#[cfg(all(feature = "app_api", feature = "push"))]
fn mk_push_request(account_id: &AccountId, token: &str) -> push::RegisterDeviceRequest {
    push::RegisterDeviceRequest {
        account_id: account_id.to_string(),
        platform: "FCM".to_string(),
        token: token.to_string(),
        topics: Some(vec!["orders".into()]),
    }
}

#[cfg(all(feature = "app_api", feature = "push"))]
fn signed_push_json<T>(
    account_id: &AccountId,
    key_pair: &KeyPair,
    method: Method,
    uri: axum::http::Uri,
    value: T,
) -> (Method, axum::http::Uri, HeaderMap, axum::body::Bytes)
where
    T: norito::json::JsonSerialize,
{
    let body = norito::json::to_vec(&value).expect("encode push body");
    let mut headers = signed_app_headers(account_id, key_pair, &method, &uri, body.as_ref());
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    (method, uri, headers, axum::body::Bytes::from(body))
}

#[cfg(all(feature = "app_api", feature = "push"))]
async fn extract_error(resp: AxResponse) -> ErrorEnvelope {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("error body");
    norito::decode_from_bytes(&bytes).expect("decode error envelope")
}

#[cfg(all(feature = "app_api", feature = "push"))]
#[tokio::test]
async fn push_registration_rejected_when_disabled() {
    let (key_pair, account_id) = push_test_identity(1);
    let app = mk_app_state_for_tests_with_world(world_with_account(&account_id));
    let req = mk_push_request(&account_id, "t-disabled");
    let uri: axum::http::Uri = "/v1/notify/devices".parse().expect("uri");
    let (method, uri, headers, body) =
        signed_push_json(&account_id, &key_pair, Method::POST, uri, req);

    let resp =
        super::handler_push_register_device(State(app.clone()), method, uri, headers, body).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let err = extract_error(resp).await;
    assert_eq!(err.code(), "push_disabled");
    assert!(
        app.push.is_none(),
        "push bridge should be absent by default"
    );
}

#[cfg(all(feature = "app_api", feature = "push"))]
#[tokio::test]
async fn push_registration_requires_credentials() {
    let (key_pair, account_id) = push_test_identity(2);
    let _data_dir = crate::test_utils::TestDataDirGuard::new();
    let app = mk_app_state_for_tests_with_world_and_push(
        world_with_account(&account_id),
        iroha_config::parameters::actual::Push {
            enabled: true,
            ..Default::default()
        },
    );
    let req = mk_push_request(&account_id, "t-missing-creds");
    let uri: axum::http::Uri = "/v1/notify/devices".parse().expect("uri");
    let (method, uri, headers, body) =
        signed_push_json(&account_id, &key_pair, Method::POST, uri, req);

    let resp =
        super::handler_push_register_device(State(app.clone()), method, uri, headers, body).await;
    assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    let err = extract_error(resp).await;
    assert_eq!(err.code(), "push_missing_credentials");
    let bridge = app.push.as_ref().expect("push bridge configured");
    assert_eq!(bridge.device_count(), 0);
}

#[cfg(all(feature = "app_api", feature = "push"))]
#[tokio::test]
async fn push_registration_succeeds_with_credentials() {
    let (key_pair, account_id) = push_test_identity(3);
    let _data_dir = crate::test_utils::TestDataDirGuard::new();
    let app = mk_app_state_for_tests_with_world_and_push(
        world_with_account(&account_id),
        push_test_config(),
    );
    let req = mk_push_request(&account_id, "t-success");
    let uri: axum::http::Uri = "/v1/notify/devices".parse().expect("uri");
    let (method, uri, headers, body) =
        signed_push_json(&account_id, &key_pair, Method::POST, uri, req);

    let resp =
        super::handler_push_register_device(State(app.clone()), method, uri, headers, body).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bridge = app.push.as_ref().expect("push bridge configured");
    assert_eq!(bridge.device_count(), 1);
}

#[cfg(all(feature = "app_api", feature = "push"))]
#[tokio::test]
async fn push_registration_accepts_account_alias_and_stores_canonical_i105() {
    let (key_pair, canonical_account) = push_test_identity(4);
    let _data_dir = crate::test_utils::TestDataDirGuard::new();
    let app = mk_app_state_for_tests_with_world_and_push(
        world_with_account(&canonical_account),
        push_test_config(),
    );
    let mut req = mk_push_request(&canonical_account, "t-alias");
    bind_account_alias_for_test(&app, &canonical_account, "wallet@universal");
    req.account_id = "wallet@universal".to_string();
    let uri: axum::http::Uri = "/v1/notify/devices".parse().expect("uri");
    let (method, uri, headers, body) =
        signed_push_json(&canonical_account, &key_pair, Method::POST, uri, req);

    let resp =
        super::handler_push_register_device(State(app.clone()), method, uri, headers, body).await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let bridge = app.push.as_ref().expect("push bridge configured");
    let device = bridge
        .registered_device("t-alias")
        .expect("registered device should exist");
    assert_eq!(device.account_id, canonical_account.to_string());
}

#[cfg(all(feature = "app_api", feature = "push"))]
#[tokio::test]
async fn push_device_writes_authenticate_before_media_and_body_decode() {
    let (_key_pair, account_id) = push_test_identity(5);
    let _data_dir = crate::test_utils::TestDataDirGuard::new();
    let app = mk_app_state_for_tests_with_world_and_push(
        world_with_account(&account_id),
        push_test_config(),
    );
    let register = super::handler_push_register_device(
        State(app.clone()),
        Method::POST,
        "/v1/notify/devices".parse().expect("uri"),
        HeaderMap::new(),
        axum::body::Bytes::from_static(b"{malformed"),
    )
    .await;
    let unregister = super::handler_push_unregister_device(
        State(app),
        Method::DELETE,
        "/v1/notify/devices".parse().expect("uri"),
        HeaderMap::new(),
        axum::body::Bytes::from_static(b"{malformed"),
    )
    .await;
    for response in [register, unregister] {
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let error = extract_error(response).await;
        assert_eq!(error.code(), "push_auth_required");
    }
}

#[cfg(all(feature = "app_api", feature = "push"))]
#[tokio::test]
async fn push_registration_rejects_body_account_mismatch_without_mutation() {
    let (signer_keys, signer) = push_test_identity(0x51);
    let (_other_keys, other) = push_test_identity(0x52);
    let _data_dir = crate::test_utils::TestDataDirGuard::new();
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id).build(&signer);
    let world = World::with(
        [domain],
        [
            Account::new(signer.clone()).build(&signer),
            Account::new(other.clone()).build(&signer),
        ],
        [],
    );
    let app = mk_app_state_for_tests_with_world_and_push(world, push_test_config());
    let request = mk_push_request(&other, "t-mismatch");
    let uri: axum::http::Uri = "/v1/notify/devices".parse().expect("uri");
    let (method, uri, headers, body) =
        signed_push_json(&signer, &signer_keys, Method::POST, uri, request);

    let response =
        super::handler_push_register_device(State(app.clone()), method, uri, headers, body).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let error = extract_error(response).await;
    assert_eq!(error.code(), "push_account_mismatch");
    assert_eq!(
        app.push
            .as_ref()
            .expect("push bridge configured")
            .device_count(),
        0
    );
}

#[cfg(all(feature = "app_api", feature = "push"))]
#[tokio::test]
async fn push_unregister_removes_device() {
    let (key_pair, account_id) = push_test_identity(6);
    let _data_dir = crate::test_utils::TestDataDirGuard::new();
    let app = mk_app_state_for_tests_with_world_and_push(
        world_with_account(&account_id),
        push_test_config(),
    );
    let uri: axum::http::Uri = "/v1/notify/devices".parse().expect("uri");
    let req = mk_push_request(&account_id, "t-remove");
    let (method, signed_uri, headers, body) = signed_push_json(
        &account_id,
        &key_pair,
        Method::POST,
        uri.clone(),
        req.clone(),
    );
    let resp =
        super::handler_push_register_device(State(app.clone()), method, signed_uri, headers, body)
            .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);

    let (method, signed_uri, headers, body) =
        signed_push_json(&account_id, &key_pair, Method::DELETE, uri, req);
    let resp = super::handler_push_unregister_device(
        State(app.clone()),
        method,
        signed_uri,
        headers,
        body,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
    let bridge = app.push.as_ref().expect("push bridge configured");
    assert_eq!(bridge.device_count(), 0);
}

fn make_signed_block(
    height: u64,
    prev_hash: Option<HashOf<BlockHeader>>,
) -> (SignedBlock, HashOf<TransactionEntrypoint>) {
    let keypair = checked_torii_test_ed25519_keypair(0x24, "derive Torii block-header fixture key");
    let authority = AccountId::new(keypair.public_key().clone());
    let tx = checked_torii_test_transaction(
        TransactionBuilder::new(
            signed_query_test_network_id(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        ),
        &keypair,
        "sign Torii block-header fixture transaction",
    );
    let entry_hash = tx.hash_as_entrypoint();
    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("nonzero height"),
        prev_hash,
        None,
        None,
        0,
        0,
    );
    let signature = checked_torii_test_block_signature(
        0,
        &keypair,
        &header,
        "sign Torii block-header fixture block",
    );
    let mut block = SignedBlock::presigned(signature, header, vec![tx]);
    let entry_hashes = [entry_hash];
    block
        .set_transaction_results(
            Vec::new(),
            &entry_hashes,
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("test block entrypoint hash should match payload");
    (block, entry_hash)
}

struct PersistedDataTriggerCompletionBlock {
    block: SignedBlock,
    tx_hash: HashOf<SignedTransaction>,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    trigger_execution_hash: HashOf<TransactionEntrypoint>,
    trigger_id: TriggerId,
}

fn make_persisted_data_trigger_completion_block(
    height: u64,
    prev_hash: Option<HashOf<BlockHeader>>,
) -> PersistedDataTriggerCompletionBlock {
    let keypair =
        checked_torii_test_ed25519_keypair(0x25, "derive Torii trigger-completion fixture key");
    let authority = AccountId::new(keypair.public_key().clone());
    let tx = checked_torii_test_transaction(
        TransactionBuilder::new(
            signed_query_test_network_id(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        ),
        &keypair,
        "sign Torii trigger-completion fixture transaction",
    );
    let tx_hash = tx.hash();
    let entrypoint_hash = tx.hash_as_entrypoint();
    let trigger_id: TriggerId = "persisted_data_trigger".parse().expect("trigger id");
    let step = DataTriggerStep {
        id: trigger_id.clone(),
        instructions: ExecutionStep(ConstVec::new_empty()),
    };
    let trigger_execution_hash = TimeTriggerEntrypoint {
        id: trigger_id.clone(),
        instructions: step.instructions.clone(),
        authority,
    }
    .hash_as_entrypoint();
    assert_ne!(trigger_execution_hash, entrypoint_hash);

    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("nonzero height"),
        prev_hash,
        None,
        None,
        0,
        0,
    );
    let signature = checked_torii_test_block_signature(
        0,
        &keypair,
        &header,
        "sign Torii trigger-completion fixture block",
    );
    let mut block = SignedBlock::presigned(signature, header, vec![tx]);
    block
        .set_transaction_results(
            Vec::new(),
            &[entrypoint_hash],
            vec![TransactionResultInner::Ok(vec![step])],
        )
        .expect("test block entrypoint hash should match payload");
    block.set_trigger_completions(vec![TriggerCompletedEvent::new(
        trigger_id.clone(),
        trigger_execution_hash,
        0,
        TriggerCompletedOutcome::Success,
    )]);

    PersistedDataTriggerCompletionBlock {
        block,
        tx_hash,
        entrypoint_hash,
        trigger_execution_hash,
        trigger_id,
    }
}

fn make_sealed_reveal_block(
    height: u64,
    prev_hash: Option<HashOf<BlockHeader>>,
) -> (SignedBlock, HashOf<TransactionEntrypoint>) {
    let keypair =
        checked_torii_test_ed25519_keypair(0x26, "derive Torii sealed-reveal fixture key");
    let network_id = signed_query_test_network_id();
    let authority = AccountId::new(keypair.public_key().clone());
    let tx = checked_torii_test_transaction(
        TransactionBuilder::new(
            network_id,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        ),
        &keypair,
        "sign Torii sealed-reveal fixture transaction",
    );
    let salt = [0xA7; 32];
    let commitment = compute_sealed_transaction_commitment(&network_id, &tx, salt, height + 2);
    let reveal = SealedTransactionReveal::new(commitment, tx.clone(), salt);
    let entrypoint = TransactionEntrypoint::SealedReveal(reveal);
    let entry_hash = entrypoint.hash();
    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("nonzero height"),
        prev_hash,
        None,
        None,
        0,
        0,
    );
    let signature = checked_torii_test_block_signature(
        0,
        &keypair,
        &header,
        "sign Torii sealed-reveal fixture block",
    );
    let mut block = SignedBlock::presigned(signature, header, vec![tx]);
    block.set_external_entrypoints(vec![entrypoint]);
    let entry_hashes = [entry_hash];
    block
        .set_transaction_results(
            Vec::new(),
            &entry_hashes,
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("test block entrypoint hash should match payload");
    (block, entry_hash)
}

fn store_block(app: &SharedAppState, block: SignedBlock) -> HashOf<BlockHeader> {
    let hash = block.hash();
    app.kura.store_block(Arc::new(block)).expect("store block");
    hash
}

fn record_committed_block_hash_for_test(
    app: &SharedAppState,
    header: BlockHeader,
    block_hash: HashOf<BlockHeader>,
) {
    let mut block_hashes = app.state.block_hashes.block();
    block_hashes.push_for_tests(block_hash);
    block_hashes.commit_for_tests();
    app.state.update_latest_block_header_cache_for_tests(header);
}

fn make_empty_signed_block(
    height: u64,
    prev_hash: Option<HashOf<BlockHeader>>,
    creation_time_ms: u64,
) -> SignedBlock {
    let keypair = checked_torii_test_ed25519_keypair(0x27, "derive Torii empty-header fixture key");
    let header = BlockHeader::new(
        NonZeroU64::new(height).expect("nonzero height"),
        prev_hash,
        None,
        None,
        creation_time_ms,
        0,
    );
    let signature = checked_torii_test_block_signature(
        0,
        &keypair,
        &header,
        "sign Torii empty committed-header fixture block",
    );
    SignedBlock::presigned(signature, header, Vec::new())
}

pub(crate) fn record_latest_committed_header_for_test(
    app: &SharedAppState,
    height: u64,
    creation_time_ms: u64,
) {
    let durable_blocks_count = app
        .kura
        .exact_durable_blocks_count()
        .expect("test Kura durable boundary must remain readable");
    assert_eq!(
        app.state.committed_height(),
        durable_blocks_count,
        "test block hash journal must match durable Kura height before appending headers"
    );
    let durable_height = u64::try_from(durable_blocks_count).expect("durable height fits into u64");
    assert!(
        height > durable_height,
        "latest test header height must advance durable Kura height"
    );

    let mut prev_hash = NonZeroUsize::new(durable_height.try_into().expect("height fits usize"))
        .and_then(|height| app.kura.get_block(height))
        .map(|block| block.hash());
    let mut block_hashes = app.state.block_hashes.block();
    let mut latest_header = None;
    for next_height in durable_height.saturating_add(1)..=height {
        let timestamp = if next_height == height {
            creation_time_ms
        } else {
            0
        };
        let block = make_empty_signed_block(next_height, prev_hash, timestamp);
        latest_header = Some(block.header());
        let hash = store_block(app, block);
        block_hashes.push_for_tests(hash);
        prev_hash = Some(hash);
    }
    block_hashes.commit_for_tests();
    if let Some(header) = latest_header {
        app.state.update_latest_block_header_cache_for_tests(header);
    }
}
