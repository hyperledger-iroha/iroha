//! Pre-decode admission for SoraNet privacy telemetry collectors.

use std::net::IpAddr;

#[cfg(feature = "telemetry")]
use axum::extract::Extension;
use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use iroha_torii_shared::ErrorEnvelope;

#[cfg(feature = "telemetry")]
use crate::{
    Error, NoritoJson,
    routing::{self, RecordSoranetPrivacyEventDto, RecordSoranetPrivacyShareDto},
};
use crate::{SharedAppState, limits, utils};

/// Maximum encoded body accepted by either SoraNet privacy ingest route.
pub(crate) const SORANET_PRIVACY_INGEST_MAX_BODY_BYTES: usize = 128 * 1024;
const SORANET_PRIVACY_TOKEN_HEADER: &str = "x-soranet-privacy-token";

#[derive(Clone)]
/// Route-local state for the configured collector credential middleware.
pub(crate) struct SoranetPrivacyCollectorAuthState {
    /// Shared Torii configuration, limiter, and telemetry handles.
    pub(crate) app: SharedAppState,
    /// Stable endpoint label used by rejection telemetry.
    pub(crate) endpoint: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct VerifiedSoranetPrivacyCollector;

fn privacy_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(SORANET_PRIVACY_TOKEN_HEADER)
        .or_else(|| headers.get("x-api-token"))
        .and_then(|value| value.to_str().ok())
}

fn privacy_reject(status: StatusCode, code: &'static str, message: impl Into<String>) -> Response {
    let payload = ErrorEnvelope::new(code, message.into());
    (status, utils::NoritoBody(payload)).into_response()
}

async fn enforce_soranet_privacy_ingest(
    app: &SharedAppState,
    headers: &HeaderMap,
    remote: Option<IpAddr>,
    endpoint: &'static str,
) -> Result<(), Response> {
    let ingest_cfg = &app.soranet_privacy_ingest;
    #[cfg(not(feature = "telemetry"))]
    let _ = endpoint;
    if !ingest_cfg.enabled {
        #[cfg(feature = "telemetry")]
        app.telemetry
            .record_soranet_privacy_ingest_reject(endpoint, "disabled")
            .await;
        return Err(privacy_reject(
            StatusCode::SERVICE_UNAVAILABLE,
            "soranet_privacy_disabled",
            "soranet privacy ingestion is disabled",
        ));
    }

    if app.soranet_privacy_allow_nets.is_empty()
        || !limits::is_allowed_by_cidr(headers, remote, &app.soranet_privacy_allow_nets)
    {
        #[cfg(feature = "telemetry")]
        app.telemetry
            .record_soranet_privacy_ingest_reject(endpoint, "namespace_blocked")
            .await;
        return Err(privacy_reject(
            StatusCode::FORBIDDEN,
            "soranet_privacy_namespace_blocked",
            "submitter not in allowed namespace",
        ));
    }

    let token = privacy_token(headers);
    if ingest_cfg.require_token {
        let Some(token_value) = token else {
            #[cfg(feature = "telemetry")]
            app.telemetry
                .record_soranet_privacy_ingest_reject(endpoint, "missing_token")
                .await;
            return Err(privacy_reject(
                StatusCode::UNAUTHORIZED,
                "soranet_privacy_token_required",
                "missing SoraNet privacy token",
            ));
        };
        if !app.soranet_privacy_tokens.contains(token_value) {
            #[cfg(feature = "telemetry")]
            app.telemetry
                .record_soranet_privacy_ingest_reject(endpoint, "invalid_token")
                .await;
            return Err(privacy_reject(
                StatusCode::UNAUTHORIZED,
                "soranet_privacy_token_invalid",
                "token not authorised for SoraNet privacy ingest",
            ));
        }
    }

    let rate_key = token
        .map(ToOwned::to_owned)
        .or_else(|| remote.map(|ip| ip.to_string()))
        .unwrap_or_else(|| "anonymous".to_owned());
    let enforce_rate = ingest_cfg.rate_per_sec.is_some();
    if !limits::allow_conditionally(&app.soranet_privacy_rate_limiter, &rate_key, enforce_rate)
        .await
    {
        #[cfg(feature = "telemetry")]
        app.telemetry
            .record_soranet_privacy_ingest_reject(endpoint, "rate_limited")
            .await;
        return Err(privacy_reject(
            StatusCode::TOO_MANY_REQUESTS,
            "soranet_privacy_rate_limited",
            "soranet privacy ingest is rate limited",
        ));
    }

    Ok(())
}

/// Authenticate one collector request before any request-body extractor runs.
pub(crate) async fn enforce_soranet_privacy_collector_authentication(
    State(state): State<SoranetPrivacyCollectorAuthState>,
    mut request: axum::http::Request<Body>,
    next: Next,
) -> Response {
    let remote = request
        .extensions()
        .get::<ConnectInfo<std::net::SocketAddr>>()
        .map(|connect| connect.0.ip());
    if let Err(response) =
        enforce_soranet_privacy_ingest(&state.app, request.headers(), remote, state.endpoint).await
    {
        return response;
    }
    request
        .extensions_mut()
        .insert(VerifiedSoranetPrivacyCollector);
    next.run(request).await
}

#[cfg(feature = "telemetry")]
/// Record one authenticated SoraNet privacy event.
pub(super) async fn handler_post_soranet_privacy_event(
    State(app): State<SharedAppState>,
    Extension(_collector): Extension<VerifiedSoranetPrivacyCollector>,
    request: NoritoJson<RecordSoranetPrivacyEventDto>,
) -> Result<Response, Error> {
    routing::handle_post_soranet_privacy_event(app.telemetry.clone(), request)
        .await
        .map(IntoResponse::into_response)
}

#[cfg(feature = "telemetry")]
/// Record one authenticated SoraNet privacy collector share.
pub(super) async fn handler_post_soranet_privacy_share(
    State(app): State<SharedAppState>,
    Extension(_collector): Extension<VerifiedSoranetPrivacyCollector>,
    request: NoritoJson<RecordSoranetPrivacyShareDto>,
) -> Result<Response, Error> {
    routing::handle_post_soranet_privacy_share(app.telemetry.clone(), request)
        .await
        .map(IntoResponse::into_response)
}

#[cfg(all(test, feature = "telemetry"))]
/// Exercise event admission and the authenticated handler in direct unit tests.
pub(crate) async fn test_handler_post_soranet_privacy_event_with_ingress(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    request: NoritoJson<RecordSoranetPrivacyEventDto>,
) -> Result<Response, Error> {
    if let Err(response) =
        enforce_soranet_privacy_ingest(&app, &headers, Some(remote.ip()), "event").await
    {
        return Ok(response);
    }
    handler_post_soranet_privacy_event(
        State(app),
        Extension(VerifiedSoranetPrivacyCollector),
        request,
    )
    .await
}

#[cfg(all(test, feature = "telemetry"))]
/// Exercise share admission and the authenticated handler in direct unit tests.
pub(crate) async fn test_handler_post_soranet_privacy_share_with_ingress(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    ConnectInfo(remote): ConnectInfo<std::net::SocketAddr>,
    request: NoritoJson<RecordSoranetPrivacyShareDto>,
) -> Result<Response, Error> {
    if let Err(response) =
        enforce_soranet_privacy_ingest(&app, &headers, Some(remote.ip()), "share").await
    {
        return Ok(response);
    }
    handler_post_soranet_privacy_share(
        State(app),
        Extension(VerifiedSoranetPrivacyCollector),
        request,
    )
    .await
}
