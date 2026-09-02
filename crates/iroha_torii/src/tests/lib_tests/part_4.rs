fn verified_zk_ivm_derive_request(
    account: AccountId,
) -> axum::Extension<crate::app_auth::VerifiedCanonicalRequest> {
    let signer = checked_torii_test_ed25519_keypair(
        0xfe,
        "derive verified ZK IVM request signer fixture key",
    )
    .public_key()
    .clone();
    axum::Extension(crate::app_auth::VerifiedCanonicalRequest {
        account,
        signer: signer.clone(),
        verified_signers: vec![signer],
    })
}
#[test]
fn zk_ivm_tooling_advances_the_committed_height_for_execution() {
    assert_eq!(zk_ivm_next_execution_height(0), Ok(1));
    assert_eq!(zk_ivm_next_execution_height(41), Ok(42));
}
#[tokio::test]
async fn configured_proof_body_layer_accepts_above_axum_default_and_rejects_limit_plus_one() {
    let app = mk_app_state_for_tests();
    let router = proof_post_router_with_body_limits(
        Router::<SharedAppState>::new().route(
            "/probe",
            post(|body: Bytes| async move {
                assert!(body.len() > 2 * 1024 * 1024);
                StatusCode::NO_CONTENT
            }),
        ),
        app.clone(),
    )
    .with_state::<()>(app.clone());
    let above_axum_default = axum::http::Request::builder()
        .method("POST")
        .uri("/probe")
        .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(vec![0_u8; 2 * 1024 * 1024 + 1]))
        .expect("request");
    let response = router
        .clone()
        .oneshot(above_axum_default)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let over_configured_limit = axum::http::Request::builder()
        .method("POST")
        .uri("/probe")
        .header(axum::http::header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(vec![0_u8; app.proof_limits.max_body_bytes + 1]))
        .expect("request");
    let response = router
        .oneshot(over_configured_limit)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
#[tokio::test]
async fn zk_ivm_json_endpoints_reject_wrong_mime_and_norito_under_json() {
    let app = mk_app_state_for_tests();
    let mut wrong_mime = HeaderMap::new();
    wrong_mime.insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    let error = match handler_zk_ivm_derive(
        State(app.clone()),
        verified_zk_ivm_derive_request(ALICE_ID.clone()),
        wrong_mime,
        crate::loopback_connect_info(),
        Bytes::from_static(b"{}"),
    )
    .await
    {
        Ok(_) => panic!("wrong MIME must fail before decoding"),
        Err(error) => error,
    };
    assert!(
        query_conversion_message(&error)
            .is_some_and(|message| message.contains("Content-Type: application/json"))
    );
    let norito = norito::to_bytes(&ZkIvmProveJobCreatedDto {
        job_id: "binary-not-json".to_owned(),
    })
    .expect("encode Norito fixture");
    let error = match handler_zk_ivm_derive(
        State(app.clone()),
        verified_zk_ivm_derive_request(ALICE_ID.clone()),
        proof_json_headers(),
        crate::loopback_connect_info(),
        Bytes::from(norito),
    )
    .await
    {
        Ok(_) => panic!("Norito bytes under application/json must not be accepted"),
        Err(error) => error,
    };
    assert!(
        query_conversion_message(&error)
            .is_some_and(|message| message.contains("invalid derive JSON request body"))
    );
}
#[tokio::test]
async fn proof_body_middleware_deadline_rejects_stall_and_releases_admission() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.proof_body_inflight = Arc::new(tokio::sync::Semaphore::new(1));
        state.proof_limits.body_read_timeout = Duration::from_millis(30);
    }
    let router = proof_post_router_with_body_limits(
        Router::<SharedAppState>::new().route(
            "/probe",
            post(|_body: Bytes| async move { StatusCode::NO_CONTENT }),
        ),
        app.clone(),
    )
    .with_state::<()>(app.clone());
    let stalled =
        futures_util::stream::pending::<std::result::Result<Bytes, std::convert::Infallible>>();
    let first_request = axum::http::Request::builder()
        .method("POST")
        .uri("/probe")
        .body(Body::from_stream(stalled))
        .expect("stalled request");
    let first_router = router.clone();
    let first = tokio::spawn(async move {
        first_router
            .oneshot(first_request)
            .await
            .expect("first response")
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while app.proof_body_inflight.available_permits() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("stalled request must acquire admission");
    let second = router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/probe")
                .body(Body::from(Bytes::from_static(b"second")))
                .expect("second request"),
        )
        .await
        .expect("second response");
    assert_eq!(second.status(), StatusCode::TOO_MANY_REQUESTS);
    let first = first.await.expect("first task");
    assert_eq!(first.status(), StatusCode::REQUEST_TIMEOUT);
    let third = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/probe")
                .body(Body::from(Bytes::from_static(b"third")))
                .expect("third request"),
        )
        .await
        .expect("third response");
    assert_eq!(
        third.status(),
        StatusCode::NO_CONTENT,
        "deadline completion must release middleware admission"
    );
}
#[cfg(feature = "app_api")]
#[tokio::test]
async fn verified_source_admission_precedes_body_polling_and_transfers_one_slot() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.verified_source_compile_inflight = Arc::new(tokio::sync::Semaphore::new(1));
        state.verified_source_body_read_timeout = Duration::from_millis(30);
    }
    let router =
        Router::<SharedAppState>::new()
            .route(
                "/verified-source",
                post(
                    |Extension(admission): Extension<VerifiedSourceCompileAdmission>,
                     _body: Bytes| async move {
                        let _permit = admission.take().expect("compiler admission handoff");
                        StatusCode::NO_CONTENT
                    },
                ),
            )
            .layer(axum::middleware::from_fn_with_state(
                app.clone(),
                verified_source_body_admission_middleware,
            ))
            .with_state::<()>(app.clone());
    let stalled =
        futures_util::stream::pending::<std::result::Result<Bytes, std::convert::Infallible>>();
    let first_router = router.clone();
    let first = tokio::spawn(async move {
        first_router
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/verified-source")
                    .body(Body::from_stream(stalled))
                    .expect("stalled request"),
            )
            .await
            .expect("first response")
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while app.verified_source_compile_inflight.available_permits() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the first request must own compiler capacity before polling its body");
    let rejected = router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/verified-source")
                .body(Body::from(Bytes::from_static(b"second")))
                .expect("second request"),
        )
        .await
        .expect("second response");
    assert_eq!(rejected.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        first.await.expect("first task").status(),
        StatusCode::REQUEST_TIMEOUT
    );
    let accepted = router
        .oneshot(
            axum::http::Request::builder()
                .method("POST")
                .uri("/verified-source")
                .body(Body::from(Bytes::from_static(b"third")))
                .expect("third request"),
        )
        .await
        .expect("third response");
    assert_eq!(accepted.status(), StatusCode::NO_CONTENT);
}
#[tokio::test]
async fn proof_body_absolute_deadline_rejects_continuous_trickle() {
    let trickle = futures_util::stream::unfold((), |_| async {
        tokio::time::sleep(Duration::from_millis(5)).await;
        Some((
            Ok::<_, std::convert::Infallible>(Bytes::from_static(b"x")),
            (),
        ))
    });
    let request = axum::http::Request::new(Body::from_stream(trickle));
    let response = collect_proof_body_with_deadline(request, 1024, Duration::from_millis(25))
        .await
        .expect_err("trickle must not reset the absolute deadline");
    assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
}
#[tokio::test]
async fn proof_json_egress_charges_the_exact_serialized_response_bytes() {
    let payload = ZkIvmProveJobCreatedDto {
        job_id: "exact-json-egress".to_owned(),
    };
    let expected = norito::json::to_vec(&payload).expect("encode expected response");
    assert!(expected.len() > 1);
    let mut limited_app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut limited_app).expect("unique app state");
        state.proof_limits.retry_after = std::time::Duration::from_secs(7);
        state.proof_egress_limiter =
            limits::RateLimiter::new_u64(Some(1), Some(expected.len() as u64 - 1));
    }
    let err = proof_json_response_with_egress(
        &limited_app,
        &HeaderMap::new(),
        Some(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        "v1/zk/ivm/prove/{job_id}",
        payload.clone(),
        true,
    )
    .await
    .expect_err("one byte below the encoded response must be throttled");
    let limited_response = err.into_response();
    assert_eq!(limited_response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        limited_response
            .headers()
            .get(axum::http::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok()),
        Some("7")
    );
    let mut exact_app = mk_app_state_for_tests();
    Arc::get_mut(&mut exact_app)
        .expect("unique app state")
        .proof_egress_limiter = limits::RateLimiter::new_u64(Some(1), Some(expected.len() as u64));
    let response = proof_json_response_with_egress(
        &exact_app,
        &HeaderMap::new(),
        Some(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        "v1/zk/ivm/prove/{job_id}",
        payload,
        true,
    )
    .await
    .expect("an exact-byte budget should pass");
    let actual = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("response body")
        .to_bytes();
    assert_eq!(actual.as_ref(), expected.as_slice());
}
#[tokio::test]
async fn buffered_sccp_response_egress_charges_exact_bytes_and_preserves_body() {
    let expected = Bytes::from_static(b"exact-sccp-proof-response");
    let response = || {
        let mut response = AxResponse::new(Body::from(expected.clone()));
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        response
    };
    let remote = Some(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    let mut limited_app = mk_app_state_for_tests();
    Arc::get_mut(&mut limited_app)
        .expect("unique limited app state")
        .proof_egress_limiter = limits::RateLimiter::new_u64(
        Some(1),
        Some(u64::try_from(expected.len()).expect("small body") - 1),
    );
    let error = proof_response_with_exact_egress(
        limited_app.as_ref(),
        &HeaderMap::new(),
        remote,
        "v1/sccp/proofs/message",
        response(),
        true,
    )
    .await
    .expect_err("one byte below the buffered SCCP response must reject");
    assert!(matches!(
        error,
        Error::ProofRateLimited {
            endpoint: "v1/sccp/proofs/message",
            ..
        }
    ));
    let mut exact_app = mk_app_state_for_tests();
    Arc::get_mut(&mut exact_app)
        .expect("unique exact app state")
        .proof_egress_limiter = limits::RateLimiter::new_u64(
        Some(1),
        Some(u64::try_from(expected.len()).expect("small body")),
    );
    let admitted = proof_response_with_exact_egress(
        exact_app.as_ref(),
        &HeaderMap::new(),
        remote,
        "v1/sccp/proofs/message",
        response(),
        true,
    )
    .await
    .expect("exact buffered SCCP response budget must pass");
    assert_eq!(
        admitted
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("application/json")
    );
    let actual = axum::body::to_bytes(admitted.into_body(), usize::MAX)
        .await
        .expect("collect admitted response");
    assert_eq!(actual, expected);
}
#[test]
fn default_proof_egress_burst_covers_worst_case_sccp_hex_expansion() {
    let binary_ceiling = u64::try_from(SCCP_SUBMIT_MAX_TRANSACTION_PAYLOAD_BYTES_V1)
        .expect("SCCP binary ceiling fits u64");
    let json_hex_and_envelope_ceiling = binary_ceiling
        .checked_mul(2)
        .and_then(|bytes| {
            bytes.checked_add(
                u64::try_from(SCCP_SUBMIT_JSON_ENVELOPE_ALLOWANCE_BYTES_V1)
                    .expect("SCCP JSON allowance fits u64"),
            )
        })
        .expect("first-release SCCP response ceiling fits u64");
    let burst = iroha_config::parameters::defaults::torii::PROOF_EGRESS_BURST_BYTES
        .expect("production proof egress shaping is enabled by default");
    assert!(
        burst >= json_hex_and_envelope_ceiling,
        "default proof egress burst must admit one maximum SCCP response"
    );
}
#[tokio::test]
async fn zk_ivm_prove_get_enforces_response_egress_with_retry_after() {
    let mut app = mk_ivm_prove_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.proof_limits.retry_after = std::time::Duration::from_secs(5);
        state.proof_egress_limiter = limits::RateLimiter::new_u64(Some(1), Some(1));
    }
    let job_id = "0123456789abcdef0123456789abcdef".to_owned();
    let (cancel, _cancel_rx) = tokio::sync::watch::channel(false);
    let response_body = zk_ivm_prove_job_response_body(
        job_id.clone(),
        ZkIvmProveJobStatus::Pending,
        None,
        None,
        None,
    )
    .expect("pending body");
    let retention = app
        .zk_ivm_prove_job_budget
        .try_reserve(ZK_IVM_PROVE_JOB_MIN_PENDING_RESERVATION_BYTES)
        .expect("test reservation");
    let created_ms = zk_ivm_prove_now_ms();
    app.zk_ivm_prove_jobs.insert(
        job_id.clone(),
        ZkIvmProveJobState {
            owner: sample_ivm_prove_authority(),
            created_ms,
            last_access_ms: created_ms,
            status: ZkIvmProveJobStatus::Pending,
            response_body,
            retention,
            cancel,
        },
    );
    let err = match call_zk_ivm_prove_get(app.clone(), job_id).await {
        Ok(_) => panic!("prove-job response larger than the egress burst must be throttled"),
        Err(err) => err,
    };
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok()),
        Some("5")
    );
    assert_eq!(
        app.zk_ivm_prove_jobs
            .get("0123456789abcdef0123456789abcdef")
            .expect("job remains cached")
            .last_access_ms,
        created_ms,
        "rejected polls must not refresh terminal LRU state"
    );
}
#[test]
fn zk_ivm_job_budget_concurrent_reservations_never_exceed_cap() {
    let budget = Arc::new(ZkIvmProveJobBudget::new(100));
    let barrier = Arc::new(std::sync::Barrier::new(33));
    let (tx, rx) = std::sync::mpsc::channel();
    let mut workers = Vec::new();
    for _ in 0..32 {
        let budget = Arc::clone(&budget);
        let barrier = Arc::clone(&barrier);
        let tx = tx.clone();
        workers.push(std::thread::spawn(move || {
            let reservation = budget.try_reserve(10);
            tx.send(reservation.is_some()).expect("report reservation");
            barrier.wait();
            drop(reservation);
        }));
    }
    drop(tx);
    let admitted = (0..32)
        .map(|_| rx.recv().expect("worker result"))
        .filter(|admitted| *admitted)
        .count();
    assert_eq!(admitted, 10);
    assert_eq!(budget.used_bytes(), 100);
    barrier.wait();
    for worker in workers {
        worker.join().expect("worker must not panic");
    }
    assert_eq!(budget.used_bytes(), 0);
}
#[test]
fn zk_ivm_job_json_states_are_minimal_and_done_proof_is_compact() {
    let job_id = "0123456789abcdef0123456789abcdef".to_owned();
    let pending = zk_ivm_prove_job_response_body(
        job_id.clone(),
        ZkIvmProveJobStatus::Pending,
        None,
        None,
        None,
    )
    .expect("pending response");
    let pending: norito::json::Value = norito::json::from_slice(&pending).expect("pending JSON");
    let pending = pending.as_object().expect("pending object");
    assert_eq!(pending.len(), 2);
    assert!(pending.contains_key("job_id") && pending.contains_key("status"));
    let proved = IvmProved {
        bytecode: IvmBytecode::from_compiled(vec![1, 2, 3]),
        overlay: iroha_primitives::const_vec::ConstVec::new_empty(),
        events_commitment: Hash::new(b"events"),
        gas_policy_commitment: Hash::new(b"gas"),
    };
    let backend = "halo2/ipa".to_owned();
    let mut attachment = iroha_data_model::proof::ProofAttachment::new_ref(
        backend.clone(),
        iroha_data_model::proof::ProofBox::new(backend.clone(), vec![1, 2, 3]),
        VerifyingKeyId::new(backend, "compact"),
    );
    attachment.vk_commitment = Some([7_u8; 32]);
    attachment.envelope_hash = Some([9_u8; 32]);
    let done = zk_ivm_prove_job_response_body(
        job_id,
        ZkIvmProveJobStatus::Done,
        None,
        Some(proved),
        Some(attachment),
    )
    .expect("done response");
    let done: norito::json::Value = norito::json::from_slice(&done).expect("done JSON");
    let done = done.as_object().expect("done object");
    assert_eq!(done.len(), 4);
    let proof = done
        .get("attachment")
        .and_then(norito::json::Value::as_object)
        .and_then(|attachment| attachment.get("proof"))
        .and_then(norito::json::Value::as_object)
        .expect("compact proof object");
    assert_eq!(
        proof.get("bytes_b64").and_then(norito::json::Value::as_str),
        Some("AQID")
    );
    assert!(!proof.contains_key("bytes"));
    let attachment = done
        .get("attachment")
        .and_then(norito::json::Value::as_object)
        .expect("compact attachment");
    for (field, expected) in [("vk_commitment", 7_u64), ("envelope_hash", 9_u64)] {
        let bytes = attachment
            .get(field)
            .and_then(norito::json::Value::as_array)
            .unwrap_or_else(|| {
                panic!(
                    "{field} must serialize as a byte array, got {:?}",
                    attachment.get(field)
                )
            });
        assert_eq!(bytes.len(), 32, "{field}");
        assert!(
            bytes.iter().all(|byte| byte.as_u64() == Some(expected)),
            "{field} values"
        );
    }
}
#[test]
fn zk_ivm_terminal_errors_do_not_leak_key_paths_or_control_bytes() {
    let secret = "TOP_SECRET_PROVING_KEY_SENTINEL";
    let (status, body) = zk_ivm_prove_terminal_body(
        "0123456789abcdef0123456789abcdef".to_owned(),
        Err(format!(
            "failed to read proving key bytes at /tmp/{secret}.pk: denied\nforbidden"
        )),
    );
    assert_eq!(status, ZkIvmProveJobStatus::Error);
    let rendered = std::str::from_utf8(&body).expect("error JSON is UTF-8");
    assert!(!rendered.contains(secret));
    assert!(!rendered.contains("/tmp/"));
    assert!(!rendered.contains("forbidden"));
    assert!(rendered.contains("proof key material is unavailable or invalid"));
}
#[test]
fn zk_ivm_pending_reservation_survives_status_delete_until_worker_exit() {
    let budget = Arc::new(ZkIvmProveJobBudget::new(1_024));
    let reservation = budget.try_reserve(512).expect("reservation");
    let worker_reservation = Arc::clone(&reservation);
    let jobs = DashMap::new();
    jobs.insert(
        "pending".to_owned(),
        ZkIvmProveJobState {
            owner: sample_ivm_prove_authority(),
            created_ms: 1,
            last_access_ms: 1,
            status: ZkIvmProveJobStatus::Pending,
            response_body: Bytes::from_static(b"{}"),
            retention: reservation,
            cancel: tokio::sync::watch::channel(false).0,
        },
    );
    jobs.remove("pending");
    assert_eq!(budget.used_bytes(), 512);
    drop(worker_reservation);
    assert_eq!(budget.used_bytes(), 0);
}
#[test]
fn zk_ivm_completion_growth_failure_discards_material_and_shrinks_to_error() {
    let budget = Arc::new(ZkIvmProveJobBudget::new(1_100));
    let reservation = budget.try_reserve(1_024).expect("pending reservation");
    let expected_reservation = Arc::clone(&reservation);
    let jobs = DashMap::new();
    jobs.insert(
        "capacity".to_owned(),
        ZkIvmProveJobState {
            owner: sample_ivm_prove_authority(),
            created_ms: 1,
            last_access_ms: 1,
            status: ZkIvmProveJobStatus::Pending,
            response_body: Bytes::from_static(b"{}"),
            retention: reservation,
            cancel: tokio::sync::watch::channel(false).0,
        },
    );
    zk_ivm_prove_store_terminal(
        &jobs,
        budget.as_ref(),
        usize::MAX,
        "capacity",
        &expected_reservation,
        ZkIvmProveJobStatus::Done,
        Bytes::from(vec![0_u8; 1_101]),
    );
    let state = jobs.get("capacity").expect("job retained as bounded error");
    assert_eq!(state.status, ZkIvmProveJobStatus::Error);
    assert!(state.response_body.len() < 1_024);
    assert_eq!(budget.used_bytes(), state.retention.retained_bytes());
    assert!(
        std::str::from_utf8(&state.response_body)
            .expect("error JSON is UTF-8")
            .contains("retained-job memory budget exhausted")
    );
}
#[test]
fn zk_ivm_terminal_eviction_is_scoped_to_the_requesting_owner() {
    let owner = sample_ivm_prove_authority();
    let other =
        checked_torii_test_account_id(0x84, "derive foreign ZK IVM prove owner fixture key");
    let budget = Arc::new(ZkIvmProveJobBudget::new(1_024));
    let jobs = DashMap::new();
    for (job_id, job_owner, last_access_ms) in
        [("owner", owner.clone(), 1_u64), ("other", other, 0_u64)]
    {
        jobs.insert(
            job_id.to_owned(),
            ZkIvmProveJobState {
                owner: job_owner,
                created_ms: 1,
                last_access_ms,
                status: ZkIvmProveJobStatus::Done,
                response_body: Bytes::from_static(b"{}"),
                retention: budget.try_reserve(2).expect("test reservation"),
                cancel: tokio::sync::watch::channel(false).0,
            },
        );
    }
    assert!(zk_ivm_prove_evict_terminal_lru(&jobs, &owner, None));
    assert!(jobs.get("owner").is_none());
    assert!(
        jobs.get("other").is_some(),
        "one tenant must never evict another tenant's completed proof"
    );
}
#[test]
fn zk_ivm_owner_count_quota_cannot_evict_another_tenant() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.zk_ivm_prove_job_max_entries = 2;
        state.zk_ivm_prove_job_max_entries_per_owner = 1;
        state.zk_ivm_prove_job_max_retained_bytes_per_owner = usize::MAX;
    }
    let owner = sample_ivm_prove_authority();
    let other =
        checked_torii_test_account_id(0x84, "derive foreign ZK IVM quota owner fixture key");
    app.zk_ivm_prove_jobs.insert(
        "other-terminal".to_owned(),
        ZkIvmProveJobState {
            owner: other,
            created_ms: 1,
            last_access_ms: 1,
            status: ZkIvmProveJobStatus::Done,
            response_body: Bytes::from_static(b"{}"),
            retention: app
                .zk_ivm_prove_job_budget
                .try_reserve(2)
                .expect("foreign reservation"),
            cancel: tokio::sync::watch::channel(false).0,
        },
    );
    let (first_cancel, _first_rx) = tokio::sync::watch::channel(false);
    let first = zk_ivm_prove_insert_pending(
        &app,
        owner.clone(),
        "owner-pending".to_owned(),
        2,
        Bytes::from_static(b"{}"),
        2,
        first_cancel,
    )
    .expect("first owner job admitted");
    let (second_cancel, _second_rx) = tokio::sync::watch::channel(false);
    assert!(
        zk_ivm_prove_insert_pending(
            &app,
            owner,
            "owner-over-quota".to_owned(),
            3,
            Bytes::from_static(b"{}"),
            2,
            second_cancel,
        )
        .is_none(),
        "pending owner work cannot be evicted to admit another job"
    );
    drop(first);
    assert!(app.zk_ivm_prove_jobs.contains_key("other-terminal"));
    assert!(app.zk_ivm_prove_jobs.contains_key("owner-pending"));
}
#[test]
fn zk_ivm_job_id_collision_never_replaces_another_owner() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.zk_ivm_prove_job_max_entries = 0;
        state.zk_ivm_prove_job_max_entries_per_owner = 0;
        state.zk_ivm_prove_job_max_retained_bytes_per_owner = 0;
    }
    let existing_owner =
        checked_torii_test_account_id(0x84, "derive existing ZK IVM collision owner fixture key");
    let requested_owner = sample_ivm_prove_authority();
    let existing_retention = app
        .zk_ivm_prove_job_budget
        .try_reserve(2)
        .expect("existing reservation");
    app.zk_ivm_prove_jobs.insert(
        "collision".to_owned(),
        ZkIvmProveJobState {
            owner: existing_owner.clone(),
            created_ms: 1,
            last_access_ms: 1,
            status: ZkIvmProveJobStatus::Pending,
            response_body: Bytes::from_static(b"{}"),
            retention: existing_retention,
            cancel: tokio::sync::watch::channel(false).0,
        },
    );
    let used_before = app.zk_ivm_prove_job_budget.used_bytes();
    let (cancel, _cancel_rx) = tokio::sync::watch::channel(false);
    assert!(
        zk_ivm_prove_insert_pending(
            &app,
            requested_owner,
            "collision".to_owned(),
            2,
            Bytes::from_static(b"replacement"),
            11,
            cancel,
        )
        .is_none()
    );
    let retained = app
        .zk_ivm_prove_jobs
        .get("collision")
        .expect("existing job remains");
    assert_eq!(retained.owner, existing_owner);
    assert_eq!(retained.response_body, Bytes::from_static(b"{}"));
    assert_eq!(app.zk_ivm_prove_job_budget.used_bytes(), used_before);
}
#[test]
fn zk_ivm_stale_worker_cannot_overwrite_reused_job_id() {
    let budget = Arc::new(ZkIvmProveJobBudget::new(1_024));
    let jobs = DashMap::new();
    let original = budget.try_reserve(2).expect("original reservation");
    let worker_reservation = Arc::clone(&original);
    jobs.insert(
        "reused".to_owned(),
        ZkIvmProveJobState {
            owner: sample_ivm_prove_authority(),
            created_ms: 1,
            last_access_ms: 1,
            status: ZkIvmProveJobStatus::Running,
            response_body: Bytes::from_static(b"{}"),
            retention: original,
            cancel: tokio::sync::watch::channel(false).0,
        },
    );
    jobs.remove("reused");
    let replacement_owner =
        checked_torii_test_account_id(0x84, "derive replacement ZK IVM job owner fixture key");
    jobs.insert(
        "reused".to_owned(),
        ZkIvmProveJobState {
            owner: replacement_owner.clone(),
            created_ms: 2,
            last_access_ms: 2,
            status: ZkIvmProveJobStatus::Pending,
            response_body: Bytes::from_static(b"replacement"),
            retention: budget.try_reserve(11).expect("replacement reservation"),
            cancel: tokio::sync::watch::channel(false).0,
        },
    );
    zk_ivm_prove_store_terminal(
        &jobs,
        budget.as_ref(),
        usize::MAX,
        "reused",
        &worker_reservation,
        ZkIvmProveJobStatus::Done,
        Bytes::from_static(b"stale result"),
    );
    let replacement = jobs.get("reused").expect("replacement remains");
    assert_eq!(replacement.owner, replacement_owner);
    assert_eq!(replacement.status, ZkIvmProveJobStatus::Pending);
    assert_eq!(
        replacement.response_body,
        Bytes::from_static(b"replacement")
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn zk_ivm_cancelled_started_worker_holds_permit_until_physical_exit() {
    let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
    let permit = semaphore.clone().acquire_owned().await.expect("permit");
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocking = crate::panic_recovery::spawn_blocking_recoverable(move || {
        started_tx.send(()).expect("started");
        release_rx.recv().expect("release");
        Err::<(ZkIvmProveJobStatus, Bytes), String>("discard me".to_owned())
    });
    let physical = Arc::new(ZkIvmProvePhysicalJob::default());
    physical.install(blocking);
    let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
    let waiter = tokio::spawn(async move {
        let _permit = permit;
        physical.await_started(&mut cancel_rx).await
    });
    started_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("worker started");
    cancel_tx.send(true).expect("cancel");
    tokio::task::yield_now().await;
    assert!(
        semaphore.clone().try_acquire_owned().is_err(),
        "cancellation must not free compute capacity while spawn_blocking still runs"
    );
    release_tx.send(()).expect("release worker");
    let (outcome, discarded) = waiter.await.expect("waiter");
    assert!(discarded);
    assert_eq!(outcome.expect_err("fixture errors"), "discard me");
    assert!(semaphore.try_acquire_owned().is_ok());
}

fn synthetic_zk_ivm_enqueue_job(app: &SharedAppState, job_id: &str) -> ZkIvmProveJob {
    let retention = app
        .zk_ivm_prove_job_budget
        .try_reserve(64)
        .expect("synthetic job reservation");
    let slot_permit = app
        .zk_ivm_prove_slots
        .clone()
        .try_acquire_owned()
        .expect("synthetic job slot");
    ZkIvmProveJob {
        control: ZkIvmProveJobControl {
            job_id: job_id.to_owned(),
            jobs: app.zk_ivm_prove_jobs.clone(),
            budget: app.zk_ivm_prove_job_budget.inner.clone(),
            owner_max_bytes: usize::MAX,
            retention,
            physical: Arc::new(ZkIvmProvePhysicalJob::default()),
            telemetry: app.telemetry.clone(),
            slots: app.zk_ivm_prove_slots.clone(),
            slots_total: app.zk_ivm_prove_slots_total,
            slot_permit: Some(slot_permit),
            inflight: app.zk_ivm_prove_inflight.clone(),
            inflight_total: app.zk_ivm_prove_inflight_total,
        },
        future: Box::pin(async {}),
    }
}

#[test]
fn zk_ivm_enqueue_classifies_supervisor_state_without_leaking_capacity() {
    let app = mk_ivm_prove_app_state_for_tests();
    let baseline_slots = app.zk_ivm_prove_slots.available_permits();

    let not_started = zk_ivm_prove_enqueue(&app, synthetic_zk_ivm_enqueue_job(&app, "not-started"))
        .expect_err("an unstarted supervisor must reject work");
    let ZkIvmProveEnqueueError::NotStarted(not_started) = not_started else {
        panic!("missing supervisor must be classified as not started");
    };
    drop(not_started);
    assert_eq!(app.zk_ivm_prove_job_budget.used_bytes(), 0);
    assert_eq!(app.zk_ivm_prove_slots.available_permits(), baseline_slots);

    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    {
        let mut registration = app
            .zk_ivm_prove_job_budget
            .supervisor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registration.sender = Some(sender);
    }
    zk_ivm_prove_enqueue(&app, synthetic_zk_ivm_enqueue_job(&app, "queued"))
        .expect("first job fills the synthetic supervisor queue");
    let full = zk_ivm_prove_enqueue(&app, synthetic_zk_ivm_enqueue_job(&app, "full"))
        .expect_err("a saturated supervisor queue must reject work");
    let ZkIvmProveEnqueueError::Full(full) = full else {
        panic!("saturated supervisor queue must be classified as full");
    };
    drop(full);
    drop(receiver.try_recv().expect("queued job remains recoverable"));
    {
        let mut registration = app
            .zk_ivm_prove_job_budget
            .supervisor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registration.sender = None;
    }
    drop(receiver);
    assert_eq!(app.zk_ivm_prove_job_budget.used_bytes(), 0);
    assert_eq!(app.zk_ivm_prove_slots.available_permits(), baseline_slots);

    let (sender, receiver) = tokio::sync::mpsc::channel(1);
    drop(receiver);
    {
        let mut registration = app
            .zk_ivm_prove_job_budget
            .supervisor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registration.sender = Some(sender);
    }
    let closed = zk_ivm_prove_enqueue(&app, synthetic_zk_ivm_enqueue_job(&app, "closed"))
        .expect_err("a closed supervisor must reject work");
    let ZkIvmProveEnqueueError::Closed(closed) = closed else {
        panic!("closed supervisor queue must be classified as closed");
    };
    drop(closed);
    {
        let mut registration = app
            .zk_ivm_prove_job_budget
            .supervisor
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registration.sender = None;
    }
    assert_eq!(app.zk_ivm_prove_job_budget.used_bytes(), 0);
    assert_eq!(app.zk_ivm_prove_slots.available_permits(), baseline_slots);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn zk_ivm_supervisor_shutdown_joins_non_preemptible_physical_work() {
    let mut app = mk_ivm_prove_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.zk_ivm_prove_slots = Arc::new(tokio::sync::Semaphore::new(1));
        state.zk_ivm_prove_inflight = Arc::new(tokio::sync::Semaphore::new(1));
        state.zk_ivm_prove_slots_total = 1;
        state.zk_ivm_prove_inflight_total = 1;
    }
    zk_ivm_prove_ensure_supervisor(&app);
    let mut supervisor =
        zk_ivm_prove_take_supervisor(app.as_ref()).expect("retained supervisor handle");
    let shutdown = app.shutdown_signal.clone();
    let job_id = "0123456789abcdef0123456789abcdef".to_owned();
    let retention = app
        .zk_ivm_prove_job_budget
        .try_reserve(512)
        .expect("test reservation");
    let (cancel, mut cancel_rx) = tokio::sync::watch::channel(false);
    app.zk_ivm_prove_jobs.insert(
        job_id.clone(),
        ZkIvmProveJobState {
            owner: sample_ivm_prove_authority(),
            created_ms: 1,
            last_access_ms: 1,
            status: ZkIvmProveJobStatus::Pending,
            response_body: Bytes::from_static(b"{}"),
            retention: retention.clone(),
            cancel,
        },
    );
    let slot_permit = app
        .zk_ivm_prove_slots
        .clone()
        .try_acquire_owned()
        .expect("slot permit");
    let inflight_permit = app
        .zk_ivm_prove_inflight
        .clone()
        .try_acquire_owned()
        .expect("inflight permit");
    let physical = Arc::new(ZkIvmProvePhysicalJob::default());
    let control = ZkIvmProveJobControl {
        job_id: job_id.clone(),
        jobs: app.zk_ivm_prove_jobs.clone(),
        budget: app.zk_ivm_prove_job_budget.inner.clone(),
        owner_max_bytes: usize::MAX,
        retention: retention.clone(),
        physical: physical.clone(),
        telemetry: app.telemetry.clone(),
        slots: app.zk_ivm_prove_slots.clone(),
        slots_total: 1,
        slot_permit: Some(slot_permit),
        inflight: app.zk_ivm_prove_inflight.clone(),
        inflight_total: 1,
    };
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let physical_for_task = physical.clone();
    let retention_for_task = retention.clone();
    let future: ZkIvmProveJobFuture = Box::pin(async move {
        let task = crate::panic_recovery::spawn_blocking_recoverable(move || {
            let _inflight_permit = inflight_permit;
            started_tx.send(()).expect("report physical start");
            release_rx.recv().expect("release physical work");
            Ok((ZkIvmProveJobStatus::Done, Bytes::from_static(b"{}")))
        });
        physical_for_task.install(task);
        let _ = physical_for_task.await_started(&mut cancel_rx).await;
        drop(retention_for_task);
    });
    assert!(
        zk_ivm_prove_enqueue(&app, ZkIvmProveJob { control, future }).is_ok(),
        "synthetic job must enter the supervised queue"
    );
    drop(retention);
    started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("physical work started");

    shutdown.send();
    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut supervisor)
            .await
            .is_err(),
        "shutdown must wait for physical work"
    );
    assert!(app.zk_ivm_prove_slots.clone().try_acquire_owned().is_err());
    assert!(
        app.zk_ivm_prove_inflight
            .clone()
            .try_acquire_owned()
            .is_err()
    );
    release_tx.send(()).expect("release physical work");
    assert_eq!(
        supervisor.await.expect("supervisor joins"),
        ToriiCriticalWorkerExit::StoppedByShutdown
    );
    assert!(app.zk_ivm_prove_jobs.is_empty());
    assert_eq!(app.zk_ivm_prove_job_budget.used_bytes(), 0);
    assert_eq!(app.zk_ivm_prove_slots.available_permits(), 1);
    assert_eq!(app.zk_ivm_prove_inflight.available_permits(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn zk_ivm_supervisor_terminalizes_wrapper_panic_without_leaking_resources() {
    let mut app = mk_ivm_prove_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.zk_ivm_prove_slots = Arc::new(tokio::sync::Semaphore::new(1));
        state.zk_ivm_prove_inflight = Arc::new(tokio::sync::Semaphore::new(1));
        state.zk_ivm_prove_slots_total = 1;
        state.zk_ivm_prove_inflight_total = 1;
    }
    zk_ivm_prove_ensure_supervisor(&app);
    let supervisor =
        zk_ivm_prove_take_supervisor(app.as_ref()).expect("retained supervisor handle");
    let job_id = "fedcba9876543210fedcba9876543210".to_owned();
    let retention = app
        .zk_ivm_prove_job_budget
        .try_reserve(512)
        .expect("test reservation");
    let (cancel, _cancel_rx) = tokio::sync::watch::channel(false);
    app.zk_ivm_prove_jobs.insert(
        job_id.clone(),
        ZkIvmProveJobState {
            owner: sample_ivm_prove_authority(),
            created_ms: 1,
            last_access_ms: 1,
            status: ZkIvmProveJobStatus::Pending,
            response_body: Bytes::from_static(b"{}"),
            retention: retention.clone(),
            cancel,
        },
    );
    let slot_permit = app
        .zk_ivm_prove_slots
        .clone()
        .try_acquire_owned()
        .expect("slot permit");
    let inflight_permit = app
        .zk_ivm_prove_inflight
        .clone()
        .try_acquire_owned()
        .expect("inflight permit");
    let physical = Arc::new(ZkIvmProvePhysicalJob::default());
    let control = ZkIvmProveJobControl {
        job_id: job_id.clone(),
        jobs: app.zk_ivm_prove_jobs.clone(),
        budget: app.zk_ivm_prove_job_budget.inner.clone(),
        owner_max_bytes: usize::MAX,
        retention: retention.clone(),
        physical: physical.clone(),
        telemetry: app.telemetry.clone(),
        slots: app.zk_ivm_prove_slots.clone(),
        slots_total: 1,
        slot_permit: Some(slot_permit),
        inflight: app.zk_ivm_prove_inflight.clone(),
        inflight_total: 1,
    };
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let physical_for_task = physical.clone();
    let retention_for_task = retention.clone();
    let future: ZkIvmProveJobFuture = Box::pin(async move {
        let _retention_for_task = retention_for_task;
        let task = crate::panic_recovery::spawn_blocking_recoverable(move || {
            let _inflight_permit = inflight_permit;
            started_tx.send(()).expect("report physical start");
            release_rx.recv().expect("release physical work");
            Ok((ZkIvmProveJobStatus::Done, Bytes::from_static(b"{}")))
        });
        physical_for_task.install(task);
        assert!(iroha_core::panic_hook::is_suppressed());
        panic!("injected IVM prove wrapper panic");
    });
    assert!(
        zk_ivm_prove_enqueue(&app, ZkIvmProveJob { control, future }).is_ok(),
        "synthetic job must enter the supervised queue"
    );
    drop(retention);
    started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("physical work started");
    release_tx.send(()).expect("release physical work");
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if app
                .zk_ivm_prove_jobs
                .get(&job_id)
                .is_some_and(|entry| entry.status == ZkIvmProveJobStatus::Error)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("wrapper panic becomes a terminal job error");
    assert!(!iroha_core::panic_hook::is_suppressed());
    {
        let state = app.zk_ivm_prove_jobs.get(&job_id).expect("terminal job");
        assert_eq!(Arc::strong_count(&state.retention), 1);
        assert_eq!(
            app.zk_ivm_prove_job_budget.used_bytes(),
            state.retention.retained_bytes()
        );
        let body = std::str::from_utf8(&state.response_body).expect("terminal JSON");
        assert!(body.contains("IVM proof generation failed"));
    }
    assert_eq!(app.zk_ivm_prove_slots.available_permits(), 1);
    assert_eq!(app.zk_ivm_prove_inflight.available_permits(), 1);

    app.shutdown_signal.send();
    assert_eq!(
        supervisor.await.expect("supervisor joins"),
        ToriiCriticalWorkerExit::StoppedByShutdown
    );
    assert!(app.zk_ivm_prove_jobs.is_empty());
    assert_eq!(app.zk_ivm_prove_job_budget.used_bytes(), 0);
}
#[tokio::test]
async fn zk_ivm_prove_job_completes_and_does_not_expose_gas_used() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut app = mk_ivm_prove_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app");
        state.zk_prover_keys_dir = temp.path().to_path_buf();
        let core = Arc::get_mut(&mut state.state).expect("unique core state");
        core.zk.halo2.enabled = true;
    }
    let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-fixture");
    let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
        Hash::new(b"code"),
        Hash::new(b"overlay"),
        Hash::new(b"events"),
        Hash::new(b"gas"),
    );
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture should include verifying key bytes");
    let vk_commitment = fixture
        .vk_hash("halo2/ipa")
        .expect("fixture should include verifying key commitment");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        iroha_core::zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
        iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        "pasta",
        fixture.schema_hash,
        vk_commitment,
    );
    vk_record.vk_len = vk_box.bytes.len() as u32;
    vk_record.max_proof_bytes = 8 * 1024 * 1024;
    vk_record.key = Some(vk_box);
    vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    // The committed VK block is height 1; both queue and worker target height 2.
    vk_record.activation_height = Some(2);
    vk_record.withdraw_height = Some(3);
    vk_record.gas_schedule_id = Some("sched_0".to_owned());
    let pk_bytes = iroha_core::zk::derive_halo2_ipa_ivm_execution_proving_key_bytes(
        vk_record.key.as_ref().expect("vk_box"),
    )
    .expect("derive proving key bytes");
    let pk_path = zk_pk_store_path(temp.path(), &vk_id);
    std::fs::write(&pk_path, &pk_bytes).expect("write proving key bytes");
    {
        let height = next_block_height(&app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        stx.world
            .verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), vk_record);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block.commit().expect("commit should persist vk record");
    }
    let meta = ivm::ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        ..Default::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);
    let req = make_ivm_prove_request(vk_id, bytecode, None);
    let body = norito::json::to_vec(&req).expect("json encode request");
    let response = call_zk_ivm_prove(app.clone(), axum::body::Bytes::from(body))
        .await
        .expect("prove submit ok")
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(axum::http::header::CACHE_CONTROL),
        Some(&HeaderValue::from_static("private, no-store"))
    );
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let created: ZkIvmProveJobCreatedDto =
        norito::json::from_slice(&body).expect("json decode created dto");
    let job_id = created.job_id;
    let mut final_dto: Option<ZkIvmProveJobDto> = None;
    for _ in 0..4000 {
        let response = call_zk_ivm_prove_get(app.clone(), job_id.clone())
            .await
            .expect("prove get ok")
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(axum::http::header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("private, no-store"))
        );
        assert_eq!(
            response.headers().get(axum::http::header::VARY),
            Some(&HeaderValue::from_static(
                crate::content::CANONICAL_CONTENT_AUTH_VARY
            ))
        );
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let rendered = std::str::from_utf8(&body).expect("utf8 body");
        assert!(
            !rendered.contains("gas_used"),
            "prove job response must not expose gas_used"
        );
        let dto: ZkIvmProveJobDto = norito::json::from_slice(&body).expect("decode job dto");
        match dto.status.as_str() {
            "pending" | "running" => tokio::time::sleep(Duration::from_millis(25)).await,
            "done" => {
                final_dto = Some(dto);
                break;
            }
            "error" => panic!("prove job failed: {:?}", dto.error),
            other => panic!("unexpected prove job status: {other}"),
        }
    }
    let dto = final_dto.expect("prove job should complete");
    let attachment = dto
        .attachment
        .as_ref()
        .expect("expected proof attachment in done response");
    assert_eq!(attachment.vk_commitment, Some(vk_commitment));
    let response = call_zk_ivm_prove_delete(app.clone(), job_id.clone())
        .await
        .expect("prove delete ok")
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
}
#[cfg(feature = "zk-stark")]
#[tokio::test]
async fn zk_ivm_prove_job_completes_for_stark_backend() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut app = mk_ivm_prove_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app");
        state.zk_prover_keys_dir = temp.path().to_path_buf();
        let core = Arc::get_mut(&mut state.state).expect("unique core state");
        core.zk.stark.enabled = true;
        core.zk.halo2.enabled = false;
        core.zk.verify_timeout = Duration::ZERO;
    }
    let backend = "stark/fri/poseidon-x7-goldilocks-6x64-v1";
    let circuit_id = "stark/fri/poseidon-x7-goldilocks-6x64-v1:ivm-execution-v1";
    let vk_id = VerifyingKeyId::new(backend, "ivm-exec-v1-stark");
    let vk_box = sample_stark_vk_box(backend, circuit_id);
    let vk_commitment = iroha_core::zk::hash_vk(&vk_box);
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        circuit_id,
        iroha_data_model::zk::BackendTag::Stark,
        "goldilocks",
        iroha_core::zk::ivm_execution_public_inputs_schema_hash(),
        vk_commitment,
    );
    vk_record.vk_len = vk_box.bytes.len() as u32;
    vk_record.max_proof_bytes = 8 * 1024 * 1024;
    vk_record.key = Some(vk_box.clone());
    vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    vk_record.gas_schedule_id = Some("sched_0".to_owned());
    {
        let height = next_block_height(&app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        stx.world
            .verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), vk_record);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block.commit().expect("commit should persist vk record");
    }
    let meta = ivm::ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        ..Default::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);
    let req = make_ivm_prove_request(vk_id, bytecode, None);
    let body = norito::json::to_vec(&req).expect("json encode request");
    let response = call_zk_ivm_prove(app.clone(), axum::body::Bytes::from(body))
        .await
        .expect("prove submit ok")
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let created: ZkIvmProveJobCreatedDto =
        norito::json::from_slice(&body).expect("json decode created dto");
    let job_id = created.job_id;
    let mut final_dto: Option<ZkIvmProveJobDto> = None;
    for _ in 0..4000 {
        let response = call_zk_ivm_prove_get(app.clone(), job_id.clone())
            .await
            .expect("prove get ok")
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let rendered = std::str::from_utf8(&body).expect("utf8 body");
        assert!(
            !rendered.contains("gas_used"),
            "prove job response must not expose gas_used"
        );
        let dto: ZkIvmProveJobDto = norito::json::from_slice(&body).expect("decode job dto");
        match dto.status.as_str() {
            "pending" | "running" => tokio::time::sleep(Duration::from_millis(25)).await,
            "done" => {
                final_dto = Some(dto);
                break;
            }
            "error" => panic!("prove job failed: {:?}", dto.error),
            other => panic!("unexpected prove job status: {other}"),
        }
    }
    let dto = final_dto.expect("prove job should complete");
    let attachment = dto
        .attachment
        .expect("expected proof attachment in done response");
    assert_eq!(attachment.backend.as_str(), backend);
    assert_eq!(attachment.vk_commitment, Some(vk_commitment));
    assert!(
        iroha_core::zk::verify_backend(backend, &attachment.proof, Some(&vk_box),),
        "generated STARK attachment should verify"
    );
}
#[tokio::test]
async fn zk_ivm_prove_job_loads_vk_bytes_from_disk_when_inline_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut app = mk_ivm_prove_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app");
        state.zk_prover_keys_dir = temp.path().to_path_buf();
        let core = Arc::get_mut(&mut state.state).expect("unique core state");
        core.zk.halo2.enabled = true;
    }
    let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-disk-vk");
    let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
        Hash::new(b"code"),
        Hash::new(b"overlay"),
        Hash::new(b"events"),
        Hash::new(b"gas"),
    );
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture should include verifying key bytes");
    let vk_commitment = fixture
        .vk_hash("halo2/ipa")
        .expect("fixture should include verifying key commitment");
    let vk_path = zk_vk_store_path(temp.path(), &vk_id);
    std::fs::write(&vk_path, &vk_box.bytes).expect("write verifying key bytes");
    let pk_bytes = iroha_core::zk::derive_halo2_ipa_ivm_execution_proving_key_bytes(&vk_box)
        .expect("derive proving key bytes");
    let pk_path = zk_pk_store_path(temp.path(), &vk_id);
    std::fs::write(&pk_path, &pk_bytes).expect("write proving key bytes");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        iroha_core::zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
        iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        "pasta",
        fixture.schema_hash,
        vk_commitment,
    );
    vk_record.vk_len = vk_box.bytes.len() as u32;
    vk_record.max_proof_bytes = 8 * 1024 * 1024;
    vk_record.key = None;
    vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    vk_record.gas_schedule_id = Some("sched_0".to_owned());
    {
        let height = next_block_height(&app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        stx.world
            .verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), vk_record);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block.commit().expect("commit should persist vk record");
    }
    let meta = ivm::ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        ..Default::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);
    let req = make_ivm_prove_request(vk_id, bytecode, None);
    let body = norito::json::to_vec(&req).expect("json encode request");
    let response = call_zk_ivm_prove(app.clone(), axum::body::Bytes::from(body))
        .await
        .expect("prove submit ok")
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let created: ZkIvmProveJobCreatedDto =
        norito::json::from_slice(&body).expect("json decode created dto");
    let job_id = created.job_id;
    let mut final_dto: Option<ZkIvmProveJobDto> = None;
    for _ in 0..4000 {
        let response = call_zk_ivm_prove_get(app.clone(), job_id.clone())
            .await
            .expect("prove get ok")
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let rendered = std::str::from_utf8(&body).expect("utf8 body");
        assert!(
            !rendered.contains("gas_used"),
            "prove job response must not expose gas_used"
        );
        let dto: ZkIvmProveJobDto = norito::json::from_slice(&body).expect("decode job dto");
        match dto.status.as_str() {
            "pending" | "running" => tokio::time::sleep(Duration::from_millis(25)).await,
            "done" => {
                final_dto = Some(dto);
                break;
            }
            "error" => panic!("prove job failed: {:?}", dto.error),
            other => panic!("unexpected prove job status: {other}"),
        }
    }
    let dto = final_dto.expect("prove job should complete");
    let attachment = dto
        .attachment
        .as_ref()
        .expect("expected proof attachment in done response");
    assert_eq!(attachment.vk_commitment, Some(vk_commitment));
}
#[tokio::test]
async fn zk_ivm_prove_job_rejects_non_archive_proving_key_bytes() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut app = mk_ivm_prove_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app");
        state.zk_prover_keys_dir = temp.path().to_path_buf();
        let core = Arc::get_mut(&mut state.state).expect("unique core state");
        core.zk.halo2.enabled = true;
    }
    let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-raw-pk");
    let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
        Hash::new(b"code"),
        Hash::new(b"overlay"),
        Hash::new(b"events"),
        Hash::new(b"gas"),
    );
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture should include verifying key bytes");
    let vk_commitment = fixture
        .vk_hash("halo2/ipa")
        .expect("fixture should include verifying key commitment");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        iroha_core::zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
        iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        "pasta",
        fixture.schema_hash,
        vk_commitment,
    );
    vk_record.vk_len = vk_box.bytes.len() as u32;
    vk_record.max_proof_bytes = 8 * 1024 * 1024;
    vk_record.key = Some(vk_box);
    vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    vk_record.gas_schedule_id = Some("sched_0".to_owned());
    let pk_path = zk_pk_store_path(temp.path(), &vk_id);
    std::fs::write(&pk_path, b"raw-halo2-proving-key").expect("write raw proving key bytes");
    {
        let height = next_block_height(&app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        stx.world
            .verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), vk_record);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block.commit().expect("commit should persist vk record");
    }
    let meta = ivm::ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        ..Default::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);
    let req = make_ivm_prove_request(vk_id, bytecode, None);
    let body = norito::json::to_vec(&req).expect("json encode request");
    let response = call_zk_ivm_prove(app.clone(), axum::body::Bytes::from(body))
        .await
        .expect("prove submit ok")
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let created: ZkIvmProveJobCreatedDto =
        norito::json::from_slice(&body).expect("json decode created dto");
    let job_id = created.job_id;
    let mut final_dto: Option<ZkIvmProveJobDto> = None;
    for _ in 0..4000 {
        let response = call_zk_ivm_prove_get(app.clone(), job_id.clone())
            .await
            .expect("prove get ok")
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let dto: ZkIvmProveJobDto = norito::json::from_slice(&body).expect("decode job dto");
        match dto.status.as_str() {
            "pending" | "running" => tokio::time::sleep(Duration::from_millis(25)).await,
            "error" => {
                final_dto = Some(dto);
                break;
            }
            "done" => panic!("prove job should fail for non-archive proving key bytes"),
            other => panic!("unexpected prove job status: {other}"),
        }
    }
    let dto = final_dto.expect("prove job should fail");
    let error = dto.error.unwrap_or_default();
    assert!(
        error.contains("failed to decode proving key archive"),
        "unexpected error: {error}"
    );
}
#[tokio::test]
async fn zk_ivm_prove_job_rejects_mismatched_client_proved_payload() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut app = mk_ivm_prove_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app");
        state.zk_prover_keys_dir = temp.path().to_path_buf();
        let core = Arc::get_mut(&mut state.state).expect("unique core state");
        core.zk.halo2.enabled = true;
    }
    let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-mismatched-proved");
    let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
        Hash::new(b"code"),
        Hash::new(b"overlay"),
        Hash::new(b"events"),
        Hash::new(b"gas"),
    );
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture should include verifying key bytes");
    let vk_commitment = fixture
        .vk_hash("halo2/ipa")
        .expect("fixture should include verifying key commitment");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        iroha_core::zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
        iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        "pasta",
        fixture.schema_hash,
        vk_commitment,
    );
    vk_record.vk_len = vk_box.bytes.len() as u32;
    vk_record.max_proof_bytes = 8 * 1024 * 1024;
    vk_record.key = Some(vk_box);
    vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    vk_record.gas_schedule_id = Some("sched_0".to_owned());
    let pk_bytes = iroha_core::zk::derive_halo2_ipa_ivm_execution_proving_key_bytes(
        vk_record.key.as_ref().expect("vk_box"),
    )
    .expect("derive proving key bytes");
    let pk_path = zk_pk_store_path(temp.path(), &vk_id);
    std::fs::write(&pk_path, &pk_bytes).expect("write proving key bytes");
    {
        let height = next_block_height(&app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        stx.world
            .verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), vk_record);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block.commit().expect("commit should persist vk record");
    }
    let meta = ivm::ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        ..Default::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);
    let mismatched_proved = IvmProved {
        bytecode: bytecode.clone(),
        overlay: iroha_primitives::const_vec::ConstVec::new_empty(),
        events_commitment: Hash::new(b"wrong-events"),
        gas_policy_commitment: Hash::new(b"wrong-gas-policy"),
    };
    let req = make_ivm_prove_request(vk_id, bytecode, Some(mismatched_proved));
    let body = norito::json::to_vec(&req).expect("json encode request");
    let response = call_zk_ivm_prove(app.clone(), axum::body::Bytes::from(body))
        .await
        .expect("prove submit ok")
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let created: ZkIvmProveJobCreatedDto =
        norito::json::from_slice(&body).expect("json decode created dto");
    let job_id = created.job_id;
    let mut final_dto: Option<ZkIvmProveJobDto> = None;
    for _ in 0..4000 {
        let response = call_zk_ivm_prove_get(app.clone(), job_id.clone())
            .await
            .expect("prove get ok")
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let dto: ZkIvmProveJobDto = norito::json::from_slice(&body).expect("decode job dto");
        match dto.status.as_str() {
            "pending" | "running" => tokio::time::sleep(Duration::from_millis(25)).await,
            "error" => {
                final_dto = Some(dto);
                break;
            }
            "done" => panic!("prove job should fail for mismatched proved payload"),
            other => panic!("unexpected prove job status: {other}"),
        }
    }
    let dto = final_dto.expect("prove job should fail");
    let error = dto.error.unwrap_or_default();
    assert!(
        error.contains("provided `proved` payload does not match node-derived execution payload"),
        "unexpected error: {error}"
    );
}
#[tokio::test]
async fn zk_ivm_derive_returns_proved_payload_without_gas_used() {
    let authority =
        checked_torii_test_account_id(0xfd, "derive ZK IVM derive authority fixture key");
    let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let account = Account::new(authority.clone()).build(&authority);
    let world = World::with_assets([domain], [account], [], [], []);
    let mut app = mk_app_state_for_tests_with_world(world);
    {
        let state = Arc::get_mut(&mut app).expect("unique app");
        let core = Arc::get_mut(&mut state.state).expect("unique core state");
        core.zk.halo2.enabled = true;
    }
    let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-derive");
    let schema_hash = iroha_core::zk::ivm_execution_public_inputs_schema_hash();
    let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
        Hash::new(b"code"),
        Hash::new(b"overlay"),
        Hash::new(b"events"),
        Hash::new(b"gas"),
    );
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture should include verifying key bytes");
    let vk_commitment = fixture
        .vk_hash("halo2/ipa")
        .expect("fixture should include verifying key commitment");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        iroha_core::zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
        iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        "pasta",
        schema_hash,
        vk_commitment,
    );
    vk_record.vk_len = vk_box.bytes.len() as u32;
    vk_record.key = Some(vk_box);
    vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    // The committed VK block is height 1; derivation targets execution at height 2.
    vk_record.activation_height = Some(2);
    vk_record.withdraw_height = Some(3);
    vk_record.gas_schedule_id = Some("sched_0".to_owned());
    {
        let height = next_block_height(&app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        stx.world
            .verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), vk_record);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block.commit().expect("commit should persist vk record");
    }
    let meta = ivm::ProgramMetadata {
        max_cycles: 1,
        mode: ivm::ivm_mode::ZK,
        ..Default::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);
    let req = ZkIvmDeriveRequestDto {
        vk_ref: vk_id,
        authority: authority.clone(),
        fee_payment: sample_ivm_fee_payment(),
        metadata: iroha_data_model::metadata::Metadata::default(),
        bytecode: bytecode.clone(),
    };
    let body = norito::json::to_vec(&req).expect("json encode request");
    let response = handler_zk_ivm_derive(
        State(app.clone()),
        verified_zk_ivm_derive_request(authority.clone()),
        proof_json_headers(),
        crate::loopback_connect_info(),
        axum::body::Bytes::from(body),
    )
    .await
    .expect("derive ok")
    .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let rendered = std::str::from_utf8(&body).expect("utf8 body");
    assert!(
        !rendered.contains("gas_used"),
        "derive response must not expose gas_used"
    );
    let dto: ZkIvmDeriveResponseDto = norito::json::from_slice(&body).expect("decode dto");
    assert_eq!(dto.proved.bytecode, bytecode);
    let foreign =
        checked_torii_test_account_id(0xfc, "derive foreign ZK IVM request authority fixture key");
    let mismatched_body = norito::json::to_vec(&req).expect("encode mismatched request");
    let err = match handler_zk_ivm_derive(
        State(app.clone()),
        verified_zk_ivm_derive_request(foreign),
        proof_json_headers(),
        crate::loopback_connect_info(),
        axum::body::Bytes::from(mismatched_body),
    )
    .await
    {
        Ok(_) => panic!("authenticated account must match the derive authority"),
        Err(err) => err,
    };
    assert_eq!(err.into_response().status(), StatusCode::FORBIDDEN);
    Arc::get_mut(&mut app)
        .expect("unique app after derive response")
        .proof_egress_limiter = limits::RateLimiter::new_u64(Some(1), Some(body.len() as u64 - 1));
    let retry_after = app.proof_limits.retry_after.as_secs().max(1).to_string();
    let request_body = norito::json::to_vec(&req).expect("re-encode derive request");
    let err = match handler_zk_ivm_derive(
        State(app.clone()),
        verified_zk_ivm_derive_request(authority),
        proof_json_headers(),
        crate::loopback_connect_info(),
        axum::body::Bytes::from(request_body),
    )
    .await
    {
        Ok(_) => panic!("derive response above the egress burst must be throttled"),
        Err(err) => err,
    };
    let response = err.into_response();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok()),
        Some(retry_after.as_str())
    );
}
#[test]
fn zk_ivm_prove_gc_evicts_expired_jobs() {
    let jobs = DashMap::new();
    let budget = Arc::new(ZkIvmProveJobBudget::new(1_024));
    jobs.insert(
        "old".to_owned(),
        ZkIvmProveJobState {
            owner: sample_ivm_prove_authority(),
            created_ms: 10,
            last_access_ms: 10,
            status: ZkIvmProveJobStatus::Done,
            response_body: Bytes::from_static(b"{}"),
            retention: budget.try_reserve(2).expect("old reservation"),
            cancel: tokio::sync::watch::channel(false).0,
        },
    );
    let ttl_ms = 1_000;
    jobs.insert(
        "fresh".to_owned(),
        ZkIvmProveJobState {
            owner: sample_ivm_prove_authority(),
            created_ms: ttl_ms + 10,
            last_access_ms: ttl_ms + 10,
            status: ZkIvmProveJobStatus::Done,
            response_body: Bytes::from_static(b"{}"),
            retention: budget.try_reserve(2).expect("fresh reservation"),
            cancel: tokio::sync::watch::channel(false).0,
        },
    );
    zk_ivm_prove_gc_jobs_at(&jobs, ttl_ms + 20, ttl_ms, 1_024);
    assert!(jobs.get("old").is_none(), "expired jobs should be removed");
    assert!(jobs.get("fresh").is_some(), "fresh jobs should be retained");
    assert_eq!(budget.used_bytes(), 2, "TTL eviction releases exactly once");
}
#[tokio::test]
async fn zk_ivm_prove_handlers_authenticate_before_job_gc_or_lookup() {
    let mut app = mk_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_token_digests = Arc::new(limits::ApiTokenDigestSet::default());
        state.zk_ivm_prove_job_ttl_ms = 1;
    }
    let get_job_id = "11111111111111111111111111111111".to_owned();
    let delete_job_id = "22222222222222222222222222222222".to_owned();
    for job_id in [&get_job_id, &delete_job_id] {
        let retention = app
            .zk_ivm_prove_job_budget
            .try_reserve(2)
            .expect("test reservation");
        app.zk_ivm_prove_jobs.insert(
            (*job_id).clone(),
            ZkIvmProveJobState {
                owner: sample_ivm_prove_authority(),
                created_ms: 0,
                last_access_ms: 0,
                status: ZkIvmProveJobStatus::Done,
                response_body: Bytes::from_static(b"{}"),
                retention,
                cancel: tokio::sync::watch::channel(false).0,
            },
        );
    }
    let Err(error) = handler_zk_ivm_prove_get(
        State(app.clone()),
        axum::http::Method::GET,
        format!("/v1/zk/ivm/prove/{get_job_id}")
            .parse()
            .expect("GET URI"),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        axum::extract::Path(get_job_id.clone()),
    )
    .await
    else {
        panic!("unauthenticated GET must fail before GC or lookup");
    };
    assert_unconfigured_api_token_error(error);
    let Err(error) = handler_zk_ivm_prove_delete(
        State(app.clone()),
        axum::http::Method::DELETE,
        format!("/v1/zk/ivm/prove/{delete_job_id}")
            .parse()
            .expect("DELETE URI"),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        axum::extract::Path(delete_job_id.clone()),
    )
    .await
    else {
        panic!("unauthenticated DELETE must fail before GC or removal");
    };
    assert_unconfigured_api_token_error(error);
    let Err(error) = handler_zk_ivm_prove(
        State(app.clone()),
        axum::http::Method::POST,
        "/v1/zk/ivm/prove".parse().expect("POST URI"),
        HeaderMap::new(),
        crate::loopback_connect_info(),
        axum::body::Bytes::new(),
    )
    .await
    else {
        panic!("unauthenticated POST must fail before GC or body parsing");
    };
    assert_unconfigured_api_token_error(error);
    assert!(
        app.zk_ivm_prove_jobs.contains_key(&get_job_id),
        "authentication failure must not garbage-collect an unrelated expired job"
    );
    assert!(
        app.zk_ivm_prove_jobs.contains_key(&delete_job_id),
        "authentication failure must not reveal or remove the selected job"
    );
}
#[tokio::test]
async fn zk_ivm_prove_jobs_reject_cross_tenant_read_and_delete() {
    let owner_key = sample_ivm_prove_authority_keypair();
    let owner = AccountId::new(owner_key.public_key().clone());
    let foreign_key = checked_torii_test_ed25519_keypair(
        0x84,
        "derive foreign ZK IVM request signer fixture key",
    );
    let foreign = AccountId::new(foreign_key.public_key().clone());
    let domain_id = DomainId::try_new("wonderland", "universal").expect("domain id");
    let domain = Domain::new(domain_id).build(&owner);
    let owner_account = Account::new(owner.clone()).build(&owner);
    let foreign_account = Account::new(foreign.clone()).build(&owner);
    let app = mk_app_state_for_tests_with_world(World::with(
        [domain],
        [owner_account, foreign_account],
        [],
    ));
    let job_id = "33333333333333333333333333333333".to_owned();
    let retention = app
        .zk_ivm_prove_job_budget
        .try_reserve(2)
        .expect("test reservation");
    app.zk_ivm_prove_jobs.insert(
        job_id.clone(),
        ZkIvmProveJobState {
            owner: owner.clone(),
            created_ms: zk_ivm_prove_now_ms(),
            last_access_ms: 0,
            status: ZkIvmProveJobStatus::Done,
            response_body: Bytes::from_static(b"{}"),
            retention,
            cancel: tokio::sync::watch::channel(false).0,
        },
    );
    let get_method = axum::http::Method::GET;
    let get_uri: axum::http::Uri = format!("/v1/zk/ivm/prove/{job_id}")
        .parse()
        .expect("GET URI");
    let foreign_get_headers =
        signed_app_headers(&foreign, &foreign_key, &get_method, &get_uri, &[]);
    let response = handler_zk_ivm_prove_get(
        State(app.clone()),
        get_method,
        get_uri,
        foreign_get_headers,
        crate::loopback_connect_info(),
        axum::extract::Path(job_id.clone()),
    )
    .await
    .expect("foreign GET is concealed as missing")
    .into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        response.headers().get(axum::http::header::CACHE_CONTROL),
        Some(&HeaderValue::from_static("private, no-store"))
    );
    assert_eq!(
        response.headers().get(axum::http::header::VARY),
        Some(&HeaderValue::from_static(
            crate::content::CANONICAL_CONTENT_AUTH_VARY
        ))
    );
    assert!(app.zk_ivm_prove_jobs.contains_key(&job_id));
    let delete_method = axum::http::Method::DELETE;
    let delete_uri: axum::http::Uri = format!("/v1/zk/ivm/prove/{job_id}")
        .parse()
        .expect("DELETE URI");
    let foreign_delete_headers =
        signed_app_headers(&foreign, &foreign_key, &delete_method, &delete_uri, &[]);
    let response = handler_zk_ivm_prove_delete(
        State(app.clone()),
        delete_method,
        delete_uri,
        foreign_delete_headers,
        crate::loopback_connect_info(),
        axum::extract::Path(job_id.clone()),
    )
    .await
    .expect("foreign DELETE is concealed as missing")
    .into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(app.zk_ivm_prove_jobs.contains_key(&job_id));
    let owner_delete_method = axum::http::Method::DELETE;
    let owner_delete_uri: axum::http::Uri = format!("/v1/zk/ivm/prove/{job_id}")
        .parse()
        .expect("owner DELETE URI");
    let owner_delete_headers = signed_app_headers(
        &owner,
        &owner_key,
        &owner_delete_method,
        &owner_delete_uri,
        &[],
    );
    let response = handler_zk_ivm_prove_delete(
        State(app.clone()),
        owner_delete_method,
        owner_delete_uri,
        owner_delete_headers,
        crate::loopback_connect_info(),
        axum::extract::Path(job_id.clone()),
    )
    .await
    .expect("owner DELETE succeeds")
    .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!app.zk_ivm_prove_jobs.contains_key(&job_id));
}
#[tokio::test]
async fn zk_ivm_prove_rejects_vk_schema_hash_mismatch() {
    let app = mk_ivm_prove_app_state_for_tests();
    let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-schema-mismatch");
    let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
        Hash::new(b"code"),
        Hash::new(b"overlay"),
        Hash::new(b"events"),
        Hash::new(b"gas"),
    );
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture should include verifying key bytes");
    let vk_commitment = fixture
        .vk_hash("halo2/ipa")
        .expect("fixture should include verifying key commitment");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        iroha_core::zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
        iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        "pasta",
        [0xAA; 32],
        vk_commitment,
    );
    vk_record.vk_len = vk_box.bytes.len() as u32;
    vk_record.max_proof_bytes = 8 * 1024 * 1024;
    vk_record.key = Some(vk_box);
    vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    {
        let height = next_block_height(&app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        stx.world
            .verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), vk_record);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block.commit().expect("commit should persist vk record");
    }
    let meta = ivm::ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        ..Default::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);
    let req = make_ivm_prove_request(vk_id, bytecode, None);
    let body = norito::json::to_vec(&req).expect("json encode request");
    let err = match call_zk_ivm_prove(app.clone(), axum::body::Bytes::from(body)).await {
        Ok(_) => panic!("schema mismatch should be rejected"),
        Err(err) => err,
    };
    match err {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(msg),
        )) => assert!(
            msg.contains("schema hash"),
            "error should mention schema hash mismatch"
        ),
        other => panic!("unexpected error: {other:?}"),
    }
}
#[tokio::test]
async fn zk_ivm_prove_rejects_when_queue_full() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut app = mk_ivm_prove_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app");
        state.zk_prover_keys_dir = temp.path().to_path_buf();
        state.zk_ivm_prove_slots = Arc::new(tokio::sync::Semaphore::new(1));
        state.zk_ivm_prove_inflight = Arc::new(tokio::sync::Semaphore::new(1));
        state.zk_ivm_prove_slots_total = 1;
        state.zk_ivm_prove_inflight_total = 1;
    }
    let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-queue-full");
    let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
        Hash::new(b"code"),
        Hash::new(b"overlay"),
        Hash::new(b"events"),
        Hash::new(b"gas"),
    );
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture should include verifying key bytes");
    let vk_commitment = fixture
        .vk_hash("halo2/ipa")
        .expect("fixture should include verifying key commitment");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        iroha_core::zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
        iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        "pasta",
        fixture.schema_hash,
        vk_commitment,
    );
    vk_record.vk_len = vk_box.bytes.len() as u32;
    vk_record.max_proof_bytes = 8 * 1024 * 1024;
    vk_record.key = Some(vk_box);
    vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    {
        let height = next_block_height(&app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        stx.world
            .verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), vk_record);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block.commit().expect("commit should persist vk record");
    }
    let _saturated = app
        .zk_ivm_prove_slots
        .clone()
        .try_acquire_owned()
        .expect("acquire slot");
    let meta = ivm::ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        ..Default::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);
    let req = make_ivm_prove_request(vk_id, bytecode, None);
    let body = norito::json::to_vec(&req).expect("json encode request");
    let err = match call_zk_ivm_prove(app.clone(), axum::body::Bytes::from(body)).await {
        Ok(_) => panic!("queue full should be rejected"),
        Err(err) => err,
    };
    match err {
        Error::ProofRateLimited { endpoint, .. } => assert_eq!(endpoint, "v1/zk/ivm/prove"),
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(
        app.zk_ivm_prove_jobs.is_empty(),
        "rejected request must not create a job entry"
    );
}
#[tokio::test]
async fn zk_ivm_prove_delete_cancels_and_frees_capacity_slot() {
    let mut app = mk_ivm_prove_app_state_for_tests();
    {
        let state = Arc::get_mut(&mut app).expect("unique app");
        state.zk_ivm_prove_slots = Arc::new(tokio::sync::Semaphore::new(1));
        // Force the job to remain queued so cancellation is deterministic.
        state.zk_ivm_prove_inflight = Arc::new(tokio::sync::Semaphore::new(0));
        state.zk_ivm_prove_slots_total = 1;
        state.zk_ivm_prove_inflight_total = 0;
    }
    let vk_id = VerifyingKeyId::new("halo2/ipa", "ivm-exec-v1-cancel");
    let fixture = iroha_core::zk::test_utils::halo2_ivm_execution_envelope(
        Hash::new(b"code"),
        Hash::new(b"overlay"),
        Hash::new(b"events"),
        Hash::new(b"gas"),
    );
    let vk_box = fixture
        .vk_box("halo2/ipa")
        .expect("fixture should include verifying key bytes");
    let vk_commitment = fixture
        .vk_hash("halo2/ipa")
        .expect("fixture should include verifying key commitment");
    let mut vk_record = VerifyingKeyRecord::new(
        1,
        iroha_core::zk::IVM_EXECUTION_V1_CANONICAL_CIRCUIT_ID,
        iroha_data_model::zk::BackendTag::Halo2IpaPasta,
        "pasta",
        fixture.schema_hash,
        vk_commitment,
    );
    vk_record.vk_len = vk_box.bytes.len() as u32;
    vk_record.max_proof_bytes = 8 * 1024 * 1024;
    vk_record.key = Some(vk_box);
    vk_record.status = iroha_data_model::confidential::ConfidentialStatus::Active;
    {
        let height = next_block_height(&app);
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("height>0"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut stx = block.transaction();
        stx.world
            .verifying_keys_mut_for_testing()
            .insert(vk_id.clone(), vk_record);
        stx.apply();
        block.transactions.insert_block(
            HashSet::new(),
            NonZeroUsize::new(height as usize).expect("block count should be non-zero"),
        );
        block.commit().expect("commit should persist vk record");
    }
    let meta = ivm::ProgramMetadata {
        mode: ivm::ivm_mode::ZK,
        ..Default::default()
    };
    let mut program = meta.encode();
    program.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let bytecode = IvmBytecode::from_compiled(program);
    let req = make_ivm_prove_request(vk_id, bytecode, None);
    let req_body = norito::json::to_vec(&req).expect("json encode request");
    let response = call_zk_ivm_prove(app.clone(), axum::body::Bytes::from(req_body.clone()))
        .await
        .expect("first submission ok")
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    let resp_body = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let created: ZkIvmProveJobCreatedDto =
        norito::json::from_slice(&resp_body).expect("json decode created dto");
    let job_id = created.job_id;
    let err = match call_zk_ivm_prove(app.clone(), axum::body::Bytes::from(req_body.clone())).await
    {
        Ok(_) => panic!("second submission should be rate limited due to capacity slot"),
        Err(err) => err,
    };
    assert!(
        matches!(err, Error::ProofRateLimited { endpoint, .. } if endpoint == "v1/zk/ivm/prove")
    );
    let response = call_zk_ivm_prove_delete(app.clone(), job_id)
        .await
        .expect("prove delete ok")
        .into_response();
    assert_eq!(response.status(), StatusCode::OK);
    // Allow the cancellation signal to reach the queued task so it can drop the slot permit.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        app.zk_ivm_prove_slots.available_permits(),
        1,
        "capacity slot should be released after delete cancels a queued job"
    );
}
#[test]
fn query_validation_message_preserves_conversion_source() {
    let err = iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::Conversion(
            "AccountId must use a canonical I105 literal".to_owned(),
        ),
    );
    assert_eq!(
        validation_fail_message(&err),
        "AccountId must use a canonical I105 literal"
    );
}
include!("part_4b_alias_multisig_auth.rs");
