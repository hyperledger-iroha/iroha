// Exercise HTTP status/header parsing through the actual gateway adapter and scheduler.
#[tokio::test(start_paused = true)]
async fn gateway_retry_after_reaches_scheduler_without_poisoning_provider_health() {
    struct QuotaEngine {
        bytes: Vec<u8>,
        calls: std::sync::atomic::AtomicUsize,
    }
    impl HttpEngine for QuotaEngine {
        fn get(&self, _: HttpRequest) -> HttpFuture {
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            let mut headers = HeaderMap::new();
            headers.insert(reqwest::header::RETRY_AFTER, HeaderValue::from_static("2"));
            let response = if call == 0 {
                HttpResponse {
                    status: StatusCode::TOO_MANY_REQUESTS,
                    headers,
                    body: br#"{"error":"stream token rate limit exceeded"}"#.to_vec(),
                }
            } else {
                HttpResponse {
                    status: StatusCode::OK,
                    headers: HeaderMap::new(),
                    body: self.bytes.clone(),
                }
            };
            Box::pin(async move { Ok(response) })
        }
    }
    let payload = sample_payload(8192);
    let plan = plan_for_payload(&payload);
    let token = sample_stream_token(
        &manifest_root_cid_for_payload(&payload),
        &provider_id_hex(),
        &chunker_handle(),
        1,
    );
    let engine = Arc::new(QuotaEngine {
        bytes: payload.clone(),
        calls: std::sync::atomic::AtomicUsize::new(0),
    });
    let config = gateway_config(&manifest_id_from_payload(&payload), &chunker_handle());
    let context = GatewayFetchContext::build_with_engine(
        config,
        [gateway_provider_input(&token)],
        engine.clone(),
    )
    .unwrap();
    let started = tokio::time::Instant::now();
    let outcome = context
        .execute_plan(
            &plan,
            FetchOptions {
                per_chunk_retry_limit: Some(1),
                provider_failure_threshold: 1,
                ..FetchOptions::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(outcome.assemble_payload(), payload);
    assert_eq!(engine.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        tokio::time::Instant::now().duration_since(started),
        Duration::from_secs(2)
    );
    assert_eq!(outcome.provider_reports[0].failures, 0);
    assert!(!outcome.provider_reports[0].disabled);
}

#[tokio::test]
async fn gateway_policy_body_reaches_scheduler_as_structured_evidence() {
    let payload = sample_payload(8192);
    let plan = plan_for_payload(&payload);
    let token = sample_stream_token(
        &manifest_root_cid_for_payload(&payload),
        &provider_id_hex(),
        &chunker_handle(),
        1,
    );
    let path = format!(
        "/v1/sorafs/storage/chunk/{}/{}",
        manifest_id_from_payload(&payload),
        hex::encode(plan.chunks[0].digest)
    );
    let engine = Arc::new(MockHttpEngine::new(HashMap::from([(path, HttpResponse {
        status: StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
        headers: HeaderMap::new(),
        body: format!(r#"{{"error":"gateway_compliance_denied","source":"baseline","catalog_digest_hex":"{}"}}"#, "ab".repeat(32)).into_bytes(),
    })])));
    let config = gateway_config(&manifest_id_from_payload(&payload), &chunker_handle());
    let context =
        GatewayFetchContext::build_with_engine(config, [gateway_provider_input(&token)], engine)
            .unwrap();
    let error = context
        .execute_plan(
            &plan,
            FetchOptions {
                per_chunk_retry_limit: Some(1),
                ..FetchOptions::default()
            },
        )
        .await
        .unwrap_err();
    let MultiSourceError::ExhaustedRetries { last_error, .. } = error else {
        panic!("unexpected failure")
    };
    let AttemptFailure::Provider {
        policy_block: Some(evidence),
        ..
    } = last_error.failure
    else {
        panic!("lost policy evidence")
    };
    assert_eq!(evidence.code, "gateway_compliance_denied");
    assert_eq!(evidence.source, "baseline");
    assert_eq!(evidence.catalog_digest_hex, "ab".repeat(32));
}
