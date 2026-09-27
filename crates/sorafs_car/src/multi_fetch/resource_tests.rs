// Scheduler-level tests exercise quotas, deadlines, typed errors, and consuming backpressure.
fn repeated_chunk_plan(count: usize) -> (CarBuildPlan, Arc<Vec<u8>>) {
    let bytes = Arc::new(vec![0x5a; ChunkProfile::DEFAULT.min_size]);
    let mut hasher = blake3::Hasher::new();
    for _ in 0..count {
        hasher.update(&bytes);
    }
    let plan = CarBuildPlan {
        chunk_profile: ChunkProfile::DEFAULT,
        payload_digest: hasher.finalize(),
        content_length: (bytes.len() * count) as u64,
        chunks: (0..count)
            .map(|index| crate::CarChunk {
                offset: (index * bytes.len()) as u64,
                length: bytes.len() as u32,
                digest: blake3::hash(&bytes).into(),
            })
            .collect(),
        files: vec![crate::FilePlan {
            path: vec!["payload.bin".into()],
            first_chunk: 0,
            chunk_count: count,
            size: (bytes.len() * count) as u64,
        }],
    };
    (plan, bytes)
}

#[tokio::test(start_paused = true)]
async fn scheduler_respects_real_byte_and_request_windows() {
    let (plan, payload) = repeated_chunk_plan(4);
    let mut metadata = ProviderMetadata::new();
    metadata.range_capability = Some(RangeCapability {
        max_chunk_span: payload.len() as u32,
        min_granularity: 1,
        supports_sparse_offsets: true,
        requires_alignment: false,
        supports_merkle_proof: true,
    });
    metadata.stream_budget = Some(StreamBudget {
        max_in_flight: 4,
        max_bytes_per_sec: (payload.len() * 2) as u64,
        burst_bytes: None,
    });
    metadata.requests_per_minute = Some(3);
    let started = tokio::time::Instant::now();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&calls);
    let outcome = fetch_plan_parallel(
        &plan,
        [FetchProvider::new("healthy").with_metadata(metadata)],
        move |_| {
            observed
                .lock()
                .unwrap()
                .push(tokio::time::Instant::now().duration_since(started));
            let bytes = payload.as_ref().clone();
            async move { Ok::<_, TestError>(ChunkResponse::new(bytes)) }
        },
        FetchOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(
        *calls.lock().unwrap(),
        [
            Duration::ZERO,
            Duration::ZERO,
            Duration::from_secs(1),
            Duration::from_secs(60)
        ]
    );
    assert_eq!(outcome.provider_reports[0].failures, 0);
}

#[tokio::test(start_paused = true)]
async fn consuming_window_remains_bounded_behind_a_slow_first_chunk() {
    for count in [16, 1025] {
        let (plan, bytes) = repeated_chunk_plan(count);
        let delivered = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&delivered);
        let chunk_len = bytes.len();
        let options = FetchOptions {
            global_parallel_limit: Some(4),
            max_buffered_bytes: chunk_len * 4,
            ..FetchOptions::default()
        };
        let outcome = fetch_plan_parallel_with_observer(
            &plan,
            [FetchProvider::new("healthy")
                .with_max_concurrent_chunks(NonZeroUsize::new(4).unwrap())],
            move |request| {
                let bytes = bytes.as_ref().clone();
                async move {
                    if request.spec.chunk_index == 0 {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Ok::<_, TestError>(ChunkResponse::new(bytes))
                }
            },
            options,
            move |chunk: ChunkDelivery<'_>| {
                assert_eq!(
                    chunk.chunk_index * chunk_len,
                    observed.fetch_add(chunk.bytes.len(), Ordering::SeqCst)
                );
                Ok(())
            },
        )
        .await
        .unwrap();
        assert_eq!(delivered.load(Ordering::SeqCst) as u64, plan.content_length);
        assert_eq!(outcome.chunk_receipts.len(), count);
        assert_eq!(outcome.peak_buffered_bytes, chunk_len * 4);
    }
}

#[tokio::test]
async fn eager_and_spool_limits_reject_before_dispatch() {
    let (plan, _) = repeated_chunk_plan(1025);
    for options in [
        FetchOptions::default(),
        FetchOptions {
            max_payload_bytes: 1,
            ..FetchOptions::default()
        },
    ] {
        let result = fetch_plan_parallel(
            &plan,
            [FetchProvider::new("unused")],
            |_| async {
                panic!("oversized payload must be rejected before fetching");
                #[allow(unreachable_code)]
                Ok::<_, TestError>(ChunkResponse::new(Vec::new()))
            },
            options,
        )
        .await;
        assert!(matches!(result, Err(MultiSourceError::ResourceLimit(_))));
    }
}

#[tokio::test]
async fn provider_inventory_is_admitted_before_dispatch() {
    let (plan, _) = repeated_chunk_plan(1);
    let error = fetch_plan_parallel(
        &plan,
        (0..257).map(|index| FetchProvider::new(index.to_string())),
        |_| async { Err::<ChunkResponse, TestError>(TestError("unexpected dispatch")) },
        FetchOptions::default(),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        MultiSourceError::ResourceLimit("provider inventory exceeds 256")
    ));
}

#[cfg(feature = "manifest")]
#[tokio::test(start_paused = true)]
async fn repeated_gateway_throttle_preserves_health_and_failure_retry_budget() {
    let (plan, bytes) = repeated_chunk_plan(1);
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let started = tokio::time::Instant::now();
    let outcome = fetch_plan_parallel(
        &plan,
        [FetchProvider::new("healthy")],
        move |_| {
            let call = observed.fetch_add(1, Ordering::SeqCst);
            let bytes = bytes.as_ref().clone();
            async move {
                if call < 4 {
                    Err(crate::gateway::GatewayFetchError::RateLimited {
                        provider: "healthy".into(),
                        retry_after: Duration::from_secs(2),
                    })
                } else {
                    Ok(ChunkResponse::new(bytes))
                }
            }
        },
        FetchOptions {
            per_chunk_retry_limit: Some(1),
            provider_failure_threshold: 1,
            ..FetchOptions::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        tokio::time::Instant::now().duration_since(started),
        Duration::from_secs(8)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 5);
    assert_eq!(outcome.provider_reports[0].failures, 0);
    assert!(!outcome.provider_reports[0].disabled);
    assert_eq!(outcome.chunk_receipts[0].attempts, 1);
}

#[cfg(feature = "manifest")]
#[tokio::test(start_paused = true)]
async fn repeated_throttle_and_hung_transport_stop_at_session_deadline() {
    let (plan, _) = repeated_chunk_plan(1);
    let options = FetchOptions {
        session_timeout: Duration::from_secs(3),
        ..FetchOptions::default()
    };
    let error = fetch_plan_parallel(
        &plan,
        [FetchProvider::new("healthy")],
        |_| async {
            Err::<ChunkResponse, _>(crate::gateway::GatewayFetchError::RateLimited {
                provider: "healthy".into(),
                retry_after: Duration::from_secs(60),
            })
        },
        options.clone(),
    )
    .await
    .unwrap_err();
    assert!(matches!(error, MultiSourceError::DeadlineExceeded));
    let error = fetch_plan_parallel(
        &plan,
        [FetchProvider::new("hung")],
        |_| async { std::future::pending::<Result<ChunkResponse, TestError>>().await },
        options,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, MultiSourceError::DeadlineExceeded));
}

#[cfg(feature = "manifest")]
#[tokio::test]
async fn full_scheduler_preserves_governed_policy_evidence() {
    let (plan, _) = repeated_chunk_plan(1);
    let error = fetch_plan_parallel(
        &plan,
        [FetchProvider::new("policy")],
        |_| async {
            Err::<ChunkResponse, _>(crate::gateway::GatewayFetchError::PolicyBlocked {
                provider: "policy".into(),
                evidence: crate::gateway::GatewayFailureEvidence {
                    observed_status: reqwest::StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS,
                    code: "gateway_compliance_denied".into(),
                    source: "baseline".into(),
                    catalog_digest_hex: "ab".repeat(32),
                },
            })
        },
        FetchOptions {
            per_chunk_retry_limit: Some(1),
            ..FetchOptions::default()
        },
    )
    .await
    .unwrap_err();
    let MultiSourceError::ExhaustedRetries { last_error, .. } = error else {
        panic!("unexpected fetch failure")
    };
    let AttemptFailure::Provider {
        policy_block: Some(evidence),
        ..
    } = last_error.failure
    else {
        panic!("lost structured policy evidence")
    };
    assert_eq!(evidence.source, "baseline");
    assert_eq!(evidence.catalog_digest_hex, "ab".repeat(32));
}

#[test]
fn scheduler_remains_usable_on_a_plain_futures_executor() {
    let (plan, bytes) = repeated_chunk_plan(1);
    let outcome = futures::executor::block_on(fetch_plan_parallel(
        &plan,
        [FetchProvider::new("plain")],
        move |_| {
            let bytes = bytes.as_ref().clone();
            async move { Ok::<_, TestError>(ChunkResponse::new(bytes)) }
        },
        FetchOptions::default(),
    ))
    .unwrap();
    assert_eq!(outcome.chunk_receipts.len(), 1);
}

#[test]
fn plan_admission_bounds_chunks_files_and_nested_path_inventory() {
    let (mut plan, _) = repeated_chunk_plan(4);
    let options = FetchOptions {
        max_metadata_entries: 6,
        ..FetchOptions::default()
    };
    options.validate_plan_limits(&plan).unwrap();
    plan.files[0].path.insert(0, "nested".into());
    assert!(matches!(
        options.validate_plan_limits(&plan),
        Err(MultiSourceError::ResourceLimit(_))
    ));
    plan.files[0].path.remove(0);
    plan.files.push(plan.files[0].clone());
    assert!(matches!(
        options.validate_plan_limits(&plan),
        Err(MultiSourceError::ResourceLimit(_))
    ));
}

#[test]
fn concurrency_burst_does_not_override_single_request_byte_quota() {
    let (plan, bytes) = repeated_chunk_plan(1);
    let mut metadata = ProviderMetadata::new();
    metadata.range_capability = Some(RangeCapability {
        max_chunk_span: bytes.len() as u32,
        min_granularity: 1,
        supports_sparse_offsets: true,
        requires_alignment: false,
        supports_merkle_proof: true,
    });
    metadata.stream_budget = Some(StreamBudget {
        max_in_flight: 1,
        max_bytes_per_sec: bytes.len() as u64 - 1,
        burst_bytes: Some(bytes.len() as u64 * 2),
    });
    assert!(
        provider_can_serve_chunk(
            &FetchProvider::new("limited").with_metadata(metadata),
            &plan.try_chunk_fetch_specs().unwrap()[0]
        )
        .is_err()
    );
}

#[tokio::test]
async fn consuming_sink_failure_and_slow_sink_never_return_success() {
    for slow in [false, true] {
        let (plan, bytes) = repeated_chunk_plan(1);
        let result = fetch_plan_parallel_with_observer(
            &plan,
            [FetchProvider::new("healthy")],
            move |_| {
                let bytes = bytes.as_ref().clone();
                async move { Ok::<_, TestError>(ChunkResponse::new(bytes)) }
            },
            FetchOptions {
                session_timeout: if slow {
                    Duration::from_millis(100)
                } else {
                    Duration::from_secs(30)
                },
                ..FetchOptions::default()
            },
            move |_: ChunkDelivery<'_>| {
                if slow {
                    std::thread::sleep(Duration::from_millis(150));
                    Ok(())
                } else {
                    Err(ObserverError::new("disk full"))
                }
            },
        )
        .await;
        assert!(if slow {
            matches!(result, Err(MultiSourceError::DeadlineExceeded))
        } else {
            matches!(result, Err(MultiSourceError::ObserverFailed { .. }))
        });
    }
}
