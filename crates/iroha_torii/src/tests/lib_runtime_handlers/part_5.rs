include!("part_5_pipeline_cache.rs");
async fn pipeline_status_response(
    app: SharedAppState,
    hash: String,
    scope: Option<&str>,
    diagnostic: &'static str,
) -> Response {
    super::handler_pipeline_transaction_status(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        crate::NoritoStringQuery(PipelineStatusQuery {
            hash: Some(hash),
            scope: scope.map(str::to_owned),
        }),
    )
    .await
    .expect(diagnostic)
}
#[test]
#[ignore = "load profile; run explicitly with --ignored --nocapture"]
fn pipeline_status_cache_prune_load_profile() {
    const WARMUP_SAMPLES: usize = 4;
    const SAMPLES: usize = 32;
    const CACHE_ITEMS: u64 = 2_048;
    let fixtures = (1..=CACHE_ITEMS)
        .map(|height| {
            let (block, _) = make_signed_block(height, None);
            let header = block.header();
            let tx_hash = block.external_transactions().next().expect("tx").hash();
            (header.height(), header.hash(), tx_hash)
        })
        .collect::<Vec<_>>();
    let run_iteration = |sample_index: usize| {
        let cache = PipelineStatusCache::with_limits(512, Duration::from_secs(1));
        let now = Instant::now();
        let stale = now
            .checked_sub(Duration::from_secs(5))
            .expect("time subtraction");
        for (index, (height, block_hash, tx_hash)) in fixtures.iter().enumerate() {
            let observed_at = if index % 2 == 0 {
                stale
            } else {
                now + Duration::from_nanos((index + sample_index) as u64)
            };
            cache.record_entry_inner(
                *tx_hash,
                PipelineStatusEntry::at_time(
                    PipelineStatusKind::Queued,
                    Some(*height),
                    None,
                    observed_at,
                ),
            );
            cache.record_pending_block(
                *height,
                PendingBlockStatus {
                    kind: PipelineStatusKind::Committed,
                    block_hash: *block_hash,
                    observed_at,
                },
            );
        }
        let start = Instant::now();
        cache.prune(now);
        let elapsed = start.elapsed();
        std::hint::black_box((cache.entries.len(), cache.pending_blocks.len()));
        elapsed
    };
    for sample_index in 0..WARMUP_SAMPLES {
        std::hint::black_box(run_iteration(sample_index));
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    let wall_start = Instant::now();
    for sample_index in 0..SAMPLES {
        samples.push(run_iteration(sample_index + WARMUP_SAMPLES));
    }
    crate::profile_stats::print_profile(
        "hot_path",
        "pipeline_status_cache_prune_pressure",
        samples,
        WARMUP_SAMPLES,
        1,
        wall_start.elapsed(),
    );
}
#[test]
fn parse_signed_transaction_hash_rejects_invalid() {
    assert!(parse_signed_transaction_hash("not-a-hash").is_err());
    let canonical = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::new(
        b"pipeline-status-exact-hash",
    ))
    .to_string();
    assert!(parse_signed_transaction_hash(&canonical).is_ok());
    for retired in [
        canonical.to_ascii_uppercase(),
        format!(" {canonical}"),
        format!("{canonical} "),
        format!("{}0", &canonical[..63]),
    ] {
        assert!(
            parse_signed_transaction_hash(&retired).is_err(),
            "noncanonical hash spelling must fail: {retired:?}"
        );
    }

    let unknown_query = format!(r#"{{"hash":"{canonical}","legacy_scope":"auto"}}"#);
    assert!(
        norito::json::from_str::<PipelineStatusQuery>(&unknown_query).is_err(),
        "pipeline status query must reject unknown compatibility fields"
    );
}
#[tokio::test]
async fn pipeline_status_string_query_preserves_decimal_hash_and_whitespace() {
    use axum::extract::FromRequestParts as _;
    let hash = "11".repeat(32);
    let padded_hash = format!(" {hash} ");
    let request = axum::http::Request::builder()
        .uri(format!(
            "/v1/pipeline/transactions/status?hash=+{hash}+&scope=%20local%20"
        ))
        .body(())
        .expect("pipeline status request");
    let (mut parts, _) = request.into_parts();
    let crate::NoritoStringQuery(query) =
        crate::NoritoStringQuery::<PipelineStatusQuery>::from_request_parts(&mut parts, &())
            .await
            .expect("pipeline status string query should decode");
    assert_eq!(query.hash.as_deref(), Some(padded_hash.as_str()));
    assert_eq!(query.scope.as_deref(), Some(" local "));
}
#[tokio::test]
async fn pipeline_status_handler_returns_queued() {
    let app = mk_app_state_for_tests();
    let keypair =
        checked_torii_test_ed25519_keypair(0x28, "derive Torii queued-status fixture key");
    let authority = AccountId::new(keypair.public_key().clone());
    let tx = checked_torii_test_transaction(
        TransactionBuilder::new(
            *app.state.network_id_ref(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log {
            level: Level::INFO,
            msg: "queued".to_string(),
        }]),
        &keypair,
        "sign Torii queued-status fixture transaction",
    );
    let params = app.state.world.view().parameters().clone();
    let max_clock_drift = params.sumeragi().max_clock_drift();
    let tx_limits = params.transaction();
    let crypto_cfg = app.state.crypto();
    let accepted = AcceptedTransaction::accept(
        tx.clone(),
        app.state.network_id_ref(),
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )
    .expect("accepted");
    app.queue
        .push(accepted, app.state.view())
        .expect("queue push");
    let resp =
        pipeline_status_response(app.clone(), tx.hash().to_string(), Some("local"), "ok").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let payload = torii_json_body(resp).await;
    let status_kind = payload
        .get("status")
        .and_then(|status| status.get("kind"))
        .and_then(norito::json::Value::as_str);
    assert_eq!(status_kind, Some("Queued"));
    let resp_entry = pipeline_status_response(
        app.clone(),
        tx.hash_as_entrypoint().to_string(),
        Some("local"),
        "ok",
    )
    .await;
    assert_eq!(resp_entry.status(), StatusCode::OK);
    let payload_entry = torii_json_body(resp_entry).await;
    let status_kind_entry = payload_entry
        .get("status")
        .and_then(|status| status.get("kind"))
        .and_then(norito::json::Value::as_str);
    assert_eq!(status_kind_entry, Some("Queued"));
}
#[tokio::test]
async fn pipeline_status_handler_returns_typed_norito_when_requested() {
    use iroha_torii_shared::PipelineTransactionStatusResponse;
    let app = mk_app_state_for_tests();
    let keypair = checked_torii_test_ed25519_keypair(0x29, "derive Torii typed-status fixture key");
    let authority = AccountId::new(keypair.public_key().clone());
    let tx = checked_torii_test_transaction(
        TransactionBuilder::new(
            *app.state.network_id_ref(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log {
            level: Level::INFO,
            msg: "queued".to_string(),
        }]),
        &keypair,
        "sign Torii typed-status fixture transaction",
    );
    let params = app.state.world.view().parameters().clone();
    let max_clock_drift = params.sumeragi().max_clock_drift();
    let tx_limits = params.transaction();
    let crypto_cfg = app.state.crypto();
    let accepted = AcceptedTransaction::accept(
        tx.clone(),
        app.state.network_id_ref(),
        max_clock_drift,
        tx_limits,
        crypto_cfg.as_ref(),
    )
    .expect("accepted");
    app.queue
        .push(accepted, app.state.view())
        .expect("queue push");
    let resp = super::handler_pipeline_transaction_status(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
        )),
        crate::NoritoStringQuery(PipelineStatusQuery {
            hash: Some(tx.hash().to_string()),
            scope: Some("local".to_owned()),
        }),
    )
    .await
    .expect("ok");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some(crate::utils::NORITO_MIME_TYPE)
    );
    let bytes = torii_body_bytes(resp, "body").await;
    let payload: PipelineTransactionStatusResponse =
        norito::decode_from_bytes(&bytes).expect("typed norito response");
    assert_eq!(payload.status.kind, "Queued");
    assert_eq!(payload.resolved_from, "queue");
}
#[tokio::test]
async fn pipeline_preflight_handler_returns_json_snapshot() {
    let app = mk_app_state_for_tests();
    let expected_block_cadence_ms = app
        .state
        .world_view()
        .parameters()
        .sumeragi()
        .block_cadence_ms()
        .get();
    let resp = super::handler_pipeline_preflight(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
    )
    .await
    .expect("ok");
    assert_eq!(resp.status(), StatusCode::OK);
    let payload = torii_json_body(resp).await;
    assert_eq!(
        payload.get("schema_version"),
        Some(&norito::json::Value::from(1_u64))
    );
    assert_eq!(
        payload
            .get("sumeragi")
            .and_then(|value| value.get("block_cadence_ms")),
        Some(&norito::json::Value::from(expected_block_cadence_ms))
    );
    assert_eq!(
        payload.get("queue").and_then(|value| value.get("size")),
        Some(&norito::json::Value::from(0_u64))
    );
    let pipeline = payload
        .get("pipeline")
        .and_then(norito::json::Value::as_object)
        .expect("preflight payload should expose pipeline settings");
    assert!(
        pipeline.get("signature_batch_max").is_none(),
        "preflight payload must not expose the retired aggregate signature-batch field"
    );
    for field in [
        "signature_batch_max_ed25519",
        "signature_batch_max_secp256k1",
        "signature_batch_max_pqc",
        "signature_batch_max_bls",
    ] {
        assert!(
            pipeline.get(field).is_some(),
            "preflight payload must expose {field}"
        );
    }
    assert!(
        payload
            .get("fees")
            .and_then(norito::json::Value::as_object)
            .is_some(),
        "preflight payload should expose Nexus fee settings"
    );
}
#[tokio::test]
async fn pipeline_preflight_handler_returns_typed_norito_when_requested() {
    let app = mk_app_state_for_tests();
    let expected_block_cadence_ms = app
        .state
        .world_view()
        .parameters()
        .sumeragi()
        .block_cadence_ms()
        .get();
    let resp = super::handler_pipeline_preflight(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
        )),
    )
    .await
    .expect("ok");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some(crate::utils::NORITO_MIME_TYPE)
    );
    let bytes = torii_body_bytes(resp, "body").await;
    let payload: routing::PipelinePreflightResponse =
        norito::decode_from_bytes(&bytes).expect("typed norito response");
    assert_eq!(payload.schema_version, 1);
    assert_eq!(payload.queue.size, 0);
    assert_eq!(payload.sumeragi.block_cadence_ms, expected_block_cadence_ms);
    assert!(
        payload
            .fees
            .successful_claim_fee_exempt_authorities
            .is_empty()
    );
    assert_eq!(
        payload.pipeline.ivm_max_cycles_upper_bound,
        app.state
            .pipeline_snapshot()
            .ivm_max_cycles_upper_bound
            .get()
    );
    assert_eq!(
        payload.pipeline.ivm_admission_cycle_limit,
        app.state.ivm_admission_cycle_limit().get()
    );
}
#[test]
fn pipeline_status_global_read_skips_non_terminal_local_cache() {
    let app = mk_app_state_for_tests();
    let tx_hash =
        HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed([0x74; Hash::LENGTH]));
    app.pipeline_status_cache.record_entry(
        tx_hash,
        PipelineStatusEntry::fresh(PipelineStatusKind::Queued, None, None),
    );
    let err = execute_pipeline_status_local_read(
        &app,
        &PipelineStatusQuery {
            hash: Some(tx_hash.to_string()),
            scope: Some("global".to_owned()),
        },
        ResponseFormat::Json,
        None,
    )
    .expect_err("global reads must route/fan out before accepting local queued cache");
    assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
}
#[test]
fn pipeline_status_local_read_evicts_stale_queued_cache() {
    let app = mk_app_state_for_tests();
    let tx_hash =
        HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed([0x75; Hash::LENGTH]));
    app.pipeline_status_cache.record_entry(
        tx_hash,
        PipelineStatusEntry::fresh(PipelineStatusKind::Queued, None, None),
    );
    let err = execute_pipeline_status_local_read(
        &app,
        &PipelineStatusQuery {
            hash: Some(tx_hash.to_string()),
            scope: Some("local".to_owned()),
        },
        ResponseFormat::Json,
        None,
    )
    .expect_err("local reads must not expose stale queued cache entries");
    assert_eq!(err.into_response().status(), StatusCode::NOT_FOUND);
    assert!(app.pipeline_status_cache.lookup(&tx_hash).is_none());
}
#[tokio::test]
async fn pipeline_status_local_read_keeps_live_pending_queued_cache() {
    let mut app = mk_app_state_for_tests();
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .high_load_tx_threshold = usize::MAX;
    let keypair =
        checked_torii_test_ed25519_keypair(0xd8, "derive live pending pipeline-status fixture key");
    let authority = AccountId::new(keypair.public_key().clone());
    let transaction = signed_log_transaction_for_test(
        *app.state.network_id_ref(),
        authority,
        "pipeline-status-live-pending",
        &keypair,
    );
    let tx_hash = transaction.hash();
    let response = super::handler_post_transaction(
        State(app.clone()),
        HeaderMap::new(),
        None,
        versioned_signed_for_test(&transaction),
    )
    .await
    .expect("accepted")
    .into_response();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert!(
        app.queue
            .contains_pending_hash(transaction.hash_as_entrypoint(), &app.state),
        "fixture transaction should remain pending in the live queue"
    );
    app.pipeline_status_cache.record_entry(
        tx_hash.clone(),
        PipelineStatusEntry::fresh(PipelineStatusKind::Queued, None, None),
    );
    let response = execute_pipeline_status_local_read(
        &app,
        &PipelineStatusQuery {
            hash: Some(tx_hash.to_string()),
            scope: Some("local".to_owned()),
        },
        ResponseFormat::Json,
        None,
    )
    .expect("local reads should keep genuinely pending queued cache entries");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(app.pipeline_status_cache.lookup(&tx_hash).is_some());
}
#[test]
fn pipeline_status_local_read_keeps_approved_cache() {
    let app = mk_app_state_for_tests();
    let tx_hash =
        HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed([0x75; Hash::LENGTH]));
    app.pipeline_status_cache.record_entry(
        tx_hash,
        PipelineStatusEntry::fresh(PipelineStatusKind::Approved, None, None),
    );
    let response = execute_pipeline_status_local_read(
        &app,
        &PipelineStatusQuery {
            hash: Some(tx_hash.to_string()),
            scope: Some("local".to_owned()),
        },
        ResponseFormat::Json,
        None,
    )
    .expect("local reads should keep block-pipeline cache entries");
    assert_eq!(response.status(), StatusCode::OK);
}
#[tokio::test]
async fn pipeline_status_handler_uses_dedicated_rate_limiter() {
    let mut app = mk_app_state_for_tests();
    let tx_hash =
        HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed([0x71; Hash::LENGTH]));
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        app_mut.rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
        app_mut.pipeline_status_rate_limiter = limits::RateLimiter::new(Some(2), Some(2));
        app_mut.pipeline_status_cache.record_entry(
            tx_hash,
            PipelineStatusEntry::fresh(PipelineStatusKind::Queued, None, None),
        );
    }
    let headers = HeaderMap::new();
    let remote_ip = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    let rate_key = rate_limit_key(
        &headers,
        Some(remote_ip),
        "v1/pipeline/transactions/status",
        false,
    );
    assert!(limits::allow_conditionally(&app.rate_limiter, &rate_key, true).await);
    assert!(!limits::allow_conditionally(&app.rate_limiter, &rate_key, true).await);
    let resp = super::handler_pipeline_transaction_status(
        State(app),
        headers,
        crate::loopback_connect_info(),
        None,
        crate::NoritoStringQuery(PipelineStatusQuery {
            hash: Some(tx_hash.to_string()),
            scope: Some("local".to_owned()),
        }),
    )
    .await
    .expect("pipeline status should bypass the general query limiter");
    assert_eq!(resp.status(), StatusCode::OK);
}
#[tokio::test]
async fn pipeline_status_handler_cache_hit_ignores_tx_rate_limiter_pressure() {
    let mut app = mk_app_state_for_tests();
    let tx_hash =
        HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed([0x72; Hash::LENGTH]));
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        app_mut.tx_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
        app_mut.pipeline_status_cache.record_entry(
            tx_hash,
            PipelineStatusEntry::fresh(PipelineStatusKind::Applied, None, None),
        );
    }
    assert!(app.tx_rate_limiter.allow("pipeline-status-test").await);
    assert!(!app.tx_rate_limiter.allow("pipeline-status-test").await);
    let resp = pipeline_status_response(
        app,
        tx_hash.to_string(),
        Some("local"),
        "cached pipeline status should stay available under tx-ingress pressure",
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
}
#[tokio::test]
async fn pipeline_status_handler_charges_cache_hits_before_local_reads() {
    let mut app = mk_app_state_for_tests();
    let tx_hash =
        HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed([0x73; Hash::LENGTH]));
    {
        let app_mut = Arc::get_mut(&mut app).expect("unique app state");
        app_mut.pipeline_status_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
        app_mut.pipeline_status_cache.record_entry(
            tx_hash,
            PipelineStatusEntry::fresh(PipelineStatusKind::Applied, None, None),
        );
    }
    let headers = HeaderMap::new();
    let remote_ip = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    let rate_key = rate_limit_key(
        &headers,
        Some(remote_ip),
        "v1/pipeline/transactions/status",
        false,
    );
    assert!(limits::allow_conditionally(&app.pipeline_status_rate_limiter, &rate_key, true).await);
    assert!(!limits::allow_conditionally(&app.pipeline_status_rate_limiter, &rate_key, true).await);
    let error = super::handler_pipeline_transaction_status(
        State(app),
        headers,
        crate::loopback_connect_info(),
        None,
        crate::NoritoStringQuery(PipelineStatusQuery {
            hash: Some(tx_hash.to_string()),
            scope: Some("local".to_owned()),
        }),
    )
    .await
    .expect_err("cached pipeline status must not bypass the pipeline-status limiter");
    assert_eq!(
        error.into_response().status(),
        StatusCode::TOO_MANY_REQUESTS
    );
}
#[test]
fn pipeline_fastpq_recovery_page_enforces_explicit_bounds() {
    assert_eq!(
        PipelineFastpqRecoveryPage::parse(&PipelineFastpqRecoveryQuery::default())
            .expect("default FASTPQ page"),
        PipelineFastpqRecoveryPage {
            offset: 0,
            limit: PIPELINE_FASTPQ_RECOVERY_DEFAULT_LIMIT,
        }
    );
    for limit in [0, PIPELINE_FASTPQ_RECOVERY_MAX_LIMIT as u64 + 1] {
        let error = PipelineFastpqRecoveryPage::parse(&PipelineFastpqRecoveryQuery {
            offset: None,
            limit: Some(limit),
        })
        .expect_err("out-of-range FASTPQ page limit must fail closed");
        assert!(query_conversion_message(&error).is_some());
    }
}
#[test]
fn pipeline_fastpq_recovery_artifact_budget_is_cumulative() {
    let mut used = 0;
    charge_fastpq_recovery_artifact_bytes(&mut used, PIPELINE_FASTPQ_RECOVERY_MAX_ARTIFACT_BYTES)
        .expect("exact FASTPQ artifact budget is admissible");
    let error = charge_fastpq_recovery_artifact_bytes(&mut used, 1)
        .expect_err("FASTPQ artifact budget overflow must fail closed");
    assert!(matches!(
        error,
        Error::AppServiceUnavailable {
            code: "pipeline_recovery_fastpq_artifact_too_large",
            ..
        }
    ));
}
#[test]
fn pipeline_fastpq_recovery_builder_paginates_and_bounds_encoding() {
    let app = mk_app_state_for_tests();
    let height = 91;
    let block_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x91; Hash::LENGTH]));
    let mut sidecar = iroha_core::kura::PipelineRecoverySidecar::new(
        height,
        block_hash,
        iroha_core::kura::PipelineDagSnapshot {
            fingerprint: [0; 32],
            key_count: 0,
        },
        Vec::new(),
    );
    for batch_index in 0..2 {
        let proof = vec![batch_index as u8 + 1];
        sidecar
            .fastpq_proofs
            .push(iroha_core::kura::FastpqProofSnapshot {
                height,
                block_hash,
                entry_hash: Hash::prehashed([batch_index as u8 + 1; Hash::LENGTH]),
                batch_index,
                parameter: "fastpq-state-transition-stark-v1".to_owned(),
                transition_count: 0,
                trace_commitment: iroha_data_model::privacy::GoldilocksDigest384V1::new(
                    [u64::from(batch_index) + 1; 6],
                )
                .expect("canonical test FASTPQ trace commitment"),
                proof_digest: Hash::new(&proof),
                batch: fastpq_prover::TransitionBatch::new(
                    "fastpq-state-transition-stark-v1",
                    fastpq_prover::PublicInputs::default(),
                ),
                proof,
            });
    }
    app.kura.write_pipeline_metadata(&sidecar);
    let serialized = build_pipeline_recovery_fastpq_response(
        &app.kura,
        height,
        PipelineFastpqRecoveryPage {
            offset: 0,
            limit: 1,
        },
    )
    .expect("bounded FASTPQ recovery page");
    assert!(serialized.len() <= PIPELINE_FASTPQ_RECOVERY_MAX_RESPONSE_BYTES);
    let value: norito::json::Value =
        norito::json::from_str(&serialized).expect("decode FASTPQ recovery page");
    assert_eq!(value.get("total_proofs").and_then(|v| v.as_u64()), Some(2));
    assert_eq!(value.get("next_offset").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(
        value.get("proofs").and_then(|v| v.as_array()).map(Vec::len),
        Some(1)
    );
    let batch = fastpq_prover::TransitionBatch::new(
        "fastpq-state-transition-stark-v1",
        fastpq_prover::PublicInputs::default(),
    );
    let mut artifact_bytes = 0;
    let (encoded, reconstructed) = encode_fastpq_recovery_batch(&batch, false, &mut artifact_bytes)
        .expect("bounded FASTPQ batch encoding");
    assert!(!encoded.is_empty());
    assert!(!reconstructed);
    assert!(artifact_bytes > 0);
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn account_get_handler_supports_json_and_norito() {
    use iroha_torii_shared::AccountReadResponse;
    let keypair = checked_torii_test_ed25519_keypair(0x2a, "derive Torii account-get fixture key");
    let account_id = AccountId::new(keypair.public_key().clone());
    let world = world_with_account(&account_id);
    let app = mk_app_state_for_tests_with_world(world);
    let json_resp = super::handler_account_get(
        State(app.clone()),
        axum::http::Method::GET,
        format!("/v1/accounts/{account_id}")
            .parse()
            .expect("valid account get uri"),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        AxPath(account_id.to_string()),
    )
    .await
    .expect("json account get");
    assert_eq!(json_resp.status(), StatusCode::OK);
    assert_eq!(
        torii_response_header(&json_resp, "content-type"),
        Some("application/json")
    );
    let json_bytes = torii_body_bytes(json_resp, "json body").await;
    let json_value: norito::json::Value =
        norito::json::from_slice(&json_bytes).expect("json account payload value");
    let json_payload: AccountReadResponse =
        norito::json::from_slice(&json_bytes).expect("json account payload");
    assert_eq!(json_payload.account_id, account_id);
    assert!(
        json_value.get("linked_domains").is_none(),
        "account read payload should not expose linked_domains"
    );
    let norito_resp = super::handler_account_get(
        State(app),
        axum::http::Method::GET,
        format!("/v1/accounts/{account_id}")
            .parse()
            .expect("valid account get uri"),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
        )),
        AxPath(account_id.to_string()),
    )
    .await
    .expect("norito account get");
    assert_eq!(norito_resp.status(), StatusCode::OK);
    assert_eq!(
        torii_response_header(&norito_resp, "content-type"),
        Some(crate::utils::NORITO_MIME_TYPE)
    );
    let norito_bytes = torii_body_bytes(norito_resp, "norito body").await;
    let norito_payload: AccountReadResponse =
        norito::decode_from_bytes(&norito_bytes).expect("norito account payload");
    assert_eq!(norito_payload.account_id, account_id);
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn account_get_handler_returns_not_found_for_missing_account() {
    let app = mk_app_state_for_tests();
    let missing =
        checked_torii_test_account_id(0x2b, "derive Torii missing account-get fixture key");
    let resp = super::handler_account_get(
        State(app),
        axum::http::Method::GET,
        format!("/v1/accounts/{missing}")
            .parse()
            .expect("valid missing account uri"),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        AxPath(missing.to_string()),
    )
    .await
    .expect("missing account response");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn account_read_for_routes_skips_route_unavailable_until_success() {
    let keypair =
        checked_torii_test_ed25519_keypair(0x2c, "derive Torii routed account-read fixture key");
    let account_id = AccountId::new(keypair.public_key().clone());
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&account_id));
    let (local_route, foreign_route) =
        configure_private_ingress_with_offline_foreign_route_for_test(&mut app);
    let response = super::execute_torii_account_read_for_routes(
        &app,
        vec![foreign_route, local_route],
        account_id.to_string(),
        ResponseFormat::Json,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        torii_response_header(&response, "x-iroha-routed-by"),
        Some("proxy"),
        "mixed local/proxy fanout should report proxy routing",
    );
    let body = torii_body_bytes(response, "account read body").await;
    let payload: AccountReadResponse =
        norito::json::from_slice(&body).expect("account read payload");
    assert_eq!(payload.account_id, account_id);
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn trusted_internal_account_handler_rejects_credentials_from_untrusted_sources() {
    let authority = checked_torii_test_account_id(
        0x2a,
        "derive trusted internal account authorization fixture key",
    );
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain id"),
        "rose".parse().expect("asset name"),
    );
    let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-api-token"),
        HeaderValue::from_static("valid-looking-api-token"),
    );
    headers.insert(
        axum::http::header::AUTHORIZATION,
        HeaderValue::from_static("Bearer valid-looking-jwt"),
    );
    let external = axum::extract::ConnectInfo(std::net::SocketAddr::from(([203, 0, 113, 8], 443)));
    let response = super::handler_internal_account_get(
        State(app.clone()),
        headers.clone(),
        external,
        None,
        AxPath(authority.to_string()),
    )
    .await
    .expect("untrusted request should return a typed rejection");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        torii_response_header(&response, "x-iroha-reject-code"),
        Some("trusted_network_required")
    );
    headers.insert(
        HeaderName::from_static(crate::limits::REMOTE_ADDR_HEADER),
        HeaderValue::from_static("127.0.0.1"),
    );
    assert!(
        !super::trusted_internal_read_source(app.as_ref(), &headers, external.0.ip()),
        "an untrusted peer must not forge the ingress-owned effective-address header",
    );
    let forged_asset_response = super::handler_internal_account_asset_get(
        State(app.clone()),
        format!(
            "/v1/internal/accounts/{authority}/assets/{asset_definition_id}?scope=dataspace:10"
        )
        .parse()
        .expect("valid internal asset URI"),
        headers.clone(),
        external,
        None,
        AxPath((authority.to_string(), asset_definition_id.to_string())),
    )
    .await
    .expect("forged external asset request should return a typed rejection");
    assert_eq!(forged_asset_response.status(), StatusCode::FORBIDDEN);
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .trusted_proxy_nets = Arc::new(crate::limits::parse_cidrs(&["127.0.0.0/8".to_owned()]));
    let mut nginx_headers = HeaderMap::new();
    nginx_headers.insert(
        HeaderName::from_static(crate::limits::REMOTE_ADDR_HEADER),
        HeaderValue::from_static("198.51.100.9"),
    );
    assert!(
        !super::trusted_internal_read_source(
            app.as_ref(),
            &nginx_headers,
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        ),
        "a configured loopback proxy must not make its external client a trusted loopback caller",
    );
    let proxied_response = super::handler_internal_account_get(
        State(app.clone()),
        nginx_headers,
        crate::loopback_connect_info(),
        None,
        AxPath(authority.to_string()),
    )
    .await
    .expect("external caller behind nginx should return a typed rejection");
    assert_eq!(proxied_response.status(), StatusCode::FORBIDDEN);
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .api_rate_limit_bypass_nets =
        Arc::new(crate::limits::parse_cidrs(&["203.0.113.0/24".to_owned()]));
    assert!(crate::limits::is_allowed_by_cidr(
        &HeaderMap::new(),
        Some(external.0.ip()),
        &app.api_rate_limit_bypass_nets,
    ));
    assert!(
        !super::trusted_internal_read_source(app.as_ref(), &HeaderMap::new(), external.0.ip()),
        "a broad rate-limit bypass must not confer internal-read trust",
    );
    assert!(super::trusted_internal_read_source(
        app.as_ref(),
        &HeaderMap::new(),
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
    ));
    Arc::get_mut(&mut app)
        .expect("unique app state")
        .internal_api_trusted_nets =
        Arc::new(crate::limits::parse_cidrs(&["203.0.113.8/32".to_owned()]));
    assert!(super::trusted_internal_read_source(
        app.as_ref(),
        &HeaderMap::new(),
        external.0.ip(),
    ));
    assert!(!super::trusted_internal_read_source(
        app.as_ref(),
        &HeaderMap::new(),
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 9)),
    ));
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn trusted_internal_account_handler_emits_exact_json_and_norito_projection() {
    let authority = checked_torii_test_account_id(
        0x2b,
        "derive trusted internal account projection fixture key",
    );
    let uaid = UniversalAccountId::from_hash(Hash::new(b"trusted-internal-account"));
    let mut metadata = iroha_data_model::metadata::Metadata::default();
    metadata.insert(
        "tier".parse().expect("metadata key"),
        Json::new("regulated"),
    );
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id).build(&authority);
    let account = Account::new(authority.clone())
        .with_metadata(metadata.clone())
        .with_uaid(Some(uaid))
        .build(&authority);
    let app = mk_app_state_for_tests_with_world(World::with([domain], [account], []));
    let json_response = super::handler_internal_account_get(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static("application/json"),
        )),
        AxPath(authority.to_string()),
    )
    .await
    .expect("loopback JSON account read");
    assert_eq!(json_response.status(), StatusCode::OK);
    let json_body = torii_body_bytes(json_response, "JSON account body").await;
    let json: Value = norito::json::from_slice(&json_body).expect("valid account JSON");
    let object = json.as_object().expect("account JSON object");
    assert_eq!(
        object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from(["id", "metadata", "opaque_ids", "uaid"]),
        "the trusted projection must not expose account aliases or the legacy label field",
    );
    let decoded_json: super::InternalAccountReadResponse =
        norito::json::from_slice(&json_body).expect("decode typed account JSON");
    assert_eq!(decoded_json.id, authority);
    assert_eq!(decoded_json.metadata, metadata);
    assert_eq!(decoded_json.uaid, Some(uaid));
    assert!(decoded_json.opaque_ids.is_empty());
    let norito_response = super::handler_internal_account_get(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
        )),
        AxPath(authority.to_string()),
    )
    .await
    .expect("loopback Norito account read");
    assert_eq!(norito_response.status(), StatusCode::OK);
    assert_eq!(
        norito_response
            .headers()
            .get(axum::http::header::CONTENT_TYPE),
        Some(&HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE))
    );
    let norito_body = torii_body_bytes(norito_response, "Norito account body").await;
    let decoded_norito: super::InternalAccountReadResponse =
        norito::decode_from_bytes(&norito_body).expect("decode typed account Norito");
    assert_eq!(decoded_norito, decoded_json);
}
#[cfg(feature = "app_api")]
#[test]
fn trusted_internal_path_literals_must_be_exactly_canonical() {
    let authority = checked_torii_test_account_id(
        0x2c,
        "derive trusted internal canonical literal fixture key",
    );
    let account_literal = authority.to_string();
    assert!(super::parse_exact_account_id_literal(&account_literal).is_ok());
    assert!(
        super::parse_exact_account_id_literal(&format!(" {account_literal}")).is_err(),
        "whitespace normalization is forbidden",
    );
    let entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"trusted-internal-canonical-hash",
    ));
    let hash_literal = entrypoint_hash.to_string();
    assert!(super::parse_exact_entrypoint_hash_literal(&hash_literal).is_ok());
    assert!(
        super::parse_exact_entrypoint_hash_literal(&hash_literal.to_ascii_uppercase()).is_err(),
        "uppercase hash aliases are forbidden",
    );
    assert!(
        super::parse_exact_entrypoint_hash_literal(&format!("{hash_literal} ")).is_err(),
        "hash whitespace normalization is forbidden",
    );
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").expect("domain id"),
        "rose".parse().expect("asset name"),
    );
    let definition_literal = asset_definition_id.to_string();
    assert!(super::parse_exact_asset_definition_id_literal(&definition_literal).is_ok());
    assert!(
        super::parse_exact_asset_definition_id_literal(&format!("{definition_literal} ")).is_err(),
        "asset-definition whitespace normalization is forbidden",
    );
    for scope in ["global", "dataspace:0", "dataspace:10"] {
        assert!(super::parse_exact_asset_balance_scope_literal(scope).is_ok());
        assert!(
            super::parse_exact_internal_asset_scope_query(Some(&format!("scope={scope}"))).is_ok()
        );
    }
    for scope in ["Global", "dataspace:010", "dataspace:+10", "dataspace:10 "] {
        assert!(
            super::parse_exact_asset_balance_scope_literal(scope).is_err(),
            "noncanonical asset scope `{scope}` must fail",
        );
    }
    for query in [
        None,
        Some(""),
        Some("asset=dataspace:10"),
        Some("scope=dataspace:10&scope=dataspace:10"),
        Some("scope=dataspace:10&extra=1"),
    ] {
        assert!(
            super::parse_exact_internal_asset_scope_query(query).is_err(),
            "missing, duplicate, or alternate asset-scope query keys must fail",
        );
    }
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn trusted_internal_transaction_read_requires_exact_hash_and_account_involvement() {
    let keypair = checked_torii_test_ed25519_keypair(
        0x24,
        "derive trusted internal transaction authority fixture key",
    );
    let authority = AccountId::new(keypair.public_key().clone());
    let unrelated = checked_torii_test_account_id(
        0x2d,
        "derive trusted internal unrelated account fixture key",
    );
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id).build(&authority);
    let app = mk_app_state_for_tests_with_world(World::with(
        [domain],
        [
            Account::new(authority.clone()).build(&authority),
            Account::new(unrelated.clone()).build(&authority),
        ],
        [],
    ));
    let (block, entrypoint_hash) = make_signed_block(1, None);
    let header = block.header();
    let block_hash = store_block(&app, block);
    record_committed_block_hash_for_test(&app, header, block_hash);
    let expected = crate::routing::committed_transactions_snapshot(app.state.as_ref())
        .expect("committed transaction snapshot")
        .into_iter()
        .find(|transaction| transaction.entrypoint_hash() == &entrypoint_hash)
        .expect("stored transaction");
    let response = super::execute_trusted_internal_account_transaction_local_read(
        &app,
        &authority.to_string(),
        &entrypoint_hash.to_string(),
        ResponseFormat::Json,
    );
    assert_eq!(response.status(), StatusCode::OK);
    let body = torii_body_bytes(response, "committed transaction JSON body").await;
    let decoded: iroha_data_model::query::CommittedTransaction =
        norito::json::from_slice(&body).expect("full committed transaction JSON");
    assert_eq!(
        decoded, expected,
        "the response must preserve every proof field"
    );
    let norito_response = super::execute_trusted_internal_account_transaction_local_read(
        &app,
        &authority.to_string(),
        &entrypoint_hash.to_string(),
        ResponseFormat::Norito,
    );
    assert_eq!(norito_response.status(), StatusCode::OK);
    let norito_body = torii_body_bytes(norito_response, "committed transaction Norito body").await;
    let decoded_norito: iroha_data_model::query::CommittedTransaction =
        norito::decode_from_bytes(&norito_body)
            .expect("full committed transaction Norito response");
    assert_eq!(decoded_norito, expected);
    let non_involved = super::execute_trusted_internal_account_transaction_local_read(
        &app,
        &unrelated.to_string(),
        &entrypoint_hash.to_string(),
        ResponseFormat::Json,
    );
    assert_eq!(non_involved.status(), StatusCode::NOT_FOUND);
    let missing_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"trusted-internal-missing-hash",
    ));
    let missing = super::execute_trusted_internal_account_transaction_local_read(
        &app,
        &authority.to_string(),
        &missing_hash.to_string(),
        ResponseFormat::Json,
    );
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn trusted_internal_asset_read_is_exactly_scoped_bound_and_conflict_safe() {
    let authority =
        checked_torii_test_account_id(0x2e, "derive trusted internal asset authority fixture key");
    let unrelated = checked_torii_test_account_id(
        0x2f,
        "derive trusted internal asset unrelated-account fixture key",
    );
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let asset_definition_id =
        AssetDefinitionId::derive_from_components(domain_id, "rose".parse().expect("asset name"));
    let asset_definition = AssetDefinition::numeric(
        asset_definition_id.clone(),
        "rose".to_owned(),
        iroha_data_model::asset::AssetBalancePolicy::Global,
        None,
    )
    .build(&authority);
    let asset_id = AssetId::with_scope(
        asset_definition_id.clone(),
        authority.clone(),
        AssetBalanceScope::Dataspace(DataSpaceId::new(10)),
    );
    let expected = Asset::new(asset_id.clone(), Quantity::from(42_u32));
    let app = mk_app_state_for_tests_with_world(World::with_assets(
        [domain],
        [
            Account::new(authority.clone()).build(&authority),
            Account::new(unrelated.clone()).build(&authority),
        ],
        [asset_definition],
        [expected.clone()],
        [],
    ));
    let json_response = super::handler_internal_account_asset_get(
        State(app.clone()),
        format!(
            "/v1/internal/accounts/{authority}/assets/{asset_definition_id}?scope=dataspace:10"
        )
        .parse()
        .expect("valid internal asset URI"),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static("application/json"),
        )),
        AxPath((authority.to_string(), asset_definition_id.to_string())),
    )
    .await
    .expect("exact JSON asset read");
    assert_eq!(json_response.status(), StatusCode::OK);
    let json_body = torii_body_bytes(json_response, "asset JSON body").await;
    let json_value: Value = norito::json::from_slice(&json_body).expect("valid asset JSON");
    assert_eq!(
        json_value
            .as_object()
            .expect("asset JSON object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["id", "value"]),
    );
    let decoded_json: Asset = norito::json::from_slice(&json_body).expect("typed asset JSON");
    assert_eq!(decoded_json, expected);
    let norito_response = super::handler_internal_account_asset_get(
        State(app.clone()),
        format!(
            "/v1/internal/accounts/{authority}/assets/{asset_definition_id}?scope=dataspace:10"
        )
        .parse()
        .expect("valid internal asset URI"),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
        )),
        AxPath((authority.to_string(), asset_definition_id.to_string())),
    )
    .await
    .expect("exact Norito asset read");
    assert_eq!(norito_response.status(), StatusCode::OK);
    let norito_body = torii_body_bytes(norito_response, "asset Norito body").await;
    let decoded_norito: Asset =
        norito::decode_from_bytes(&norito_body).expect("typed asset Norito");
    assert_eq!(decoded_norito, expected);
    let missing_json_response = super::handler_internal_account_asset_get(
        State(app.clone()),
        format!("/v1/internal/accounts/{authority}/assets/{asset_definition_id}?scope=global")
            .parse()
            .expect("valid missing internal asset URI"),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static("application/json"),
        )),
        AxPath((authority.to_string(), asset_definition_id.to_string())),
    )
    .await
    .expect("missing JSON asset read");
    assert_eq!(missing_json_response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        missing_json_response
            .headers()
            .get(axum::http::header::CONTENT_TYPE),
        Some(&HeaderValue::from_static("application/json")),
    );
    assert_eq!(
        torii_response_header(&missing_json_response, "x-iroha-reject-code"),
        Some("not_found"),
    );
    let missing_json_body =
        torii_body_bytes(missing_json_response, "missing asset JSON body").await;
    let missing_envelope: ErrorEnvelope =
        norito::json::from_slice(&missing_json_body).expect("typed missing asset JSON");
    assert_eq!(missing_envelope.code, "not_found");
    for (account, scope) in [
        (authority.clone(), "global"),
        (authority.clone(), "dataspace:11"),
        (unrelated, "dataspace:10"),
    ] {
        let response = super::execute_trusted_internal_account_asset_local_read(
            &app,
            &account.to_string(),
            &asset_definition_id.to_string(),
            scope,
            ResponseFormat::Json,
        );
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "wrong scope or account binding must not resolve the asset",
        );
    }
    let conflicting = Asset::new(asset_id, Quantity::from(43_u32));
    let mut budget = super::torii_local_routed_read_budget(&app).expect("local routed-read budget");
    let expected =
        super::torii_bounded_routed_read_source_payload::<Asset, _>(&expected, &mut budget)
            .expect("bound expected asset payload");
    let conflicting =
        super::torii_bounded_routed_read_source_payload::<Asset, _>(&conflicting, &mut budget)
            .expect("bound conflicting asset payload");
    let conflict = match super::merged_trusted_internal_read_response(
        vec![expected, conflicting],
        ResponseFormat::Json,
        "local",
        budget,
    ) {
        Ok(_) => panic!("conflicting route payloads must fail closed"),
        Err(response) => response,
    };
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(
        torii_response_header(&conflict, "x-iroha-reject-code"),
        Some("route_conflict"),
    );
    assert_eq!(
        conflict.headers().get(axum::http::header::CONTENT_TYPE),
        Some(&HeaderValue::from_static("application/json")),
    );
    let conflict_body = torii_body_bytes(conflict, "route-conflict JSON body").await;
    let conflict_envelope: ErrorEnvelope =
        norito::json::from_slice(&conflict_body).expect("typed route-conflict JSON");
    assert_eq!(conflict_envelope.code, "route_conflict");
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn account_read_for_routes_prefers_not_found_over_route_unavailable_when_missing() {
    let missing =
        checked_torii_test_account_id(0x2d, "derive Torii missing routed account-read fixture key");
    let mut app = mk_app_state_for_tests();
    let (local_route, foreign_route) =
            crate::tests_runtime_handlers::configure_private_ingress_with_offline_foreign_route_for_test(&mut app);
    let response = super::execute_torii_account_read_for_routes(
        &app,
        vec![foreign_route, local_route],
        missing.to_string(),
        ResponseFormat::Json,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_ne!(
        torii_response_header(&response, "x-iroha-reject-code"),
        Some("route_unavailable"),
        "a definitive missing-account response should outrank an unrelated unavailable route",
    );
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn account_read_for_routes_returns_route_unavailable_when_only_unavailable() {
    let missing = checked_torii_test_account_id(
        0x2e,
        "derive Torii unavailable routed account-read fixture key",
    );
    let mut app = mk_app_state_for_tests();
    let (_local_route, foreign_route) =
            crate::tests_runtime_handlers::configure_private_ingress_with_offline_foreign_route_for_test(&mut app);
    let response = super::execute_torii_account_read_for_routes(
        &app,
        vec![foreign_route],
        missing.to_string(),
        ResponseFormat::Json,
    )
    .await;
    assert_route_unavailable_response(&response);
}
#[test]
fn trigger_completion_query_falls_back_to_reconstructed_entrypoint_hash() {
    let app = mk_app_state_for_tests();
    let sample = make_persisted_data_trigger_completion_block(1, None);
    let header = sample.block.header();
    let block_hash = store_block(&app, sample.block);
    record_committed_block_hash_for_test(&app, header, block_hash);
    let response = super::trigger_completion_query_response(
        &app,
        &TriggerCompletionQuery {
            id: None,
            entrypoint_hash: Some(sample.entrypoint_hash.to_string()),
            outcome: None,
            from_height: Some(1),
            to_height: Some(1),
            limit: Some(10),
            scan_limit_blocks: Some(1),
            include_reconstructed: Some(true),
        },
    )
    .expect("query response");
    assert_eq!(response.completions.len(), 1);
    let record = response.completions.first().expect("completion");
    assert_eq!(record.source, "reconstructed_result");
    assert_eq!(record.block_height, 1);
    assert_eq!(record.entrypoint_index, Some(0));
    assert_eq!(record.completion.trigger_id, sample.trigger_id.to_string());
    assert_eq!(
        record.completion.trigger_execution_hash,
        sample.entrypoint_hash.to_string()
    );
    let without_reconstruction = super::trigger_completion_query_response(
        &app,
        &TriggerCompletionQuery {
            id: None,
            entrypoint_hash: Some(sample.entrypoint_hash.to_string()),
            outcome: None,
            from_height: Some(1),
            to_height: Some(1),
            limit: Some(10),
            scan_limit_blocks: Some(1),
            include_reconstructed: Some(false),
        },
    )
    .expect("query response");
    assert!(without_reconstruction.completions.is_empty());
    let persisted_response = super::trigger_completion_query_response(
        &app,
        &TriggerCompletionQuery {
            id: None,
            entrypoint_hash: Some(sample.trigger_execution_hash.to_string()),
            outcome: None,
            from_height: Some(1),
            to_height: Some(1),
            limit: Some(10),
            scan_limit_blocks: Some(1),
            include_reconstructed: Some(true),
        },
    )
    .expect("query response");
    assert_eq!(persisted_response.completions.len(), 1);
    let persisted = persisted_response.completions.first().expect("completion");
    assert_eq!(persisted.source, "block_result");
    assert_eq!(
        persisted.completion.trigger_execution_hash,
        sample.trigger_execution_hash.to_string()
    );
}
#[test]
fn trigger_completion_record_visit_stops_without_buffering_the_block() {
    let mut sample = make_persisted_data_trigger_completion_block(1, None);
    sample.block.set_trigger_completions(vec![
        TriggerCompletedEvent::new(
            sample.trigger_id.clone(),
            sample.trigger_execution_hash,
            0,
            TriggerCompletedOutcome::Success,
        ),
        TriggerCompletedEvent::new(
            sample.trigger_id.clone(),
            sample.trigger_execution_hash,
            1,
            TriggerCompletedOutcome::Success,
        ),
    ]);
    let mut visited = 0_u8;
    let completed =
        super::visit_trigger_completion_records_for_block(&sample.block, 1, false, None, |_| {
            visited = visited.saturating_add(1);
            false
        });
    assert!(!completed);
    assert_eq!(
        visited, 1,
        "the visitor must stop before building later records"
    );
}
#[test]
fn trigger_completion_query_caps_explicit_from_height() {
    let app = mk_app_state_for_tests();
    let sample = make_persisted_data_trigger_completion_block(1, None);
    let header = sample.block.header();
    let block_hash = store_block(&app, sample.block);
    record_committed_block_hash_for_test(&app, header, block_hash);
    let mut prev_hash = Some(block_hash);
    for height in 2..=4 {
        let mut block = make_empty_signed_block(height, prev_hash, 0);
        block
            .set_transaction_results(Vec::new(), &[], Vec::new())
            .expect("empty test block should accept empty results");
        let header = block.header();
        let hash = store_block(&app, block);
        record_committed_block_hash_for_test(&app, header, hash);
        prev_hash = Some(hash);
    }
    let bounded_window = super::trigger_completion_query_response(
        &app,
        &TriggerCompletionQuery {
            id: Some(sample.trigger_id.to_string()),
            entrypoint_hash: None,
            outcome: None,
            from_height: None,
            to_height: Some(4),
            limit: Some(10),
            scan_limit_blocks: Some(2),
            include_reconstructed: Some(true),
        },
    )
    .expect("query response");
    assert_eq!(bounded_window.from_height, 3);
    assert_eq!(bounded_window.scanned_blocks, 2);
    assert!(bounded_window.completions.is_empty());
    let explicit_history = super::trigger_completion_query_response(
        &app,
        &TriggerCompletionQuery {
            id: Some(sample.trigger_id.to_string()),
            entrypoint_hash: None,
            outcome: None,
            from_height: Some(1),
            to_height: Some(4),
            limit: Some(10),
            scan_limit_blocks: Some(2),
            include_reconstructed: Some(true),
        },
    )
    .expect("query response");
    assert_eq!(explicit_history.from_height, 3);
    assert_eq!(explicit_history.scanned_blocks, 2);
    assert!(explicit_history.completions.is_empty());
}
#[tokio::test]
async fn pipeline_status_handler_returns_applied_from_state() {
    let app = mk_app_state_for_tests();
    let (block, _) = make_signed_block(1, None);
    let header = block.header();
    let tx = block.external_transactions().next().expect("tx");
    let tx_hash = tx.hash();
    let tx_entry_hash = tx.hash_as_entrypoint();
    store_block(&app, block);
    let height = header.height();
    let height_usize = usize::try_from(height.get()).expect("height usize");
    let height_nz = NonZeroUsize::new(height_usize).expect("height");
    let mut state_block = app.state.block(header);
    let tx_hashes: HashSet<_> = [tx_entry_hash].into_iter().collect();
    state_block.transactions.insert_block(tx_hashes, height_nz);
    state_block.commit().expect("commit");
    let resp = pipeline_status_response(app.clone(), tx_hash.to_string(), None, "ok").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let payload = torii_json_body(resp).await;
    let status_kind = payload
        .get("status")
        .and_then(|status| status.get("kind"))
        .and_then(norito::json::Value::as_str);
    assert_eq!(status_kind, Some("Applied"));
    let resp_entry =
        pipeline_status_response(app.clone(), tx_entry_hash.to_string(), None, "ok").await;
    assert_eq!(resp_entry.status(), StatusCode::OK);
    let payload_entry = torii_json_body(resp_entry).await;
    let status_kind_entry = payload_entry
        .get("status")
        .and_then(|status| status.get("kind"))
        .and_then(norito::json::Value::as_str);
    assert_eq!(status_kind_entry, Some("Applied"));
}
#[tokio::test]
async fn pipeline_status_handler_rejects_inconsistent_committed_membership() {
    let app = mk_app_state_for_tests();
    let (block, _) = make_signed_block(1, None);
    let header = block.header();
    store_block(&app, block);
    let bogus_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed(
        [0x76; Hash::LENGTH],
    ));
    let height = NonZeroUsize::new(
        usize::try_from(header.height().get()).expect("committed height fits usize"),
    )
    .expect("committed height is non-zero");
    let mut state_block = app.state.block(header);
    state_block
        .transactions
        .insert_block([bogus_hash].into_iter().collect(), height);
    state_block.commit().expect("commit inconsistent fixture");
    let result = super::handler_pipeline_transaction_status(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static("application/json"),
        )),
        crate::NoritoStringQuery(PipelineStatusQuery {
            hash: Some(bogus_hash.to_string()),
            scope: Some("local".to_owned()),
        }),
    )
    .await;
    let Err(super::Error::Query(ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
    ))) = result
    else {
        panic!("inconsistent committed membership must fail closed");
    };
    assert!(message.contains("absent from its external body and has no merge reference"));
}
#[tokio::test]
async fn public_pipeline_status_never_hydrates_trigger_completion_details() {
    let app = mk_app_state_for_tests();
    let sample = make_persisted_data_trigger_completion_block(1, None);
    let header = sample.block.header();
    let height = header.height();
    let height_usize = usize::try_from(height.get()).expect("height usize");
    let height_nz = NonZeroUsize::new(height_usize).expect("height");
    let block_hash = store_block(&app, sample.block);
    record_committed_block_hash_for_test(&app, header.clone(), block_hash);
    let mut state_block = app.state.block(header);
    let tx_hashes: HashSet<_> = [sample.entrypoint_hash].into_iter().collect();
    state_block.transactions.insert_block(tx_hashes, height_nz);
    state_block.commit().expect("commit");
    let resp = pipeline_status_response(app.clone(), sample.tx_hash.to_string(), None, "ok").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let payload = torii_json_body(resp).await;
    assert_eq!(
        payload
            .get("status")
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str),
        Some("Applied")
    );
    assert!(payload.get("trigger_completions").is_none());
    assert!(payload.get("batch_transfer_outcomes").is_none());
    let encoded = norito::json::to_json(&payload).expect("public status JSON");
    assert!(!encoded.contains(&sample.trigger_id.to_string()));
}
fn transaction_details_test_world(accounts: &[AccountId]) -> World {
    let owner = accounts.first().expect("at least one test account");
    let domain = Domain::new(
        DomainId::try_new("wonderland", "universal").expect("transaction-details test domain"),
    )
    .build(owner);
    let accounts = accounts
        .iter()
        .cloned()
        .map(|account_id| Account::new(account_id).build(owner));
    World::with([domain], accounts, [])
}
fn store_and_index_transaction_details_block(
    app: &SharedAppState,
    block: SignedBlock,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
) -> HashOf<SignedTransaction> {
    let signed_hash = block
        .external_transactions()
        .next()
        .expect("transaction-details fixture contains an external transaction")
        .hash();
    assert_eq!(
        iroha_core::tx::external_entrypoint_hash_from_signed_hash(signed_hash),
        entrypoint_hash,
    );
    let header = block.header();
    let height = NonZeroUsize::new(
        usize::try_from(header.height().get()).expect("transaction-details height fits usize"),
    )
    .expect("transaction-details height is nonzero");
    let block_hash = store_block(app, block);
    record_committed_block_hash_for_test(app, header.clone(), block_hash);
    let mut state_block = app.state.block(header);
    state_block
        .transactions
        .insert_block([entrypoint_hash].into_iter().collect(), height);
    state_block
        .commit()
        .expect("commit transaction-details membership index");
    signed_hash
}
fn signed_transaction_details_query(
    key_pair: &KeyPair,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
) -> SignedQuery {
    authorize_query_for_test(
        iroha_data_model::query::QueryRequest::Start(
            build_exact_transaction_details_query_for_test(entrypoint_hash),
        ),
        AccountId::new(key_pair.public_key().clone()),
    )
    .sign(key_pair)
}
#[tokio::test]
async fn transaction_details_allows_sender_and_batch_recipient_but_rejects_other_accounts() {
    use iroha_data_model::events::data::prelude::{
        AssetBatchTransferLegStatus, AssetBatchTransferOutcome,
    };
    let sender_key =
        checked_torii_test_ed25519_keypair(0x24, "derive transaction-details sender key");
    let recipient_key =
        checked_torii_test_ed25519_keypair(0x35, "derive transaction-details recipient key");
    let unrelated_key =
        checked_torii_test_ed25519_keypair(0x36, "derive transaction-details unrelated key");
    let sender = AccountId::new(sender_key.public_key().clone());
    let recipient = AccountId::new(recipient_key.public_key().clone());
    let unrelated = AccountId::new(unrelated_key.public_key().clone());
    let app = mk_app_state_for_tests_with_world(transaction_details_test_world(&[
        sender.clone(),
        recipient.clone(),
        unrelated,
    ]));
    let (mut block, entrypoint_hash) = make_signed_block(1, None);
    let outcome = AssetBatchTransferOutcome {
        leg_index: 0,
        leg_id: "private-receipt".to_owned(),
        asset: AssetId::new(
            AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").expect("asset domain"),
                Name::from_str("rose").expect("asset name"),
            ),
            sender.clone(),
        ),
        destination: recipient.clone(),
        amount: Quantity::from(7_u32),
        status: AssetBatchTransferLegStatus::Applied,
    };
    block
        .set_batch_transfer_outcomes(std::collections::BTreeMap::from([(
            entrypoint_hash,
            vec![outcome.clone()],
        )]))
        .expect("attach batch receipt to transaction-details fixture");
    let signed_hash = store_and_index_transaction_details_block(&app, block, entrypoint_hash);
    let public = pipeline_status_response(
        app.clone(),
        signed_hash.to_string(),
        Some("local"),
        "public status projection",
    )
    .await;
    let public_json = decode_torii_json(public, "public status body", "public status JSON").await;
    assert!(public_json.get("batch_transfer_outcomes").is_none());
    assert!(public_json.get("transaction").is_none());
    let public_text = norito::json::to_json(&public_json).expect("render public status JSON");
    assert!(!public_text.contains("private-receipt"));
    assert!(!public_text.contains(&recipient.to_string()));
    for (label, key_pair) in [("sender", &sender_key), ("recipient", &recipient_key)] {
        let response = super::handler_pipeline_transaction_details(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            None,
            versioned_query_for_test(signed_transaction_details_query(key_pair, entrypoint_hash)),
        )
        .await
        .unwrap_or_else(|error| {
            panic!("{label} should read involved transaction details: {error}")
        });
        assert_eq!(response.status(), StatusCode::OK);
        let body = torii_body_bytes(response, "transaction-details response body").await;
        let details: iroha_torii_shared::PipelineTransactionDetailsResponse =
            norito::json::from_slice(&body).expect("typed transaction-details JSON");
        assert_eq!(details.transaction.entrypoint_hash(), &entrypoint_hash);
        assert_eq!(
            details.transaction.result().batch_transfer_outcomes(),
            &[outcome.clone()]
        );
    }
    let error = match super::handler_pipeline_transaction_details(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        versioned_query_for_test(signed_transaction_details_query(
            &unrelated_key,
            entrypoint_hash,
        )),
    )
    .await
    {
        Ok(_) => panic!("an uninvolved account must not read transaction details"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        super::Error::Query(ValidationFail::NotPermitted(_))
    ));
}
#[tokio::test]
async fn transaction_details_allows_operator_and_rejects_wrong_network_and_replay() {
    let sender_key =
        checked_torii_test_ed25519_keypair(0x24, "derive operator fixture transaction sender");
    let operator_key =
        checked_torii_test_ed25519_keypair(0x37, "derive transaction-details operator key");
    let sender = AccountId::new(sender_key.public_key().clone());
    let operator = AccountId::new(operator_key.public_key().clone());
    let app = mk_app_state_for_tests_with_world(transaction_details_test_world(&[
        sender,
        operator.clone(),
    ]));
    let (block, entrypoint_hash) = make_signed_block(1, None);
    store_and_index_transaction_details_block(&app, block, entrypoint_hash);
    grant_account_permission_for_test(&app, &operator, CanReadAllLedgerData.into());
    let signed = signed_transaction_details_query(&operator_key, entrypoint_hash);
    let signed_bytes = norito::to_bytes(&signed).expect("encode replayable signed-query fixture");
    let first: SignedQuery =
        norito::decode_from_bytes(&signed_bytes).expect("decode first signed-query fixture");
    let replayed: SignedQuery =
        norito::decode_from_bytes(&signed_bytes).expect("decode replayed signed-query fixture");
    let response = super::handler_pipeline_transaction_details(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        versioned_query_for_test(first),
    )
    .await
    .expect("operator capability should authorize transaction details");
    assert_eq!(response.status(), StatusCode::OK);
    let replay = match super::handler_pipeline_transaction_details(
        State(app.clone()),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        versioned_query_for_test(replayed),
    )
    .await
    {
        Ok(_) => panic!("the same signed-query nonce must be one-shot"),
        Err(error) => error,
    };
    assert!(format!("{replay:?}").contains("nonce already used"));
    let mut wrong_network = authorize_query_for_test(
        iroha_data_model::query::QueryRequest::Start(
            build_exact_transaction_details_query_for_test(entrypoint_hash),
        ),
        operator,
    );
    wrong_network.network_id = NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x38; Hash::LENGTH])),
    );
    let wrong_network = wrong_network.sign(&operator_key);
    let error = match super::handler_pipeline_transaction_details(
        State(app),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        None,
        versioned_query_for_test(wrong_network),
    )
    .await
    {
        Ok(_) => panic!("a transaction-details query cannot cross network lineages"),
        Err(error) => error,
    };
    assert!(format!("{error:?}").contains("different network genesis"));
}
#[test]
fn transaction_details_rejects_unsigned_and_broadened_queries() {
    let key_pair = checked_torii_test_ed25519_keypair(0x39, "derive transaction-details shape key");
    let authority = AccountId::new(key_pair.public_key().clone());
    let entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"transaction-details-shape",
    ));
    let unsigned = authorize_query_for_test(
        iroha_data_model::query::QueryRequest::Start(
            build_exact_transaction_details_query_for_test(entrypoint_hash),
        ),
        authority.clone(),
    );
    let bytes = norito::to_bytes(&unsigned).expect("encode unsigned query request");
    assert!(
        norito::decode_from_bytes::<SignedQuery>(&bytes).is_err(),
        "the signed-query endpoint must not decode an unsigned request as an admission witness",
    );
    let broadened = authorize_query_for_test(
        iroha_data_model::query::QueryRequest::Start(build_find_transactions_query_for_test()),
        authority,
    );
    assert!(
        exact_transaction_details_query_hash(&broadened).is_err(),
        "unbounded transaction history must not qualify as an exact details query",
    );
}
#[tokio::test]
async fn pipeline_status_handler_resolves_sealed_reveal_carrier_and_signed_alias() {
    let app = mk_app_state_for_tests();
    let (block, reveal_entry_hash) = make_sealed_reveal_block(1, None);
    let signed_hash = block
        .external_transactions()
        .next()
        .expect("sealed reveal carries one signed transaction")
        .hash();
    let signed_entrypoint_alias =
        iroha_core::tx::external_entrypoint_hash_from_signed_hash(signed_hash.clone());
    let header = block.header();
    store_block(&app, block);
    let height = header.height();
    let height_usize = usize::try_from(height.get()).expect("height usize");
    let height_nz = NonZeroUsize::new(height_usize).expect("height");
    let mut state_block = app.state.block(header);
    let entrypoint_hashes: HashSet<_> = [reveal_entry_hash, signed_entrypoint_alias]
        .into_iter()
        .collect();
    state_block
        .transactions
        .insert_block(entrypoint_hashes, height_nz);
    state_block.commit().expect("commit");
    assert_eq!(
        canonical_carrier_hash_for_indexed_transaction_identity(
            app.as_ref(),
            height_nz,
            &signed_entrypoint_alias,
        )
        .expect("signed execution alias resolves its sealed carrier"),
        reveal_entry_hash
    );
    let resp =
        pipeline_status_response(app.clone(), reveal_entry_hash.to_string(), None, "ok").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let payload = torii_json_body(resp).await;
    assert_eq!(
        payload
            .get("status")
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str),
        Some("Applied")
    );
    assert_eq!(
        payload
            .get("resolved_from")
            .and_then(norito::json::Value::as_str),
        Some("state")
    );
    let resp = pipeline_status_response(app, signed_hash.to_string(), None, "signed alias").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let payload = torii_json_body(resp).await;
    assert_eq!(
        payload
            .get("status")
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str),
        Some("Applied")
    );
    assert_eq!(
        payload
            .get("resolved_from")
            .and_then(norito::json::Value::as_str),
        Some("state")
    );
}
#[tokio::test]
async fn pipeline_status_handler_prefers_state_over_stale_queued_cache() {
    let app = mk_app_state_for_tests();
    let (block, _) = make_signed_block(1, None);
    let header = block.header();
    let tx = block.external_transactions().next().expect("tx");
    let tx_hash = tx.hash();
    let tx_entry_hash = tx.hash_as_entrypoint();
    store_block(&app, block);
    app.pipeline_status_cache.record_entry(
        tx_hash,
        PipelineStatusEntry::fresh(PipelineStatusKind::Queued, None, None),
    );
    let height = header.height();
    let height_usize = usize::try_from(height.get()).expect("height usize");
    let height_nz = NonZeroUsize::new(height_usize).expect("height");
    let mut state_block = app.state.block(header);
    let tx_hashes: HashSet<_> = [tx_entry_hash].into_iter().collect();
    state_block.transactions.insert_block(tx_hashes, height_nz);
    state_block.commit().expect("commit");
    let resp = pipeline_status_response(app.clone(), tx_hash.to_string(), None, "ok").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let payload = torii_json_body(resp).await;
    assert_eq!(
        payload
            .get("status")
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str),
        Some("Applied")
    );
    assert_eq!(
        payload
            .get("resolved_from")
            .and_then(norito::json::Value::as_str),
        Some("state")
    );
}
#[tokio::test]
async fn pipeline_status_handler_prefers_state_over_stale_rejected_cache() {
    let app = mk_app_state_for_tests();
    let (block, _) = make_signed_block(1, None);
    let header = block.header();
    let tx = block.external_transactions().next().expect("tx");
    let tx_hash = tx.hash();
    let tx_entry_hash = tx.hash_as_entrypoint();
    store_block(&app, block);
    let rejection = TransactionRejectionReason::Validation(ValidationFail::TooComplex);
    app.pipeline_status_cache.record_entry(
        tx_hash,
        PipelineStatusEntry::fresh(
            PipelineStatusKind::Rejected,
            None,
            Some(pipeline_rejection_summary(&rejection)),
        ),
    );
    let height = header.height();
    let height_usize = usize::try_from(height.get()).expect("height usize");
    let height_nz = NonZeroUsize::new(height_usize).expect("height");
    let mut state_block = app.state.block(header);
    let tx_hashes: HashSet<_> = [tx_entry_hash].into_iter().collect();
    state_block.transactions.insert_block(tx_hashes, height_nz);
    state_block.commit().expect("commit");
    let resp = pipeline_status_response(app.clone(), tx_hash.to_string(), None, "ok").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let payload = torii_json_body(resp).await;
    assert_eq!(
        payload
            .get("status")
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str),
        Some("Applied")
    );
    assert_eq!(
        payload
            .get("resolved_from")
            .and_then(norito::json::Value::as_str),
        Some("state")
    );
}
#[tokio::test]
async fn public_pipeline_status_does_not_expose_rejection_details() {
    let app = mk_app_state_for_tests();
    let (block, _) = make_signed_block(1, None);
    let tx_hash = block.external_transactions().next().expect("tx").hash();
    let reason = TransactionRejectionReason::Validation(ValidationFail::TooComplex);
    app.pipeline_status_cache.record_entry(
        tx_hash,
        PipelineStatusEntry::fresh(
            PipelineStatusKind::Rejected,
            None,
            Some(pipeline_rejection_summary(&reason)),
        ),
    );
    let resp = pipeline_status_response(app.clone(), tx_hash.to_string(), None, "ok").await;
    assert_eq!(resp.status(), StatusCode::OK);
    let payload = torii_json_body(resp).await;
    assert_eq!(
        payload
            .get("status")
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str),
        Some("Rejected")
    );
    assert!(
        payload
            .get("status")
            .and_then(|status| status.get("rejection_reason"))
            .is_none()
    );
    let encoded = norito::json::to_json(&payload).expect("public status JSON");
    assert!(!encoded.contains(&reason.to_string()));
}
#[tokio::test]
async fn ledger_headers_respect_from_and_limit() {
    let app = mk_app_state_for_tests();
    let (block1, _) = make_signed_block(1, None);
    let first_header = block1.header();
    let first_hash = store_block(&app, block1);
    record_committed_block_hash_for_test(&app, first_header, first_hash);
    let (block2, _) = make_signed_block(2, Some(first_hash));
    let second_header = block2.header();
    let second_hash = store_block(&app, block2);
    record_committed_block_hash_for_test(&app, second_header, second_hash);
    let resp = super::handler_ledger_headers(
        State(app.clone()),
        crate::NoritoQuery(routing::HistoryWindowQuery {
            from: Some(2),
            limit: Some(1),
        }),
        HeaderMap::new(),
    )
    .await
    .expect("ok");
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CONTENT_TYPE)
            .map(HeaderValue::as_bytes),
        Some(b"application/json".as_slice())
    );
    let bytes = torii_body_bytes(resp, "body bytes").await;
    let headers: Vec<BlockHeader> = norito::json::from_slice(&bytes).expect("json decode");
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].height().get(), 2);
    let norito_resp = super::handler_ledger_headers(
        State(app),
        crate::NoritoQuery(routing::HistoryWindowQuery {
            from: Some(2),
            limit: Some(2),
        }),
        {
            let mut headers = HeaderMap::new();
            headers.insert(
                axum::http::header::ACCEPT,
                HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
            );
            headers
        },
    )
    .await
    .expect("ok");
    let norito_bytes = torii_body_bytes(norito_resp, "norito body").await;
    let archived = norito::from_bytes::<Vec<BlockHeader>>(&norito_bytes).expect("archive");
    let decoded: Vec<BlockHeader> = norito::core::NoritoDeserialize::deserialize(archived);
    assert_eq!(decoded.len(), 2);
    assert_eq!(decoded[0].height().get(), 2);
    assert_eq!(decoded[1].height().get(), 1);
}
#[tokio::test]
async fn ledger_state_endpoints_return_exact_v2_finality_in_json_and_norito() {
    let (app, _, expected_artifact) = app_with_indexed_sccp_message_for_test(true);
    let expected_root = expected_artifact
        .commit_qc
        .execution_commitment
        .post_state_root;
    let result_root = app
        .state
        .block_by_height(NonZeroUsize::new(1).expect("nonzero height"))
        .expect("committed fixture block")
        .header()
        .result_merkle_root()
        .map(|hash| Hash::prehashed(*hash.as_ref()))
        .expect("fixture result root");
    assert_ne!(
        expected_root, result_root,
        "the result Merkle root must be an adversarially distinct fallback candidate"
    );
    let resp = handler_ledger_state_root(
        State(Arc::clone(&app)),
        axum::extract::Path(1),
        HeaderMap::new(),
    )
    .await
    .expect("authenticated state-root response");
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CONTENT_TYPE)
            .map(HeaderValue::as_bytes),
        Some(b"application/json".as_slice())
    );
    let bytes = torii_body_bytes(resp, "json body").await;
    let value: norito::json::Value = norito::json::from_slice(&bytes).expect("JSON value");
    let mut fields = value
        .as_object()
        .expect("state finality object")
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    fields.sort_unstable();
    assert_eq!(
        fields,
        [
            "block_hash",
            "block_header",
            "finality_artifact",
            "height",
            "state_root",
        ]
    );
    for retired in ["source", "commit_qc"] {
        let mut hostile = value.clone();
        hostile
            .as_object_mut()
            .expect("state finality object")
            .insert(retired.to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<StateFinalityResponse>(hostile).is_err(),
            "closed first-release response accepted retired field {retired}"
        );
    }
    let payload: StateFinalityResponse =
        norito::json::from_slice(&bytes).expect("typed state finality JSON");
    assert_eq!(payload.height, 1);
    assert_eq!(payload.block_hash, expected_artifact.block_hash);
    assert_eq!(payload.block_header.hash(), expected_artifact.block_hash);
    assert_eq!(payload.state_root, expected_root);
    assert_eq!(payload.finality_artifact, expected_artifact);
    let mut accept = HeaderMap::new();
    accept.insert(
        axum::http::header::ACCEPT,
        HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
    );
    let norito_resp = handler_ledger_state_root(
        State(Arc::clone(&app)),
        axum::extract::Path(1),
        accept.clone(),
    )
    .await
    .expect("authenticated state-root Norito response");
    let norito_bytes = torii_body_bytes(norito_resp, "norito body").await;
    let archived =
        norito::from_bytes::<StateFinalityResponse>(&norito_bytes).expect("state root archive");
    let decoded: StateFinalityResponse = norito::core::NoritoDeserialize::deserialize(archived);
    assert_eq!(decoded.state_root, expected_root);
    assert_eq!(decoded.finality_artifact, expected_artifact);
    let resp = handler_ledger_state_proof(
        State(Arc::clone(&app)),
        axum::extract::Path(1),
        HeaderMap::new(),
    )
    .await
    .expect("authenticated state-proof JSON response");
    let body = torii_body_bytes(resp, "body").await;
    let proof: StateFinalityResponse = norito::json::from_slice(&body).expect("JSON proof");
    assert_eq!(proof.height, 1);
    assert_eq!(proof.block_hash, expected_artifact.block_hash);
    assert_eq!(proof.state_root, expected_root);
    assert_eq!(proof.finality_artifact, expected_artifact);
    let norito_resp = handler_ledger_state_proof(State(app), axum::extract::Path(1), accept)
        .await
        .expect("authenticated state-proof Norito response");
    let norito_bytes = torii_body_bytes(norito_resp, "bytes").await;
    let archived =
        norito::from_bytes::<StateFinalityResponse>(&norito_bytes).expect("state proof archive");
    let decoded: StateFinalityResponse = norito::core::NoritoDeserialize::deserialize(archived);
    assert_eq!(decoded.state_root, expected_root);
    assert_eq!(decoded.finality_artifact, expected_artifact);
}
#[tokio::test]
async fn state_proof_http_roundtrip_supports_json_and_norito() {
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };
    use tower::ServiceExt as _;
    let (app, _, expected_artifact) = app_with_indexed_sccp_message_for_test(true);
    let expected_root = expected_artifact
        .commit_qc
        .execution_commitment
        .post_state_root;
    let router = Router::new()
        .route(uri::LEDGER_STATE_PROOF, get(handler_ledger_state_proof))
        .with_state(app.clone());
    let request = Request::builder()
        .uri("/v1/ledger/state-proof/1")
        .body(Body::empty())
        .expect("request");
    let response = router.clone().oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = torii_body_bytes(response, "body").await;
    let proof: StateFinalityResponse = norito::json::from_slice(&bytes).expect("JSON proof");
    assert_eq!(proof.height, 1);
    assert_eq!(proof.block_hash, expected_artifact.block_hash);
    assert_eq!(proof.state_root, expected_root);
    assert_eq!(proof.finality_artifact, expected_artifact);
    let request = Request::builder()
        .uri("/v1/ledger/state-proof/1")
        .header(axum::http::header::ACCEPT, crate::utils::NORITO_MIME_TYPE)
        .body(Body::empty())
        .expect("request");
    let response = router.oneshot(request).await.expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = torii_body_bytes(response, "body").await;
    let archived =
        norito::from_bytes::<StateFinalityResponse>(&bytes).expect("archived state proof");
    let proof: StateFinalityResponse = norito::core::NoritoDeserialize::deserialize(archived);
    assert_eq!(proof.height, 1);
    assert_eq!(proof.block_hash, expected_artifact.block_hash);
    assert_eq!(proof.state_root, expected_root);
    assert_eq!(proof.finality_artifact, expected_artifact);
}
fn assert_ledger_state_handler_status(error: Error, expected: StatusCode) {
    assert_eq!(error.into_response().status(), expected);
}
#[tokio::test]
async fn ledger_state_endpoints_require_v2_finality() {
    let (app, _, _) = app_with_indexed_sccp_message_for_test(false);
    let root_error = handler_ledger_state_root(
        State(Arc::clone(&app)),
        axum::extract::Path(1),
        HeaderMap::new(),
    )
    .await
    .expect_err("a state root without v2 finality must fail closed");
    assert_ledger_state_handler_status(root_error, StatusCode::NOT_FOUND);
    let proof_error =
        handler_ledger_state_proof(State(app), axum::extract::Path(1), HeaderMap::new())
            .await
            .expect_err("a state proof without v2 finality must fail closed");
    assert_ledger_state_handler_status(proof_error, StatusCode::NOT_FOUND);
}
#[tokio::test]
async fn ledger_state_endpoints_reject_wrong_height_finality_record() {
    let (app, _, artifact) = app_with_indexed_sccp_message_for_test(true);
    let (block, _) = make_signed_block(2, Some(artifact.block_hash));
    let block_header = block.header();
    let block_hash = store_block(&app, block);
    record_committed_block_hash_for_test(&app, block_header, block_hash);
    let height_one = app.kura.v2_finality_artifact_path_for_testing(1);
    let height_two = app.kura.v2_finality_artifact_path_for_testing(2);
    std::fs::copy(height_one, height_two).expect("install wrong-height finality record");
    let root_error = handler_ledger_state_root(
        State(Arc::clone(&app)),
        axum::extract::Path(2),
        HeaderMap::new(),
    )
    .await
    .expect_err("wrong-height finality must fail closed");
    assert_ledger_state_handler_status(root_error, StatusCode::INTERNAL_SERVER_ERROR);
    let proof_error =
        handler_ledger_state_proof(State(app), axum::extract::Path(2), HeaderMap::new())
            .await
            .expect_err("wrong-height finality proof must fail closed");
    assert_ledger_state_handler_status(proof_error, StatusCode::INTERNAL_SERVER_ERROR);
}
#[tokio::test]
async fn ledger_state_endpoints_reject_forged_v2_finality_signature() {
    let (app, _, artifact) = app_with_indexed_sccp_message_for_test(true);
    let path = app.kura.v2_finality_artifact_path_for_testing(1);
    let mut bytes = std::fs::read(&path).expect("read finality record");
    let signature = artifact.commit_qc.aggregate_signature.as_slice();
    let offsets = bytes
        .windows(signature.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == signature).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(
        offsets.len(),
        1,
        "aggregate signature must have one exact encoded location"
    );
    bytes[offsets[0]] ^= 0x01;
    std::fs::write(path, bytes).expect("forge finality aggregate signature");
    let root_error = handler_ledger_state_root(
        State(Arc::clone(&app)),
        axum::extract::Path(1),
        HeaderMap::new(),
    )
    .await
    .expect_err("forged finality must fail closed");
    assert_ledger_state_handler_status(root_error, StatusCode::INTERNAL_SERVER_ERROR);
    let proof_error =
        handler_ledger_state_proof(State(app), axum::extract::Path(1), HeaderMap::new())
            .await
            .expect_err("forged finality proof must fail closed");
    assert_ledger_state_handler_status(proof_error, StatusCode::INTERNAL_SERVER_ERROR);
}
#[tokio::test]
async fn block_proof_handler_emits_norito() {
    let app = mk_app_state_for_tests();
    let (block, entry_hash) = make_signed_block(1, None);
    let expected_block_hash = block.hash();
    let expected_executed_wire_hash = block
        .executed_block_wire_hash()
        .expect("executed block wire hash");
    let expected_entry_commitment = block
        .full_entry_merkle_commitment()
        .expect("full entry commitment");
    let expected_result_commitment = block.result_merkle_commitment().expect("result commitment");
    store_block(&app, block);
    let entry_hex = hex::encode(entry_hash.as_ref());
    let resp = super::handler_block_proof(State(app), axum::extract::Path((1, entry_hex)))
        .await
        .expect("ok")
        .into_response();
    assert_eq!(
        resp.headers()
            .get(axum::http::header::CONTENT_TYPE)
            .map(HeaderValue::as_bytes),
        Some(crate::utils::NORITO_MIME_TYPE.as_bytes())
    );
    let bytes = torii_body_bytes(resp, "norito payload").await;
    let archived = norito::from_bytes::<BlockProofs>(&bytes).expect("archive decode");
    let proofs: BlockProofs = norito::core::NoritoDeserialize::deserialize(archived);
    assert_eq!(proofs.block_height.get(), 1);
    assert_eq!(proofs.block_hash, expected_block_hash);
    assert_eq!(proofs.executed_block_wire_hash, expected_executed_wire_hash);
    assert_eq!(proofs.entry_hash, entry_hash);
    assert_eq!(proofs.entry_commitment, expected_entry_commitment);
    assert!(proofs.entry_proof.verify(&expected_entry_commitment));
    assert_eq!(proofs.result_commitment, expected_result_commitment);
    let result_proof = proofs.result_proof;
    assert_eq!(
        proofs.entry_proof.proof().leaf_index(),
        result_proof.proof().leaf_index()
    );
    assert!(result_proof.verify(&expected_result_commitment));
    assert!(proofs.fastpq_transcripts.is_empty());
}
const EXECUTED_BLOCK_WIRE_TEST_OWNER_STACK_BYTES: usize = 8 * 1024 * 1024;
fn run_executed_block_wire_handler_test<F, Fut>(name: &'static str, test: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let owner = std::thread::Builder::new()
        .name(name.to_owned())
        .stack_size(EXECUTED_BLOCK_WIRE_TEST_OWNER_STACK_BYTES)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build executed-block handler test runtime")
                .block_on(test());
        })
        .expect("spawn executed-block handler test owner");
    if let Err(payload) = owner.join() {
        std::panic::resume_unwind(payload);
    }
}
#[test]
fn executed_block_wire_handler_returns_the_exact_finalized_canonical_wire() {
    run_executed_block_wire_handler_test("executed-wire-canonical", || async {
        let app = mk_app_state_for_tests();
        let (block, _) = make_signed_block(1, None);
        let header = block.header();
        let expected_wire = block.encode_wire().expect("canonical executed wire");
        let block_hash = store_block(&app, block);
        record_committed_block_hash_for_test(&app, header, block_hash);
        let response =
            super::handler_ledger_executed_block_wire(State(app), axum::extract::Path(1))
                .await
                .expect("finalized block wire")
                .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .map(HeaderValue::as_bytes),
            Some(crate::utils::NORITO_MIME_TYPE.as_bytes())
        );
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::X_CONTENT_TYPE_OPTIONS)
                .map(HeaderValue::as_bytes),
            Some(b"nosniff".as_slice())
        );
        let actual_wire = torii_body_bytes(response, "wire body").await;
        assert_eq!(actual_wire.as_ref(), expected_wire.as_slice());
    });
}
#[test]
fn executed_block_wire_handler_rejects_missing_and_unfinalized_heights() {
    run_executed_block_wire_handler_test("executed-wire-negative-heights", || async {
        let app = mk_app_state_for_tests();
        let (staged, _) = make_signed_block(1, None);
        store_block(&app, staged);
        for height in [1_u64, 2] {
            let error = super::handler_ledger_executed_block_wire(
                State(app.clone()),
                axum::extract::Path(height),
            )
            .await
            .expect_err("unfinalized or missing block must fail");
            assert_eq!(error.into_response().status(), StatusCode::NOT_FOUND);
        }
        let error =
            super::handler_ledger_executed_block_wire(State(app.clone()), axum::extract::Path(0))
                .await
                .expect_err("zero height must fail");
        assert_eq!(error.into_response().status(), StatusCode::BAD_REQUEST);
        const SMALL_HANDLER_STACK_BYTES: usize = 256 * 1024;
        let small_stack_app = app.clone();
        let caller = std::thread::Builder::new()
            .name("executed-wire-small-handler".to_owned())
            .stack_size(SMALL_HANDLER_STACK_BYTES)
            .spawn(move || {
                for height in [1_u64, 2] {
                    let error =
                        futures::executor::block_on(super::handler_ledger_executed_block_wire(
                            State(small_stack_app.clone()),
                            axum::extract::Path(height),
                        ))
                        .expect_err("small-stack staged or missing block must fail");
                    assert_eq!(error.into_response().status(), StatusCode::NOT_FOUND);
                }
                let error = futures::executor::block_on(super::handler_ledger_executed_block_wire(
                    State(small_stack_app),
                    axum::extract::Path(0),
                ))
                .expect_err("small-stack zero height must fail");
                assert_eq!(error.into_response().status(), StatusCode::BAD_REQUEST);
            })
            .expect("spawn small-stack executed-block handler caller");
        if let Err(payload) = caller.join() {
            std::panic::resume_unwind(payload);
        }
    });
}
#[test]
fn executed_block_wire_handler_fails_closed_on_hash_and_execution_shape_drift() {
    run_executed_block_wire_handler_test("executed-wire-fail-closed", || async {
        let mismatched = mk_app_state_for_tests();
        let (block, _) = make_signed_block(1, None);
        let header = block.header();
        store_block(&mismatched, block);
        let other_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"different finalized block hash",
        ));
        record_committed_block_hash_for_test(&mismatched, header, other_hash);
        let error =
            super::handler_ledger_executed_block_wire(State(mismatched), axum::extract::Path(1))
                .await
                .expect_err("state/Kura hash drift must fail");
        assert_eq!(
            error.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        let resultless = mk_app_state_for_tests();
        let (block, _) = make_signed_block(1, None);
        let block = block.canonical_resultless_proposal();
        let header = block.header();
        let block_hash = store_block(&resultless, block);
        record_committed_block_hash_for_test(&resultless, header, block_hash);
        let error =
            super::handler_ledger_executed_block_wire(State(resultless), axum::extract::Path(1))
                .await
                .expect_err("resultless proposal must not be exposed as executed wire");
        assert_eq!(
            error.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    });
}
#[test]
fn executed_block_wire_carrier_bound_is_exact() {
    let maximum =
        iroha_data_model::block::proofs::AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1;
    assert!(super::finalized_block_wire_fits_carrier_v1(maximum));
    assert!(!super::finalized_block_wire_fits_carrier_v1(
        maximum.saturating_add(1)
    ));
    assert_eq!(
        super::executed_block_wire_too_large_response(
            NonZeroU64::new(1).expect("non-zero height"),
        )
        .status(),
        StatusCode::PAYLOAD_TOO_LARGE,
    );
}
#[test]
fn block_proof_errors_distinguish_absence_from_persisted_corruption() {
    let height = NonZeroU64::new(7).expect("non-zero height");
    let other_height = NonZeroU64::new(8).expect("non-zero height");
    let entry_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
        b"missing block proof entrypoint",
    ));
    for error in [
        BlockProofError::BlockNotFound(height),
        BlockProofError::EntrypointNotFound {
            entry_hash,
            block_height: height,
        },
    ] {
        assert_eq!(
            super::map_block_proof_error(error).into_response().status(),
            StatusCode::NOT_FOUND
        );
    }
    for error in [
        BlockProofError::BlockHashMismatch {
            block_height: height,
            expected: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"expected committed block hash",
            )),
            actual: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"mismatched Kura body hash",
            )),
        },
        BlockProofError::BlockHeightMismatch {
            requested: height,
            actual: other_height,
        },
        BlockProofError::MissingResults(height),
        BlockProofError::ExecutionResultMissing {
            entry_hash,
            block_height: height,
        },
        BlockProofError::MerkleProofUnavailable {
            entry_hash,
            block_height: height,
        },
        BlockProofError::ExecutedBlockWireHashUnavailable(height),
    ] {
        assert_eq!(
            super::map_block_proof_error(error).into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
include!("part_5b_sccp_bundle.rs");
