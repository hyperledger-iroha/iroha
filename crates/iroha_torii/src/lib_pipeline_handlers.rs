// Transaction batch and pipeline status/recovery handlers.
fn accept_transaction_metadata(
    err: &iroha_core::tx::AcceptTransactionFail,
) -> (&'static str, String) {
    match err {
        iroha_core::tx::AcceptTransactionFail::SignatureVerification(fail) => {
            let detail = if fail.detail.is_empty() {
                fail.code().summary().to_owned()
            } else {
                fail.detail.clone()
            };
            (
                fail.code().as_str(),
                format!("failed to accept transaction: {detail}"),
            )
        }
        iroha_core::tx::AcceptTransactionFail::NetworkTimeUnhealthy { .. } => (
            "PRTRY:NTS_UNHEALTHY",
            format!("failed to accept transaction: {err}"),
        ),
        iroha_core::tx::AcceptTransactionFail::TransactionLimit(limit) => (
            "transaction_rejected",
            format!("failed to accept transaction: {}", limit.reason),
        ),
        _ => (
            "transaction_rejected",
            format!("failed to accept transaction: {err}"),
        ),
    }
}
fn transaction_batch_submission_response(accepted_count: usize) -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::ACCEPTED;
    response.headers_mut().insert(
        HeaderName::from_static("preference-applied"),
        HeaderValue::from_static(PREFER_RETURN_MINIMAL),
    );
    if let Ok(header) = HeaderValue::from_str(&accepted_count.to_string()) {
        response.headers_mut().insert(
            HeaderName::from_static("x-iroha-transactions-accepted"),
            header,
        );
    }
    response
}
async fn allow_transaction_batch_rate_limit(
    limiter: &limits::RateLimiter,
    verified_authorities: &[AccountId],
) -> bool {
    admit_verified_transaction_authorities(limiter, verified_authorities)
        .await
        .is_ok()
}
async fn handler_post_transactions_batch(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    crate::utils::extractors::NoritoBytes(body): crate::utils::extractors::NoritoBytes,
) -> Result<Response, Error> {
    let token_hdr = validate_api_token(app.as_ref(), &headers)?.authenticated_principal();
    validate_transaction_batch_body_size(&body, app.transaction_batch_max_bytes)?;
    let compute_permit =
        try_acquire_transaction_ingress_compute(&app.transaction_ingress_compute_inflight)?;
    let (transactions, compute_permit) = run_transaction_ingress_compute_job(
        compute_permit,
        "transaction_batch_decode_worker_failed",
        {
            let queue = app.queue.clone();
            let state = app.state.clone();
            let max_transactions = app.transaction_batch_max_transactions;
            move || {
                decode_transaction_batch_request(
                    body,
                    max_transactions,
                    queue.as_ref(),
                    state.as_ref(),
                )
            }
        },
    )
    .await?;
    admit_transaction_api_token_preauth(
        &app.tx_preauth_rate_limiter,
        token_hdr,
        transactions.len(),
    )
    .await?;
    let ((accepted_transactions, stateless_cache_warm), compute_permit) = {
        let app = app.clone();
        run_transaction_ingress_compute_job(
            compute_permit,
            "transaction_batch_admission_worker_failed",
            move || {
                let mut accepted_transactions = Vec::with_capacity(transactions.len());
                let mut stateless_cache_warm = Vec::new();
                let prechecks = precheck_transaction_batch_ed25519(
                    &transactions,
                    app.state.pipeline.signature_batch_max_ed25519,
                );
                for (transaction, precheck) in transactions.into_iter().zip(prechecks) {
                    // Exact lifecycle multisig is intentionally outside generic transaction
                    // admission, so preserve its dedicated-route error before that policy runs.
                    routing::ensure_generic_transaction_batch_not_ordinary_kagemusha_lifecycle(
                        app.queue.as_ref(),
                        transaction.signed(),
                    )?;
                    let accepted_tx =
                        routing::accept_decoded_signed_transaction_for_ingress_with_precheck(
                            app.state.clone(),
                            transaction,
                            &app.telemetry,
                            precheck.single_ed25519_prechecked,
                            precheck.precheck_rejection,
                        )?;
                    // No route in the batch may mask a later reserved entrypoint's
                    // dedicated admission boundary.
                    routing::ensure_generic_transaction_batch_entrypoint_allowed(
                        app.queue.as_ref(),
                        accepted_tx.entrypoint(),
                    )?;
                    if precheck.single_ed25519_prechecked {
                        stateless_cache_warm.push(accepted_tx.clone());
                    }
                    accepted_transactions.push(accepted_tx);
                }
                let mut accepted = Vec::with_capacity(accepted_transactions.len());
                #[cfg(feature = "connect")]
                let mut local_route_cache = Vec::new();
                for accepted_tx in accepted_transactions {
                    let routing_plan = app
                        .queue
                        .route_plan_with_state(&accepted_tx, app.state.as_ref())
                        .map_err(|error| routing_resolve_error_to_torii_error(&app, error))?;
                    let routing_decision = routing_plan.coordinator_route();
                    #[cfg(feature = "connect")]
                    if !should_execute_route_locally_cached(
                        app.as_ref(),
                        routing_decision,
                        &mut local_route_cache,
                    ) {
                        return Err(Error::AppServiceUnavailable {
                            code: "transaction_batch_route_not_local",
                            message: "batched transaction submission currently accepts only transactions routed to the receiving Torii node".to_owned(),
                        });
                    }
                    accepted.push((accepted_tx, routing_plan));
                }
                Ok::<_, Error>((accepted, stateless_cache_warm))
            },
        )
        .await?
    };
    let verified_authorities = accepted_transactions
        .iter()
        .map(|(transaction, _)| transaction.authority().clone())
        .collect::<Vec<_>>();
    if !allow_transaction_batch_rate_limit(&app.tx_rate_limiter, &verified_authorities).await {
        drop(compute_permit);
        return Err(transaction_rate_limit_error());
    }
    let accepted_count = accepted_transactions.len();
    let app_for_push = app.clone();
    let (_, _compute_permit) = run_transaction_ingress_compute_job(
        compute_permit,
        "transaction_batch_queue_worker_failed",
        move || {
            routing::push_accepted_transactions_for_ingress_with_routing_plans(
                app_for_push.queue.clone(),
                app_for_push.state.clone(),
                accepted_transactions,
            )?;
            app_for_push
                .state
                .warm_stateless_validation_cache_for_torii_prechecked_batch(&stateless_cache_warm);
            Ok::<(), Error>(())
        },
    )
    .await?;
    Ok(transaction_batch_submission_response(accepted_count))
}
#[cfg(feature = "app_api")]
async fn handler_proof_record_get(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    AxPath(id): AxPath<String>,
) -> Result<impl IntoResponse, Error> {
    let remote_ip = remote.ip();
    let enforce =
        !limits::is_allowed_by_cidr(&headers, Some(remote_ip), &app.api_rate_limit_bypass_nets);
    let start = std::time::Instant::now();
    check_proof_access(
        &app,
        &headers,
        Some(remote_ip),
        "/v1/proofs/{id}",
        1,
        enforce,
    )
    .await?;
    let routes = torii_all_dataspace_routes(app.as_ref());
    let (rec, diagnostics, routed_by, fanout_reservation) =
        match resolve_torii_proof_record_for_routes(&app, routes, id).await {
            Ok(result) => result,
            Err(response) => return Ok(response),
        };
    let etag_value = format!("\"{}:{}\"", rec.id.backend, hex::encode(rec.id.proof_hash));
    let cache_control_value = format!(
        "public, max-age={}",
        app.proof_limits.cache_max_age.as_secs().max(1)
    );
    if crate::utils::if_none_match_matches(&headers, &etag_value) {
        app.telemetry.with_metrics(|tel| {
            tel.inc_torii_proof_cache_hit("/v1/proofs/{id}");
            tel.observe_torii_proof_request("/v1/proofs/{id}", "not_modified", 0, start.elapsed())
        });
        let mut resp = axum::response::Response::builder()
            .status(axum::http::StatusCode::NOT_MODIFIED)
            .body(axum::body::Body::empty())
            .map_err(|err| {
                Error::Query(iroha_data_model::ValidationFail::InternalError(
                    err.to_string(),
                ))
            })?;
        if let Ok(cache_header) = axum::http::HeaderValue::from_str(&cache_control_value) {
            resp.headers_mut()
                .insert(axum::http::header::CACHE_CONTROL, cache_header);
        }
        if let Ok(etag) = axum::http::HeaderValue::from_str(&etag_value) {
            resp.headers_mut().insert(axum::http::header::ETAG, etag);
        }
        insert_routed_by_header(&mut resp, routed_by);
        return Ok(hold_query_fanout_memory_in_response_body(
            with_torii_fanout_headers(resp, diagnostics),
            fanout_reservation,
        ));
    }
    let response_budget = ToriiRoutedReadMemoryBudget::new(
        app.query_fanout_working_set_bytes,
        app.torii_proxy_max_response_bytes,
    )
    .map_err(|_| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            "failed to derive the admitted proof response envelope".to_owned(),
        ))
    })?;
    let bytes = norito::core::to_bytes_bounded(&rec, response_budget.final_body_limit()).map_err(
        |err| {
            Error::Query(iroha_data_model::ValidationFail::InternalError(
                err.to_string(),
            ))
        },
    )?;
    let body_len = bytes.len() as u64;
    enforce_proof_egress(
        &app,
        &headers,
        Some(remote_ip),
        "/v1/proofs/{id}",
        body_len,
        enforce,
    )
    .await?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(bytes));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static(utils::NORITO_MIME_TYPE),
    );
    if let Ok(cache_header) = axum::http::HeaderValue::from_str(&cache_control_value) {
        resp.headers_mut()
            .insert(axum::http::header::CACHE_CONTROL, cache_header);
    }
    if let Ok(etag) = axum::http::HeaderValue::from_str(&etag_value) {
        resp.headers_mut().insert(axum::http::header::ETAG, etag);
    }
    insert_routed_by_header(&mut resp, routed_by);
    app.telemetry.with_metrics(|tel| {
        tel.observe_torii_proof_request("/v1/proofs/{id}", "ok", body_len, start.elapsed())
    });
    Ok(hold_query_fanout_memory_in_response_body(
        with_torii_fanout_headers(resp, diagnostics),
        fanout_reservation,
    ))
}
async fn handler_proof_retention_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    let remote_ip = remote.ip();
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    let enforce =
        !limits::is_allowed_by_cidr(&headers, Some(remote_ip), &app.api_rate_limit_bypass_nets);
    check_operator_proof_access(
        &app,
        &headers,
        Some(remote_ip),
        iroha_torii_shared::uri::PROOF_RETENTION_STATUS,
        1,
        enforce,
    )
    .await?;
    let status = routing::handle_proof_retention_status(app.state.clone())?;
    Ok(crate::utils::respond_with_format(status, format))
}
/// Debug endpoint exposing the current AXT proof cache state per dataspace.
#[cfg(feature = "telemetry")]
async fn handler_axt_proof_cache_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    let remote_ip = remote.ip();
    check_access(&app, &headers, Some(remote_ip), "debug/axt/cache").await?;
    let snapshot = app.state.metrics().axt_debug_status();
    Ok(crate::utils::JsonBody(snapshot))
}
/// Fallback when telemetry is disabled.
#[cfg(not(feature = "telemetry"))]
async fn handler_axt_proof_cache_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    let _ = (headers, remote);
    Ok(telemetry_unavailable_response(
        iroha_torii_shared::uri::AXT_PROOF_CACHE_STATUS,
        &app.telemetry,
    ))
}
async fn handler_pipeline_recovery(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    AxPath(height): AxPath<u64>,
) -> Result<Response, Error> {
    check_operator_rate_limit(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/pipeline/recovery",
        true,
    )
    .await?;
    let admission = acquire_query_admission(&app, true).await?;
    let kura = Arc::clone(&app.kura);
    let (result, _admission) = crate::panic_recovery::join_recoverable(
        crate::panic_recovery::spawn_blocking_recoverable(move || {
            // Keep both general-query and heavy-work permits in the physical worker.
            // Cancelling the HTTP future cannot release capacity while Kura reads,
            // JSON projection, or encoding still consume memory.
            let result = build_pipeline_recovery_response(&kura, height);
            (result, admission)
        }),
    )
    .await
    .map_err(|error| Error::AppServiceUnavailable {
        code: "pipeline_recovery_worker_failed",
        message: error.to_string(),
    })?;
    let serialized = result?;
    Ok(axum::http::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(serialized))
        .expect("static pipeline recovery response is valid"))
}
/// Maximum canonical source size for one persisted recovery sidecar.
const PIPELINE_RECOVERY_SOURCE_MAX_BYTES: usize =
    iroha_data_model::merge::MAX_MERGE_EXECUTION_CERTIFIED_SOURCE_BYTES;
/// Hard transport ceiling for the compact JSON rendering of a recovery sidecar.
///
/// The canonical source is independently limited to one MiB before persistence.
/// Eight times that source ceiling leaves conservative room for JSON escaping
/// and structure while keeping the response allocation finite.
const PIPELINE_RECOVERY_MAX_RESPONSE_BYTES: usize = PIPELINE_RECOVERY_SOURCE_MAX_BYTES * 8;
fn build_pipeline_recovery_response(kura: &Kura, height: u64) -> Result<String, Error> {
    let sidecar = kura.read_pipeline_metadata(height).ok_or_else(|| {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        ))
    })?;
    let serialized = norito::json::to_json(&sidecar.to_json_value()).map_err(|source| {
        Error::SerializationFailure {
            context: "pipeline_recovery_sidecar",
            source: Box::new(source),
        }
    })?;
    bounded_pipeline_recovery_json(serialized)
}
fn bounded_pipeline_recovery_json(serialized: String) -> Result<String, Error> {
    if serialized.len() > PIPELINE_RECOVERY_MAX_RESPONSE_BYTES {
        return Err(Error::AppServiceUnavailable {
            code: "pipeline_recovery_response_too_large",
            message: format!(
                "encoded pipeline recovery sidecar exceeds the {PIPELINE_RECOVERY_MAX_RESPONSE_BYTES}-byte response budget"
            ),
        });
    }
    Ok(serialized)
}
#[cfg(test)]
mod pipeline_recovery_response_bounds_tests {
    use super::*;
    #[test]
    fn pipeline_recovery_handler_retains_bounded_physical_work_admission() {
        let source = include_str!("lib_pipeline_handlers.rs");
        let handler = source
            .split_once("async fn handler_pipeline_recovery(")
            .and_then(|(_, tail)| tail.split_once("async fn handler_pipeline_preflight("))
            .map(|(handler, _)| handler)
            .expect("locate pipeline recovery handler source");
        assert!(handler.contains("check_operator_rate_limit("));
        assert!(!handler.contains("validate_api_token("));
        assert!(handler.contains("acquire_query_admission(&app, true)"));
        assert!(handler.contains("spawn_blocking_recoverable"));
        assert!(handler.contains("join_recoverable"));
        assert!(handler.contains("(result, admission)"));
    }
    #[test]
    fn pipeline_recovery_json_rejects_response_larger_than_protocol_budget() {
        let oversized = "x".repeat(PIPELINE_RECOVERY_MAX_RESPONSE_BYTES + 1);
        let error = bounded_pipeline_recovery_json(oversized)
            .expect_err("response over the recovery transport budget must be rejected");
        match error {
            Error::AppServiceUnavailable { code, .. } => {
                assert_eq!(code, "pipeline_recovery_response_too_large");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
    #[test]
    fn pipeline_recovery_json_accepts_response_at_protocol_budget() {
        let exact = "x".repeat(PIPELINE_RECOVERY_MAX_RESPONSE_BYTES);
        assert_eq!(
            bounded_pipeline_recovery_json(exact)
                .expect("response at the recovery transport budget must be accepted")
                .len(),
            PIPELINE_RECOVERY_MAX_RESPONSE_BYTES
        );
    }
}
async fn handler_pipeline_preflight(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    accept: Option<crate::utils::extractors::ExtractAccept>,
) -> Result<Response, Error> {
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(format) => format,
        Err(resp) => return Ok(resp),
    };
    check_operator_rate_limit(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/pipeline/preflight",
        true,
    )
    .await?;
    Ok(crate::utils::respond_with_format(
        routing::build_pipeline_preflight_response(app.state.as_ref(), app.queue.as_ref()),
        format,
    ))
}
async fn handler_pipeline_recovery_fastpq_proofs(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    AxPath(height): AxPath<u64>,
    crate::NoritoStringQuery(query): crate::NoritoStringQuery<PipelineFastpqRecoveryQuery>,
) -> Result<Response, Error> {
    // The route's sealed middleware has already verified its canonical,
    // replay-resistant operator signature before extractors or this handler can
    // run. Charge the authenticated request before touching Kura or acquiring
    // compute capacity.
    check_access_with_rate_limiter(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/pipeline/recovery/fastpq-proofs",
        &app.rate_limiter,
    )
    .await?;
    let page = PipelineFastpqRecoveryPage::parse(&query)?;
    let admission = acquire_query_admission(&app, true).await?;
    let kura = Arc::clone(&app.kura);
    let (result, _admission) = crate::panic_recovery::join_recoverable(
        crate::panic_recovery::spawn_blocking_recoverable(move || {
            // Keep both general-query and heavy-work permits in the physical worker.
            // Cancelling the HTTP future therefore cannot release capacity while
            // Kura reads, transcript reconstruction, or encoding still run.
            let result = build_pipeline_recovery_fastpq_response(&kura, height, page);
            (result, admission)
        }),
    )
    .await
    .map_err(|error| Error::AppServiceUnavailable {
        code: "pipeline_recovery_fastpq_worker_failed",
        message: error.to_string(),
    })?;
    let serialized = result?;
    Ok(axum::http::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(serialized))
        .expect("static FASTPQ recovery response is valid"))
}
const PIPELINE_FASTPQ_RECOVERY_DEFAULT_LIMIT: usize = 16;
const PIPELINE_FASTPQ_RECOVERY_MAX_LIMIT: usize = 64;
const PIPELINE_FASTPQ_RECOVERY_MAX_PROOF_BYTES: usize = 4 * 1024 * 1024;
const PIPELINE_FASTPQ_RECOVERY_MAX_BATCH_BYTES: usize = 8 * 1024 * 1024;
const PIPELINE_FASTPQ_RECOVERY_MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
const PIPELINE_FASTPQ_RECOVERY_MAX_RESPONSE_BYTES: usize = 24 * 1024 * 1024;
#[derive(JsonDeserialize, crate::json_macros::JsonSerialize, Clone, Debug, Default)]
struct PipelineFastpqRecoveryQuery {
    #[norito(default)]
    offset: Option<u64>,
    #[norito(default)]
    limit: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PipelineFastpqRecoveryPage {
    offset: usize,
    limit: usize,
}
impl PipelineFastpqRecoveryPage {
    fn parse(query: &PipelineFastpqRecoveryQuery) -> Result<Self, Error> {
        let offset = usize::try_from(query.offset.unwrap_or_default())
            .map_err(|_| conversion_error("FASTPQ recovery offset exceeds this host".to_owned()))?;
        let raw_limit = query
            .limit
            .unwrap_or(PIPELINE_FASTPQ_RECOVERY_DEFAULT_LIMIT as u64);
        if raw_limit == 0 || raw_limit > PIPELINE_FASTPQ_RECOVERY_MAX_LIMIT as u64 {
            return Err(conversion_error(format!(
                "FASTPQ recovery limit must be between 1 and {PIPELINE_FASTPQ_RECOVERY_MAX_LIMIT}"
            )));
        }
        let limit = usize::try_from(raw_limit)
            .map_err(|_| conversion_error("FASTPQ recovery limit exceeds this host".to_owned()))?;
        Ok(Self { offset, limit })
    }
}
fn fastpq_recovery_capacity_error(message: impl Into<String>) -> Error {
    Error::AppServiceUnavailable {
        code: "pipeline_recovery_fastpq_artifact_too_large",
        message: message.into(),
    }
}
fn charge_fastpq_recovery_artifact_bytes(used: &mut usize, amount: usize) -> Result<(), Error> {
    let next = used.checked_add(amount).ok_or_else(|| {
        fastpq_recovery_capacity_error("FASTPQ recovery artifact size overflowed")
    })?;
    if next > PIPELINE_FASTPQ_RECOVERY_MAX_ARTIFACT_BYTES {
        return Err(fastpq_recovery_capacity_error(format!(
            "selected FASTPQ recovery artifacts exceed the {} byte response-source budget",
            PIPELINE_FASTPQ_RECOVERY_MAX_ARTIFACT_BYTES
        )));
    }
    *used = next;
    Ok(())
}
fn build_pipeline_recovery_fastpq_response(
    kura: &Kura,
    height: u64,
    page: PipelineFastpqRecoveryPage,
) -> Result<String, Error> {
    let Some(sidecar) = kura.read_pipeline_metadata(height) else {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound,
        )));
    };
    let total = sidecar.fastpq_proofs.len();
    let start = page.offset.min(total);
    let end = start.saturating_add(page.limit).min(total);
    let mut artifact_bytes = 0;
    let proofs = sidecar.fastpq_proofs[start..end]
        .iter()
        .map(|snapshot| {
            fastpq_proof_snapshot_recovery_json(kura, height, snapshot, &mut artifact_bytes)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut root = norito::json::Map::new();
    root.insert(
        "height".to_owned(),
        norito::json::to_value(&sidecar.height).expect("serialize pipeline height"),
    );
    root.insert(
        "block_hash".to_owned(),
        norito::json::to_value(&sidecar.block_hash.to_string())
            .expect("serialize pipeline block hash"),
    );
    root.insert(
        "total_proofs".to_owned(),
        norito::json::to_value(&u64::try_from(total).unwrap_or(u64::MAX))
            .expect("serialize FASTPQ proof total"),
    );
    root.insert(
        "offset".to_owned(),
        norito::json::to_value(&u64::try_from(page.offset).unwrap_or(u64::MAX))
            .expect("serialize FASTPQ proof offset"),
    );
    root.insert(
        "limit".to_owned(),
        norito::json::to_value(&u64::try_from(page.limit).unwrap_or(u64::MAX))
            .expect("serialize FASTPQ proof limit"),
    );
    root.insert(
        "next_offset".to_owned(),
        norito::json::to_value(&(end < total).then(|| u64::try_from(end).unwrap_or(u64::MAX)))
            .expect("serialize FASTPQ proof continuation"),
    );
    root.insert("proofs".to_owned(), norito::json::Value::Array(proofs));
    let serialized =
        norito::json::to_json_pretty(&norito::json::Value::Object(root)).map_err(|source| {
            Error::SerializationFailure {
                context: "pipeline_recovery_fastpq_proofs",
                source: Box::new(source),
            }
        })?;
    if serialized.len() > PIPELINE_FASTPQ_RECOVERY_MAX_RESPONSE_BYTES {
        return Err(fastpq_recovery_capacity_error(format!(
            "encoded FASTPQ recovery page exceeds the {} byte response budget",
            PIPELINE_FASTPQ_RECOVERY_MAX_RESPONSE_BYTES
        )));
    }
    Ok(serialized)
}
fn fastpq_proof_snapshot_recovery_json(
    kura: &Kura,
    height: u64,
    snapshot: &iroha_core::kura::FastpqProofSnapshot,
    artifact_bytes: &mut usize,
) -> Result<norito::json::Value, Error> {
    if snapshot.proof.len() > PIPELINE_FASTPQ_RECOVERY_MAX_PROOF_BYTES {
        return Err(fastpq_recovery_capacity_error(format!(
            "FASTPQ proof exceeds the {} byte per-proof budget",
            PIPELINE_FASTPQ_RECOVERY_MAX_PROOF_BYTES
        )));
    }
    charge_fastpq_recovery_artifact_bytes(artifact_bytes, snapshot.proof.len())?;
    // Build the object field-by-field. `FastpqProofSnapshot::to_json_value`
    // eagerly encodes its batch, which would duplicate expensive work before
    // this endpoint can enforce its source-byte budget.
    let mut object = norito::json::Map::new();
    object.insert(
        "entry_hash".to_owned(),
        norito::json::to_value(&snapshot.entry_hash.to_string())
            .expect("serialize FASTPQ entry hash"),
    );
    object.insert(
        "batch_index".to_owned(),
        norito::json::to_value(&snapshot.batch_index).expect("serialize FASTPQ batch index"),
    );
    object.insert(
        "parameter".to_owned(),
        norito::json::to_value(&snapshot.parameter).expect("serialize FASTPQ parameter"),
    );
    object.insert(
        "transition_count".to_owned(),
        norito::json::to_value(&snapshot.transition_count)
            .expect("serialize FASTPQ transition count"),
    );
    object.insert(
        "trace_commitment".to_owned(),
        norito::json::to_value(&hex::encode(snapshot.trace_commitment.to_le_bytes()))
            .expect("serialize FASTPQ trace commitment"),
    );
    object.insert(
        "proof_digest".to_owned(),
        norito::json::to_value(&snapshot.proof_digest.to_string())
            .expect("serialize FASTPQ proof digest"),
    );
    object.insert(
        "proof".to_owned(),
        norito::json::to_value(&base64::engine::general_purpose::STANDARD.encode(&snapshot.proof))
            .expect("serialize FASTPQ proof"),
    );
    match fastpq_committed_batch_base64(kura, height, snapshot, artifact_bytes)? {
        Some((batch, reconstructed)) => {
            object.insert(
                "batch".to_string(),
                norito::json::to_value(&batch).expect("serialize FASTPQ batch"),
            );
            object.insert(
                "batch_compact".to_string(),
                norito::json::to_value(&false).expect("serialize FASTPQ batch compact flag"),
            );
            object.insert(
                "batch_reconstructed_from_block".to_string(),
                norito::json::to_value(&reconstructed)
                    .expect("serialize FASTPQ batch reconstruction flag"),
            );
        }
        None => {
            object.insert(
                "batch_compact".to_string(),
                norito::json::to_value(&snapshot.batch.transitions.is_empty())
                    .expect("serialize FASTPQ batch compact flag"),
            );
            if snapshot.transition_count > 0 && snapshot.batch.transitions.is_empty() {
                object.insert(
                    "batch_reconstruction_error".to_string(),
                    norito::json::to_value(
                        "committed block transcripts were not available for this FASTPQ proof",
                    )
                    .expect("serialize FASTPQ batch reconstruction error"),
                );
            }
        }
    }
    Ok(norito::json::Value::Object(object))
}
fn fastpq_committed_batch_base64(
    kura: &Kura,
    height: u64,
    snapshot: &iroha_core::kura::FastpqProofSnapshot,
    artifact_bytes: &mut usize,
) -> Result<Option<(String, bool)>, Error> {
    if snapshot.transition_count == 0 {
        return Ok(None);
    }
    if !snapshot.batch.transitions.is_empty() {
        return encode_fastpq_recovery_batch(&snapshot.batch, false, artifact_bytes).map(Some);
    }
    let Some(height) = usize::try_from(height).ok().and_then(NonZeroUsize::new) else {
        iroha_logger::warn!(
            height,
            "cannot reconstruct FASTPQ batch for invalid block height"
        );
        return Ok(None);
    };
    let Some(block) = kura.get_block(height) else {
        iroha_logger::warn!(
            height = height.get(),
            entry_hash = %snapshot.entry_hash,
            "cannot reconstruct FASTPQ batch because committed block is unavailable"
        );
        return Ok(None);
    };
    let Some(transcripts) = block.fastpq_transcripts().get(&snapshot.entry_hash) else {
        iroha_logger::warn!(
            height = height.get(),
            entry_hash = %snapshot.entry_hash,
            "cannot reconstruct FASTPQ batch because committed block has no matching transcript"
        );
        return Ok(None);
    };
    match iroha_core::fastpq::batch_from_transcript_bundle(
        snapshot.parameter.clone(),
        snapshot.batch.public_inputs,
        snapshot.entry_hash,
        transcripts,
    ) {
        Ok(batch) => encode_fastpq_recovery_batch(&batch, true, artifact_bytes).map(Some),
        Err(err) => {
            iroha_logger::warn!(
                height = height.get(),
                entry_hash = %snapshot.entry_hash,
                ?err,
                "failed to reconstruct FASTPQ batch from committed transcripts"
            );
            Ok(None)
        }
    }
}
fn encode_fastpq_recovery_batch(
    batch: &fastpq_prover::TransitionBatch,
    reconstructed: bool,
    artifact_bytes: &mut usize,
) -> Result<(String, bool), Error> {
    let bytes =
        match norito::core::to_bytes_bounded(batch, PIPELINE_FASTPQ_RECOVERY_MAX_BATCH_BYTES) {
            Ok(bytes) => bytes,
            Err(norito::core::BoundedEncodeError::FrameTooLarge { .. }) => {
                return Err(fastpq_recovery_capacity_error(format!(
                    "FASTPQ batch exceeds the {} byte per-batch budget",
                    PIPELINE_FASTPQ_RECOVERY_MAX_BATCH_BYTES
                )));
            }
            Err(source) => {
                return Err(Error::SerializationFailure {
                    context: "pipeline_recovery_fastpq_batch",
                    source: Box::new(source),
                });
            }
        };
    charge_fastpq_recovery_artifact_bytes(artifact_bytes, bytes.len())?;
    Ok((
        base64::engine::general_purpose::STANDARD.encode(bytes),
        reconstructed,
    ))
}
#[derive(JsonDeserialize, crate::json_macros::JsonSerialize, Clone, Debug)]
#[norito(deny_unknown_fields)]
struct PipelineStatusQuery {
    #[norito(default)]
    hash: Option<String>,
    #[norito(default)]
    scope: Option<String>,
}
#[derive(JsonDeserialize, crate::json_macros::JsonSerialize, Clone, Debug)]
struct TriggerCompletionQuery {
    #[norito(default)]
    id: Option<String>,
    #[norito(default)]
    entrypoint_hash: Option<String>,
    #[norito(default)]
    outcome: Option<String>,
    #[norito(default)]
    from_height: Option<u64>,
    #[norito(default)]
    to_height: Option<u64>,
    #[norito(default)]
    limit: Option<u64>,
    #[norito(default)]
    scan_limit_blocks: Option<u64>,
    #[norito(default)]
    include_reconstructed: Option<bool>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TriggerCompletionOutcomeFilter {
    All,
    Success,
    Failure,
}
impl TriggerCompletionOutcomeFilter {
    fn parse(raw: Option<&str>) -> Result<Self, Error> {
        let normalized = raw
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("all")
            .to_ascii_lowercase();
        match normalized.as_str() {
            "all" | "*" => Ok(Self::All),
            "success" | "ok" => Ok(Self::Success),
            "failure" | "failed" | "error" => Ok(Self::Failure),
            _ => Err(conversion_error(format!(
                "invalid outcome query parameter \"{normalized}\" (expected all|success|failure)"
            ))),
        }
    }
    fn matches(self, outcome: &str) -> bool {
        match self {
            Self::All => true,
            Self::Success => matches!(outcome, "Success"),
            Self::Failure => matches!(outcome, "Failure"),
        }
    }
}
const TRIGGER_COMPLETION_DEFAULT_LIMIT: u64 = 100;
const TRIGGER_COMPLETION_MAX_LIMIT: u64 = 1_000;
const TRIGGER_COMPLETION_DEFAULT_SCAN_BLOCKS: u64 = 1_000;
const TRIGGER_COMPLETION_MAX_SCAN_BLOCKS: u64 = 10_000;
fn trigger_completion_outcome(outcome: &TriggerCompletedOutcome) -> (&'static str, Option<String>) {
    match outcome {
        TriggerCompletedOutcome::Success => ("Success", None),
        TriggerCompletedOutcome::Failure(message) => ("Failure", Some(message.clone())),
    }
}
fn trigger_completion_summary_from_event(
    event: &TriggerCompletedEvent,
) -> TriggerCompletionSummary {
    let (outcome, message) = trigger_completion_outcome(event.outcome());
    TriggerCompletionSummary {
        trigger_id: event.trigger_id().to_string(),
        trigger_execution_hash: event.trigger_execution_hash().to_string(),
        step_index: *event.step_index(),
        outcome: outcome.to_owned(),
        message,
    }
}
fn trigger_completion_record_from_event(
    block: &iroha_data_model::block::SignedBlock,
    block_height: u64,
    event: &TriggerCompletedEvent,
    source: &str,
) -> TriggerCompletionRecord {
    let entrypoint_index = block
        .entrypoint_hashes()
        .position(|hash| hash == *event.trigger_execution_hash())
        .and_then(|index| u64::try_from(index).ok());
    TriggerCompletionRecord {
        block_height,
        entrypoint_index,
        completion: trigger_completion_summary_from_event(event),
        source: source.to_owned(),
    }
}
fn trigger_completion_record_from_parts(
    block_height: u64,
    entrypoint_index: usize,
    trigger_id: String,
    trigger_execution_hash: String,
    step_index: u32,
    outcome: &str,
    message: Option<String>,
    source: &str,
) -> TriggerCompletionRecord {
    TriggerCompletionRecord {
        block_height,
        entrypoint_index: u64::try_from(entrypoint_index).ok(),
        completion: TriggerCompletionSummary {
            trigger_id,
            trigger_execution_hash,
            step_index,
            outcome: outcome.to_owned(),
            message,
        },
        source: source.to_owned(),
    }
}
fn visit_reconstructed_trigger_completion_records<F>(
    block: &iroha_data_model::block::SignedBlock,
    block_height: u64,
    visit: &mut F,
) -> bool
where
    F: FnMut(TriggerCompletionRecord) -> bool,
{
    for (entrypoint_index, entrypoint, result) in block.entrypoint_results() {
        let execution_hash = entrypoint.hash().to_string();
        if let TransactionEntrypoint::Time(time_entrypoint) = &entrypoint {
            let record = match &result.0 {
                Ok(_) => trigger_completion_record_from_parts(
                    block_height,
                    entrypoint_index,
                    time_entrypoint.id.to_string(),
                    execution_hash.clone(),
                    0,
                    "Success",
                    None,
                    "reconstructed_result",
                ),
                Err(reason) => trigger_completion_record_from_parts(
                    block_height,
                    entrypoint_index,
                    time_entrypoint.id.to_string(),
                    execution_hash.clone(),
                    0,
                    "Failure",
                    Some(reason.to_string()),
                    "reconstructed_result",
                ),
            };
            if !visit(record) {
                return false;
            }
        }
        let Ok(sequence) = &result.0 else {
            continue;
        };
        let first_data_step = if matches!(entrypoint, TransactionEntrypoint::Time(_)) {
            1_u32
        } else {
            0_u32
        };
        for (offset, step) in sequence.iter().enumerate() {
            let step_index =
                first_data_step.saturating_add(u32::try_from(offset).unwrap_or(u32::MAX));
            let record = trigger_completion_record_from_parts(
                block_height,
                entrypoint_index,
                step.id.to_string(),
                execution_hash.clone(),
                step_index,
                "Success",
                None,
                "reconstructed_result",
            );
            if !visit(record) {
                return false;
            }
        }
    }
    true
}
fn trigger_completion_record_matches(
    record: &TriggerCompletionRecord,
    trigger_id: Option<&str>,
    entrypoint_hash: Option<&str>,
    outcome: TriggerCompletionOutcomeFilter,
) -> bool {
    if let Some(trigger_id) = trigger_id
        && record.completion.trigger_id != trigger_id
    {
        return false;
    }
    if let Some(entrypoint_hash) = entrypoint_hash
        && record.completion.trigger_execution_hash != entrypoint_hash
    {
        return false;
    }
    outcome.matches(&record.completion.outcome)
}
fn visit_trigger_completion_records_for_block<F>(
    block: &iroha_data_model::block::SignedBlock,
    block_height: u64,
    include_reconstructed: bool,
    entrypoint_hash: Option<&str>,
    mut visit: F,
) -> bool
where
    F: FnMut(TriggerCompletionRecord) -> bool,
{
    let persisted = block.trigger_completions().unwrap_or_default();
    let reconstruct = include_reconstructed
        && (persisted.is_empty()
            || entrypoint_hash.is_some_and(|entrypoint_hash| {
                !persisted
                    .iter()
                    .any(|event| event.trigger_execution_hash().to_string() == entrypoint_hash)
            }));
    if reconstruct {
        return visit_reconstructed_trigger_completion_records(block, block_height, &mut visit);
    }
    persisted.iter().all(|event| {
        visit(trigger_completion_record_from_event(
            block,
            block_height,
            event,
            "block_result",
        ))
    })
}
fn trigger_completion_from_height(query: &TriggerCompletionQuery, requested_to: u64) -> u64 {
    let scan_limit = query
        .scan_limit_blocks
        .unwrap_or(TRIGGER_COMPLETION_DEFAULT_SCAN_BLOCKS)
        .clamp(1, TRIGGER_COMPLETION_MAX_SCAN_BLOCKS);
    let bounded_from = requested_to
        .saturating_sub(scan_limit.saturating_sub(1))
        .max(1);
    query.from_height.unwrap_or(1).max(1).max(bounded_from)
}
fn trigger_completion_query_response(
    app: &SharedAppState,
    query: &TriggerCompletionQuery,
) -> Result<TriggerCompletionListResponse, Error> {
    let latest_height = u64::try_from(app.state.committed_height()).unwrap_or(u64::MAX);
    let limit = query
        .limit
        .unwrap_or(TRIGGER_COMPLETION_DEFAULT_LIMIT)
        .clamp(1, TRIGGER_COMPLETION_MAX_LIMIT);
    let include_reconstructed = query.include_reconstructed.unwrap_or(true);
    let outcome = TriggerCompletionOutcomeFilter::parse(query.outcome.as_deref())?;
    let requested_to = query.to_height.unwrap_or(latest_height).min(latest_height);
    if latest_height == 0 || requested_to == 0 {
        return Ok(TriggerCompletionListResponse {
            latest_height,
            from_height: 0,
            to_height: requested_to,
            scanned_blocks: 0,
            limit,
            completions: Vec::new(),
        });
    }
    let from_height = trigger_completion_from_height(query, requested_to);
    if from_height > requested_to {
        return Ok(TriggerCompletionListResponse {
            latest_height,
            from_height,
            to_height: requested_to,
            scanned_blocks: 0,
            limit,
            completions: Vec::new(),
        });
    }
    let mut completions = Vec::new();
    let mut scanned_blocks = 0_u64;
    for height in (from_height..=requested_to).rev() {
        scanned_blocks = scanned_blocks.saturating_add(1);
        let Some(height_usize) = usize::try_from(height).ok().and_then(NonZeroUsize::new) else {
            continue;
        };
        let Some(block) = app.kura.get_block(height_usize) else {
            continue;
        };
        let mut reached_limit = false;
        visit_trigger_completion_records_for_block(
            block.as_ref(),
            height,
            include_reconstructed,
            query.entrypoint_hash.as_deref(),
            |record| {
                if !trigger_completion_record_matches(
                    &record,
                    query.id.as_deref(),
                    query.entrypoint_hash.as_deref(),
                    outcome,
                ) {
                    return true;
                }
                completions.push(record);
                reached_limit = u64::try_from(completions.len()).unwrap_or(u64::MAX) >= limit;
                !reached_limit
            },
        );
        if reached_limit {
            return Ok(TriggerCompletionListResponse {
                latest_height,
                from_height,
                to_height: requested_to,
                scanned_blocks,
                limit,
                completions,
            });
        }
    }
    Ok(TriggerCompletionListResponse {
        latest_height,
        from_height,
        to_height: requested_to,
        scanned_blocks,
        limit,
        completions,
    })
}
fn trigger_completion_summaries_for_entrypoint_hash(
    app: &SharedAppState,
    block_height: u64,
    entrypoint_hash: &str,
) -> Vec<TriggerCompletionSummary> {
    let query = TriggerCompletionQuery {
        id: None,
        entrypoint_hash: Some(entrypoint_hash.to_owned()),
        outcome: None,
        from_height: Some(block_height),
        to_height: Some(block_height),
        limit: Some(TRIGGER_COMPLETION_MAX_LIMIT),
        scan_limit_blocks: Some(1),
        include_reconstructed: Some(true),
    };
    trigger_completion_query_response(app, &query)
        .map(|response| {
            response
                .completions
                .into_iter()
                .map(|record| record.completion)
                .collect()
        })
        .unwrap_or_default()
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PipelineStatusReadScope {
    Local,
    Global,
}
impl PipelineStatusReadScope {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Global => "global",
        }
    }
}
fn parse_pipeline_status_scope(raw: Option<&str>) -> Result<PipelineStatusReadScope, Error> {
    let scope = raw.unwrap_or("global");
    match scope {
        "local" => Ok(PipelineStatusReadScope::Local),
        "global" => Ok(PipelineStatusReadScope::Global),
        _ => Err(conversion_error(format!(
            "invalid scope query parameter \"{scope}\" (expected local|global)"
        ))),
    }
}
fn parse_signed_transaction_hash(raw: &str) -> Result<HashOf<SignedTransaction>, Error> {
    if raw.is_empty() || raw.trim() != raw {
        return Err(conversion_error(
            "signed transaction hash must use exact canonical lowercase text".to_owned(),
        ));
    }
    let hash = raw
        .parse::<HashOf<SignedTransaction>>()
        .map_err(|_| conversion_error("invalid signed transaction hash".to_owned()))?;
    if hash.to_string() != raw {
        return Err(conversion_error(
            "signed transaction hash must use exact canonical lowercase text".to_owned(),
        ));
    }
    Ok(hash)
}
fn pipeline_status_response(
    hash: &HashOf<SignedTransaction>,
    entry: &PipelineStatusEntry,
    scope: PipelineStatusReadScope,
    resolved_from: &'static str,
) -> PipelineTransactionStatusResponse {
    PipelineTransactionStatusResponse::new(
        hash.to_string(),
        PipelineTransactionStatus {
            kind: entry.kind.as_str().to_owned(),
            block_height: entry.block_height.map(NonZeroU64::get),
        },
        scope.as_str().to_owned(),
        resolved_from.to_owned(),
    )
}
fn pipeline_status_projection_error(message: impl std::fmt::Display) -> Error {
    Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
            "committed transaction status projection is inconsistent: {message}"
        )),
    ))
}
fn certified_merge_pipeline_transactions(
    carrier_hash: HashOf<BlockHeader>,
    reference: &iroha_data_model::block::CertifiedMergeLedgerReference,
    entry: &iroha_data_model::merge::MergeLedgerEntry,
) -> Result<
    Vec<(
        HashOf<TransactionEntrypoint>,
        Option<HashOf<SignedTransaction>>,
        iroha_data_model::query::CommittedTransaction,
    )>,
    Error,
> {
    let transactions = iroha_core::smartcontracts::isi::tx::certified_merge_committed_transactions(
        carrier_hash,
        reference,
        entry,
    )
    .map_err(pipeline_status_projection_error)?;
    let batch = entry.execution_batch.as_ref().ok_or_else(|| {
        pipeline_status_projection_error(
            "execution carrier references an entry without an execution batch",
        )
    })?;
    let membership_identities = batch
        .lanes
        .iter()
        .flat_map(|execution| {
            execution.entrypoints.iter().map(|entrypoint| {
                (
                    entrypoint.hash(),
                    signed_transaction_hash_for_entrypoint(entrypoint),
                )
            })
        })
        .collect::<Vec<_>>();
    if transactions.len() != membership_identities.len() {
        return Err(pipeline_status_projection_error(format!(
            "authenticated transcript has {} transactions but State membership has {} hashes",
            transactions.len(),
            membership_identities.len()
        )));
    }
    Ok(membership_identities
        .into_iter()
        .rev()
        .zip(transactions)
        .map(
            |((entrypoint_hash, signed_transaction_hash), transaction)| {
                (entrypoint_hash, signed_transaction_hash, transaction)
            },
        )
        .collect())
}
#[derive(Clone, Debug)]
enum CanonicalTransactionOutcome {
    Applied {
        height: NonZeroU64,
        settled_at: SystemTime,
    },
    Rejected {
        height: NonZeroU64,
        reason: TransactionRejectionReason,
    },
}
impl CanonicalTransactionOutcome {
    fn into_pipeline_status_entry(self) -> PipelineStatusEntry {
        match self {
            Self::Applied { height, .. } => {
                PipelineStatusEntry::fresh(PipelineStatusKind::Applied, Some(height), None)
            }
            Self::Rejected { height, reason } => PipelineStatusEntry::fresh(
                PipelineStatusKind::Rejected,
                Some(height),
                Some(pipeline_rejection_summary(&reason)),
            ),
        }
    }
}
fn canonical_transaction_outcome(
    state: &CoreState,
    kura: &Kura,
    hash: &HashOf<SignedTransaction>,
) -> Result<Option<CanonicalTransactionOutcome>, Error> {
    let entrypoint_hash = iroha_core::tx::external_entrypoint_hash_from_signed_hash(hash.clone());
    let state_view = state.view();
    let Some(height) = state_view.transactions.get(&entrypoint_hash) else {
        return Ok(None);
    };
    let height_u64 = u64::try_from(height.get())
        .map_err(|_| pipeline_status_projection_error("committed height exceeds u64"))?;
    let height_nz = NonZeroU64::new(height_u64)
        .ok_or_else(|| pipeline_status_projection_error("committed height is zero"))?;
    let expected_hash = state_view
        .block_hashes()
        .get(height.get().saturating_sub(1))
        .copied()
        .ok_or_else(|| {
            pipeline_status_projection_error(format!(
                "transaction {hash} is indexed beyond the committed block-hash journal at height {}",
                height.get()
            ))
        })?;
    let block = kura.get_block(height).ok_or_else(|| {
        pipeline_status_projection_error(format!("canonical block {} is unavailable", height.get()))
    })?;
    let block_ref = block.as_ref();
    if block_ref.header().height() != height_nz {
        return Err(pipeline_status_projection_error(format!(
            "Kura returned block height {} for indexed height {}",
            block_ref.header().height(),
            height.get()
        )));
    }
    if block_ref.hash() != expected_hash {
        return Err(pipeline_status_projection_error(format!(
            "Kura block hash at height {} does not match the committed State journal",
            height.get()
        )));
    }
    let settled_at = UNIX_EPOCH
        .checked_add(block_ref.header().creation_time())
        .ok_or_else(|| {
            pipeline_status_projection_error(format!(
                "block {} creation time exceeds SystemTime",
                height.get()
            ))
        })?;
    let mut direct_result = None;
    for (index, entrypoint, result) in block_ref.entrypoint_results() {
        if index >= block_ref.external_entrypoint_count() {
            break;
        }
        if !transaction_entrypoint_matches_indexed_identity(&entrypoint, &entrypoint_hash) {
            continue;
        }
        if direct_result.replace(result).is_some() {
            return Err(pipeline_status_projection_error(format!(
                "transaction {hash} occurs more than once in canonical block {}",
                height.get()
            )));
        }
    }
    if let Some(result) = direct_result {
        return Ok(Some(match &result.0 {
            Ok(_) => CanonicalTransactionOutcome::Applied {
                height: height_nz,
                settled_at,
            },
            Err(reason) => CanonicalTransactionOutcome::Rejected {
                height: height_nz,
                reason: reason.clone(),
            },
        }));
    }
    let reference = block_ref
        .execution_context()
        .and_then(|context| context.merge_entry.as_ref())
        .ok_or_else(|| {
            pipeline_status_projection_error(format!(
                "transaction {hash} is indexed at block {} but is absent from its external body and has no merge reference",
                height.get()
            ))
        })?;
    let entry = kura
        .get_merge_entry_by_carrier_height(height)
        .map_err(pipeline_status_projection_error)?
        .ok_or_else(|| {
            pipeline_status_projection_error(format!(
                "block {} has a merge reference but no canonical sidecar",
                height.get()
            ))
        })?;
    let transactions = certified_merge_pipeline_transactions(block_ref.hash(), reference, &entry)?;
    let mut matches = transactions.iter().filter(|(_, _, transaction)| {
        transaction_entrypoint_matches_indexed_identity(transaction.entrypoint(), &entrypoint_hash)
    });
    let transaction = matches.next().map(|(_, _, transaction)| transaction).ok_or_else(|| {
            pipeline_status_projection_error(format!(
                "transaction {hash} is indexed at merge carrier {} but its authenticated transcript does not contain it",
                height.get()
            ))
        })?;
    if matches.next().is_some() {
        return Err(pipeline_status_projection_error(format!(
            "transaction {hash} occurs more than once in merge carrier {}",
            height.get()
        )));
    }
    Ok(Some(match &transaction.result().0 {
        Ok(_) => CanonicalTransactionOutcome::Applied {
            height: height_nz,
            settled_at,
        },
        Err(reason) => CanonicalTransactionOutcome::Rejected {
            height: height_nz,
            reason: reason.clone(),
        },
    }))
}
fn pipeline_status_from_state(
    state: &CoreState,
    kura: &Kura,
    hash: &HashOf<SignedTransaction>,
) -> Result<Option<PipelineStatusEntry>, Error> {
    canonical_transaction_outcome(state, kura, hash)
        .map(|outcome| outcome.map(CanonicalTransactionOutcome::into_pipeline_status_entry))
}
fn pipeline_status_terminal_or_state_entry(
    app: &SharedAppState,
    hash: &HashOf<SignedTransaction>,
) -> Result<Option<(PipelineStatusEntry, &'static str)>, Error> {
    app.pipeline_status_cache.refresh_pending_blocks(&app.kura);
    if let Some(entry) = pipeline_status_from_state(&app.state, &app.kura, hash)? {
        app.pipeline_status_cache
            .record_entry(hash.clone(), entry.clone());
        return Ok(Some((entry, "state")));
    }
    if let Some(entry) = app.pipeline_status_cache.lookup(hash) {
        if entry.kind.is_terminal() {
            return Ok(Some((entry, "cache")));
        }
    }
    Ok(None)
}
fn pipeline_status_local_entry_checked(
    app: &SharedAppState,
    hash: &HashOf<SignedTransaction>,
) -> Result<Option<(PipelineStatusEntry, &'static str)>, Error> {
    if let Some(entry) = pipeline_status_terminal_or_state_entry(app, hash)? {
        return Ok(Some(entry));
    }
    if let Some(entry) = app.pipeline_status_cache.lookup(hash) {
        if entry.kind == PipelineStatusKind::Queued
            && !app.queue.contains_pending_hash(
                iroha_core::tx::external_entrypoint_hash_from_signed_hash(hash.clone()),
                &app.state,
            )
        {
            app.pipeline_status_cache.remove_entry_by_hash(hash);
            return Ok(None);
        }
        return Ok(Some((entry, "cache")));
    }
    if app.queue.contains_pending_hash(
        iroha_core::tx::external_entrypoint_hash_from_signed_hash(hash.clone()),
        &app.state,
    ) {
        let entry = PipelineStatusEntry::fresh(PipelineStatusKind::Queued, None, None);
        app.pipeline_status_cache
            .record_entry(hash.clone(), entry.clone());
        return Ok(Some((entry, "queue")));
    }
    Ok(None)
}
fn pipeline_status_local_entry(
    app: &SharedAppState,
    hash: &HashOf<SignedTransaction>,
) -> Option<(PipelineStatusEntry, &'static str)> {
    match pipeline_status_local_entry_checked(app, hash) {
        Ok(entry) => entry,
        Err(error) => {
            iroha_logger::error!(
                ?error,
                %hash,
                "internal pipeline-status consumer rejected canonical transaction evidence"
            );
            None
        }
    }
}
fn pipeline_status_response_with_route(
    hash: &HashOf<SignedTransaction>,
    entry: &PipelineStatusEntry,
    scope: PipelineStatusReadScope,
    resolved_from: &'static str,
    format: ResponseFormat,
    route: Option<(RoutingDecision, &'static str)>,
) -> Response {
    let mut response = crate::utils::respond_with_format(
        pipeline_status_response(hash, entry, scope, resolved_from),
        format,
    );
    if let Some((routing_decision, routed_by)) = route {
        insert_routing_headers(&mut response, routing_decision, routed_by);
    }
    response
}
fn pipeline_status_not_found_error() -> Error {
    Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::NotFound,
    ))
}
fn pipeline_status_error_is_not_found(error: &Error) -> bool {
    matches!(
        error,
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::NotFound
        ))
    )
}
fn exact_transaction_details_query_hash(
    request: &iroha_data_model::query::QueryRequestWithAuthority,
) -> Result<HashOf<TransactionEntrypoint>, Error> {
    use iroha_data_model::query::{
        CommittedTxFilters, QueryItemKind, QueryRequest, transaction::prelude::FindTransactions,
    };
    fn exact_hash_from_predicate(
        predicate: &iroha_data_model::query::dsl::CompoundPredicate<CommittedTransaction>,
    ) -> Option<HashOf<TransactionEntrypoint>> {
        let filters = predicate.committed_tx_filters()?;
        let entry_eq = filters.entry_eq.clone()?;
        (filters
            == CommittedTxFilters {
                entry_eq: Some(entry_eq.clone()),
                ..CommittedTxFilters::default()
            })
        .then_some(entry_eq)
    }
    let QueryRequest::Start(query) = request.request() else {
        return Err(conversion_error(
            "transaction details requires a signed FindTransactions start query".to_owned(),
        ));
    };
    if query.params() != &iroha_data_model::query::parameters::QueryParams::default() {
        return Err(conversion_error(
            "transaction details query parameters must use their canonical defaults".to_owned(),
        ));
    }
    let (item_kind, predicate_bytes, selector_bytes, payload) = query.parts();
    if item_kind != QueryItemKind::CommittedTransaction
        || !payload_matches_query::<FindTransactions>(payload)
    {
        return Err(conversion_error(
            "transaction details requires canonical FindTransactions".to_owned(),
        ));
    }
    let predicate = decode_query_payload::<
        iroha_data_model::query::dsl::CompoundPredicate<CommittedTransaction>,
    >(predicate_bytes)
    .ok_or_else(|| conversion_error("transaction details predicate is not canonical".to_owned()))?;
    let selector = decode_query_payload::<
        iroha_data_model::query::dsl::SelectorTuple<CommittedTransaction>,
    >(selector_bytes)
    .ok_or_else(|| conversion_error("transaction details selector is not canonical".to_owned()))?;
    if selector != iroha_data_model::query::dsl::SelectorTuple::<CommittedTransaction>::default() {
        return Err(conversion_error(
            "transaction details query does not accept a projection".to_owned(),
        ));
    }
    let hash = exact_hash_from_predicate(&predicate);
    hash.ok_or_else(|| {
        conversion_error(
            "transaction details requires exactly one entrypoint_hash equality predicate"
                .to_owned(),
        )
    })
}
fn transaction_details_operator_authority(
    world: &impl WorldReadOnly,
    authority: &AccountId,
) -> bool {
    let permission: Permission = CanReadAllLedgerData.into();
    torii_account_has_permission(world, authority, &permission)
}
fn transaction_details_authority_is_involved(
    authority: &AccountId,
    transaction: &CommittedTransaction,
) -> bool {
    transaction.entrypoint().authority() == authority
        || transaction
            .result()
            .batch_transfer_outcomes()
            .iter()
            .any(|outcome| {
                outcome.asset.account() == authority || &outcome.destination == authority
            })
}
fn canonical_carrier_hash_for_indexed_transaction_identity(
    app: &AppState,
    block_height: NonZeroUsize,
    indexed_identity: &HashOf<TransactionEntrypoint>,
) -> Result<HashOf<TransactionEntrypoint>, Error> {
    let block = app.kura.get_block(block_height).ok_or_else(|| {
        pipeline_status_projection_error(format!(
            "canonical block {} is unavailable",
            block_height.get()
        ))
    })?;
    let block_ref = block.as_ref();
    for (index, entrypoint, _) in block_ref.entrypoint_results() {
        if index >= block_ref.external_entrypoint_count() {
            break;
        }
        if transaction_entrypoint_matches_indexed_identity(&entrypoint, indexed_identity) {
            return Ok(entrypoint.hash());
        }
    }
    let reference = block_ref
        .execution_context()
        .and_then(|context| context.merge_entry.as_ref())
        .ok_or_else(|| {
            pipeline_status_projection_error(format!(
                "indexed transaction identity {indexed_identity} is absent from block {}",
                block_height.get()
            ))
        })?;
    let entry = app
        .kura
        .get_merge_entry_by_carrier_height(block_height)
        .map_err(pipeline_status_projection_error)?
        .ok_or_else(|| {
            pipeline_status_projection_error(format!(
                "block {} has a merge reference but no canonical sidecar",
                block_height.get()
            ))
        })?;
    certified_merge_pipeline_transactions(block_ref.hash(), reference, &entry)?
        .into_iter()
        .find(|(_, _, transaction)| {
            transaction_entrypoint_matches_indexed_identity(
                transaction.entrypoint(),
                indexed_identity,
            )
        })
        .map(|(carrier_hash, _, _)| carrier_hash)
        .ok_or_else(|| {
            pipeline_status_projection_error(format!(
                "indexed transaction identity {indexed_identity} is absent from merge carrier {}",
                block_height.get()
            ))
        })
}
fn pipeline_transaction_details_response(
    app: &SharedAppState,
    authority: &AccountId,
    entrypoint_hash: HashOf<TransactionEntrypoint>,
) -> Result<PipelineTransactionDetailsResponse, Error> {
    use iroha_data_model::query::{CommittedTxFilters, dsl::CompoundPredicate};
    let block_height = app
        .state
        .committed_entrypoint_height(&entrypoint_hash)
        .ok_or_else(pipeline_status_not_found_error)?;
    let state_view = app.state.view();
    let world = state_view.world();
    world.account(authority).map_err(|_| {
        Error::Query(iroha_data_model::ValidationFail::NotPermitted(format!(
            "transaction-details authority `{authority}` is not a registered account"
        )))
    })?;
    let is_operator = transaction_details_operator_authority(world, authority);
    drop(state_view);
    let canonical_entrypoint_hash = canonical_carrier_hash_for_indexed_transaction_identity(
        app.as_ref(),
        block_height,
        &entrypoint_hash,
    )?;
    let canonical_entrypoint_hash_text = canonical_entrypoint_hash.to_string();
    let state_view = app.state.view();
    let mut transactions =
        iroha_core::smartcontracts::isi::tx::committed_transactions_indexed_snapshot(
            &state_view,
            CompoundPredicate::from_filters(CommittedTxFilters {
                entry_eq: Some(canonical_entrypoint_hash),
                ..CommittedTxFilters::default()
            }),
        )
        .map_err(pipeline_status_projection_error)?;
    if transactions.len() != 1 {
        return if transactions.is_empty() {
            Err(pipeline_status_not_found_error())
        } else {
            Err(pipeline_status_projection_error(format!(
                "entrypoint hash resolved to {} committed transactions",
                transactions.len()
            )))
        };
    }
    let transaction = transactions
        .pop()
        .expect("length checked before committed transaction extraction");
    if !is_operator && !transaction_details_authority_is_involved(authority, &transaction) {
        return Err(Error::Query(
            iroha_data_model::ValidationFail::NotPermitted(
                "transaction details are restricted to an involved account or operator".to_owned(),
            ),
        ));
    }
    let block_height = u64::try_from(block_height.get())
        .map_err(|_| pipeline_status_projection_error("committed height exceeds u64"))?;
    let hash = entrypoint_hash.to_string();
    Ok(PipelineTransactionDetailsResponse {
        trigger_completions: trigger_completion_summaries_for_entrypoint_hash(
            app,
            block_height,
            &canonical_entrypoint_hash_text,
        ),
        hash,
        transaction,
    })
}
fn pipeline_status_proxy_query(
    hash: &HashOf<SignedTransaction>,
    scope: PipelineStatusReadScope,
) -> Result<Option<String>, Error> {
    encode_torii_proxy_query(&PipelineStatusQuery {
        hash: Some(hash.to_string()),
        scope: Some(scope.as_str().to_owned()),
    })
}
fn execute_pipeline_status_local_read(
    app: &SharedAppState,
    query: &PipelineStatusQuery,
    format: ResponseFormat,
    route: Option<(RoutingDecision, &'static str)>,
) -> Result<Response, Error> {
    let hash_raw = query
        .hash
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| conversion_error("missing hash query parameter".to_owned()))?;
    let read_scope = parse_pipeline_status_scope(query.scope.as_deref())?;
    let hash = parse_signed_transaction_hash(hash_raw)?;
    let local_entry = if matches!(read_scope, PipelineStatusReadScope::Local) {
        pipeline_status_local_entry_checked(app, &hash)?
    } else {
        pipeline_status_terminal_or_state_entry(app, &hash)?
    };
    if let Some((entry, resolved_from)) = local_entry {
        return Ok(pipeline_status_response_with_route(
            &hash,
            &entry,
            read_scope,
            resolved_from,
            format,
            route,
        ));
    }
    Err(pipeline_status_not_found_error())
}
#[cfg(feature = "app_api")]
fn pipeline_status_payload_is_authoritative_hint(
    payload: &PipelineTransactionStatusResponse,
) -> bool {
    match payload.status.kind.as_str() {
        "Applied" | "Rejected" | "Expired" => payload.resolved_from == "state",
        _ => false,
    }
}
#[cfg(feature = "app_api")]
async fn pipeline_status_hinted_global_response(
    response: Response,
    max_response_bytes: usize,
) -> Result<Option<Response>, Response> {
    if should_skip_singleton_routed_query_route_error(&response) {
        return Ok(None);
    }
    if !response.status().is_success() {
        return Ok(Some(response));
    }
    let (parts, body) = response.into_parts();
    let bytes = axum::body::to_bytes(body, max_response_bytes.max(1))
        .await
        .map_err(|error| {
            torii_proxy_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid_proxy_response",
                format!("failed to read hinted pipeline status response: {error}"),
            )
        })?;
    let is_terminal = norito::json::from_slice::<PipelineTransactionStatusResponse>(&bytes)
        .map(|payload| pipeline_status_payload_is_authoritative_hint(&payload))
        .unwrap_or(false);
    let response = Response::from_parts(parts, Body::from(bytes));
    Ok(is_terminal.then_some(response))
}
async fn handler_pipeline_transaction_status(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    crate::NoritoStringQuery(query): crate::NoritoStringQuery<PipelineStatusQuery>,
) -> Result<Response, Error> {
    let remote_ip = remote.ip();
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(format) => format,
        Err(resp) => return Ok(resp),
    };
    check_access_with_rate_limiter(
        &app,
        &headers,
        Some(remote_ip),
        "v1/pipeline/transactions/status",
        &app.pipeline_status_rate_limiter,
    )
    .await?;
    let read_scope = parse_pipeline_status_scope(query.scope.as_deref())?;
    let hash_raw = query
        .hash
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| conversion_error("missing hash query parameter".to_owned()))?;
    let hash = parse_signed_transaction_hash(hash_raw)?;
    match execute_pipeline_status_local_read(&app, &query, format, None) {
        Ok(response) => return Ok(response),
        Err(error) if pipeline_status_error_is_not_found(&error) => {}
        Err(error) => return Err(error),
    }
    if matches!(read_scope, PipelineStatusReadScope::Local) {
        return Err(pipeline_status_not_found_error());
    }
    #[cfg(feature = "app_api")]
    {
        let query_string = pipeline_status_proxy_query(&hash, read_scope)?;
        let entrypoint_hash =
            iroha_core::tx::external_entrypoint_hash_from_signed_hash(hash.clone());
        if let Some(route) = app
            .queue
            .routing_plan_hint(&entrypoint_hash)
            .map(|plan| plan.coordinator_route())
        {
            let hinted = execute_torii_single_route_read(
                &app,
                route,
                ToriiReadEndpointV1::PipelineTransactionStatusGet,
                Vec::new(),
                query_string.clone(),
                Vec::new(),
            )
            .await;
            match pipeline_status_hinted_global_response(hinted, app.torii_proxy_max_response_bytes)
                .await
            {
                Ok(Some(hinted)) => return Ok(hinted),
                Ok(None) => {}
                Err(response) => return Ok(response),
            }
        }
        Ok(execute_torii_fanout_singleton_read(
            &app,
            ToriiReadEndpointV1::PipelineTransactionStatusGet,
            Vec::new(),
            query_string,
            Vec::new(),
        )
        .await)
    }
    #[cfg(not(feature = "app_api"))]
    {
        let _ = headers;
        let _ = remote_ip;
        let _ = hash;
        Err(pipeline_status_not_found_error())
    }
}
async fn handler_pipeline_transaction_details(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    crate::utils::extractors::JsonOrNoritoVersioned(query): crate::utils::extractors::JsonOrNoritoVersioned<
        SignedQuery,
    >,
) -> Result<Response, Error> {
    let format =
        match crate::utils::negotiate_response_format(accept.as_ref().map(|value| &value.0)) {
            Ok(format) => format,
            Err(response) => return Ok(response),
        };
    if !limits::is_allowed_by_cidr(&headers, Some(remote.ip()), &app.api_rate_limit_bypass_nets) {
        admit_signed_query_preauth(app.as_ref(), &headers, Some(remote.ip())).await?;
    }
    // Exact network, freshness, signature, and one-shot nonce checks complete before the first
    // state or Kura access. The signed request itself binds the sole permitted entrypoint hash.
    let verified =
        routing::verify_signed_query_request(query, app.signed_query_admission.as_ref())?;
    let authority = verified.authority().clone();
    let entrypoint_hash = exact_transaction_details_query_hash(&verified)?;
    admit_signed_query_authority(app.as_ref(), &authority).await?;
    let _admission = acquire_signed_query_physical_admission(app.as_ref(), &verified).await?;
    let response = pipeline_transaction_details_response(&app, &authority, entrypoint_hash)?;
    Ok(crate::utils::respond_with_format(response, format))
}
async fn handler_trigger_completions(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    AxQuery(query): AxQuery<TriggerCompletionQuery>,
) -> Result<Response, Error> {
    let format = match crate::utils::negotiate_response_format(accept.as_ref().map(|v| &v.0)) {
        Ok(format) => format,
        Err(resp) => return Ok(resp),
    };
    check_operator_rate_limit(
        &app,
        &headers,
        Some(remote.ip()),
        "v1/triggers/completed",
        true,
    )
    .await?;
    let admission = acquire_query_admission(&app, true).await?;
    let worker = crate::panic_recovery::spawn_blocking_recoverable(move || {
        // Keep the admission permits inside the physical task. Dropping the
        // HTTP future must not free capacity while Kura reconstruction still
        // consumes CPU and memory.
        let result = trigger_completion_query_response(&app, &query);
        (result, admission)
    });
    let (result, _admission) = crate::panic_recovery::join_recoverable(worker)
        .await
        .map_err(|error| Error::AppServiceUnavailable {
            code: "trigger_completion_worker_failed",
            message: error.to_string(),
        })?;
    Ok(crate::utils::respond_with_format(result?, format))
}
async fn handler_policy(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    #[allow(clippy::too_many_arguments, clippy::ref_option)]
    fn build_policy_body(
        require_token: bool,
        fee_policy: &FeePolicy,
        queue_len: u64,
        normal_th: usize,
        stream_th: usize,
        sub_th: usize,
        token_required: bool,
    ) -> axum::response::Response {
        let mut obj = norito::json::Map::new();
        obj.insert(
            "require_api_token".into(),
            norito::json::Value::from(require_token),
        );
        obj.insert(
            "token_required".into(),
            norito::json::Value::from(token_required),
        );
        match fee_policy.asset_id() {
            Some(asset) => obj.insert(
                "fee_asset_id".into(),
                norito::json::Value::from(asset.to_owned()),
            ),
            None => obj.insert("fee_asset_id".into(), norito::json::Value::Null),
        };
        match fee_policy.receiver() {
            Some(r) => obj.insert(
                "fee_receiver".into(),
                norito::json::Value::from(r.to_owned()),
            ),
            None => obj.insert("fee_receiver".into(), norito::json::Value::Null),
        };
        match fee_policy.amount() {
            Some(amount) => obj.insert(
                "fee_amount".into(),
                norito::json::Value::from(amount.to_string()),
            ),
            None => obj.insert("fee_amount".into(), norito::json::Value::Null),
        };
        obj.insert("queue_len".into(), norito::json::Value::from(queue_len));
        obj.insert(
            "rate_limit_threshold".into(),
            norito::json::Value::from(normal_th as u64),
        );
        obj.insert(
            "stream_rate_limit_threshold".into(),
            norito::json::Value::from(stream_th as u64),
        );
        obj.insert(
            "subscription_rate_limit_threshold".into(),
            norito::json::Value::from(sub_th as u64),
        );
        let fees_enabled = fee_policy.is_enabled();
        let enforced = true;
        let stream_shed = (queue_len as usize) >= stream_th;
        let sub_shed = (queue_len as usize) >= sub_th;
        obj.insert(
            "rate_limit_enforced".into(),
            norito::json::Value::from(enforced),
        );
        obj.insert(
            "stream_rate_limit_enforced".into(),
            norito::json::Value::from(true),
        );
        obj.insert(
            "subscription_rate_limit_enforced".into(),
            norito::json::Value::from(true),
        );
        obj.insert(
            "stream_admission_shed".into(),
            norito::json::Value::from(stream_shed),
        );
        obj.insert(
            "subscription_admission_shed".into(),
            norito::json::Value::from(sub_shed),
        );
        let explain = format!(
            "rate_limits_always_on=true, fees_enabled={}, queue_len={}, high_load_admission_shed_thresholds(normal={}, stream={}, subscription={})",
            fees_enabled, queue_len, normal_th, stream_th, sub_th
        );
        obj.insert("explain".into(), norito::json::Value::from(explain));
        let body = norito::json::to_json_pretty(&norito::json::Value::Object(obj))
            .unwrap_or_else(|_| "{}".into());
        let mut resp = axum::response::Response::new(axum::body::Body::from(body));
        resp.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        resp
    }
    let queue_len = app.queue.active_len() as u64;
    let token_required = app.require_api_token;
    let enforce_rate =
        !limits::is_allowed_by_cidr(&headers, Some(remote.ip()), &app.api_rate_limit_bypass_nets);
    check_operator_rate_limit(&app, &headers, Some(remote.ip()), "v1/policy", enforce_rate).await?;
    Ok(build_policy_body(
        app.require_api_token,
        &app.fee_policy,
        queue_len,
        app.high_load_tx_threshold,
        app.high_load_stream_tx_threshold,
        app.high_load_subscription_tx_threshold,
        token_required,
    ))
}
