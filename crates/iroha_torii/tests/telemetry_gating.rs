#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![doc = "Telemetry gating integration tests exercising profile-based access."]
#![cfg(feature = "telemetry")]
use axum::{http::StatusCode, response::IntoResponse};
use iroha_config::parameters::actual::{LaneRoutingPolicy, TelemetryProfile};
use iroha_core::telemetry::Telemetry;
use iroha_telemetry::metrics::Metrics;
use iroha_torii::{MaybeTelemetry, StatusView, handle_metrics, handle_status};
use std::sync::Arc;
#[path = "fixtures.rs"]
mod fixtures;
fn telemetry_disabled() -> MaybeTelemetry {
    MaybeTelemetry::from_profile(None, TelemetryProfile::Disabled)
}
fn telemetry_for(profile: TelemetryProfile, configure: impl Fn(&Arc<Metrics>)) -> MaybeTelemetry {
    let metrics = fixtures::shared_metrics();
    configure(&metrics);
    let telemetry = Telemetry::new(metrics, true);
    MaybeTelemetry::from_profile(Some(telemetry), profile)
}
#[tokio::test]
async fn disabled_profile_hides_status_and_metrics() {
    let telemetry = telemetry_disabled();
    let status_err = handle_status(
        &telemetry,
        None,
        StatusView::Full,
        LaneRoutingPolicy::default(),
        0,
        None,
    )
    .await
    .unwrap_err();
    assert_eq!(
        status_err.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let metrics_err = handle_metrics(&telemetry).await.unwrap_err();
    assert_eq!(
        metrics_err.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
#[tokio::test]
async fn operator_profile_exposes_status_only() {
    let telemetry = telemetry_for(TelemetryProfile::Operator, |_| {});
    let status_resp = handle_status(
        &telemetry,
        None,
        StatusView::Full,
        LaneRoutingPolicy::default(),
        0,
        None,
    )
    .await
    .unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);
    let metrics_err = handle_metrics(&telemetry).await.unwrap_err();
    assert_eq!(
        metrics_err.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
#[tokio::test]
async fn extended_profile_exposes_prometheus_metrics() {
    let telemetry = telemetry_for(TelemetryProfile::Extended, |metrics| {
        metrics.sumeragi_new_view_publish_total.inc();
    });
    let status_resp = handle_status(
        &telemetry,
        None,
        StatusView::Full,
        LaneRoutingPolicy::default(),
        0,
        None,
    )
    .await
    .unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);
    let prometheus = handle_metrics(&telemetry).await.unwrap();
    assert!(
        !prometheus.trim().is_empty(),
        "expected non-empty Prometheus payload"
    );
}
#[tokio::test]
async fn developer_profile_hides_prometheus_metrics() {
    let developer = telemetry_for(TelemetryProfile::Developer, |_| {});
    let metrics_err = handle_metrics(&developer).await.unwrap_err();
    assert_eq!(
        metrics_err.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}
#[tokio::test]
async fn full_profile_combines_all_capabilities() {
    let telemetry = telemetry_for(TelemetryProfile::Full, |metrics| {
        metrics.sumeragi_new_view_publish_total.inc();
    });
    let status = handle_status(
        &telemetry,
        None,
        StatusView::Full,
        LaneRoutingPolicy::default(),
        0,
        None,
    )
    .await
    .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let prometheus = handle_metrics(&telemetry).await.unwrap();
    assert!(prometheus.contains("sumeragi_new_view_publish_total"));
}
