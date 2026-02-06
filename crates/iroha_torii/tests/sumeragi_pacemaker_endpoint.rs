#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Router-level test for GET /v1/sumeragi/pacemaker (telemetry-gated)
#![cfg(feature = "telemetry")]

#[tokio::test]
async fn sumeragi_pacemaker_endpoint_shape() {
    use axum::{Router, routing::get};
    use iroha_config::parameters::actual::TelemetryProfile;
    use iroha_torii::MaybeTelemetry;
    use tower::ServiceExt;

    // Prepare telemetry with some values
    let telemetry = MaybeTelemetry::for_tests().map_gate(TelemetryProfile::Developer);
    if let Some(tele) = telemetry.telemetry() {
        let metrics = tele.metrics().await;
        metrics.sumeragi_pacemaker_backoff_ms.set(250);
        metrics.sumeragi_pacemaker_rtt_floor_ms.set(100);
        metrics.sumeragi_pacemaker_backoff_multiplier.set(2);
        metrics.sumeragi_pacemaker_rtt_floor_multiplier.set(3);
        metrics.sumeragi_pacemaker_max_backoff_ms.set(10_000);
        metrics.sumeragi_pacemaker_jitter_ms.set(7);
        metrics.sumeragi_pacemaker_jitter_frac_permille.set(50);
        metrics.sumeragi_pacemaker_round_elapsed_ms.set(1234);
        metrics.sumeragi_pacemaker_view_timeout_target_ms.set(2000);
        metrics
            .sumeragi_pacemaker_view_timeout_remaining_ms
            .set(750);
    }

    // Build a tiny router with the pacemaker endpoint handler
    let app = Router::new().route(
        "/v1/sumeragi/pacemaker",
        get({
            let telemetry = telemetry.clone();
            move || {
                let telemetry = telemetry.clone();
                async move { iroha_torii::handle_v1_sumeragi_pacemaker(&telemetry, None).await }
            }
        }),
    );

    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/v1/sumeragi/pacemaker")
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
    for k in [
        "backoff_ms",
        "rtt_floor_ms",
        "jitter_ms",
        "backoff_multiplier",
        "rtt_floor_multiplier",
        "max_backoff_ms",
        "jitter_frac_permille",
        "round_elapsed_ms",
        "view_timeout_target_ms",
        "view_timeout_remaining_ms",
    ] {
        assert!(v.get(k).is_some(), "missing key: {k}");
    }
}
