//! Authenticated current-root role collections with ordinary collection custody.

use super::*;
use axum::extract::Path;
use routing::collection_sources::CollectionTarget;

/// List complete role objects after canonical account authentication.
pub(super) async fn roles(
    State(app): State<SharedAppState>,
    Extension(verified): Extension<app_auth::VerifiedCanonicalRequest>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    JsonOnly(body): JsonOnly<norito::json::Value>,
) -> Result<Response, Error> {
    execute(
        app,
        verified,
        headers,
        remote,
        CollectionTarget::Roles,
        body,
    )
    .await
}

/// List the exact role assignments of the canonical account in the path.
pub(super) async fn account_roles(
    State(app): State<SharedAppState>,
    Extension(verified): Extension<app_auth::VerifiedCanonicalRequest>,
    Path(account): Path<String>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(remote): axum::extract::ConnectInfo<std::net::SocketAddr>,
    JsonOnly(body): JsonOnly<norito::json::Value>,
) -> Result<Response, Error> {
    execute(
        app,
        verified,
        headers,
        remote,
        CollectionTarget::AccountRoles(account),
        body,
    )
    .await
}

async fn execute(
    app: SharedAppState,
    verified: app_auth::VerifiedCanonicalRequest,
    headers: HeaderMap,
    remote: std::net::SocketAddr,
    mut target: CollectionTarget,
    body: norito::json::Value,
) -> Result<Response, Error> {
    let caller = ToriiAccountReadVisibility::Signed(verified.account);
    let visibility = torii_global_reader_visibility(app.as_ref(), &caller, target.endpoint())?;
    if !limits::is_allowed_by_cidr(&headers, Some(remote.ip()), &app.api_rate_limit_bypass_nets) {
        check_access_enforced(
            &app,
            &headers,
            Some(remote.ip()),
            target.endpoint().trim_start_matches('/'),
            true,
        )
        .await?;
    }
    if let CollectionTarget::AccountRoles(literal) = &target {
        let (account, _) = routing::parse_account_path_segment_with_state(
            &app.state,
            literal,
            &app.telemetry_handle(),
            target.endpoint(),
        )?;
        target = CollectionTarget::AccountRoles(account.to_string());
    }
    execute_scoped_direct_collection_read(&app, target, list_query_from_json(body)?, visibility)
        .await
}
