#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![doc = "Telemetry gating integration tests exercising profile-based access."]
#![cfg(feature = "telemetry")]

use std::sync::Arc;

use axum::{http::StatusCode, response::IntoResponse};
use iroha_config::parameters::actual::TelemetryProfile;
use iroha_core::telemetry::Telemetry;
use iroha_telemetry::metrics::Metrics;
use iroha_torii::{MaybeTelemetry, handle_metrics, handle_status, handle_v1_sumeragi_pacemaker};

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

    let status_err = handle_status(&telemetry, None, None, true)
        .await
        .unwrap_err();
    assert_eq!(
        status_err.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let metrics_err = handle_metrics(&telemetry, true).await.unwrap_err();
    assert_eq!(
        metrics_err.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn operator_profile_exposes_status_only() {
    let telemetry = telemetry_for(TelemetryProfile::Operator, |_| {});

    let status_resp = handle_status(&telemetry, None, None, true).await.unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);

    let metrics_err = handle_metrics(&telemetry, true).await.unwrap_err();
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

    let status_resp = handle_status(&telemetry, None, None, true).await.unwrap();
    assert_eq!(status_resp.status(), StatusCode::OK);

    let prometheus = handle_metrics(&telemetry, true).await.unwrap();
    assert!(
        !prometheus.trim().is_empty(),
        "expected non-empty Prometheus payload"
    );
}

#[tokio::test]
async fn developer_profile_allows_developer_routes_only() {
    let operator = telemetry_for(TelemetryProfile::Operator, |_| {});
    let operator_status = match handle_v1_sumeragi_pacemaker(&operator, None).await {
        Err(err) => err.into_response().status(),
        Ok(_) => panic!("developer endpoints should be gated for operator profile"),
    };
    assert_eq!(operator_status, StatusCode::SERVICE_UNAVAILABLE);

    let developer = telemetry_for(TelemetryProfile::Developer, |metrics| {
        metrics.sumeragi_pacemaker_backoff_ms.set(250);
        metrics.sumeragi_pacemaker_rtt_floor_ms.set(100);
        metrics.sumeragi_pacemaker_max_backoff_ms.set(1_000);
    });
    let response = handle_v1_sumeragi_pacemaker(&developer, None)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let metrics_err = handle_metrics(&developer, true).await.unwrap_err();
    assert_eq!(
        metrics_err.into_response().status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn full_profile_combines_all_capabilities() {
    let telemetry = telemetry_for(TelemetryProfile::Full, |metrics| {
        metrics.block_height.inc_by(21);
        metrics.sumeragi_pacemaker_backoff_ms.set(300);
        metrics.sumeragi_pacemaker_rtt_floor_ms.set(120);
        metrics.sumeragi_pacemaker_max_backoff_ms.set(5_000);
    });

    let prometheus = handle_metrics(&telemetry, true).await.unwrap();
    assert!(prometheus.contains("sumeragi_pacemaker_backoff_ms"));

    let response = handle_v1_sumeragi_pacemaker(&telemetry, None)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
