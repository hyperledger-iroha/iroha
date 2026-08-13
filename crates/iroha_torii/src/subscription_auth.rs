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
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    AxQuery(p): AxQuery<crate::routing::SubscriptionPlanListParams>,
) -> Result<impl IntoResponse, Error> {
    let remote_ip = remote.ip();
    if limits::is_allowed_by_cidr(&headers, Some(remote_ip), &app.api_rate_limit_bypass_nets) {
        return routing::handle_v1_subscription_plans(app.state.clone(), AxQuery(p)).await;
    }
    let enforce =
        app.fee_policy.is_enabled() || app.queue.active_len() >= app.high_load_tx_threshold;
    check_access_enforced(
        &app,
        &headers,
        Some(remote_ip),
        "v1/subscriptions/plans",
        enforce,
    )
    .await?;
    routing::handle_v1_subscription_plans(app.state.clone(), AxQuery(p)).await
}
