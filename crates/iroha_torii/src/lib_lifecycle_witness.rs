// Bounded historical native evidence for the SDK scheduled verifier.
#[derive(JsonDeserialize, crate::json_macros::JsonSerialize, Clone, Debug)]
#[norito(deny_unknown_fields)]
struct TriggerLifecycleWitnessQuery {
    start_height: u64,
    end_height: u64,
}

async fn handler_trigger_lifecycle_witness(
    State(app): State<SharedAppState>,
    Extension(finality_interval::RouteExecutionDeadline(deadline)): Extension<
        finality_interval::RouteExecutionDeadline,
    >,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    AxQuery(query): AxQuery<TriggerLifecycleWitnessQuery>,
) -> Result<Response, Error> {
    validate_api_token(app.as_ref(), &headers)?;
    let key = rate_limit_key(
        &headers,
        Some(remote.ip()),
        "/v1/triggers/lifecycle-witness",
        app.authenticated_api_token_principal(&headers),
    );
    rate_limit_requests_with_cost(&app, &key, FINALITY_HEAVY_QUERY_RATE_COST).await?;
    let mut admission = acquire_query_admission(&app, true).await?;
    history_producer::HistoryProducerOwner::validate_finality_inheritance(&admission)?;
    if admission.fanout_memory.is_none() {
        admission.fanout_memory = Some(match try_acquire_native_finality_memory(&app) {
            Ok(reservation) => reservation,
            Err(response) => return Ok(response),
        });
    }
    let prepared = prepare_lifecycle_witness(
        Arc::clone(&app.state),
        query,
        admission,
        app.torii_proxy_max_response_bytes,
        deadline,
    )
    .await?;
    enforce_proof_egress(
        app.as_ref(),
        &headers,
        Some(remote.ip()),
        "v1/triggers/lifecycle-witness",
        u64::try_from(prepared.bytes).map_err(|_| native_projection_response::capacity())?,
        true,
    )
    .await?;
    Ok(prepared.response)
}

async fn prepare_lifecycle_witness(
    state: Arc<CoreState>,
    query: TriggerLifecycleWitnessQuery,
    admission: QueryAdmissionPermit,
    response_limit: usize,
    deadline: Instant,
) -> Result<finality_interval::PreparedResponse, Error> {
    let owner = history_producer::HistoryProducerOwner::from_admission(&admission)?;
    let response_limit = response_limit.min(owner.response_bytes());
    finality_interval::run_with_stop(admission, owner, move |owner, stop| {
        let mut waiter = finality_interval::state_view_waiter(&owner)?;
        let proof_budget = owner.finality_budget()?;
        let body = match iroha_core::query::native_receipts::lifecycle_activation_witness_owned(
            &state,
            &proof_budget,
            query.start_height,
            query.end_height,
            response_limit,
            deadline,
            &stop,
            &mut waiter,
        ) {
            Ok(body) => body,
            Err(error) => return lifecycle_witness_error(error),
        };
        let bytes = body.as_ref().len();
        let encoded = response_memory_custody::owned(body, &owner)
            .map_err(|_| native_projection_response::capacity())?;
        let response = response_memory_custody::json(encoded, &owner)
            .map_err(|_| native_projection_response::capacity())?;
        Ok(finality_interval::PreparedResponse { response, bytes })
    })
    .await
}

fn lifecycle_witness_error(
    error: iroha_core::query::native_receipts::LifecycleWitnessReadError,
) -> Result<finality_interval::PreparedResponse, Error> {
    use iroha_core::query::native_receipts::LifecycleWitnessReadError;
    match error {
        LifecycleWitnessReadError::Finality(
            iroha_core::state::FinalityProofIntervalReadError::Proof(
                iroha_core::sumeragi::finality::NativeFinalityProofIntervalError::Deadline
                | iroha_core::sumeragi::finality::NativeFinalityProofIntervalError::Destination(
                    iroha_core::sumeragi::finality::ProofDestinationError::Deadline,
                ),
            ),
        ) => Ok(finality_interval::PreparedResponse::deadline(
            ResponseFormat::Json,
        )),
        LifecycleWitnessReadError::Finality(error) => Err(finality_interval::source_error(error)),
        error if error.is_local_refusal() => Err(native_projection_response::capacity()),
        error => Err(conversion_error(error.to_string())),
    }
}

#[cfg(test)]
mod lifecycle_witness_tests {
    use super::*;

    #[test]
    fn lifecycle_witness_projection_decode_capacity_keeps_its_http_refusal_class() {
        use iroha_core::query::native_receipts::LifecycleWitnessReadError;
        for (original, expected) in [
            (
                norito::Error::FieldLengthExceeded {
                    length: 2,
                    limit: 1,
                },
                StatusCode::TOO_MANY_REQUESTS,
            ),
            (
                norito::Error::TotalAllocationExceeded {
                    attempted: 2,
                    limit: 1,
                },
                StatusCode::TOO_MANY_REQUESTS,
            ),
            (norito::Error::NonCanonicalEncoding, StatusCode::BAD_REQUEST),
            (norito::Error::ChecksumMismatch, StatusCode::BAD_REQUEST),
        ] {
            let error = LifecycleWitnessReadError::Projection(original.into());
            let Err(error) = lifecycle_witness_error(error) else {
                panic!("projection refusal must not create a successful witness");
            };
            assert_eq!(error.into_response().status(), expected);
        }
    }

    fn paid_admission() -> (
        QueryAdmissionPermit,
        ByteWeightedMemoryPool,
        Arc<tokio::sync::Semaphore>,
    ) {
        let working = 48_000_000;
        let pool = ByteWeightedMemoryPool::new(working).unwrap();
        let reservation = QueryFanoutMemoryReservation::from_admitted_fanout(
            pool.try_acquire_parts([working as u64]).unwrap(),
            QueryFanoutMemoryEnvelope::for_bodyless_finality(working).unwrap(),
            pool.generation(),
        )
        .unwrap();
        let query = Arc::new(tokio::sync::Semaphore::new(1));
        (
            QueryAdmissionPermit {
                _query: Arc::clone(&query).try_acquire_owned().unwrap(),
                heavy: None,
                body: None,
                fanout_memory: Some(reservation),
            },
            pool,
            query,
        )
    }

    fn selected_chain() -> iroha_core::sumeragi::test_chain::CertifiedTestChain {
        use iroha_core::{
            state::World,
            sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
        };
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1000)).unwrap();
        chain.commit(Vec::new());
        // Fixture-only selected state exercises historical reads; it grants no governed
        // activation or deployment qualification to a production node.
        chain.select_lifecycle_for_testing(1002);
        chain.commit_at(1002, Vec::new());
        chain.commit_at(1003, Vec::new());
        chain
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn lifecycle_witness_retains_original_memory_through_last_response_slice() {
        let chain = selected_chain();
        let (admission, pool, query) = paid_admission();
        let owner = history_producer::HistoryProducerOwner::from_admission(&admission).unwrap();
        let frames = owner.cold_frames().clone();
        let limit = owner.response_bytes();
        drop(owner);
        let prepared = prepare_lifecycle_witness(
            Arc::clone(chain.state()),
            TriggerLifecycleWitnessQuery {
                start_height: 3,
                end_height: 4,
            },
            admission,
            limit,
            Instant::now() + route_timeout_for_path("/v1/triggers/lifecycle-witness"),
        )
        .await
        .unwrap();
        assert_eq!(prepared.response.status(), StatusCode::OK);
        assert!(frames.reserved_bytes() >= prepared.bytes);
        assert_eq!(
            query.available_permits(),
            1,
            "physical worker has completed"
        );
        let bytes = axum::body::to_bytes(prepared.response.into_body(), limit)
            .await
            .unwrap();
        assert_eq!(bytes.len(), prepared.bytes);
        let value: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 5);
        assert_eq!(value["runtime_proof"]["height"].as_u64(), Some(3));
        assert_eq!(value["continuation_proofs"].as_array().unwrap().len(), 1);
        assert_eq!(value["continuation_proofs"][0]["height"].as_u64(), Some(4));
        let last = bytes.slice(bytes.len() - 1..);
        drop(bytes);
        assert!(frames.reserved_bytes() > 0);
        assert!(
            pool.try_acquire_parts([48_000_000]).is_none(),
            "last byte retains original memory credit"
        );
        drop(last);
        assert_eq!(frames.reserved_bytes(), 0);
        assert!(pool.try_acquire_parts([48_000_000]).is_some());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn lifecycle_witness_deadline_and_small_response_limit_refuse_before_history_io() {
        let chain = selected_chain();
        for expired in [true, false] {
            let (admission, pool, query) = paid_admission();
            chain.kura().reset_canonical_query_reads_for_test();
            let result = prepare_lifecycle_witness(
                Arc::clone(chain.state()),
                TriggerLifecycleWitnessQuery {
                    start_height: 3,
                    end_height: 4,
                },
                admission,
                if expired { 32 * 1024 * 1024 } else { 1 },
                if expired {
                    Instant::now()
                } else {
                    Instant::now() + Duration::from_secs(60)
                },
            )
            .await;
            if expired {
                let prepared = result.unwrap();
                assert_eq!(prepared.response.status(), StatusCode::REQUEST_TIMEOUT);
                assert_eq!(prepared.bytes, 0);
                drop(prepared);
            } else {
                assert!(
                    result.is_err(),
                    "one response byte cannot admit the original header"
                );
            }
            assert_eq!(chain.kura().canonical_query_reads_for_test(), (0, 0));
            assert_eq!(query.available_permits(), 1);
            assert!(pool.try_acquire_parts([48_000_000]).is_some());
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn lifecycle_witness_route_uses_original_deadline_and_closed_query() {
        use tower::ServiceExt as _;
        let chain = selected_chain();
        let mut app = crate::tests_runtime_handlers::mk_app_state_for_tests();
        let fixture = Arc::get_mut(&mut app).unwrap();
        fixture.state = Arc::clone(chain.state());
        fixture.kura = Arc::clone(chain.kura());
        let router = axum::Router::new()
            .route(
                route_catalog::core::TRIGGER_LIFECYCLE_WITNESS.path(),
                axum::routing::get(handler_trigger_lifecycle_witness),
            )
            .layer(axum::middleware::from_fn(enforce_route_timeout))
            .with_state(app);
        for (suffix, expected) in [
            ("", StatusCode::OK),
            ("&selection=caller", StatusCode::BAD_REQUEST),
        ] {
            let mut request = axum::http::Request::builder()
                .uri(format!(
                    "/v1/triggers/lifecycle-witness?start_height=3&end_height=4{suffix}"
                ))
                .body(Body::empty())
                .unwrap();
            request
                .extensions_mut()
                .insert(crate::loopback_connect_info());
            let response = router.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), expected);
            let bytes = axum::body::to_bytes(response.into_body(), 32 * 1024 * 1024)
                .await
                .unwrap();
            if expected == StatusCode::OK {
                let value: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
                assert_eq!(value.as_object().unwrap().len(), 5);
                assert_eq!(value["runtime_proof"]["height"].as_u64(), Some(3));
            }
        }
    }

    #[test]
    fn lifecycle_witness_activation_policy_route_is_public_and_read_only() {
        let route = route_catalog::core::TRIGGER_LIFECYCLE_ACTIVATION;
        assert_eq!(route.path(), "/v1/triggers/lifecycle-activation");
        assert_eq!(route.method(), route_catalog::HttpMethod::Get);
        assert_eq!(route.effect(), route_catalog::RouteEffect::ReadOnly);
        assert_eq!(route.admission(), route_catalog::AdmissionPolicy::Public);
        assert!(
            route_catalog::core::INFO_ROUTES
                .iter()
                .any(|entry| entry.path() == route.path())
        );
    }

    #[test]
    fn lifecycle_witness_query_is_closed_and_route_is_read_only() {
        let parsed: TriggerLifecycleWitnessQuery =
            norito::json::from_json(r#"{"start_height":2,"end_height":4097}"#).unwrap();
        assert_eq!((parsed.start_height, parsed.end_height), (2, 4097));
        for malformed in [
            r#"{"start_height":2}"#,
            r#"{"start_height":2,"end_height":3,"selection":"caller"}"#,
            r#"{"start_height":-1,"end_height":3}"#,
        ] {
            assert!(norito::json::from_json::<TriggerLifecycleWitnessQuery>(malformed).is_err());
        }
        let route = route_catalog::core::TRIGGER_LIFECYCLE_WITNESS;
        assert_eq!(route.path(), "/v1/triggers/lifecycle-witness");
        assert!(
            route_catalog::core::INFO_ROUTES
                .iter()
                .any(|entry| entry.path() == route.path())
        );
    }
}

// Current committed governed activation lineage. Certified runtime witnesses remain
// the authority for output semantics; the unchanged JSON field names let clients
// pin the original activation height and stable governed-lineage fingerprint.
async fn handler_trigger_lifecycle_activation(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<Response, Error> {
    check_operator_rate_limit(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/triggers/lifecycle-activation",
        true,
    )
    .await?;
    let admission = acquire_query_admission(&app, true).await?;
    let worker = crate::panic_recovery::spawn_blocking_recoverable(move || {
        let result = (|| -> Result<String, String> {
            let view = app.state.view();
            let policy = view.lifecycle_activation_policy()?;
            let fingerprint = view.lifecycle_consensus_config_fingerprint()?;
            let active = policy.active;
            let activation = policy.activation.map(|(_, height)| height.get());
            let network = view.network_id();
            let height = view.height();
            norito::json::to_json(&norito::json!({
                "network_id": network,
                "v2_activation_height": activation,
                "committed_height": height,
                "active": active,
                "consensus_config_fingerprint": fingerprint
            }))
            .map_err(|error| error.to_string())
        })();
        (result, admission)
    });
    let (result, _admission) = crate::panic_recovery::join_recoverable(worker)
        .await
        .map_err(|error| Error::AppServiceUnavailable {
            code: "lifecycle_activation_worker_failed",
            message: error.to_string(),
        })?;
    let mut response = Response::new(Body::from(result.map_err(conversion_error)?));
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    Ok(response)
}
