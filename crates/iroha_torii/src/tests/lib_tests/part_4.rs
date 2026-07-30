
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
            .body(Body::from(vec![
                0_u8;
                app.proof_limits.max_body_bytes as usize + 1
            ]))
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
            .proof_egress_limiter =
            limits::RateLimiter::new_u64(Some(1), Some(expected.len() as u64));
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
        let mut app = mk_app_state_for_tests();
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
                created_ms,
                last_access_ms: created_ms,
                status: ZkIvmProveJobStatus::Pending,
                response_body,
                retention,
                cancel,
            },
        );

        let err = match handler_zk_ivm_prove_get(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            axum::extract::Path(job_id),
        )
        .await
        {
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
        let pending: norito::json::Value =
            norito::json::from_slice(&pending).expect("pending JSON");
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
        let jobs = DashMap::new();
        jobs.insert(
            "capacity".to_owned(),
            ZkIvmProveJobState {
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
            "capacity",
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn zk_ivm_cancelled_started_worker_holds_permit_until_physical_exit() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1));
        let permit = semaphore.clone().acquire_owned().await.expect("permit");
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let blocking = tokio::task::spawn_blocking(move || {
            started_tx.send(()).expect("started");
            release_rx.recv().expect("release");
            Err::<(), String>("discard me".to_owned())
        });
        let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);
        let waiter = tokio::spawn(async move {
            let _permit = permit;
            zk_ivm_await_started_prove_job::<()>(blocking, &mut cancel_rx).await
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

    #[tokio::test]
    async fn zk_ivm_prove_job_completes_and_does_not_expose_gas_used() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = mk_app_state_for_tests();
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
            "halo2/ipa:ivm-execution-v1",
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
        let req = make_ivm_prove_request(vk_id, bytecode, None);
        let body = norito::json::to_vec(&req).expect("json encode request");
        let response = handler_zk_ivm_prove(
            State(app.clone()),
            proof_json_headers(),
            crate::loopback_connect_info(),
            axum::body::Bytes::from(body),
        )
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
            let response = handler_zk_ivm_prove_get(
                State(app.clone()),
                HeaderMap::new(),
                crate::loopback_connect_info(),
                axum::extract::Path(job_id.clone()),
            )
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

        let response = handler_zk_ivm_prove_delete(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            axum::extract::Path(job_id.clone()),
        )
        .await
        .expect("prove delete ok")
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[cfg(feature = "zk-stark")]
    #[tokio::test]
    async fn zk_ivm_prove_job_completes_for_stark_backend() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app");
            state.zk_prover_keys_dir = temp.path().to_path_buf();
            let core = Arc::get_mut(&mut state.state).expect("unique core state");
            core.zk.stark.enabled = true;
            core.zk.halo2.enabled = false;
            core.zk.verify_timeout = Duration::ZERO;
        }

        let backend = "stark/fri/sha256-goldilocks";
        let circuit_id = "stark/fri/sha256-goldilocks:ivm-execution-v1";
        let vk_id = VerifyingKeyId::new(backend, "ivm-exec-v1-stark");
        let vk_box = sample_stark_vk_box(
            backend,
            circuit_id,
            iroha_core::zk_stark::STARK_HASH_SHA256_V1,
        );
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
        let response = handler_zk_ivm_prove(
            State(app.clone()),
            proof_json_headers(),
            crate::loopback_connect_info(),
            axum::body::Bytes::from(body),
        )
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
            let response = handler_zk_ivm_prove_get(
                State(app.clone()),
                HeaderMap::new(),
                crate::loopback_connect_info(),
                axum::extract::Path(job_id.clone()),
            )
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
        let mut app = mk_app_state_for_tests();
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
            "halo2/ipa:ivm-execution-v1",
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
        let response = handler_zk_ivm_prove(
            State(app.clone()),
            proof_json_headers(),
            crate::loopback_connect_info(),
            axum::body::Bytes::from(body),
        )
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
            let response = handler_zk_ivm_prove_get(
                State(app.clone()),
                HeaderMap::new(),
                crate::loopback_connect_info(),
                axum::extract::Path(job_id.clone()),
            )
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
        let mut app = mk_app_state_for_tests();
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
            "halo2/ipa:ivm-execution-v1",
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
        let response = handler_zk_ivm_prove(
            State(app.clone()),
            proof_json_headers(),
            crate::loopback_connect_info(),
            axum::body::Bytes::from(body),
        )
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
            let response = handler_zk_ivm_prove_get(
                State(app.clone()),
                HeaderMap::new(),
                crate::loopback_connect_info(),
                axum::extract::Path(job_id.clone()),
            )
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
        let mut app = mk_app_state_for_tests();
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
            "halo2/ipa:ivm-execution-v1",
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
        let response = handler_zk_ivm_prove(
            State(app.clone()),
            proof_json_headers(),
            crate::loopback_connect_info(),
            axum::body::Bytes::from(body),
        )
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
            let response = handler_zk_ivm_prove_get(
                State(app.clone()),
                HeaderMap::new(),
                crate::loopback_connect_info(),
                axum::extract::Path(job_id.clone()),
            )
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
            error.contains(
                "provided `proved` payload does not match node-derived execution payload"
            ),
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
            "halo2/ipa:ivm-execution-v1",
            iroha_data_model::zk::BackendTag::Halo2IpaPasta,
            "pasta",
            schema_hash,
            vk_commitment,
        );
        vk_record.vk_len = vk_box.bytes.len() as u32;
        vk_record.key = Some(vk_box);
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

        Arc::get_mut(&mut app)
            .expect("unique app after derive response")
            .proof_egress_limiter =
            limits::RateLimiter::new_u64(Some(1), Some(body.len() as u64 - 1));
        let retry_after = app.proof_limits.retry_after.as_secs().max(1).to_string();
        let request_body = norito::json::to_vec(&req).expect("re-encode derive request");
        let err = match handler_zk_ivm_derive(
            State(app.clone()),
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
    async fn zk_ivm_prove_rejects_vk_schema_hash_mismatch() {
        let app = mk_app_state_for_tests();

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
            "halo2/ipa:ivm-execution-v1",
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
        let err = match handler_zk_ivm_prove(
            State(app.clone()),
            proof_json_headers(),
            crate::loopback_connect_info(),
            axum::body::Bytes::from(body),
        )
        .await
        {
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
        let mut app = mk_app_state_for_tests();
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
            "halo2/ipa:ivm-execution-v1",
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
        let err = match handler_zk_ivm_prove(
            State(app.clone()),
            proof_json_headers(),
            crate::loopback_connect_info(),
            axum::body::Bytes::from(body),
        )
        .await
        {
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
        let mut app = mk_app_state_for_tests();
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
            "halo2/ipa:ivm-execution-v1",
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

        let response = handler_zk_ivm_prove(
            State(app.clone()),
            proof_json_headers(),
            crate::loopback_connect_info(),
            axum::body::Bytes::from(req_body.clone()),
        )
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

        let err = match handler_zk_ivm_prove(
            State(app.clone()),
            proof_json_headers(),
            crate::loopback_connect_info(),
            axum::body::Bytes::from(req_body.clone()),
        )
        .await
        {
            Ok(_) => panic!("second submission should be rate limited due to capacity slot"),
            Err(err) => err,
        };
        assert!(
            matches!(err, Error::ProofRateLimited { endpoint, .. } if endpoint == "v1/zk/ivm/prove")
        );

        let response = handler_zk_ivm_prove_delete(
            State(app.clone()),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            axum::extract::Path(job_id),
        )
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

    #[tokio::test]
    async fn alias_resolve_index_rejects_unsigned_request() {
        let authority = checked_torii_test_account_id(
            0x0a,
            "derive alias resolve-index unsigned authority fixture key",
        );
        let alias_label = AccountAlias::new(
            "banking".parse().expect("label"),
            Some(iroha_data_model::account::rekey::AccountAliasDomain::new(
                "centralbank".parse::<Name>().expect("domain id"),
            )),
            DataSpaceId::UNIVERSAL,
        );
        let authority_account = Account::new(authority.clone()).build(&authority);
        let domain = Domain::new(DomainId::try_new("centralbank", "universal").expect("domain id"))
            .build(&authority);
        let account = Account::new(authority.clone())
            .with_label(Some(alias_label))
            .build(&authority);
        let body = norito::json::to_vec(&routing::AliasResolveIndexRequestDto { index: 0 })
            .expect("encode request");
        let error = handler_alias_resolve_index(
            State(mk_app_state_for_tests_with_world(World::with(
                [domain],
                [authority_account, account],
                [],
            ))),
            axum::http::Method::POST,
            "/v1/aliases/resolve-index"
                .parse()
                .expect("alias resolve-index uri"),
            HeaderMap::new(),
            axum::body::Bytes::from(body),
        )
        .await
        .expect_err("unsigned index enumeration must be rejected");

        assert!(matches!(
            error,
            Error::AppUnauthorized {
                code: "alias_auth_required",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn alias_resolve_index_rejects_malformed_json_body() {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x0b,
            "derive alias resolve-index malformed body authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let body = b"{";
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, body);

        let err = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from_static(body),
        )
        .await
        .expect_err("malformed resolve-index bodies should be rejected");

        match err {
            Error::Query(ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
            )) => assert!(
                !message.trim().is_empty(),
                "malformed request bodies should surface a parse diagnostic"
            ),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    fn multisig_read_payload<T>(value: T) -> NoritoJsonWithBytes<T>
    where
        T: norito::json::JsonSerialize,
    {
        let raw = norito::json::to_vec(&value).expect("encode multisig read request");
        NoritoJsonWithBytes {
            value,
            raw: axum::body::Bytes::from(raw),
        }
    }

    #[tokio::test]
    async fn contract_code_artifact_read_rejects_unsigned_requests() {
        let uri: axum::http::Uri = format!("/v1/contracts/code-bytes/{}", "a".repeat(64))
            .parse()
            .expect("contract code URI");
        let error = match handler_get_contract_code_bytes(
            State(mk_app_state_for_tests()),
            Method::GET,
            uri,
            HeaderMap::new(),
            crate::loopback_connect_info(),
            axum::extract::Path("a".repeat(64)),
        )
        .await
        {
            Ok(_) => panic!("unsigned contract artifact read must fail closed"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            Error::AppUnauthorized {
                code: "contract_code_auth_required",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn multisig_spec_rejects_unsigned_alias_selector() {
        let request = routing::MultisigSpecRequestDto {
            selector: routing::MultisigAccountSelectorDto {
                multisig_account_id: None,
                multisig_account_alias: Some("banking@centralbank.universal".to_owned()),
            },
        };
        let error = handler_post_multisig_spec(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_SPEC
                .parse()
                .expect("multisig spec uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed");

        assert!(matches!(
            error,
            Error::AppUnauthorized {
                code: "multisig_read_auth_required",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn multisig_proposals_query_rejects_unsigned_alias_selector() {
        let request = routing::MultisigProposalsQueryRequestDto {
            selector: routing::MultisigAccountSelectorDto {
                multisig_account_id: None,
                multisig_account_alias: Some("banking@centralbank.universal".to_owned()),
            },
            status: Vec::new(),
            cursor: None,
            limit: None,
        };
        let error = handler_post_multisig_proposals_query(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_QUERY
                .parse()
                .expect("multisig proposals query uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed");

        assert!(matches!(
            error,
            Error::AppUnauthorized {
                code: "multisig_read_auth_required",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn multisig_proposals_resolve_rejects_unsigned_alias_selector() {
        let request = routing::MultisigProposalsResolveRequestDto {
            selector: routing::MultisigAccountSelectorDto {
                multisig_account_id: None,
                multisig_account_alias: Some("banking@centralbank.universal".to_owned()),
            },
            proposal_id: Some("deadbeef".to_owned()),
            instructions_hash: None,
        };
        let error = handler_post_multisig_proposals_resolve(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_RESOLVE
                .parse()
                .expect("multisig proposals resolve uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed");

        assert!(matches!(
            error,
            Error::AppUnauthorized {
                code: "multisig_read_auth_required",
                ..
            }
        ));
    }

    #[tokio::test]
    async fn multisig_reads_reject_unsigned_concrete_account_selectors() {
        let selector = || routing::MultisigAccountSelectorDto {
            multisig_account_id: Some((*ALICE_ID).clone()),
            multisig_account_alias: None,
        };

        let spec = handler_post_multisig_spec(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_SPEC
                .parse()
                .expect("multisig spec uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(routing::MultisigSpecRequestDto {
                selector: selector(),
            }),
        )
        .await
        .expect_err("unsigned concrete spec read must fail closed")
        .into_response();
        assert_eq!(spec.status(), StatusCode::UNAUTHORIZED);

        let query = handler_post_multisig_proposals_query(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_QUERY
                .parse()
                .expect("multisig proposals query uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(routing::MultisigProposalsQueryRequestDto {
                selector: selector(),
                status: Vec::new(),
                cursor: None,
                limit: None,
            }),
        )
        .await
        .expect_err("unsigned concrete proposal query must fail closed")
        .into_response();
        assert_eq!(query.status(), StatusCode::UNAUTHORIZED);

        let resolve = handler_post_multisig_proposals_resolve(
            State(mk_app_state_for_tests()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_RESOLVE
                .parse()
                .expect("multisig proposals resolve uri"),
            HeaderMap::new(),
            crate::loopback_connect_info(),
            multisig_read_payload(routing::MultisigProposalsResolveRequestDto {
                selector: selector(),
                proposal_id: Some("a".repeat(64)),
                instructions_hash: None,
            }),
        )
        .await
        .expect_err("unsigned concrete proposal resolve must fail closed")
        .into_response();
        assert_eq!(resolve.status(), StatusCode::UNAUTHORIZED);
    }

    fn multisig_read_contract_test_router(app: SharedAppState) -> Router {
        Router::new()
            .route(
                route_catalog::contracts_and_verification_keys::MULTISIG_SPEC_POST.path(),
                post(handler_post_multisig_spec)
                    .layer(DefaultBodyLimit::max(MULTISIG_READ_MAX_BODY_BYTES)),
            )
            .route(
                route_catalog::contracts_and_verification_keys::MULTISIG_PROPOSALS_QUERY_POST
                    .path(),
                post(handler_post_multisig_proposals_query)
                    .layer(DefaultBodyLimit::max(MULTISIG_READ_MAX_BODY_BYTES)),
            )
            .route(
                route_catalog::contracts_and_verification_keys::MULTISIG_PROPOSALS_RESOLVE_POST
                    .path(),
                post(handler_post_multisig_proposals_resolve)
                    .layer(DefaultBodyLimit::max(MULTISIG_READ_MAX_BODY_BYTES)),
            )
            .fallback(|| async { StatusCode::NOT_FOUND })
            .with_state(app)
    }

    fn multisig_read_contract_request(
        method: HttpMethod,
        path: &str,
        body: impl Into<Body>,
    ) -> Request<Body> {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .header(axum::http::header::ACCEPT, "application/json")
            .body(body.into())
            .expect("multisig read contract request");
        request
            .extensions_mut()
            .insert(crate::loopback_connect_info());
        request
    }

    #[tokio::test]
    async fn multisig_read_http_contract_is_signed_post_only_closed_and_bounded() {
        let router = multisig_read_contract_test_router(mk_app_state_for_tests());
        let alias_body = r#"{"multisig_account_alias":"banking@centralbank.universal"}"#;

        let unsigned = router
            .clone()
            .oneshot(multisig_read_contract_request(
                HttpMethod::POST,
                "/v1/multisig/spec",
                alias_body,
            ))
            .await
            .expect("unsigned spec response");
        assert_eq!(
            unsigned.status(),
            StatusCode::UNAUTHORIZED,
            "unsigned alias selectors must fail before alias resolution"
        );

        for path in [
            "/v1/multisig/spec",
            "/v1/multisig/proposals/query",
            "/v1/multisig/proposals/resolve",
        ] {
            let method_response = router
                .clone()
                .oneshot(multisig_read_contract_request(
                    HttpMethod::GET,
                    path,
                    Body::empty(),
                ))
                .await
                .expect("method response");
            assert_eq!(
                method_response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{path}"
            );
        }
        for retired in [
            "/v1/multisig/proposals/lookup",
            "/v1/multisig/proposals/list",
            "/v1/multisig/proposals/get",
            "/v1/multisig/proposals/search",
        ] {
            let response = router
                .clone()
                .oneshot(multisig_read_contract_request(
                    HttpMethod::POST,
                    retired,
                    alias_body,
                ))
                .await
                .expect("retired route response");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{retired}");
        }

        for (path, body) in [
            (
                "/v1/multisig/spec",
                r#"{"multisig_account_alias":"banking@centralbank.universal","extra":true}"#,
            ),
            (
                "/v1/multisig/proposals/query",
                r#"{"multisig_account_alias":"banking@centralbank.universal","status":[],"extra":true}"#,
            ),
            (
                "/v1/multisig/proposals/resolve",
                r#"{"multisig_account_alias":"banking@centralbank.universal","proposal_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","extra":true}"#,
            ),
        ] {
            let response = router
                .clone()
                .oneshot(multisig_read_contract_request(HttpMethod::POST, path, body))
                .await
                .expect("closed-schema response");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{path}");
        }

        let malformed = router
            .clone()
            .oneshot(multisig_read_contract_request(
                HttpMethod::POST,
                "/v1/multisig/proposals/query",
                r#"{"multisig_account_alias": "unterminated"#,
            ))
            .await
            .expect("malformed JSON response");
        assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);

        let mut missing_content_type = multisig_read_contract_request(
            HttpMethod::POST,
            "/v1/multisig/proposals/query",
            alias_body,
        );
        missing_content_type
            .headers_mut()
            .remove(axum::http::header::CONTENT_TYPE);
        let missing_content_type = router
            .clone()
            .oneshot(missing_content_type)
            .await
            .expect("missing Content-Type response");
        assert_eq!(
            missing_content_type.status(),
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );

        let oversized = format!(
            "{{\"multisig_account_alias\":\"banking@centralbank.universal\",\"padding\":\"{}\"}}",
            "x".repeat(MULTISIG_READ_MAX_BODY_BYTES)
        );
        let oversized_response = router
            .oneshot(multisig_read_contract_request(
                HttpMethod::POST,
                "/v1/multisig/proposals/query",
                oversized,
            ))
            .await
            .expect("oversized response");
        assert_eq!(oversized_response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn multisig_read_handler_requires_api_token_and_signed_viewer_auth() {
        let mut app = mk_app_state_for_tests();
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_tokens_set = Arc::new(HashSet::from(["valid-token".to_owned()]));
        let router = multisig_read_contract_test_router(app);
        let canonical_account_id = checked_torii_test_account_id(
            0x0c,
            "derive multisig API-token policy account fixture key",
        );
        let body = norito::json::to_vec(&routing::MultisigSpecRequestDto {
            selector: routing::MultisigAccountSelectorDto {
                multisig_account_id: Some(canonical_account_id),
                multisig_account_alias: None,
            },
        })
        .expect("encode canonical multisig selector");

        let missing = router
            .clone()
            .oneshot(multisig_read_contract_request(
                HttpMethod::POST,
                "/v1/multisig/spec",
                body.clone(),
            ))
            .await
            .expect("missing-token response");
        assert_eq!(missing.status(), StatusCode::FORBIDDEN);

        let mut authenticated =
            multisig_read_contract_request(HttpMethod::POST, "/v1/multisig/spec", body);
        authenticated
            .headers_mut()
            .insert(HEADER_API_TOKEN, HeaderValue::from_static("valid-token"));
        let still_unsigned = router
            .oneshot(authenticated)
            .await
            .expect("authenticated read response");
        assert_eq!(still_unsigned.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn browser_read_endpoints_are_not_throttled_by_deploy_limiter() {
        let app = mk_app_state_for_tests_with_options(None, Some((1, 1)), None, None);
        let headers = HeaderMap::new();
        let remote_ip = std::net::IpAddr::from([127, 0, 0, 1]);

        let key = super::rate_limit_key(
            &headers,
            Some(remote_ip),
            "v1/contracts/state",
            app.api_token_enforced(),
        );
        assert!(app.deploy_rate_limiter.allow(&key).await);
        assert!(!app.deploy_rate_limiter.allow(&key).await);

        let contract_state_response = match handler_get_contract_state(
            State(app.clone()),
            headers.clone(),
            crate::loopback_connect_info(),
            AxQuery(routing::ContractStateQuery {
                prefix: Some("missing".to_owned()),
                ..Default::default()
            }),
        )
        .await
        {
            Ok(response) => response.into_response(),
            Err(error) => error.into_response(),
        };
        assert_ne!(
            contract_state_response.status(),
            StatusCode::TOO_MANY_REQUESTS
        );

        let selector = || routing::MultisigAccountSelectorDto {
            multisig_account_id: None,
            multisig_account_alias: Some("banking@centralbank.universal".to_owned()),
        };

        let spec_request = routing::MultisigSpecRequestDto {
            selector: selector(),
        };
        let spec_response = handler_post_multisig_spec(
            State(app.clone()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_SPEC
                .parse()
                .expect("multisig spec uri"),
            headers.clone(),
            crate::loopback_connect_info(),
            multisig_read_payload(spec_request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed")
        .into_response();
        assert_ne!(spec_response.status(), StatusCode::TOO_MANY_REQUESTS);

        let query_request = routing::MultisigProposalsQueryRequestDto {
            selector: selector(),
            status: Vec::new(),
            cursor: None,
            limit: None,
        };
        let query_response = handler_post_multisig_proposals_query(
            State(app.clone()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_QUERY
                .parse()
                .expect("multisig proposals query uri"),
            headers.clone(),
            crate::loopback_connect_info(),
            multisig_read_payload(query_request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed")
        .into_response();
        assert_ne!(query_response.status(), StatusCode::TOO_MANY_REQUESTS);

        let resolve_request = routing::MultisigProposalsResolveRequestDto {
            selector: selector(),
            proposal_id: Some("deadbeef".to_owned()),
            instructions_hash: None,
        };
        let resolve_response = handler_post_multisig_proposals_resolve(
            State(app),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_RESOLVE
                .parse()
                .expect("multisig proposals resolve uri"),
            headers,
            crate::loopback_connect_info(),
            multisig_read_payload(resolve_request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed")
        .into_response();
        assert_ne!(resolve_response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn browser_read_endpoints_use_route_scoped_query_rate_keys() {
        let mut app = mk_app_state_for_tests();
        {
            let state = Arc::get_mut(&mut app).expect("unique app state");
            state.rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
        }
        let headers = HeaderMap::new();
        let remote_ip = std::net::IpAddr::from([127, 0, 0, 1]);

        let shared_key = super::rate_limit_key(
            &headers,
            Some(remote_ip),
            "v1/contracts/state",
            app.api_token_enforced(),
        );
        assert!(app.rate_limiter.allow(&shared_key).await);
        assert!(!app.rate_limiter.allow(&shared_key).await);

        let selector = || routing::MultisigAccountSelectorDto {
            multisig_account_id: None,
            multisig_account_alias: Some("banking@centralbank.universal".to_owned()),
        };

        let spec_request = routing::MultisigSpecRequestDto {
            selector: selector(),
        };
        let spec_response = handler_post_multisig_spec(
            State(app.clone()),
            Method::POST,
            routing::ENDPOINT_MULTISIG_SPEC
                .parse()
                .expect("multisig spec uri"),
            headers.clone(),
            crate::loopback_connect_info(),
            multisig_read_payload(spec_request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed")
        .into_response();
        assert_ne!(spec_response.status(), StatusCode::TOO_MANY_REQUESTS);

        let query_request = routing::MultisigProposalsQueryRequestDto {
            selector: selector(),
            status: Vec::new(),
            cursor: None,
            limit: None,
        };
        let query_response = handler_post_multisig_proposals_query(
            State(app),
            Method::POST,
            routing::ENDPOINT_MULTISIG_PROPOSALS_QUERY
                .parse()
                .expect("multisig proposals query uri"),
            headers,
            crate::loopback_connect_info(),
            multisig_read_payload(query_request),
        )
        .await
        .expect_err("unsigned alias selectors must fail closed")
        .into_response();
        assert_ne!(query_response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn alias_resolve_index_returns_on_chain_alias_record() {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x0d,
            "derive alias resolve-index on-chain authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let alias_label = AccountAlias::new(
            "banking".parse().expect("label"),
            Some(iroha_data_model::account::rekey::AccountAliasDomain::new(
                "centralbank".parse::<Name>().expect("domain id"),
            )),
            DataSpaceId::UNIVERSAL,
        );
        let authority_account = Account::new(authority.clone()).build(&authority);
        let domain = Domain::new(DomainId::try_new("centralbank", "universal").expect("domain id"))
            .build(&authority);
        let account = Account::new(authority.clone())
            .with_label(Some(alias_label.clone()))
            .build(&authority);
        let app = mk_app_state_for_tests_with_world(World::with(
            [domain],
            [authority_account, account],
            [],
        ));
        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);

        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let dto: routing::AliasResolveIndexResponseDto =
            norito::json::from_slice(&body).expect("json decode");
        assert_eq!(dto.index, 0);
        assert_eq!(dto.alias, "banking@centralbank.universal");
        assert_eq!(dto.account_id, authority.to_string());
        assert_eq!(dto.source.as_deref(), Some("on_chain"));
    }

    #[tokio::test]
    async fn alias_resolve_index_fanout_returns_single_match_from_reachable_dataspace() {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x0e,
            "derive alias resolve-index fanout authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        configure_multiple_dataspace_routes_for_test(&mut app);
        bind_account_alias_for_test(&app, &authority, "merchant@secondary");

        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);
        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-attempted")
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-succeeded")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let dto: routing::AliasResolveIndexResponseDto =
            norito::json::from_slice(&body).expect("json decode");
        assert_eq!(dto.index, 0);
        assert_eq!(dto.alias, "merchant@secondary");
        assert_eq!(dto.account_id, authority.to_string());
        assert_eq!(dto.source.as_deref(), Some("fanout"));
    }

    #[tokio::test]
    async fn alias_resolve_index_fanout_returns_route_conflict_for_incompatible_bindings() {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x0f,
            "derive alias resolve-index route-conflict authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let mut app = mk_app_state_for_tests_with_world(world_with_account(&authority));
        configure_multiple_dataspace_routes_for_test(&mut app);
        bind_account_alias_for_test(&app, &authority, "merchant@universal");
        bind_account_alias_for_test(&app, &authority, "merchant@secondary");

        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);
        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("route_conflict")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-succeeded")
                .and_then(|value| value.to_str().ok()),
            Some("2")
        );
    }

    #[tokio::test]
    async fn alias_resolve_index_returns_not_found_when_index_is_missing() {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x20,
            "derive alias resolve-index missing authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let authority_account = Account::new(authority.clone()).build(&authority);
        let app = mk_app_state_for_tests_with_world(World::with([], [authority_account], []));
        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);

        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should succeed")
        .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn alias_resolve_index_returns_permission_denied_when_denied_routes_block_miss_fallback()
    {
        let authority_keypair = checked_torii_test_ed25519_keypair(
            0x21,
            "derive alias resolve-index signed authority fixture key",
        );
        let authority = AccountId::new(authority_keypair.public_key().clone());
        let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::alias-index-miss-offline"));
        let mut app = mk_app_state_for_tests_with_world(world_with_account_bound_to_dataspace(
            &authority,
            uaid,
            DataSpaceId::new(12),
        ));
        let (_local_route, _foreign_route) =
            crate::tests_runtime_handlers::configure_private_ingress_with_offline_foreign_route_for_test(
                &mut app,
            );

        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&authority, &authority_keypair, &method, &uri, &body);
        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("permission_denied"),
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-denied")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-unavailable")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let payload =
            norito::decode_from_bytes::<super::ErrorEnvelope>(&body).expect("decode error");
        assert_eq!(payload.code, "permission_denied");
    }

    #[tokio::test]
    async fn alias_resolve_index_returns_permission_denied_when_only_hidden_routes_can_resolve() {
        let caller_keypair = checked_torii_test_ed25519_keypair(
            0x22,
            "derive alias resolve-index hidden-route caller fixture key",
        );
        let caller = AccountId::new(caller_keypair.public_key().clone());
        let target = checked_torii_test_account_id(
            0x23,
            "derive alias resolve-index hidden-route target fixture key",
        );
        let uaid = UniversalAccountId::from_hash(Hash::new(b"torii::alias-index-denied-fanout"));
        let mut app =
            mk_app_state_for_tests_with_world(world_with_target_and_caller_bound_to_dataspace(
                &target,
                &caller,
                uaid,
                DataSpaceId::new(10),
            ));
        configure_private_ingress_routes_for_test(&mut app);
        bind_account_alias_for_test(&app, &target, "merchant@restricted");

        let request = routing::AliasResolveIndexRequestDto { index: 0 };
        let body = norito::json::to_vec(&request).expect("encode request");
        let method = axum::http::Method::POST;
        let uri: axum::http::Uri = "/v1/aliases/resolve-index"
            .parse()
            .expect("alias resolve-index uri");
        let headers = signed_app_headers(&caller, &caller_keypair, &method, &uri, &body);
        let response = handler_alias_resolve_index(
            State(app),
            method,
            uri,
            headers,
            axum::body::Bytes::from(body),
        )
        .await
        .expect("handler should succeed")
        .into_response();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("permission_denied")
        );
        assert_eq!(
            response
                .headers()
                .get("x-iroha-fanout-routes-denied")
                .and_then(|value| value.to_str().ok()),
            Some("1")
        );
        let body = http_body_util::BodyExt::collect(response.into_body())
            .await
            .unwrap()
            .to_bytes();
        let payload =
            norito::decode_from_bytes::<super::ErrorEnvelope>(&body).expect("decode error");
        assert_eq!(payload.code, "permission_denied");
    }

    #[tokio::test]
    async fn validate_api_token_rejects_missing_or_unconfigured() {
        let mut app = mk_app_state_for_tests();
        let state = Arc::get_mut(&mut app).expect("unique app state");
        state.require_api_token = true;
        state.api_tokens_set = Arc::new(HashSet::new());

        let headers = HeaderMap::new();
        assert!(validate_api_token(state, &headers).is_err());

        let mut configured_headers = HeaderMap::new();
        configured_headers.insert(HEADER_API_TOKEN, HeaderValue::from_static("secret"));
        let mut tokens = HashSet::new();
        tokens.insert("secret".to_string());
        state.api_tokens_set = Arc::new(tokens);
        assert!(validate_api_token(state, &configured_headers).is_ok());
    }

    #[tokio::test]
    #[cfg(feature = "telemetry")]
    async fn norito_rpc_gate_records_metrics() {
        let cfg = actual::NoritoRpcTransport {
            enabled: true,
            require_mtls: false,
            stage: actual::NoritoRpcStage::Canary,
            allowed_clients: vec!["ok".into()],
            mtls_trusted_proxy_cidrs:
                iroha_config::parameters::defaults::torii::transport::norito_rpc::mtls_trusted_proxy_cidrs(),
        };
        let (app, metrics) = mk_norito_rpc_test_harness(cfg.clone()).await;
        let trusted_remote = Some("127.0.0.1".parse().expect("trusted proxy"));
        let untrusted_remote = Some("198.51.100.10".parse().expect("untrusted proxy"));

        let mut headers = HeaderMap::new();
        headers.insert(HEADER_API_TOKEN, HeaderValue::from_static("ok"));
        app.check_norito_rpc_allowed(&headers, trusted_remote)
            .expect("canary token should be allowed");
        assert_eq!(
            metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[cfg.stage.label(), "allowed"])
                .get(),
            1
        );

        let missing_token_headers = HeaderMap::new();
        assert!(
            app.check_norito_rpc_allowed(&missing_token_headers, trusted_remote)
                .is_err()
        );
        assert_eq!(
            metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[cfg.stage.label(), "canary_missing_token"])
                .get(),
            1
        );

        let mut wrong_token_headers = HeaderMap::new();
        wrong_token_headers.insert(HEADER_API_TOKEN, HeaderValue::from_static("wrong"));
        assert!(
            app.check_norito_rpc_allowed(&wrong_token_headers, trusted_remote)
                .is_err()
        );
        assert_eq!(
            metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[cfg.stage.label(), "canary_denied"])
                .get(),
            1
        );

        let mtls_cfg = actual::NoritoRpcTransport {
            enabled: true,
            require_mtls: true,
            stage: actual::NoritoRpcStage::Ga,
            allowed_clients: Vec::new(),
            mtls_trusted_proxy_cidrs:
                iroha_config::parameters::defaults::torii::transport::norito_rpc::mtls_trusted_proxy_cidrs(),
        };
        let (mtls_app, mtls_metrics) = mk_norito_rpc_test_harness(mtls_cfg.clone()).await;
        assert!(
            mtls_app
                .check_norito_rpc_allowed(&HeaderMap::new(), trusted_remote)
                .is_err()
        );
        assert_eq!(
            mtls_metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[mtls_cfg.stage.label(), "mtls_required"])
                .get(),
            1
        );
        let mut mtls_headers = HeaderMap::new();
        mtls_headers.insert(HEADER_MTLS_FORWARD, HeaderValue::from_static("present"));
        mtls_app
            .check_norito_rpc_allowed(&mtls_headers, trusted_remote)
            .expect("mtls header should allow RPC");
        assert_eq!(
            mtls_metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[mtls_cfg.stage.label(), "allowed"])
                .get(),
            1
        );
        assert!(
            mtls_app
                .check_norito_rpc_allowed(&mtls_headers, untrusted_remote)
                .is_err()
        );
        assert_eq!(
            mtls_metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[mtls_cfg.stage.label(), "mtls_required"])
                .get(),
            2
        );

        let disabled_cfg = actual::NoritoRpcTransport::default();
        let (disabled_app, disabled_metrics) =
            mk_norito_rpc_test_harness(disabled_cfg.clone()).await;
        assert!(
            disabled_app
                .check_norito_rpc_allowed(&HeaderMap::new(), trusted_remote)
                .is_err()
        );
        assert_eq!(
            disabled_metrics
                .torii_norito_rpc_gate_total
                .with_label_values(&[actual::NoritoRpcStage::Disabled.label(), "disabled"])
                .get(),
            1
        );
    }
