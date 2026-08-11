async fn check_access_with_rate_limiter(
    app: &AppState,
    headers: &axum::http::HeaderMap,
    remote: Option<IpAddr>,
    hint: &str,
    rate_limiter: &limits::RateLimiter,
) -> Result<(), Error> {
    validate_api_token(app, headers)?;
    let key = rate_limit_key(headers, remote, hint, app.api_token_enforced());
    if !limits::allow_conditionally(rate_limiter, &key, true).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(())
}

// Operator-signature routes have already completed exact-network authentication
// before entering their handlers. Keep their ingress throttle independent from
// the legacy API-token switch so enabling tokens cannot turn them into a
// two-credential surface.
async fn check_operator_rate_limit(
    app: &AppState,
    headers: &axum::http::HeaderMap,
    remote: Option<IpAddr>,
    hint: &str,
    enforce_rate: bool,
) -> Result<(), Error> {
    let key = rate_limit_key(headers, remote, hint, false);
    if !limits::allow_conditionally(&app.rate_limiter, &key, enforce_rate).await {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::CapacityLimit,
        )));
    }
    Ok(())
}

async fn check_operator_proof_access(
    app: &AppState,
    headers: &axum::http::HeaderMap,
    remote: Option<IpAddr>,
    hint: &'static str,
    cost: u64,
    enforce_rate: bool,
) -> Result<(), Error> {
    check_operator_rate_limit(app, headers, remote, hint, enforce_rate).await?;
    let key = rate_limit_key(headers, remote, hint, false);
    if limits::allow_cost_conditionally(&app.proof_rate_limiter, &key, cost, enforce_rate).await {
        return Ok(());
    }
    app.telemetry
        .with_metrics(|tel| tel.inc_torii_proof_throttle(hint));
    app.telemetry.with_metrics(|tel| {
        tel.observe_torii_proof_request(hint, "rate_limited", 0, std::time::Duration::from_secs(0))
    });
    let retry_after_secs = app.proof_limits.retry_after.as_secs().max(1);
    iroha_logger::warn!(
        %hint,
        %retry_after_secs,
        %key,
        "proof endpoint throttled request"
    );
    Err(Error::ProofRateLimited {
        endpoint: hint,
        retry_after_secs,
    })
}
