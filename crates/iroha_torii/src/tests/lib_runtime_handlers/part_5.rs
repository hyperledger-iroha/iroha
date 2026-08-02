    #[test]
    fn pipeline_status_merge_prefers_committed_success_over_cached_rejection() {
        let now = Instant::now();
        let rejection = TransactionRejectionReason::Validation(ValidationFail::TooComplex);
        let mut entry =
            PipelineStatusEntry::at_time(PipelineStatusKind::Rejected, None, Some(rejection), now);
        entry.merge_from_event(PipelineStatusEntry::at_time(
            PipelineStatusKind::Committed,
            NonZeroU64::new(7),
            None,
            now + Duration::from_secs(1),
        ));
        assert_eq!(entry.kind, PipelineStatusKind::Committed);
        assert_eq!(entry.block_height, NonZeroU64::new(7));
        assert!(entry.rejection.is_none());

        entry.merge_from_event(PipelineStatusEntry::at_time(
            PipelineStatusKind::Applied,
            NonZeroU64::new(7),
            None,
            now + Duration::from_secs(2),
        ));
        assert_eq!(entry.kind, PipelineStatusKind::Applied);
        assert_eq!(entry.block_height, NonZeroU64::new(7));
        assert!(entry.rejection.is_none());
    }

    #[test]
    fn pipeline_status_cache_records_transaction_event() {
        let cache = PipelineStatusCache::new();
        let (block, _) = make_signed_block(1, None);
        let tx_hash = block.external_transactions().next().expect("tx").hash();
        let height = NonZeroU64::new(2).expect("height");
        let event = TransactionEvent {
            hash: tx_hash,
            block_height: Some(height),
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(1),
            status: TransactionStatus::Approved,
        };
        cache.record_transaction_event(&event);
        let stored = cache.lookup(&tx_hash).expect("entry");
        assert_eq!(stored.kind, PipelineStatusKind::Approved);
        assert_eq!(stored.block_height, Some(height));
        assert!(stored.rejection.is_none());
    }

    #[tokio::test]
    async fn pipeline_status_cache_records_block_event() {
        let app = mk_app_state_for_tests();
        let (block, _) = make_signed_block(1, None);
        let header = block.header();
        let tx = block.external_transactions().next().expect("tx");
        let tx_hash = tx.hash();
        store_block(&app, block);
        let event = BlockEvent {
            header,
            status: BlockStatus::Applied,
        };
        app.pipeline_status_cache
            .record_block_event(&event, &app.kura);
        let stored = app.pipeline_status_cache.lookup(&tx_hash).expect("entry");
        assert_eq!(stored.kind, PipelineStatusKind::Applied);
        let height = NonZeroU64::new(1).expect("height");
        assert_eq!(stored.block_height, Some(height));
    }

    #[tokio::test]
    async fn pipeline_status_cache_refreshes_pending_block() {
        let app = mk_app_state_for_tests();
        let (block, _) = make_signed_block(1, None);
        let header = block.header();
        let tx_hash = block.external_transactions().next().expect("tx").hash();
        let event = BlockEvent {
            header,
            status: BlockStatus::Committed,
        };
        app.pipeline_status_cache
            .record_block_event(&event, &app.kura);
        assert!(app.pipeline_status_cache.lookup(&tx_hash).is_none());
        store_block(&app, block);
        app.pipeline_status_cache.refresh_pending_blocks(&app.kura);
        let stored = app.pipeline_status_cache.lookup(&tx_hash).expect("entry");
        assert_eq!(stored.kind, PipelineStatusKind::Committed);
    }

    #[test]
    fn pipeline_status_cache_prunes_stale_entries() {
        let cache = PipelineStatusCache::with_limits(10, Duration::from_secs(1));
        let (block, _) = make_signed_block(1, None);
        let tx_hash = block.external_transactions().next().expect("tx").hash();
        let now = Instant::now();
        let stale = now
            .checked_sub(Duration::from_secs(5))
            .expect("time subtraction");
        cache.record_entry(
            tx_hash,
            PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, stale),
        );
        cache.prune(now);
        assert!(cache.lookup(&tx_hash).is_none());
    }

    #[test]
    fn pipeline_status_cache_eviction_respects_capacity() {
        let cache = PipelineStatusCache::with_limits(1, Duration::from_secs(60));
        let (block_a, _) = make_signed_block(1, None);
        let (block_b, _) = make_signed_block(2, None);
        let hash_a = block_a.external_transactions().next().expect("tx").hash();
        let hash_b = block_b.external_transactions().next().expect("tx").hash();
        let now = Instant::now();
        let stale = now
            .checked_sub(Duration::from_secs(5))
            .expect("time subtraction");
        cache.record_entry(
            hash_a,
            PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, stale),
        );
        cache.record_entry(
            hash_b,
            PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, now),
        );
        cache.prune(now);
        assert!(cache.lookup(&hash_a).is_none());
        assert!(cache.lookup(&hash_b).is_some());
    }

    #[test]
    fn pipeline_status_cache_live_counts_track_entries_and_pending_blocks() {
        let cache = PipelineStatusCache::with_limits(1, Duration::from_secs(60));
        let (block_a, _) = make_signed_block(1, None);
        let (block_b, _) = make_signed_block(2, None);
        let hash_a = block_a.external_transactions().next().expect("tx").hash();
        let hash_b = block_b.external_transactions().next().expect("tx").hash();
        let height_a = NonZeroU64::new(1).expect("height");
        let now = Instant::now();

        cache.record_entry(
            hash_a,
            PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, now),
        );
        cache.record_entry(
            hash_a,
            PipelineStatusEntry::at_time(PipelineStatusKind::Approved, None, None, now),
        );
        assert_eq!(cache.entry_count.load(Ordering::Relaxed), 1);

        cache.record_entry(
            hash_b,
            PipelineStatusEntry::at_time(
                PipelineStatusKind::Queued,
                None,
                None,
                now + Duration::from_secs(1),
            ),
        );
        cache.prune(now + Duration::from_secs(1));
        assert_eq!(
            cache.entry_count.load(Ordering::Relaxed),
            cache.entries.len()
        );
        assert!(cache.lookup(&hash_a).is_none());
        assert!(cache.lookup(&hash_b).is_some());

        cache.record_pending_block(
            height_a,
            PendingBlockStatus {
                kind: PipelineStatusKind::Committed,
                block_hash: block_a.header().hash(),
                observed_at: now,
            },
        );
        cache.record_pending_block(
            height_a,
            PendingBlockStatus {
                kind: PipelineStatusKind::Applied,
                block_hash: block_b.header().hash(),
                observed_at: now + Duration::from_secs(1),
            },
        );
        assert_eq!(cache.pending_count.load(Ordering::Relaxed), 1);
        assert!(cache.remove_pending_by_height(&height_a));
        assert_eq!(cache.pending_count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn pipeline_status_cache_keeps_refreshed_entry_when_stale_marker_prunes() {
        let cache = PipelineStatusCache::with_limits(10, Duration::from_secs(1));
        let (block, _) = make_signed_block(1, None);
        let tx_hash = block.external_transactions().next().expect("tx").hash();
        let now = Instant::now();
        let stale = now
            .checked_sub(Duration::from_secs(5))
            .expect("time subtraction");
        cache.record_entry(
            tx_hash,
            PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, stale),
        );
        cache.record_entry(
            tx_hash,
            PipelineStatusEntry::at_time(PipelineStatusKind::Queued, None, None, now),
        );

        cache.prune(now);

        assert!(cache.lookup(&tx_hash).is_some());
    }

    #[test]
    fn pipeline_status_cache_pending_blocks_prune_by_ttl_and_capacity() {
        let cache = PipelineStatusCache::with_limits(1, Duration::from_secs(1));
        let (block_a, _) = make_signed_block(1, None);
        let (block_b, _) = make_signed_block(2, None);
        let height_a = NonZeroU64::new(1).expect("height");
        let height_b = NonZeroU64::new(2).expect("height");
        let now = Instant::now();
        let stale = now
            .checked_sub(Duration::from_secs(5))
            .expect("time subtraction");
        cache.record_pending_block(
            height_a,
            PendingBlockStatus {
                kind: PipelineStatusKind::Committed,
                block_hash: block_a.header().hash(),
                observed_at: stale,
            },
        );
        cache.record_pending_block(
            height_b,
            PendingBlockStatus {
                kind: PipelineStatusKind::Applied,
                block_hash: block_b.header().hash(),
                observed_at: now,
            },
        );

        cache.prune(now);

        assert!(cache.pending_blocks.get(&height_a).is_none());
        assert!(cache.pending_blocks.get(&height_b).is_some());
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
    }

    #[tokio::test]
    async fn pipeline_status_string_query_preserves_all_decimal_hash() {
        use axum::extract::FromRequestParts as _;

        let hash = "11".repeat(32);
        let request = axum::http::Request::builder()
            .uri(format!(
                "/v1/pipeline/transactions/status?hash={hash}&scope=local"
            ))
            .body(())
            .expect("pipeline status request");
        let (mut parts, _) = request.into_parts();
        let crate::NoritoStringQuery(query) =
            crate::NoritoStringQuery::<PipelineStatusQuery>::from_request_parts(&mut parts, &())
                .await
                .expect("pipeline status string query should decode");

        assert_eq!(query.hash.as_deref(), Some(hash.as_str()));
        assert_eq!(query.scope.as_deref(), Some("local"));
    }

    #[tokio::test]
    async fn pipeline_status_handler_returns_queued() {
        let app = mk_app_state_for_tests();
        let keypair =
            checked_torii_test_ed25519_keypair(0x28, "derive Torii queued-status fixture key");
        let authority = AccountId::new(keypair.public_key().clone());
        let tx = checked_torii_test_transaction(
            TransactionBuilder::new(
                (*app.chain_id).clone(),
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
            app.chain_id.as_ref(),
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .expect("accepted");
        app.queue
            .push(accepted, app.state.view())
            .expect("queue push");

        let resp = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            None,
            crate::NoritoStringQuery(PipelineStatusQuery {
                hash: Some(tx.hash().to_string()),
                scope: Some("local".to_owned()),
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: norito::json::Value = norito::json::from_slice(&bytes).expect("json");
        let status_kind = payload
            .get("status")
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str);
        assert_eq!(status_kind, Some("Queued"));

        let resp_entry = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            None,
            crate::NoritoStringQuery(PipelineStatusQuery {
                hash: Some(tx.hash_as_entrypoint().to_string()),
                scope: Some("local".to_owned()),
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp_entry.status(), StatusCode::OK);
        let bytes_entry = axum::body::to_bytes(resp_entry.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload_entry: norito::json::Value =
            norito::json::from_slice(&bytes_entry).expect("json");
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
        let keypair =
            checked_torii_test_ed25519_keypair(0x29, "derive Torii typed-status fixture key");
        let authority = AccountId::new(keypair.public_key().clone());
        let tx = checked_torii_test_transaction(
            TransactionBuilder::new(
                (*app.chain_id).clone(),
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
            app.chain_id.as_ref(),
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
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
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
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: norito::json::Value = norito::json::from_slice(&bytes).expect("json");
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
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: routing::PipelinePreflightResponse =
            norito::decode_from_bytes(&bytes).expect("typed norito response");
        assert_eq!(payload.schema_version, 1);
        assert_eq!(payload.queue.size, 0);
        assert_eq!(payload.sumeragi.block_cadence_ms, expected_block_cadence_ms);
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
        let tx_hash = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(
            [0x74; Hash::LENGTH],
        ));
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
        let tx_hash = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(
            [0x75; Hash::LENGTH],
        ));
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
        let keypair = checked_torii_test_keypair_from_seed_byte(
            0xd8,
            Algorithm::Ed25519,
            "derive live pending pipeline-status fixture key",
        );
        let authority = AccountId::new(keypair.public_key().clone());
        let transaction = TransactionBuilder::new(
            (*app.chain_id).clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "pipeline-status-live-pending".to_string(),
        )])
        .sign(keypair.private_key());
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
            app.queue.contains_pending_hash(tx_hash.clone(), &app.state),
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
        let tx_hash = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(
            [0x75; Hash::LENGTH],
        ));
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
        let tx_hash = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(
            [0x71; Hash::LENGTH],
        ));
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
        let tx_hash = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(
            [0x72; Hash::LENGTH],
        ));
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

        let resp = super::handler_pipeline_transaction_status(
            State(app),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            None,
            crate::NoritoStringQuery(PipelineStatusQuery {
                hash: Some(tx_hash.to_string()),
                scope: Some("local".to_owned()),
            }),
        )
        .await
        .expect("cached pipeline status should stay available under tx-ingress pressure");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn pipeline_status_handler_cache_hit_ignores_pipeline_status_rate_limiter_pressure() {
        let mut app = mk_app_state_for_tests();
        let tx_hash = HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(
            [0x73; Hash::LENGTH],
        ));
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
        assert!(
            limits::allow_conditionally(&app.pipeline_status_rate_limiter, &rate_key, true).await
        );
        assert!(
            !limits::allow_conditionally(&app.pipeline_status_rate_limiter, &rate_key, true).await
        );

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
        .expect("cached pipeline status should bypass the pipeline-status limiter");
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn account_get_handler_supports_json_and_norito() {
        use iroha_torii_shared::AccountReadResponse;

        let keypair =
            checked_torii_test_ed25519_keypair(0x2a, "derive Torii account-get fixture key");
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
            json_resp
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let json_bytes = axum::body::to_bytes(json_resp.into_body(), usize::MAX)
            .await
            .expect("json body");
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
            norito_resp
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some(crate::utils::NORITO_MIME_TYPE)
        );
        let norito_bytes = axum::body::to_bytes(norito_resp.into_body(), usize::MAX)
            .await
            .expect("norito body");
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
        let keypair = checked_torii_test_ed25519_keypair(
            0x2c,
            "derive Torii routed account-read fixture key",
        );
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
            response
                .headers()
                .get("x-iroha-routed-by")
                .and_then(|value| value.to_str().ok()),
            Some("proxy"),
            "mixed local/proxy fanout should report proxy routing",
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("account read body");
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
        let asset_definition_id = AssetDefinitionId::new(
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
        let external =
            axum::extract::ConnectInfo(std::net::SocketAddr::from(([203, 0, 113, 8], 443)));

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
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
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

        Arc::get_mut(&mut app).expect("unique app state").allow_nets =
            Arc::new(crate::limits::parse_cidrs(&["203.0.113.0/24".to_owned()]));
        assert!(super::trusted_internal_read_source(
            app.as_ref(),
            &HeaderMap::new(),
            external.0.ip(),
        ));
        assert!(super::trusted_internal_read_source(
            app.as_ref(),
            &HeaderMap::new(),
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
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
        let json_body = axum::body::to_bytes(json_response.into_body(), usize::MAX)
            .await
            .expect("JSON account body");
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
        let norito_body = axum::body::to_bytes(norito_response.into_body(), usize::MAX)
            .await
            .expect("Norito account body");
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

        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "rose".parse().expect("asset name"),
        );
        let definition_literal = asset_definition_id.to_string();
        assert!(super::parse_exact_asset_definition_id_literal(&definition_literal).is_ok());
        assert!(
            super::parse_exact_asset_definition_id_literal(&format!("{definition_literal} "))
                .is_err(),
            "asset-definition whitespace normalization is forbidden",
        );
        for scope in ["global", "dataspace:0", "dataspace:10"] {
            assert!(super::parse_exact_asset_balance_scope_literal(scope).is_ok());
            assert!(
                super::parse_exact_internal_asset_scope_query(Some(&format!("scope={scope}")))
                    .is_ok()
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
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("committed transaction JSON body");
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
        let norito_body = axum::body::to_bytes(norito_response.into_body(), usize::MAX)
            .await
            .expect("committed transaction Norito body");
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
        let authority = checked_torii_test_account_id(
            0x2e,
            "derive trusted internal asset authority fixture key",
        );
        let unrelated = checked_torii_test_account_id(
            0x2f,
            "derive trusted internal asset unrelated-account fixture key",
        );
        let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(&authority);
        let asset_definition_id =
            AssetDefinitionId::new(domain_id, "rose".parse().expect("asset name"));
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("rose".to_owned())
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
        let json_body = axum::body::to_bytes(json_response.into_body(), usize::MAX)
            .await
            .expect("asset JSON body");
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
        let norito_body = axum::body::to_bytes(norito_response.into_body(), usize::MAX)
            .await
            .expect("asset Norito body");
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
            missing_json_response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("not_found"),
        );
        let missing_json_body = axum::body::to_bytes(missing_json_response.into_body(), usize::MAX)
            .await
            .expect("missing asset JSON body");
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
        let conflict = match super::merged_trusted_internal_read_response(
            vec![expected, conflicting],
            ResponseFormat::Json,
            "local",
        ) {
            Ok(_) => panic!("conflicting route payloads must fail closed"),
            Err(response) => response,
        };
        assert_eq!(conflict.status(), StatusCode::CONFLICT);
        assert_eq!(
            conflict
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_conflict"),
        );
        assert_eq!(
            conflict.headers().get(axum::http::header::CONTENT_TYPE),
            Some(&HeaderValue::from_static("application/json")),
        );
        let conflict_body = axum::body::to_bytes(conflict.into_body(), usize::MAX)
            .await
            .expect("route-conflict JSON body");
        let conflict_envelope: ErrorEnvelope =
            norito::json::from_slice(&conflict_body).expect("typed route-conflict JSON");
        assert_eq!(conflict_envelope.code, "route_conflict");
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn account_read_for_routes_prefers_not_found_over_route_unavailable_when_missing() {
        let missing = checked_torii_test_account_id(
            0x2d,
            "derive Torii missing routed account-read fixture key",
        );
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
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
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

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_unavailable")
        );
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
    fn trigger_completion_query_honors_explicit_from_height() {
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
        assert_eq!(explicit_history.from_height, 1);
        assert_eq!(explicit_history.scanned_blocks, 4);
        assert_eq!(explicit_history.completions.len(), 1);
        assert_eq!(
            explicit_history.completions[0].completion.trigger_id,
            sample.trigger_id.to_string()
        );
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
        let tx_hashes: HashSet<_> = [tx_hash].into_iter().collect();
        state_block.transactions.insert_block(tx_hashes, height_nz);
        state_block.commit().expect("commit");

        let resp = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            None,
            crate::NoritoStringQuery(PipelineStatusQuery {
                hash: Some(tx_hash.to_string()),
                scope: None,
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: norito::json::Value = norito::json::from_slice(&bytes).expect("json");
        let status_kind = payload
            .get("status")
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str);
        assert_eq!(status_kind, Some("Applied"));

        let resp_entry = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            None,
            crate::NoritoStringQuery(PipelineStatusQuery {
                hash: Some(tx_entry_hash.to_string()),
                scope: None,
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp_entry.status(), StatusCode::OK);
        let bytes_entry = axum::body::to_bytes(resp_entry.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload_entry: norito::json::Value =
            norito::json::from_slice(&bytes_entry).expect("json");
        let status_kind_entry = payload_entry
            .get("status")
            .and_then(|status| status.get("kind"))
            .and_then(norito::json::Value::as_str);
        assert_eq!(status_kind_entry, Some("Applied"));
    }

    #[tokio::test]
    async fn pipeline_status_hydrates_reconstructed_trigger_completion_for_entrypoint_hash() {
        let app = mk_app_state_for_tests();
        let sample = make_persisted_data_trigger_completion_block(1, None);
        let header = sample.block.header();
        let height = header.height();
        let height_usize = usize::try_from(height.get()).expect("height usize");
        let height_nz = NonZeroUsize::new(height_usize).expect("height");
        let block_hash = store_block(&app, sample.block);
        record_committed_block_hash_for_test(&app, header.clone(), block_hash);

        let mut state_block = app.state.block(header);
        let tx_hashes: HashSet<_> = [sample.tx_hash].into_iter().collect();
        state_block.transactions.insert_block(tx_hashes, height_nz);
        state_block.commit().expect("commit");

        let resp = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            None,
            crate::NoritoStringQuery(PipelineStatusQuery {
                hash: Some(sample.tx_hash.to_string()),
                scope: None,
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: PipelineTransactionStatusResponse =
            norito::json::from_slice(&bytes).expect("json");

        assert_eq!(payload.status.kind, "Applied");
        assert_eq!(payload.trigger_completions.len(), 1);
        let completion = payload.trigger_completions.first().expect("completion");
        assert_eq!(completion.trigger_id, sample.trigger_id.to_string());
        assert_eq!(
            completion.trigger_execution_hash,
            sample.entrypoint_hash.to_string()
        );
    }

    #[tokio::test]
    async fn pipeline_status_handler_returns_applied_for_sealed_reveal_entrypoint_hash() {
        let app = mk_app_state_for_tests();
        let (block, reveal_entry_hash) = make_sealed_reveal_block(1, None);
        let header = block.header();
        store_block(&app, block);

        let height = header.height();
        let height_usize = usize::try_from(height.get()).expect("height usize");
        let height_nz = NonZeroUsize::new(height_usize).expect("height");
        let reveal_status_hash =
            HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::from(reveal_entry_hash));
        let mut state_block = app.state.block(header);
        let tx_hashes: HashSet<_> = [reveal_status_hash].into_iter().collect();
        state_block.transactions.insert_block(tx_hashes, height_nz);
        state_block.commit().expect("commit");

        let resp = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            None,
            crate::NoritoStringQuery(PipelineStatusQuery {
                hash: Some(reveal_status_hash.to_string()),
                scope: None,
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: norito::json::Value = norito::json::from_slice(&bytes).expect("json");
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
        store_block(&app, block);

        app.pipeline_status_cache.record_entry(
            tx_hash,
            PipelineStatusEntry::fresh(PipelineStatusKind::Queued, None, None),
        );

        let height = header.height();
        let height_usize = usize::try_from(height.get()).expect("height usize");
        let height_nz = NonZeroUsize::new(height_usize).expect("height");
        let mut state_block = app.state.block(header);
        let tx_hashes: HashSet<_> = [tx_hash].into_iter().collect();
        state_block.transactions.insert_block(tx_hashes, height_nz);
        state_block.commit().expect("commit");

        let resp = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            None,
            crate::NoritoStringQuery(PipelineStatusQuery {
                hash: Some(tx_hash.to_string()),
                scope: None,
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: norito::json::Value = norito::json::from_slice(&bytes).expect("json");
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
        store_block(&app, block);

        let rejection = TransactionRejectionReason::Validation(ValidationFail::TooComplex);
        app.pipeline_status_cache.record_entry(
            tx_hash,
            PipelineStatusEntry::fresh(PipelineStatusKind::Rejected, None, Some(rejection)),
        );

        let height = header.height();
        let height_usize = usize::try_from(height.get()).expect("height usize");
        let height_nz = NonZeroUsize::new(height_usize).expect("height");
        let mut state_block = app.state.block(header);
        let tx_hashes: HashSet<_> = [tx_hash].into_iter().collect();
        state_block.transactions.insert_block(tx_hashes, height_nz);
        state_block.commit().expect("commit");

        let resp = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            None,
            crate::NoritoStringQuery(PipelineStatusQuery {
                hash: Some(tx_hash.to_string()),
                scope: None,
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: norito::json::Value = norito::json::from_slice(&bytes).expect("json");
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
    async fn pipeline_status_handler_encodes_rejection_as_base64() {
        let app = mk_app_state_for_tests();
        let (block, _) = make_signed_block(1, None);
        let tx_hash = block.external_transactions().next().expect("tx").hash();
        let reason = TransactionRejectionReason::Validation(ValidationFail::TooComplex);
        app.pipeline_status_cache.record_entry(
            tx_hash,
            PipelineStatusEntry::fresh(PipelineStatusKind::Rejected, None, Some(reason.clone())),
        );

        let resp = super::handler_pipeline_transaction_status(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            None,
            crate::NoritoStringQuery(PipelineStatusQuery {
                hash: Some(tx_hash.to_string()),
                scope: None,
            }),
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: norito::json::Value = norito::json::from_slice(&bytes).expect("json");
        let rejection_payload = payload
            .get("status")
            .and_then(|status| status.get("rejection_reason"))
            .cloned()
            .expect("rejection content");
        let expected = norito::json::to_value(&reason).expect("rejection json");
        assert_eq!(rejection_payload, expected);
    }

    fn sample_commit_qc(
        chain_id: &ChainId,
        block_hash: HashOf<BlockHeader>,
        post_state_root: iroha_crypto::Hash,
        height: u64,
        view: u64,
        epoch: u64,
    ) -> (Qc, Vec<u8>) {
        let parent_state_root = iroha_crypto::Hash::prehashed([0x11; 32]);
        let keypair = checked_torii_test_keypair_from_seed_byte(
            0x2f,
            Algorithm::BlsNormal,
            "derive Torii commit-QC fixture key",
        );
        let vote = Vote {
            phase: Phase::Commit,
            block_hash,
            parent_state_root,
            post_state_root,
            height,
            view,
            epoch,
            chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let preimage = vote_preimage(chain_id, PERMISSIONED_TAG, &vote);
        let signature =
            checked_torii_test_signature(&keypair, &preimage, "sign Torii commit-QC fixture vote");
        let sig_bytes = signature.payload().to_vec();
        let sig_refs = vec![sig_bytes.as_slice()];
        let aggregate_signature =
            iroha_crypto::bls_normal_aggregate_signatures(&sig_refs).expect("aggregate signatures");
        let validator_pop = iroha_crypto::bls_normal_pop_prove(keypair.private_key())
            .expect("generate validator pop");

        let peer_id = PeerId::from(keypair.public_key().clone());
        let validator_set = vec![peer_id];
        (
            Qc {
                phase: Phase::Commit,
                subject_block_hash: block_hash,
                parent_state_root,
                post_state_root,
                height,
                view,
                epoch,
                chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
                rechain_seq: 0,
                mode_tag: PERMISSIONED_TAG.to_string(),
                highest_qc: None,
                validator_set_hash: HashOf::new(&validator_set),
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set,
                aggregate: QcAggregate {
                    signers_bitmap: vec![0b0000_0001],
                    bls_aggregate_signature: aggregate_signature,
                },
            },
            validator_pop,
        )
    }

    fn record_commit_cert(height: u64) -> Qc {
        let chain_id: ChainId = "chain".parse().expect("chain id");
        let keypair = checked_torii_test_keypair_from_seed_byte(
            0x30,
            Algorithm::BlsNormal,
            "derive Torii recorded commit-cert fixture key",
        );
        let peer_id = PeerId::from(keypair.public_key().clone());
        let block_hash = HashOf::from_untyped_unchecked(Hash::prehashed([height as u8; 32]));
        let parent_state_root = iroha_crypto::Hash::prehashed([0x22; 32]);
        let post_state_root = iroha_crypto::Hash::prehashed([0x33; 32]);
        let vote = Vote {
            phase: Phase::Commit,
            block_hash,
            parent_state_root,
            post_state_root,
            height,
            view: 0,
            epoch: 0,
            chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let preimage = vote_preimage(&chain_id, PERMISSIONED_TAG, &vote);
        let signature = checked_torii_test_signature(
            &keypair,
            &preimage,
            "sign Torii recorded commit-cert fixture vote",
        );
        let cert = Qc {
            phase: Phase::Commit,
            height,
            subject_block_hash: block_hash,
            parent_state_root,
            post_state_root,
            view: 0,
            epoch: 0,
            chain_order_hash: iroha_data_model::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&vec![peer_id.clone()]),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: vec![peer_id],
            aggregate: QcAggregate {
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: signature.payload().to_vec(),
            },
        };
        record_commit_qc_for_tests(cert.clone());
        cert
    }

    #[tokio::test]
    async fn ledger_headers_respect_from_and_limit() {
        let app = mk_app_state_for_tests();
        let (block1, _) = make_signed_block(1, None);
        let first_hash = store_block(&app, block1);
        let (block2, _) = make_signed_block(2, Some(first_hash));
        store_block(&app, block2);

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
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body bytes");
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
        let norito_bytes = axum::body::to_bytes(norito_resp.into_body(), usize::MAX)
            .await
            .expect("norito body");
        let archived = norito::from_bytes::<Vec<BlockHeader>>(&norito_bytes).expect("archive");
        let decoded: Vec<BlockHeader> = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].height().get(), 2);
        assert_eq!(decoded[1].height().get(), 1);
    }

    #[tokio::test]
    async fn commit_qc_window_clamped() {
        let high = 10_000;
        let latest = record_commit_cert(high + 1);
        let older = record_commit_cert(high);

        let resp = handle_v1_sumeragi_commit_qcs(
            crate::NoritoQuery(routing::HistoryWindowQuery {
                from: Some(high + 1),
                limit: Some(1),
            }),
            None,
        )
        .await
        .expect("ok");
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("bytes");
        let certs: Vec<Qc> = norito::json::from_slice(&bytes).expect("decode certs json");
        assert_eq!(certs.len(), 1);
        assert_eq!(certs[0].height, latest.height);

        let norito_resp = handle_v1_sumeragi_commit_qcs(
            crate::NoritoQuery(routing::HistoryWindowQuery {
                from: Some(high + 1),
                limit: Some(2),
            }),
            Some(HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE)),
        )
        .await
        .expect("ok");
        let norito_bytes = axum::body::to_bytes(norito_resp.into_body(), usize::MAX)
            .await
            .expect("bytes");
        let archived = norito::from_bytes::<Vec<Qc>>(&norito_bytes).expect("arch");
        let decoded: Vec<Qc> = norito::core::NoritoDeserialize::deserialize(archived);
        assert!(decoded.iter().any(|c| c.height == latest.height));
        assert!(decoded.iter().any(|c| c.height == older.height));
    }

    #[tokio::test]
    async fn validator_set_history_returns_snapshots() {
        let high = 5;
        let latest = record_commit_cert(high + 1);
        let older = record_commit_cert(high);

        let resp = routing::handle_v1_sumeragi_validator_sets(
            crate::NoritoQuery(routing::HistoryWindowQuery {
                from: Some(high + 1),
                limit: Some(2),
            }),
            None,
        )
        .await
        .expect("ok");
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("json");
        let sets: Vec<routing::ValidatorSetSnapshot> =
            norito::json::from_slice(&bytes).expect("decode json");
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].height, latest.height);
        assert_eq!(sets[1].height, older.height);

        let norito_resp = routing::handle_v1_sumeragi_validator_sets(
            crate::NoritoQuery(routing::HistoryWindowQuery {
                from: Some(high + 1),
                limit: Some(1),
            }),
            Some(HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE)),
        )
        .await
        .expect("ok");
        let norito_bytes = axum::body::to_bytes(norito_resp.into_body(), usize::MAX)
            .await
            .expect("bytes");
        let archived =
            norito::from_bytes::<Vec<routing::ValidatorSetSnapshot>>(&norito_bytes).expect("arch");
        let decoded: Vec<routing::ValidatorSetSnapshot> =
            norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.len(), 1);
        assert_eq!(decoded[0].height, latest.height);
        assert_eq!(decoded[0].validator_set_hash, latest.validator_set_hash);
    }

    #[tokio::test]
    async fn validator_set_by_height_returns_exact_match() {
        let high = 20;
        record_commit_cert(high);
        let wanted = record_commit_cert(high + 1);

        let resp = routing::handle_v1_sumeragi_validator_set_by_height(
            axum::extract::Path(high + 1),
            Some(HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE)),
        )
        .await
        .expect("ok");
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("norito body");
        let archived =
            norito::from_bytes::<routing::ValidatorSetSnapshot>(&bytes).expect("archive");
        let decoded: routing::ValidatorSetSnapshot =
            norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.height, wanted.height);
        assert_eq!(decoded.block_hash, wanted.subject_block_hash);
    }

    #[tokio::test]
    async fn ledger_state_root_uses_result_merkle_root_when_no_commit_qc() {
        let app = mk_app_state_for_tests();
        let (mut block, _) = make_signed_block(1, None);
        let entry_hashes = [block
            .payload()
            .transactions
            .first()
            .expect("tx")
            .hash_as_entrypoint()];
        block
            .set_transaction_results(
                Vec::new(),
                &entry_hashes,
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("test block entrypoint hash should match payload");
        let result_root = block
            .header()
            .result_merkle_root()
            .map(|hash| iroha_crypto::Hash::prehashed(*hash.as_ref()))
            .expect("result root");
        let block_hash = block.hash();
        store_block(&app, block);

        let resp =
            handler_ledger_state_root(State(app.clone()), axum::extract::Path(1), HeaderMap::new())
                .await
                .expect("ok");
        assert_eq!(
            resp.headers()
                .get(axum::http::header::CONTENT_TYPE)
                .map(HeaderValue::as_bytes),
            Some(b"application/json".as_slice())
        );
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("json body");
        let payload: StateRootResponse = norito::json::from_slice(&bytes).expect("decode json");
        assert_eq!(payload.height, 1);
        assert_eq!(payload.block_hash, block_hash);
        assert_eq!(payload.state_root, result_root);
        assert_eq!(payload.source, "result_merkle_root");

        let mut accept = HeaderMap::new();
        accept.insert(
            axum::http::header::ACCEPT,
            HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
        );
        let norito_resp = handler_ledger_state_root(State(app), axum::extract::Path(1), accept)
            .await
            .expect("ok");
        let norito_bytes = axum::body::to_bytes(norito_resp.into_body(), usize::MAX)
            .await
            .expect("norito body");
        let archived = norito::from_bytes::<StateRootResponse>(&norito_bytes).expect("archive");
        let decoded: StateRootResponse = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.state_root, result_root);
        assert_eq!(decoded.source, "result_merkle_root");
    }

    #[tokio::test]
    async fn ledger_state_proof_returns_commit_qc() {
        let app = mk_app_state_for_tests();
        let (mut block, _) = make_signed_block(1, None);
        let entry_hashes = [block
            .payload()
            .transactions
            .first()
            .expect("tx")
            .hash_as_entrypoint()];
        block
            .set_transaction_results(
                Vec::new(),
                &entry_hashes,
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("test block entrypoint hash should match payload");
        let expected_root = block
            .header()
            .result_merkle_root()
            .map(|hash| iroha_crypto::Hash::prehashed(*hash.as_ref()))
            .expect("result root");
        let block_hash = block.hash();
        store_block(&app, block);

        let (qc, _) =
            sample_commit_qc(app.state.chain_id_ref(), block_hash, expected_root, 1, 2, 0);
        let mut app = app;
        let app_mut = Arc::get_mut(&mut app).expect("unique app state for test");
        Arc::get_mut(&mut app_mut.state)
            .expect("unique core state for test")
            .insert_commit_qc_for_testing(block_hash, qc.clone());

        let resp = handler_ledger_state_proof(
            State(app.clone()),
            axum::extract::Path(1),
            HeaderMap::new(),
        )
        .await
        .expect("ok");
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let proof: StateProofResponse = norito::json::from_slice(&body).expect("json decode");
        assert_eq!(proof.height, 1);
        assert_eq!(proof.block_hash, block_hash);
        assert_eq!(proof.state_root, expected_root);
        assert_eq!(proof.commit_qc.subject_block_hash, block_hash);
        assert_eq!(proof.commit_qc.post_state_root, expected_root);
        assert_eq!(
            proof.commit_qc.aggregate.signers_bitmap,
            qc.aggregate.signers_bitmap
        );
        assert_eq!(
            proof.commit_qc.aggregate.bls_aggregate_signature,
            qc.aggregate.bls_aggregate_signature
        );

        let mut accept = HeaderMap::new();
        accept.insert(
            axum::http::header::ACCEPT,
            HeaderValue::from_static(crate::utils::NORITO_MIME_TYPE),
        );
        let norito_resp = handler_ledger_state_proof(State(app), axum::extract::Path(1), accept)
            .await
            .expect("ok");
        let norito_bytes = axum::body::to_bytes(norito_resp.into_body(), usize::MAX)
            .await
            .expect("bytes");
        let archived = norito::from_bytes::<StateProofResponse>(&norito_bytes).expect("arch");
        let decoded: StateProofResponse = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(decoded.commit_qc.post_state_root, expected_root);
    }

    #[tokio::test]
    async fn state_proof_http_roundtrip_supports_json_and_norito() {
        use axum::{
            Router,
            body::Body,
            http::{Request, StatusCode},
            routing::get,
        };
        use http_body_util::BodyExt as _;
        use tower::ServiceExt as _;

        let app = mk_app_state_for_tests();
        let (mut block, _) = make_signed_block(1, None);
        let entry_hashes = [block
            .payload()
            .transactions
            .first()
            .expect("tx")
            .hash_as_entrypoint()];
        block
            .set_transaction_results(
                Vec::new(),
                &entry_hashes,
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("test block entrypoint hash should match payload");
        let expected_root = block
            .header()
            .result_merkle_root()
            .map(|hash| iroha_crypto::Hash::prehashed(*hash.as_ref()))
            .expect("result root");
        let block_hash = block.hash();
        store_block(&app, block);

        let (qc, _) =
            sample_commit_qc(app.state.chain_id_ref(), block_hash, expected_root, 1, 2, 0);
        let mut app = Arc::into_inner(app).unwrap_or_else(|| panic!("unique app state for test"));
        let mut state =
            Arc::into_inner(app.state).unwrap_or_else(|| panic!("unique core state for test"));
        state.insert_commit_qc_for_testing(block_hash, qc.clone());
        app.state = Arc::new(state);
        let app: SharedAppState = Arc::new(app);

        let router = Router::new()
            .route(uri::LEDGER_STATE_PROOF, get(handler_ledger_state_proof))
            .with_state(app.clone());

        let request = Request::builder()
            .uri("/v1/ledger/state-proof/1")
            .body(Body::empty())
            .expect("request");
        let response = router.clone().oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let proof: StateProofResponse = norito::json::from_slice(&bytes).expect("json decode");
        assert_eq!(proof.height, 1);
        assert_eq!(proof.block_hash, block_hash);
        assert_eq!(proof.state_root, expected_root);
        assert_eq!(
            proof.commit_qc.aggregate.signers_bitmap,
            qc.aggregate.signers_bitmap
        );
        assert_eq!(
            proof.commit_qc.aggregate.bls_aggregate_signature,
            qc.aggregate.bls_aggregate_signature
        );

        let request = Request::builder()
            .uri("/v1/ledger/state-proof/1")
            .header(axum::http::header::ACCEPT, crate::utils::NORITO_MIME_TYPE)
            .body(Body::empty())
            .expect("request");
        let response = router.oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let archived =
            norito::from_bytes::<StateProofResponse>(&bytes).expect("archived state proof");
        let proof: StateProofResponse = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(proof.height, 1);
        assert_eq!(proof.block_hash, block_hash);
        assert_eq!(proof.state_root, expected_root);
        assert_eq!(proof.commit_qc.post_state_root, expected_root);
    }

    #[tokio::test]
    async fn block_proof_handler_emits_norito() {
        let app = mk_app_state_for_tests();
        let (block, entry_hash) = make_signed_block(1, None);
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
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("norito payload");
        let archived = norito::from_bytes::<BlockProofs>(&bytes).expect("archive decode");
        let proofs: BlockProofs = norito::core::NoritoDeserialize::deserialize(archived);
        assert_eq!(proofs.block_height.get(), 1);
        assert_eq!(proofs.entry_hash, entry_hash);
    }

    fn app_with_indexed_sccp_message_for_test(
        persist_finality: bool,
    ) -> (SharedAppState, [u8; 32], V2FinalityArtifact) {
        const HEIGHT: u64 = 1;
        let keypair = checked_torii_test_ed25519_keypair(
            0x31,
            "derive indexed Torii SCCP-message fixture key",
        );
        let chain: ChainId = iroha_sccp::SCCP_TAIRA_FINALITY_CHAIN_ID_V1
            .parse()
            .expect("SCCP Taira finality chain id");
        let app = mk_app_state_for_tests_with_chain_id(chain.clone());
        let authority = AccountId::new(keypair.public_key().clone());
        let payload = iroha_sccp::SccpPayloadV1::Transfer(iroha_sccp::TransferPayloadV1 {
            version: 1,
            source_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            dest_domain: iroha_sccp::SCCP_DOMAIN_ETH,
            nonce: 7,
            route_revision: 1,
            asset_home_domain: iroha_sccp::SCCP_DOMAIN_SORA,
            asset_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            asset_id: iroha_sccp::SCCP_TAIRA_XOR_ASSET_KEY_V1.as_bytes().to_vec(),
            amount: 123,
            sender_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            sender: b"alice".to_vec(),
            recipient_codec: iroha_sccp::SCCP_CODEC_EVM_ADDRESS20,
            recipient: vec![0x91; 20],
            route_id_codec: iroha_sccp::SCCP_CODEC_CANONICAL_TEXT,
            route_id: iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1
                .as_bytes()
                .to_vec(),
        });
        let context = iroha_data_model::bridge::SccpOutboundMessageContextV1::new(
            iroha_data_model::bridge::SccpLaneIdV1 {
                source: iroha_data_model::bridge::SccpNetworkV1::SoraTaira,
                target: iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
            },
            [0xd1; 32],
            [0xc1; 32],
        )
        .expect("well-formed SCCP context");
        let record = iroha_data_model::isi::bridge::RecordSccpMessage::new(
            context,
            iroha_sccp::canonical_sccp_payload_bytes(&payload)
                .expect("valid SCCP indexed-message fixture payload encodes"),
        );
        let tx = checked_torii_test_transaction(
            TransactionBuilder::new(
                chain,
                authority,
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([record]),
            &keypair,
            "sign indexed Torii SCCP-message fixture transaction",
        );
        let entry_hash = tx.hash_as_entrypoint();
        let header = BlockHeader::new(
            std::num::NonZeroU64::new(HEIGHT).expect("nonzero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let signature = checked_torii_test_block_signature(
            0,
            &keypair,
            &header,
            "sign indexed Torii SCCP-message fixture block",
        );
        let mut block = SignedBlock::presigned(signature, header, vec![tx]);
        block
            .set_transaction_results(
                Vec::new(),
                &[entry_hash],
                vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
            )
            .expect("test block entrypoint hash should match payload");
        let legacy_post_state_root = block
            .header()
            .result_merkle_root()
            .map(|hash| iroha_crypto::Hash::prehashed(*hash.as_ref()))
            .expect("SCCP fixture result root");
        let messages = iroha_core::bridge::collect_sccp_messages_from_signed_block(&block);
        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        let commitment_root = iroha_core::bridge::sccp_commitment_root_from_messages(&messages)
            .expect("SCCP commitment root");
        block.set_sccp_commitment_root(Some(commitment_root));
        let block_hash = block.hash();
        let message_id = message.commitment.message_id;
        let key = iroha_data_model::bridge::SccpOutboundMessageKeyV1::new(context.lane, message_id)
            .expect("valid outbound key");
        let durable = iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1 {
            destination_binding_hash: context.destination_binding_hash,
            route_configuration_hash: context.route_configuration_hash,
            payload_hash: message.commitment.payload_hash,
            payload_bytes: iroha_sccp::canonical_sccp_payload_bytes(&message.payload)
                .expect("canonical indexed Torii SCCP-message payload"),
            recorded_at_height: HEIGHT,
            commitment_index: 0,
        };
        app.state
            .insert_sccp_outbound_message_for_testing(key, durable)
            .expect("insert indexed outbound record");
        let mut validator_keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("derive deterministic SCCP finality validator")
            })
            .collect::<Vec<_>>();
        validator_keys.sort_by(|left, right| {
            PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
        });
        let roster = validator_keys
            .iter()
            .zip([1_u64; 4])
            .map(|(key, power)| ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power,
            })
            .collect::<Vec<_>>();
        let context = HeightContext {
            chain_id: app.state.chain_id_ref().clone(),
            protocol_version: PROTOCOL_VERSION,
            height: HEIGHT,
            epoch: 0,
            epoch_end_height: 10,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Npos,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: DualQuorum::from_roster(&roster).expect("valid SCCP finality roster"),
            roster,
            nexus_amx_context_hash: Hash::new(b"Torii SCCP exact-v2 finality context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: DataAvailabilityLayout {
                encoding: PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0x42; 32],
        };
        let subject = BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash,
            payload_hash: block
                .canonical_proposal_wire_hash()
                .expect("hash exact SCCP fixture proposal wire"),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height: HEIGHT,
            view: block.header().view_change_index(),
        };
        let mut commit_qc = QuorumCertificate {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment: ExecutionCommitment::without_topups(
                Hash::new(b"Torii SCCP exact-v2 parent state"),
                Hash::new(b"Torii SCCP exact-v2 post state"),
                Hash::new(b"Torii SCCP exact-v2 ordinary writes"),
                block
                    .executed_block_wire_hash()
                    .expect("hash exact SCCP fixture block wire"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        };
        let preimage = commit_qc
            .signer_preimage(&context, 0)
            .expect("valid SCCP finality signer");
        let signatures = commit_qc
            .signers
            .iter()
            .map(|index| {
                Signature::try_new(
                    validator_keys[usize::try_from(*index).expect("fixture signer index")]
                        .private_key(),
                    &preimage,
                )
                .expect("sign exact SCCP Commit vote")
                .payload()
                .to_vec()
            })
            .collect::<Vec<_>>();
        commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
            &signatures.iter().map(Vec::as_slice).collect::<Vec<_>>(),
        )
        .expect("aggregate exact SCCP Commit votes");
        let validator_set_pops = validator_keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("derive SCCP finality validator PoP")
            })
            .collect();
        let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
        artifact
            .validate_for_header(&block.header())
            .expect("SCCP finality fixture binds the exact block header");
        artifact
            .verify()
            .expect("SCCP finality fixture is cryptographically valid");

        // Seed the retired QC model as an adversarial control. Proof routes
        // must still require the exact durable v2 artifact below; a valid
        // legacy QC in world state is not an alternative finality source.
        let (legacy_qc, legacy_validator_pop) = sample_commit_qc(
            app.state.chain_id_ref(),
            block_hash,
            legacy_post_state_root,
            HEIGHT,
            HEIGHT.saturating_add(1),
            0,
        );

        let stored_block_hash = store_block(&app, block);
        assert_eq!(stored_block_hash, artifact.block_hash);
        if persist_finality {
            let receipt = app
                .kura
                .store_v2_finality_artifact(&artifact)
                .expect("persist exact SCCP v2 finality artifact");
            assert_eq!(receipt.height(), artifact.height);
            assert_eq!(receipt.block_hash(), artifact.block_hash);
            assert_eq!(receipt.context_id(), artifact.context_id());
            assert_eq!(receipt.subject(), artifact.subject);
            assert_eq!(receipt.certificate(), artifact.commit_qc.as_ref());
            assert_eq!(receipt.artifact_hash(), HashOf::new(&artifact));
        }
        let mut app = app;
        let app_mut = Arc::get_mut(&mut app).expect("unique app state for SCCP fixture");
        let state = Arc::get_mut(&mut app_mut.state).expect("unique core state for SCCP fixture");
        state.world.register_validator_pop_for_testing(
            legacy_qc.validator_set[0].public_key().clone(),
            legacy_validator_pop,
        );
        state.insert_commit_qc_for_testing(block_hash, legacy_qc);
        assert!(
            state.world_view().commit_qcs().get(&block_hash).is_some(),
            "SCCP adversarial fixture retains a valid legacy QC"
        );
        (app, message_id, artifact)
    }

    #[tokio::test]
    async fn sccp_bundle_endpoint_uses_exact_v2_artifact_and_authoritative_index() {
        let (app, message_id, expected_artifact) = app_with_indexed_sccp_message_for_test(true);
        let message_id_hex = hex::encode(message_id);
        let bundle_response = routing::handle_v1_sccp_message_bundle(
            Arc::clone(&app.state),
            message_id_hex.clone(),
            utils::ResponseFormat::Json,
            acquire_query_admission(app.as_ref(), true)
                .await
                .expect("acquire bundle test admission"),
        )
        .await
        .expect("indexed bundle response");
        let bundle_bytes = axum::body::to_bytes(bundle_response.into_body(), usize::MAX)
            .await
            .expect("bundle body");
        let bundle = norito::json::from_slice::<iroha_sccp::TairaSccpMessageProofV1>(&bundle_bytes)
            .expect("typed bundle JSON");
        assert_eq!(bundle.commitment.message_id, message_id);
        assert!(iroha_sccp::verify_message_bundle_structure(&bundle));
        let verified_finality =
            iroha_sccp::verified_sccp_message_taira_finality_proof_cryptographically_self_consistent(
                &bundle,
            )
            .expect("bundle carries a cryptographically self-consistent exact-v2 proof");
        assert_eq!(verified_finality.finality_artifact, expected_artifact);

        let request_error = routing::handle_v1_sccp_proof_request(
            Arc::clone(&app.state),
            message_id_hex,
            utils::ResponseFormat::Json,
            acquire_query_admission(app.as_ref(), true)
                .await
                .expect("acquire proof-request test admission"),
        )
        .await
        .expect_err("proof request must require its historical governed route");
        let Error::Query(ValidationFail::InternalError(message)) = request_error else {
            panic!("unexpected missing-route error: {request_error}");
        };
        assert!(message.contains("retained destination binding"));
    }
