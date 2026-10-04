/// Require a subscription draft authority to match its authenticated account.
fn require_subscription_draft_account(
    requested: &iroha_data_model::account::AccountId,
    verified: &crate::app_auth::VerifiedCanonicalRequest,
    context: &'static str,
) -> Result<(), Error> {
    require_runtime_governance_account(requested, &verified.account, context)
}
#[cfg(feature = "app_api")]
async fn handler_subscription_plans_list(
    State(app): State<SharedAppState>,
    uri: axum::http::Uri,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<Response, Error> {
    if !limits::is_allowed_by_cidr(&headers, Some(remote.ip()), &app.api_rate_limit_bypass_nets) {
        let enforce =
            app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
        check_access_enforced(
            &app,
            &headers,
            Some(remote.ip()),
            "/v1/subscriptions/plans",
            enforce,
        )
        .await?;
    }
    execute_direct_collection_read(
        &app,
        routing::collection_sources::CollectionTarget::SubscriptionPlans,
        list_query_from_query_string(uri.query())?,
    )
    .await
}

#[cfg(feature = "app_api")]
async fn handler_subscription_plans_query(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    crate::JsonOnly(body): crate::JsonOnly<norito::json::Value>,
) -> Result<Response, Error> {
    if !limits::is_allowed_by_cidr(&headers, Some(remote.ip()), &app.api_rate_limit_bypass_nets) {
        check_access_enforced(
            &app,
            &headers,
            Some(remote.ip()),
            "/v1/subscriptions/plans/query",
            true,
        )
        .await?;
    }
    execute_direct_collection_read(
        &app,
        routing::collection_sources::CollectionTarget::SubscriptionPlans,
        list_query_from_json(body)?,
    )
    .await
}
