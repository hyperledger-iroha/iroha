#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for GET /v2/sumeragi/rbc (telemetry-gated)
#![cfg(feature = "telemetry")]

#[tokio::test]
async fn sumeragi_rbc_endpoint_shape() {
    use axum::{Router, routing::get};
    use iroha_config::parameters::actual::TelemetryProfile;
    use iroha_torii::MaybeTelemetry;
    use tower::ServiceExt;

    // Prepare telemetry with some values
    let tel = MaybeTelemetry::for_tests().map_gate(TelemetryProfile::Developer);
    if let Some(telemetry) = tel.telemetry() {
        let metrics = telemetry.metrics().await;
        metrics.sumeragi_rbc_sessions_active.set(2);
        metrics.sumeragi_rbc_sessions_pruned_total.inc();
        metrics.sumeragi_rbc_ready_broadcasts_total.inc();
        metrics
            .sumeragi_rbc_rebroadcast_skipped_total
            .with_label_values(&["ready"])
            .inc();
        metrics.sumeragi_rbc_deliver_broadcasts_total.inc();
        metrics.sumeragi_rbc_payload_bytes_delivered_total.set(123);
        metrics
            .sumeragi_rbc_rebroadcast_skipped_total
            .with_label_values(&["payload"])
            .inc();
    }

    // Build a tiny router with the RBC endpoint handler
    let app = Router::new().route(
        "/v2/sumeragi/rbc",
        get({
            let tel = tel.clone();
            move || {
                let tel = tel.clone();
                async move {
                    iroha_torii::handle_v1_sumeragi_rbc_status(&tel)
                        .await
                        .map(axum::response::IntoResponse::into_response)
                }
            }
        }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v2/sumeragi/rbc")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);

    let body = http_body_util::BodyExt::collect(resp.into_body())
        .await
        .unwrap()
        .to_bytes();
    let s = String::from_utf8(body.to_vec()).unwrap();

    // Parse JSON and check expected keys exist
    let v: norito::json::Value = norito::json::from_str(&s).unwrap();
    assert!(v.get("sessions_active").is_some());
    assert!(v.get("sessions_pruned_total").is_some());
    assert!(v.get("ready_broadcasts_total").is_some());
    assert!(v.get("ready_rebroadcasts_skipped_total").is_some());
    assert!(v.get("deliver_broadcasts_total").is_some());
    assert!(v.get("payload_bytes_delivered_total").is_some());
    assert!(v.get("payload_rebroadcasts_skipped_total").is_some());
}
