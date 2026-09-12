// Actual request/refresh owners with independently signed archive fixtures.
mod worker_boundary_tests {
    use super::*;
    include!("worker_boundary_support.rs");

    #[tokio::test(flavor = "current_thread")]
    async fn signed_replay_handlers_preserve_exact_bodies_and_headers() {
        for witness in [false, true] {
            let (_data_dir, fixture, service, gate, app) = worker_fixture();
            let expected = if witness {
                norito::encode_canonical(
                    &service
                        .witness_response(&fixture.accumulator_id, [0; 32])
                        .expect("independently verified witness"),
                )
                .expect("canonical witness")
            } else {
                norito::encode_canonical(
                    &service
                        .root_response(&fixture.accumulator_id)
                        .expect("independently verified signed root"),
                )
                .expect("canonical root")
            };
            let response = request(app.clone(), witness, false)
                .await
                .expect("real handler result");
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                crate::utils::NORITO_MIME_TYPE
            );
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(
                response.headers()[header::X_CONTENT_TYPE_OPTIONS],
                "nosniff"
            );
            assert_eq!(bytes(response).await, expected);
            assert!(gate.suppressed.load(Ordering::Acquire));
            assert!(gate.physical_thread.load(Ordering::Acquire));
            assert!(!iroha_core::panic_hook::is_suppressed());
            assert_capacity(&app, 1);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn replay_handlers_preserve_path_current_state_and_byte_limit_failures() {
        for witness in [false, true] {
            let (_data_dir, _fixture, _service, gate, mut app) = worker_fixture();
            let before = gate.entered.load(Ordering::Acquire);
            let response = request(app.clone(), witness, true)
                .await
                .expect("path failure response");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                bytes(response).await,
                bytes(crate::sccp_replay_path_error_response()).await
            );
            assert_eq!(gate.entered.load(Ordering::Acquire), before);
            gate.mode.store(UNAVAILABLE, Ordering::Release);
            let response = request(app.clone(), witness, false)
                .await
                .expect("current-state failure response");
            assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
            assert_eq!(
                bytes(response).await,
                bytes(crate::sccp_replay_endpoint_error_response(
                    ToriiSccpReplayEndpointErrorV1::Unavailable
                ))
                .await
            );
            gate.mode.store(PASS, Ordering::Release);
            Arc::get_mut(&mut app)
                .expect("request clones are released")
                .torii_proxy_max_response_bytes = 1;
            let response = request(app.clone(), witness, false)
                .await
                .expect("bounded response failure");
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(
                bytes(response).await,
                bytes(crate::sccp_replay_endpoint_error_response(
                    ToriiSccpReplayEndpointErrorV1::Integrity
                ))
                .await
            );
            assert_capacity(&app, 1);
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn replay_handler_physical_panics_return_integrity_and_release_admission() {
        for witness in [false, true] {
            let (_data_dir, _fixture, _service, gate, app) = worker_fixture();
            gate.mode.store(PANIC, Ordering::Release);
            let response = request(app.clone(), witness, false)
                .await
                .expect("panic is a controlled response");
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(
                response.headers()[header::CACHE_CONTROL],
                "private, no-store"
            );
            assert_eq!(
                bytes(response).await,
                bytes(crate::sccp_replay_endpoint_error_response(
                    ToriiSccpReplayEndpointErrorV1::Integrity
                ))
                .await
            );
            assert_eq!(gate.entered.load(Ordering::Acquire), 1);
            assert_eq!(gate.finished.load(Ordering::Acquire), 1);
            assert!(gate.suppressed.load(Ordering::Acquire));
            assert!(!iroha_core::panic_hook::is_suppressed());
            assert_capacity(&app, 1);
            gate.mode.store(PASS, Ordering::Release);
            assert_eq!(
                request(app.clone(), witness, false)
                    .await
                    .expect("following valid request")
                    .status(),
                StatusCode::OK
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cancelled_replay_handlers_retain_both_permits_until_physical_completion() {
        for witness in [false, true] {
            let (_data_dir, _fixture, _service, gate, app) = worker_fixture();
            let _release = ReleaseOnDrop(gate.clone());
            gate.mode.store(BLOCK, Ordering::Release);
            let worker = tokio::spawn(request(app.clone(), witness, false));
            gate.wait_count(&gate.entered, 1).await;
            tokio::time::timeout(Duration::from_secs(1), async {
                for _ in 0..32 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("current-thread executor progresses while real worker waits");
            assert!(gate.physical_thread.load(Ordering::Acquire));
            assert!(gate.suppressed.load(Ordering::Acquire));
            assert_capacity(&app, 0);
            worker.abort();
            assert!(
                worker
                    .await
                    .expect_err("waiting request was cancelled")
                    .is_cancelled()
            );
            assert_eq!(gate.finished.load(Ordering::Acquire), 0);
            assert_capacity(&app, 0);
            assert!(matches!(
                request(app.clone(), witness, false).await,
                Err(crate::Error::Query(
                    iroha_data_model::ValidationFail::QueryFailed(
                        iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
                    )
                ))
            ));
            assert_eq!(
                gate.entered.load(Ordering::Acquire),
                1,
                "capacity failure precedes authority work"
            );
            gate.release();
            tokio::time::timeout(WAIT, async {
                while app.query_inflight.available_permits() != 1 {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("physical completion releases query capacity");
            assert_capacity(&app, 1);
            assert_eq!(
                gate.entered.load(Ordering::Acquire),
                gate.finished.load(Ordering::Acquire)
            );
            assert_eq!(
                request(app.clone(), witness, false)
                    .await
                    .expect("new admitted read")
                    .status(),
                StatusCode::OK
            );
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn refresh_physical_panic_remains_fatal_under_ordinary_supervision() {
        let (_data_dir, _fixture, _service, gate, app) = worker_fixture();
        let torii = refresh_torii(&app);
        let shutdown = crate::ShutdownSignal::new();
        gate.mode.store(PANIC, Ordering::Release);
        let worker = torii.spawn_sccp_replay_refresh_worker(shutdown.clone());
        trigger_refresh(&app);
        let server_shutdown = shutdown.clone();
        let outcome = tokio::time::timeout(
            WAIT,
            crate::supervise_sccp_replay_refresh_worker(shutdown.clone(), worker, async move {
                server_shutdown.receive().await;
                Ok(())
            }),
        )
        .await
        .expect("fatal refresh supervision terminates")
        .expect_err("physical panic is fatal");
        assert_eq!(
            outcome.to_string(),
            "SCCP replay refresh worker exited unexpectedly"
        );
        assert!(shutdown.is_sent());
        assert_eq!(gate.entered.load(Ordering::Acquire), 1);
        assert_eq!(gate.finished.load(Ordering::Acquire), 1);
        assert!(gate.suppressed.load(Ordering::Acquire));
        assert!(!iroha_core::panic_hook::is_suppressed());
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(
            gate.entered.load(Ordering::Acquire),
            1,
            "fatal worker cannot retry"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn refresh_domain_errors_retry_without_accepting_stale_state() {
        let (_data_dir, fixture, service, gate, app) = worker_fixture();
        let original = loaded_head(&service).head;
        fixture
            .source
            .set_all(&fixture.config.replicas, b"invalid replacement");
        gate.mode.store(UNAVAILABLE, Ordering::Release);
        let torii = refresh_torii(&app);
        let retained_owners = Arc::strong_count(&service);
        let shutdown = crate::ShutdownSignal::new();
        let mut worker = torii
            .spawn_sccp_replay_refresh_worker(shutdown.clone())
            .expect("worker enabled");
        trigger_refresh(&app);
        gate.wait_count(&gate.entered, 2).await;
        assert!(
            !shutdown.is_sent(),
            "ordinary refresh error remains retryable"
        );
        assert_eq!(
            service.root_response(&fixture.accumulator_id),
            Err(ToriiSccpReplayEndpointErrorV1::Unavailable)
        );
        assert_eq!(
            loaded_head(&service).head,
            original,
            "invalid remote data cannot replace the signed durable head"
        );
        gate.mode.store(PASS, Ordering::Release);
        assert!(service.root_response(&fixture.accumulator_id).is_ok());
        shutdown.send();
        tokio::time::timeout(WAIT, worker.join())
            .await
            .expect("bounded outer shutdown")
            .expect("normal worker exit");
        wait_service_owners(&service, retained_owners).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn refresh_shutdown_detaches_only_the_started_physical_attempt() {
        let (_data_dir, _fixture, service, gate, app) = worker_fixture();
        let _release = ReleaseOnDrop(gate.clone());
        gate.mode.store(BLOCK, Ordering::Release);
        let torii = refresh_torii(&app);
        let retained_owners = Arc::strong_count(&service);
        let shutdown = crate::ShutdownSignal::new();
        let worker = torii.spawn_sccp_replay_refresh_worker(shutdown.clone());
        let server_shutdown = shutdown.clone();
        let supervisor = tokio::spawn(crate::supervise_sccp_replay_refresh_worker(
            shutdown.clone(),
            worker,
            async move {
                server_shutdown.receive().await;
                Ok(())
            },
        ));
        trigger_refresh(&app);
        gate.wait_count(&gate.entered, 1).await;
        shutdown.send();
        tokio::time::timeout(WAIT, supervisor)
            .await
            .expect("outer supervision exits")
            .expect("supervisor joins")
            .expect("requested shutdown stays successful");
        assert_eq!(
            gate.finished.load(Ordering::Acquire),
            0,
            "no fictional preemption of physical work"
        );
        assert_eq!(gate.entered.load(Ordering::Acquire), 1);
        gate.release();
        gate.wait_count(&gate.finished, 1).await;
        wait_service_owners(&service, retained_owners).await;
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(
            gate.entered.load(Ordering::Acquire),
            1,
            "shutdown cannot schedule a replacement"
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn ordinary_refresh_supervisor_preserves_panic_cancel_and_server_exit() {
        for cancelled in [false, true] {
            let shutdown = crate::ShutdownSignal::new();
            let task = if cancelled {
                let task = tokio::spawn(std::future::pending::<()>());
                task.abort();
                task
            } else {
                tokio::spawn(async {
                    assert!(!iroha_core::panic_hook::is_suppressed());
                    panic!("injected ordinary supervisor invariant");
                })
            };
            let server_shutdown = shutdown.clone();
            let error = tokio::time::timeout(
                WAIT,
                crate::supervise_sccp_replay_refresh_worker(
                    shutdown.clone(),
                    Some(crate::SccpReplayRefreshWorkerHandle::new(task)),
                    async move {
                        server_shutdown.receive().await;
                        Ok(())
                    },
                ),
            )
            .await
            .expect("supervisor terminates")
            .expect_err("critical failure stays fatal");
            assert!(shutdown.is_sent());
            assert_eq!(
                error.to_string(),
                if cancelled {
                    "SCCP replay refresh worker was cancelled"
                } else {
                    "SCCP replay refresh worker panicked"
                }
            );
        }
        let shutdown = crate::ShutdownSignal::new();
        let worker_shutdown = shutdown.clone();
        let worker = crate::SccpReplayRefreshWorkerHandle::new(tokio::spawn(async move {
            worker_shutdown.receive().await;
        }));
        let error = tokio::time::timeout(
            WAIT,
            crate::supervise_sccp_replay_refresh_worker(shutdown.clone(), Some(worker), async {
                Ok(())
            }),
        )
        .await
        .expect("server exit is supervised")
        .expect_err("unrequested server exit is fatal");
        assert!(shutdown.is_sent());
        assert_eq!(
            error.to_string(),
            "Torii server exited before SCCP replay refresh worker shutdown"
        );
    }
    #[tokio::test(flavor = "current_thread")]
    async fn dropped_refresh_handle_aborts_only_outer_scheduling() {
        let (_data_dir, _fixture, service, gate, app) = worker_fixture();
        let _release = ReleaseOnDrop(gate.clone());
        gate.mode.store(BLOCK, Ordering::Release);
        let torii = refresh_torii(&app);
        let retained_owners = Arc::strong_count(&service);
        let shutdown = crate::ShutdownSignal::new();
        let worker = torii.spawn_sccp_replay_refresh_worker(shutdown);
        trigger_refresh(&app);
        gate.wait_count(&gate.entered, 1).await;
        drop(worker);
        tokio::task::yield_now().await;
        assert_eq!(gate.finished.load(Ordering::Acquire), 0);
        assert_eq!(gate.entered.load(Ordering::Acquire), 1);
        gate.release();
        wait_service_owners(&service, retained_owners).await;
        assert_eq!(gate.finished.load(Ordering::Acquire), 1);
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert_eq!(gate.entered.load(Ordering::Acquire), 1);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn replay_handler_token_authentication_precedes_capacity_and_authority() {
        for witness in [false, true] {
            let (_data_dir, _fixture, _service, gate, mut app) = worker_fixture();
            let state = Arc::get_mut(&mut app).expect("unique new app");
            state.require_api_token = true;
            state.api_token_digests = Arc::new(crate::limits::ApiTokenDigestSet::from_tokens([
                "sccp-worker-public-test-token",
            ]));
            let query = app
                .query_inflight
                .clone()
                .try_acquire_owned()
                .expect("query slot");
            let heavy = app
                .query_heavy_inflight
                .clone()
                .try_acquire_owned()
                .expect("heavy slot");
            for token in [None, Some("invalid-public-test-token")] {
                assert!(
                    matches!(request_with_token(app.clone(), witness, false, token).await,
                    Err(crate::Error::Query(iroha_data_model::ValidationFail::NotPermitted(message)))
                        if message == "missing or invalid API token")
                );
                assert_capacity(&app, 0);
                assert_eq!(gate.entered.load(Ordering::Acquire), 0);
            }
            assert!(matches!(
                request_with_token(
                    app.clone(),
                    witness,
                    false,
                    Some("sccp-worker-public-test-token")
                )
                .await,
                Err(crate::Error::Query(
                    iroha_data_model::ValidationFail::QueryFailed(
                        iroha_data_model::query::error::QueryExecutionFail::CapacityLimit
                    )
                ))
            ));
            assert_eq!(gate.entered.load(Ordering::Acquire), 0);
            drop((query, heavy));
            assert_capacity(&app, 1);
            let response = request_with_token(
                app.clone(),
                witness,
                false,
                Some("sccp-worker-public-test-token"),
            )
            .await
            .expect("authenticated signed archive read");
            assert_eq!(response.status(), StatusCode::OK);
            assert!(gate.entered.load(Ordering::Acquire) >= 2);
            assert_capacity(&app, 1);
        }
    }
}
