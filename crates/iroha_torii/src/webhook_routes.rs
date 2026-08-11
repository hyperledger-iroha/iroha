// Operator-authenticated application webhook registry handlers.

#[cfg(feature = "app_api")]
async fn handler_webhooks_create(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    body: crate::utils::extractors::JsonOnly<crate::webhook::WebhookCreate>,
) -> Result<impl IntoResponse, Error> {
    let remote_ip = remote.ip();
    if limits::is_allowed_by_cidr(&headers, Some(remote_ip), &app.api_rate_limit_bypass_nets) {
        return Ok(webhook::handle_create_webhook(body).await);
    }

    check_access_enforced(&app, &headers, Some(remote_ip), "v1/webhooks", true).await?;

    Ok(webhook::handle_create_webhook(body).await)
}

#[cfg(feature = "app_api")]
async fn handler_webhooks_list(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Result<impl IntoResponse, Error> {
    let remote_ip = remote.ip();
    if limits::is_allowed_by_cidr(&headers, Some(remote_ip), &app.api_rate_limit_bypass_nets) {
        return Ok(webhook::handle_list_webhooks().await);
    }

    check_access_enforced(&app, &headers, Some(remote_ip), "v1/webhooks", true).await?;

    Ok(webhook::handle_list_webhooks().await)
}

#[cfg(feature = "app_api")]
async fn handler_webhooks_delete(
    State(app): State<SharedAppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    AxPath(id): AxPath<u64>,
) -> Result<impl IntoResponse, Error> {
    let remote_ip = remote.ip();
    if limits::is_allowed_by_cidr(&headers, Some(remote_ip), &app.api_rate_limit_bypass_nets) {
        return Ok(webhook::handle_delete_webhook(axum::extract::Path(id)).await);
    }

    check_access_enforced(&app, &headers, Some(remote_ip), "v1/webhooks", true).await?;

    Ok(webhook::handle_delete_webhook(axum::extract::Path(id)).await)
}
